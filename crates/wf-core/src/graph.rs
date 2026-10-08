//! The dependency graph between modules.
//!
//! Pure structure: nodes, edges and the traversals a build needs from them. Nothing here
//! knows what a WebAssembly module is, where a source file lives, or what a runtime can
//! do — the graph only knows that one named thing requires another.

use std::fmt;

use crate::NameError;

/// Identity of a module inside a project.
///
/// The name the manifest gives it, so a report, a graph and a cache entry all agree on
/// what they are talking about.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ModuleId {
    name: String,
}

impl ModuleId {
    /// Builds an identity from the module's name.
    pub fn new(name: &str) -> Result<Self, NameError> {
        if name.is_empty() {
            return Err(NameError::new("module id"));
        }
        Ok(ModuleId {
            name: name.to_owned(),
        })
    }

    /// The name this module is known by.
    pub fn as_str(&self) -> &str {
        &self.name
    }
}

impl fmt::Display for ModuleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name)
    }
}

/// Why one module needs another.
///
/// The import that created the edge is kept, because a graph that only knows "a needs b"
/// cannot answer "which import of a is unsatisfied".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dependency {
    from: ModuleId,
    to: ModuleId,
    import: String,
}

impl Dependency {
    /// Records that `from` imports `import` from `to`.
    pub fn new(from: ModuleId, to: ModuleId, import: impl Into<String>) -> Self {
        Dependency {
            from,
            to,
            import: import.into(),
        }
    }

    /// The module that needs another.
    pub fn from(&self) -> &ModuleId {
        &self.from
    }

    /// The module being needed.
    pub fn to(&self) -> &ModuleId {
        &self.to
    }

    /// The import that created this edge, rendered `namespace.name`.
    pub fn import(&self) -> &str {
        &self.import
    }
}

/// Why the graph cannot answer a question about itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphError {
    /// An edge names a module that was never added.
    UnknownNode {
        /// The name that was not in the graph.
        name: String,
    },
    /// The graph contains a cycle, so no build order exists.
    Cycle {
        /// Every module in the loop, in order, with the first repeated at the end.
        path: Vec<ModuleId>,
    },
}

impl fmt::Display for GraphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GraphError::UnknownNode { name } => {
                write!(f, "module `{name}` is not part of this graph")
            }
            GraphError::Cycle { path } => {
                let names = path
                    .iter()
                    .map(ModuleId::as_str)
                    .collect::<Vec<_>>()
                    .join(" -> ");
                write!(f, "dependency cycle: {names}")
            }
        }
    }
}

impl std::error::Error for GraphError {}

/// How a node stands with respect to the search that reached it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Visit {
    Unvisited,
    InProgress,
    Done,
}

/// A directed graph of module dependencies.
///
/// Nodes and edges keep insertion order, and every traversal walks them in that order.
/// That is what makes the errors deterministic: the same inputs produce the same cycle
/// path and the same build order, on every run and every machine.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct DependencyGraph {
    nodes: Vec<ModuleId>,
    edges: Vec<Dependency>,
}

impl DependencyGraph {
    /// An empty graph.
    pub fn new() -> Self {
        DependencyGraph::default()
    }

    /// Adds a module. Adding one twice does nothing.
    pub fn add_node(&mut self, id: ModuleId) {
        if !self.nodes.contains(&id) {
            self.nodes.push(id);
        }
    }

    /// Whether a module was added.
    pub fn contains(&self, id: &ModuleId) -> bool {
        self.nodes.contains(id)
    }

    /// Every module, in the order they were added.
    pub fn nodes(&self) -> &[ModuleId] {
        &self.nodes
    }

    /// Records that `dependency.from` imports something from `dependency.to`.
    ///
    /// Refuses an edge to a module that was never added, rather than silently growing a
    /// graph whose nodes are only implied: an implied node would appear in no traversal
    /// and would make the graph disagree with the project.
    pub fn add_dependency(&mut self, dependency: Dependency) -> Result<(), GraphError> {
        for id in [dependency.from(), dependency.to()] {
            if !self.contains(id) {
                return Err(GraphError::UnknownNode {
                    name: id.as_str().to_owned(),
                });
            }
        }

        if !self.edges.contains(&dependency) {
            self.edges.push(dependency);
        }
        Ok(())
    }

    /// What a module depends on, in edge insertion order.
    pub fn dependencies(&self, id: &ModuleId) -> Vec<&Dependency> {
        self.edges.iter().filter(|edge| edge.from() == id).collect()
    }

    /// What depends on a module, in edge insertion order.
    pub fn dependents(&self, id: &ModuleId) -> Vec<&Dependency> {
        self.edges.iter().filter(|edge| edge.to() == id).collect()
    }

