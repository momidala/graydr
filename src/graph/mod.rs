//! Dependency graph for graydr resource ordering.
//!
//! Edge convention: add_edge(dependency, dependent) — dependency must appear BEFORE dependent
//! in topological order. Edge A→B means "A must come before B".

use petgraph::graph::DiGraph;
use petgraph::algo::{toposort, kosaraju_scc};
use std::collections::HashMap;
use std::sync::Arc;
use crate::resolver::error::ResolveError;
use crate::ast::span::Span;

/// Directed dependency graph over resource names.
///
/// Nodes are resource names (Strings). An edge from `dependency` to `dependent`
/// means `dependency` must be assembled before `dependent`.
pub struct DependencyGraph {
    graph: DiGraph<String, ()>,
    name_to_idx: HashMap<String, petgraph::graph::NodeIndex>,
}

impl DependencyGraph {
    /// Build a new graph with the given resource names as nodes (no edges yet).
    pub fn new(resource_names: &[String]) -> Self {
        let mut graph = DiGraph::new();
        let mut name_to_idx = HashMap::new();
        for name in resource_names {
            let idx = graph.add_node(name.clone());
            name_to_idx.insert(name.clone(), idx);
        }
        Self { graph, name_to_idx }
    }

    /// Add an explicit edge from `dependency` to `dependent` (dependency before dependent).
    ///
    /// Returns `UnknownDependency` if `dependency` is not in the graph.
    pub fn add_explicit_edge(
        &mut self,
        dependency: &str,
        dependent: &str,
        resource_span: &Span,
    ) -> Result<(), ResolveError> {
        let dep_idx = self.name_to_idx.get(dependency).copied().ok_or_else(|| {
            ResolveError::UnknownDependency {
                span: resource_span.clone(),
                resource: dependent.to_string(),
                unknown: dependency.to_string(),
            }
        })?;
        let ant_idx = self.name_to_idx.get(dependent).copied().ok_or_else(|| {
            ResolveError::UnknownDependency {
                span: resource_span.clone(),
                resource: dependency.to_string(),
                unknown: dependent.to_string(),
            }
        })?;
        self.graph.add_edge(dep_idx, ant_idx, ());
        Ok(())
    }

    /// Add implicit edges inferred from output variable references.
    ///
    /// `output_var_map` maps variable name → producer resource name.
    /// For each `var` in `consumer_input_vars`, if `output_var_map` contains `var`,
    /// add an edge from the producer to `consumer_name`.
    /// Silently skips variables not found in `output_var_map`.
    pub fn add_implicit_edges(
        &mut self,
        output_var_map: &HashMap<String, String>,
        consumer_name: &str,
        consumer_input_vars: &[String],
    ) {
        for var_name in consumer_input_vars {
            if let Some(producer) = output_var_map.get(var_name) {
                if let (Some(&producer_idx), Some(&consumer_idx)) = (
                    self.name_to_idx.get(producer),
                    self.name_to_idx.get(consumer_name),
                ) {
                    self.graph.add_edge(producer_idx, consumer_idx, ());
                }
            }
        }
    }

    /// Return resources in topological order (dependencies before dependents).
    ///
    /// Returns `Err(CircularDependency)` if a cycle is detected.
    pub fn topo_order(&self) -> Result<Vec<String>, ResolveError> {
        match toposort(&self.graph, None) {
            Ok(order) => {
                let names = order.iter().map(|&idx| self.graph[idx].clone()).collect();
                Ok(names)
            }
            Err(_cycle) => {
                let sccs = kosaraju_scc(&self.graph);
                let members: Vec<String> = sccs
                    .into_iter()
                    .filter(|scc| scc.len() > 1)
                    .flat_map(|scc| scc.into_iter().map(|idx| self.graph[idx].clone()))
                    .collect();
                let span = Span {
                    file: Arc::from(""),
                    start_line: 0,
                    start_col: 0,
                    end_line: 0,
                    end_col: 0,
                };
                Err(ResolveError::CircularDependency { span, members })
            }
        }
    }
}

/// Map a logical region name to a provider-specific region name using a mapping table.
///
/// If `logical` is not in `mapping`, returns `logical` unchanged.
pub fn resolve_region(logical: &str, mapping: &HashMap<String, String>) -> String {
    mapping.get(logical).cloned().unwrap_or_else(|| logical.to_string())
}

