set enable_planner_v2 = 1;

select '====SELECT_FROM_NUMBERS====';
select * from numbers(10);

select '====ALIAS====';
select number as a, number + 1 as b from numbers(1);
select number as a, number + 1 as b from numbers(1) group by a, number order by number;

select '====SCALAR_EXPRESSION====';
select extract(day from to_date('2022-05-13'));
select date_trunc(month, to_date('2022-07-07'));

-- Comparison expressions
select '====COMPARISON====';
select * from numbers(10) where number between 1 and 9 and number > 2 and number < 8 and number is not null and number = 5 and number >= 5 and number <= 5;

-- Cast expression
select '====CAST====';
select * from numbers(10) where cast(number as string) = '5';
select * from numbers(10) where try_cast(number as string) = '5';

-- Binary operator
select '====BINARY_OPERATOR====';
select (number + 1 - 2) * 3 / 4 from numbers(1);

-- Functions
select '====FUNCTIONS====';
select sin(cos(number)) from numbers(1);

-- In list
select '====IN_LIST====';
select * from numbers(5) where number in (1, 3);

-- Map access
select '====MAP_ACCESS====';
select parse_json('{"k1": [0, 1, 2]}'):k1[2];
select parse_json('{"k1": [0, 1, 2]}')['k1'][2];
select parse_json('{"k1": {"k2": [0, 1, 2]}}'):k1.k2[2];

-- Aggregator operator
select '====AGGREGATOR====';
create table t(a int, b int);
insert into t values(1, 2), (2, 3), (3, 4);
select sum(a) + 1 from t group by a;
select sum(a) from t group by a;
select sum(a) from t;
select count(a) from t group by a;
select count(a) from t;
select count() from t;
select count() from t group by a;
select count(1) from t;
select count(1) from t group by a;
select count(*) from t;
select sum(a) from t group by a having sum(a) > 1;
select sum(a+1) from t group by a+1 having sum(a+1) = 2;
select sum(a+1) from t group by a+1, b having sum(a+1) > 3;
drop table t;

select 1, sum(number) from numbers_mt(1000000);
select count(*) = count(1) from numbers(1000);
select count(1) from numbers(1000);
select sum(3) from numbers(1000);
select count(null) from numbers(1000);

SELECT max(number) FROM numbers_mt (10) where number > 99999999998;
SELECT max(number) FROM numbers_mt (10) where number > 2;

SELECT number%3 as c1, number%2 as c2 FROM numbers_mt(10000) where number > 2 group by number%3, number%2 order by c1,c2;
SELECT number%3 as c1 FROM numbers_mt(10) where number > 2 group by number%3 order by c1;

CREATE TABLE t(a UInt64 null, b UInt32 null, c UInt32) Engine = Fuse;
INSERT INTO t(a,b, c)  SELECT if (number % 3 = 1, null, number) as a, number + 3 as b, number + 4 as c FROM numbers(10);
-- nullable(u8)
SELECT a%3 as a1, count(1) as ct from t GROUP BY a1 ORDER BY a1,ct;

-- nullable(u8), nullable(u8)
SELECT a%2 as a1, a%3 as a2, count(0) as ct FROM t GROUP BY a1, a2 ORDER BY a1, a2;

-- nullable(u8), u64
SELECT a%2 as a1, to_uint64(c % 3) as c1, count(0) as ct FROM t GROUP BY a1, c1 ORDER BY a1, c1, ct;
-- u64, nullable(u8)
SELECT to_uint64(c % 3) as c1, a%2 as a1, count(0) as ct FROM t GROUP BY a1, c1 ORDER BY a1, c1, ct;

select number%2 as b from numbers(5) group by number % 2 having count(*) = 3 and sum(number) > 5;

select count(*) from numbers(5) group by number % 2 having number % 2 + 1 = 2;

select number, sum(number) from numbers(10) group by 1, number having sum(number) = 5;

SELECT arg_min(user_name, salary)  FROM (SELECT sum(number) AS salary, number%3 AS user_name FROM numbers_mt(10000) GROUP BY user_name);

