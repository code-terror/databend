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

use std::sync::Arc;

use common_arrow::arrow::bitmap::Bitmap;
use common_arrow::arrow::bitmap::MutableBitmap;
use common_arrow::arrow::compute::cast;
use common_arrow::arrow::compute::cast::CastOptions as ArrowOption;
use common_arrow::ArrayRef;
use common_datavalues::prelude::*;
use common_datavalues::with_match_primitive_type_id;
use common_exception::ErrorCode;
use common_exception::Result;
use common_io::prelude::FormatSettings;

use super::cast_from_datetimes::cast_from_date;
use super::cast_from_string::cast_from_string;
use super::cast_from_variant::cast_from_variant;
use crate::scalars::expressions::cast_from_datetimes::cast_from_timestamp;
use crate::scalars::FunctionContext;

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub struct CastOptions {
    pub exception_mode: ExceptionMode,
    pub parsing_mode: ParsingMode,
}

pub const DEFAULT_CAST_OPTIONS: CastOptions = CastOptions {
    exception_mode: ExceptionMode::Throw,
    parsing_mode: ParsingMode::Strict,
};

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum ExceptionMode {
    /// Throw exception if value cannot be parsed.
    Throw,
    /// Fill with zero or default if value cannot be parsed.
    Zero,
}

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum ParsingMode {
    Strict,
    Partial,
}

impl CastOptions {
    fn as_arrow(&self) -> ArrowOption {
        ArrowOption {
            wrapped: true,
            partial: self.parsing_mode == ParsingMode::Partial,
        }
    }
}

pub fn cast_column_field(
    column_with_field: &ColumnWithField,
    from_type: &DataTypeImpl,
    target_type: &DataTypeImpl,
    func_ctx: &FunctionContext,
) -> Result<ColumnRef> {
    cast_with_type(
        column_with_field.column(),
        from_type,
        target_type,
        &DEFAULT_CAST_OPTIONS,
        func_ctx,
    )
}

// No logical type is specified
// Use Default options
pub fn default_column_cast(column: &ColumnRef, data_type: &DataTypeImpl) -> Result<ColumnRef> {
    let func_ctx = FunctionContext::default();
    cast_with_type(
        column,
        &column.data_type(),
        data_type,
        &DEFAULT_CAST_OPTIONS,
        &func_ctx,
    )
}

pub fn cast_with_type(
    column: &ColumnRef,
    from_type: &DataTypeImpl,
    target_type: &DataTypeImpl,
    cast_options: &CastOptions,
    func_ctx: &FunctionContext,
) -> Result<ColumnRef> {
    // they are pyhsically the same type
    if &column.data_type() == target_type {
        return Ok(column.clone());
    }

    if target_type.data_type_id() == TypeID::Null {
        return Ok(Arc::new(NullColumn::new(column.len())));
    }

    if from_type.data_type_id() == TypeID::Null {
        //all is null
        if target_type.is_nullable() {
            return target_type.create_constant_column(&DataValue::Null, column.len());
        }
        return Err(ErrorCode::BadDataValueType(
            "Can't cast column from null into non-nullable type".to_string(),
        ));
    }

    if column.is_const() {
        let col: &ConstColumn = Series::check_get(column)?;
        let inner = col.inner();
        let res = cast_with_type(inner, from_type, target_type, cast_options, func_ctx)?;
        return Ok(ConstColumn::new(res, column.len()).arc());
    }

    let nonull_from_type = remove_nullable(from_type);
    let nonull_data_type = remove_nullable(target_type);

    let (result, valids) = match nonull_from_type.data_type_id() {
        TypeID::String => cast_from_string(
            column,
            &nonull_from_type,
            &nonull_data_type,
            cast_options,
            func_ctx,
        ),
        TypeID::Date => cast_from_date(
            column,
            &nonull_from_type,
            &nonull_data_type,
            cast_options,
            func_ctx,
        ),
        TypeID::Timestamp => cast_from_timestamp(
            column,
            &nonull_from_type,
            &nonull_data_type,
            cast_options,
            func_ctx,
        ),
        TypeID::Variant | TypeID::VariantArray | TypeID::VariantObject => {
            cast_from_variant(column, &nonull_data_type, func_ctx)
        }
        _ => arrow_cast_compute(
            column,
            &nonull_from_type,
            &nonull_data_type,
            cast_options,
            func_ctx,
        ),
    }?;

    // check date/timestamp bound
    if nonull_data_type.data_type_id() == TypeID::Date {
        let viewer = i32::try_create_viewer(&result)?;
        for x in viewer {
            check_date(x)?;
        }
    } else if nonull_data_type.data_type_id() == TypeID::Timestamp {
        let viewer = i64::try_create_viewer(&result)?;
        for x in viewer {
            check_timestamp(x)?;
        }
    } else if nonull_data_type.data_type_id() == TypeID::Array
        || nonull_data_type.data_type_id() == TypeID::Struct
    {
        return Err(ErrorCode::BadDataValueType(format!(
            "Cast error happens in casting from {} to {}",
            from_type.name(),
            target_type.name()
        )));
    }

    let (all_nulls, source_valids) = column.validity();
    let bitmap = combine_validities_2(source_valids.cloned(), valids);
    if target_type.is_nullable() {
        return Ok(NullableColumn::wrap_inner(result, bitmap));
    }

    if let Some(bitmap) = bitmap {
        let null_cnt = bitmap.unset_bits();
        let source_null_cnt = match (all_nulls, source_valids) {
            (true, _) => column.len(),
            (false, None) => 0,
            (false, Some(b)) => b.unset_bits(),
        };

        if cast_options.exception_mode == ExceptionMode::Throw
            && (from_type.is_nullable() && null_cnt > source_null_cnt)
            || (!from_type.is_nullable() && null_cnt > 0)
        {
            // TODO get the data to error msg
            return Err(ErrorCode::BadDataValueType(format!(
                "Cast error happens in casting from {} to {}",
                from_type.name(),
                target_type.name()
            )));
        }
    }

    Ok(result)
}

