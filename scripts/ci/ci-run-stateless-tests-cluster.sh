#!/bin/bash
# Copyright 2020-2021 The Databend Authors.
# SPDX-License-Identifier: Apache-2.0.

echo "Starting Cluster databend-query"
./scripts/ci/deploy/databend-query-cluster-3-nodes.sh

SCRIPT_PATH="$(cd "$(dirname "$0")" >/dev/null 2>&1 && pwd)"
cd "$SCRIPT_PATH/../../tests" || exit

echo "Starting databend-test"
# Now Planner v2 not support cluster execute. So skip some tests that need enable planner v2.
./databend-test '^0[^4]_' --mode 'cluster' --run-dir 0_stateless --skip '02_0057_function_nullif' '02_0058_function_ifnull' '03_0004_select_order_by_db_table_col_v2' '02_0059_function_is_distinct_from' '06_0012_show_table_status_v2' '06_0004_show_tables_v2' '03_0009_select_exists'
