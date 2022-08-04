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

use std::collections::HashMap;
use std::sync::Arc;

use common_base::base::tokio;
use common_catalog::table_context::TableContext;
use common_datablocks::DataBlock;
use common_datavalues::prelude::*;
use common_fuse_meta::meta::ColumnStatistics;
use databend_query::storages::fuse::io::BlockWriter;
use databend_query::storages::fuse::io::TableMetaLocationGenerator;
use databend_query::storages::fuse::statistics::gen_columns_statistics;
use databend_query::storages::fuse::statistics::reducers;
use databend_query::storages::fuse::statistics::BlockStatistics;
use databend_query::storages::fuse::statistics::StatisticsAccumulator;
use opendal::Accessor;
use opendal::Operator;

use crate::storages::fuse::table_test_fixture::TestFixture;

#[test]
fn test_ft_stats_block_stats() -> common_exception::Result<()> {
    let schema = DataSchemaRefExt::create(vec![DataField::new("a", i32::to_data_type())]);
    let block = DataBlock::create(schema, vec![Series::from_data(vec![1, 2, 3])]);
    let r = gen_columns_statistics(&block)?;
    assert_eq!(1, r.len());
    let col_stats = r.get(&0).unwrap();
    assert_eq!(col_stats.min, DataValue::Int64(1));
    assert_eq!(col_stats.max, DataValue::Int64(3));
    Ok(())
}

#[test]
fn test_ft_tuple_stats_block_stats() -> common_exception::Result<()> {
    let inner_names = vec!["a".to_string(), "b".to_string()];
    let inner_data_types = vec![i32::to_data_type(), i32::to_data_type()];
    let tuple_data_type = StructType::new_impl(Some(inner_names), inner_data_types);
    let schema = DataSchemaRefExt::create(vec![DataField::new("t", tuple_data_type.clone())]);
    let inner_columns = vec![
        Series::from_data(vec![1, 2, 3]),
        Series::from_data(vec![4, 5, 6]),
    ];
    let column = StructColumn::from_data(inner_columns, tuple_data_type).arc();

    let block = DataBlock::create(schema, vec![column]);
    let r = gen_columns_statistics(&block)?;
    assert_eq!(2, r.len());
    let col0_stats = r.get(&0).unwrap();
    assert_eq!(col0_stats.min, DataValue::Int64(1));
    assert_eq!(col0_stats.max, DataValue::Int64(3));
    let col1_stats = r.get(&1).unwrap();
    assert_eq!(col1_stats.min, DataValue::Int64(4));
    assert_eq!(col1_stats.max, DataValue::Int64(6));
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
        .map(|b| gen_columns_statistics(&b.clone().unwrap()))
        .collect::<common_exception::Result<Vec<_>>>()?;
    let r = reducers::reduce_block_statistics(&col_stats);
    assert!(r.is_ok());
    let r = r.unwrap();
    assert_eq!(3, r.len());
    let col0_stats = r.get(&0).unwrap();
    assert_eq!(col0_stats.min, DataValue::Int64(val_start_with as i64));
    assert_eq!(col0_stats.max, DataValue::Int64(num_of_blocks as i64));
    let col1_stats = r.get(&1).unwrap();
    assert_eq!(
        col1_stats.min,
        DataValue::Int64((val_start_with * 2) as i64)
    );
    assert_eq!(col1_stats.max, DataValue::Int64((num_of_blocks * 2) as i64));
    let col2_stats = r.get(&2).unwrap();
    assert_eq!(
        col2_stats.min,
        DataValue::Int64((val_start_with * 3) as i64)
    );
    assert_eq!(col2_stats.max, DataValue::Int64((num_of_blocks * 3) as i64));

    Ok(())
}

#[test]
fn test_reduce_block_statistics_in_memory_size() -> common_exception::Result<()> {
    let iter = |mut idx| {
        std::iter::from_fn(move || {
            idx += 1;
            Some((idx, ColumnStatistics {
                min: DataValue::Null,
                max: DataValue::Null,
                null_count: 1,
                in_memory_size: 1,
            }))
        })
    };

    let num_of_cols = 100;
    // combine two statistics
    let col_stats_left = HashMap::from_iter(iter(0).take(num_of_cols));
    let col_stats_right = HashMap::from_iter(iter(0).take(num_of_cols));
    let r = reducers::reduce_block_statistics(&[col_stats_left, col_stats_right])?;
    assert_eq!(num_of_cols, r.len());
    // there should be 100 columns in the result
    for idx in 1..=100 {
        let col_stats = r.get(&idx);
        assert!(col_stats.is_some());
        let col_stats = col_stats.unwrap();
        // for each column, the reduced value of in_memory_size should be 1 + 1
        assert_eq!(col_stats.in_memory_size, 2);
        // for each column, the reduced value of null_count should be 1 + 1
        assert_eq!(col_stats.null_count, 2);
    }
    Ok(())
}

#[tokio::test]
async fn test_accumulator() -> common_exception::Result<()> {
    let blocks = TestFixture::gen_sample_blocks(10, 1);
    let fixture = TestFixture::new().await;
    let ctx = fixture.ctx();
    let mut stats_acc = StatisticsAccumulator::new();

    let mut builder = opendal::services::memory::Backend::build();
    let accessor: Arc<dyn Accessor> = builder.finish().await?;
    let operator = Operator::new(accessor);
    let table_ctx: Arc<dyn TableContext> = ctx;
    let loc_generator = TableMetaLocationGenerator::with_prefix("/".to_owned());

    for item in blocks {
        let block = item?;
        let block_statistics = BlockStatistics::from(&block, "does_not_matter".to_owned(), None)?;
        let block_writer = BlockWriter::new(&table_ctx, &operator, &loc_generator);
        let block_meta = block_writer.write(block).await?;
        stats_acc.add_with_block_meta(block_meta, block_statistics)?;
    }

    assert_eq!(10, stats_acc.blocks_statistics.len());
    assert!(stats_acc.index_size > 0);
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
