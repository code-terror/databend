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

use std::collections::HashMap;

use common_arrow::parquet::metadata::ThriftFileMetaData;
use common_datablocks::DataBlock;
use common_datavalues::prelude::*;
use common_exception::Result;
use common_functions::aggregates::eval_aggr;

use crate::storages::fuse::meta::BlockMeta;
use crate::storages::fuse::meta::ColumnId;
use crate::storages::fuse::meta::ColumnMeta;
use crate::storages::fuse::meta::Versioned;
use crate::storages::fuse::operations::column_metas;
use crate::storages::index::ClusterStatistics;
use crate::storages::index::ColumnStatistics;
use crate::storages::index::StatisticsOfColumns;

#[derive(Default)]
pub struct StatisticsAccumulator {
    pub blocks_metas: Vec<BlockMeta>,
    pub blocks_statistics: Vec<StatisticsOfColumns>,
    pub summary_row_count: u64,
    pub summary_block_count: u64,
    pub in_memory_size: u64,
    pub file_size: u64,
}

impl StatisticsAccumulator {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn begin(
        mut self,
        block: &DataBlock,
        cluster_stats: Option<ClusterStatistics>,
    ) -> Result<PartiallyAccumulated> {
        let row_count = block.num_rows() as u64;
        let block_in_memory_size = block.memory_size() as u64;

        self.summary_block_count += 1;
        self.summary_row_count += row_count;
        self.in_memory_size += block_in_memory_size;
        let block_stats = columns_statistics(block)?;
        self.blocks_statistics.push(block_stats.clone());
        Ok(PartiallyAccumulated {
            accumulator: self,
            block_row_count: block.num_rows() as u64,
            block_size: block.memory_size() as u64,
            block_columns_statistics: block_stats,
            block_cluster_statistics: cluster_stats,
        })
    }

    pub fn add_block(
        &mut self,
        file_size: u64,
        meta: ThriftFileMetaData,
        statistics: BlockStatistics,
    ) -> Result<()> {
        self.file_size += file_size;
        self.summary_block_count += 1;
        self.in_memory_size += statistics.block_bytes_size;
        self.summary_row_count += statistics.block_rows_size;
        self.blocks_statistics
            .push(statistics.block_column_statistics.clone());

        let row_count = statistics.block_rows_size;
        let block_size = statistics.block_bytes_size;
        let col_stats = statistics.block_column_statistics.clone();
        let location = (statistics.block_file_location, DataBlock::VERSION);
        let col_metas = column_metas(&meta)?;
        let cluster_stats = statistics.block_cluster_statistics;

        self.blocks_metas.push(BlockMeta::new(
            row_count,
            block_size,
            file_size,
            col_stats,
            col_metas,
            cluster_stats,
            location,
        ));

        Ok(())
    }

    pub fn acc_columns(data_block: &DataBlock) -> common_exception::Result<StatisticsOfColumns> {
        let mut statistics = StatisticsOfColumns::new();

        let rows = data_block.num_rows();
        for idx in 0..data_block.num_columns() {
            let col = data_block.column(idx);
            let field = data_block.schema().field(idx);
            let column_field = ColumnWithField::new(col.clone(), field.clone());

            let mut min = DataValue::Null;
            let mut max = DataValue::Null;

            let mins = eval_aggr("min", vec![], &[column_field.clone()], rows)?;
            let maxs = eval_aggr("max", vec![], &[column_field], rows)?;

            if mins.len() > 0 {
                min = mins.get(0);
            }
            if maxs.len() > 0 {
                max = maxs.get(0);
            }
            let (is_all_null, bitmap) = col.validity();
            let unset_bits = match (is_all_null, bitmap) {
                (true, _) => rows,
                (false, Some(bitmap)) => bitmap.unset_bits(),
                (false, None) => 0,
            };

            let in_memory_size = col.memory_size() as u64;
            let col_stats = ColumnStatistics {
                min,
                max,
                null_count: unset_bits as u64,
                in_memory_size,
            };

            statistics.insert(idx as u32, col_stats);
        }
        Ok(statistics)
    }

    pub fn summary(&self) -> Result<StatisticsOfColumns> {
        super::reduce_block_statistics(&self.blocks_statistics)
    }
}

