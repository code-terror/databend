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

use std::alloc::Layout;
use std::intrinsics::likely;

use bumpalo::Bump;
use common_datablocks::HashMethod;
use common_datablocks::HashMethodFixedKeys;
use common_datablocks::HashMethodSerializer;
use common_datavalues::prelude::*;
use common_functions::aggregates::StateAddr;
use common_hashtable::HashMapIteratorKind;
use common_hashtable::HashMapKind;
use common_hashtable::HashTableEntity;
use common_hashtable::HashTableKeyable;
use common_hashtable::KeyValueEntity;

use crate::pipelines::processors::transforms::group_by::aggregator_state_entity::ShortFixedKeyable;
use crate::pipelines::processors::transforms::group_by::aggregator_state_entity::ShortFixedKeysStateEntity;
use crate::pipelines::processors::transforms::group_by::aggregator_state_entity::StateEntity;
use crate::pipelines::processors::transforms::group_by::aggregator_state_iterator::ShortFixedKeysStateIterator;
use crate::pipelines::processors::transforms::group_by::keys_ref::KeysRef;
use crate::pipelines::processors::transforms::group_by::AggregatorParams;
use crate::pipelines::processors::AggregatorParams as NewAggregatorParams;

/// Aggregate state of the SELECT query, destroy when group by is completed.
///
/// It helps manage the following states:
///     - Aggregate data(HashMap or MergeSort set in future)
///     - Aggregate function state data memory pool
///     - Group by key data memory pool (if necessary)
#[allow(clippy::len_without_is_empty)]
pub trait AggregatorState<Method: HashMethod>: Sync + Send {
    type Key;
    type Entity: StateEntity<Self::Key>;
    type Iterator: Iterator<Item = *mut Self::Entity>;

    fn len(&self) -> usize;

    fn iter(&self) -> Self::Iterator;

    fn alloc_place(&self, layout: Layout) -> StateAddr;

    fn alloc_layout(&self, params: &AggregatorParams) -> Option<StateAddr> {
        params.layout?;
        let place: StateAddr = self.alloc_place(params.layout.unwrap());

        for idx in 0..params.offsets_aggregate_states.len() {
            let aggr_state = params.offsets_aggregate_states[idx];
            let aggr_state_place = place.next(aggr_state);
            params.aggregate_functions[idx].init_state(aggr_state_place);
        }
        Some(place)
    }

    fn alloc_layout2(&self, params: &NewAggregatorParams) -> Option<StateAddr> {
        params.layout?;
        let place: StateAddr = self.alloc_place(params.layout.unwrap());

        for idx in 0..params.offsets_aggregate_states.len() {
            let aggr_state = params.offsets_aggregate_states[idx];
            let aggr_state_place = place.next(aggr_state);
            params.aggregate_functions[idx].init_state(aggr_state_place);
        }
        Some(place)
    }