pub fn cast_to_variant(
    column: &ColumnRef,
    from_type: &DataTypeImpl,
    data_type: &DataTypeImpl,
    _func_ctx: &FunctionContext,
) -> Result<(ColumnRef, Option<Bitmap>)> {
    let column = Series::remove_nullable(column);
    let size = column.len();

    if data_type.data_type_id() == TypeID::VariantArray {
        return Err(ErrorCode::BadDataValueType(format!(
            "Expression type does not match column data type, expecting ARRAY but got {}",
            from_type.data_type_id()
        )));
    } else if data_type.data_type_id() == TypeID::VariantObject {
        return Err(ErrorCode::BadDataValueType(format!(
            "Expression type does not match column data type, expecting OBJECT but got {}",
            from_type.data_type_id()
        )));
    }
    let mut builder = ColumnBuilder::<VariantValue>::with_capacity(size);
    if from_type.data_type_id().is_numeric() || from_type.data_type_id() == TypeID::Boolean {
        let serializer = from_type.create_serializer(&column)?;
        let format = FormatSettings::default();
        match serializer.serialize_json_object(None, &format) {
            Ok(values) => {
                for v in values {
                    builder.append(&VariantValue::from(v));
                }
            }
            Err(e) => return Err(e),
        }
        return Ok((builder.build(size), None));
    }
    // other data types can't automatically casted to variant
    Err(ErrorCode::BadDataValueType(format!(
        "Expression type does not match column data type, expecting VARIANT but got {}",
        from_type.data_type_id()
    )))
}

pub fn cast_to_timestamp(
    column: &ColumnRef,
    from_type: &DataTypeImpl,
) -> Result<(ColumnRef, Option<Bitmap>)> {
    let column = Series::remove_nullable(column);
    let size = column.len();
    let mut builder = ColumnBuilder::<i64>::with_capacity(size);

    with_match_primitive_type_id!(from_type.data_type_id(), |$T| {
        let col: &PrimitiveColumn<$T> = Series::check_get(&column)?;
        for v in col.iter() {
            // The value is treated as a number of seconds, milliseconds or microseconds
            // depending on the size of the numbers.
            // If the value is less than 31536000000, it is treated as a number of seconds,
            // If the value is greater than or equal to 31536000000 and less than 31536000000000,
            // it is treated as milliseconds.
            // If the value is greater than or equal to 31536000000000, it is treated as microseconds.
            let val = *v as i64;
            if val < 31536000000 {
                builder.append(val * 1000000);
            } else if val < 31536000000000 {
                builder.append(val * 1000);
            } else {
                builder.append(val);
            }
        }
    }, {
        if from_type.data_type_id() == TypeID::Boolean {
            let col: &BooleanColumn = Series::check_get(&column)?;
            for v in col.iter() {
                if v {
                    builder.append(1i64);
                } else {
                    builder.append(0i64);
                }
            }
        } else {
            return Err(ErrorCode::BadDataValueType(format!(
                "Cast error happens in casting from {} to Timestamp.",
                from_type.data_type_id()
            )));
        }
    });
    return Ok((builder.build(size), None));
}

// cast using arrow's cast compute
pub fn arrow_cast_compute(
    column: &ColumnRef,
    from_type: &DataTypeImpl,
    data_type: &DataTypeImpl,
    cast_options: &CastOptions,
    func_ctx: &FunctionContext,
) -> Result<(ColumnRef, Option<Bitmap>)> {
    if data_type.data_type_id().is_variant() {
        return cast_to_variant(column, from_type, data_type, func_ctx);
    } else if data_type.data_type_id() == TypeID::Timestamp {
        return cast_to_timestamp(column, from_type);
    }

    let arrow_array = column.as_arrow_array();
    let arrow_options = cast_options.as_arrow();
    let result: ArrayRef =
        cast::cast(arrow_array.as_ref(), &data_type.arrow_type(), arrow_options)?;
    let bitmap = result.validity().cloned();
    Ok((result.into_column(), bitmap))
}

pub fn new_mutable_bitmap(size: usize, valid: bool) -> MutableBitmap {
    let mut bitmap = MutableBitmap::with_capacity(size);
    bitmap.extend_constant(size, valid);

    bitmap
}
