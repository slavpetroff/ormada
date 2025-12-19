use ormada::fields::Condition;
use ormada::query::{
    Aggregated, Aggregation, CanExecute, CanFilter, CanOrder, CanPaginate, FilterExpr, FilterOp,
    Filtered, Fresh, OrderDirection, Ordered, Paginated, QueryOp, QueryPlan, QuerySetState,
    QueryState, SoftDeleteMode, Q,
};
use sea_orm::sea_query::{ColumnRef, Expr};
use sea_orm::DbErr;

fn is_unique_violation(err: &DbErr) -> bool {
    let msg = err.to_string().to_lowercase();
    msg.contains("unique constraint")
        || msg.contains("duplicate key")
        || msg.contains("duplicate entry")
        || msg.contains("unique violation")
        || msg.contains("constraint failed")
}

#[test]
fn test_is_unique_violation_sqlite() {
    let err = DbErr::Custom("UNIQUE constraint failed: users.email".to_string());
    assert!(is_unique_violation(&err));
}

#[test]
fn test_is_unique_violation_postgres() {
    let err = DbErr::Custom("duplicate key value violates unique constraint".to_string());
    assert!(is_unique_violation(&err));
}

#[test]
fn test_is_unique_violation_mysql() {
    let err = DbErr::Custom("Duplicate entry '123' for key 'PRIMARY'".to_string());
    assert!(is_unique_violation(&err));
}

#[test]
fn test_is_unique_violation_negative() {
    let err = DbErr::Custom("Connection refused".to_string());
    assert!(!is_unique_violation(&err));
}

#[test]
fn test_q_all_constructor() {
    let q = Q::all();
    let _: Condition = q.into();
}

#[test]
fn test_q_any_constructor() {
    let q = Q::any();
    let _: Condition = q.into();
}

#[test]
fn test_q_not_transformation() {
    let q = Q::all().not();
    let _: Condition = q.into();
}

#[test]
fn test_q_add_chaining() {
    let q = Q::all().add(Expr::value(true)).add(Expr::value(false));
    let _: Condition = q.into();
}

#[test]
fn test_aggregation_count_all() {
    let agg = Aggregation::count_all();
    assert!(matches!(agg, Aggregation::CountAll));
}

#[test]
fn test_aggregation_enum_is_debug() {
    let agg = Aggregation::count_all();
    let debug_str = format!("{:?}", agg);
    assert!(debug_str.contains("CountAll"));
}

#[test]
fn test_aggregation_enum_is_clone() {
    let agg = Aggregation::count_all();
    let cloned = agg.clone();
    assert_eq!(agg, cloned);
}

#[test]
fn test_aggregation_enum_is_eq() {
    let agg1 = Aggregation::count_all();
    let agg2 = Aggregation::count_all();
    assert_eq!(agg1, agg2);
}

#[test]
fn test_aggregation_pattern_matching() {
    let aggregations = vec![Aggregation::count_all()];

    for agg in aggregations {
        match agg {
            Aggregation::CountAll => {}
            Aggregation::Count(_) => panic!("Expected CountAll"),
            Aggregation::Sum(_) => panic!("Expected CountAll"),
            Aggregation::Avg(_) => panic!("Expected CountAll"),
            Aggregation::Max(_) => panic!("Expected CountAll"),
            Aggregation::Min(_) => panic!("Expected CountAll"),
        }
    }
}

#[test]
fn test_filter_expr_and() {
    let filter = FilterExpr::And(vec![]);
    assert!(filter.is_and());
    assert!(!filter.is_or());
    assert!(!matches!(filter, FilterExpr::Not(_)));
}

#[test]
fn test_filter_expr_or() {
    let filter = FilterExpr::Or(vec![]);
    assert!(filter.is_or());
    assert!(!filter.is_and());
    assert!(!matches!(filter, FilterExpr::Not(_)));
}

