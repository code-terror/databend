// Copyright 2022 Datafuse Labs.
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

use std::sync::Arc;

use common_exception::Result;
use common_meta_types::GrantObject;
use common_meta_types::UserPrivilegeType;
use common_planners::DropTableClusterKeyPlan;
use common_streams::DataBlockStream;
use common_streams::SendableDataBlockStream;

use super::Interpreter;
use crate::sessions::QueryContext;

pub struct DropTableClusterKeyInterpreter {
    ctx: Arc<QueryContext>,
    plan: DropTableClusterKeyPlan,
}

impl DropTableClusterKeyInterpreter {
    pub fn try_create(ctx: Arc<QueryContext>, plan: DropTableClusterKeyPlan) -> Result<Self> {
        Ok(DropTableClusterKeyInterpreter { ctx, plan })
    }
}

#[async_trait::async_trait]
impl Interpreter for DropTableClusterKeyInterpreter {
    fn name(&self) -> &str {
        "DropTableClusterKeyInterpreter"
    }

    async fn execute(
        &self,
        _input_stream: Option<SendableDataBlockStream>,
    ) -> Result<SendableDataBlockStream> {
        let plan = &self.plan;
        self.ctx
            .get_current_session()
            .validate_privilege(
                &GrantObject::Table(
                    plan.catalog.clone(),
                    plan.database.clone(),
                    plan.table.clone(),
                ),
                UserPrivilegeType::Alter,
            )
            .await?;

        let tenant = self.ctx.get_tenant();
        let catalog = self.ctx.get_catalog(&plan.catalog)?;

        let table = catalog
            .get_table(tenant.as_str(), &plan.database, &plan.table)
            .await?;

        table
            .drop_table_cluster_keys(self.ctx.clone(), &self.plan.catalog)
            .await?;
        Ok(Box::pin(DataBlockStream::create(
            self.plan.schema(),
            None,
            vec![],
        )))
    }
}
