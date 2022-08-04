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

//! Test metasrv SchemaApi on a single node.

use std::sync::Arc;
use std::sync::Mutex;

use common_base::base::tokio;
use common_meta_api::SchemaApiTestSuite;
use common_meta_api::ShareApiTestSuite;

use crate::init_meta_ut;
use crate::tests::service::MetaSrvBuilder;

#[async_entry::test(worker_threads = 3, init = "init_meta_ut!()", tracing_span = "debug")]
async fn test_meta_grpc_client_single() -> anyhow::Result<()> {
    let builder = MetaSrvBuilder {
        test_contexts: Arc::new(Mutex::new(vec![])),
    };

    SchemaApiTestSuite::test_single_node(builder.clone()).await?;
    ShareApiTestSuite::test_single_node_share(builder).await?;

    Ok(())
}

#[async_entry::test(worker_threads = 5, init = "init_meta_ut!()", tracing_span = "debug")]
async fn test_meta_grpc_client_cluster() -> anyhow::Result<()> {
    let builder = MetaSrvBuilder {
        test_contexts: Arc::new(Mutex::new(vec![])),
    };

    SchemaApiTestSuite::test_cluster(builder).await?;

    Ok(())
}

// #[tokio::test(flavor = "multi_thread", worker_threads = 3)]
// async fn test_meta_grpc_client_share_create_get_drop() -> anyhow::Result<()> {
//     let (_log_guards, ut_span) = init_meta_ut!();
//     let _ent = ut_span.enter();
//
//     let (_tc, addr) = start_metasrv().await?;
//
//     let client = MetaGrpcClient::try_create(addr.as_str(), "root", "xxx", None, None).await?;
//
//     SchemaApiTestSuite {}.share_create_get_drop(&client).await
// }

// TODO(xp): uncomment following tests when the function is ready
// ------------------------------------------------------------

// #[tokio::test(flavor = "multi_thread", worker_threads = 3)]
// async fn test_meta_grpc_client_flight_get_database_meta_ddl_table() -> anyhow::Result<()> {
// let (_log_guards, ut_span) = init_meta_ut!();
// let _ent = ut_span.enter();
// let (_tc, addr) = crate::tests::start_metasrv().await?;
// let client = MetaGrpcClient::try_create(vec![addr], "root", "xxx", None, None).await?;
//
// let test_db = "db1";
// let plan = CreateDatabasePlan {
// if_not_exists: false,
// db: test_db.to_string(),
// engine: "Local".to_string(),
// options: Default::default(),
// };
// client.create_database(plan).await?;
//
// After `create db`, meta_ver will be increased to 1
//
// let schema = Arc::new(DataSchema::new(vec![DataField::new(
// "number",
// DataType::UInt64,
// false,
// )]));
//
// create-tbl operation will increases meta_version
// let plan = CreateTablePlan {
// if_not_exists: true,
// db: test_db.to_string(),
// table: "tbl1".to_string(),
// schema: schema.clone(),
// options: Default::default(),
// engine: "JSON".to_string(),
// };
//
// client.create_table(plan.clone()).await?;
//
// let res = client.get_database_meta(None).await?;
// assert!(res.is_some());
// let snapshot = res.unwrap();
// assert_eq!(2, snapshot.meta_ver);
// assert_eq!(1, snapshot.db_metas.len());
// assert_eq!(1, snapshot.tbl_metas.len());
//
// if lower_bound < current meta version, returns database meta
// let res = client.get_database_meta(Some(0)).await?;
// assert!(res.is_some());
// let snapshot = res.unwrap();
// assert_eq!(2, snapshot.meta_ver);
// assert_eq!(1, snapshot.db_metas.len());
//
// if lower_bound equals current meta version, returns None
// let res = client.get_database_meta(Some(2)).await?;
// assert!(res.is_none());
//
// failed ddl do not effect meta version
//  recall: plan.if_not_exist == true
// let _r = client.create_table(plan).await?;
// let res = client.get_database_meta(Some(2)).await?;
// assert!(res.is_none());
//
// drop-table will increase meta version
// let plan = DropTablePlan {
// if_exists: true,
// db: test_db.to_string(),
// table: "tbl1".to_string(),
// };
//
// client.drop_table(plan).await?;
// let res = client.get_database_meta(Some(2)).await?;
// assert!(res.is_some());
// let snapshot = res.unwrap();
// assert_eq!(3, snapshot.meta_ver);
// assert_eq!(1, snapshot.db_metas.len());
// assert_eq!(0, snapshot.tbl_metas.len());
//
// Ok(())
// }
//
// #[tokio::test(flavor = "multi_thread", worker_threads = 3)]
// async fn test_meta_grpc_client_flight_get_database_meta_empty_db() -> anyhow::Result<()> {
// let (_log_guards, ut_span) = init_meta_ut!();
// let _ent = ut_span.enter();
// let (_tc, addr) = crate::tests::start_metasrv().await?;
// let client = MetaGrpcClient::try_create(vec![addr], "root", "xxx", None, None).await?;
//
// Empty Database
// let res = client.get_database_meta(None).await?;
// assert!(res.is_none());
//
// Ok(())
// }
//
// #[tokio::test(flavor = "multi_thread", worker_threads = 3)]
// async fn test_meta_grpc_client_flight_get_database_meta_ddl_db() -> anyhow::Result<()> {
// let (_log_guards, ut_span) = init_meta_ut!();
// let _ent = ut_span.enter();
// let (_tc, addr) = crate::tests::start_metasrv().await?;
// let client = MetaGrpcClient::try_create(vec![addr], "root", "xxx", None, None).await?;
//
// create-db operation will increases meta_version
// let plan = CreateDatabasePlan {
// if_not_exists: false,
// db: "db1".to_string(),
// engine: "Local".to_string(),
// options: Default::default(),
// };
// client.create_database(plan).await?;
//
// let res = client.get_database_meta(None).await?;
// assert!(res.is_some());
// let snapshot = res.unwrap();
// assert_eq!(1, snapshot.meta_ver);
// assert_eq!(1, snapshot.db_metas.len());
//
// if lower_bound < current meta version, returns database meta
// let res = client.get_database_meta(Some(0)).await?;
// assert!(res.is_some());
// let snapshot = res.unwrap();
// assert_eq!(1, snapshot.meta_ver);
// assert_eq!(1, snapshot.db_metas.len());
//
// if lower_bound equals current meta version, returns None
// let res = client.get_database_meta(Some(1)).await?;
// assert!(res.is_none());
//
// failed ddl do not effect meta version
// let plan = CreateDatabasePlan {
// if_not_exists: true, // <<--
// db: "db1".to_string(),
// engine: "Local".to_string(),
// options: Default::default(),
// };
//
// client.create_database(plan).await?;
// let res = client.get_database_meta(Some(1)).await?;
// assert!(res.is_none());
//
// drop-db will increase meta version
// let plan = DropDatabasePlan {
// if_exists: true,
// db: "db1".to_string(),
// };
//
// client.drop_database(plan).await?;
// let res = client.get_database_meta(Some(1)).await?;
// assert!(res.is_some());
// let snapshot = res.unwrap();
//
// assert_eq!(2, snapshot.meta_ver);
// assert_eq!(0, snapshot.db_metas.len());
//
// Ok(())
// }