    /// Modules nothing depends on.
    ///
    /// The top of each chain: what a user would name as the thing to run.
    pub fn roots(&self) -> Vec<&ModuleId> {
        self.nodes
            .iter()
            .filter(|id| self.dependents(id).is_empty())
            .collect()
    }

    /// Modules that depend on nothing.
    ///
    /// The bottom of each chain, and where a build has to start.
    pub fn leaves(&self) -> Vec<&ModuleId> {
        self.nodes
            .iter()
            .filter(|id| self.dependencies(id).is_empty())
            .collect()
    }

    /// Reports the first cycle found, walking nodes in insertion order.
    ///
    /// Returns the whole path, because "a cycle exists" leaves the reader to find it.
    pub fn detect_cycle(&self) -> Option<GraphError> {
        let mut state = vec![Visit::Unvisited; self.nodes.len()];
        let mut path: Vec<usize> = Vec::new();

        for start in 0..self.nodes.len() {
            if state[start] != Visit::Unvisited {
                continue;
            }
            if let Some(cycle) = self.visit(start, &mut state, &mut path) {
                return Some(cycle);
            }
        }
        None
    }

    /// Depth-first search carrying the current path.
    fn visit(&self, node: usize, state: &mut [Visit], path: &mut Vec<usize>) -> Option<GraphError> {
        state[node] = Visit::InProgress;
        path.push(node);

        for dependency in self.dependencies(&self.nodes[node]) {
            let Some(target) = self.index_of(dependency.to()) else {
                continue;
            };

            match state[target] {
                // The edge closes a loop, so report where the loop started.
                Visit::InProgress => {
                    let start = path.iter().position(|&index| index == target).unwrap_or(0);
                    let mut loop_path: Vec<ModuleId> = path[start..]
                        .iter()
                        .map(|&index| self.nodes[index].clone())
                        .collect();
                    loop_path.push(self.nodes[target].clone());
                    return Some(GraphError::Cycle { path: loop_path });
                }
                Visit::Unvisited => {
                    if let Some(cycle) = self.visit(target, state, path) {
                        return Some(cycle);
                    }
                }
                Visit::Done => {}
            }
        }

        path.pop();
        state[node] = Visit::Done;
        None
    }

    /// Every module in an order where each one comes after everything it depends on.
    ///
    /// The order a build needs: by the time a module is reached, what it needs already
    /// has been. Modules with no order between them keep their insertion order.
    pub fn topological_order(&self) -> Result<Vec<&ModuleId>, GraphError> {
        // Kahn's algorithm counting *outgoing* edges, because an edge means "from needs
        // to": a module becomes ready once everything it needs is already ordered.
        //
        // Starting from nodes with nothing left to wait for means starting at the
        // leaves, which is where a build has to begin. `ready` stays in ascending node
        // order so that two modules with nothing between them are emitted in the order
        // they were added — the result is a function of the input, not of a tie-break
        // nobody chose.
        let mut pending: Vec<usize> = (0..self.nodes.len())
            .map(|index| self.dependencies(&self.nodes[index]).len())
            .collect();
        let mut ready: Vec<usize> = (0..self.nodes.len())
            .filter(|&index| pending[index] == 0)
            .collect();
        let mut order: Vec<&ModuleId> = Vec::with_capacity(self.nodes.len());

        while let Some(node) = ready.first().copied() {
            ready.remove(0);
            order.push(&self.nodes[node]);

            // Everything that needed this module has one fewer thing to wait for.
            for dependency in self.dependents(&self.nodes[node]) {
                let Some(dependent) = self.index_of(dependency.from()) else {
                    continue;
                };
                pending[dependent] -= 1;
                if pending[dependent] == 0 {
                    let position = ready.iter().position(|&existing| existing > dependent);
                    match position {
                        Some(at) => ready.insert(at, dependent),
                        None => ready.push(dependent),
                    }
                }
            }
        }

        if order.len() != self.nodes.len() {
            return Err(self.detect_cycle().unwrap_or_else(|| {
                // Unreachable for a well-formed graph, and if it is reached the message
                // still names what was left unordered rather than saying nothing.
                GraphError::Cycle {
                    path: self
                        .nodes
                        .iter()
                        .filter(|id| !order.contains(id))
                        .cloned()
                        .collect(),
                }
            }));
        }

        Ok(order)
    }

    /// Position of a module, if it is a node.
    fn index_of(&self, id: &ModuleId) -> Option<usize> {
        self.nodes.iter().position(|node| node == id)
    }
}
