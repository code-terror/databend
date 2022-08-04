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

use async_recursion::async_recursion;
use common_base::base::tokio;
use common_base::base::tokio::sync::Semaphore;
use common_datablocks::DataBlock;
use common_datavalues::DataField;
use common_datavalues::DataSchema;
use common_datavalues::DataSchemaRef;
use common_exception::ErrorCode;
use common_exception::Result;
use common_meta_app::schema::TableInfo;
use common_planners::Expression;
use common_planners::Extras;
use common_planners::Partitions;
use common_planners::ReadDataSourcePlan;
use common_planners::Statistics;
use common_planners::TruncateTablePlan;
use futures::TryStreamExt;
use opendal::ObjectMode;
use opendal::Operator;

use super::hive_partition_pruner::HivePartitionPruner;
use super::hive_table_options::HiveTableOptions;
use super::HiveCatalog;
use crate::catalogs::catalog_manager::CATALOG_HIVE;
use crate::catalogs::hive::hive_partition_filler::HivePartitionFiller;
use crate::catalogs::hive::hive_table_source::HiveTableSource;
use crate::catalogs::hive::HivePartInfo;
use crate::pipelines::processors::port::OutputPort;
use crate::pipelines::processors::processor::ProcessorPtr;
use crate::pipelines::processors::SyncSource;
use crate::pipelines::processors::SyncSourcer;
use crate::pipelines::Pipeline;
use crate::pipelines::SourcePipeBuilder;
use crate::sessions::TableContext;
use crate::storages::hive::HiveParquetBlockReader;
use crate::storages::Table;
use crate::storages::TableStatistics;

/// ! Dummy implementation for HIVE TABLE

pub const HIVE_TABLE_ENGIE: &str = "hive";

pub struct HiveTable {
    table_info: TableInfo,
    table_options: HiveTableOptions,
}

impl HiveTable {
    pub fn try_create(table_info: TableInfo) -> Result<HiveTable> {
        let table_options = table_info.engine_options().try_into()?;
        Ok(HiveTable {
            table_info,
            table_options,
        })
    }

    fn filter_hive_partition_from_partition_keys(
        &self,
        projections: Vec<usize>,
    ) -> (Vec<usize>, Vec<DataField>) {
        let partition_keys = &self.table_options.partition_keys;
        match partition_keys {
            Some(partition_keys) => {
                let schema = self.table_info.schema();
                let mut not_partitions = vec![];
                let mut partition_fields = vec![];
                for i in projections.into_iter() {
                    let field = schema.field(i);
                    if !partition_keys.contains(field.name()) {
                        not_partitions.push(i);
                    } else {
                        partition_fields.push(field.clone());
                    }
                }
                (not_partitions, partition_fields)
            }
            None => (projections, vec![]),
        }
    }

    #[inline]
    pub fn do_read2(
        &self,
        ctx: Arc<dyn TableContext>,
        plan: &ReadDataSourcePlan,
        pipeline: &mut Pipeline,
    ) -> Result<()> {
        let push_downs = &plan.push_downs;
        let block_reader = self.create_block_reader(&ctx, push_downs)?;

        let parts_len = plan.parts.len();
        let max_threads = ctx.get_settings().get_max_threads()? as usize;
        let max_threads = std::cmp::min(parts_len, max_threads);

        let mut source_builder = SourcePipeBuilder::create();

        for _index in 0..std::cmp::max(1, max_threads) {
            let output = OutputPort::create();
            source_builder.add_source(
                output.clone(),
                HiveTableSource::create(ctx.clone(), output, block_reader.clone())?,
            );
        }

        pipeline.add_pipe(source_builder.finalize());
        Ok(())
    }

    fn create_block_reader(
        &self,
        ctx: &Arc<dyn TableContext>,
        push_downs: &Option<Extras>,
    ) -> Result<Arc<HiveParquetBlockReader>> {
        let projection = if let Some(Extras {
            projection: Some(prj),
            ..
        }) = push_downs
        {
            prj.clone()
        } else {
            (0..self.table_info.schema().fields().len())
                .into_iter()
                .collect::<Vec<usize>>()
        };

        let (projection, partition_fields) =
            self.filter_hive_partition_from_partition_keys(projection);

        let hive_partition_filler = if !partition_fields.is_empty() {
            Some(HivePartitionFiller::create(partition_fields))
        } else {
            None
        };

        let operator = ctx.get_storage_operator()?;
        let table_schema = self.table_info.schema();
        // todo, support csv, orc format
        HiveParquetBlockReader::create(operator, table_schema, projection, hive_partition_filler)
    }

    fn get_column_schemas(&self, columns: Vec<String>) -> Result<Arc<DataSchema>> {
        let mut fields = Vec::with_capacity(columns.len());
        for column in columns {
            let schema = self.table_info.schema();
            let data_field = schema.field_with_name(&column)?;
            fields.push(data_field.clone());
        }

        Ok(Arc::new(DataSchema::new(fields)))
    }