#[test]
fn test_filter_expr_not() {
    let inner = FilterExpr::And(vec![]);
    let filter = FilterExpr::Not(Box::new(inner));
    assert!(matches!(filter, FilterExpr::Not(_)));
    assert!(!filter.is_and());
    assert!(!filter.is_or());
}

#[test]
fn test_filter_expr_is_debug() {
    let filter = FilterExpr::And(vec![FilterExpr::Or(vec![])]);
    let debug_str = format!("{:?}", filter);
    assert!(debug_str.contains("And"));
    assert!(debug_str.contains("Or"));
}

#[test]
fn test_filter_expr_is_clone() {
    let filter = FilterExpr::And(vec![FilterExpr::Or(vec![])]);
    let cloned = filter.clone();
    assert!(cloned.is_and());
}

#[test]
fn test_filter_expr_nested_structure() {
    let ab = FilterExpr::And(vec![
        FilterExpr::Raw(Expr::value(true).into()),
        FilterExpr::Raw(Expr::value(false).into()),
    ]);
    let cd = FilterExpr::And(vec![
        FilterExpr::Raw(Expr::value(true).into()),
        FilterExpr::Raw(Expr::value(true).into()),
    ]);
    let combined = FilterExpr::Or(vec![ab, cd]);

    match combined {
        FilterExpr::Or(children) => {
            assert_eq!(children.len(), 2);
            for child in children {
                assert!(child.is_and());
            }
        }
        _ => panic!("Expected Or"),
    }
}

#[test]
fn test_filter_expr_into_condition() {
    let filter = FilterExpr::And(vec![FilterExpr::Raw(Expr::value(true).into())]);
    let _condition: Condition = filter.into();
}

#[test]
fn test_filter_expr_pattern_matching() {
    let filters = vec![
        FilterExpr::And(vec![]),
        FilterExpr::Or(vec![]),
        FilterExpr::Not(Box::new(FilterExpr::And(vec![]))),
        FilterExpr::Raw(Expr::value(true).into()),
    ];

    for filter in filters {
        match &filter {
            FilterExpr::And(_) => assert!(filter.is_and()),
            FilterExpr::Or(_) => assert!(filter.is_or()),
            FilterExpr::Not(_) => assert!(matches!(filter, FilterExpr::Not(_))),
            FilterExpr::Typed { .. } => assert!(filter.is_typed()),
            FilterExpr::Raw(_) => {
                assert!(filter.is_raw());
                assert!(!filter.is_and());
                assert!(!filter.is_or());
                assert!(!matches!(filter, FilterExpr::Not(_)));
            }
        }
    }
}

#[test]
fn test_query_op_limit() {
    let op = QueryOp::limit(10);
    assert!(op.is_limit());
    assert!(!op.is_filter());
    assert!(!op.is_order_by());
}

#[test]
fn test_query_op_offset() {
    let op = QueryOp::offset(20);
    assert!(op.is_offset());
    assert!(!op.is_limit());
}

#[test]
fn test_query_op_distinct() {
    let op = QueryOp::distinct();
    assert!(op.is_distinct());
    assert!(!op.is_filter());
}

#[test]
fn test_query_op_filter() {
    let op = QueryOp::filter(FilterExpr::And(vec![]));
    assert!(op.is_filter());
    assert!(!op.is_exclude());
}

#[test]
fn test_query_op_exclude() {
    let op = QueryOp::exclude(FilterExpr::And(vec![]));
    assert!(op.is_exclude());
    assert!(!op.is_filter());
}

#[test]
fn test_query_op_is_debug() {
    let op = QueryOp::Limit(10);
    let debug_str = format!("{:?}", op);
    assert!(debug_str.contains("Limit"));
    assert!(debug_str.contains("10"));
}

#[test]
fn test_query_op_is_clone() {
    let op = QueryOp::Limit(10);
    let cloned = op.clone();
    assert!(cloned.is_limit());
}

#[test]
fn test_order_direction() {
    assert_eq!(OrderDirection::Asc, OrderDirection::Asc);
    assert_ne!(OrderDirection::Asc, OrderDirection::Desc);
}

