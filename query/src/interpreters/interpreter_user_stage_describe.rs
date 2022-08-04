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

use common_exception::ErrorCode;
use common_exception::Result;
use common_planners::DescribeUserStagePlan;
use common_planners::PlanNode;
use common_streams::SendableDataBlockStream;

use super::SelectInterpreter;
use crate::interpreters::Interpreter;
use crate::optimizers::Optimizers;
use crate::sessions::QueryContext;
use crate::sessions::TableContext;
use crate::sql::PlanParser;

#[derive(Debug)]
pub struct DescribeUserStageInterpreter {
    ctx: Arc<QueryContext>,
    plan: DescribeUserStagePlan,
}

impl DescribeUserStageInterpreter {
    pub fn try_create(ctx: Arc<QueryContext>, plan: DescribeUserStagePlan) -> Result<Self> {
        Ok(DescribeUserStageInterpreter { ctx, plan })
    }

    fn build_query(&self, name: &str) -> Result<String> {
        Ok(format!("SELECT * FROM system.stages WHERE name = '{name}'"))
    }
}

#[async_trait::async_trait]
impl Interpreter for DescribeUserStageInterpreter {
    fn name(&self) -> &str {
        "DescribeUserStageInterpreter"
    }

    #[tracing::instrument(level = "info", skip(self), fields(ctx.id = self.ctx.get_id().as_str()))]
    async fn execute(&self) -> Result<SendableDataBlockStream> {
        let user_mgr = self.ctx.get_user_manager();
        let tenant = self.ctx.get_tenant();
        user_mgr.get_stage(&tenant, &self.plan.name).await?;

        let query = self.build_query(&self.plan.name)?;
        let plan = PlanParser::parse(self.ctx.clone(), &query).await?;
        let optimized = Optimizers::create(self.ctx.clone()).optimize(&plan)?;

        if let PlanNode::Select(plan) = optimized {
            let interpreter = SelectInterpreter::try_create(self.ctx.clone(), plan)?;
            interpreter.execute().await
        } else {
            return Err(ErrorCode::LogicalError("Describe stage build query error"));
        }
    }
}
