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
use std::fmt::Formatter;
use std::sync::Arc;

use common_exception::ErrorCode;
use common_exception::Result;
use common_planners::AggregatorFinalPlan;
use common_planners::AggregatorPartialPlan;
use common_planners::Partitions;
use common_planners::PlanBuilder;
use common_planners::PlanNode;
use common_planners::PlanRewriter;
use common_planners::ReadDataSourcePlan;

use crate::interpreters::fragments::partition_state::PartitionState;
use crate::interpreters::fragments::query_fragment::QueryFragment;
use crate::interpreters::fragments::query_fragment_actions::QueryFragmentAction;
use crate::interpreters::fragments::query_fragment_actions::QueryFragmentActions;
use crate::interpreters::fragments::query_fragment_actions::QueryFragmentsActions;
use crate::sessions::QueryContext;

pub struct ReadDatasourceQueryFragment {
    ctx: Arc<QueryContext>,
    read_data_source: ReadDataSourcePlan,
}

impl ReadDatasourceQueryFragment {
    pub fn create(
        ctx: Arc<QueryContext>,
        plan: &ReadDataSourcePlan,
    ) -> Result<Box<dyn QueryFragment>> {
        Ok(Box::new(ReadDatasourceQueryFragment {
            ctx,
            read_data_source: plan.clone(),
        }))
    }
}

impl ReadDatasourceQueryFragment {
    pub fn repartition(&self, new_size: usize) -> Vec<Partitions> {
        // We always put adjacent partitions in the same node
        let partitions = &self.read_data_source.parts;
        let parts_per_node = partitions.len() / new_size;

        let mut nodes_parts = Vec::with_capacity(new_size);
        for index in 0..new_size {
            let begin = parts_per_node * index;
            let end = parts_per_node * (index + 1);
            let node_parts = partitions[begin..end].to_vec();

            nodes_parts.push(node_parts);
        }

        // For some irregular partitions, we assign them to the head nodes
        let begin = parts_per_node * new_size;
        let remain_cluster_parts = &partitions[begin..];
        for index in 0..remain_cluster_parts.len() {
            nodes_parts[index].push(remain_cluster_parts[index].clone());
        }

        nodes_parts
    }
}

impl QueryFragment for ReadDatasourceQueryFragment {
    fn distribute_query(&self) -> Result<bool> {
        let read_table = self
            .ctx
            .build_table_from_source_plan(&self.read_data_source)?;

        Ok(!read_table.is_local())
    }

    fn get_out_partition(&self) -> Result<PartitionState> {
        let read_table = self
            .ctx
            .build_table_from_source_plan(&self.read_data_source)?;

        match read_table.is_local() {
            true => Ok(PartitionState::NotPartition),
            false => Ok(PartitionState::Random),
        }
    }

    fn finalize(&self, actions: &mut QueryFragmentsActions) -> Result<()> {
        let fragment_id = self.ctx.get_fragment_id();
        let mut fragment_actions = QueryFragmentActions::create(false, fragment_id);

        match self.get_out_partition()? {
            PartitionState::NotPartition => {
                fragment_actions.add_action(QueryFragmentAction::create(
                    actions.get_local_executor(),
                    PlanNode::ReadSource(self.read_data_source.clone()),
                ));
            }
            _ => {
                let executors = actions.get_executors();
                let new_partitions = self.repartition(executors.len());

                for (index, executor) in executors.iter().enumerate() {
                    fragment_actions.add_action(QueryFragmentAction::create(
                        executor.clone(),
                        PlanNode::ReadSource(ReadDataSourcePlan {
                            parts: new_partitions[index].to_owned(),
                            catalog: self.read_data_source.catalog.clone(),
                            source_info: self.read_data_source.source_info.clone(),
                            scan_fields: self.read_data_source.scan_fields.clone(),
                            statistics: self.read_data_source.statistics.clone(),
                            description: self.read_data_source.description.clone(),
                            tbl_args: self.read_data_source.tbl_args.clone(),
                            push_downs: self.read_data_source.push_downs.clone(),
                        }),
                    ))
                }
            }
        }

        actions.add_fragment_actions(fragment_actions)
    }

    fn rewrite_remote_plan(&self, node: &PlanNode, new_node: &PlanNode) -> Result<PlanNode> {
        if !matches!(new_node, PlanNode::ReadSource(_)) {
            return Err(ErrorCode::UnknownPlan(
                "Unknown plan type while in rewrite_remote_plan",
            ));
        }

        // use new node replace node children.
        ReplaceDataSource { new_node }.rewrite_plan_node(node)
    }
}

struct ReplaceDataSource<'a> {
    new_node: &'a PlanNode,
}

impl<'a> PlanRewriter for ReplaceDataSource<'a> {
    fn rewrite_aggregate_partial(&mut self, plan: &AggregatorPartialPlan) -> Result<PlanNode> {
        PlanBuilder::from(&self.rewrite_plan_node(&plan.input)?)
            .aggregate_partial(&plan.aggr_expr, &plan.group_expr)?
            .build()
    }

    fn rewrite_aggregate_final(&mut self, plan: &AggregatorFinalPlan) -> Result<PlanNode> {
        let schema = plan.schema_before_group_by.clone();
        let new_input = self.rewrite_plan_node(&plan.input)?;

        PlanBuilder::from(&new_input)
            .aggregate_final(schema, &plan.aggr_expr, &plan.group_expr)?
            .build()
    }

    fn rewrite_read_data_source(&mut self, _: &ReadDataSourcePlan) -> Result<PlanNode> {
        Ok(self.new_node.clone())
    }
}

impl Debug for ReadDatasourceQueryFragment {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReadDatasourceQueryFragment").finish()
    }
}
