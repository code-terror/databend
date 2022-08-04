---
title: CREATE TABLE
description: Create a new table.
---

`CREATE TABLE` is the most complicated part of many Databases, you need to:
* Manually specify the engine
* Manually specify the indexes
* And even specify the data partitions or data shard
 
In Databend, you **don't need to specify any of these**, one of Databend's design goals is to make it easier to use.

## Syntax

### Create Table
```sql
CREATE [TRANSIENT] TABLE [IF NOT EXISTS] [db.]table_name
(
    <column_name> <data_type> [ NOT NULL | NULL] [ { DEFAULT <expr> }],
    <column_name> <data_type> [ NOT NULL | NULL] [ { DEFAULT <expr> }],
    ...
) [CLUSTER BY(<expr> [, <expr>, ...] )]

<data_type>:
  TINYINT
| SMALLINT 
| INT
| BIGINT
| FLOAT
| DOUBLE
| DATE
| TIMESTAMP 
| VARCHAR
| ARRAY
| OBJECT
| VARIANT
```

:::tip
Data type reference:
* [Boolean Data Types](../../../10-data-types/00-data-type-logical-types.md)
* [Numeric Data Types](../../../10-data-types/10-data-type-numeric-types.md)
* [Date & Time Data Types](../../../10-data-types/20-data-type-time-date-types.md)
* [String Data Types](../../../10-data-types/30-data-type-string-types.md)
* [Semi-structured Data Types](../../../10-data-types/40-data-type-semi-structured-types.md)
:::

For detailed information about the CLUSTER BY clause, see [SET CLUSTER KEY](../70-clusterkey/dml-set-cluster-key.md).

### CREATE TABLE ... LIKE

Creates an empty copy of an existing table, the new table automatically copies all column names, their data types, and their not-null constraints.

Syntax:
```sql
CREATE TABLE [IF NOT EXISTS] [db.]table_name
LIKE [db.]origin_table_name
```

### CREATE TABLE ... AS [SELECT query]

Creates a table and fills it with data computed by a SELECT command.

Syntax:
```sql
CREATE TABLE [IF NOT EXISTS] [db.]table_name
LIKE [db.]origin_table_name
AS SELECT query
```

### CREATE TRANSIENT TABLE ...
Creates a transient table. 

Transient tables are used to hold transitory data that does not require a data protection or recovery mechanism. Dataebend does not hold historical data for a transient table so you will not be able to query from a previous version of the transient table with the Time Travel feature, for example, the [AT](./../../20-query-syntax/dml-at.md) clause in the SELECT statement will not work for transient tables. Please note that you can still [drop](./20-ddl-drop-table.md) and [undrop](./21-ddl-undrop-table.md) a transient table.

Transient tables help save your storage expenses because they do not need extra space for historical data compared to non-transient tables. See [example](#create-transient-table-1) for detailed explanations.

Syntax:
```sql
CREATE TRANSIENT TABLE ...
```

## Column Nullable

By default, **all columns are not nullable(NOT NULL)**, if you want to specify a column default to `NULL`, please use:
```sql
CREATE TABLE [IF NOT EXISTS] [db.]table_name
(
    <column_name> <data_type> NULL,
     ...
)
```

Let check it out how difference the column is `NULL` or `NOT NULL`.

Create a table `t_not_null` which column with `NOT NULL`(Databend Column is `NOT NULL` by default):
```sql
CREATE TABLE t_not_null(a INT);
```

```sql
DESC t_not_null;
+-------+-------+------+---------+
| Field | Type  | Null | Default |
+-------+-------+------+---------+
| a     | Int32 | NO   | 0       |
+-------+-------+------+---------+
```

Create another table `t_null` column with `NULL`:
```sql
CREATE TABLE t_null(a INT NULL);
```

```sql
DESC t_null;
+-------+-------+------+---------+
| Field | Type  | Null | Default |
+-------+-------+------+---------+
| a     | Int32 | YES  | NULL    |
+-------+-------+------+---------+
```

## Default Values
```sql
DEFAULT <expression>
```
Specifies a default value inserted in the column if a value is not specified via an INSERT or CREATE TABLE AS SELECT statement.

For example:
```sql
CREATE TABLE t_default_value(a TINYINT UNSIGNED, b SMALLINT DEFAULT (a+3), c VARCHAR DEFAULT 'c');
```

Desc the `t_default_value` table:
```sql
DESC t_default_value;
+-------+--------+------+---------+
| Field | Type   | Null | Default |
+-------+--------+------+---------+
| a     | UInt8  | NO   | 0       |
| b     | Int16  | NO   | (a + 3) |
| c     | String | NO   | c       |
+-------+--------+------+---------+
```

Insert a value:
```sql
INSERT INTO T_default_value(a) VALUES(1);
```

Check the table values:
```sql
SELECT * FROM t_default_value;
+------+------+------+
| a    | b    | c    |
+------+------+------+
|    1 |    4 | c    |
+------+------+------+
```

## MySQL Compatibility

Databend’s syntax is difference from MySQL mainly in the data type and some specific index hints.

## Examples

### Create Table

```sql
CREATE TABLE test(a BIGINT UNSIGNED, b VARCHAR , c VARCHAR  DEFAULT concat(b, '-b'));
```

```sql
DESC test;
+-------+--------+------+---------------+
| Field | Type   | Null | Default       |
+-------+--------+------+---------------+
| a     | UInt64 | NO   | 0             |
| b     | String | NO   |               |
| c     | String | NO   | concat(b, -b) |
+-------+--------+------+---------------+
```

```sql
INSERT INTO test(a,b) VALUES(888, 'stars');
```

```sql
SELECT * FROM test;
+------+-------+---------+
| a    | b     | c       |
+------+-------+---------+
|  888 | stars | stars-b |
+------+-------+---------+
```

### Create Table Like Statement
```sql
CREATE TABLE test2 LIKE test;
```

```sql
DESC test2;
+-------+--------+------+---------------+
| Field | Type   | Null | Default       |
+-------+--------+------+---------------+
| a     | UInt64 | NO   | 0             |
| b     | String | NO   |               |
| c     | String | NO   | concat(b, -b) |
+-------+--------+------+---------------+
```

```sql
INSERT INTO test2(a,b) VALUES(888, 'stars');
```

```sql
SELECT * FROM test2;
+------+-------+---------+
| a    | b     | c       |
+------+-------+---------+
|  888 | stars | stars-b |
+------+-------+---------+
```

### Create Table As SELECT (CTAS) Statement

```sql
CREATE TABLE test3 AS SELECT * FROM test2;
```
```sql
DESC test3;
+-------+--------+------+---------------+
| Field | Type   | Null | Default       |
+-------+--------+------+---------------+
| a     | UInt64 | NO   | 0             |
| b     | String | NO   |               |
| c     | String | NO   | concat(b, -b) |
+-------+--------+------+---------------+
```

```sql
SELECT * FROM test3;
+------+-------+---------+
| a    | b     | c       |
+------+-------+---------+
|  888 | stars | stars-b |
+------+-------+---------+
```
### Create Transient Table

```sql
-- Create a transient table
CREATE TRANSIENT TABLE mytemp (c bigint);

-- Insert values
insert into mytemp values(1);
insert into mytemp values(2);
insert into mytemp values(3);

-- Only one snapshot is stored. This explains why the Time Travel feature does not work for transient tables.
select count(*) from fuse_snapshot('default', 'mytemp');
+---------+
| count() |
+---------+
|       1 | 
```