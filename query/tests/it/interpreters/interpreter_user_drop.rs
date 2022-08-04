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
use common_meta_types::AuthInfo;
use common_meta_types::PasswordHashMethod;
use common_meta_types::UserInfo;
use databend_query::interpreters::*;
use databend_query::sessions::TableContext;
use databend_query::sql::*;
use pretty_assertions::assert_eq;

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_drop_user_interpreter() -> Result<()> {
    let ctx = crate::tests::create_query_context().await?;
    let mut planner = Planner::new(ctx.clone());

    let tenant = ctx.get_tenant();

    {
        let query = "DROP USER 'test'@'localhost'";
        let (plan, _, _) = planner.plan_sql(query).await?;
        let executor = InterpreterFactoryV2::get(ctx.clone(), &plan)?;
        assert_eq!(executor.name(), "DropUserInterpreter");
        let ret = executor.execute().await;
        assert!(ret.is_err())
    }

    {
        let query = "DROP USER IF EXISTS 'test'@'localhost'";
        let (plan, _, _) = planner.plan_sql(query).await?;
        let executor = InterpreterFactoryV2::get(ctx.clone(), &plan)?;
        assert_eq!(executor.name(), "DropUserInterpreter");
        let ret = executor.execute().await;
        assert!(ret.is_ok())
    }

    {
        let name = "test";
        let hostname = "localhost";
        let password = "test";
        let auth_info = AuthInfo::Password {
            hash_value: Vec::from(password),
            hash_method: PasswordHashMethod::Sha256,
        };

        let user_info = UserInfo::new(name, hostname, auth_info);
        let user_mgr = ctx.get_user_manager();
        user_mgr.add_user(&tenant, user_info.clone(), false).await?;

        let old_user = user_mgr.get_user(&tenant, user_info.identity()).await?;
        assert_eq!(
            old_user.auth_info.get_password().unwrap(),
            Vec::from(password)
        );

        let query = "DROP USER 'test'@'localhost'";
        let (plan, _, _) = planner.plan_sql(query).await?;
        let executor = InterpreterFactoryV2::get(ctx.clone(), &plan)?;
        assert_eq!(executor.name(), "DropUserInterpreter");
        executor.execute().await?;
    }

    Ok(())
}
