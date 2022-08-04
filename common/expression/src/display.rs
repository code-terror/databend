// Copyright 2022 Datafuse Labs.
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

use std::fmt::Debug;
use std::fmt::Display;
use std::fmt::Formatter;

use comfy_table::Cell;
use comfy_table::Table;
use itertools::Itertools;

use crate::chunk::Chunk;
use crate::expression::Expr;
use crate::expression::Literal;
use crate::expression::RawExpr;
use crate::function::Function;
use crate::function::FunctionSignature;
use crate::property::BooleanDomain;
use crate::property::Domain;
use crate::property::FloatDomain;
use crate::property::FunctionProperty;
use crate::property::IntDomain;
use crate::property::NullableDomain;
use crate::property::StringDomain;
use crate::property::UIntDomain;
use crate::types::AnyType;
use crate::types::DataType;
use crate::types::ValueType;
use crate::values::ScalarRef;
use crate::values::Value;
use crate::values::ValueRef;

impl Debug for Chunk {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut table = Table::new();
        table.load_preset("||--+-++|    ++++++");

        table.set_header(vec!["Column ID", "Column Data"]);

        for (i, col) in self.columns().iter().enumerate() {
            table.add_row(vec![i.to_string(), format!("{:?}", col)]);
        }

        write!(f, "{}", table)
    }
}

impl Display for Chunk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut table = Table::new();
        table.load_preset("||--+-++|    ++++++");

        table.set_header((0..self.num_columns()).map(|idx| format!("Column {idx}")));

        for index in 0..self.num_rows() {
            let row: Vec<_> = self
                .columns()
                .iter()
                .map(|val| val.as_ref().index(index).unwrap().to_string())
                .map(Cell::new)
                .collect();
            table.add_row(row);
        }
        write!(f, "{table}")
    }
}

impl<'a> Display for ScalarRef<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ScalarRef::Null => write!(f, "NULL"),
            ScalarRef::EmptyArray => write!(f, "[]"),
            ScalarRef::Int8(i) => write!(f, "{}", i),
            ScalarRef::Int16(i) => write!(f, "{}", i),
            ScalarRef::Int32(i) => write!(f, "{}", i),
            ScalarRef::Int64(i) => write!(f, "{}", i),
            ScalarRef::UInt8(i) => write!(f, "{}", i),
            ScalarRef::UInt16(i) => write!(f, "{}", i),
            ScalarRef::UInt32(i) => write!(f, "{}", i),
            ScalarRef::UInt64(i) => write!(f, "{}", i),
            ScalarRef::Float32(i) => write!(f, "{:?}", i),
            ScalarRef::Float64(i) => write!(f, "{:?}", i),
            ScalarRef::Boolean(b) => write!(f, "{}", b),
            ScalarRef::String(s) => write!(f, "{:?}", String::from_utf8_lossy(s)),
            ScalarRef::Array(col) => write!(f, "[{}]", col.iter().join(", ")),
            ScalarRef::Tuple(fields) => {
                write!(
                    f,
                    "({})",
                    fields.iter().map(ScalarRef::to_string).join(", ")
                )
            }
        }
    }
}

impl Display for RawExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RawExpr::Literal { lit, .. } => write!(f, "{lit}"),
            RawExpr::ColumnRef { id, data_type, .. } => write!(f, "ColumnRef({id})::{data_type}"),
            RawExpr::Cast {
                expr, dest_type, ..
            } => {
                write!(f, "CAST({expr} AS {dest_type})")
            }
            RawExpr::TryCast {
                expr, dest_type, ..
            } => {
                write!(f, "TRY_CAST({expr} AS {dest_type})")
            }
            RawExpr::FunctionCall {
                name, args, params, ..
            } => {
                write!(f, "{name}")?;
                if !params.is_empty() {
                    write!(f, "(")?;
                    for (i, param) in params.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{param}")?;
                    }
                    write!(f, ")")?;
                }
                write!(f, "(")?;
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{arg}")?;
                }
                write!(f, ")")
            }
        }
    }
}

impl Display for Literal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Literal::Null => write!(f, "NULL"),
            Literal::Boolean(val) => write!(f, "{val}"),
            Literal::UInt8(val) => write!(f, "{val}_u8"),
            Literal::UInt16(val) => write!(f, "{val}_u16"),
            Literal::UInt32(val) => write!(f, "{val}_u32"),
            Literal::UInt64(val) => write!(f, "{val}_u64"),
            Literal::Float32(val) => write!(f, "{val}_f32"),
            Literal::Float64(val) => write!(f, "{val}_f64"),
            Literal::Int8(val) => write!(f, "{val}_i8"),
            Literal::Int16(val) => write!(f, "{val}_i16"),
            Literal::Int32(val) => write!(f, "{val}_i32"),
            Literal::Int64(val) => write!(f, "{val}_i64"),
            Literal::String(val) => write!(f, "{:?}", String::from_utf8_lossy(val)),
        }
    }
}

