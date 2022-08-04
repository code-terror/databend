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

use std::fmt::Debug;
use std::sync::Arc;

use common_exception::ErrorCode;
use common_exception::Result;
use common_planners::AggregatorFinalPlan;
use common_planners::AggregatorPartialPlan;
use common_planners::BroadcastPlan;
use common_planners::ExpressionPlan;
use common_planners::FilterPlan;
use common_planners::HavingPlan;
use common_planners::LimitByPlan;
use common_planners::LimitPlan;
use common_planners::PlanNode;
use common_planners::ProjectionPlan;
use common_planners::ReadDataSourcePlan;
use common_planners::SelectPlan;
use common_planners::SinkPlan;
use common_planners::SortPlan;
use common_planners::StagePlan;
use common_planners::SubQueriesSetPlan;
use common_planners::WindowFuncPlan;

use crate::interpreters::fragments::partition_state::PartitionState;
use crate::interpreters::fragments::query_fragment_actions::QueryFragmentsActions;
use crate::interpreters::fragments::query_fragment_broadcast::BroadcastQueryFragment;
use crate::interpreters::fragments::query_fragment_read_source::ReadDatasourceQueryFragment;
use crate::interpreters::fragments::query_fragment_stage::StageQueryFragment;
use crate::interpreters::fragments::query_fragment_subqueries::SubQueriesFragment;
use crate::sessions::QueryContext;

// A fragment of query, the smallest execution unit of a distributed query
pub trait QueryFragment: Debug + Sync + Send {
    fn is_distributed_query(&self) -> Result<bool>;

    fn get_out_partition(&self) -> Result<PartitionState>;

    fn finalize(&self, nodes: &mut QueryFragmentsActions) -> Result<()>;

    fn rewrite_remote_plan(&self, node: &PlanNode, new: &PlanNode) -> Result<PlanNode>;
}

pub struct QueryFragmentsBuilder;

impl QueryFragmentsBuilder {
    pub fn build(ctx: Arc<QueryContext>, plan: &PlanNode) -> Result<Box<dyn QueryFragment>> {
        BuilderVisitor { ctx }.visit(plan)
    }
}

pub struct BuilderVisitor {
    ctx: Arc<QueryContext>,
}

impl BuilderVisitor {
    pub fn create(ctx: Arc<QueryContext>) -> BuilderVisitor {
        BuilderVisitor { ctx }
    }

    pub fn visit(&self, plan: &PlanNode) -> Result<Box<dyn QueryFragment>> {
        match plan {
            PlanNode::Stage(node) => self.visit_stage(node),
            PlanNode::Select(node) => self.visit_select(node),
            PlanNode::Broadcast(node) => self.visit_broadcast(node),
            PlanNode::AggregatorFinal(node) => self.visit_aggr_final(node),
            PlanNode::AggregatorPartial(node) => self.visit_aggr_part(node),
            // PlanNode::Empty(plan) => self.visit_empty(plan, tasks),
            PlanNode::Filter(node) => self.visit_filter(node),
            PlanNode::Projection(node) => self.visit_projection(node),
            PlanNode::Sort(node) => self.visit_sort(node),
            PlanNode::Limit(node) => self.visit_limit(node),
            PlanNode::LimitBy(node) => self.visit_limit_by(node),
            PlanNode::ReadSource(node) => self.visit_read_data_source(node),
            PlanNode::Sink(node) => self.visit_sink(node),
            PlanNode::Having(node) => self.visit_having(node),
            PlanNode::Expression(node) => self.visit_expression(node),
            PlanNode::WindowFunc(node) => self.visit_window_func(node),
            PlanNode::SubQueryExpression(node) => self.visit_subquery_expr(node),
            _ => Err(ErrorCode::UnknownPlan("Unknown plan type")),
        }
    }

    fn visit_stage(&self, node: &StagePlan) -> Result<Box<dyn QueryFragment>> {
        StageQueryFragment::create(self.ctx.clone(), node, self.visit(&node.input)?)
    }

    fn visit_select(&self, node: &SelectPlan) -> Result<Box<dyn QueryFragment>> {
        self.visit(&node.input)
    }

    fn visit_sort(&self, node: &SortPlan) -> Result<Box<dyn QueryFragment>> {
        self.visit(&node.input)
    }

    fn visit_sink(&self, node: &SinkPlan) -> Result<Box<dyn QueryFragment>> {
        self.visit(&node.input)
    }

    fn visit_limit(&self, node: &LimitPlan) -> Result<Box<dyn QueryFragment>> {
        self.visit(&node.input)
    }

    fn visit_having(&self, node: &HavingPlan) -> Result<Box<dyn QueryFragment>> {
        self.visit(&node.input)
    }

    fn visit_filter(&self, node: &FilterPlan) -> Result<Box<dyn QueryFragment>> {
        self.visit(&node.input)
    }

    fn visit_limit_by(&self, node: &LimitByPlan) -> Result<Box<dyn QueryFragment>> {
        self.visit(&node.input)
    }

    fn visit_expression(&self, node: &ExpressionPlan) -> Result<Box<dyn QueryFragment>> {
        self.visit(&node.input)
    }

    fn visit_projection(&self, node: &ProjectionPlan) -> Result<Box<dyn QueryFragment>> {
        self.visit(&node.input)
    }

    fn visit_aggr_final(&self, node: &AggregatorFinalPlan) -> Result<Box<dyn QueryFragment>> {
        self.visit(&node.input)
    }

    fn visit_aggr_part(&self, node: &AggregatorPartialPlan) -> Result<Box<dyn QueryFragment>> {
        self.visit(&node.input)
    }

    fn visit_window_func(&self, node: &WindowFuncPlan) -> Result<Box<dyn QueryFragment>> {
        self.visit(&node.input)
    }

    fn visit_read_data_source(&self, node: &ReadDataSourcePlan) -> Result<Box<dyn QueryFragment>> {
        ReadDatasourceQueryFragment::create(self.ctx.clone(), node)
    }

    fn visit_broadcast(&self, node: &BroadcastPlan) -> Result<Box<dyn QueryFragment>> {
        BroadcastQueryFragment::create(self.ctx.clone(), node, self.visit(&node.input)?)
    }

    fn visit_subquery_expr(&self, node: &SubQueriesSetPlan) -> Result<Box<dyn QueryFragment>> {
        let input = self.visit(&node.input)?;
        SubQueriesFragment::create(self.ctx.clone(), node, input)
    }
}
