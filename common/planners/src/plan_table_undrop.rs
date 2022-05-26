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

use common_datavalues::DataSchema;
use common_datavalues::DataSchemaRef;
use common_meta_types::TableNameIdent;
use common_meta_types::UndropTableReq;

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct UnDropTablePlan {
    pub tenant: String,
    pub catalog: String,
    pub db: String,
    pub table: String,
}

impl UnDropTablePlan {
    pub fn schema(&self) -> DataSchemaRef {
        Arc::new(DataSchema::empty())
    }
}

/// The table name
impl From<UnDropTablePlan> for UndropTableReq {
    fn from(p: UnDropTablePlan) -> Self {
        UndropTableReq {
            name_ident: TableNameIdent {
                tenant: p.tenant,
                db_name: p.db,
                table_name: p.table,
            },
        }
    }
}
