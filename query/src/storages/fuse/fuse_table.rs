//  Copyright 2021 Datafuse Labs.
//
//  Licensed under the Apache License, Version 2.0 (the "License");
//  you may not use this file except in compliance with the License.
//  You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
//  Unless required by applicable law or agreed to in writing, software
//  distributed under the License is distributed on an "AS IS" BASIS,
//  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//  See the License for the specific language governing permissions and
//  limitations under the License.
//

use std::any::Any;
use std::convert::TryFrom;
use std::sync::Arc;

use common_cache::Cache;
use common_datablocks::DataBlock;
use common_exception::ErrorCode;
use common_exception::Result;
use common_meta_app::schema::TableInfo;
use common_meta_app::schema::TableMeta;
use common_meta_app::schema::UpdateTableMetaReq;
use common_meta_types::MatchSeq;
use common_planners::DeletePlan;
use common_planners::Expression;
use common_planners::Extras;
use common_planners::OptimizeTablePlan;
use common_planners::Partitions;
use common_planners::ReadDataSourcePlan;
use common_planners::Statistics;
use common_planners::TruncateTablePlan;
use common_streams::SendableDataBlockStream;
use common_tracing::tracing;
use futures::StreamExt;
use uuid::Uuid;

use crate::pipelines::new::NewPipeline;
use crate::sessions::QueryContext;
use crate::sql::PlanParser;
use crate::sql::OPT_KEY_DATABASE_ID;
use crate::sql::OPT_KEY_LEGACY_SNAPSHOT_LOC;
use crate::sql::OPT_KEY_SNAPSHOT_LOCATION;
use crate::storages::fuse::io::write_meta;
use crate::storages::fuse::io::MetaReaders;
use crate::storages::fuse::io::TableMetaLocationGenerator;
use crate::storages::fuse::meta::ClusterKey;
use crate::storages::fuse::meta::Statistics as FuseStatistics;
use crate::storages::fuse::meta::TableSnapshot;
use crate::storages::fuse::meta::Versioned;
use crate::storages::fuse::operations::AppendOperationLogEntry;
use crate::storages::NavigationPoint;
use crate::storages::StorageContext;
use crate::storages::StorageDescription;
use crate::storages::Table;
use crate::storages::TableStatistics;

#[derive(Clone)]
pub struct FuseTable {
    pub(crate) table_info: TableInfo,
    pub(crate) meta_location_generator: TableMetaLocationGenerator,

    pub(crate) cluster_keys: Vec<Expression>,
    pub(crate) cluster_key_meta: Option<ClusterKey>,
    pub(crate) read_only: bool,
}

impl FuseTable {
    pub fn try_create(_ctx: StorageContext, table_info: TableInfo) -> Result<Box<dyn Table>> {
        let r = Self::do_create(table_info, false)?;
        Ok(r)
    }

    pub fn do_create(table_info: TableInfo, read_only: bool) -> Result<Box<FuseTable>> {
        let storage_prefix = Self::parse_storage_prefix(&table_info)?;
        let cluster_key_meta = table_info.meta.cluster_key();
        let mut cluster_keys = Vec::new();
        if let Some((_, order)) = &cluster_key_meta {
            cluster_keys = PlanParser::parse_exprs(order)?;
        }

        Ok(Box::new(FuseTable {
            table_info,
            cluster_keys,
            cluster_key_meta,
            meta_location_generator: TableMetaLocationGenerator::with_prefix(storage_prefix),
            read_only,
        }))
    }

    pub fn description() -> StorageDescription {
        StorageDescription {
            engine_name: "FUSE".to_string(),
            comment: "FUSE Storage Engine".to_string(),
            support_cluster_key: true,
        }
    }

    pub fn meta_location_generator(&self) -> &TableMetaLocationGenerator {
        &self.meta_location_generator
    }

