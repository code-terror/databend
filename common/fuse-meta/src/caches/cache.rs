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

use common_config::QueryConfig;

use crate::caches::memory_cache::new_bytes_cache;
use crate::caches::memory_cache::BloomIndexCache;
use crate::caches::memory_cache::BloomIndexMetaCache;
use crate::caches::memory_cache::BytesCache;
use crate::caches::new_item_cache;
use crate::caches::ItemCache;
use crate::caches::SegmentInfoCache;
use crate::caches::TableSnapshotCache;

// default number of index meta cached, default 3000 items
static DEFAULT_BLOOM_INDEX_META_CACHE_ITEMS: u64 = 3000;
// default size of cached bloom filter index (in bytes), 1G
static DEFAULT_BLOOM_INDEX_COLUMN_CACHE_SIZE: u64 = 1024 * 1024 * 1024;

/// Where all the caches reside
pub struct CacheManager {
    table_snapshot_cache: Option<TableSnapshotCache>,
    segment_info_cache: Option<SegmentInfoCache>,
    bloom_index_cache: Option<BloomIndexCache>,
    bloom_index_meta_cache: Option<BloomIndexMetaCache>,
    cluster_id: String,
    tenant_id: String,
}

impl CacheManager {
    /// Initialize the caches according to the relevant configurations.
    ///
    /// For convenience, ids of cluster and tenant are also kept
    pub fn init(config: &QueryConfig) -> CacheManager {
        if !config.table_cache_enabled {
            Self {
                table_snapshot_cache: None,
                segment_info_cache: None,
                bloom_index_cache: None,
                bloom_index_meta_cache: None,
                cluster_id: config.cluster_id.clone(),
                tenant_id: config.tenant_id.clone(),
            }
        } else {
            let table_snapshot_cache = Self::new_item_cache(config.table_cache_snapshot_count);
            let segment_info_cache = Self::new_item_cache(config.table_cache_segment_count);
            let bloom_index_cache = Self::new_bytes_cache(DEFAULT_BLOOM_INDEX_META_CACHE_ITEMS);
            let bloom_index_meta_cache =
                Self::new_item_cache(DEFAULT_BLOOM_INDEX_COLUMN_CACHE_SIZE);
            Self {
                table_snapshot_cache,
                segment_info_cache,
                bloom_index_cache,
                bloom_index_meta_cache,
                cluster_id: config.cluster_id.clone(),
                tenant_id: config.tenant_id.clone(),
            }
        }
    }

    pub fn get_table_snapshot_cache(&self) -> Option<TableSnapshotCache> {
        self.table_snapshot_cache.clone()
    }

    pub fn get_table_segment_cache(&self) -> Option<SegmentInfoCache> {
        self.segment_info_cache.clone()
    }

    pub fn get_bloom_index_cache(&self) -> Option<BloomIndexCache> {
        self.bloom_index_cache.clone()
    }

    pub fn get_bloom_index_meta_cache(&self) -> Option<BloomIndexMetaCache> {
        self.bloom_index_meta_cache.clone()
    }

    pub fn get_tenant_id(&self) -> &str {
        self.tenant_id.as_str()
    }

    pub fn get_cluster_id(&self) -> &str {
        self.cluster_id.as_str()
    }

    fn new_item_cache<T>(capacity: u64) -> Option<ItemCache<T>> {
        if capacity > 0 {
            Some(new_item_cache(capacity))
        } else {
            None
        }
    }

    fn new_bytes_cache(capacity: u64) -> Option<BytesCache> {
        if capacity > 0 {
            Some(new_bytes_cache(capacity))
        } else {
            None
        }
    }
}
