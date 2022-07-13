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

use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use common_datavalues::DataSchemaRef;
use common_exception::ErrorCode;
use common_exception::Result;
use common_planners::Extras;
use common_tracing::tracing;
use futures::StreamExt;
use futures::TryStreamExt;

use crate::sessions::QueryContext;
use crate::storages::fuse::io::MetaReaders;
use crate::storages::fuse::meta::BlockMeta;
use crate::storages::fuse::meta::SegmentInfo;
use crate::storages::fuse::meta::TableSnapshot;
use crate::storages::index::RangeFilter;
use crate::storages::index::StatisticsOfColumns;

pub struct BlockPruner {
    table_snapshot: Arc<TableSnapshot>,
}

type Pred = Box<dyn Fn(&StatisticsOfColumns) -> Result<bool> + Send + Sync + Unpin>;
impl BlockPruner {
    pub fn new(table_snapshot: Arc<TableSnapshot>) -> Self {
        Self { table_snapshot }
    }

    #[tracing::instrument(level = "debug", name="block_pruner_apply", skip(self, schema, ctx), fields(ctx.id = ctx.get_id().as_str()))]
    pub async fn apply(
        &self,
        ctx: &QueryContext,
        schema: DataSchemaRef,
        push_down: &Option<Extras>,
    ) -> Result<Vec<(usize, BlockMeta)>> {
        let block_pred: Pred = match push_down {
            Some(exprs) if !exprs.filters.is_empty() => {
                // for the time being, we only handle the first expr
                let range_filter =
                    RangeFilter::try_create(Arc::new(ctx.clone()), &exprs.filters[0], schema)?;
                Box::new(move |v: &StatisticsOfColumns| range_filter.eval(v))
            }
            _ => Box::new(|_: &StatisticsOfColumns| Ok(true)),
        };

        let segment_locs = self.table_snapshot.segments.clone();
        let segment_num = segment_locs.len();

        if segment_locs.is_empty() {
            return Ok(vec![]);
        };

        let limit = push_down
            .as_ref()
            .filter(|p| p.order_by.is_empty())
            .and_then(|p| p.limit)
            .unwrap_or(usize::MAX);

        // Segments and blocks are accumulated concurrently, thus an atomic counter is used
        // to **try** collecting as less blocks as possible. But concurrency is preferred to
        // "accuracy". In [FuseTable::do_read_partitions], the "limit" will be treated precisely.

        let accumulated_rows = AtomicUsize::new(0);

        // A !Copy Wrapper of u64
        struct NonCopy<T>(T);

        // convert u64 (which is Copy) into NonCopy( struct which is !Copy)
        // so that "async move" can be avoided in the latter async block
        // See https://github.com/rust-lang/rust/issues/81653
        let segment_locs = segment_locs
            .into_iter()
            .enumerate()
            .map(|(idx, (loc, ver))| (NonCopy(idx), (loc, NonCopy(ver))));

        let stream = futures::stream::iter(segment_locs)
            .map(|(idx, (seg_loc, u))| async {
                let version = { u }.0; // use block expression to force moving
                let idx = { idx }.0;
                if accumulated_rows.load(Ordering::Acquire) < limit {
                    let reader = MetaReaders::segment_info_reader(ctx);
                    let segment_info = reader.read(seg_loc, None, version).await?;
                    Ok::<_, ErrorCode>(
                        Self::filter_segment(
                            segment_info.as_ref(),
                            &block_pred,
                            &accumulated_rows,
                            limit,
                        )?
                        .into_iter()
                        .map(|v| (idx, v))
                        .collect::<Vec<_>>(),
                    )
                } else {
                    Ok(vec![])
                }
            })
            // configuration of the max size of buffered futures
            .buffered(std::cmp::min(10, segment_num))
            .try_collect::<Vec<_>>()
            .await?
            .into_iter()
            .flatten();

        Ok(stream.collect::<Vec<_>>())
    }

    #[inline]
    fn filter_segment(
        segment_info: &SegmentInfo,
        pred: &Pred,
        accumulated_rows: &AtomicUsize,
        limit: usize,
    ) -> Result<Vec<BlockMeta>> {
        if pred(&segment_info.summary.col_stats)? {
            let block_num = segment_info.blocks.len();
            let mut acc = Vec::with_capacity(block_num);
            for block_meta in &segment_info.blocks {
                if pred(&block_meta.col_stats)? {
                    let num_rows = block_meta.row_count as usize;
                    if accumulated_rows.fetch_add(num_rows, Ordering::Release) < limit {
                        acc.push(block_meta.clone());
                    }
                }
            }
            Ok(acc)
        } else {
            Ok(vec![])
        }
    }
}
