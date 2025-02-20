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

use std::any::Any;
use std::collections::VecDeque;
use std::str::FromStr;
use std::sync::Arc;

use common_datablocks::DataBlock;
use common_exception::ErrorCode;
use common_exception::Result;
use common_formats::output_format::OutputFormatType;
use common_meta_app::schema::TableInfo;
use common_pipeline::processors::port::InputPort;
use common_pipeline::SinkPipeBuilder;
use common_planners::Extras;
use common_planners::Partitions;
use common_planners::ReadDataSourcePlan;
use common_planners::StageTableInfo;
use common_planners::Statistics;
use common_planners::TruncateTablePlan;
use parking_lot::Mutex;
use tracing::info;

use super::StageSourceHelper;
use crate::pipelines::processors::port::OutputPort;
use crate::pipelines::processors::ContextSink;
use crate::pipelines::processors::TransformLimit;
use crate::pipelines::Pipeline;
use crate::pipelines::SourcePipeBuilder;
use crate::sessions::TableContext;
use crate::storages::Table;

pub struct StageTable {
    table_info: StageTableInfo,
    // This is no used but a placeholder.
    // But the Table trait need it:
    // fn get_table_info(&self) -> &TableInfo).
    table_info_placeholder: TableInfo,
}

impl StageTable {
    pub fn try_create(table_info: StageTableInfo) -> Result<Arc<dyn Table>> {
        let table_info_placeholder = TableInfo::default().set_schema(table_info.schema());

        Ok(Arc::new(Self {
            table_info,
            table_info_placeholder,
        }))
    }
}

#[async_trait::async_trait]
impl Table for StageTable {
    fn as_any(&self) -> &dyn Any {
        self
    }

    // External stage has no table info yet.
    fn get_table_info(&self) -> &TableInfo {
        &self.table_info_placeholder
    }

    async fn read_partitions(
        &self,
        _ctx: Arc<dyn TableContext>,
        _push_downs: Option<Extras>,
    ) -> Result<(Statistics, Partitions)> {
        Ok((Statistics::default(), vec![]))
    }

    fn read2(
        &self,
        ctx: Arc<dyn TableContext>,
        _plan: &ReadDataSourcePlan,
        pipeline: &mut Pipeline,
    ) -> Result<()> {
        let settings = ctx.get_settings();
        let mut builder = SourcePipeBuilder::create();
        let table_info = &self.table_info;
        let schema = table_info.schema.clone();
        let mut files_deque = VecDeque::with_capacity(table_info.files.len());
        for f in &table_info.files {
            files_deque.push_back(f.to_string());
        }
        let files = Arc::new(Mutex::new(files_deque));

        let stage_source = StageSourceHelper::try_create(ctx, schema, table_info.clone(), files)?;

        for _index in 0..settings.get_max_threads()? {
            let output = OutputPort::create();
            builder.add_source(output.clone(), stage_source.get_splitter(output)?);
        }
        pipeline.add_pipe(builder.finalize());

        pipeline.add_transform(|transform_input_port, transform_output_port| {
            stage_source.get_deserializer(transform_input_port, transform_output_port)
        })?;

        let limit = self.table_info.stage_info.copy_options.size_limit;
        if limit > 0 {
            pipeline.resize(1)?;
            pipeline.add_transform(|transform_input_port, transform_output_port| {
                TransformLimit::try_create(
                    Some(limit),
                    0,
                    transform_input_port,
                    transform_output_port,
                )
            })?;
        }
        Ok(())
    }

    fn append2(&self, ctx: Arc<dyn TableContext>, pipeline: &mut Pipeline) -> Result<()> {
        let mut sink_pipeline_builder = SinkPipeBuilder::create();
        for _ in 0..pipeline.output_len() {
            let input_port = InputPort::create();
            sink_pipeline_builder.add_sink(
                input_port.clone(),
                ContextSink::create(input_port, ctx.clone()),
            );
        }
        pipeline.add_pipe(sink_pipeline_builder.finalize());
        Ok(())
    }

    // TODO use tmp file_name & rename to have atomic commit
    async fn commit_insertion(
        &self,
        ctx: Arc<dyn TableContext>,
        _catalog_name: &str,
        operations: Vec<DataBlock>,
        _overwrite: bool,
    ) -> Result<()> {
        let format_name = format!(
            "{:?}",
            self.table_info.stage_info.file_format_options.format
        );
        let path = format!(
            "{}{}.{}",
            self.table_info.path,
            uuid::Uuid::new_v4(),
            format_name.to_ascii_lowercase()
        );
        info!(
            "try commit stage table {} to file {path}",
            self.table_info.stage_info.stage_name
        );

        let op = StageSourceHelper::get_op(&ctx, &self.table_info.stage_info).await?;

        let fmt = OutputFormatType::from_str(format_name.as_str())?;
        let mut format_settings = ctx.get_format_settings()?;

        let format_options = &self.table_info.stage_info.file_format_options;
        {
            format_settings.skip_header = format_options.skip_header;
            if !format_options.field_delimiter.is_empty() {
                format_settings.field_delimiter =
                    format_options.field_delimiter.as_bytes().to_vec();
            }
            if !format_options.record_delimiter.is_empty() {
                format_settings.record_delimiter =
                    format_options.record_delimiter.as_bytes().to_vec();
            }
        }

        let mut output_format = fmt.create_format(self.table_info.schema(), format_settings);

        let prefix = output_format.serialize_prefix()?;
        let written_bytes: usize = operations.iter().map(|b| b.memory_size()).sum();
        let mut bytes = Vec::with_capacity(written_bytes + prefix.len());
        bytes.extend_from_slice(&prefix);
        for block in operations {
            let bs = output_format.serialize_block(&block)?;
            bytes.extend_from_slice(bs.as_slice());
        }

        let bs = output_format.finalize()?;
        bytes.extend_from_slice(bs.as_slice());

        ctx.get_dal_context()
            .get_metrics()
            .inc_write_bytes(bytes.len());

        let object = op.object(&path);
        object.write(bytes.as_slice()).await?;
        Ok(())
    }

    // Truncate the stage file.
    async fn truncate(
        &self,
        _ctx: Arc<dyn TableContext>,
        _truncate_plan: TruncateTablePlan,
    ) -> Result<()> {
        Err(ErrorCode::UnImplement(
            "S3 external table truncate() unimplemented yet!",
        ))
    }
}
