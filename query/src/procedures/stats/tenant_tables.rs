//  Copyright 2022 Datafuse Labs.
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

use std::sync::Arc;

use common_datablocks::DataBlock;
use common_datavalues::prelude::*;
use common_datavalues::DataField;
use common_datavalues::DataSchema;
use common_datavalues::DataSchemaRefExt;
use common_exception::Result;
use common_meta_app::schema::CountTablesReq;
use common_meta_types::UserOptionFlag;

use crate::procedures::Procedure;
use crate::procedures::ProcedureFeatures;
use crate::sessions::QueryContext;
use crate::sessions::TableContext;

pub struct TenantTablesProcedure {}

impl TenantTablesProcedure {
    pub fn try_create() -> Result<Box<dyn Procedure>> {
        Ok(Box::new(TenantTablesProcedure {}))
    }
}

#[async_trait::async_trait]
impl Procedure for TenantTablesProcedure {
    fn name(&self) -> &str {
        "TENANT_TABLES"
    }

    // args:
    // tenant_id: []string
    fn features(&self) -> ProcedureFeatures {
        ProcedureFeatures::default()
            .variadic_arguments(1, usize::MAX - 1)
            .management_mode_required(true)
            .user_option_flag(UserOptionFlag::TenantSetting)
    }

    async fn inner_eval(&self, ctx: Arc<QueryContext>, args: Vec<String>) -> Result<DataBlock> {
        let mut table_counts: Vec<u64> = Vec::with_capacity(args.len());
        for tenant in args.iter() {
            let catalog = ctx.get_catalog(&ctx.get_current_catalog())?;
            let table_count = catalog
                .count_tables(CountTablesReq {
                    tenant: tenant.to_string(),
                })
                .await?;
            table_counts.push(table_count.count);
        }

        Ok(DataBlock::create(self.schema(), vec![
            Series::from_data(args),
            Series::from_data(table_counts),
        ]))
    }

    fn schema(&self) -> Arc<DataSchema> {
        DataSchemaRefExt::create(vec![
            DataField::new("tenant_id", Vu8::to_data_type()),
            DataField::new("table_count", u64::to_data_type()),
        ])
    }
}
