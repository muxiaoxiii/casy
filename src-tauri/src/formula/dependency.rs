//! Formula dependency graph: tracks which formula fields depend on which data fields,
//! provides topological sort for recalculation order, and BFS traversal for cascading updates.

use std::collections::{HashMap, HashSet, VecDeque};

/// A dependency graph mapping formula fields to their input dependencies.
///
/// `dependents[field_id]` = list of field_ids that `field_id` depends on.
/// For example, if formula `formula_case_status` reads `case_progress`,
/// then `dependents["formula_case_status"]` contains `"case_progress"`.
#[derive(Debug, Default)]
pub struct DependencyGraph {
    /// field_id → Vec<field_ids that this field depends on>
    dependents: HashMap<String, Vec<String>>,
    /// reverse: field_id → Vec<formula field_ids that depend on it>
    dependents_of: HashMap<String, Vec<String>>,
}

impl DependencyGraph {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a formula field with its dependencies.
    ///
    /// `formula_field`: the formula cache column (e.g., "formula_case_status")
    /// `depends_on`: the data field(s) it reads (e.g., ["case_progress"])
    pub fn register(&mut self, formula_field: &str, depends_on: Vec<String>) {
        for dep in &depends_on {
            self.dependents_of
                .entry(dep.clone())
                .or_default()
                .push(formula_field.to_string());
        }
        self.dependents
            .insert(formula_field.to_string(), depends_on);
    }

    /// Given a changed data field, return all formula fields that need recalculation,
    /// in topological order (dependencies first).
    ///
    /// Returns `None` if no formula depends on this field.
    pub fn get_recalc_order(&self, changed_field: &str) -> Option<Vec<String>> {
        let mut to_recalc = HashSet::new();
        let mut queue = VecDeque::new();

        // Start with direct dependents of the changed field
        if let Some(direct) = self.dependents_of.get(changed_field) {
            for d in direct {
                if to_recalc.insert(d.clone()) {
                    queue.push_back(d.clone());
                }
            }
        }

        // BFS: if a formula depends on another formula, cascade
        while let Some(field) = queue.pop_front() {
            if let Some(transitive) = self.dependents_of.get(&field) {
                for t in transitive {
                    if to_recalc.insert(t.clone()) {
                        queue.push_back(t.clone());
                    }
                }
            }
        }

        if to_recalc.is_empty() {
            return None;
        }

        // Topological sort: formula fields that have no formula dependencies come first
        Some(self.topo_sort(&to_recalc))
    }

    /// Get all formula fields that are registered.
    pub fn all_formula_fields(&self) -> Vec<&str> {
        self.dependents.keys().map(|s| s.as_str()).collect()
    }

    /// Get dependencies of a specific formula field.
    pub fn get_dependencies(&self, formula_field: &str) -> Option<&Vec<String>> {
        self.dependents.get(formula_field)
    }

    /// Topological sort of a subset of formula fields.
    /// Fields with fewer formula-level dependencies come first.
    fn topo_sort(&self, fields: &HashSet<String>) -> Vec<String> {
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        let field_set: HashSet<&str> = fields.iter().map(|s| s.as_str()).collect();

        for f in fields {
            let mut count = 0;
            if let Some(deps) = self.dependents.get(f) {
                for dep in deps {
                    if field_set.contains(dep.as_str()) {
                        count += 1;
                    }
                }
            }
            in_degree.insert(f.clone(), count);
        }

        let mut queue: VecDeque<String> = VecDeque::new();
        for (f, &deg) in &in_degree {
            if deg == 0 {
                queue.push_back(f.clone());
            }
        }

        let mut result = Vec::new();
        while let Some(f) = queue.pop_front() {
            result.push(f.clone());
            // Reduce in-degree for fields that depend on f
            if let Some(dependents_of_f) = self.dependents_of.get(&f) {
                for d in dependents_of_f {
                    if let Some(deg) = in_degree.get_mut(d) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push_back(d.clone());
                        }
                    }
                }
            }
        }

        result
    }
}

