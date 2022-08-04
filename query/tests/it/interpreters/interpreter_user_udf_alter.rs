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
use common_exception::Result;
use databend_query::interpreters::*;
use databend_query::sessions::TableContext;
use databend_query::sql::*;
use futures::stream::StreamExt;
use pretty_assertions::assert_eq;

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_alter_udf_interpreter() -> Result<()> {
    let ctx = crate::tests::create_query_context().await?;
    let mut planner = Planner::new(ctx.clone());
    let tenant = ctx.get_tenant();

    {
        let query = "CREATE FUNCTION IF NOT EXISTS isnotempty AS (p) -> not(is_null(p)) DESC = 'This is a description'";
        let (plan, _, _) = planner.plan_sql(query).await?;
        let executor = InterpreterFactoryV2::get(ctx.clone(), &plan)?;
        assert_eq!(executor.name(), "CreateUserUDFInterpreter");
        let mut stream = executor.execute().await?;
        while let Some(_block) = stream.next().await {}
        let udf = ctx
            .get_user_manager()
            .get_udf(&tenant, "isnotempty")
            .await?;

        assert_eq!(udf.name, "isnotempty");
        assert_eq!(udf.parameters, vec!["p".to_string()]);
        assert_eq!(udf.definition, "NOT is_null(p)");
        assert_eq!(udf.description, "This is a description")
    }

    {
        let query = "ALTER FUNCTION isnotempty AS (d) -> not(is_not_null(d)) DESC = 'This is a new description'";
        let (plan, _, _) = planner.plan_sql(query).await?;
        let executor = InterpreterFactoryV2::get(ctx.clone(), &plan)?;
        assert_eq!(executor.name(), "AlterUserUDFInterpreter");

        let mut stream = executor.execute().await?;
        while let Some(_block) = stream.next().await {}

        let udf = ctx
            .get_user_manager()
            .get_udf(&tenant, "isnotempty")
            .await?;

        assert_eq!(udf.name, "isnotempty");
        assert_eq!(udf.parameters, vec!["d".to_string()]);
        assert_eq!(udf.definition, "NOT is_not_null(d)");
        assert_eq!(udf.description, "This is a new description")
    }

    Ok(())
}