-- aggregator combinator
-- distinct
select sum_distinct(number) from ( select number % 100 as number from numbers(100000));
select count_distinct(number) from ( select number % 100 as number from numbers(100000));
select sum_distinct(number) /  count_distinct(number) = avg_distinct(number) from ( select number % 100 as number from numbers(100000));

-- if
select sum_if(number, number >= 100000 - 1) from numbers(100000);
select sum_if(number, number > 100) /  count_if(number,  number > 100) = avg_if(number,  number > 100) from numbers(100000);
select count_if(number, number>9) from numbers(10);

-- boolean
select sum(number > 314) from numbers(1000);
select avg(number > 314) from numbers(1000);

drop table t;

select '====Having alias====';
select number as a from numbers(1) group by a having a = 0;
select number+1 as a from numbers(1) group by a having a = 1;

-- Inner join
select '====INNER_JOIN====';
create table t(a int);
insert into t values(1),(2),(3);
create table t1(b float);
insert into t1 values(1.0),(2.0),(3.0);
create table t2(c smallint unsigned null);
insert into t2 values(1),(2),(null);

select * from t inner join t1 on t.a = t1.b;
select * from t inner join t2 on t.a = t2.c;
select * from t inner join t2 on t.a = t2.c + 1;
select * from t inner join t2 on t.a = t2.c + 1 and t.a - 1 = t2.c;
select * from t1 inner join t on t.a = t1.b;
select * from t2 inner join t on t.a = t2.c;
select * from t2 inner join t on t.a = t2.c + 1;
select * from t2 inner join t on t.a = t2.c + 1 and t.a - 1 = t2.c;
select count(*) from numbers(1000) as t inner join numbers(1000) as t1 on t.number = t1.number;

select t.number from numbers(10000) as t inner join numbers(1000) as t1 on t.number % 1000 = t1.number order by number limit 5;

-- order by
select '====ORDER_BY====';
SELECT number%3 as c1, number%2 as c2 FROM numbers_mt (10) order by c1 desc, c2 asc;
SELECT number, null from numbers(3) order by number desc;
SELECT number%3 as c1, number%2 as c2 FROM numbers_mt (10) order by c1, number desc;
SELECT SUM(number) AS s FROM numbers_mt(10) GROUP BY number ORDER BY s;
create table t3(a int, b int);
insert into t3 values(1,2),(2,3);
select * from t3 order by 2 desc;
select a from t3 order by 1 desc;
drop table t;
drop table t1;
drop table t2;
drop table t3;

-- Select without from
select '====SELECT_WITHOUT_FROM====';
select 1 + 1;
select to_int(8);
select 'new_planner';

-- limit
select '=== Test limit ===';
select number from numbers(100) order by number asc limit 10;
select '==================';
select number*2 as number from numbers(100) order by number limit 10;
select '=== Test limit n, m ===';
select number from numbers(100) order by number asc limit 9, 11;
select '==================';
select number-2 as number from numbers(100) order by number asc limit 10, 10;
select '=== Test limit with offset ===';
select number from numbers(100) order by number asc limit 10 offset 10;
select '==============================';
select number/2 as number from numbers(100) order by number asc limit 10 offset 10;
select '=== Test offset ===';
select number from numbers(10) order by number asc offset 5;
select '===================';
select number+number as number from numbers(10) order by number asc offset 5;
select number from numbers(10000) order by number limit 1;

-- Memory engine
select '====Memory Table====';
drop table if exists temp;
create table temp (a int) engine = Memory;
insert into temp values (1);
select a from temp;
drop table temp;


-- CASE WHEN
select '=== Test CASE-WHEN ===';
select count_if(a = '1'), count_if(a = '2'), count_if(a = '3'), count_if(a is null) from (
	SELECT (CASE WHEN number % 4 = 1 THEN '1' WHEN number % 4 = 2 THEN '2' WHEN number % 4 = 3 THEN '3' END) as a FROM numbers(100)
);
select case when number >= 2 then 'ge2' WHEN number >= 1 then 'ge1' ELSE null end from numbers(3);
select case when 1 = 3 then null when 1 = 2 then 20.0 when 1 = 1 then 1 ELSE null END;

