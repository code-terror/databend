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

use common_exception::Result;

use crate::sql::optimizer::rule::transform_state::TransformState;
use crate::sql::optimizer::rule::Rule;
use crate::sql::optimizer::rule::RuleID;
use crate::sql::optimizer::SExpr;
use crate::sql::plans::LogicalInnerJoin;
use crate::sql::plans::PatternPlan;
use crate::sql::plans::PhysicalHashJoin;
use crate::sql::plans::PlanType;

pub struct RuleImplementHashJoin {
    id: RuleID,
    pattern: SExpr,
}

impl RuleImplementHashJoin {
    pub fn create() -> Self {
        RuleImplementHashJoin {
            id: RuleID::ImplementHashJoin,
            pattern: SExpr::create_binary(
                PatternPlan {
                    plan_type: PlanType::LogicalInnerJoin,
                }
                .into(),
                SExpr::create_leaf(
                    PatternPlan {
                        plan_type: PlanType::Pattern,
                    }
                    .into(),
                ),
                SExpr::create_leaf(
                    PatternPlan {
                        plan_type: PlanType::Pattern,
                    }
                    .into(),
                ),
            ),
        }
    }
}

impl Rule for RuleImplementHashJoin {
    fn id(&self) -> RuleID {
        self.id
    }

    fn apply(&self, expression: &SExpr, state: &mut TransformState) -> Result<()> {
        let plan = expression.plan().clone();
        let logical_inner_join: LogicalInnerJoin = plan.try_into()?;

        let result = SExpr::create(
            PhysicalHashJoin {
                build_keys: logical_inner_join.right_conditions,
                probe_keys: logical_inner_join.left_conditions,
            }
            .into(),
            expression.children().to_vec(),
            expression.original_group(),
        );
        state.add_result(result);

        Ok(())
    }

    fn pattern(&self) -> &SExpr {
        &self.pattern
    }
}
