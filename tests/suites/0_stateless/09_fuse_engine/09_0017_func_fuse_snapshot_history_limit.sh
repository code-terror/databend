#!/usr/bin/env bash

CURDIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
. "$CURDIR"/../../../shell_env.sh


## setup
echo "create table t09_0017(c int)" | $MYSQL_CLIENT_CONNECT
echo "insert into t09_0017 values(1)" | $MYSQL_CLIENT_CONNECT
echo "insert into t09_0017 values(2)" | $MYSQL_CLIENT_CONNECT

## test limit clause
echo "limit 1 using new planner"
echo "set enable_planner_v2 = 1; select * from fuse_snapshot('default', 't09_0017') limit 1" | $MYSQL_CLIENT_CONNECT | wc -l | sed 's/ //g'
echo "limit 0 using new planner"
echo "set enable_planner_v2 = 1; select * from fuse_snapshot('default', 't09_0017') limit 0" | $MYSQL_CLIENT_CONNECT | wc -l | sed 's/ //g'

echo "limit 1 using old planner"
echo "set enable_planner_v2 = 0; select * from fuse_snapshot('default', 't09_0017') limit 1" | $MYSQL_CLIENT_CONNECT | wc -l | sed 's/ //g'
echo "limit 0 using old planner"
echo "set enable_planner_v2 = 0; select * from fuse_snapshot('default', 't09_0017') limit 0" | $MYSQL_CLIENT_CONNECT | wc -l | sed 's/ //g'

echo "limit 1 using stored procedure"
echo "call system\$fuse_snapshot('default', 't09_0017', 1)" | $MYSQL_CLIENT_CONNECT | wc -l | sed 's/ //g'
echo "limit 0 using stored procedure"
echo "call system\$fuse_snapshot('default', 't09_0017', 0)" | $MYSQL_CLIENT_CONNECT | wc -l | sed 's/ //g'
echo "limit 0 using stored procedure, limit as string"
echo "call system\$fuse_snapshot('default', 't09_0017', '0')" | $MYSQL_CLIENT_CONNECT | wc -l | sed 's/ //g'
echo "no limits using stored procedure, expects 2"
echo "call system\$fuse_snapshot('default', 't09_0017')" | $MYSQL_CLIENT_CONNECT | wc -l | sed 's/ //g'
echo "invalid limit, negative value"
echo "call system\$fuse_snapshot('default', 't09_0017', -1)" | $MYSQL_CLIENT_CONNECT 2>&1
echo "invalid limit, no numeric"
echo "call system\$fuse_snapshot('default', 't09_0017', "a")" | $MYSQL_CLIENT_CONNECT 2>&1
## Drop table.
echo "drop table  t09_0017" | $MYSQL_CLIENT_CONNECT
