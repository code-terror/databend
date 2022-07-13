// Copyright 2021 Datafuse Labs.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use async_trait::async_trait;
use common_ast::udfs::UDFDefinition;
use common_ast::udfs::UDFExprTraverser;
use common_ast::udfs::UDFExprVisitor;
use common_ast::udfs::UDFFetcher;
use common_ast::udfs::UDFParser;
use common_ast::udfs::UDFTransformer;
use common_datavalues::prelude::*;
use common_datavalues::type_coercion::merge_types;
use common_exception::ErrorCode;
use common_exception::Result;
use common_functions::aggregates::AggregateFunctionFactory;
use common_functions::is_builtin_function;
use common_meta_types::UserDefinedFunction;
use common_planners::Expression;
use sqlparser::ast::DateTimeField;
use sqlparser::ast::Expr;
use sqlparser::ast::FunctionArgExpr;
use sqlparser::ast::Ident;
use sqlparser::ast::Query;
use sqlparser::ast::UnaryOperator;
use sqlparser::ast::Value;

use crate::sessions::SessionType;
use crate::sql::statements::analyzer_value_expr::ValueExprAnalyzer;
use crate::sql::SQLCommon;

#[derive(Clone)]
pub struct ExpressionSyncAnalyzer {}

impl ExpressionSyncAnalyzer {
    pub fn create() -> ExpressionSyncAnalyzer {
        ExpressionSyncAnalyzer {}
    }

    pub fn analyze(&self, expr: &Expr) -> Result<Expression> {
        let mut stack = Vec::new();

        // Build RPN for expr. Because async function unsupported recursion
        for rpn_item in &ExprRPNBuilder::build(expr, vec![])? {
            match rpn_item {
                ExprRPNItem::Value(v) => Self::analyze_value(v, &mut stack, SessionType::MySQL)?,
                ExprRPNItem::Identifier(v) => self.analyze_identifier(v, &mut stack)?,
                ExprRPNItem::QualifiedIdentifier(v) => self.analyze_identifiers(v, &mut stack)?,
                ExprRPNItem::Function(v) => self.analyze_function(v, &mut stack)?,
                ExprRPNItem::Cast(v, pg_style) => self.analyze_cast(v, *pg_style, &mut stack)?,
                ExprRPNItem::Between(negated) => self.analyze_between(*negated, &mut stack)?,
                ExprRPNItem::InList(v) => self.analyze_inlist(v, &mut stack)?,
                ExprRPNItem::MapAccess(v) => self.analyze_map_access(v, &mut stack)?,
                ExprRPNItem::Array(v) => self.analyze_array(*v, &mut stack)?,

                _ => {
                    return Err(ErrorCode::LogicalError(format!(
                        "Logical error: can't analyze {:?} in sync mode, it's a bug",
                        expr
                    )))
                }
            }
        }

        match stack.len() {
            1 => Ok(stack.remove(0)),
            _ => Err(ErrorCode::LogicalError(
                "Logical error: this is expr rpn bug.",
            )),
        }
    }

    pub fn analyze_function_arg(&self, arg_expr: &FunctionArgExpr) -> Result<Expression> {
        match arg_expr {
            FunctionArgExpr::Expr(expr) => self.analyze(expr),
            FunctionArgExpr::Wildcard => Ok(Expression::Wildcard),
            FunctionArgExpr::QualifiedWildcard(_) => Err(ErrorCode::SyntaxException(std::format!(
                "Unsupported arg statement: {}",
                arg_expr
            ))),
        }
    }

    fn analyze_value(value: &Value, args: &mut Vec<Expression>, typ: SessionType) -> Result<()> {
        args.push(ValueExprAnalyzer::analyze(value, typ)?);
        Ok(())
    }

    fn analyze_inlist(&self, info: &InListInfo, args: &mut Vec<Expression>) -> Result<()> {
        let mut list = Vec::with_capacity(info.list_size);
        for _ in 0..info.list_size {
            match args.pop() {
                None => {
                    return Err(ErrorCode::LogicalError("It's a bug."));
                }
                Some(arg) => {
                    list.insert(0, arg);
                }
            }
        }

        let expr = args
            .pop()
            .ok_or_else(|| ErrorCode::LogicalError("It's a bug."))?;
        list.insert(0, expr);

        let op = if info.negated {
            "NOT_IN".to_string()
        } else {
            "IN".to_string()
        };

        args.push(Expression::ScalarFunction { op, args: list });
        Ok(())
    }

