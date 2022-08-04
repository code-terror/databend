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

use crate::Expression;

/// Extras is a wrapper for push down items.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Extras {
    /// Optional column indices to use as a projection
    pub projection: Option<Vec<usize>>,
    /// Optional filter expression plan
    pub filters: Vec<Expression>,
    /// Optional limit to skip read
    pub limit: Option<usize>,
    /// Optional order_by expression plan
    pub order_by: Vec<Expression>,
}

impl Extras {
    pub fn default() -> Self {
        Extras {
            projection: None,
            filters: vec![],
            limit: None,
            order_by: vec![],
        }
    }
}
