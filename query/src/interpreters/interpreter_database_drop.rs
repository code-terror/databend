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

use std::sync::Arc;

use common_exception::Result;
use common_meta_types::GrantObject;
use common_meta_types::UserPrivilegeType;
use common_planners::DropDatabasePlan;
use common_streams::DataBlockStream;
use common_streams::SendableDataBlockStream;

use crate::interpreters::Interpreter;
use crate::sessions::QueryContext;
use crate::sessions::TableContext;

pub struct DropDatabaseInterpreter {
    ctx: Arc<QueryContext>,
    plan: DropDatabasePlan,
}

impl DropDatabaseInterpreter {
    pub fn try_create(ctx: Arc<QueryContext>, plan: DropDatabasePlan) -> Result<Self> {
        Ok(DropDatabaseInterpreter { ctx, plan })
    }
}

#[async_trait::async_trait]
impl Interpreter for DropDatabaseInterpreter {
    fn name(&self) -> &str {
        "DropDatabaseInterpreter"
    }

    async fn execute(&self) -> Result<SendableDataBlockStream> {
        self.ctx
            .get_current_session()
            .validate_privilege(&GrantObject::Global, UserPrivilegeType::Drop)
            .await?;

        let catalog = self.ctx.get_catalog(&self.plan.catalog)?;
        catalog.drop_database(self.plan.clone().into()).await?;

        Ok(Box::pin(DataBlockStream::create(
            self.plan.schema(),
            None,
            vec![],
        )))
    }
}