impl Display for DataType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        match &self {
            DataType::Boolean => write!(f, "Boolean"),
            DataType::String => write!(f, "String"),
            DataType::UInt8 => write!(f, "UInt8"),
            DataType::UInt16 => write!(f, "UInt16"),
            DataType::UInt32 => write!(f, "UInt32"),
            DataType::UInt64 => write!(f, "UInt64"),
            DataType::Int8 => write!(f, "Int8"),
            DataType::Int16 => write!(f, "Int16"),
            DataType::Int32 => write!(f, "Int32"),
            DataType::Int64 => write!(f, "Int64"),
            DataType::Float32 => write!(f, "Float32"),
            DataType::Float64 => write!(f, "Float64"),
            DataType::Null => write!(f, "NULL"),
            DataType::Nullable(inner) => write!(f, "{inner} NULL"),
            DataType::EmptyArray => write!(f, "Array(Nothing)"),
            DataType::Array(inner) => write!(f, "Array({inner})"),
            DataType::Map(inner) => write!(f, "Map({inner})"),
            DataType::Tuple(tys) => {
                if tys.len() == 1 {
                    write!(f, "({},)", tys[0])
                } else {
                    write!(f, "(")?;
                    for (i, ty) in tys.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{ty}")?;
                    }
                    write!(f, ")")
                }
            }
            DataType::Generic(index) => write!(f, "T{index}"),
        }
    }
}

impl Display for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Expr::Constant { scalar, .. } => write!(f, "{}", scalar.as_ref()),
            Expr::ColumnRef { id, .. } => write!(f, "ColumnRef({id})"),
            Expr::Cast {
                expr, dest_type, ..
            } => {
                write!(f, "CAST({expr} AS {dest_type})")
            }
            Expr::TryCast {
                expr, dest_type, ..
            } => {
                write!(f, "TRY_CAST({expr} AS {dest_type})")
            }
            Expr::FunctionCall {
                function,
                args,
                generics,
                ..
            } => {
                write!(f, "{}", function.signature.name)?;
                if !generics.is_empty() {
                    write!(f, "<")?;
                    for (i, ty) in generics.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "T{i}={ty}")?;
                    }
                    write!(f, ">")?;
                }
                write!(f, "<")?;
                for (i, ty) in function.signature.args_type.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{ty}")?;
                }
                write!(f, ">")?;
                write!(f, "(")?;
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{arg}")?;
                }
                write!(f, ")")
            }
        }
    }
}

impl<T: ValueType> Debug for Value<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Scalar(s) => write!(f, "Scalar({:?})", s),
            Value::Column(c) => write!(f, "Column({:?})", c),
        }
    }
}

impl<'a, T: ValueType> Debug for ValueRef<'a, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValueRef::Scalar(s) => write!(f, "Scalar({:?})", s),
            ValueRef::Column(c) => write!(f, "Column({:?})", c),
        }
    }
}

impl<T: ValueType> Display for Value<T> {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            Value::Scalar(scalar) => write!(f, "{:?}", scalar),
            Value::Column(col) => write!(f, "{:?}", col),
        }
    }
}

impl<'a, T: ValueType> Display for ValueRef<'a, T> {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            ValueRef::Scalar(scalar) => write!(f, "{:?}", scalar),
            ValueRef::Column(col) => write!(f, "{:?}", col),
        }
    }
}

impl Debug for Function {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.signature)
    }
}

impl Display for FunctionSignature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}({}) :: {}",
            self.name,
            self.args_type.iter().map(|t| t.to_string()).join(", "),
            self.return_type
        )
    }
}

impl Display for FunctionProperty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut properties = Vec::new();
        if self.commutative {
            properties.push("commutative");
        }
        if !properties.is_empty() {
            write!(f, "{{{}}}", properties.join(", "))?;
        }
        Ok(())
    }
}

impl Display for NullableDomain<AnyType> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        if let Some(value) = &self.value {
            if self.has_null {
                write!(f, "{} ∪ {{NULL}}", value)
            } else {
                write!(f, "{}", value)
            }
        } else {
            assert!(self.has_null);
            write!(f, "{{NULL}}")
        }
    }
}

impl Display for BooleanDomain {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        if self.has_false && self.has_true {
            write!(f, "{{FALSE, TRUE}}")
        } else if self.has_false {
            write!(f, "{{FALSE}}")
        } else {
            write!(f, "{{TRUE}}")
        }
    }
}

impl Display for StringDomain {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        if let Some(max) = &self.max {
            write!(
                f,
                "{{{:?}..={:?}}}",
                String::from_utf8_lossy(&self.min),
                String::from_utf8_lossy(max)
            )
        } else {
            write!(f, "{{{:?}..}}", String::from_utf8_lossy(&self.min))
        }
    }
}

impl Display for IntDomain {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{{{}..={}}}", self.min, self.max)
    }
}

impl Display for UIntDomain {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{{{}..={}}}", self.min, self.max)
    }
}

impl Display for FloatDomain {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{{{:?}..={:?}}}", self.min, self.max)
    }
}

impl Display for Domain {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Domain::Int(domain) => write!(f, "{domain}"),
            Domain::UInt(domain) => write!(f, "{domain}"),
            Domain::Float(domain) => write!(f, "{domain}"),
            Domain::Boolean(domain) => write!(f, "{domain}"),
            Domain::String(domain) => write!(f, "{domain}"),
            Domain::Nullable(domain) => write!(f, "{domain}"),
            Domain::Array(None) => write!(f, "[]"),
            Domain::Array(Some(domain)) => write!(f, "[{domain}]"),
            Domain::Tuple(fields) => {
                write!(f, "(")?;
                for (i, domain) in fields.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{domain}")?;
                }
                write!(f, ")")
            }
            Domain::Undefined => write!(f, "_"),
        }
    }
}
