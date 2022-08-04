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

use std::marker::PhantomData;

use bumpalo::Bump;
use common_datablocks::HashMethod;
use common_datablocks::HashMethodFixedKeys;
use common_datablocks::HashMethodKeysU128;
use common_datablocks::HashMethodKeysU256;
use common_datablocks::HashMethodKeysU512;
use common_datablocks::HashMethodSerializer;
use common_datavalues::prelude::*;
use common_exception::Result;
use common_hashtable::HashMapKind;
use primitive_types::U256;
use primitive_types::U512;

use super::aggregator_keys_builder::LargeFixedKeysColumnBuilder;
use super::aggregator_keys_iter::LargeFixedKeysColumnIter;
use crate::pipelines::processors::transforms::group_by::aggregator_groups_builder::FixedKeysGroupColumnsBuilder;
use crate::pipelines::processors::transforms::group_by::aggregator_groups_builder::GroupColumnsBuilder;
use crate::pipelines::processors::transforms::group_by::aggregator_groups_builder::SerializedKeysGroupColumnsBuilder;
use crate::pipelines::processors::transforms::group_by::aggregator_keys_builder::FixedKeysColumnBuilder;
use crate::pipelines::processors::transforms::group_by::aggregator_keys_builder::KeysColumnBuilder;
use crate::pipelines::processors::transforms::group_by::aggregator_keys_builder::SerializedKeysColumnBuilder;
use crate::pipelines::processors::transforms::group_by::aggregator_keys_iter::FixedKeysColumnIter;
use crate::pipelines::processors::transforms::group_by::aggregator_keys_iter::KeysColumnIter;
use crate::pipelines::processors::transforms::group_by::aggregator_keys_iter::SerializedKeysColumnIter;
use crate::pipelines::processors::transforms::group_by::aggregator_state::LongerFixedKeysAggregatorState;
use crate::pipelines::processors::transforms::group_by::aggregator_state::SerializedKeysAggregatorState;
use crate::pipelines::processors::transforms::group_by::aggregator_state::ShortFixedKeysAggregatorState;
use crate::pipelines::processors::transforms::group_by::AggregatorState;
use crate::pipelines::processors::AggregatorParams;

// Provide functions for all HashMethod to help implement polymorphic group by key
//
// When we want to add new HashMethod, we need to add the following components
//     - HashMethod, more information in [HashMethod] trait
//     - AggregatorState, more information in [AggregatorState] trait
//     - KeysColumnBuilder, more information in [KeysColumnBuilder] trait
//     - PolymorphicKeysHelper, more information in following comments
//
// For example:
//
// use bumpalo::Bump;
// use databend_query::common::HashTable;
// use common_datablocks::HashMethodSerializer;
// use common_datavalues::prelude::*;
// use databend_query::pipelines::processors::transforms::group_by::PolymorphicKeysHelper;
// use databend_query::pipelines::processors::transforms::group_by::aggregator_state::SerializedKeysAggregatorState;
// use databend_query::pipelines::processors::transforms::group_by::aggregator_keys_builder::SerializedKeysColumnBuilder;
//
// impl PolymorphicKeysHelper<HashMethodSerializer> for HashMethodSerializer {
//     type State = SerializedKeysAggregatorState;
//     fn aggregate_state(&self) -> Self::State {
//         SerializedKeysAggregatorState {
//             keys_area: Bump::new(),
//             state_area: Bump::new(),
//             data_state_map: HashTable::create(),
//         }
//     }
//
//     type ColumnBuilder = SerializedKeysColumnBuilder;
//     fn state_array_builder(&self, capacity: usize) -> Self::ColumnBuilder {
//         SerializedKeysColumnBuilder {
//             inner_builder: MutableStringColumn::with_capacity(capacity),
//         }
//     }
// }
//
pub trait PolymorphicKeysHelper<Method: HashMethod> {
    type State: AggregatorState<Method>;
    fn aggregate_state(&self) -> Self::State;

    type ColumnBuilder: KeysColumnBuilder<<Self::State as AggregatorState<Method>>::Key>;
    fn keys_column_builder(&self, capacity: usize) -> Self::ColumnBuilder;

    type KeysColumnIter: KeysColumnIter<<Self::State as AggregatorState<Method>>::Key>;
    fn keys_iter_from_column(&self, column: &ColumnRef) -> Result<Self::KeysColumnIter>;

    type GroupColumnsBuilder: GroupColumnsBuilder<<Self::State as AggregatorState<Method>>::Key>;
    fn group_columns_builder(
        &self,
        capacity: usize,
        params: &AggregatorParams,
    ) -> Self::GroupColumnsBuilder;
}