    fn entity(&mut self, key: Method::HashKeyRef<'_>, inserted: &mut bool) -> *mut Self::Entity;

    fn entity_by_key(&mut self, key: &Self::Key, inserted: &mut bool) -> *mut Self::Entity;

    fn is_two_level(&self) -> bool {
        false
    }

    fn convert_to_two_level(&mut self) {}
}

/// The fixed length array is used as the data structure to locate the key by subscript
pub struct ShortFixedKeysAggregatorState<T: ShortFixedKeyable> {
    area: Bump,
    size: usize,
    max_size: usize,
    data: *mut ShortFixedKeysStateEntity<T>,
    two_level_flag: bool,
}

// TODO:(Winter) Hack:
// The *mut ShortFixedKeysStateEntity needs to be used externally, but we can ensure that *mut
// ShortFixedKeysStateEntity will not be used multiple async, so ShortFixedKeysAggregatorState is Send
unsafe impl<T: ShortFixedKeyable + Send> Send for ShortFixedKeysAggregatorState<T> {}

// TODO:(Winter) Hack:
// The *mut ShortFixedKeysStateEntity needs to be used externally, but we can ensure that &*mut
// ShortFixedKeysStateEntity will not be used multiple async, so ShortFixedKeysAggregatorState is Sync
unsafe impl<T: ShortFixedKeyable + Sync> Sync for ShortFixedKeysAggregatorState<T> {}

impl<T: ShortFixedKeyable> ShortFixedKeysAggregatorState<T> {
    pub fn create(max_size: usize) -> Self {
        unsafe {
            let size = max_size * std::mem::size_of::<ShortFixedKeysStateEntity<T>>();
            let entity_align = std::mem::align_of::<ShortFixedKeysStateEntity<T>>();
            let entity_layout = Layout::from_size_align_unchecked(size, entity_align);

            let raw_ptr = std::alloc::alloc_zeroed(entity_layout);

            ShortFixedKeysAggregatorState::<T> {
                area: Default::default(),
                data: raw_ptr as *mut ShortFixedKeysStateEntity<T>,
                size: 0,
                max_size,
                two_level_flag: false,
            }
        }
    }
}

impl<T: ShortFixedKeyable> Drop for ShortFixedKeysAggregatorState<T> {
    fn drop(&mut self) {
        unsafe {
            let size = self.max_size * std::mem::size_of::<ShortFixedKeysStateEntity<T>>();
            let entity_align = std::mem::align_of::<ShortFixedKeysStateEntity<T>>();
            let layout = Layout::from_size_align_unchecked(size, entity_align);
            std::alloc::dealloc(self.data as *mut u8, layout);
        }
    }
}

impl<T> AggregatorState<HashMethodFixedKeys<T>> for ShortFixedKeysAggregatorState<T>
where
    T: PrimitiveType + ShortFixedKeyable,
    for<'a> HashMethodFixedKeys<T>: HashMethod<HashKey = T, HashKeyRef<'a> = T>,
    for<'a> <HashMethodFixedKeys<T> as HashMethod>::HashKey: HashTableKeyable,
{
    type Key = T;
    type Entity = ShortFixedKeysStateEntity<T>;
    type Iterator = ShortFixedKeysStateIterator<T>;

    #[inline(always)]
    fn len(&self) -> usize {
        self.size
    }

    #[inline(always)]
    fn iter(&self) -> Self::Iterator {
        Self::Iterator::create(self.data, self.max_size as isize)
    }

    #[inline(always)]
    fn alloc_place(&self, layout: Layout) -> StateAddr {
        self.area.alloc_layout(layout).into()
    }

    #[inline(always)]
    fn entity(&mut self, key: T, inserted: &mut bool) -> *mut Self::Entity {
        unsafe {
            let index = key.lookup();
            let value = self.data.offset(index);

            if likely((*value).fill) {
                *inserted = false;
                return value;
            }

            *inserted = true;
            self.size += 1;
            (*value).key = key;
            (*value).fill = true;
            value
        }
    }

    #[inline(always)]
    fn entity_by_key(&mut self, key: &Self::Key, inserted: &mut bool) -> *mut Self::Entity {
        self.entity(*key, inserted)
    }

    #[inline(always)]
    fn is_two_level(&self) -> bool {
        self.two_level_flag
    }

    #[inline(always)]
    fn convert_to_two_level(&mut self) {
        self.two_level_flag = true;
    }
}

pub struct LongerFixedKeysAggregatorState<T: HashTableKeyable> {
    pub area: Bump,
    pub data: HashMapKind<T, usize>,
    pub two_level_flag: bool,
}

impl<T: HashTableKeyable> Default for LongerFixedKeysAggregatorState<T> {
    fn default() -> Self {
        Self {
            area: Bump::new(),
            data: HashMapKind::create_hash_table(),
            two_level_flag: false,
        }
    }
}

// TODO:(Winter) Hack:
// The *mut KeyValueEntity needs to be used externally, but we can ensure that *mut KeyValueEntity
// will not be used multiple async, so KeyValueEntity is Send
#[allow(clippy::non_send_fields_in_send_ty)]
unsafe impl<T: HashTableKeyable + Send> Send for LongerFixedKeysAggregatorState<T> {}

// TODO:(Winter) Hack:
// The *mut KeyValueEntity needs to be used externally, but we can ensure that &*mut KeyValueEntity
// will not be used multiple async, so KeyValueEntity is Sync
unsafe impl<T: HashTableKeyable + Sync> Sync for LongerFixedKeysAggregatorState<T> {}

impl<T: Copy + Send + Sync + Sized + 'static> AggregatorState<HashMethodFixedKeys<T>>
    for LongerFixedKeysAggregatorState<T>
where
    for<'a> HashMethodFixedKeys<T>: HashMethod<HashKey = T, HashKeyRef<'a> = T>,
    for<'a> <HashMethodFixedKeys<T> as HashMethod>::HashKey: HashTableKeyable,
{
    type Key = T;
    type Entity = KeyValueEntity<T, usize>;
    type Iterator = HashMapIteratorKind<T, usize>;

    #[inline(always)]
    fn len(&self) -> usize {
        self.data.len()
    }

    #[inline(always)]
    fn iter(&self) -> Self::Iterator {
        self.data.iter()
    }

    #[inline(always)]
    fn alloc_place(&self, layout: Layout) -> StateAddr {
        self.area.alloc_layout(layout).into()
    }

    #[inline(always)]
    fn entity(&mut self, key: Self::Key, inserted: &mut bool) -> *mut Self::Entity {
        self.data.insert_key(&key, inserted)
    }

    #[inline(always)]
    fn entity_by_key(&mut self, key: &Self::Key, inserted: &mut bool) -> *mut Self::Entity {
        self.entity(*key, inserted)
    }

    #[inline(always)]
    fn is_two_level(&self) -> bool {
        self.two_level_flag
    }

    #[inline(always)]
    fn convert_to_two_level(&mut self) {
        unsafe {
            self.data.convert_to_two_level();
        }
        self.two_level_flag = true;
    }
}

pub struct SerializedKeysAggregatorState {
    pub keys_area: Bump,
    pub state_area: Bump,
    pub data_state_map: HashMapKind<KeysRef, usize>,
    pub two_level_flag: bool,
}

// TODO:(Winter) Hack:
// The *mut KeyValueEntity needs to be used externally, but we can ensure that *mut KeyValueEntity
// will not be used multiple async, so KeyValueEntity is Send
#[allow(clippy::non_send_fields_in_send_ty)]
unsafe impl Send for SerializedKeysAggregatorState {}

// TODO:(Winter) Hack:
// The *mut KeyValueEntity needs to be used externally, but we can ensure that &*mut KeyValueEntity
// will not be used multiple async, so KeyValueEntity is Sync
unsafe impl Sync for SerializedKeysAggregatorState {}

impl AggregatorState<HashMethodSerializer> for SerializedKeysAggregatorState {
    type Key = KeysRef;
    type Entity = KeyValueEntity<KeysRef, usize>;
    type Iterator = HashMapIteratorKind<KeysRef, usize>;