#[test]
fn test_query_plan_new() {
    let plan = QueryPlan::new();
    assert!(plan.is_empty());
    assert_eq!(plan.len(), 0);
}

#[test]
fn test_query_plan_push() {
    let mut plan = QueryPlan::new();
    plan.push(QueryOp::Limit(10));
    plan.push(QueryOp::Offset(5));
    assert_eq!(plan.len(), 2);
    assert!(!plan.is_empty());
}

#[test]
fn test_query_plan_operations() {
    let mut plan = QueryPlan::new();
    plan.push(QueryOp::Limit(10));
    plan.push(QueryOp::Distinct);

    let ops = plan.operations();
    assert_eq!(ops.len(), 2);
    assert!(ops[0].is_limit());
    assert!(ops[1].is_distinct());
}

#[test]
fn test_query_plan_has_filters() {
    let mut plan = QueryPlan::new();
    assert!(!plan.has_filters());

    plan.push(QueryOp::filter(FilterExpr::And(vec![])));
    assert!(plan.has_filters());
}

#[test]
fn test_query_plan_has_ordering() {
    let mut plan = QueryPlan::new();
    assert!(!plan.has_ordering());

    plan.push(QueryOp::OrderBy {
        column: ColumnRef::Asterisk(None),
        direction: OrderDirection::Asc,
    });
    assert!(plan.has_ordering());
}

#[test]
fn test_query_plan_has_limit() {
    let mut plan = QueryPlan::new();
    assert!(!plan.has_limit());

    plan.push(QueryOp::Limit(10));
    assert!(plan.has_limit());
}

#[test]
fn test_query_plan_get_limit() {
    let mut plan = QueryPlan::new();
    assert_eq!(plan.get_limit(), None);

    plan.push(QueryOp::Limit(25));
    assert_eq!(plan.get_limit(), Some(25));
}

#[test]
fn test_query_plan_get_offset() {
    let mut plan = QueryPlan::new();
    assert_eq!(plan.get_offset(), None);

    plan.push(QueryOp::Offset(100));
    assert_eq!(plan.get_offset(), Some(100));
}

#[test]
fn test_query_plan_filters() {
    let mut plan = QueryPlan::new();
    plan.push(QueryOp::filter(FilterExpr::And(vec![])));
    plan.push(QueryOp::Limit(10));
    plan.push(QueryOp::filter(FilterExpr::Or(vec![])));

    let filters = plan.filters();
    assert_eq!(filters.len(), 2);
}

#[test]
fn test_query_plan_iter() {
    let mut plan = QueryPlan::new();
    plan.push(QueryOp::Limit(10));
    plan.push(QueryOp::Offset(5));

    let count = plan.iter().count();
    assert_eq!(count, 2);
}

#[test]
fn test_query_plan_is_debug() {
    let mut plan = QueryPlan::new();
    plan.push(QueryOp::Limit(10));
    let debug_str = format!("{:?}", plan);
    assert!(debug_str.contains("QueryPlan"));
    assert!(debug_str.contains("Limit"));
}

#[test]
fn test_query_plan_is_clone() {
    let mut plan = QueryPlan::new();
    plan.push(QueryOp::Limit(10));
    let cloned = plan.clone();
    assert_eq!(cloned.len(), 1);
}

#[test]
fn test_query_plan_pattern_matching() {
    let mut plan = QueryPlan::new();
    plan.push(QueryOp::Filter(FilterExpr::And(vec![])));
    plan.push(QueryOp::Limit(10));
    plan.push(QueryOp::Distinct);

    for op in plan.iter() {
        match op {
            QueryOp::Filter(_) => assert!(op.is_filter()),
            QueryOp::Limit(n) => assert_eq!(*n, 10),
            QueryOp::Distinct => assert!(op.is_distinct()),
            _ => {}
        }
    }
}

#[test]
fn test_query_op_soft_delete_variant() {
    let op = QueryOp::SoftDelete(SoftDeleteMode::OnlyDeleted);

    match op {
        QueryOp::SoftDelete(mode) => {
            assert_eq!(mode, SoftDeleteMode::OnlyDeleted);
        }
        _ => panic!("Expected SoftDelete"),
    }
}