impl PolymorphicKeysHelper<HashMethodFixedKeys<u8>> for HashMethodFixedKeys<u8> {
    type State = ShortFixedKeysAggregatorState<u8>;
    fn aggregate_state(&self) -> Self::State {
        Self::State::create((u8::MAX as usize) + 1)
    }
    type ColumnBuilder = FixedKeysColumnBuilder<u8>;
    fn keys_column_builder(&self, capacity: usize) -> Self::ColumnBuilder {
        FixedKeysColumnBuilder::<u8> {
            inner_builder: MutablePrimitiveColumn::<u8>::with_capacity(capacity),
        }
    }
    type KeysColumnIter = FixedKeysColumnIter<u8>;
    fn keys_iter_from_column(&self, column: &ColumnRef) -> Result<Self::KeysColumnIter> {
        FixedKeysColumnIter::create(Series::check_get::<PrimitiveColumn<u8>>(column)?)
    }
    type GroupColumnsBuilder = FixedKeysGroupColumnsBuilder<u8>;
    fn group_columns_builder(
        &self,
        capacity: usize,
        params: &AggregatorParams,
    ) -> Self::GroupColumnsBuilder {
        FixedKeysGroupColumnsBuilder::<u8>::create(capacity, params)
    }
}

impl PolymorphicKeysHelper<HashMethodFixedKeys<u16>> for HashMethodFixedKeys<u16> {
    type State = ShortFixedKeysAggregatorState<u16>;
    fn aggregate_state(&self) -> Self::State {
        Self::State::create((u16::MAX as usize) + 1)
    }
    type ColumnBuilder = FixedKeysColumnBuilder<u16>;
    fn keys_column_builder(&self, capacity: usize) -> Self::ColumnBuilder {
        FixedKeysColumnBuilder::<u16> {
            inner_builder: MutablePrimitiveColumn::<u16>::with_capacity(capacity),
        }
    }
    type KeysColumnIter = FixedKeysColumnIter<u16>;
    fn keys_iter_from_column(&self, column: &ColumnRef) -> Result<Self::KeysColumnIter> {
        FixedKeysColumnIter::create(Series::check_get::<PrimitiveColumn<u16>>(column)?)
    }
    type GroupColumnsBuilder = FixedKeysGroupColumnsBuilder<u16>;
    fn group_columns_builder(
        &self,
        capacity: usize,
        params: &AggregatorParams,
    ) -> Self::GroupColumnsBuilder {
        FixedKeysGroupColumnsBuilder::<u16>::create(capacity, params)
    }
}

impl PolymorphicKeysHelper<HashMethodFixedKeys<u32>> for HashMethodFixedKeys<u32> {
    type State = LongerFixedKeysAggregatorState<u32>;
    fn aggregate_state(&self) -> Self::State {
        Self::State::default()
    }
    type ColumnBuilder = FixedKeysColumnBuilder<u32>;
    fn keys_column_builder(&self, capacity: usize) -> Self::ColumnBuilder {
        FixedKeysColumnBuilder::<u32> {
            inner_builder: MutablePrimitiveColumn::<u32>::with_capacity(capacity),
        }
    }
    type KeysColumnIter = FixedKeysColumnIter<u32>;
    fn keys_iter_from_column(&self, column: &ColumnRef) -> Result<Self::KeysColumnIter> {
        FixedKeysColumnIter::create(Series::check_get::<PrimitiveColumn<u32>>(column)?)
    }
    type GroupColumnsBuilder = FixedKeysGroupColumnsBuilder<u32>;
    fn group_columns_builder(
        &self,
        capacity: usize,
        params: &AggregatorParams,
    ) -> Self::GroupColumnsBuilder {
        FixedKeysGroupColumnsBuilder::<u32>::create(capacity, params)
    }
}

impl PolymorphicKeysHelper<HashMethodFixedKeys<u64>> for HashMethodFixedKeys<u64> {
    type State = LongerFixedKeysAggregatorState<u64>;
    fn aggregate_state(&self) -> Self::State {
        Self::State::default()
    }
    type ColumnBuilder = FixedKeysColumnBuilder<u64>;
    fn keys_column_builder(&self, capacity: usize) -> Self::ColumnBuilder {
        FixedKeysColumnBuilder::<u64> {
            inner_builder: MutablePrimitiveColumn::<u64>::with_capacity(capacity),
        }
    }
    type KeysColumnIter = FixedKeysColumnIter<u64>;
    fn keys_iter_from_column(&self, column: &ColumnRef) -> Result<Self::KeysColumnIter> {
        FixedKeysColumnIter::create(Series::check_get::<PrimitiveColumn<u64>>(column)?)
    }
    type GroupColumnsBuilder = FixedKeysGroupColumnsBuilder<u64>;
    fn group_columns_builder(
        &self,
        capacity: usize,
        params: &AggregatorParams,
    ) -> Self::GroupColumnsBuilder {
        FixedKeysGroupColumnsBuilder::<u64>::create(capacity, params)
    }
}

