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

use std::collections::HashSet;
use std::fmt;

use common_datavalues::prelude::*;
use common_datavalues::type_coercion::numerical_coercion;
use common_exception::ErrorCode;
use common_exception::Result;
use ordered_float::OrderedFloat;

use crate::scalars::cast_column_field;
use crate::scalars::Function;
use crate::scalars::FunctionContext;
use crate::scalars::FunctionDescription;
use crate::scalars::FunctionFeatures;

#[derive(Clone)]
pub struct InFunction<const NEGATED: bool> {
    is_null: bool,
}

impl<const NEGATED: bool> InFunction<NEGATED> {
    pub fn try_create(_display_name: &str, args: &[&DataTypeImpl]) -> Result<Box<dyn Function>> {
        for dt in args {
            let type_id = remove_nullable(dt).data_type_id();
            if type_id.is_interval() || type_id.is_array() || type_id.is_struct() {
                return Err(ErrorCode::UnexpectedError(format!(
                    "{} type is not supported for IN now",
                    type_id
                )));
            }
        }

        let is_null = args[0].data_type_id() == TypeID::Null;
        Ok(Box::new(InFunction::<NEGATED> { is_null }))
    }

    pub fn desc() -> FunctionDescription {
        FunctionDescription::creator(Box::new(Self::try_create)).features(
            FunctionFeatures::default()
                .disable_passthrough_null()
                .variadic_arguments(2, usize::MAX),
        )
    }
}

macro_rules! scalar_contains {
    ($T: ident, $INPUT_COL: expr, $ROWS: expr, $COLUMNS: expr, $CAST_TYPE: ident, $FUNC_CTX: expr) => {{
        let mut builder: ColumnBuilder<bool> = ColumnBuilder::with_capacity($ROWS);
        let mut vals_set = HashSet::with_capacity($ROWS - 1);
        for col in &$COLUMNS[1..] {
            let col = cast_column_field(col, col.data_type(), &$CAST_TYPE, &$FUNC_CTX)?;
            let col_viewer = $T::try_create_viewer(&col)?;
            if col_viewer.valid_at(0) {
                let val = col_viewer.value_at(0).to_owned_scalar();
                vals_set.insert(val);
            }
        }
        let input_viewer = $T::try_create_viewer(&$INPUT_COL)?;
        for (row, val) in input_viewer.iter().enumerate() {
            let contains = vals_set.contains(&val.to_owned());
            let valid = input_viewer.valid_at(row);
            builder.append(valid && ((contains && !NEGATED) || (!contains && NEGATED)));
        }
        return Ok(builder.build($ROWS));
    }};
}

macro_rules! float_contains {
    ($T: ident, $INPUT_COL: expr, $ROWS: expr, $COLUMNS: expr, $CAST_TYPE: ident, $FUNC_CTX: expr) => {{
        let mut builder: ColumnBuilder<bool> = ColumnBuilder::with_capacity($ROWS);
        let mut vals_set = HashSet::with_capacity($ROWS - 1);
        for col in &$COLUMNS[1..] {
            let col = cast_column_field(col, col.data_type(), &$CAST_TYPE, &$FUNC_CTX)?;
            let col_viewer = $T::try_create_viewer(&col)?;
            if col_viewer.valid_at(0) {
                let val = col_viewer.value_at(0);
                vals_set.insert(OrderedFloat::from(val));
            }
        }
        let input_viewer = $T::try_create_viewer(&$INPUT_COL)?;
        for (row, val) in input_viewer.iter().enumerate() {
            let contains = vals_set.contains(&OrderedFloat::from(val));
            let valid = input_viewer.valid_at(row);
            builder.append(valid && ((contains && !NEGATED) || (!contains && NEGATED)));
        }
        return Ok(builder.build($ROWS));
    }};
}

impl<const NEGATED: bool> Function for InFunction<NEGATED> {
    fn name(&self) -> &str {
        "InFunction"
    }

    fn return_type(&self) -> DataTypeImpl {
        if self.is_null {
            return NullType::new_impl();
        }
        BooleanType::new_impl()
    }