    async fn get_query_locations_from_partition_table(
        &self,
        ctx: Arc<dyn TableContext>,
        partition_keys: Vec<String>,
        filters: &[Expression],
        location: String,
    ) -> Result<Vec<(String, Option<String>)>> {
        let hive_catalog = ctx.get_catalog(CATALOG_HIVE)?;
        let hive_catalog = hive_catalog.as_any().downcast_ref::<HiveCatalog>().unwrap();

        // todo may use get_partition_names_ps to filter
        let table_info = self.table_info.desc.split('.').collect::<Vec<&str>>();
        let mut partitions = hive_catalog
            .get_partition_names_async(table_info[0].to_string(), table_info[1].to_string(), -1)
            .await?;

        if !filters.is_empty() {
            let partition_schemas = self.get_column_schemas(partition_keys.clone())?;
            let partition_pruner =
                HivePartitionPruner::create(ctx, filters[0].clone(), partition_schemas);
            partitions = partition_pruner.prune(partitions)?;
        }

        let partitions = partitions
            .into_iter()
            .map(|part| (format!("{}{}/", location, part), Some(part)))
            .collect::<Vec<(String, Option<String>)>>();
        Ok(partitions)
    }

    async fn get_query_locations(
        &self,
        ctx: Arc<dyn TableContext>,
        push_downs: &Option<Extras>,
    ) -> Result<Vec<(String, Option<String>)>> {
        let path = match &self.table_options.location {
            Some(path) => path,
            None => {
                return Err(ErrorCode::TableInfoError(format!(
                    "{}, table location is empty",
                    self.table_info.name
                )));
            }
        };
        let location = convert_hdfs_path(path, true);

        if let Some(partition_keys) = &self.table_options.partition_keys {
            if !partition_keys.is_empty() {
                if let Some(extras) = push_downs {
                    if extras.filters.len() > 1 {
                        return Err(ErrorCode::UnImplement(format!(
                            "more than one filters, {:?}",
                            extras.filters
                        )));
                    }
                    return self
                        .get_query_locations_from_partition_table(
                            ctx.clone(),
                            partition_keys.clone(),
                            &extras.filters,
                            location.clone(),
                        )
                        .await;
                }
            }
        }

        Ok(vec![(location, None)])
    }

    #[tracing::instrument(level = "info", skip(self, ctx))]
    async fn list_files_from_dirs(
        &self,
        ctx: Arc<dyn TableContext>,
        dirs: Vec<(String, Option<String>)>,
    ) -> Result<Vec<(String, Option<String>)>> {
        let operator = ctx.get_storage_operator()?;

        let sem = Arc::new(Semaphore::new(60));

        let mut tasks = Vec::with_capacity(dirs.len());
        for (dir, partition) in dirs {
            let sem_t = sem.clone();
            let operator_t = operator.clone();
            let dir_t = dir.to_string();
            let task =
                tokio::spawn(async move { list_files_from_dir(operator_t, dir_t, sem_t).await });
            tasks.push((task, partition));
        }

        let mut all_files = vec![];
        for (task, partition) in tasks {
            let files = task.await.unwrap()?;
            for file in files {
                all_files.push((file, partition.clone()));
            }
        }

        Ok(all_files)
    }

    #[tracing::instrument(level = "info", skip(self, ctx))]
    async fn do_read_partitions(
        &self,
        ctx: Arc<dyn TableContext>,
        push_downs: Option<Extras>,
    ) -> Result<(Statistics, Partitions)> {
        let dirs = self.get_query_locations(ctx.clone(), &push_downs).await?;
        let all_files = self.list_files_from_dirs(ctx, dirs).await?;

        let partitions = all_files
            .into_iter()
            .map(|(filename, partition)| HivePartInfo::create(filename, partition))
            .collect();

        Ok((Default::default(), partitions))
    }
}

#[async_trait::async_trait]
impl Table for HiveTable {
    fn is_local(&self) -> bool {
        false
    }

    fn as_any(&self) -> &(dyn std::any::Any + 'static) {
        todo!()
    }

    fn get_table_info(&self) -> &TableInfo {
        &self.table_info
    }

    fn benefit_column_prune(&self) -> bool {
        true
    }

    fn has_exact_total_row_count(&self) -> bool {
        false
    }

    async fn read_partitions(
        &self,
        ctx: Arc<dyn TableContext>,
        push_downs: Option<Extras>,
    ) -> Result<(Statistics, Partitions)> {
        self.do_read_partitions(ctx, push_downs).await
    }

    fn table_args(&self) -> Option<Vec<Expression>> {
        None
    }

    fn read2(
        &self,
        ctx: Arc<dyn TableContext>,
        plan: &ReadDataSourcePlan,
        pipeline: &mut Pipeline,
    ) -> Result<()> {
        self.do_read2(ctx, plan, pipeline)
    }

    async fn commit_insertion(
        &self,
        _ctx: Arc<dyn TableContext>,
        _catalog_name: &str,
        _operations: Vec<DataBlock>,
        _overwrite: bool,
    ) -> Result<()> {
        Err(ErrorCode::UnImplement(format!(
            "commit_insertion operation for table {} is not implemented, table engine is {}",
            self.name(),
            self.get_table_info().meta.engine
        )))
    }

