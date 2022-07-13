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

use common_arrow::arrow::datatypes::DataType as ArrowType;
use common_exception::ErrorCode;
use common_exception::Result;

use super::data_type::DataType;
use super::data_type::DataTypeImpl;
use super::type_id::TypeID;
use crate::prelude::*;
use crate::serializations::StructSerializer;
use crate::serializations::TypeSerializerImpl;

#[derive(Default, Clone, serde::Deserialize, serde::Serialize)]
pub struct StructType {
    names: Vec<String>,
    types: Vec<DataTypeImpl>,
}

impl StructType {
    pub fn new_impl(names: Vec<String>, types: Vec<DataTypeImpl>) -> DataTypeImpl {
        DataTypeImpl::Struct(Self::create(names, types))
    }

    pub fn create(names: Vec<String>, types: Vec<DataTypeImpl>) -> Self {
        debug_assert!(names.len() == types.len());
        StructType { names, types }
    }

    pub fn names(&self) -> &Vec<String> {
        &self.names
    }

    pub fn types(&self) -> &Vec<DataTypeImpl> {
        &self.types
    }
}

impl DataType for StructType {
    fn data_type_id(&self) -> TypeID {
        TypeID::Struct
    }

    #[inline]
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn name(&self) -> String {
        let mut type_name = String::new();
        type_name.push_str("Struct(");
        let mut first = true;
        for (name, ty) in self.names.iter().zip(self.types.iter()) {
            if !first {
                type_name.push_str(", ");
            }
            first = false;
            type_name.push_str(name);
            type_name.push(' ');
            type_name.push_str(&ty.name());
        }
        type_name.push(')');

        type_name
    }

    fn can_inside_nullable(&self) -> bool {
        false
    }

    fn default_value(&self) -> DataValue {
        let c: Vec<DataValue> = self.types.iter().map(|t| t.default_value()).collect();
        DataValue::Struct(c)
    }

    fn create_constant_column(&self, data: &DataValue, size: usize) -> Result<ColumnRef> {
        if let DataValue::Struct(value) = data {
            debug_assert!(value.len() == self.types.len());

            let cols = value
                .iter()
                .zip(self.types.iter())
                .map(|(v, typ)| typ.create_constant_column(v, size))
                .collect::<Result<Vec<_>>>()?;
            let struct_column = StructColumn::from_data(cols, DataTypeImpl::Struct(self.clone()));
            return Ok(Arc::new(ConstColumn::new(Arc::new(struct_column), size)));
        }

        Err(ErrorCode::BadDataValueType(format!(
            "Unexpected type:{:?} to generate list column",
            data.value_type()
        )))
    }

    fn arrow_type(&self) -> ArrowType {
        let fields = self
            .names
            .iter()
            .zip(self.types.iter())
            .map(|(name, type_)| type_.to_arrow_field(name))
            .collect();

        ArrowType::Struct(fields)
    }

    fn create_serializer_inner<'a>(&self, col: &'a ColumnRef) -> Result<TypeSerializerImpl<'a>> {
        let column: &StructColumn = Series::check_get(col)?;
        let cols = column.values();
        let mut inners = vec![];
        for (t, c) in self.types.iter().zip(cols) {
            inners.push(t.create_serializer(c)?)
        }
        Ok(StructSerializer {
            names: self.names.clone(),
            inners,
            column: col,
        }
        .into())
    }

    fn create_deserializer(&self, capacity: usize) -> TypeDeserializerImpl {
        let inners_mutable = self
            .types
            .iter()
            .map(|v| v.create_mutable(capacity))
            .collect();

        let inners_desers = self
            .types
            .iter()
            .map(|v| v.create_deserializer(capacity))
            .collect();

        StructDeserializer {
            builder: MutableStructColumn::from_data(self.clone().into(), inners_mutable),
            inners: inners_desers,
        }
        .into()
    }

    fn create_mutable(&self, capacity: usize) -> Box<dyn MutableColumn> {
        let inners = self
            .types
            .iter()
            .map(|v| v.create_mutable(capacity))
            .collect();
        Box::new(MutableStructColumn::from_data(self.clone().into(), inners))
    }

    fn create_column(&self, datas: &[DataValue]) -> common_exception::Result<ColumnRef> {
        let mut values = Vec::with_capacity(self.types.len());
        for _ in 0..self.types.len() {
            values.push(Vec::<DataValue>::with_capacity(datas.len()));
        }

        for data in datas.iter() {
            if let DataValue::Struct(value) = data {
                debug_assert!(value.len() == self.types.len());
                for (i, v) in value.iter().enumerate() {
                    values[i].push(v.clone());
                }
            } else {
                return Result::Err(ErrorCode::BadDataValueType(format!(
                    "Unexpected type:{:?}, expect to be struct",
                    data.value_type()
                )));
            }
        }

        let mut columns = Vec::with_capacity(self.types.len());
        for (idx, value) in values.iter().enumerate() {
            columns.push(self.types[idx].create_column(value)?);
        }

        Ok(StructColumn::from_data(columns, DataTypeImpl::Struct(self.clone())).arc())
    }
}

impl std::fmt::Debug for StructType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}
