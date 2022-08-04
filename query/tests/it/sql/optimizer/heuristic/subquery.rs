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
use common_base::base::tokio;
use common_exception::Result;
use databend_query::sql::optimizer::DEFAULT_REWRITE_RULES;
use goldenfile::Mint;

use super::run_suites;
use super::Suite;
use crate::sql::optimizer::heuristic::run_test;
use crate::tests::create_query_context;

#[tokio::test]
pub async fn test_heuristic_optimizer_subquery() -> Result<()> {
    let mut mint = Mint::new("tests/it/sql/optimizer/heuristic/testdata/");
    let mut file = mint.new_goldenfile("subquery.test")?;

    let ctx = create_query_context().await?;

    let suites = vec![
        Suite {
            comment: "# Correlated subquery with joins".to_string(),
            query: "select t.number from numbers(1) as t, numbers(1) as t1 where t.number = (select count(*) from numbers(1) as t2, numbers(1) as t3 where t.number = t2.number)"
                .to_string(),
            rules: DEFAULT_REWRITE_RULES.clone(),
        },
        Suite {
            comment: "# Exists correlated subquery with joins".to_string(),
            query: "select t.number from numbers(1) as t where exists (select t1.number from numbers(1) as t1 where t.number = t1.number) or t.number > 1"
                .to_string(),
            rules: DEFAULT_REWRITE_RULES.clone(),
        },
        Suite {
            comment: "# Uncorrelated subquery".to_string(),
            query: "select t.number from numbers(1) as t where exists (select * from numbers(1) where number = 0)"
                .to_string(),
            rules: DEFAULT_REWRITE_RULES.clone(),
        },
        Suite {
            comment: "# Uncorrelated subquery".to_string(),
            query: "select t.number from numbers(1) as t where number = (select * from numbers(1) where number = 0)"
                .to_string(),
            rules: DEFAULT_REWRITE_RULES.clone(),
        },
        Suite {
            comment: "# Correlated subquery can be translated to SemiJoin".to_string(),
            query: "select t.number from numbers(1) as t where exists (select * from numbers(1) where number = t.number)"
                .to_string(),
            rules: DEFAULT_REWRITE_RULES.clone(),
        },
        Suite {
            comment: "# Correlated subquery can be translated to AntiJoin".to_string(),
            query: "select t.number from numbers(1) as t where not exists (select * from numbers(1) where number = t.number)"
                .to_string(),
            rules: DEFAULT_REWRITE_RULES.clone(),
        },
        Suite {
            comment: "".to_string(),
            query: "select * from numbers(1) as t where exists (select number as a from numbers(1) where number = t.number)"
                .to_string(),
            rules: DEFAULT_REWRITE_RULES.clone(),
        },
        Suite {
            comment: "# Exists with different kinds of predicate".to_string(),
            query: "select t.number from numbers(1) as t where exists (select * from numbers(1) where number = t.number and number = 0 and t.number < 10)"
                .to_string(),
            rules: DEFAULT_REWRITE_RULES.clone(),
        },
        Suite {
            comment: "# Exists with non-equi predicate".to_string(),
            query: "select t.number from numbers(1) as t where exists (select * from numbers(1) where number = t.number and t.number < number)"
                .to_string(),
            rules: DEFAULT_REWRITE_RULES.clone(),
        },
        Suite {
            comment: "# Exists project required columns".to_string(),
            query: "select t.number from numbers(1) as t where exists (select number as a, number as b, number as c from numbers(1) where number = t.number)".to_string(),
            rules: DEFAULT_REWRITE_RULES.clone(),
        },
        Suite {
            comment: "# Push down filter through CrossApply".to_string(),
            query: "select t.number from numbers(1) as t, numbers(1) as t1 where (select count(*) = 1 from numbers(1) where t.number = number) and t.number = t1.number"
                .to_string(),
            rules: DEFAULT_REWRITE_RULES.clone(),
        },
        Suite {
            comment: "# Semi join with other conditions".to_string(),
            query: "select t.number from numbers(1) as t where exists(select * from numbers(1) as t1 where t.number > t1.number) and not exists(select * from numbers(1) as t1 where t.number < t1.number)".to_string(),
            rules: DEFAULT_REWRITE_RULES.clone(),
        },
    ];

    run_suites(ctx, &mut file, &suites, run_test).await
}
