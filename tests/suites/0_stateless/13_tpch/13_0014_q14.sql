set enable_planner_v2 = 1;
select
    TRUNCATE(100.00 * sum(case
                             when p_type like 'PROMO%'
                                 then l_extendedprice * (1 - l_discount)
                             else 0
            end) / sum(l_extendedprice * (1 - l_discount)), 5) as promo_revenue
from
    lineitem,
    part
where
        l_partkey = p_partkey
  and l_shipdate >= to_date('1995-09-01')
  and l_shipdate < addMonths(to_date('1995-09-01'), 1);