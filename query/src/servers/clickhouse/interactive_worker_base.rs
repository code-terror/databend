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

use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use common_base::base::tokio;
use common_base::base::tokio::sync::mpsc::channel;
use common_base::base::tokio::time::interval;
use common_base::base::ProgressValues;
use common_base::base::TrySpawn;
use common_datablocks::DataBlock;
use common_datavalues::DataSchemaRef;
use common_exception::ErrorCode;
use common_exception::Result;
use common_exception::ToErrorCode;
use common_planners::InsertPlan;
use common_planners::PlanNode;
use futures::channel::mpsc;
use futures::channel::mpsc::Receiver;
use futures::SinkExt;
use futures::StreamExt;
use metrics::histogram;
use opensrv_clickhouse::types::Block as ClickHouseBlock;
use opensrv_clickhouse::CHContext;
use tokio_stream::wrappers::IntervalStream;
use tokio_stream::wrappers::ReceiverStream;
use tracing::debug;
use tracing::error;
use tracing::warn;

use super::writers::from_clickhouse_block;
use crate::interpreters::Interpreter;
use crate::interpreters::InterpreterFactory;
use crate::interpreters::InterpreterFactoryV2;
use crate::pipelines::processors::port::OutputPort;
use crate::pipelines::processors::SyncReceiverCkSource;
use crate::pipelines::SourcePipeBuilder;
use crate::servers::utils::use_planner_v2;
use crate::sessions::QueryContext;
use crate::sessions::SessionRef;
use crate::sessions::TableContext;
use crate::sql::DfParser;
use crate::sql::PlanParser;
use crate::sql::Planner;

pub struct InteractiveWorkerBase;

pub enum BlockItem {
    Block(Result<DataBlock>),
    // for insert prepare, we do not need to send another block again
    InsertSample(DataBlock),
    ProgressTicker(ProgressValues),
}

impl InteractiveWorkerBase {
    pub async fn do_query(
        ch_ctx: &mut CHContext,
        session: SessionRef,
    ) -> Result<Receiver<BlockItem>> {
        let query = &ch_ctx.state.query;
        debug!("{}", query);

        let ctx = session.create_query_context().await?;
        ctx.attach_query_str(query);

        let statements = DfParser::parse_sql(query, ctx.get_current_session().get_type());

        if use_planner_v2(&ctx.get_settings(), &statements)? {
            let mut planner = Planner::new(ctx.clone());
            let interpreter = planner
                .plan_sql(query)
                .await
                .and_then(|v| InterpreterFactoryV2::get(ctx.clone(), &v.0))?;
            Self::process_query(ctx, interpreter).await
        } else {
            let statements = statements.unwrap().0;
            let plan = PlanParser::build_plan(statements, ctx.clone()).await?;

            match plan {
                PlanNode::Insert(ref insert) => {
                    // It has select plan, so we do not need to consume data from client
                    // data is from server and insert into server, just behave like select query
                    if insert.has_select_plan() {
                        let interpreter = InterpreterFactory::get(ctx.clone(), plan)?;
                        return Self::process_query(ctx, interpreter).await;
                    }

                    Self::process_insert_query(insert.clone(), ch_ctx, ctx).await
                }
                _ => {
                    let interpreter = InterpreterFactory::get(ctx.clone(), plan)?;
                    Self::process_query(ctx, interpreter).await
                }
            }
        }
    }

    pub async fn process_insert_query(
        insert: InsertPlan,
        ch_ctx: &mut CHContext,
        ctx: Arc<QueryContext>,
    ) -> Result<Receiver<BlockItem>> {
        let sample_block = DataBlock::empty_with_schema(insert.schema());
        let (sender, rec) = channel(2);
        ch_ctx.state.out = Some(sender);

        let sc = sample_block.schema().clone();
        let stream = ReceiverStream::new(rec);
        let ck_stream = FromClickHouseBlockStream {
            input: stream,
            schema: sc.clone(),
        };

        let interpreter = InterpreterFactory::get(ctx.clone(), PlanNode::Insert(insert))?;
        let name = interpreter.name().to_string();

        let output_port = OutputPort::create();
        let sync_receiver_ck_source = SyncReceiverCkSource::create(
            ctx.clone(),
            ck_stream.input.into_inner(),
            output_port.clone(),
            sc,
        )?;
        let mut source_pipe_builder = SourcePipeBuilder::create();
        source_pipe_builder.add_source(output_port, sync_receiver_ck_source);

        let _ = interpreter
            .set_source_pipe_builder(Option::from(source_pipe_builder))
            .map_err(|e| error!("interpreter.set_source_pipe_builder.error: {:?}", e));

        let (mut tx, rx) = mpsc::channel(2);

        tx.send(BlockItem::InsertSample(sample_block)).await.ok();

        // the data is coming in async mode
        let sent_all_data = ch_ctx.state.sent_all_data.clone();
        let start = Instant::now();
        ctx.try_spawn(async move {
            interpreter.execute().await.unwrap();
            sent_all_data.notify_one();
        })?;
        histogram!(
            super::clickhouse_metrics::METRIC_INTERPRETER_USEDTIME,
            start.elapsed(),
            "interpreter" => name
        );

        Ok(rx)
    }

