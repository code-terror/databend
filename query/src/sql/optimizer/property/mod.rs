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

mod builder;

use std::collections::HashSet;

pub use builder::RelExpr;

use crate::sql::common::IndexType;

pub type ColumnSet = HashSet<IndexType>;

pub struct NamedColumn {
    pub index: IndexType,
    pub name: String,
}

#[derive(Default, Clone)]
pub struct RequiredProperty {
    required_columns: ColumnSet,
}

impl RequiredProperty {
    pub fn create(required_columns: ColumnSet) -> Self {
        RequiredProperty { required_columns }
    }

    pub fn required_columns(&self) -> &ColumnSet {
        &self.required_columns
    }

    pub fn provided_by(
        &self,
        relational_prop: &RelationalProperty,
        _physical_prop: &PhysicalProperty,
    ) -> bool {
        self.required_columns()
            .is_subset(&relational_prop.output_columns)
    }
}

#[derive(Default, Clone)]
pub struct RelationalProperty {
    pub output_columns: ColumnSet,
    pub outer_columns: ColumnSet,
}

#[derive(Default, Clone)]
pub struct PhysicalProperty {}