/// A group of resources sharing the same provider and region, ordered for assembly.
pub struct AssemblyGroup {
    pub provider: String,
    pub region: String,
    pub resources_in_order: Vec<String>,
}

/// Group resources by provider+region and order each group by topological dependency.
///
/// `topo_order` is the globally sorted list of resource names.
/// `provider_map` maps resource name → provider string.
/// `region_map` maps resource name → logical region string.
/// `region_mapping` maps logical region → provider-specific region.
pub fn assemble_by_provider_region(
    topo_order: &[String],
    provider_map: &HashMap<String, String>,
    region_map: &HashMap<String, String>,
    region_mapping: &HashMap<String, String>,
) -> Vec<AssemblyGroup> {
    use std::collections::BTreeMap;

    let mut groups: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();

    for resource_name in topo_order {
        let provider = match provider_map.get(resource_name) {
            Some(p) => p.clone(),
            None => continue,
        };
        let logical_region = match region_map.get(resource_name) {
            Some(r) => r.clone(),
            None => continue,
        };
        let physical_region = resolve_region(&logical_region, region_mapping);
        groups
            .entry((provider, physical_region))
            .or_default()
            .push(resource_name.clone());
    }

    groups
        .into_iter()
        .map(|((provider, region), resources_in_order)| AssemblyGroup {
            provider,
            region,
            resources_in_order,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use crate::ast::span::Span;

    fn dummy_span() -> Span {
        Span {
            file: Arc::from("test.gtpl"),
            start_line: 1,
            start_col: 1,
            end_line: 1,
            end_col: 10,
        }
    }

    /// B depends on A → A must appear before B in topological order.
    #[test]
    fn test_topo_order_respects_dependency() {
        let names = vec!["a".to_string(), "b".to_string()];
        let mut g = DependencyGraph::new(&names);
        g.add_explicit_edge("a", "b", &dummy_span()).unwrap();
        let order = g.topo_order().unwrap();
        let pos_a = order.iter().position(|x| x == "a").unwrap();
        let pos_b = order.iter().position(|x| x == "b").unwrap();
        assert!(pos_a < pos_b, "a must appear before b in topo order");
    }

    /// A→B, B→A cycle → CircularDependency error naming both "a" and "b".
    #[test]
    fn test_cycle_names_all_members() {
        let names = vec!["a".to_string(), "b".to_string()];
        let mut g = DependencyGraph::new(&names);
        g.add_explicit_edge("a", "b", &dummy_span()).unwrap();
        g.add_explicit_edge("b", "a", &dummy_span()).unwrap();
        let err = g.topo_order().unwrap_err();
        match err {
            ResolveError::CircularDependency { members, .. } => {
                assert!(members.contains(&"a".to_string()), "members must include 'a'");
                assert!(members.contains(&"b".to_string()), "members must include 'b'");
            }
            other => panic!("expected CircularDependency, got {:?}", other),
        }
    }

    /// Explicit depends_on edge: A declared before B, explicit edge A→B → A appears first.
    #[test]
    fn test_explicit_depends_on_edge() {
        let names = vec!["a".to_string(), "b".to_string()];
        let mut g = DependencyGraph::new(&names);
        g.add_explicit_edge("a", "b", &dummy_span()).unwrap();
        let order = g.topo_order().unwrap();
        let pos_a = order.iter().position(|x| x == "a").unwrap();
        let pos_b = order.iter().position(|x| x == "b").unwrap();
        assert!(pos_a < pos_b);
    }

    /// Implicit output ref edge: output_var_map says "vpc_id" comes from "network";
    /// consumer "db" uses "vpc_id" in inputs → edge network→db (network before db).
    #[test]
    fn test_implicit_output_ref_edge() {
        let names = vec!["network".to_string(), "db".to_string()];
        let mut g = DependencyGraph::new(&names);
        let mut output_var_map = HashMap::new();
        output_var_map.insert("vpc_id".to_string(), "network".to_string());
        g.add_implicit_edges(&output_var_map, "db", &["vpc_id".to_string()]);
        let order = g.topo_order().unwrap();
        let pos_network = order.iter().position(|x| x == "network").unwrap();
        let pos_db = order.iter().position(|x| x == "db").unwrap();
        assert!(pos_network < pos_db, "network must come before db");
    }

    /// Same dependency from both explicit and implicit sources → deduped; same topo order.
    #[test]
    fn test_explicit_and_implicit_edge_dedup() {
        let names = vec!["a".to_string(), "b".to_string()];
        let mut g = DependencyGraph::new(&names);
        // Add explicit edge a→b
        g.add_explicit_edge("a", "b", &dummy_span()).unwrap();
        // Add implicit edge also a→b (same direction)
        let mut output_var_map = HashMap::new();
        output_var_map.insert("some_var".to_string(), "a".to_string());
        g.add_implicit_edges(&output_var_map, "b", &["some_var".to_string()]);
        // Must not cycle or panic; a still before b
        let order = g.topo_order().unwrap();
        let pos_a = order.iter().position(|x| x == "a").unwrap();
        let pos_b = order.iter().position(|x| x == "b").unwrap();
        assert!(pos_a < pos_b);
    }

    /// depends_on names resource not in graph → UnknownDependency error.
    #[test]
    fn test_unknown_depends_on_error() {
        let names = vec!["a".to_string()];
        let mut g = DependencyGraph::new(&names);
        let err = g.add_explicit_edge("nonexistent", "a", &dummy_span()).unwrap_err();
        match err {
            ResolveError::UnknownDependency { unknown, .. } => {
                assert_eq!(unknown, "nonexistent");
            }
            other => panic!("expected UnknownDependency, got {:?}", other),
        }
    }

    /// Two resources with same provider+region → one AssemblyGroup.
    #[test]
    fn test_assembly_groups_by_provider_region() {
        let names = vec!["a".to_string(), "b".to_string()];
        let _g = DependencyGraph::new(&names);
        let topo = vec!["a".to_string(), "b".to_string()];
        let mut provider_map = HashMap::new();
        provider_map.insert("a".to_string(), "aws".to_string());
        provider_map.insert("b".to_string(), "aws".to_string());
        let mut region_map = HashMap::new();
        region_map.insert("a".to_string(), "us-east".to_string());
        region_map.insert("b".to_string(), "us-east".to_string());
        let region_mapping = HashMap::new();
        let groups = assemble_by_provider_region(&topo, &provider_map, &region_map, &region_mapping);
        assert_eq!(groups.len(), 1, "same provider+region → one group");
        assert_eq!(groups[0].provider, "aws");
    }

    /// Two resources with different regions → two AssemblyGroups.
    #[test]
    fn test_assembly_separate_provider_region() {
        let topo = vec!["a".to_string(), "b".to_string()];
        let mut provider_map = HashMap::new();
        provider_map.insert("a".to_string(), "aws".to_string());
        provider_map.insert("b".to_string(), "aws".to_string());
        let mut region_map = HashMap::new();
        region_map.insert("a".to_string(), "us-east".to_string());
        region_map.insert("b".to_string(), "eu-west".to_string());
        let region_mapping = HashMap::new();
        let groups = assemble_by_provider_region(&topo, &provider_map, &region_map, &region_mapping);
        assert_eq!(groups.len(), 2, "different regions → two groups");
    }

    /// Within a group, dependency resource appears before dependent resource.
    #[test]
    fn test_assembly_order_is_topo_sorted() {
        // topo_order already has a before b
        let topo = vec!["a".to_string(), "b".to_string()];
        let mut provider_map = HashMap::new();
        provider_map.insert("a".to_string(), "aws".to_string());
        provider_map.insert("b".to_string(), "aws".to_string());
        let mut region_map = HashMap::new();
        region_map.insert("a".to_string(), "us-east".to_string());
        region_map.insert("b".to_string(), "us-east".to_string());
        let region_mapping = HashMap::new();
        let groups = assemble_by_provider_region(&topo, &provider_map, &region_map, &region_mapping);
        assert_eq!(groups.len(), 1);
        let pos_a = groups[0].resources_in_order.iter().position(|x| x == "a").unwrap();
        let pos_b = groups[0].resources_in_order.iter().position(|x| x == "b").unwrap();
        assert!(pos_a < pos_b, "a must appear before b within the group");
    }

    /// Logical "us-east" maps to "us-east-1" via mapping table.
    #[test]
    fn test_region_mapping_lookup() {
        let mut mapping = HashMap::new();
        mapping.insert("us-east".to_string(), "us-east-1".to_string());
        assert_eq!(resolve_region("us-east", &mapping), "us-east-1");
    }

    /// Logical "eu-central" not in mapping → returned as-is.
    #[test]
    fn test_region_mapping_fallback() {
        let mapping = HashMap::new();
        assert_eq!(resolve_region("eu-central", &mapping), "eu-central");
    }
}