/// Build the standard Casy formula dependency graph from the Feishu design doc.
///
/// This pre-populates the graph with all 10 formula fields and their data dependencies.
pub fn build_casy_dependency_graph() -> DependencyGraph {
    let mut graph = DependencyGraph::new();

    // ── cases table formulas ──────────────────────────────────

    // formula_case_status depends on case_progress
    graph.register("formula_case_status", vec!["case_progress".to_string()]);

    // formula_defense_deadline depends on cause_action + complaint_received_date
    graph.register(
        "formula_defense_deadline",
        vec![
            "cause_action".to_string(),
            "complaint_received_date".to_string(),
        ],
    );

    // formula_estimated_trial_limit depends on filing_date + cause_action + procedure_type + stay_date
    graph.register(
        "formula_estimated_trial_limit",
        vec![
            "filing_date".to_string(),
            "cause_action".to_string(),
            "procedure_type".to_string(),
            "stay_date".to_string(),
        ],
    );

    // formula_petitioner_first depends on cause_action + filing_date
    graph.register(
        "formula_petitioner_first",
        vec!["cause_action".to_string(), "filing_date".to_string()],
    );

    // formula_petitioner_supp depends on cause_action + petitioner_first_invalid
    graph.register(
        "formula_petitioner_supp",
        vec![
            "cause_action".to_string(),
            "petitioner_first_invalid".to_string(),
        ],
    );

    // formula_petitioner_reply depends on cause_action + petitioner_received_date
    graph.register(
        "formula_petitioner_reply",
        vec![
            "cause_action".to_string(),
            "petitioner_received_date".to_string(),
        ],
    );

    // formula_patentee_statement depends on cause_action + patentee_received_date
    graph.register(
        "formula_patentee_statement",
        vec![
            "cause_action".to_string(),
            "patentee_received_date".to_string(),
        ],
    );

    // formula_patentee_supp depends on cause_action + patentee_received_supp_date
    graph.register(
        "formula_patentee_supp",
        vec![
            "cause_action".to_string(),
            "patentee_received_supp_date".to_string(),
        ],
    );

    // ── hearings table formulas ───────────────────────────────

    // formula_status depends on hearing_date (cross-table: hearings.hearing_date)
    graph.register(
        "formula_status",
        vec!["hearing_date".to_string()],
    );

    // ── tasks table formulas ──────────────────────────────────

    // formula_days_until_deadline depends on completed + deadline
    graph.register(
        "formula_days_until_deadline",
        vec!["completed".to_string(), "deadline".to_string()],
    );

    graph
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topo_sort_linear() {
        let mut g = DependencyGraph::new();
        g.register("C", vec!["B".to_string()]);
        g.register("B", vec!["A".to_string()]);

        let order = g.get_recalc_order("A").unwrap();
        // B depends on A directly, C depends on B (transitive)
        assert!(order.contains(&"B".to_string()));
        assert!(order.contains(&"C".to_string()));
        // B should come before C
        let b_pos = order.iter().position(|x| x == "B").unwrap();
        let c_pos = order.iter().position(|x| x == "C").unwrap();
        assert!(b_pos < c_pos);
    }

    #[test]
    fn topo_sort_independent() {
        let mut g = DependencyGraph::new();
        g.register("X", vec!["A".to_string()]);
        g.register("Y", vec!["B".to_string()]);

        let order = g.get_recalc_order("A").unwrap();
        assert_eq!(order, vec!["X"]);
    }

    #[test]
    fn no_dependents() {
        let mut g = DependencyGraph::new();
        g.register("X", vec!["A".to_string()]);
        assert!(g.get_recalc_order("Z").is_none());
    }

    #[test]
    fn casy_graph_case_progress() {
        let g = build_casy_dependency_graph();
        let order = g.get_recalc_order("case_progress").unwrap();
        assert!(order.contains(&"formula_case_status".to_string()));
    }

    #[test]
    fn casy_graph_filing_date() {
        let g = build_casy_dependency_graph();
        let order = g.get_recalc_order("filing_date").unwrap();
        // filing_date affects: estimated_trial_limit, petitioner_first
        assert!(order.contains(&"formula_estimated_trial_limit".to_string()));
        assert!(order.contains(&"formula_petitioner_first".to_string()));
    }

    #[test]
    fn casy_all_formulas() {
        let g = build_casy_dependency_graph();
        let all = g.all_formula_fields();
        assert_eq!(all.len(), 10);
    }
}