    async fn truncate(
        &self,
        _ctx: Arc<dyn TableContext>,
        _truncate_plan: TruncateTablePlan,
    ) -> Result<()> {
        Err(ErrorCode::UnImplement(format!(
            "truncate for table {} is not implemented",
            self.name()
        )))
    }

    async fn optimize(&self, _ctx: Arc<dyn TableContext>, _keep_last_snapshot: bool) -> Result<()> {
        Ok(())
    }

    async fn statistics(&self, _ctx: Arc<dyn TableContext>) -> Result<Option<TableStatistics>> {
        Ok(None)
    }
}

// Dummy Impl
struct HiveSource {
    finish: bool,
    schema: DataSchemaRef,
}

impl HiveSource {
    #[allow(dead_code)]
    pub fn create(
        ctx: Arc<dyn TableContext>,
        output: Arc<OutputPort>,
        schema: DataSchemaRef,
    ) -> Result<ProcessorPtr> {
        SyncSourcer::create(ctx, output, HiveSource {
            finish: false,
            schema,
        })
    }
}

impl SyncSource for HiveSource {
    const NAME: &'static str = "HiveSource";

    fn generate(&mut self) -> Result<Option<DataBlock>> {
        if self.finish {
            return Ok(None);
        }

        self.finish = true;
        Ok(Some(DataBlock::empty_with_schema(self.schema.clone())))
    }
}

// convert hdfs path format to opendal path formated
//
// there are two rules:
// 1. erase the schema related info from hdfs path, for example, hdfs://namenode:8020/abc/a is converted to /abc/a
// 2. if the path is dir, append '/' if necessary
// org.apache.hadoop.fs.Path#Path(String pathString) shows how to parse hdfs path
pub fn convert_hdfs_path(hdfs_path: &str, is_dir: bool) -> String {
    let mut start = 0;
    let slash = hdfs_path.find('/');
    let colon = hdfs_path.find(':');
    if let Some(colon) = colon {
        match slash {
            Some(slash) => {
                if colon < slash {
                    start = colon + 1;
                }
            }
            None => {
                start = colon + 1;
            }
        }
    }

    let mut path = &hdfs_path[start..];
    start = 0;
    if path.starts_with("//") && path.len() > 2 {
        path = &path[2..];
        let next_slash = path.find('/');
        start = match next_slash {
            Some(slash) => slash,
            None => path.len(),
        };
    }
    path = &path[start..];

    let end_with_slash = path.ends_with('/');
    let mut format_path = path.to_string();
    if is_dir && !end_with_slash {
        format_path.push('/')
    }
    format_path
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::convert_hdfs_path;

    #[test]
    fn test_convert_hdfs_path() {
        let mut m = HashMap::new();
        m.insert("hdfs://namenode:8020/user/a", "/user/a/");
        m.insert("hdfs://namenode:8020/user/a/", "/user/a/");
        m.insert("hdfs://namenode:8020/", "/");
        m.insert("hdfs://namenode:8020", "/");
        m.insert("/user/a", "/user/a/");
        m.insert("/", "/");

        for (hdfs_path, expected_path) in &m {
            let path = convert_hdfs_path(*hdfs_path, true);
            assert_eq!(path, *expected_path);
        }
    }
}

#[async_recursion]
async fn list_files_from_dir(
    operator: Operator,
    location: String,
    sem: Arc<Semaphore>,
) -> Result<Vec<String>> {
    let (files, dirs) = do_list_files_from_dir(operator.clone(), location, sem.clone()).await?;
    let mut all_files = files;
    let mut tasks = Vec::with_capacity(dirs.len());
    for dir in dirs {
        let sem_t = sem.clone();
        let operator_t = operator.clone();
        let task = tokio::spawn(async move { list_files_from_dir(operator_t, dir, sem_t).await });
        tasks.push(task);
    }

    // let dir_files = tasks.map(|task| task.await.unwrap()).flatten().collect::<Vec<_>>();
    // all_files.extend(dir_files);

    for task in tasks {
        let files = task.await.unwrap()?;
        all_files.extend(files);
    }

    Ok(all_files)
}

async fn do_list_files_from_dir(
    operator: Operator,
    location: String,
    sem: Arc<Semaphore>,
) -> Result<(Vec<String>, Vec<String>)> {
    let _a = sem.acquire().await.unwrap();
    let object = operator.object(&location);
    let mut m = object.list().await?;

    let mut all_files = vec![];
    let mut all_dirs = vec![];
    while let Some(de) = m.try_next().await? {
        let path = de.path();
        let file_offset = path.rfind('/').unwrap_or_default() + 1;
        if path[file_offset..].starts_with('.') || path[file_offset..].starts_with('_') {
            continue;
        }
        match de.mode() {
            ObjectMode::FILE => {
                all_files.push(path.to_string());
            }
            ObjectMode::DIR => {
                all_dirs.push(path.to_string());
            }
            _ => {
                return Err(ErrorCode::ReadTableDataError(format!(
                    "{} couldn't get file mode",
                    path
                )));
            }
        }
    }
    Ok((all_files, all_dirs))
}