pub struct PartiallyAccumulated {
    accumulator: StatisticsAccumulator,
    block_row_count: u64,
    block_size: u64,
    block_columns_statistics: HashMap<ColumnId, ColumnStatistics>,
    block_cluster_statistics: Option<ClusterStatistics>,
}

impl PartiallyAccumulated {
    pub fn end(
        mut self,
        file_size: u64,
        location: String,
        col_metas: HashMap<ColumnId, ColumnMeta>,
    ) -> StatisticsAccumulator {
        let mut stats = &mut self.accumulator;
        stats.file_size += file_size;

        let row_count = self.block_row_count;
        let block_size = self.block_size;
        let col_stats = self.block_columns_statistics;
        let cluster_stats = self.block_cluster_statistics;
        let location = (location, DataBlock::VERSION);

        let block_meta = BlockMeta::new(
            row_count,
            block_size,
            file_size,
            col_stats,
            col_metas,
            cluster_stats,
            location,
        );
        stats.blocks_metas.push(block_meta);
        self.accumulator
    }
}

pub struct BlockStatistics {
    pub block_rows_size: u64,
    pub block_bytes_size: u64,
    pub block_file_location: String,
    pub block_column_statistics: HashMap<ColumnId, ColumnStatistics>,
    pub block_cluster_statistics: Option<ClusterStatistics>,
}

impl BlockStatistics {
    pub fn from(
        data_block: &DataBlock,
        location: String,
        cluster_stats: Option<ClusterStatistics>,
    ) -> Result<BlockStatistics> {
        Ok(BlockStatistics {
            block_file_location: location,
            block_rows_size: data_block.num_rows() as u64,
            block_bytes_size: data_block.memory_size() as u64,
            block_column_statistics: columns_statistics(data_block)?,
            block_cluster_statistics: cluster_stats,
        })
    }

    pub fn clusters_statistics(
        cluster_key_id: u32,
        cluster_key_index: &[usize],
        block: &DataBlock,
    ) -> Result<Option<ClusterStatistics>> {
        if cluster_key_index.is_empty() {
            return Ok(None);
        }

        let mut min = Vec::with_capacity(cluster_key_index.len());
        let mut max = Vec::with_capacity(cluster_key_index.len());

        for key in cluster_key_index.iter() {
            let col = block.column(*key);

            let mut left = col.get_checked(0)?;
            // To avoid high cardinality, for the string column,
            // cluster statistics uses only the first 5 bytes.
            if let DataValue::String(v) = &left {
                let l = v.len() as usize;
                let e = if l < 5 { l } else { 5 };
                left = DataValue::from(&v[0..e]);
            }
            min.push(left);

            let mut right = col.get_checked(col.len() - 1)?;
            if let DataValue::String(v) = &right {
                let l = v.len() as usize;
                let e = if l < 5 { l } else { 5 };
                right = DataValue::from(&v[0..e]);
            }
            max.push(right);
        }

        Ok(Some(ClusterStatistics {
            cluster_key_id,
            min,
            max,
        }))
    }
}

pub fn columns_statistics(data_block: &DataBlock) -> Result<StatisticsOfColumns> {
    let mut statistics = StatisticsOfColumns::new();

    let rows = data_block.num_rows();
    for idx in 0..data_block.num_columns() {
        let col = data_block.column(idx);
        let field = data_block.schema().field(idx);
        let column_field = ColumnWithField::new(col.clone(), field.clone());

        let mut min = DataValue::Null;
        let mut max = DataValue::Null;

        let mins = eval_aggr("min", vec![], &[column_field.clone()], rows)?;
        let maxs = eval_aggr("max", vec![], &[column_field], rows)?;

        if mins.len() > 0 {
            min = mins.get(0);
        }
        if maxs.len() > 0 {
            max = maxs.get(0);
        }
        let (is_all_null, bitmap) = col.validity();
        let unset_bits = match (is_all_null, bitmap) {
            (true, _) => rows,
            (false, Some(bitmap)) => bitmap.unset_bits(),
            (false, None) => 0,
        };

        let in_memory_size = col.memory_size() as u64;
        let col_stats = ColumnStatistics {
            min,
            max,
            null_count: unset_bits as u64,
            in_memory_size,
        };

        statistics.insert(idx as u32, col_stats);
    }
    Ok(statistics)
}
