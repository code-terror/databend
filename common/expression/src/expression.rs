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

use std::sync::Arc;

use serde::Deserialize;
use serde::Serialize;

use crate::function::Function;
use crate::function::FunctionID;
use crate::function::FunctionRegistry;
use crate::types::DataType;
use crate::values::Scalar;

pub type Span = Option<std::ops::Range<usize>>;

#[derive(Debug, Clone)]
pub enum RawExpr {
    Literal {
        span: Span,
        lit: Literal,
    },
    ColumnRef {
        span: Span,
        id: usize,
        data_type: DataType,
    },
    Cast {
        span: Span,
        expr: Box<RawExpr>,
        dest_type: DataType,
    },
    TryCast {
        span: Span,
        expr: Box<RawExpr>,
        dest_type: DataType,
    },
    FunctionCall {
        span: Span,
        name: String,
        params: Vec<usize>,
        args: Vec<RawExpr>,
    },
}

#[derive(Debug, Clone)]
pub enum Expr {
    Constant {
        span: Span,
        scalar: Scalar,
    },
    ColumnRef {
        span: Span,
        id: usize,
    },
    Cast {
        span: Span,
        expr: Box<Expr>,
        dest_type: DataType,
    },
    TryCast {
        span: Span,
        expr: Box<Expr>,
        dest_type: DataType,
    },
    FunctionCall {
        span: Span,
        id: FunctionID,
        function: Arc<Function>,
        generics: Vec<DataType>,
        args: Vec<Expr>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RemoteExpr {
    Constant {
        span: Span,
        scalar: Scalar,
    },
    ColumnRef {
        span: Span,
        id: usize,
    },
    Cast {
        span: Span,
        expr: Box<RemoteExpr>,
        dest_type: DataType,
    },
    TryCast {
        span: Span,
        expr: Box<RemoteExpr>,
        dest_type: DataType,
    },
    FunctionCall {
        span: Span,
        id: FunctionID,
        generics: Vec<DataType>,
        args: Vec<RemoteExpr>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Literal {
    Null,
    Int8(i8),
    Int16(i16),
    Int32(i32),
    Int64(i64),
    UInt8(u8),
    UInt16(u16),
    UInt32(u32),
    UInt64(u64),
    Float32(f32),
    Float64(f64),
    Boolean(bool),
    String(Vec<u8>),
}

impl RemoteExpr {
    pub fn from_expr(expr: Expr) -> Self {
        match expr {
            Expr::Constant { span, scalar } => RemoteExpr::Constant { span, scalar },
            Expr::ColumnRef { span, id } => RemoteExpr::ColumnRef { span, id },
            Expr::Cast {
                span,
                expr,
                dest_type,
            } => RemoteExpr::Cast {
                span,
                expr: Box::new(RemoteExpr::from_expr(*expr)),
                dest_type,
            },
            Expr::TryCast {
                span,
                expr,
                dest_type,
            } => RemoteExpr::TryCast {
                span,
                expr: Box::new(RemoteExpr::from_expr(*expr)),
                dest_type,
            },
            Expr::FunctionCall {
                span,
                id,
                function: _,
                generics,
                args,
            } => RemoteExpr::FunctionCall {
                span,
                id,
                generics,
                args: args.into_iter().map(RemoteExpr::from_expr).collect(),
            },
        }
    }

    pub fn into_expr(self, fn_registry: &FunctionRegistry) -> Option<Expr> {
        Some(match self {
            RemoteExpr::Constant { span, scalar } => Expr::Constant { span, scalar },
            RemoteExpr::ColumnRef { span, id } => Expr::ColumnRef { span, id },
            RemoteExpr::Cast {
                span,
                expr,
                dest_type,
            } => Expr::Cast {
                span,
                expr: Box::new(expr.into_expr(fn_registry)?),
                dest_type,
            },
            RemoteExpr::TryCast {
                span,
                expr,
                dest_type,
            } => Expr::TryCast {
                span,
                expr: Box::new(expr.into_expr(fn_registry)?),
                dest_type,
            },
            RemoteExpr::FunctionCall {
                span,
                id,
                generics,
                args,
            } => {
                let function = fn_registry.get(&id)?;
                Expr::FunctionCall {
                    span,
                    id,
                    function,
                    generics,
                    args: args
                        .into_iter()
                        .map(|arg| arg.into_expr(fn_registry))
                        .collect::<Option<_>>()?,
                }
            }
        })
    }
}