select COALESCE(NULL, NULL, 1, 2);
-- subquery in from
select '=== Test Subquery In From ===';
create table t(a int, b int);
insert into t values(1, 2),(2, 3);
select t1.a from (select * from t) as t1;
SELECT a,b,count() from (SELECT cast((number%4) AS bigint) as a, cast((number%20) AS bigint) as b from numbers(100)) group by a,b order by a,b limit 3 ;
drop table t;

select '====Context Function====';
use default;
select database();

-- distinct
select '==== Distinct =====';
SELECT DISTINCT * FROM numbers(3) ORDER BY  number;
SELECT DISTINCT 1 FROM numbers(3);
SELECT DISTINCT (number %3) as c FROM numbers(1000) ORDER BY c;
SELECT DISTINCT count(number %3) as c FROM numbers(10)  group by number % 3 ORDER BY c;

-- Inner join with using
select '===Inner Join with Using===';
drop table if exists t1;
create table t1(a int, b int);
insert into t1 values(7, 8), (3, 4), (5, 6);
drop table if exists t2;
create table t2(a int, d int);
insert into t2 values(1, 2), (3, 4), (5, 6);
select * from t1 join t2 using(a);
select t1.a from t1 join t2 using(a);
select t2.d from t1 join t2 using(a);
select * from t1 natural join t2;
drop table t1;
drop table t2;

-- Join: right table with duplicate build keys
select '===Inner Join with duplicate keys===';
create table t1(a int, b int);
insert into t1 values(1, 2), (1, 3), (2, 4);
create table t2(c int, d int);
insert into t2 values(1, 2), (2, 6);
select * from t2 inner join t1 on t1.a = t2.c;
drop table t1;
drop table t2;

-- trim function
select '===Trim Function===';
select trim(leading ' ' from '      abc');
select trim(leading ' ' from '');
select trim(leading 'ab' from 'abab');
select trim(leading 'ab' from 'abc');
select trim(trailing ' ' from 'abc    ');
select trim(trailing ' ' from '');
select trim(trailing 'ab' from 'abab');
select trim(trailing 'ab' from 'cab');
select trim(both 'ab' from 'abab');
select trim(both 'ab' from 'abcab');
select trim(' abc ');

-- Select Array Literal
select '===Array Literal===';
select [1, 2, 3];
select [];
select [[1, 2, 3],[1, 2, 3]];

select '====Correlated Subquery====';
select * from numbers(10) as t where exists (select * from numbers(2) as t1 where t.number = t1.number);
select (select number from numbers(10) as t1 where t.number = t1.number) from numbers(10) as t order by number;

-- explain
select '===Explain===';
create table t1(a int, b int);
create table t2(a int, b int);
explain select t1.a from t1 where a > 0;
explain select * from t1, t2 where (t1.a = t2.a and t1.a > 3) or (t1.a = t2.a and t2.a > 5 and t1.a > 1);
explain select * from t1, t2 where (t1.a = t2.a and t1.a > 3) or (t1.a = t2.a);
select '===Explain Pipeline===';
explain pipeline select t1.a from t1 join t2 on t1.a = t2.a;
drop table t1;
drop table t2;
-- position function
select '===Position Function===';
SELECT POSITION('bar' IN 'foobarbar');
SELECT POSITION('xbar' IN 'foobar');
drop table if exists t;
create table t (a varchar);
insert into t values ('foo');
select POSITION('o' IN t.a) from t;
drop table t;

select '====Tuple====';
select ('field', number) from numbers(5);

select '====View====';
drop view if exists temp;
create view temp as select number from numbers(1);
select number from temp;
drop view temp;

-- cross join
select '====Cross Join====';
create table t1(a int, b int);
create table t2(c int, d int);
insert into t1 values(1, 2), (2, 3), (3 ,4);
insert into t2 values(2,2), (3, 5), (7 ,8);
select * from t1, t2;
drop table t1;
drop table t2;

