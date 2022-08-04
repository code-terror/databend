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

use common_exception::Result;
use common_fuse_meta::caches::TenantLabel;
use common_fuse_meta::meta::SegmentInfo;
use common_fuse_meta::meta::SegmentInfoVersion;
use common_fuse_meta::meta::SnapshotVersion;
use common_fuse_meta::meta::TableSnapshot;
use futures::io::BufReader;
use opendal::BytesReader;

use super::cached_reader::CachedReader;
use super::cached_reader::HasTenantLabel;
use super::cached_reader::Loader;
use super::versioned_reader::VersionedReader;
use crate::sessions::TableContext;

/// Provider of [BufReader]
///
/// Mainly used as a auxiliary facility in the implementation of [Loader], such that the acquirement
/// of an [BufReader] can be deferred or avoided (e.g. if hits cache).
#[async_trait::async_trait]
pub trait BufReaderProvider {
    async fn buf_reader(&self, path: &str, len: Option<u64>) -> Result<BufReader<BytesReader>>;
}

pub type SegmentInfoReader<'a> = CachedReader<SegmentInfo, &'a dyn TableContext>;
pub type TableSnapshotReader<'a> = CachedReader<TableSnapshot, &'a dyn TableContext>;

pub struct MetaReaders;

impl MetaReaders {
    pub fn segment_info_reader(ctx: &dyn TableContext) -> SegmentInfoReader {
        SegmentInfoReader::new(
            ctx.get_storage_cache_manager().get_table_segment_cache(),
            ctx,
            "SEGMENT_INFO_CACHE".to_owned(),
        )
    }

    pub fn table_snapshot_reader(ctx: &dyn TableContext) -> TableSnapshotReader {
        TableSnapshotReader::new(
            ctx.get_storage_cache_manager().get_table_snapshot_cache(),
            ctx,
            "SNAPSHOT_CACHE".to_owned(),
        )
    }
}

#[async_trait::async_trait]
impl<T> Loader<TableSnapshot> for T
where T: BufReaderProvider + Sync
{
    async fn load(
        &self,
        key: &str,
        length_hint: Option<u64>,
        version: u64,
    ) -> Result<TableSnapshot> {
        let version = SnapshotVersion::try_from(version)?;
        let reader = self.buf_reader(key, length_hint).await?;
        version.read(reader).await
    }
}

#[async_trait::async_trait]
impl<T> Loader<SegmentInfo> for T
where T: BufReaderProvider + Sync
{
    async fn load(&self, key: &str, length_hint: Option<u64>, version: u64) -> Result<SegmentInfo> {
        let version = SegmentInfoVersion::try_from(version)?;
        let reader = self.buf_reader(key, length_hint).await?;
        version.read(reader).await
    }
}

#[async_trait::async_trait]
impl BufReaderProvider for &dyn TableContext {
    async fn buf_reader(&self, path: &str, len: Option<u64>) -> Result<BufReader<BytesReader>> {
        let operator = self.get_storage_operator()?;
        let object = operator.object(path);

        let len = match len {
            Some(l) => l,
            None => {
                let meta = object.metadata().await?;

                meta.content_length()
            }
        };

        let reader = object.range_reader(..len).await?;
        let read_buffer_size = self.get_settings().get_storage_read_buffer_size()?;
        Ok(BufReader::with_capacity(
            read_buffer_size as usize,
            Box::new(reader),
        ))
    }
}

impl HasTenantLabel for &dyn TableContext {
    fn tenant_label(&self) -> TenantLabel {
        ctx_tenant_label(*self)
    }
}

impl HasTenantLabel for Arc<dyn TableContext> {
    fn tenant_label(&self) -> TenantLabel {
        ctx_tenant_label(self.as_ref())
    }
}

fn ctx_tenant_label(ctx: &dyn TableContext) -> TenantLabel {
    let mgr = ctx.get_storage_cache_manager();
    TenantLabel {
        tenant_id: mgr.get_tenant_id().to_owned(),
        cluster_id: mgr.get_cluster_id().to_owned(),
    }
}
