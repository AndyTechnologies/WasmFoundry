// Tests for the dependency graph.
//
// The graph is pure: nodes and edges, no WebAssembly and no files. Everything asserted
// here is about what a build needs from a module graph — what depends on what, what to
// build first, and whether the graph is acyclic at all.

use wf_core::{Dependency, DependencyGraph, GraphError, ModuleId};

/// Builds an id, panicking only on an empty name.
fn id(name: &str) -> ModuleId {
    ModuleId::new(name).expect("non-empty name")
}

/// Builds the edge `from --import--> to`.
fn edge(from: &str, import: &str, to: &str) -> Dependency {
    Dependency::new(id(from), id(to), import)
}

/// A → B → C, where the arrow means "depends on".
fn chain() -> DependencyGraph {
    let mut graph = DependencyGraph::new();
    for name in ["a", "b", "c"] {
        graph.add_node(id(name));
    }
    graph
        .add_dependency(edge("a", "b.log", "b"))
        .expect("a and b exist");
    graph
        .add_dependency(edge("b", "c.log", "c"))
        .expect("b and c exist");
    graph
}

#[test]
fn a_module_id_is_the_name_the_manifest_uses() {
    assert_eq!(id("app").as_str(), "app");
    assert!(ModuleId::new("").is_err(), "an empty id is not a module");
}

#[test]
fn nodes_can_be_added_and_are_not_duplicated() {
    let mut graph = DependencyGraph::new();
    graph.add_node(id("a"));
    graph.add_node(id("a"));

    assert!(graph.contains(&id("a")));
    assert_eq!(
        graph.nodes().len(),
        1,
        "adding twice must not create two nodes"
    );
}

#[test]
fn an_edge_points_from_the_consumer_to_what_it_consumes() {
    let dependency = edge("a", "b.log", "b");
    assert_eq!(dependency.from(), &id("a"));
    assert_eq!(dependency.to(), &id("b"));
    assert_eq!(dependency.import(), "b.log");
}

#[test]
fn dependencies_are_what_a_module_needs() {
    let graph = chain();

    let a_dependencies = graph.dependencies(&id("a"));
    assert_eq!(a_dependencies.len(), 1);
    assert_eq!(a_dependencies[0].to(), &id("b"));
    assert_eq!(
        graph.dependencies(&id("c")).len(),
        0,
        "the foundation imports nothing"
    );
}

#[test]
fn dependents_are_who_needs_a_module() {
    let graph = chain();

    let b_dependents = graph.dependents(&id("b"));
    assert_eq!(b_dependents.len(), 1);
    assert_eq!(b_dependents[0].from(), &id("a"));
    assert_eq!(graph.dependents(&id("a")).len(), 0);
}

#[test]
fn roots_are_the_top_of_the_chain_and_leaves_are_the_bottom() {
    let graph = chain();

    // A root has nothing depending on it; a leaf depends on nothing. The distinction
    // matters because a build starts from the leaves.
    assert_eq!(
        graph
            .roots()
            .iter()
            .map(|id| id.as_str())
            .collect::<Vec<_>>(),
        vec!["a"],
        "a is the only module nobody imports"
    );
    assert_eq!(
        graph
            .leaves()
            .iter()
            .map(|id| id.as_str())
            .collect::<Vec<_>>(),
        vec!["c"],
        "c is the only module that imports nothing"
    );
}

#[test]
fn topological_order_lists_a_module_after_its_dependencies() {
    let order = chain()
        .topological_order()
        .expect("a chain is acyclic")
        .into_iter()
        .map(|id| id.as_str().to_owned())
        .collect::<Vec<_>>();

    assert_eq!(order, vec!["c", "b", "a"]);
    assert_eq!(
        order.iter().position(|n| n == "c").expect("c is present"),
        0,
        "a module must come after everything it depends on, or the order cannot be built"
    );
}

#[test]
fn topological_order_is_deterministic_for_the_same_graph() {
    // Bound to a variable: the order borrows the graph, and a temporary would drop it.
    let graph = chain();
    let first = graph.topological_order().expect("acyclic");
    let second = graph.topological_order().expect("acyclic");
    assert_eq!(first, second, "the order must be a function of the input");

    // A graph whose nodes were added in another order but say the same thing must not
    // silently disagree about which modules exist.
    let mut other = DependencyGraph::new();
    for name in ["a", "b", "c"] {
        other.add_node(id(name));
    }
    other
        .add_dependency(edge("a", "b.log", "b"))
        .expect("a and b exist");
    other
        .add_dependency(edge("b", "c.log", "c"))
        .expect("b and c exist");
    assert_eq!(first, other.topological_order().expect("acyclic"));
}