#[test]
fn test_query_op_annotate_variant() {
    let op = QueryOp::Annotate {
        alias: "total".to_string(),
        aggregation: Aggregation::CountAll,
    };

    match op {
        QueryOp::Annotate { alias, aggregation } => {
            assert_eq!(alias, "total");
            assert!(matches!(aggregation, Aggregation::CountAll));
        }
        _ => panic!("Expected Annotate"),
    }
}

#[test]
fn test_query_op_order_by_variant() {
    let op = QueryOp::OrderBy {
        column: ColumnRef::Asterisk(None),
        direction: OrderDirection::Desc,
    };

    assert!(op.is_order_by());
    match op {
        QueryOp::OrderBy { direction, .. } => {
            assert_eq!(direction, OrderDirection::Desc);
        }
        _ => panic!("Expected OrderBy"),
    }
}

#[test]
fn test_query_op_group_by_variant() {
    let op = QueryOp::GroupBy(ColumnRef::Asterisk(None));

    match op {
        QueryOp::GroupBy(_) => {}
        _ => panic!("Expected GroupBy"),
    }
}

#[test]
fn test_query_op_exclude_variant() {
    let op = QueryOp::Exclude(FilterExpr::And(vec![]));
    assert!(op.is_exclude());
    assert!(!op.is_filter());
}

#[test]
fn test_filter_op_sql_operators() {
    assert_eq!(FilterOp::Eq.sql_operator(), "=");
    assert_eq!(FilterOp::Ne.sql_operator(), "!=");
    assert_eq!(FilterOp::Lt.sql_operator(), "<");
    assert_eq!(FilterOp::Lte.sql_operator(), "<=");
    assert_eq!(FilterOp::Gt.sql_operator(), ">");
    assert_eq!(FilterOp::Gte.sql_operator(), ">=");
    assert_eq!(FilterOp::Like.sql_operator(), "LIKE");
    assert_eq!(FilterOp::NotLike.sql_operator(), "NOT LIKE");
    assert_eq!(FilterOp::In.sql_operator(), "IN");
    assert_eq!(FilterOp::NotIn.sql_operator(), "NOT IN");
    assert_eq!(FilterOp::IsNull.sql_operator(), "IS NULL");
    assert_eq!(FilterOp::IsNotNull.sql_operator(), "IS NOT NULL");
    assert_eq!(FilterOp::Between.sql_operator(), "BETWEEN");
    assert_eq!(FilterOp::Contains.sql_operator(), "LIKE");
    assert_eq!(FilterOp::StartsWith.sql_operator(), "LIKE");
    assert_eq!(FilterOp::EndsWith.sql_operator(), "LIKE");
}

#[test]
fn test_filter_op_is_comparison() {
    assert!(FilterOp::Eq.is_comparison());
    assert!(FilterOp::Ne.is_comparison());
    assert!(FilterOp::Lt.is_comparison());
    assert!(FilterOp::Lte.is_comparison());
    assert!(FilterOp::Gt.is_comparison());
    assert!(FilterOp::Gte.is_comparison());
    assert!(!FilterOp::Like.is_comparison());
    assert!(!FilterOp::IsNull.is_comparison());
}

#[test]
fn test_filter_op_is_string_op() {
    assert!(FilterOp::Like.is_string_op());
    assert!(FilterOp::NotLike.is_string_op());
    assert!(FilterOp::Contains.is_string_op());
    assert!(FilterOp::StartsWith.is_string_op());
    assert!(FilterOp::EndsWith.is_string_op());
    assert!(!FilterOp::Eq.is_string_op());
    assert!(!FilterOp::IsNull.is_string_op());
}

#[test]
fn test_filter_op_is_null_check() {
    assert!(FilterOp::IsNull.is_null_check());
    assert!(FilterOp::IsNotNull.is_null_check());
    assert!(!FilterOp::Eq.is_null_check());
    assert!(!FilterOp::Like.is_null_check());
}

