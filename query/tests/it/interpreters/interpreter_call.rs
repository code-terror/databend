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

use common_base::base::tokio;
use common_exception::ErrorCode;
use common_exception::Result;
use common_meta_types::GrantObject;
use common_meta_types::UserGrantSet;
use common_meta_types::UserOptionFlag;
use databend_query::interpreters::*;
use databend_query::sessions::TableContext;
use databend_query::sql::Planner;
use futures::TryStreamExt;
use pretty_assertions::assert_eq;

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_call_interpreter() -> Result<()> {
    let ctx = crate::tests::create_query_context().await?;
    let mut planner = Planner::new(ctx.clone());

    let query = "call system$test()";
    let (plan, _, _) = planner.plan_sql(query).await?;
    let executor = InterpreterFactoryV2::get(ctx.clone(), &plan)?;
    assert_eq!(executor.name(), "CallInterpreter");
    let res = executor.execute().await;
    assert_eq!(res.is_err(), true);
    assert_eq!(
        res.err().unwrap().code(),
        ErrorCode::UnknownFunction("").code()
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_call_fuse_snapshot_interpreter() -> Result<()> {
    let ctx = crate::tests::create_query_context().await?;
    let mut planner = Planner::new(ctx.clone());

    // NumberArgumentsNotMatch
    {
        let query = "call system$fuse_snapshot()";
        let (plan, _, _) = planner.plan_sql(query).await?;
        let executor = InterpreterFactoryV2::get(ctx.clone(), &plan)?;
        assert_eq!(executor.name(), "CallInterpreter");
        let res = executor.execute().await;
        assert_eq!(res.is_err(), true);
        let expect = "Code: 1028, displayText = Function `FUSE_SNAPSHOT` expect to have [2, 3] arguments, but got 0.";
        assert_eq!(expect, res.err().unwrap().to_string());
    }

    // UnknownTable
    {
        let query = "call system$fuse_snapshot(default, test)";
        let (plan, _, _) = planner.plan_sql(query).await?;
        let executor = InterpreterFactoryV2::get(ctx.clone(), &plan)?;
        assert_eq!(executor.name(), "CallInterpreter");
        let res = executor.execute().await;
        assert_eq!(res.is_err(), true);
        assert_eq!(
            res.err().unwrap().code(),
            ErrorCode::UnknownTable("").code()
        );
    }

    // BadArguments
    {
        let query = "call system$fuse_snapshot(system, tables)";
        let (plan, _, _) = planner.plan_sql(query).await?;
        let executor = InterpreterFactoryV2::get(ctx.clone(), &plan)?;
        assert_eq!(executor.name(), "CallInterpreter");
        let res = executor.execute().await;
        assert_eq!(res.is_err(), true);
        let expect =
            "Code: 1015, displayText = expects table of engine FUSE, but got SystemTables.";
        assert_eq!(expect, res.err().unwrap().to_string());
    }

    // Create table
    {
        let query = "\
            CREATE TABLE default.a(a bigint)\
        ";

        let (plan, _, _) = planner.plan_sql(query).await?;
        let executor = InterpreterFactoryV2::get(ctx.clone(), &plan)?;
        let _ = executor.execute().await?;
    }

    // FuseHistory
    {
        let query = "call system$fuse_snapshot(default, a)";
        let (plan, _, _) = planner.plan_sql(query).await?;
        let executor = InterpreterFactoryV2::get(ctx.clone(), &plan)?;
        let _ = executor.execute().await?;
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_call_clustering_information_interpreter() -> Result<()> {
    let ctx = crate::tests::create_query_context().await?;
    let mut planner = Planner::new(ctx.clone());

    // NumberArgumentsNotMatch
    {
        let query = "call system$clustering_information()";
        let (plan, _, _) = planner.plan_sql(query).await?;
        let executor = InterpreterFactoryV2::get(ctx.clone(), &plan)?;
        assert_eq!(executor.name(), "CallInterpreter");
        let res = executor.execute().await;
        assert_eq!(res.is_err(), true);
        let expect = "Code: 1028, displayText = Function `CLUSTERING_INFORMATION` expect to have 2 arguments, but got 0.";
        assert_eq!(expect, res.err().unwrap().to_string());
    }

    // UnknownTable
    {
        let query = "call system$clustering_information(default, test)";
        let (plan, _, _) = planner.plan_sql(query).await?;
        let executor = InterpreterFactoryV2::get(ctx.clone(), &plan)?;
        assert_eq!(executor.name(), "CallInterpreter");
        let res = executor.execute().await;
        assert_eq!(res.is_err(), true);
        assert_eq!(
            res.err().unwrap().code(),
            ErrorCode::UnknownTable("").code()
        );
    }

    // BadArguments
    {
        let query = "call system$clustering_information(system, tables)";
        let (plan, _, _) = planner.plan_sql(query).await?;
        let executor = InterpreterFactoryV2::get(ctx.clone(), &plan)?;
        assert_eq!(executor.name(), "CallInterpreter");
        let res = executor.execute().await;
        assert_eq!(res.is_err(), true);
        let expect =
            "Code: 1015, displayText = expects table of engine FUSE, but got SystemTables.";
        assert_eq!(expect, res.err().unwrap().to_string());
    }

    // Create table a
    {
        let query = "\
            CREATE TABLE default.a(a bigint)\
        ";

        let (plan, _, _) = planner.plan_sql(query).await?;
        let executor = InterpreterFactoryV2::get(ctx.clone(), &plan)?;
        let _ = executor.execute().await?;
    }

    // Unclustered.
    {
        let query = "call system$clustering_information(default, a)";
        let (plan, _, _) = planner.plan_sql(query).await?;
        let executor = InterpreterFactoryV2::get(ctx.clone(), &plan)?;
        let res = executor.execute().await;
        assert_eq!(res.is_err(), true);
        let expect =
            "Code: 1081, displayText = Invalid clustering keys or table a is not clustered.";
        assert_eq!(expect, res.err().unwrap().to_string());
    }

    // Create table b
    {
        let query = "\
        CREATE TABLE default.b(a bigint) cluster by(a)\
    ";

        let (plan, _, _) = planner.plan_sql(query).await?;
        let executor = InterpreterFactoryV2::get(ctx.clone(), &plan)?;
        let _ = executor.execute().await?;
    }

    // FuseHistory
    {
        let query = "call system$clustering_information(default, b)";
        let (plan, _, _) = planner.plan_sql(query).await?;
        let executor = InterpreterFactoryV2::get(ctx.clone(), &plan)?;
        let _ = executor.execute().await?;
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_call_bootstrap_tenant_interpreter() -> Result<()> {
    let ctx = crate::tests::create_query_context().await?;
    let mut planner = Planner::new(ctx.clone());

    // NumberArgumentsNotMatch
    {
        let query = "call admin$bootstrap_tenant()";
        let (plan, _, _) = planner.plan_sql(query).await?;
        let executor = InterpreterFactoryV2::get(ctx.clone(), &plan)?;
        let res = executor.execute().await;
        assert_eq!(res.is_err(), true);
        let expect = "Code: 1028, displayText = Function `BOOTSTRAP_TENANT` expect to have 1 arguments, but got 0.";
        assert_eq!(expect, res.err().unwrap().to_string());
    }

    // Access denied
    {
        let query = "call admin$bootstrap_tenant(tenant1)";
        let (plan, _, _) = planner.plan_sql(query).await?;
        let executor = InterpreterFactoryV2::get(ctx.clone(), &plan)?;
        let res = executor.execute().await;
        assert_eq!(res.is_err(), true);
        let expect = "Code: 1062, displayText = Access denied: 'BOOTSTRAP_TENANT' only used in management-mode.";
        assert_eq!(expect, res.err().unwrap().to_string());
    }

    let conf = crate::tests::ConfigBuilder::create()
        .with_management_mode()
        .config();
    let ctx = crate::tests::create_query_context_with_config(conf.clone(), None).await?;

    // Management Mode, without user option
    {
        let query = "call admin$bootstrap_tenant(tenant1)";
        let (plan, _, _) = planner.plan_sql(query).await?;
        let executor = InterpreterFactoryV2::get(ctx.clone(), &plan)?;
        let res = executor.execute().await;
        assert_eq!(res.is_err(), true);
        let expect = "Code: 1063, displayText = Access denied: 'BOOTSTRAP_TENANT' requires user TENANTSETTING option flag.";
        assert_eq!(expect, res.err().unwrap().to_string());
    }

    let mut user_info = ctx.get_current_user()?;
    user_info
        .option
        .set_option_flag(UserOptionFlag::TenantSetting);
    let ctx = crate::tests::create_query_context_with_config(conf.clone(), Some(user_info)).await?;

    // Management Mode, with user option
    {
        let query = "call admin$bootstrap_tenant(tenant1)";
        let (plan, _, _) = planner.plan_sql(query).await?;
        let executor = InterpreterFactoryV2::get(ctx.clone(), &plan)?;
        executor.execute().await?;

        let user_mgr = ctx.get_user_manager();
        // should create account admin role
        let role = "account_admin".to_string();
        let role_info = user_mgr.get_role("tenant1", role.clone()).await?;
        let mut grants = UserGrantSet::empty();
        grants.grant_privileges(
            &GrantObject::Global,
            GrantObject::Global.available_privileges(),
        );
        assert_eq!(role_info.grants, grants);
    }

    // Idempotence on call bootstrap tenant
    {
        let query = "call admin$bootstrap_tenant(tenant1)";
        let (plan, _, _) = planner.plan_sql(query).await?;
        let executor = InterpreterFactoryV2::get(ctx.clone(), &plan)?;
        executor.execute().await?;
        let user_mgr = ctx.get_user_manager();
        // should create account admin role
        let role = "account_admin".to_string();
        let role_info = user_mgr.get_role("tenant1", role.clone()).await?;
        let mut grants = UserGrantSet::empty();
        grants.grant_privileges(
            &GrantObject::Global,
            GrantObject::Global.available_privileges(),
        );
        assert_eq!(role_info.grants, grants);
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_call_tenant_quota_interpreter() -> Result<()> {
    let ctx = crate::tests::create_query_context().await?;
    let mut planner = Planner::new(ctx.clone());

    // Access denied
    {
        let query = "call admin$tenant_quota(tenant1)";
        let (plan, _, _) = planner.plan_sql(query).await?;
        let executor = InterpreterFactoryV2::get(ctx.clone(), &plan)?;
        let res = executor.execute().await;
        assert_eq!(res.is_err(), true);
        let expect =
            "Code: 1062, displayText = Access denied: 'TENANT_QUOTA' only used in management-mode.";
        assert_eq!(expect, res.err().unwrap().to_string());
    }

    let conf = crate::tests::ConfigBuilder::create()
        .with_management_mode()
        .config();
    let mut user_info = ctx.get_current_user()?;
    user_info
        .option
        .set_option_flag(UserOptionFlag::TenantSetting);
    let ctx = crate::tests::create_query_context_with_config(conf.clone(), Some(user_info)).await?;

    // current tenant
    {
        let query = "call admin$tenant_quota()";
        let (plan, _, _) = planner.plan_sql(query).await?;
        let executor = InterpreterFactoryV2::get(ctx.clone(), &plan)?;
        let stream = executor.execute().await?;
        let result = stream.try_collect::<Vec<_>>().await?;
        let expected = vec![
            "+---------------+-------------------------+------------+---------------------+",
            "| max_databases | max_tables_per_database | max_stages | max_files_per_stage |",
            "+---------------+-------------------------+------------+---------------------+",
            "| 0             | 0                       | 0          | 0                   |",
            "+---------------+-------------------------+------------+---------------------+",
        ];
        common_datablocks::assert_blocks_sorted_eq(expected, result.as_slice());
    }

    // query other tenant
    {
        let query = "call admin$tenant_quota(tenant1)";
        let (plan, _, _) = planner.plan_sql(query).await?;
        let executor = InterpreterFactoryV2::get(ctx.clone(), &plan)?;
        let stream = executor.execute().await?;
        let result = stream.try_collect::<Vec<_>>().await?;
        let expected = vec![
            "+---------------+-------------------------+------------+---------------------+",
            "| max_databases | max_tables_per_database | max_stages | max_files_per_stage |",
            "+---------------+-------------------------+------------+---------------------+",
            "| 0             | 0                       | 0          | 0                   |",
            "+---------------+-------------------------+------------+---------------------+",
        ];
        common_datablocks::assert_blocks_sorted_eq(expected, result.as_slice());
    }

    // set other tenant quota
    {
        let query = "call admin$tenant_quota(tenant1, 7, 5, 3, 3)";
        let (plan, _, _) = planner.plan_sql(query).await?;
        let executor = InterpreterFactoryV2::get(ctx.clone(), &plan)?;
        let stream = executor.execute().await?;
        let result = stream.try_collect::<Vec<_>>().await?;
        let expected = vec![
            "+---------------+-------------------------+------------+---------------------+",
            "| max_databases | max_tables_per_database | max_stages | max_files_per_stage |",
            "+---------------+-------------------------+------------+---------------------+",
            "| 7             | 5                       | 3          | 3                   |",
            "+---------------+-------------------------+------------+---------------------+",
        ];
        common_datablocks::assert_blocks_sorted_eq(expected, result.as_slice());
    }

    {
        let query = "call admin$tenant_quota(tenant1, 8)";
        let (plan, _, _) = planner.plan_sql(query).await?;
        let executor = InterpreterFactoryV2::get(ctx.clone(), &plan)?;
        let stream = executor.execute().await?;
        let result = stream.try_collect::<Vec<_>>().await?;
        let expected = vec![
            "+---------------+-------------------------+------------+---------------------+",
            "| max_databases | max_tables_per_database | max_stages | max_files_per_stage |",
            "+---------------+-------------------------+------------+---------------------+",
            "| 8             | 5                       | 3          | 3                   |",
            "+---------------+-------------------------+------------+---------------------+",
        ];
        common_datablocks::assert_blocks_sorted_eq(expected, result.as_slice());
    }

    {
        let query = "call admin$tenant_quota(tenant1)";
        let (plan, _, _) = planner.plan_sql(query).await?;
        let executor = InterpreterFactoryV2::get(ctx.clone(), &plan)?;
        let stream = executor.execute().await?;
        let result = stream.try_collect::<Vec<_>>().await?;
        let expected = vec![
            "+---------------+-------------------------+------------+---------------------+",
            "| max_databases | max_tables_per_database | max_stages | max_files_per_stage |",
            "+---------------+-------------------------+------------+---------------------+",
            "| 8             | 5                       | 3          | 3                   |",
            "+---------------+-------------------------+------------+---------------------+",
        ];
        common_datablocks::assert_blocks_sorted_eq(expected, result.as_slice());
    }

    // current tenant again
    {
        let query = "call admin$tenant_quota()";
        let (plan, _, _) = planner.plan_sql(query).await?;
        let executor = InterpreterFactoryV2::get(ctx.clone(), &plan)?;
        let stream = executor.execute().await?;
        let result = stream.try_collect::<Vec<_>>().await?;
        let expected = vec![
            "+---------------+-------------------------+------------+---------------------+",
            "| max_databases | max_tables_per_database | max_stages | max_files_per_stage |",
            "+---------------+-------------------------+------------+---------------------+",
            "| 0             | 0                       | 0          | 0                   |",
            "+---------------+-------------------------+------------+---------------------+",
        ];
        common_datablocks::assert_blocks_sorted_eq(expected, result.as_slice());
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_call_stats_tenant_tables_interpreter() -> Result<()> {
    let ctx = crate::tests::create_query_context().await?;
    let mut planner = Planner::new(ctx.clone());

    // NumberArgumentsNotMatch
    {
        let query = "call stats$tenant_tables()";
        let (plan, _, _) = planner.plan_sql(query).await?;
        let executor = InterpreterFactoryV2::get(ctx.clone(), &plan)?;
        assert_eq!(executor.name(), "CallInterpreter");
        let res = executor.execute().await;
        assert_eq!(res.is_err(), true);
        let expect = "Code: 1028, displayText = Function `TENANT_TABLES` expect to have [1, 18446744073709551614] arguments, but got 0.";
        assert_eq!(expect, res.err().unwrap().to_string());
    }

    // Access denied
    {
        let query = "call stats$tenant_tables(tenant1)";
        let (plan, _, _) = planner.plan_sql(query).await?;
        let executor = InterpreterFactoryV2::get(ctx.clone(), &plan)?;
        let res = executor.execute().await;
        assert_eq!(res.is_err(), true);
        let expect = "Code: 1062, displayText = Access denied: 'TENANT_TABLES' only used in management-mode.";
        assert_eq!(expect, res.err().unwrap().to_string());
    }

    let conf = crate::tests::ConfigBuilder::create()
        .with_management_mode()
        .config();
    let mut user_info = ctx.get_current_user()?;
    user_info
        .option
        .set_option_flag(UserOptionFlag::TenantSetting);
    let ctx = crate::tests::create_query_context_with_config(conf.clone(), Some(user_info)).await?;

    {
        let query = "call stats$tenant_tables(tenant1, tenant2, tenant3)";
        let (plan, _, _) = planner.plan_sql(query).await?;
        let executor = InterpreterFactoryV2::get(ctx.clone(), &plan)?;
        let stream = executor.execute().await?;
        let result = stream.try_collect::<Vec<_>>().await?;
        let expected = vec![
            "+-----------+-------------+",
            "| tenant_id | table_count |",
            "+-----------+-------------+",
            "| tenant1   | 0           |",
            "| tenant2   | 0           |",
            "| tenant3   | 0           |",
            "+-----------+-------------+",
        ];
        common_datablocks::assert_blocks_sorted_eq(expected, result.as_slice());
    }

    Ok(())
}