    pub fn parse_storage_prefix(table_info: &TableInfo) -> Result<String> {
        let table_id = table_info.ident.table_id;
        let db_id = table_info
            .options()
            .get(OPT_KEY_DATABASE_ID)
            .ok_or_else(|| {
                ErrorCode::LogicalError(format!(
                    "Invalid fuse table, table option {} not found",
                    OPT_KEY_DATABASE_ID
                ))
            })?;
        Ok(format!("{}/{}", db_id, table_id))
    }

    #[tracing::instrument(level = "debug", skip(self, ctx), fields(ctx.id = ctx.get_id().as_str()))]
    pub(crate) async fn read_table_snapshot(
        &self,
        ctx: &QueryContext,
    ) -> Result<Option<Arc<TableSnapshot>>> {
        if let Some(loc) = self.snapshot_loc() {
            let reader = MetaReaders::table_snapshot_reader(ctx);
            let ver = self.snapshot_format_version();
            Ok(Some(reader.read(loc.as_str(), None, ver).await?))
        } else {
            Ok(None)
        }
    }

    pub fn snapshot_format_version(&self) -> u64 {
        match self.snapshot_loc() {
            Some(loc) => TableMetaLocationGenerator::snapshot_version(loc.as_str()),
            None => {
                // No snapshot location here, indicates that there are no data of this table yet
                // in this case, we just returns the current snapshot version
                TableSnapshot::VERSION
            }
        }
    }

    pub fn snapshot_loc(&self) -> Option<String> {
        let options = self.table_info.options();

        options
            .get(OPT_KEY_SNAPSHOT_LOCATION)
            // for backward compatibility, we check the legacy table option
            .or_else(|| options.get(OPT_KEY_LEGACY_SNAPSHOT_LOC))
            .cloned()
    }

    pub fn try_from_table(tbl: &dyn Table) -> Result<&FuseTable> {
        tbl.as_any().downcast_ref::<FuseTable>().ok_or_else(|| {
            ErrorCode::LogicalError(format!(
                "expects table of engine FUSE, but got {}",
                tbl.engine()
            ))
        })
    }

    pub fn check_mutable(&self) -> Result<()> {
        if self.read_only {
            Err(ErrorCode::TableNotWritable(format!(
                "Table {} is in read-only mode",
                self.table_info.desc.as_str()
            )))
        } else {
            Ok(())
        }
    }

    pub async fn update_table_meta(
        &self,
        ctx: &QueryContext,
        catalog_name: &str,
        snapshot: &TableSnapshot,
        meta: &mut TableMeta,
    ) -> Result<()> {
        let uuid = snapshot.snapshot_id;
        let snapshot_loc = self
            .meta_location_generator()
            .snapshot_location_from_uuid(&uuid, TableSnapshot::VERSION)?;
        let operator = ctx.get_storage_operator()?;
        write_meta(&operator, &snapshot_loc, snapshot).await?;

        // set new snapshot location
        meta.options
            .insert(OPT_KEY_SNAPSHOT_LOCATION.to_owned(), snapshot_loc.clone());
        // remove legacy options
        meta.options.remove(OPT_KEY_LEGACY_SNAPSHOT_LOC);

        let table_id = self.table_info.ident.table_id;
        let table_version = self.table_info.ident.seq;
        let req = UpdateTableMetaReq {
            table_id,
            seq: MatchSeq::Exact(table_version),
            new_table_meta: meta.clone(),
        };

        let catalog = ctx.get_catalog(catalog_name)?;
        let result = catalog.update_table_meta(req).await;
        match result {
            Ok(_) => {
                if let Some(snapshot_cache) =
                    ctx.get_storage_cache_manager().get_table_snapshot_cache()
                {
                    let cache = &mut snapshot_cache.write().await;
                    cache.put(snapshot_loc, Arc::new(snapshot.clone()));
                }
                Ok(())
            }
            Err(e) => {
                // commit snapshot to meta server failed, try to delete it.
                // "major GC" will collect this, if deletion failure (even after DAL retried)
                let _ = operator.object(&snapshot_loc).delete().await;
                Err(e)
            }
        }
    }

    pub fn transient(&self) -> bool {
        self.table_info.meta.options.contains_key("TRANSIENT")
    }
}

