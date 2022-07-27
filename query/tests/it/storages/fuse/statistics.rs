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

use common_datablocks::DataBlock;
use common_datavalues::prelude::*;
use databend_query::storages::fuse::statistics::accumulator;
use databend_query::storages::fuse::statistics::reducers;

use crate::storages::fuse::statistics::accumulator::BlockStatistics;
use crate::storages::fuse::table_test_fixture::TestFixture;

#[test]
fn test_ft_stats_block_stats() -> common_exception::Result<()> {
    let schema = DataSchemaRefExt::create(vec![DataField::new("a", i32::to_data_type())]);
    let block = DataBlock::create(schema, vec![Series::from_data(vec![1, 2, 3])]);
    let r = accumulator::columns_statistics(&block)?;
    assert_eq!(1, r.len());
    let col_stats = r.get(&0).unwrap();
    assert_eq!(col_stats.min, DataValue::Int64(1));
    assert_eq!(col_stats.max, DataValue::Int64(3));
    Ok(())
}

#[test]
fn test_ft_stats_col_stats_reduce() -> common_exception::Result<()> {
    let num_of_blocks = 10;
    let rows_per_block = 3;
    let val_start_with = 1;

    let blocks = TestFixture::gen_sample_blocks_ex(num_of_blocks, rows_per_block, val_start_with);
    let col_stats = blocks
        .iter()
        .map(|b| accumulator::columns_statistics(&b.clone().unwrap()))
        .collect::<common_exception::Result<Vec<_>>>()?;
    let r = reducers::reduce_block_statistics(&col_stats);
    assert!(r.is_ok());
    let r = r.unwrap();
    assert_eq!(1, r.len());
    let col_stats = r.get(&0).unwrap();
    assert_eq!(col_stats.min, DataValue::Int64(val_start_with as i64));
    assert_eq!(col_stats.max, DataValue::Int64(num_of_blocks as i64));
    Ok(())
}

#[test]
fn test_ft_stats_accumulator() -> common_exception::Result<()> {
    let blocks = TestFixture::gen_sample_blocks(10, 1);
    let mut stats_acc = accumulator::StatisticsAccumulator::new();
    let test_file_size = 1;
    for item in blocks {
        let block_acc = stats_acc.begin(&item?, None)?;
        stats_acc = block_acc.end(test_file_size, "".to_owned(), HashMap::new());
    }
    assert_eq!(10, stats_acc.blocks_statistics.len());
    // TODO more cases here pls
    Ok(())
}

#[test]
fn test_ft_stats_cluster_stats() -> common_exception::Result<()> {
    let schema = DataSchemaRefExt::create(vec![
        DataField::new("a", i32::to_data_type()),
        DataField::new("b", Vu8::to_data_type()),
    ]);
    let blocks = DataBlock::create(schema, vec![
        Series::from_data(vec![1i32, 2, 3]),
        Series::from_data(vec!["123456", "234567", "345678"]),
    ]);
    let stats = BlockStatistics::clusters_statistics(0, &[0], &blocks)?;
    assert!(stats.is_some());
    let stats = stats.unwrap();
    assert_eq!(vec![DataValue::Int64(1)], stats.min);
    assert_eq!(vec![DataValue::Int64(3)], stats.max);

    let stats = BlockStatistics::clusters_statistics(1, &[1], &blocks)?;
    assert!(stats.is_some());
    let stats = stats.unwrap();
    assert_eq!(vec![DataValue::String(b"12345".to_vec())], stats.min);
    assert_eq!(vec![DataValue::String(b"34567".to_vec())], stats.max);

    Ok(())
}