    pub async fn process_query(
        ctx: Arc<QueryContext>,
        interpreter: Arc<dyn Interpreter>,
    ) -> Result<Receiver<BlockItem>> {
        let start = Instant::now();
        let name = interpreter.name().to_string();
        histogram!(
            super::clickhouse_metrics::METRIC_INTERPRETER_USEDTIME,
            start.elapsed(),
            "interpreter" => name
        );

        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_clone = cancel.clone();

        let (tx, rx) = mpsc::channel(2);
        let mut data_tx = tx.clone();
        let mut progress_tx = tx;

        let progress_ctx = ctx.clone();
        let mut interval_stream = IntervalStream::new(interval(Duration::from_millis(30)));
        tokio::spawn(async move {
            let mut prev_progress_values = ProgressValues::default();
            while !cancel.load(Ordering::Relaxed) {
                let _ = interval_stream.next().await;
                let cur_progress_values = progress_ctx.get_scan_progress_value();
                let diff_progress_values = ProgressValues {
                    rows: cur_progress_values.rows - prev_progress_values.rows,
                    bytes: cur_progress_values.bytes - prev_progress_values.bytes,
                };

                if diff_progress_values.rows > 0 {
                    prev_progress_values = cur_progress_values;
                    progress_tx
                        .send(BlockItem::ProgressTicker(diff_progress_values))
                        .await
                        .ok();
                }
            }
        });

        let query_result = ctx.try_spawn(async move {
            // Query log start.
            let _ = interpreter
                .start()
                .await
                .map_err(|e| error!("interpreter.start.error: {:?}", e));

            // Execute and read stream data.
            let data_stream = interpreter.execute();
            let mut data_stream = match data_stream.await {
                Ok(stream) => stream,
                Err(e) => {
                    cancel_clone.store(true, Ordering::Relaxed);
                    return Err(e);
                }
            };

            tokio::spawn(async move {
                'worker: loop {
                    match data_stream.next().await {
                        None => {
                            break 'worker;
                        }
                        Some(Ok(data)) => {
                            if let Err(cause) = data_tx.send(BlockItem::Block(Ok(data))).await {
                                warn!("Cannot send data to channel, cause: {:?}", cause);
                                break 'worker;
                            }
                        }
                        Some(Err(error_code)) => {
                            if let Err(cause) =
                                data_tx.send(BlockItem::Block(Err(error_code))).await
                            {
                                warn!("Cannot send data to channel, cause: {:?}", cause);
                            }
                            break 'worker;
                        }
                    }
                }

                let _ = interpreter
                    .finish()
                    .await
                    .map_err(|e| error!("interpreter.finish.error: {:?}", e));
                cancel_clone.store(true, Ordering::Relaxed);
                Ok::<(), ErrorCode>(())
            });

            Ok(())
        })?;
        let query_result = query_result.await.map_err_to_code(
            ErrorCode::TokioError,
            || "Cannot join handle from context's runtime",
        )?;
        query_result.map(|_| rx)
    }
}

pub struct FromClickHouseBlockStream {
    input: ReceiverStream<ClickHouseBlock>,
    schema: DataSchemaRef,
}

impl futures::stream::Stream for FromClickHouseBlockStream {
    type Item = Result<DataBlock>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.input.poll_next_unpin(cx).map(|x| match x {
            Some(v) => Some(from_clickhouse_block(self.schema.clone(), v)),
            _ => None,
        })
    }
}
