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

mod display_rel_operator;
use std::fmt::Display;
mod indent_format;

pub struct FormatTreeNode<T: Display> {
    payload: T,
    children: Vec<Self>,
}

impl<T> FormatTreeNode<T>
where T: Display
{
    pub fn new(payload: T) -> Self {
        Self {
            payload,
            children: vec![],
        }
    }

    pub fn with_children(payload: T, children: Vec<Self>) -> Self {
        Self { payload, children }
    }
}
