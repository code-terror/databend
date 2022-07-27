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

use common_datablocks::DataBlock;
use common_datavalues::DataSchemaRef;
use common_meta_types::MetaId;

use crate::PlanNode;

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub enum InsertInputSource {
    SelectPlan(Box<PlanNode>),
    StreamingWithFormat(String),
    Values(InsertValueBlock),
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct InsertValueBlock {
    #[serde(skip)]
    pub block: DataBlock,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct InsertPlan {
    pub catalog: String,
    pub database: String,
    pub table: String,
    pub table_id: MetaId,
    pub schema: DataSchemaRef,
    pub overwrite: bool,
    pub source: InsertInputSource,
}

impl PartialEq for InsertPlan {
    fn eq(&self, other: &Self) -> bool {
        self.catalog == other.catalog
            && self.database == other.database
            && self.table == other.table
            && self.schema == other.schema
    }
}

impl InsertPlan {
    pub fn schema(&self) -> DataSchemaRef {
        self.schema.clone()
    }

    pub fn has_select_plan(&self) -> bool {
        matches!(&self.source, InsertInputSource::SelectPlan(_))
    }

    pub fn format(&self) -> Option<&str> {
        match &self.source {
            InsertInputSource::SelectPlan(_) => None,
            InsertInputSource::StreamingWithFormat(v) => Some(v.as_str()),
            InsertInputSource::Values(_) => Some("values"),
        }
    }
}