impl PolymorphicKeysHelper<HashMethodKeysU128> for HashMethodKeysU128 {
    type State = LongerFixedKeysAggregatorState<u128>;
    fn aggregate_state(&self) -> Self::State {
        Self::State::default()
    }

    type ColumnBuilder = LargeFixedKeysColumnBuilder<u128>;
    fn keys_column_builder(&self, capacity: usize) -> Self::ColumnBuilder {
        LargeFixedKeysColumnBuilder {
            inner_builder: MutableStringColumn::with_capacity(capacity * 16),
            _t: PhantomData,
        }
    }

    type KeysColumnIter = LargeFixedKeysColumnIter<u128>;
    fn keys_iter_from_column(&self, column: &ColumnRef) -> Result<Self::KeysColumnIter> {
        LargeFixedKeysColumnIter::create(Series::check_get::<StringColumn>(column)?)
    }

    type GroupColumnsBuilder = FixedKeysGroupColumnsBuilder<u128>;
    fn group_columns_builder(
        &self,
        capacity: usize,
        params: &AggregatorParams,
    ) -> Self::GroupColumnsBuilder {
        FixedKeysGroupColumnsBuilder::create(capacity, params)
    }
}

impl PolymorphicKeysHelper<HashMethodKeysU256> for HashMethodKeysU256 {
    type State = LongerFixedKeysAggregatorState<U256>;
    fn aggregate_state(&self) -> Self::State {
        Self::State::default()
    }

    type ColumnBuilder = LargeFixedKeysColumnBuilder<U256>;
    fn keys_column_builder(&self, capacity: usize) -> Self::ColumnBuilder {
        LargeFixedKeysColumnBuilder {
            inner_builder: MutableStringColumn::with_capacity(capacity * 32),
            _t: PhantomData,
        }
    }

    type KeysColumnIter = LargeFixedKeysColumnIter<U256>;
    fn keys_iter_from_column(&self, column: &ColumnRef) -> Result<Self::KeysColumnIter> {
        LargeFixedKeysColumnIter::create(Series::check_get::<StringColumn>(column)?)
    }

    type GroupColumnsBuilder = FixedKeysGroupColumnsBuilder<U256>;
    fn group_columns_builder(
        &self,
        capacity: usize,
        params: &AggregatorParams,
    ) -> Self::GroupColumnsBuilder {
        FixedKeysGroupColumnsBuilder::create(capacity, params)
    }
}

impl PolymorphicKeysHelper<HashMethodKeysU512> for HashMethodKeysU512 {
    type State = LongerFixedKeysAggregatorState<U512>;
    fn aggregate_state(&self) -> Self::State {
        Self::State::default()
    }

    type ColumnBuilder = LargeFixedKeysColumnBuilder<U512>;
    fn keys_column_builder(&self, capacity: usize) -> Self::ColumnBuilder {
        LargeFixedKeysColumnBuilder {
            inner_builder: MutableStringColumn::with_capacity(capacity * 64),
            _t: PhantomData,
        }
    }

    type KeysColumnIter = LargeFixedKeysColumnIter<U512>;
    fn keys_iter_from_column(&self, column: &ColumnRef) -> Result<Self::KeysColumnIter> {
        LargeFixedKeysColumnIter::create(Series::check_get::<StringColumn>(column)?)
    }

    type GroupColumnsBuilder = FixedKeysGroupColumnsBuilder<U512>;
    fn group_columns_builder(
        &self,
        capacity: usize,
        params: &AggregatorParams,
    ) -> Self::GroupColumnsBuilder {
        FixedKeysGroupColumnsBuilder::create(capacity, params)
    }
}

impl PolymorphicKeysHelper<HashMethodSerializer> for HashMethodSerializer {
    type State = SerializedKeysAggregatorState;
    fn aggregate_state(&self) -> Self::State {
        SerializedKeysAggregatorState {
            keys_area: Bump::new(),
            state_area: Bump::new(),
            data_state_map: HashMapKind::create_hash_table(),
            two_level_flag: false,
        }
    }

    type ColumnBuilder = SerializedKeysColumnBuilder;
    fn keys_column_builder(&self, capacity: usize) -> Self::ColumnBuilder {
        SerializedKeysColumnBuilder {
            inner_builder: MutableStringColumn::with_capacity(capacity),
        }
    }

    type KeysColumnIter = SerializedKeysColumnIter;
    fn keys_iter_from_column(&self, column: &ColumnRef) -> Result<Self::KeysColumnIter> {
        SerializedKeysColumnIter::create(Series::check_get::<StringColumn>(column)?)
    }

    type GroupColumnsBuilder = SerializedKeysGroupColumnsBuilder;
    fn group_columns_builder(
        &self,
        capacity: usize,
        params: &AggregatorParams,
    ) -> Self::GroupColumnsBuilder {
        SerializedKeysGroupColumnsBuilder::create(capacity, params)
    }
}