    fn analyze_function(&self, info: &FunctionExprInfo, args: &mut Vec<Expression>) -> Result<()> {
        let mut arguments = Vec::with_capacity(info.args_count);
        for _ in 0..info.args_count {
            match args.pop() {
                None => {
                    return Err(ErrorCode::LogicalError("It's a bug."));
                }
                Some(arg) => {
                    arguments.insert(0, arg);
                }
            }
        }

        args.push(
            match AggregateFunctionFactory::instance().check(&info.name) {
                true => {
                    return Err(ErrorCode::LogicalError(
                        "Unsupport aggregate function, it's a bug.",
                    ))
                }
                false => match info.kind {
                    OperatorKind::Unary => Self::unary_function(info, &arguments),
                    OperatorKind::Binary => Self::binary_function(info, &arguments),
                    OperatorKind::Other => Self::other_function(info, &arguments),
                },
            }?,
        );
        Ok(())
    }

    fn other_function(info: &FunctionExprInfo, args: &[Expression]) -> Result<Expression> {
        let op = info.name.clone();
        let arguments = args.to_owned();
        Ok(Expression::ScalarFunction {
            op,
            args: arguments,
        })
    }

    fn unary_function(info: &FunctionExprInfo, args: &[Expression]) -> Result<Expression> {
        match args.is_empty() {
            true => Err(ErrorCode::LogicalError("Unary operator must be one child.")),
            false => Ok(Expression::UnaryExpression {
                op: info.name.clone(),
                expr: Box::new(args[0].to_owned()),
            }),
        }
    }

    fn binary_function(info: &FunctionExprInfo, args: &[Expression]) -> Result<Expression> {
        let op = info.name.clone();
        match args.len() < 2 {
            true => Err(ErrorCode::LogicalError(
                "Binary operator must be two children.",
            )),
            false => Ok(Expression::BinaryExpression {
                op,
                left: Box::new(args[0].to_owned()),
                right: Box::new(args[1].to_owned()),
            }),
        }
    }

    fn analyze_identifier(&self, ident: &Ident, arguments: &mut Vec<Expression>) -> Result<()> {
        let column_name = ident.clone().value;
        arguments.push(Expression::Column(column_name));
        Ok(())
    }

    fn analyze_identifiers(&self, idents: &[Ident], arguments: &mut Vec<Expression>) -> Result<()> {
        let mut names = Vec::with_capacity(idents.len());

        for ident in idents {
            names.push(ident.clone().value);
        }

        arguments.push(Expression::QualifiedColumn(names));
        Ok(())
    }

    fn analyze_cast(
        &self,
        data_type: &DataTypeImpl,
        pg_style: bool,
        args: &mut Vec<Expression>,
    ) -> Result<()> {
        match args.pop() {
            None => Err(ErrorCode::LogicalError(
                "Cast operator must be one children.",
            )),
            Some(inner_expr) => {
                args.push(Expression::Cast {
                    expr: Box::new(inner_expr),
                    data_type: data_type.clone(),
                    pg_style,
                });
                Ok(())
            }
        }
    }

    fn analyze_between(&self, negated: bool, args: &mut Vec<Expression>) -> Result<()> {
        if args.len() < 3 {
            return Err(ErrorCode::SyntaxException(
                "Between must be a ternary expression.",
            ));
        }

        let s_args = args.split_off(args.len() - 3);
        let expression = s_args[0].clone();
        let low_expression = s_args[1].clone();
        let high_expression = s_args[2].clone();

        match negated {
            false => args.push(
                expression
                    .gt_eq(low_expression)
                    .and(expression.lt_eq(high_expression)),
            ),
            true => args.push(
                expression
                    .lt(low_expression)
                    .or(expression.gt(high_expression)),
            ),
        };

        Ok(())
    }

    fn analyze_map_access(&self, keys: &[Value], args: &mut Vec<Expression>) -> Result<()> {
        match args.pop() {
            None => Err(ErrorCode::LogicalError(
                "MapAccess operator must be one children.",
            )),
            Some(inner_expr) => {
                let path_name: String = keys
                    .iter()
                    .enumerate()
                    .map(|(i, k)| match k {
                        k @ Value::Number(_, _) => format!("[{}]", k),
                        Value::SingleQuotedString(s) => format!("[\"{}\"]", s),
                        Value::ColonString(s) => {
                            if i == 0 {
                                s.to_string()
                            } else {
                                format!(":{}", s)
                            }
                        }
                        Value::PeriodString(s) => format!(".{}", s),
                        _ => format!("[{}]", k),
                    })
                    .collect();

                let name = match keys[0] {
                    Value::ColonString(_) => format!("{}:{}", inner_expr.column_name(), path_name),
                    _ => format!("{}{}", inner_expr.column_name(), path_name),
                };
                let path =
                    Expression::create_literal(DataValue::String(path_name.as_bytes().to_vec()));
                let arguments = vec![inner_expr, path];

                args.push(Expression::MapAccess {
                    name,
                    args: arguments,
                });
                Ok(())
            }
        }
    }

