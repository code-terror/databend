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
use common_meta_app::schema::RenameTableReq;
use common_meta_app::schema::TableNameIdent;
use common_planners::RenameTablePlan;
use common_streams::DataBlockStream;
use common_streams::SendableDataBlockStream;

use crate::interpreters::Interpreter;
use crate::sessions::QueryContext;

pub struct RenameTableInterpreter {
    ctx: Arc<QueryContext>,
    plan: RenameTablePlan,
}

impl RenameTableInterpreter {
    pub fn try_create(ctx: Arc<QueryContext>, plan: RenameTablePlan) -> Result<Self> {
        Ok(RenameTableInterpreter { ctx, plan })
    }
}

#[async_trait::async_trait]
impl Interpreter for RenameTableInterpreter {
    fn name(&self) -> &str {
        "RenameTableInterpreter"
    }

    async fn execute(
        &self,
        _input_stream: Option<SendableDataBlockStream>,
    ) -> Result<SendableDataBlockStream> {
        // TODO check privileges
        // You must have ALTER and DROP privileges for the original table,
        // and CREATE and INSERT privileges for the new table.
        for entity in &self.plan.entities {
            let tenant = self.plan.tenant.clone();
            let catalog = self.ctx.get_catalog(&entity.catalog)?;
            catalog
                .rename_table(RenameTableReq {
                    if_exists: entity.if_exists,
                    name_ident: TableNameIdent {
                        tenant,
                        db_name: entity.database.clone(),
                        table_name: entity.table.clone(),
                    },
                    new_db_name: entity.new_database.clone(),
                    new_table_name: entity.new_table.clone(),
                })
                .await?;
        }

        Ok(Box::pin(DataBlockStream::create(
            self.plan.schema(),
            None,
            vec![],
        )))
    }
}
