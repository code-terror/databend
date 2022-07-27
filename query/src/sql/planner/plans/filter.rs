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

use common_exception::Result;

use crate::sql::optimizer::ColumnSet;
use crate::sql::optimizer::PhysicalProperty;
use crate::sql::optimizer::RelExpr;
use crate::sql::optimizer::RelationalProperty;
use crate::sql::optimizer::SExpr;
use crate::sql::plans::LogicalPlan;
use crate::sql::plans::Operator;
use crate::sql::plans::PhysicalPlan;
use crate::sql::plans::RelOp;
use crate::sql::plans::Scalar;
use crate::sql::plans::ScalarExpr;

#[derive(Clone, Debug)]
pub struct Filter {
    pub predicates: Vec<Scalar>,
    // True if the plan represents having, else the plan represents where
    pub is_having: bool,
}

impl Operator for Filter {
    fn rel_op(&self) -> RelOp {
        RelOp::Filter
    }

    fn is_physical(&self) -> bool {
        true
    }

    fn is_logical(&self) -> bool {
        true
    }

    fn as_physical(&self) -> Option<&dyn PhysicalPlan> {
        Some(self)
    }

    fn as_logical(&self) -> Option<&dyn LogicalPlan> {
        Some(self)
    }
}

impl PhysicalPlan for Filter {
    fn compute_physical_prop(&self, _expression: &SExpr) -> PhysicalProperty {
        todo!()
    }
}

impl LogicalPlan for Filter {
    fn derive_relational_prop<'a>(&self, rel_expr: &RelExpr<'a>) -> Result<RelationalProperty> {
        let input_prop = rel_expr.derive_relational_prop_child(0)?;
        let output_columns = input_prop.output_columns;

        // Derive outer columns
        let mut outer_columns = input_prop.outer_columns;
        for scalar in self.predicates.iter() {
            let used_columns = scalar.used_columns();
            let outer = used_columns
                .difference(&output_columns)
                .cloned()
                .collect::<ColumnSet>();
            outer_columns = outer_columns.union(&outer).cloned().collect();
        }
        outer_columns = outer_columns.difference(&output_columns).cloned().collect();

        Ok(RelationalProperty {
            output_columns,
            outer_columns,
        })
    }
}
