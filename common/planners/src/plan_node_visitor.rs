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

use crate::plan_broadcast::BroadcastPlan;
use crate::plan_subqueries_set::SubQueriesSetPlan;
use crate::plan_table_undrop::UndropTablePlan;
use crate::plan_window_func::WindowFuncPlan;
use crate::AggregatorFinalPlan;
use crate::AggregatorPartialPlan;
use crate::AlterTableClusterKeyPlan;
use crate::AlterUserPlan;
use crate::AlterUserUDFPlan;
use crate::AlterViewPlan;
use crate::CallPlan;
use crate::CopyPlan;
use crate::CreateDatabasePlan;
use crate::CreateRolePlan;
use crate::CreateTablePlan;
use crate::CreateUserPlan;
use crate::CreateUserStagePlan;
use crate::CreateUserUDFPlan;
use crate::CreateViewPlan;
use crate::DeletePlan;
use crate::DescribeTablePlan;
use crate::DescribeUserStagePlan;
use crate::DropDatabasePlan;
use crate::DropRolePlan;
use crate::DropTableClusterKeyPlan;
use crate::DropTablePlan;
use crate::DropUserPlan;
use crate::DropUserStagePlan;
use crate::DropUserUDFPlan;
use crate::DropViewPlan;
use crate::EmptyPlan;
use crate::ExistsTablePlan;
use crate::ExplainPlan;
use crate::Expression;
use crate::ExpressionPlan;
use crate::FilterPlan;
use crate::GrantPrivilegePlan;
use crate::GrantRolePlan;
use crate::HavingPlan;
use crate::InsertPlan;
use crate::KillPlan;
use crate::LimitByPlan;
use crate::LimitPlan;
use crate::ListPlan;
use crate::OptimizeTablePlan;
use crate::PlanNode;
use crate::ProjectionPlan;
use crate::ReadDataSourcePlan;
use crate::RemotePlan;
use crate::RemoveUserStagePlan;
use crate::RenameDatabasePlan;
use crate::RenameTablePlan;
use crate::RevokePrivilegePlan;
use crate::RevokeRolePlan;
use crate::SelectPlan;
use crate::SettingPlan;
use crate::ShowCreateDatabasePlan;
use crate::ShowCreateTablePlan;
use crate::ShowPlan;
use crate::SinkPlan;
use crate::SortPlan;
use crate::StagePlan;
use crate::TruncateTablePlan;
use crate::UndropDatabasePlan;
use crate::UseDatabasePlan;