    fn analyze_array(&self, nums: usize, args: &mut Vec<Expression>) -> Result<()> {
        let mut values = Vec::with_capacity(nums);
        let mut types = Vec::with_capacity(nums);
        for _ in 0..nums {
            match args.pop() {
                None => {
                    break;
                }
                Some(inner_expr) => {
                    if let Expression::Literal {
                        value, data_type, ..
                    } = inner_expr
                    {
                        values.push(value);
                        types.push(data_type);
                    }
                }
            };
        }
        if values.len() != nums {
            return Err(ErrorCode::LogicalError(format!(
                "Array must have {} children.",
                nums
            )));
        }
        let inner_type = if types.is_empty() {
            NullType::new_impl()
        } else {
            types
                .iter()
                .fold(Ok(types[0].clone()), |acc, v| merge_types(&acc?, v))
                .map_err(|e| ErrorCode::LogicalError(e.message()))?
        };
        values.reverse();

        let array_value = Expression::create_literal_with_type(
            DataValue::Array(values),
            ArrayType::new_impl(inner_type),
        );
        args.push(array_value);
        Ok(())
    }
}

enum OperatorKind {
    Unary,
    Binary,
    Other,
}

struct FunctionExprInfo {
    name: String,
    args_count: usize,
    kind: OperatorKind,
}

struct InListInfo {
    list_size: usize,
    negated: bool,
}

enum ExprRPNItem {
    Value(Value),
    Identifier(Ident),
    QualifiedIdentifier(Vec<Ident>),
    Function(FunctionExprInfo),
    Wildcard,
    Exists(Box<Query>),
    Subquery(Box<Query>),
    Cast(DataTypeImpl, bool),
    Between(bool),
    InList(InListInfo),
    MapAccess(Vec<Value>),
    Array(usize),
}

impl ExprRPNItem {
    pub fn function(name: String, args_count: usize) -> ExprRPNItem {
        ExprRPNItem::Function(FunctionExprInfo {
            name,
            args_count,
            kind: OperatorKind::Other,
        })
    }

    pub fn binary_operator(name: String) -> ExprRPNItem {
        ExprRPNItem::Function(FunctionExprInfo {
            name,
            args_count: 2,
            kind: OperatorKind::Binary,
        })
    }

    pub fn unary_operator(name: String) -> ExprRPNItem {
        ExprRPNItem::Function(FunctionExprInfo {
            name,
            args_count: 1,
            kind: OperatorKind::Unary,
        })
    }
}

struct ExprRPNBuilder {
    rpn: Vec<ExprRPNItem>,
    udfs: Vec<UserDefinedFunction>,
}

impl ExprRPNBuilder {
    pub fn build(expr: &Expr, udfs: Vec<UserDefinedFunction>) -> Result<Vec<ExprRPNItem>> {
        let mut builder = ExprRPNBuilder {
            rpn: Vec::new(),
            udfs,
        };
        UDFExprTraverser::accept(expr, &mut builder)?;
        Ok(builder.rpn)
    }