#[test]
fn test_filter_op_equality() {
    assert_eq!(FilterOp::Eq, FilterOp::Eq);
    assert_ne!(FilterOp::Eq, FilterOp::Ne);
}

#[test]
fn test_query_state_default() {
    let state = QueryState::default();
    assert!(state.is_fresh());
    assert!(!state.is_filtered());
    assert!(!state.is_ordered());
    assert!(!state.is_paginated());
    assert!(!state.is_aggregated());
    assert!(!state.is_executed());
}

#[test]
fn test_query_state_transitions() {
    let mut state = QueryState::Fresh;

    state.filter();
    assert!(state.is_filtered());
    assert_eq!(state, QueryState::Filtered);

    state = QueryState::Fresh;
    state.filter();
    state.order();
    assert!(state.is_ordered());
    assert_eq!(state, QueryState::Ordered);

    state.paginate();
    assert!(state.is_paginated());
    assert_eq!(state, QueryState::Paginated);

    state = QueryState::Fresh;
    state.aggregate();
    assert!(state.is_aggregated());
    assert_eq!(state, QueryState::Aggregated);

    state.execute();
    assert!(state.is_executed());
    assert_eq!(state, QueryState::Executed);
}

#[test]
fn test_query_state_pattern_matching() {
    let states = vec![
        QueryState::Fresh,
        QueryState::Filtered,
        QueryState::Ordered,
        QueryState::Paginated,
        QueryState::Aggregated,
        QueryState::Executed,
    ];

    for state in states {
        match state {
            QueryState::Fresh => assert!(state.is_fresh()),
            QueryState::Filtered => assert!(state.is_filtered()),
            QueryState::Ordered => assert!(state.is_ordered()),
            QueryState::Paginated => assert!(state.is_paginated()),
            QueryState::Aggregated => assert!(state.is_aggregated()),
            QueryState::Executed => assert!(state.is_executed()),
        }
    }
}

#[test]
fn test_query_state_clone_copy() {
    let state = QueryState::Filtered;
    let cloned = state.clone();
    let copied = state;

    assert_eq!(state, cloned);
    assert_eq!(state, copied);
}

#[test]
fn test_filter_expr_typed_is_typed() {
    let filter = FilterExpr::Typed {
        column: "price".to_string(),
        op: FilterOp::Eq,
        value_repr: "100".to_string(),
        expr: Expr::value(100).into(),
    };
    assert!(filter.is_typed());
    assert!(!filter.is_raw());
    assert!(!filter.is_and());
    assert!(!filter.is_or());
    assert!(!matches!(filter, FilterExpr::Not(_)));
}

#[test]
fn test_filter_expr_get_op() {
    let filter = FilterExpr::Typed {
        column: "price".to_string(),
        op: FilterOp::Lt,
        value_repr: "50".to_string(),
        expr: Expr::value(50).into(),
    };
    assert_eq!(filter.get_op(), Some(&FilterOp::Lt));

    let raw = FilterExpr::Raw(Expr::value(true).into());
    assert_eq!(raw.get_op(), None);
}

#[test]
fn test_filter_expr_get_column() {
    let filter = FilterExpr::Typed {
        column: "author_id".to_string(),
        op: FilterOp::Eq,
        value_repr: "1".to_string(),
        expr: Expr::value(1).into(),
    };
    assert_eq!(filter.get_column(), Some("author_id"));

    let and = FilterExpr::And(vec![]);
    assert_eq!(and.get_column(), None);
}

#[test]
fn test_filter_expr_get_value_repr() {
    let filter = FilterExpr::Typed {
        column: "name".to_string(),
        op: FilterOp::Contains,
        value_repr: "test".to_string(),
        expr: Expr::value("test").into(),
    };
    assert_eq!(filter.get_value_repr(), Some("test"));

    let or = FilterExpr::Or(vec![]);
    assert_eq!(or.get_value_repr(), None);
}