#[async_trait::async_trait]
impl Table for FuseTable {
    fn is_local(&self) -> bool {
        false
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn get_table_info(&self) -> &TableInfo {
        &self.table_info
    }

    fn benefit_column_prune(&self) -> bool {
        true
    }

    fn has_exact_total_row_count(&self) -> bool {
        true
    }

    fn cluster_keys(&self) -> Vec<Expression> {
        self.cluster_keys.clone()
    }

    async fn alter_table_cluster_keys(
        &self,
        ctx: Arc<QueryContext>,
        catalog_name: &str,
        cluster_key_str: String,
    ) -> Result<()> {
        let mut new_table_meta = self.get_table_info().meta.clone();
        new_table_meta = new_table_meta.push_cluster_key(cluster_key_str);
        let cluster_key_meta = new_table_meta.cluster_key();
        let schema = self.schema().as_ref().clone();

        let prev = self.read_table_snapshot(ctx.as_ref()).await?;
        let prev_version = self.snapshot_format_version();
        let prev_timestamp = prev.as_ref().and_then(|v| v.timestamp);
        let prev_snapshot_id = prev.as_ref().map(|v| (v.snapshot_id, prev_version));
        let (summary, segments) = if let Some(v) = prev {
            (v.summary.clone(), v.segments.clone())
        } else {
            (FuseStatistics::default(), vec![])
        };

        let new_snapshot = TableSnapshot::new(
            Uuid::new_v4(),
            &prev_timestamp,
            prev_snapshot_id,
            schema,
            summary,
            segments,
            cluster_key_meta,
        );

        self.update_table_meta(
            ctx.as_ref(),
            catalog_name,
            &new_snapshot,
            &mut new_table_meta,
        )
        .await
    }

    async fn drop_table_cluster_keys(
        &self,
        ctx: Arc<QueryContext>,
        catalog_name: &str,
    ) -> Result<()> {
        if self.cluster_key_meta.is_none() {
            return Ok(());
        }

        let mut new_table_meta = self.get_table_info().meta.clone();
        new_table_meta.default_cluster_key = None;
        new_table_meta.default_cluster_key_id = None;

        let schema = self.schema().as_ref().clone();

        let prev = self.read_table_snapshot(ctx.as_ref()).await?;
        let prev_version = self.snapshot_format_version();
        let prev_timestamp = prev.as_ref().and_then(|v| v.timestamp);
        let prev_snapshot_id = prev.as_ref().map(|v| (v.snapshot_id, prev_version));
        let (summary, segments) = if let Some(v) = prev {
            (v.summary.clone(), v.segments.clone())
        } else {
            (FuseStatistics::default(), vec![])
        };

        let new_snapshot = TableSnapshot::new(
            Uuid::new_v4(),
            &prev_timestamp,
            prev_snapshot_id,
            schema,
            summary,
            segments,
            None,
        );

        self.update_table_meta(
            ctx.as_ref(),
            catalog_name,
            &new_snapshot,
            &mut new_table_meta,
        )
        .await
    }

    #[tracing::instrument(level = "debug", name = "fuse_table_read_partitions", skip(self, ctx), fields(ctx.id = ctx.get_id().as_str()))]
    async fn read_partitions(
        &self,
        ctx: Arc<QueryContext>,
        push_downs: Option<Extras>,
    ) -> Result<(Statistics, Partitions)> {
        self.do_read_partitions(ctx, push_downs).await
    }

    #[tracing::instrument(level = "debug", name = "fuse_table_read2", skip(self, ctx, pipeline), fields(ctx.id = ctx.get_id().as_str()))]
    fn read2(
        &self,
        ctx: Arc<QueryContext>,
        plan: &ReadDataSourcePlan,
        pipeline: &mut NewPipeline,
    ) -> Result<()> {
        self.do_read2(ctx, plan, pipeline)
    }

    fn append2(&self, ctx: Arc<QueryContext>, pipeline: &mut NewPipeline) -> Result<()> {
        self.check_mutable()?;
        self.do_append2(ctx, pipeline)
    }

    #[tracing::instrument(level = "debug", name = "fuse_table_append_data", skip(self, ctx, stream), fields(ctx.id = ctx.get_id().as_str()))]
    async fn append_data(
        &self,
        ctx: Arc<QueryContext>,
        stream: SendableDataBlockStream,
    ) -> Result<SendableDataBlockStream> {
        self.check_mutable()?;
        let log_entry_stream = self.append_chunks(ctx, stream).await?;
        let data_block_stream =
            log_entry_stream.map(|append_log_entry_res| match append_log_entry_res {
                Ok(log_entry) => DataBlock::try_from(log_entry),
                Err(err) => Err(err),
            });
        Ok(Box::pin(data_block_stream))
    }

    #[tracing::instrument(level = "debug", name = "fuse_table_commit_insertion", skip(self, ctx, operations), fields(ctx.id = ctx.get_id().as_str()))]
    async fn commit_insertion(
        &self,
        ctx: Arc<QueryContext>,
        catalog_name: &str,
        operations: Vec<DataBlock>,
        overwrite: bool,
    ) -> Result<()> {
        self.check_mutable()?;
        // only append operation supported currently
        let append_log_entries = operations
            .iter()
            .map(AppendOperationLogEntry::try_from)
            .collect::<Result<Vec<AppendOperationLogEntry>>>()?;
        self.do_commit(ctx, catalog_name, append_log_entries, overwrite)
            .await
    }

    #[tracing::instrument(level = "debug", name = "fuse_table_truncate", skip(self, ctx), fields(ctx.id = ctx.get_id().as_str()))]
    async fn truncate(
        &self,
        ctx: Arc<QueryContext>,
        truncate_plan: TruncateTablePlan,
    ) -> Result<()> {
        self.check_mutable()?;
        self.do_truncate(ctx, truncate_plan.purge, truncate_plan.catalog.as_str())
            .await
    }

    #[tracing::instrument(level = "debug", name = "fuse_table_optimize", skip(self, ctx), fields(ctx.id = ctx.get_id().as_str()))]
    async fn optimize(&self, ctx: Arc<QueryContext>, keep_last_snapshot: bool) -> Result<()> {
        self.check_mutable()?;
        self.do_gc(&ctx, keep_last_snapshot).await
    }

    async fn statistics(&self, _ctx: Arc<QueryContext>) -> Result<Option<TableStatistics>> {
        let s = &self.table_info.meta.statistics;
        Ok(Some(TableStatistics {
            num_rows: Some(s.number_of_rows),
            data_size: Some(s.data_bytes),
            data_size_compressed: Some(s.compressed_data_bytes),
            index_length: None, // we do not have it yet
        }))
    }

    #[tracing::instrument(level = "debug", name = "fuse_table_navigate_to", skip(self, ctx), fields(ctx.id = ctx.get_id().as_str()))]
    async fn navigate_to(
        &self,
        ctx: Arc<QueryContext>,
        point: &NavigationPoint,
    ) -> Result<Arc<dyn Table>> {
        match point {
            NavigationPoint::SnapshotID(snapshot_id) => Ok(self
                .navigate_to_snapshot(ctx.as_ref(), snapshot_id.as_str())
                .await?),
            NavigationPoint::TimePoint(time_point) => {
                Ok(self.navigate_to_time_point(&ctx, *time_point).await?)
            }
        }
    }

    #[tracing::instrument(level = "debug", name = "fuse_table_delete", skip(self, ctx), fields(ctx.id = ctx.get_id().as_str()))]
    async fn delete(&self, ctx: Arc<QueryContext>, delete_plan: DeletePlan) -> Result<()> {
        self.do_delete(ctx, &delete_plan).await
    }

    #[tracing::instrument(level = "debug", name = "fuse_table_compact", skip(self, ctx), fields(ctx.id = ctx.get_id().as_str()))]
    async fn compact(&self, ctx: Arc<QueryContext>, plan: OptimizeTablePlan) -> Result<()> {
        self.do_compact(ctx, &plan).await
    }
}