-- test error code hint

select 3 as a, 4 as a;
-- udf
select '====UDF====';
CREATE FUNCTION a_plus_3 AS (a) -> a+3;
SELECT a_plus_3(2);
CREATE FUNCTION cal1 AS (a,b,c,d,e) -> a + c * (e / b) - d;
SELECT cal1(1, 2, 3, 4, 6);
CREATE FUNCTION notnull1 AS (p) -> not(is_null(p));
SELECT notnull1(null);
SELECT notnull1('null');

drop function a_plus_3;
drop function cal1;
drop function notnull1;

--set operator
select '====Intersect Distinct===';
create table t1(a int, b int);
create table t2(c int, d int);
insert into t1 values(1, 2), (2, 3), (3 ,4), (2, 3);
insert into t2 values(2,2), (3, 5), (7 ,8), (2, 3), (3, 4);
select * from t1 intersect select * from t2;
select '====Except Distinct===';
select * from t1 except select * from t2;
drop table t1;
drop table t2;

--outer join
select '====Outer Join====';
create table t1(a int, b int);
create table t2(c int, d int);
insert into t1 values(1, 2), (3 ,4), (7, 8);
insert into t2 values(1, 4), (2, 3), (6, 8);
select * from t1 right join t2 on t1.a = t2.c;
select * from t1 left join t2 on t1.a = t2.c;

select * from t1 left outer join t2 on t1.a = t2.c and t1.a > 3 order by a,b,c,d;
select * from t1 left outer join t2 on t1.a = t2.c and t2.c > 4 order by a,b,c,d;
select * from t1 left outer join t2 on t2.c > 4 and t1.a > 3 order by a,b,c,d;
select * from t1 left outer join t2 on t1.a > 3 order by a,b,c,d;
select * from t1 left outer join t2 on t2.c > 4 order by a,b,c,d;
select * from t1 left outer join t2 on t1.a > t2.c order by a,b,c,d;
drop table t1;
drop table t2;

-- NULL
select '====NULL====';
create table n( a int null, b int null) ;
insert into n select  if (number % 3, null, number), if (number % 2, null, number) from numbers(10);
select a + b, a and b, a - b, a or b as c from n order by c nulls first;
drop table n;

-- Subquery SemiJoin and AntiJoin
select * from numbers(5) as t where exists (select * from numbers(3) where number = t.number);
select * from numbers(5) as t where not exists (select * from numbers(3) where number = t.number);

select * from numbers(5) as t where exists (select number as a from numbers(3) where number = t.number and number > 0 and t.number < 2);
select * from numbers(5) as t where exists (select * from numbers(3) where number > t.number);

-- (Not)IN/ANY/SOME/ALL Subquery
create table t1(a int, b int);
create table t2(a int, b int);
insert into t1 values(1, 2), (2, 3);
insert into t2 values(3, 4), (2, 3);
select * from t1 where t1.a not in (select t2.a from t2);
select * from t1 where t1.a in (select t2.a from t2);
select * from t1 where t1.a = any (select t2.a from t2);
select * from t1 where t1.a = some (select t2.a from t2);
select * from t1 where t1.a != all (select t2.a from t2);
select * from t1 where t1.a >= any (select t2.a from t2);
select * from t1 where t1.a = all (select t2.a from t2);
set enable_planner_v2 = 0;
create table t3 as select *  from numbers(10000);
insert into t3 values(1);
set enable_planner_v2 = 1;
select count(*) from numbers(10000) as t4 where t4.number in (select t3.number from t3);
drop table t1;
drop table t2;
drop table t3;

select '====Database====';
select database(), currentDatabase(), current_database();
select '====User====';
select user(), currentuser(), current_user();

-- Query has keyword
SELECT '====WITH_KEYWORD====';
SELECT database, table, name, type, default_kind as default_type, default_expression, comment FROM system.columns  WHERE database LIKE 'system'  AND table LIKE 'settings' ORDER BY name;
set enable_planner_v2 = 0;