    fn process_expr(&mut self, expr: &Expr) -> Result<()> {
        match expr {
            Expr::Value(value) => {
                self.rpn.push(ExprRPNItem::Value(value.clone()));
            }
            Expr::Identifier(ident) => {
                self.rpn.push(ExprRPNItem::Identifier(ident.clone()));
            }
            Expr::CompoundIdentifier(idents) => {
                self.rpn
                    .push(ExprRPNItem::QualifiedIdentifier(idents.to_vec()));
            }
            Expr::IsNull(_) => {
                self.rpn
                    .push(ExprRPNItem::function(String::from("is_null"), 1));
            }
            Expr::IsNotNull(_) => {
                self.rpn
                    .push(ExprRPNItem::function(String::from("is_not_null"), 1));
            }
            Expr::UnaryOp { op, .. } => {
                match op {
                    UnaryOperator::Plus => {}
                    // In order to distinguish it from binary addition.
                    UnaryOperator::Minus => self
                        .rpn
                        .push(ExprRPNItem::unary_operator("NEGATE".to_string())),
                    _ => self.rpn.push(ExprRPNItem::unary_operator(op.to_string())),
                }
            }
            Expr::BinaryOp { op, .. } => {
                self.rpn.push(ExprRPNItem::binary_operator(op.to_string()));
            }
            Expr::Exists(subquery) => {
                self.rpn.push(ExprRPNItem::Exists(subquery.clone()));
            }
            Expr::Subquery(subquery) => {
                self.rpn.push(ExprRPNItem::Subquery(subquery.clone()));
            }
            Expr::Function(function) => {
                self.rpn.push(ExprRPNItem::Function(FunctionExprInfo {
                    name: function.name.to_string(),
                    args_count: function.args.len(),
                    kind: OperatorKind::Other,
                }));
            }
            Expr::Cast {
                data_type,
                pg_style,
                ..
            } => {
                self.rpn.push(ExprRPNItem::Cast(
                    SQLCommon::make_data_type(data_type)?,
                    *pg_style,
                ));
            }
            Expr::TryCast { data_type, .. } => {
                let mut ty = SQLCommon::make_data_type(data_type)?;
                if ty.can_inside_nullable() {
                    ty = NullableType::new_impl(ty)
                }
                self.rpn.push(ExprRPNItem::Cast(ty, false));
            }
            Expr::TypedString { data_type, value } => {
                self.rpn.push(ExprRPNItem::Value(Value::SingleQuotedString(
                    value.to_string(),
                )));
                self.rpn.push(ExprRPNItem::Cast(
                    SQLCommon::make_data_type(data_type)?,
                    false,
                ));
            }
            Expr::Position { .. } => {
                let name = String::from("position");
                self.rpn.push(ExprRPNItem::function(name, 2));
            }
            Expr::Substring {
                substring_from,
                substring_for,
                ..
            } => {
                if substring_from.is_none() {
                    self.rpn
                        .push(ExprRPNItem::Value(Value::Number(String::from("1"), false)));
                }

                let name = String::from("substring");
                match substring_for {
                    None => self.rpn.push(ExprRPNItem::function(name, 2)),
                    Some(_) => {
                        self.rpn.push(ExprRPNItem::function(name, 3));
                    }
                }
            }
            Expr::Between { negated, .. } => {
                self.rpn.push(ExprRPNItem::Between(*negated));
            }
            Expr::Tuple(exprs) => {
                let len = exprs.len();

                if len > 1 {
                    self.rpn
                        .push(ExprRPNItem::function(String::from("tuple"), len));
                }
            }
            Expr::InList {
                expr: _,
                list,
                negated,
            } => self.rpn.push(ExprRPNItem::InList(InListInfo {
                list_size: list.len(),
                negated: *negated,
            })),
            Expr::Extract { field, .. } => match field {
                DateTimeField::Year => self
                    .rpn
                    .push(ExprRPNItem::function(String::from("toYear"), 1)),
                DateTimeField::Month => self
                    .rpn
                    .push(ExprRPNItem::function(String::from("toMonth"), 1)),
                DateTimeField::Day => self
                    .rpn
                    .push(ExprRPNItem::function(String::from("toDayOfMonth"), 1)),
                DateTimeField::Hour => self
                    .rpn
                    .push(ExprRPNItem::function(String::from("toHour"), 1)),
                DateTimeField::Minute => self
                    .rpn
                    .push(ExprRPNItem::function(String::from("toMinute"), 1)),
                DateTimeField::Second => self
                    .rpn
                    .push(ExprRPNItem::function(String::from("toSecond"), 1)),
            },
            Expr::MapAccess { keys, .. } => {
                self.rpn.push(ExprRPNItem::MapAccess(keys.to_owned()));
            }
            Expr::Trim { trim_where, .. } => match trim_where {
                None => self
                    .rpn
                    .push(ExprRPNItem::function(String::from("trim"), 1)),
                Some(_) => {
                    self.rpn
                        .push(ExprRPNItem::function(String::from("trim"), 2));
                }
            },
            Expr::Array(exprs) => {
                self.rpn.push(ExprRPNItem::Array(exprs.len()));
            }
            _ => (),
        }

        Ok(())
    }
}

#[async_trait]
impl UDFFetcher for ExprRPNBuilder {
    fn get_udf_definition(&self, name: &str) -> Result<UDFDefinition> {
        let udf = self.udfs.iter().find(|udf| udf.name == name);

        if let Some(udf) = udf {
            let mut udf_parser = UDFParser::default();
            let definition = udf_parser.parse(&udf.name, &udf.parameters, &udf.definition)?;
            return Ok(UDFDefinition::new(udf.parameters.clone(), definition));
        }
        Err(ErrorCode::UnknownUDF(format!("Unknown Function {}", name)))
    }
}

#[async_trait]
impl UDFExprVisitor for ExprRPNBuilder {
    fn pre_visit(&mut self, expr: &Expr) -> Result<Expr> {
        if let Expr::Function(function) = expr {
            if !is_builtin_function(&function.name.to_string()) {
                return UDFTransformer::transform_function(function, self);
            }
        }

        Ok(expr.clone())
    }

    fn post_visit(&mut self, expr: &Expr) -> Result<()> {
        self.process_expr(expr)
    }

    fn visit_wildcard(&mut self) -> Result<()> {
        self.rpn.push(ExprRPNItem::Wildcard);
        Ok(())
    }
}