#[test]
fn an_empty_graph_orders_to_nothing_without_failing() {
    let graph = DependencyGraph::new();
    assert!(graph.topological_order().expect("vacuous order").is_empty());
    assert!(graph.detect_cycle().is_none());
    assert!(graph.roots().is_empty());
    assert!(graph.leaves().is_empty());
}

#[test]
fn independent_modules_are_all_roots_and_all_leaves() {
    let mut graph = DependencyGraph::new();
    graph.add_node(id("a"));
    graph.add_node(id("b"));

    assert_eq!(graph.roots().len(), 2);
    assert_eq!(graph.leaves().len(), 2);
    assert_eq!(graph.topological_order().expect("acyclic").len(), 2);
}

#[test]
fn adding_the_same_dependency_twice_keeps_one_edge() {
    let mut graph = DependencyGraph::new();
    graph.add_node(id("a"));
    graph.add_node(id("b"));
    graph
        .add_dependency(edge("a", "b.log", "b"))
        .expect("first");
    graph
        .add_dependency(edge("a", "b.log", "b"))
        .expect("second");

    assert_eq!(graph.dependencies(&id("a")).len(), 1);
}

#[test]
fn an_edge_to_an_unknown_node_is_refused() {
    let mut graph = DependencyGraph::new();
    graph.add_node(id("a"));

    let error = graph
        .add_dependency(edge("a", "b.log", "b"))
        .expect_err("b does not exist");
    assert!(
        matches!(error, GraphError::UnknownNode { ref name } if name == "b"),
        "it must name the node it could not find: {error:?}"
    );
    assert_eq!(
        graph.dependencies(&id("a")).len(),
        0,
        "nothing may be recorded"
    );
}

#[test]
fn a_two_module_cycle_is_reported_with_both_ends() {
    let mut graph = DependencyGraph::new();
    graph.add_node(id("a"));
    graph.add_node(id("b"));
    graph
        .add_dependency(edge("a", "b.log", "b"))
        .expect("edge 1");
    graph
        .add_dependency(edge("b", "a.log", "a"))
        .expect("edge 2");

    let error = graph.topological_order().expect_err("a cycle has no order");
    let message = error.to_string();
    assert!(message.contains("a"), "{message}");
    assert!(message.contains("b"), "{message}");
}

#[test]
fn a_three_module_cycle_names_the_whole_path() {
    // The plan is explicit: reporting "cycle detected" alone is not enough. Whoever
    // reads this has to see which modules form the loop.
    let mut graph = DependencyGraph::new();
    for name in ["a", "b", "c"] {
        graph.add_node(id(name));
    }
    graph
        .add_dependency(edge("a", "b.log", "b"))
        .expect("edge 1");
    graph
        .add_dependency(edge("b", "c.log", "c"))
        .expect("edge 2");
    graph
        .add_dependency(edge("c", "a.log", "a"))
        .expect("edge 3");

    let error = graph.detect_cycle().expect("there is a cycle");
    let message = error.to_string();

    assert!(
        message.contains("a") && message.contains("b") && message.contains("c"),
        "every member must appear: {message}"
    );
    assert!(
        message.starts_with("dependency cycle"),
        "the message must name the kind of failure: {message}"
    );
}

#[test]
fn a_self_dependency_is_a_cycle_of_one() {
    let mut graph = DependencyGraph::new();
    graph.add_node(id("a"));
    graph
        .add_dependency(edge("a", "a.self", "a"))
        .expect("self edge");

    let error = graph.detect_cycle().expect("a module cannot import itself");
    let message = error.to_string();
    assert!(message.contains("a"), "{message}");
}

#[test]
fn cycle_detection_is_deterministic() {
    let mut graph = DependencyGraph::new();
    for name in ["a", "b", "c"] {
        graph.add_node(id(name));
    }
    graph
        .add_dependency(edge("a", "b.log", "b"))
        .expect("edge 1");
    graph
        .add_dependency(edge("b", "c.log", "c"))
        .expect("edge 2");
    graph
        .add_dependency(edge("c", "a.log", "a"))
        .expect("edge 3");

    let first = graph.detect_cycle().expect("cycle").to_string();
    let second = graph.detect_cycle().expect("cycle").to_string();
    assert_eq!(
        first, second,
        "the same graph must always report the same path"
    );
}

#[test]
fn a_cycle_makes_the_order_fail_instead_of_looking_valid() {
    let mut graph = DependencyGraph::new();
    graph.add_node(id("a"));
    graph.add_node(id("b"));
    graph
        .add_dependency(edge("a", "b.log", "b"))
        .expect("edge 1");
    graph
        .add_dependency(edge("b", "a.log", "a"))
        .expect("edge 2");

    let error = graph
        .topological_order()
        .expect_err("no valid order exists");
    assert!(matches!(error, GraphError::Cycle { .. }), "{error:?}");
}
