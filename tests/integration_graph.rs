// Integration tests for Phase 4 dependency graph end-to-end scenarios.
// These tests construct TemplateDefinition AST nodes inline (same pattern as integration_dispatch.rs).

use std::collections::HashMap;
use std::sync::Arc;
use graydr::graph::{DependencyGraph, assemble_by_provider_region};
use graydr::resolver::error::ResolveError;
use graydr::ast::span::Span;

fn test_span() -> Span {
    Span { file: Arc::from("test.gtpl"), start_line: 1, start_col: 1, end_line: 1, end_col: 1 }
}

#[test]
fn test_topo_order_end_to_end() {
    // Resources in declaration order (db first, as if it appeared first in .gtpl file)
    let resource_names = vec!["db".to_string(), "network".to_string()];
    let mut graph = DependencyGraph::new(&resource_names);

    // Implicit edge: network produces "vpc_id" output; db consumes "vpc_id" as input variable
    let output_var_map: HashMap<String, String> = [("vpc_id".to_string(), "network".to_string())].into();
    graph.add_implicit_edges(&output_var_map, "db", &["vpc_id".to_string()]);

    // Topo order must put network before db
    let order = graph.topo_order().expect("should not be cyclic");
    let network_pos = order.iter().position(|r| r == "network").unwrap();
    let db_pos = order.iter().position(|r| r == "db").unwrap();
    assert!(network_pos < db_pos, "network must come before db, got order: {order:?}");

    // Assembly grouping: both resources share provider=aws, region=us-east-1
    let provider_map: HashMap<String, String> = [
        ("network".to_string(), "aws".to_string()),
        ("db".to_string(), "aws".to_string()),
    ].into();
    let region_map: HashMap<String, String> = [
        ("network".to_string(), "us-east".to_string()),
        ("db".to_string(), "us-east".to_string()),
    ].into();
    let region_mapping: HashMap<String, String> = [
        ("us-east".to_string(), "us-east-1".to_string()),
    ].into();

    let groups = assemble_by_provider_region(&order, &provider_map, &region_map, &region_mapping);
    assert_eq!(groups.len(), 1, "should be one group for aws/us-east-1");
    assert_eq!(groups[0].provider, "aws");
    assert_eq!(groups[0].region, "us-east-1");
    assert_eq!(groups[0].resources_in_order, vec!["network", "db"],
        "assembly must respect topo order: network before db");
}

#[test]
fn test_cycle_error_end_to_end() {
    let resource_names = vec!["a".to_string(), "b".to_string()];
    let mut graph = DependencyGraph::new(&resource_names);

    // "a depends_on b" → b is dependency, a is dependent → add_explicit_edge("b", "a", ...)
    graph.add_explicit_edge("b", "a", &test_span()).expect("edge b→a valid");
    // "b depends_on a" → a is dependency, b is dependent → add_explicit_edge("a", "b", ...)
    graph.add_explicit_edge("a", "b", &test_span()).expect("edge a→b valid");

    let result = graph.topo_order();
    assert!(result.is_err(), "cyclic graph must return Err");
    match result.unwrap_err() {
        ResolveError::CircularDependency { members, .. } => {
            assert!(members.contains(&"a".to_string()), "cycle error must name 'a', got: {members:?}");
            assert!(members.contains(&"b".to_string()), "cycle error must name 'b', got: {members:?}");
        }
        other => panic!("expected CircularDependency, got: {other:?}"),
    }
}
