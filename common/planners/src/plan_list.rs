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

use std::fmt::Debug;
use std::fmt::Formatter;
use std::sync::Arc;

use common_datavalues::prelude::ToDataType;
use common_datavalues::prelude::*;
use common_datavalues::DataField;
use common_datavalues::DataSchema;
use common_datavalues::DataSchemaRef;
use common_meta_types::UserStageInfo;

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone)]
pub struct ListPlan {
    pub stage: UserStageInfo,
    pub path: String,
    pub pattern: String,
}

impl ListPlan {
    pub fn schema(&self) -> DataSchemaRef {
        let name = DataField::new("name", Vu8::to_data_type());
        let size = DataField::new("size", u64::to_data_type());
        let md5 = DataField::new_nullable("md5", Vu8::to_data_type());
        let last_modified = DataField::new("last_modified", Vu8::to_data_type());
        let creator = DataField::new_nullable("creator", Vu8::to_data_type());
        Arc::new(DataSchema::new(vec![
            name,
            size,
            md5,
            last_modified,
            creator,
        ]))
    }
}

impl Debug for ListPlan {
    // Ignore the schema.
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "List {:?}", self.stage)?;
        if !self.pattern.is_empty() {
            write!(f, " ,pattern:{:?}", self.pattern)?;
        }
        Ok(())
    }
}