    fn eval(
        &self,
        func_ctx: FunctionContext,
        columns: &ColumnsWithField,
        input_rows: usize,
    ) -> Result<ColumnRef> {
        if self.is_null {
            let col = NullType::new_impl().create_constant_column(&DataValue::Null, input_rows)?;
            return Ok(col);
        }

        let null_flag = columns[1..]
            .iter()
            .any(|column| column.field().data_type().is_null());

        let mut least_super_dt = columns[0].field().data_type().clone();
        let mut nonull_least_super_dt = remove_nullable(&least_super_dt);

        // avoid precision loss
        if nonull_least_super_dt.data_type_id().is_numeric() {
            for column in columns[1..].iter() {
                if column.data_type().data_type_id().is_numeric() {
                    nonull_least_super_dt =
                        numerical_coercion(&nonull_least_super_dt, column.data_type(), false)
                            .unwrap();
                }
            }
        }

        if null_flag || least_super_dt.is_null() {
            least_super_dt = wrap_nullable(&nonull_least_super_dt);
        } else {
            least_super_dt = nonull_least_super_dt;
        }

        let least_super_type_id = remove_nullable(&least_super_dt).data_type_id();
        let input_col = cast_column_field(
            &columns[0],
            columns[0].data_type(),
            &least_super_dt,
            &func_ctx,
        )?;

        match least_super_type_id {
            TypeID::Boolean => {
                scalar_contains!(
                    bool,
                    input_col,
                    input_rows,
                    columns,
                    least_super_dt,
                    func_ctx
                )
            }
            TypeID::UInt8 => {
                scalar_contains!(u8, input_col, input_rows, columns, least_super_dt, func_ctx)
            }
            TypeID::UInt16 => {
                scalar_contains!(
                    u16,
                    input_col,
                    input_rows,
                    columns,
                    least_super_dt,
                    func_ctx
                )
            }
            TypeID::UInt32 => {
                scalar_contains!(
                    u32,
                    input_col,
                    input_rows,
                    columns,
                    least_super_dt,
                    func_ctx
                )
            }
            TypeID::UInt64 => {
                scalar_contains!(
                    u64,
                    input_col,
                    input_rows,
                    columns,
                    least_super_dt,
                    func_ctx
                )
            }
            TypeID::Int8 => {
                scalar_contains!(i8, input_col, input_rows, columns, least_super_dt, func_ctx)
            }
            TypeID::Int16 => {
                scalar_contains!(
                    i16,
                    input_col,
                    input_rows,
                    columns,
                    least_super_dt,
                    func_ctx
                )
            }
            TypeID::Int32 => {
                scalar_contains!(
                    i32,
                    input_col,
                    input_rows,
                    columns,
                    least_super_dt,
                    func_ctx
                )
            }
            TypeID::Int64 => {
                scalar_contains!(
                    i64,
                    input_col,
                    input_rows,
                    columns,
                    least_super_dt,
                    func_ctx
                )
            }
            TypeID::String => {
                scalar_contains!(
                    Vu8,
                    input_col,
                    input_rows,
                    columns,
                    least_super_dt,
                    func_ctx
                )
            }
            TypeID::Float32 => {
                float_contains!(
                    f32,
                    input_col,
                    input_rows,
                    columns,
                    least_super_dt,
                    func_ctx
                )
            }
            TypeID::Float64 => {
                float_contains!(
                    f64,
                    input_col,
                    input_rows,
                    columns,
                    least_super_dt,
                    func_ctx
                )
            }
            TypeID::Date => {
                scalar_contains!(
                    i32,
                    input_col,
                    input_rows,
                    columns,
                    least_super_dt,
                    func_ctx
                )
            }
            TypeID::Timestamp => {
                scalar_contains!(
                    i64,
                    input_col,
                    input_rows,
                    columns,
                    least_super_dt,
                    func_ctx
                )
            }
            _ => Result::Err(ErrorCode::BadDataValueType(format!(
                "{} type is not supported for IN now",
                least_super_type_id
            ))),
        }
    }
}

impl<const NEGATED: bool> fmt::Display for InFunction<NEGATED> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if NEGATED {
            write!(f, "NOT IN")
        } else {
            write!(f, "IN")
        }
    }
}