    fn len(&self) -> usize {
        self.data_state_map.len()
    }
    fn iter(&self) -> Self::Iterator {
        self.data_state_map.iter()
    }

    #[inline(always)]
    fn alloc_place(&self, layout: Layout) -> StateAddr {
        self.state_area.alloc_layout(layout).into()
    }

    #[inline(always)]
    fn entity(&mut self, keys: &[u8], inserted: &mut bool) -> *mut Self::Entity {
        let mut keys_ref = KeysRef::create(keys.as_ptr() as usize, keys.len());
        let state_entity = self.data_state_map.insert_key(&keys_ref, inserted);

        if *inserted {
            unsafe {
                // Keys will be destroyed after call we need copy the keys to the memory pool.
                let global_keys = self.keys_area.alloc_slice_copy(keys);
                let inserted_hash = state_entity.get_hash();
                keys_ref.address = global_keys.as_ptr() as usize;
                // TODO: maybe need set key method.
                state_entity.set_key_and_hash(&keys_ref, inserted_hash)
            }
        }

        state_entity
    }

    #[inline(always)]
    fn entity_by_key(&mut self, keys_ref: &KeysRef, inserted: &mut bool) -> *mut Self::Entity {
        let state_entity = self.data_state_map.insert_key(keys_ref, inserted);

        if *inserted {
            unsafe {
                // Keys will be destroyed after call we need copy the keys to the memory pool.
                let data_ptr = keys_ref.address as *mut u8;
                let keys = std::slice::from_raw_parts_mut(data_ptr, keys_ref.length);
                let global_keys = self.keys_area.alloc_slice_copy(keys);
                let inserted_hash = state_entity.get_hash();
                let address = global_keys.as_ptr() as usize;
                let new_keys_ref = KeysRef::create(address, keys_ref.length);
                state_entity.set_key_and_hash(&new_keys_ref, inserted_hash)
            }
        }

        state_entity
    }

    #[inline(always)]
    fn is_two_level(&self) -> bool {
        self.two_level_flag
    }

    #[inline(always)]
    fn convert_to_two_level(&mut self) {
        unsafe {
            self.data_state_map.convert_to_two_level();
        }
        self.two_level_flag = true;
    }
}