#[test]
fn test_filter_expr_typed_into_condition() {
    let filter = FilterExpr::Typed {
        column: "status".to_string(),
        op: FilterOp::Eq,
        value_repr: "active".to_string(),
        expr: Expr::value("active").into(),
    };
    let _condition: Condition = filter.into();
}

#[test]
fn test_filter_expr_pattern_matching_with_typed() {
    let filters = vec![
        FilterExpr::And(vec![]),
        FilterExpr::Or(vec![]),
        FilterExpr::Not(Box::new(FilterExpr::And(vec![]))),
        FilterExpr::Typed {
            column: "id".to_string(),
            op: FilterOp::Gt,
            value_repr: "10".to_string(),
            expr: Expr::value(10),
        },
        FilterExpr::Raw(Expr::value(true)),
    ];

    for filter in filters {
        match &filter {
            FilterExpr::And(_) => assert!(filter.is_and()),
            FilterExpr::Or(_) => assert!(filter.is_or()),
            FilterExpr::Not(_) => assert!(matches!(filter, FilterExpr::Not(_))),
            FilterExpr::Typed { op, .. } => {
                assert!(filter.is_typed());
                assert_eq!(filter.get_op(), Some(op));
            }
            FilterExpr::Raw(_) => assert!(filter.is_raw()),
        }
    }
}

#[test]
fn test_typestate_fresh_default() {
    fn assert_state<S: QuerySetState>() {}
    let _fresh: Fresh = Fresh;
    assert_state::<Fresh>();
}

#[test]
fn test_typestate_filtered_default() {
    fn assert_state<S: QuerySetState>() {}
    let _filtered: Filtered = Filtered;
    assert_state::<Filtered>();
}

#[test]
fn test_typestate_ordered_default() {
    fn assert_state<S: QuerySetState>() {}
    let _ordered: Ordered = Ordered;
    assert_state::<Ordered>();
}

#[test]
fn test_typestate_paginated_default() {
    fn assert_state<S: QuerySetState>() {}
    let _paginated: Paginated = Paginated;
    assert_state::<Paginated>();
}

#[test]
fn test_typestate_aggregated_default() {
    fn assert_state<S: QuerySetState>() {}
    let _aggregated: Aggregated = Aggregated;
    assert_state::<Aggregated>();
}

#[test]
fn test_can_filter_trait() {
    fn assert_can_filter<S: CanFilter>() {}
    assert_can_filter::<Fresh>();
    assert_can_filter::<Filtered>();
}

#[test]
fn test_can_order_trait() {
    fn assert_can_order<S: CanOrder>() {}
    assert_can_order::<Fresh>();
    assert_can_order::<Filtered>();
    assert_can_order::<Ordered>();
}

#[test]
fn test_can_paginate_trait() {
    fn assert_can_paginate<S: CanPaginate>() {}
    assert_can_paginate::<Fresh>();
    assert_can_paginate::<Filtered>();
    assert_can_paginate::<Ordered>();
    assert_can_paginate::<Paginated>();
}

#[test]
fn test_can_execute_trait() {
    fn assert_can_execute<S: CanExecute>() {}
    assert_can_execute::<Fresh>();
    assert_can_execute::<Filtered>();
    assert_can_execute::<Ordered>();
    assert_can_execute::<Paginated>();
    assert_can_execute::<Aggregated>();
}

#[test]
fn test_typestate_clone_copy() {
    let fresh = Fresh;
    let cloned = fresh;
    assert_eq!(fresh, cloned);

    let filtered = Filtered;
    let copied = filtered;
    assert_eq!(filtered, copied);
}

#[test]
fn test_typestate_debug() {
    assert!(format!("{Fresh:?}").contains("Fresh"));
    assert!(format!("{Filtered:?}").contains("Filtered"));
    assert!(format!("{Ordered:?}").contains("Ordered"));
    assert!(format!("{Paginated:?}").contains("Paginated"));
    assert!(format!("{Aggregated:?}").contains("Aggregated"));
}
