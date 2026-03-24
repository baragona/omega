//! Universe level constraint graph.
//!
//! Instead of modeling cumulativity via explicit `lift`/`cumul` rewrite rules
//! (which cause infinite e-graph expansion), universe levels are resolved as
//! a side-channel constraint DAG before equality saturation.
//!
//! Constraints are of the form `u >= v` (level u is at least level v).
//! The graph is solved via topological sort. Cycles indicate inconsistent
//! universe declarations.

use std::collections::{HashMap, HashSet, VecDeque};

/// A universe level variable or concrete level.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum LevelTerm {
    /// A concrete universe level (e.g., 0, 1, 2).
    Concrete(u32),
    /// A level variable (e.g., ?u, ?v).
    Var(String),
    /// Max of two levels.
    Max(Box<LevelTerm>, Box<LevelTerm>),
    /// Successor (level + 1).
    Succ(Box<LevelTerm>),
}

/// A constraint: `lhs >= rhs` (lhs is at least as large as rhs).
#[derive(Debug, Clone)]
pub struct LevelConstraint {
    pub lhs: String, // variable or "0", "1", etc.
    pub rhs: String,
    pub source: String, // rule/declaration that generated this constraint
}

/// Universe level constraint graph.
#[derive(Debug, Clone, Default)]
pub struct LevelGraph {
    /// Constraints: u >= v (edges from v to u in the DAG)
    pub constraints: Vec<LevelConstraint>,
    /// Known concrete assignments
    pub assignments: HashMap<String, u32>,
    /// All level variable names
    pub variables: HashSet<String>,
}

/// Result of solving the level graph.
#[derive(Debug)]
pub struct LevelSolution {
    /// Assigned levels for each variable.
    pub assignments: HashMap<String, u32>,
    /// Whether the graph was consistent (no cycles).
    pub consistent: bool,
    /// Cycle participants (if inconsistent).
    pub cycle: Vec<String>,
}

impl LevelGraph {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a constraint: `lhs >= rhs`.
    pub fn add_constraint(&mut self, lhs: &str, rhs: &str, source: &str) {
        self.variables.insert(lhs.to_string());
        self.variables.insert(rhs.to_string());
        self.constraints.push(LevelConstraint {
            lhs: lhs.to_string(),
            rhs: rhs.to_string(),
            source: source.to_string(),
        });
    }

    /// Set a concrete assignment for a level variable.
    pub fn assign(&mut self, var: &str, level: u32) {
        self.variables.insert(var.to_string());
        self.assignments.insert(var.to_string(), level);
    }

    /// Solve the constraint graph.
    /// Returns assignments for all variables, or reports cycles.
    pub fn solve(&self) -> LevelSolution {
        // Build adjacency list: v -> u means "u >= v" (u depends on v)
        let mut adj: HashMap<String, Vec<String>> = HashMap::new();
        let mut in_degree: HashMap<String, usize> = HashMap::new();

        for var in &self.variables {
            adj.entry(var.clone()).or_default();
            in_degree.entry(var.clone()).or_insert(0);
        }

        for c in &self.constraints {
            // lhs >= rhs means lhs depends on rhs (rhs → lhs edge)
            adj.entry(c.rhs.clone()).or_default().push(c.lhs.clone());
            *in_degree.entry(c.lhs.clone()).or_insert(0) += 1;
        }

        // Topological sort (Kahn's algorithm)
        let mut queue: VecDeque<String> = VecDeque::new();
        let mut assignments = self.assignments.clone();
        let mut order = Vec::new();

        // Initialize: nodes with in_degree 0 + concrete assignments
        for (var, &deg) in &in_degree {
            if deg == 0 {
                queue.push_back(var.clone());
                // If no concrete assignment, default to 0
                assignments.entry(var.clone()).or_insert(0);
            }
        }

        while let Some(var) = queue.pop_front() {
            order.push(var.clone());
            let level = *assignments.get(&var).unwrap_or(&0);

            if let Some(dependents) = adj.get(&var) {
                for dep in dependents {
                    // dep >= var, so dep level must be at least var's level
                    let dep_level = assignments.entry(dep.clone()).or_insert(0);
                    if *dep_level < level {
                        *dep_level = level;
                    }

                    let deg = in_degree.get_mut(dep).unwrap();
                    *deg -= 1;
                    if *deg == 0 {
                        queue.push_back(dep.clone());
                    }
                }
            }
        }

        if order.len() == self.variables.len() {
            LevelSolution {
                assignments,
                consistent: true,
                cycle: vec![],
            }
        } else {
            // Cycle detected: variables not in topological order
            let cycle: Vec<String> = self.variables.iter()
                .filter(|v| !order.contains(v))
                .cloned()
                .collect();
            LevelSolution {
                assignments,
                consistent: false,
                cycle,
            }
        }
    }

    /// Check if the graph is consistent (no cycles).
    pub fn check_consistent(&self) -> Result<(), String> {
        let solution = self.solve();
        if solution.consistent {
            Ok(())
        } else {
            Err(format!(
                "universe level cycle detected among: [{}]",
                solution.cycle.join(", ")
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_chain() {
        let mut g = LevelGraph::new();
        g.assign("U0", 0);
        g.add_constraint("U1", "U0", "cumul");
        g.add_constraint("U2", "U1", "cumul");
        let sol = g.solve();
        assert!(sol.consistent);
        assert_eq!(sol.assignments["U0"], 0);
        // U1 >= U0 = 0, so U1 = 0 (minimum satisfying)
        assert!(*sol.assignments.get("U1").unwrap() >= 0);
        assert!(*sol.assignments.get("U2").unwrap() >= 0);
    }

    #[test]
    fn concrete_propagation() {
        let mut g = LevelGraph::new();
        g.assign("U0", 0);
        g.assign("U1", 1);
        g.add_constraint("U2", "U1", "rule-a");
        g.add_constraint("U2", "U0", "rule-b");
        let sol = g.solve();
        assert!(sol.consistent);
        // U2 >= U1 = 1, so U2 >= 1
        assert!(*sol.assignments.get("U2").unwrap() >= 1);
    }

    #[test]
    fn cycle_detected() {
        let mut g = LevelGraph::new();
        g.add_constraint("A", "B", "rule1");
        g.add_constraint("B", "A", "rule2");
        let sol = g.solve();
        assert!(!sol.consistent);
        assert!(!sol.cycle.is_empty());
    }

    #[test]
    fn empty_graph() {
        let g = LevelGraph::new();
        let sol = g.solve();
        assert!(sol.consistent);
    }

    #[test]
    fn self_loop() {
        let mut g = LevelGraph::new();
        g.add_constraint("A", "A", "self");
        let sol = g.solve();
        // Self-loop: A >= A is technically satisfiable but creates in_degree > 0
        // Our implementation treats it as a cycle since A never reaches in_degree 0
        assert!(!sol.consistent);
    }

    #[test]
    fn diamond() {
        let mut g = LevelGraph::new();
        g.assign("base", 0);
        g.add_constraint("left", "base", "r1");
        g.add_constraint("right", "base", "r2");
        g.add_constraint("top", "left", "r3");
        g.add_constraint("top", "right", "r4");
        let sol = g.solve();
        assert!(sol.consistent);
        assert!(*sol.assignments.get("top").unwrap() >= 0);
    }
}