/// `PlanVisitor` implements visitor pattern(reference [syn](https://docs.rs/syn/1.0.72/syn/visit/trait.Visit.html)) for `PlanNode`.
///
/// `PlanVisitor` would provide default implementations for each variant of `PlanNode` to visit a plan tree in preorder.
/// You can customize the way to visit nodes by overriding corresponding methods.
///
/// Since a visitor will always modify itself during visiting, we pass `&mut self` to each visit method.
///
/// # Example
/// Here's an example of printing table names of all `Scan` nodes in a plan tree:
/// ```ignore
/// struct MyVisitor {}
///
/// impl<'plan> PlanVisitor<'plan> for MyVisitor {
///     fn visit_read_data_source(&mut self, plan: &'plan ReadDataSourcePlan) {
///         println!("{}", plan.schema_name)
///     }
/// }
///
/// let visitor = MyVisitor {};
/// let plan = PlanNode::ReadDataSource(ReadDataSourcePlan {
///     schema_name: "table",
///     ...
/// });
/// visitor.visit_plan_node(&plan); // Output: table
/// ```
///
/// By default, `PlanVisitor` will visit all `PlanNode` with depth first traversal(i.e. recursively access children of a node).
/// In some cases, people want to explicitly traverse the tree in pre-order or post-order, for whom the default implementation
/// doesn't work. Here we provide an example of pre-order traversal:
/// ```ignore
/// struct PreOrder {
///     pub process: FnMut(&PlanNode)
/// }
///
/// impl<'plan> PlanVisitor<'plan> for PreOrder {
///     fn visit_plan_node(&mut self, plan: &PlanNode) {
///         self.process(plan); // Process current node first
///         PlanVisitor::visit_plan_node(self, plan.child().as_ref()); // Then process children
///     }
/// }
/// ```
pub trait PlanVisitor {
    fn visit_plan_node(&mut self, node: &PlanNode) -> Result<()> {
        match node {
            // Base.
            PlanNode::AggregatorPartial(plan) => self.visit_aggregate_partial(plan),
            PlanNode::AggregatorFinal(plan) => self.visit_aggregate_final(plan),
            PlanNode::Empty(plan) => self.visit_empty(plan),
            PlanNode::Projection(plan) => self.visit_projection(plan),
            PlanNode::Filter(plan) => self.visit_filter(plan),
            PlanNode::Sort(plan) => self.visit_sort(plan),
            PlanNode::Stage(plan) => self.visit_stage(plan),
            PlanNode::Broadcast(plan) => self.visit_broadcast(plan),
            PlanNode::Remote(plan) => self.visit_remote(plan),
            PlanNode::Having(plan) => self.visit_having(plan),
            PlanNode::WindowFunc(plan) => self.visit_window_func(plan),
            PlanNode::Expression(plan) => self.visit_expression(plan),
            PlanNode::Limit(plan) => self.visit_limit(plan),
            PlanNode::LimitBy(plan) => self.visit_limit_by(plan),
            PlanNode::ReadSource(plan) => self.visit_read_data_source(plan),
            PlanNode::SubQueryExpression(plan) => self.visit_sub_queries_sets(plan),
            PlanNode::Sink(plan) => self.visit_append(plan),

            // Query.
            PlanNode::Select(plan) => self.visit_select(plan),

            // Explain.
            PlanNode::Explain(plan) => self.visit_explain(plan),

            // Insert.
            PlanNode::Insert(plan) => self.visit_insert_into(plan),

            // Insert.
            PlanNode::Delete(plan) => self.visit_delete_into(plan),

            // Copy.
            PlanNode::Copy(plan) => self.visit_copy(plan),

            // Call.
            PlanNode::Call(plan) => self.visit_call(plan),

            // Show.
            PlanNode::Show(plan) => self.visit_show(plan),

            // Database.
            PlanNode::CreateDatabase(plan) => self.visit_create_database(plan),
            PlanNode::DropDatabase(plan) => self.visit_drop_database(plan),
            PlanNode::ShowCreateDatabase(plan) => self.visit_show_create_database(plan),
            PlanNode::RenameDatabase(plan) => self.visit_rename_database(plan),
            PlanNode::UndropDatabase(plan) => self.visit_undrop_database(plan),
            // Table.
            PlanNode::CreateTable(plan) => self.visit_create_table(plan),
            PlanNode::DropTable(plan) => self.visit_drop_table(plan),
            PlanNode::UndropTable(plan) => self.visit_undrop_table(plan),
            PlanNode::RenameTable(plan) => self.visit_rename_table(plan),
            PlanNode::TruncateTable(plan) => self.visit_truncate_table(plan),
            PlanNode::OptimizeTable(plan) => self.visit_optimize_table(plan),
            PlanNode::ExistsTable(plan) => self.visit_exists_table(plan),
            PlanNode::DescribeTable(plan) => self.visit_describe_table(plan),
            PlanNode::ShowCreateTable(plan) => self.visit_show_create_table(plan),

            // View.
            PlanNode::CreateView(v) => self.visit_create_view(v),
            PlanNode::AlterView(v) => self.visit_alter_view(v),
            PlanNode::DropView(v) => self.visit_drop_view(v),

            // User.
            PlanNode::CreateUser(plan) => self.visit_create_user(plan),
            PlanNode::AlterUser(plan) => self.visit_alter_user(plan),
            PlanNode::DropUser(plan) => self.visit_drop_user(plan),

            // Grant
            PlanNode::GrantPrivilege(plan) => self.visit_grant_privilege(plan),
            PlanNode::GrantRole(plan) => self.visit_grant_role(plan),

            // Revoke
            PlanNode::RevokePrivilege(plan) => self.visit_revoke_privilege(plan),
            PlanNode::RevokeRole(plan) => self.visit_revoke_role(plan),

            // Role.
            PlanNode::CreateRole(plan) => self.visit_create_role(plan),
            PlanNode::DropRole(plan) => self.visit_drop_role(plan),

            // Stage.
            PlanNode::CreateUserStage(plan) => self.visit_create_user_stage(plan),
            PlanNode::DropUserStage(plan) => self.visit_drop_user_stage(plan),
            PlanNode::DescribeUserStage(plan) => self.visit_describe_user_stage(plan),
            PlanNode::List(plan) => self.visit_list(plan),
            PlanNode::RemoveUserStage(plan) => self.visit_remove_user_stage(plan),

            // UDF.
            PlanNode::CreateUserUDF(plan) => self.visit_create_user_udf(plan),
            PlanNode::DropUserUDF(plan) => self.visit_drop_user_udf(plan),
            PlanNode::AlterUserUDF(plan) => self.visit_alter_user_udf(plan),

            // Use.
            PlanNode::UseDatabase(plan) => self.visit_use_database(plan),

            // Set.
            PlanNode::SetVariable(plan) => self.visit_set_variable(plan),

            // Kill.
            PlanNode::Kill(plan) => self.visit_kill_query(plan),

            // Cluster Key.
            PlanNode::AlterTableClusterKey(plan) => self.visit_alter_table_cluster_key(plan),
            PlanNode::DropTableClusterKey(plan) => self.visit_drop_table_cluster_key(plan),
        }
    }

