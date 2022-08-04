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

use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;

use common_datablocks::DataBlock;
use common_datavalues::prelude::*;
use common_exception::ErrorCode;
use common_exception::Result;
use common_functions::scalars::Function;
use common_functions::scalars::FunctionContext;
use common_functions::scalars::FunctionFactory;

use crate::api::rpc::flight_scatter::FlightScatter;
use crate::evaluator::EvalNode;
use crate::evaluator::Evaluator;
use crate::evaluator::TypedVector;
use crate::sql::executor::ColumnID;
use crate::sql::executor::PhysicalScalar;

#[derive(Clone)]
pub struct HashFlightScatterV2 {
    func_ctx: FunctionContext,
    hash_keys: Vec<EvalNode<ColumnID>>,
    hash_functions: Vec<Box<dyn Function>>,
    scatter_size: usize,
}

impl HashFlightScatterV2 {
    pub fn try_create(
        func_ctx: FunctionContext,
        scalars: Vec<PhysicalScalar>,
        scatter_size: usize,
    ) -> Result<Self> {
        let hash_keys = scalars
            .iter()
            .map(Evaluator::eval_physical_scalar)
            .collect::<Result<Vec<_>>>()?;
        let hash_functions = scalars
            .iter()
            .map(|k| FunctionFactory::instance().get("sipHash", &[&k.data_type()]))
            .collect::<Result<Vec<_>>>()?;

        Ok(Self {
            func_ctx,
            hash_keys,
            hash_functions,
            scatter_size,
        })
    }

    pub fn combine_hash_keys(
        &self,
        hash_keys: &[TypedVector],
        num_rows: usize,
    ) -> Result<Vec<u64>> {
        if self.hash_functions.len() != hash_keys.len() {
            return Err(ErrorCode::LogicalError(
                "Hash keys and hash functions must be the same length.",
            ));
        }
        let mut hash = vec![DefaultHasher::default(); num_rows];
        for (key, func) in hash_keys.iter().zip(self.hash_functions.iter()) {
            let column = func.eval(
                self.func_ctx.clone(),
                &[ColumnWithField::new(
                    key.vector().clone(),
                    DataField::new("dummy", key.logical_type()),
                )],
                num_rows,
            )?;
            let hash_values = Self::get_hash_values(&column)?;
            for (i, value) in hash_values.iter().enumerate() {
                hash[i].write_u64(*value);
            }
        }

        Ok(hash.into_iter().map(|h| h.finish()).collect())
    }

    fn get_hash_values(column: &ColumnRef) -> Result<Vec<u64>> {
        if let Ok(column) = Series::check_get::<PrimitiveColumn<u64>>(column) {
            Ok(column.values().to_vec())
        } else if let Ok(column) = Series::check_get::<NullableColumn>(column) {
            let null_map = column.ensure_validity();
            let mut values = Self::get_hash_values(column.inner())?;
            for (i, v) in values.iter_mut().enumerate() {
                if null_map.get_bit(i) {
                    // Set hash value of NULL to 0
                    *v = 0;
                }
            }
            Ok(values)
        } else if let Ok(column) = Series::check_get::<ConstColumn>(column) {
            Self::get_hash_values(&column.convert_full_column())
        } else {
            Err(ErrorCode::LogicalError("Hash keys must be of type u64."))
        }
    }
}

impl FlightScatter for HashFlightScatterV2 {
    fn execute(
        &self,
        data_block: &DataBlock,
        _num: usize,
    ) -> common_exception::Result<Vec<DataBlock>> {
        let hash_keys = self
            .hash_keys
            .iter()
            .map(|eval| eval.eval(&self.func_ctx, data_block))
            .collect::<Result<Vec<_>>>()?;
        let hash = self.combine_hash_keys(&hash_keys, data_block.num_rows())?;
        let indices: Vec<usize> = hash
            .iter()
            .map(|c| (*c as usize) % self.scatter_size)
            .collect();
        DataBlock::scatter_block(data_block, &indices, self.scatter_size)
    }
}