    fn visit_subquery_plan(&mut self, subquery_plan: &PlanNode) -> Result<()> {
        self.visit_plan_node(subquery_plan)
    }

    // TODO: Move it to ExpressionsVisitor trait
    fn visit_expr(&mut self, expr: &Expression) -> Result<()> {
        match expr {
            Expression::Subquery { query_plan, .. } => {
                self.visit_subquery_plan(query_plan.as_ref())
            }
            Expression::ScalarSubquery { query_plan, .. } => {
                self.visit_subquery_plan(query_plan.as_ref())
            }
            _ => Ok(()),
        }
    }

    // TODO: Move it to ExpressionsVisitor trait
    fn visit_exprs(&mut self, exprs: &[Expression]) -> Result<()> {
        for expr in exprs {
            self.visit_expr(expr)?;
        }

        Ok(())
    }

    fn visit_aggregate_partial(&mut self, plan: &AggregatorPartialPlan) -> Result<()> {
        self.visit_plan_node(plan.input.as_ref())?;
        self.visit_exprs(&plan.aggr_expr)?;
        self.visit_exprs(&plan.group_expr)
    }

    fn visit_aggregate_final(&mut self, plan: &AggregatorFinalPlan) -> Result<()> {
        self.visit_plan_node(plan.input.as_ref())?;
        self.visit_exprs(&plan.aggr_expr)?;
        self.visit_exprs(&plan.group_expr)
    }

    fn visit_empty(&mut self, _: &EmptyPlan) -> Result<()> {
        Ok(())
    }

    fn visit_stage(&mut self, plan: &StagePlan) -> Result<()> {
        self.visit_plan_node(plan.input.as_ref())
    }

    fn visit_broadcast(&mut self, plan: &BroadcastPlan) -> Result<()> {
        self.visit_plan_node(plan.input.as_ref())
    }

    fn visit_remote(&mut self, _: &RemotePlan) -> Result<()> {
        Ok(())
    }

    fn visit_projection(&mut self, plan: &ProjectionPlan) -> Result<()> {
        self.visit_plan_node(plan.input.as_ref())?;
        self.visit_exprs(&plan.expr)
    }

    fn visit_expression(&mut self, plan: &ExpressionPlan) -> Result<()> {
        self.visit_plan_node(plan.input.as_ref())?;
        self.visit_exprs(&plan.exprs)
    }

    fn visit_sub_queries_sets(&mut self, plan: &SubQueriesSetPlan) -> Result<()> {
        self.visit_plan_node(plan.input.as_ref())?;
        self.visit_exprs(&plan.expressions)
    }

    fn visit_filter(&mut self, plan: &FilterPlan) -> Result<()> {
        self.visit_plan_node(plan.input.as_ref())?;
        self.visit_expr(&plan.predicate)
    }

    fn visit_having(&mut self, plan: &HavingPlan) -> Result<()> {
        self.visit_plan_node(plan.input.as_ref())?;
        self.visit_expr(&plan.predicate)
    }

    fn visit_window_func(&mut self, plan: &WindowFuncPlan) -> Result<()> {
        self.visit_plan_node(plan.input.as_ref())?;
        self.visit_expr(&plan.window_func)
    }

    fn visit_sort(&mut self, plan: &SortPlan) -> Result<()> {
        self.visit_plan_node(plan.input.as_ref())?;
        self.visit_exprs(&plan.order_by)
    }

    fn visit_limit(&mut self, plan: &LimitPlan) -> Result<()> {
        self.visit_plan_node(plan.input.as_ref())
    }

    fn visit_limit_by(&mut self, plan: &LimitByPlan) -> Result<()> {
        self.visit_plan_node(plan.input.as_ref())
    }

    fn visit_read_data_source(&mut self, _: &ReadDataSourcePlan) -> Result<()> {
        Ok(())
    }

    fn visit_select(&mut self, plan: &SelectPlan) -> Result<()> {
        self.visit_plan_node(plan.input.as_ref())
    }

    fn visit_explain(&mut self, plan: &ExplainPlan) -> Result<()> {
        self.visit_plan_node(plan.input.as_ref())
    }

    fn visit_create_database(&mut self, _: &CreateDatabasePlan) -> Result<()> {
        Ok(())
    }

    fn visit_drop_database(&mut self, _: &DropDatabasePlan) -> Result<()> {
        Ok(())
    }

    fn visit_rename_database(&mut self, _: &RenameDatabasePlan) -> Result<()> {
        Ok(())
    }

    fn visit_create_table(&mut self, _: &CreateTablePlan) -> Result<()> {
        Ok(())
    }

    fn visit_create_user(&mut self, _: &CreateUserPlan) -> Result<()> {
        Ok(())
    }

    fn visit_alter_user(&mut self, _: &AlterUserPlan) -> Result<()> {
        Ok(())
    }

    fn visit_drop_user(&mut self, _: &DropUserPlan) -> Result<()> {
        Ok(())
    }

    fn visit_grant_privilege(&mut self, _: &GrantPrivilegePlan) -> Result<()> {
        Ok(())
    }

    fn visit_grant_role(&mut self, _: &GrantRolePlan) -> Result<()> {
        Ok(())
    }

    fn visit_revoke_privilege(&mut self, _: &RevokePrivilegePlan) -> Result<()> {
        Ok(())
    }

    fn visit_revoke_role(&mut self, _: &RevokeRolePlan) -> Result<()> {
        Ok(())
    }

    fn visit_create_role(&mut self, _: &CreateRolePlan) -> Result<()> {
        Ok(())
    }

    fn visit_drop_role(&mut self, _: &DropRolePlan) -> Result<()> {
        Ok(())
    }

    fn visit_describe_table(&mut self, _: &DescribeTablePlan) -> Result<()> {
        Ok(())
    }

    fn visit_rename_table(&mut self, _: &RenameTablePlan) -> Result<()> {
        Ok(())
    }

    fn visit_optimize_table(&mut self, _: &OptimizeTablePlan) -> Result<()> {
        Ok(())
    }

    fn visit_exists_table(&mut self, _: &ExistsTablePlan) -> Result<()> {
        Ok(())
    }

    fn visit_describe_user_stage(&mut self, _: &DescribeUserStagePlan) -> Result<()> {
        Ok(())
    }

    fn visit_list(&mut self, _: &ListPlan) -> Result<()> {
        Ok(())
    }

    fn visit_drop_table(&mut self, _: &DropTablePlan) -> Result<()> {
        Ok(())
    }

    fn visit_undrop_table(&mut self, _: &UndropTablePlan) -> Result<()> {
        Ok(())
    }

    fn visit_undrop_database(&mut self, _: &UndropDatabasePlan) -> Result<()> {
        Ok(())
    }

    fn visit_use_database(&mut self, _: &UseDatabasePlan) -> Result<()> {
        Ok(())
    }

    fn visit_set_variable(&mut self, _: &SettingPlan) -> Result<()> {
        Ok(())
    }

    fn visit_insert_into(&mut self, _: &InsertPlan) -> Result<()> {
        Ok(())
    }

    fn visit_delete_into(&mut self, _: &DeletePlan) -> Result<()> {
        Ok(())
    }

    fn visit_copy(&mut self, _: &CopyPlan) -> Result<()> {
        Ok(())
    }

    fn visit_call(&mut self, _: &CallPlan) -> Result<()> {
        Ok(())
    }

    fn visit_show_create_table(&mut self, _: &ShowCreateTablePlan) -> Result<()> {
        Ok(())
    }

    fn visit_truncate_table(&mut self, _: &TruncateTablePlan) -> Result<()> {
        Ok(())
    }

    fn visit_create_view(&mut self, _: &CreateViewPlan) -> Result<()> {
        Ok(())
    }

    fn visit_drop_view(&mut self, _: &DropViewPlan) -> Result<()> {
        Ok(())
    }

    fn visit_alter_view(&mut self, _: &AlterViewPlan) -> Result<()> {
        Ok(())
    }

    fn visit_kill_query(&mut self, _: &KillPlan) -> Result<()> {
        Ok(())
    }
    fn visit_append(&mut self, _: &SinkPlan) -> Result<()> {
        Ok(())
    }

    fn visit_create_user_stage(&mut self, _: &CreateUserStagePlan) -> Result<()> {
        Ok(())
    }

    fn visit_drop_user_stage(&mut self, _: &DropUserStagePlan) -> Result<()> {
        Ok(())
    }

    fn visit_show_create_database(&mut self, _: &ShowCreateDatabasePlan) -> Result<()> {
        Ok(())
    }

    fn visit_show(&mut self, _: &ShowPlan) -> Result<()> {
        Ok(())
    }

    fn visit_create_user_udf(&mut self, _: &CreateUserUDFPlan) -> Result<()> {
        Ok(())
    }

    fn visit_drop_user_udf(&mut self, _: &DropUserUDFPlan) -> Result<()> {
        Ok(())
    }

    fn visit_alter_user_udf(&mut self, _: &AlterUserUDFPlan) -> Result<()> {
        Ok(())
    }

    fn visit_remove_user_stage(&mut self, _: &RemoveUserStagePlan) -> Result<()> {
        Ok(())
    }

    fn visit_alter_table_cluster_key(&mut self, _: &AlterTableClusterKeyPlan) -> Result<()> {
        Ok(())
    }

    fn visit_drop_table_cluster_key(&mut self, _: &DropTableClusterKeyPlan) -> Result<()> {
        Ok(())
    }
}
