/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Loading-query result values and DOT output rendering.

use std::fmt;
use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;

use crate::graph::QueryError;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Allocative)]
pub enum QueryOrder {
    Auto,
    Full,
}

impl QueryOrder {
    pub const fn is_full(self) -> bool {
        matches!(self, Self::Full)
    }

    pub fn parse(value: &str) -> Result<Self, QueryError> {
        match value {
            "auto" => Ok(Self::Auto),
            "full" => Ok(Self::Full),
            "deps" | "no" => Err(QueryError::syntax(format!(
                "--order_output={value} is not supported by this loading-query slice"
            ))),
            _ => Err(QueryError::syntax(format!(
                "unknown --order_output value: {value}"
            ))),
        }
    }
}

impl fmt::Display for QueryOrder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Auto => "auto",
            Self::Full => "full",
        })
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub struct QueryOutput {
    pub labels: Arc<[CompactString]>,
    pub(crate) graph: SelectedQueryGraph,
}

impl QueryOutput {
    pub fn stdout(&self) -> String {
        let mut output = String::new();
        for label in self.labels.iter() {
            output.push_str(label);
            output.push('\n');
        }
        output
    }

    /// Render Bazel's `label_kind` output from the selected nodes retained by
    /// evaluation. This preserves text output order without re-entering DICE
    /// or query evaluation.
    pub fn label_kind_stdout(&self) -> String {
        let mut output = String::new();
        let mut kinds = SmallMap::with_capacity(self.graph.nodes.len());
        for node in &self.graph.nodes {
            if let Some(kind) = node.kind.as_deref() {
                kinds.insert(&node.label, kind);
            }
        }
        for label in self.labels.iter() {
            let kind = kinds
                .get(label)
                .copied()
                .expect("label_kind output requires a completed selected-node kind");
            output.push_str(kind);
            output.push(' ');
            output.push_str(label);
            output.push('\n');
        }
        output
    }

    /// Render Bazel's package output from the selected labels without
    /// re-entering DICE. Main-repository package identifiers omit their `//`
    /// prefix for backwards compatibility.
    pub fn package_stdout(&self) -> String {
        let mut packages = self
            .labels
            .iter()
            .map(|label| {
                let package = label
                    .rsplit_once(':')
                    .map_or(label.as_str(), |(package, _)| package);
                package.strip_prefix("//").unwrap_or(package)
            })
            .collect::<Vec<_>>();
        packages.sort_unstable();
        packages.dedup();

        let mut output = String::new();
        for package in packages {
            output.push_str(package);
            output.push('\n');
        }
        output
    }

    /// Render the selected graph retained by the evaluation that produced this
    /// output. Formatting never re-enters DICE or query evaluation.
    pub fn graph_stdout(&self, factored: bool, sort_labels: bool) -> String {
        self.graph.stdout(factored, sort_labels)
    }
}

/// Request-local selected graph. This is intentionally compact: labels are
/// shared `CompactString`s and edges are checked `u32` node indexes.
#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub(crate) struct SelectedQueryGraph {
    pub(crate) nodes: Vec<SelectedQueryGraphNode>,
    pub(crate) generated_file_labels: SmallSet<CompactString>,
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub(crate) struct SelectedQueryGraphNode {
    pub(crate) label: CompactString,
    pub(crate) kind: Option<CompactString>,
    pub(crate) successors: Vec<u32>,
}

impl SelectedQueryGraph {
    const NODE_LIMIT: usize = 512;
    const RESERVED_LABEL_CHARS: usize = "\\n...and 9999999 more items".len();

    fn stdout(&self, factored: bool, sort_labels: bool) -> String {
        let mut classes = if factored {
            self.factored_classes(sort_labels)
        } else {
            (0..self.nodes.len())
                .map(|index| {
                    vec![
                        index
                            .try_into()
                            .expect("query graph exceeds u32 node capacity"),
                    ]
                })
                .collect()
        };
        if !factored && sort_labels {
            classes.sort_unstable_by(|left, right| {
                self.nodes[left[0] as usize]
                    .label
                    .cmp(&self.nodes[right[0] as usize].label)
            });
        }
        let mut class_for_node = vec![0_u32; self.nodes.len()];
        for (class, nodes) in classes.iter().enumerate() {
            let class: u32 = class
                .try_into()
                .expect("query graph exceeds u32 node capacity");
            for node in nodes {
                class_for_node[*node as usize] = class;
            }
        }
        let labels = classes
            .iter()
            .map(|class| self.class_label(class))
            .collect::<Vec<_>>();
        let mut successors = vec![Vec::<u32>::new(); classes.len()];
        for (class, nodes) in classes.iter().enumerate() {
            let class_id: u32 = class
                .try_into()
                .expect("query graph exceeds u32 node capacity");
            let mut seen = SmallSet::new();
            for node in nodes {
                for successor in &self.nodes[*node as usize].successors {
                    let successor_class = class_for_node[*successor as usize];
                    if successor_class != class_id && seen.insert(successor_class) {
                        successors[class].push(successor_class);
                    }
                }
            }
            if sort_labels {
                // With label sorting enabled, class IDs are the ranks of
                // Bazel's lexicographical node-sequence comparator. Comparing
                // joined DOT labels would be wrong at a `\\n` boundary.
                successors[class].sort_unstable();
            }
        }

        let order = topological_order(&successors);
        let mut output = String::from("digraph mygraph {\n  node [shape=box];\n");
        for node in order {
            let label = &labels[node as usize];
            output.push_str("  \"");
            output.push_str(label);
            output.push_str("\"\n");
            for successor in &successors[node as usize] {
                output.push_str("  \"");
                output.push_str(label);
                output.push_str("\" -> \"");
                output.push_str(&labels[*successor as usize]);
                output.push_str("\"\n");
            }
        }
        output.push_str("}\n");
        output
    }

    fn factored_classes(&self, sort_labels: bool) -> Vec<Vec<u32>> {
        let mut predecessors = vec![Vec::<u32>::new(); self.nodes.len()];
        for (node, value) in self.nodes.iter().enumerate() {
            for successor in &value.successors {
                predecessors[*successor as usize].push(
                    node.try_into()
                        .expect("query graph exceeds u32 node capacity"),
                );
            }
        }
        for predecessors in &mut predecessors {
            predecessors.sort_unstable();
        }

        let mut assigned = vec![false; self.nodes.len()];
        let mut classes = Vec::new();
        for node in 0..self.nodes.len() {
            if assigned[node] {
                continue;
            }
            assigned[node] = true;
            let mut class = vec![
                node.try_into()
                    .expect("query graph exceeds u32 node capacity"),
            ];
            for sibling in (node + 1)..self.nodes.len() {
                if !assigned[sibling]
                    && predecessors[node] == predecessors[sibling]
                    && self.nodes[node].successors == self.nodes[sibling].successors
                {
                    assigned[sibling] = true;
                    class.push(
                        sibling
                            .try_into()
                            .expect("query graph exceeds u32 node capacity"),
                    );
                }
            }
            if sort_labels {
                class.sort_unstable_by(|left, right| {
                    self.nodes[*left as usize]
                        .label
                        .cmp(&self.nodes[*right as usize].label)
                });
            } else if class.iter().all(|node| {
                self.generated_file_labels
                    .contains(&self.nodes[*node as usize].label)
            }) {
                // Bazel's unsorted factored visitor renders output-list
                // members in its reverse traversal order. Preserve that
                // order only for the generated-file equivalence class;
                // ordinary factored classes retain their existing order.
                class.reverse();
            }
            classes.push(class);
        }
        if sort_labels {
            classes.sort_unstable_by(|left, right| {
                left.iter()
                    .map(|node| &self.nodes[*node as usize].label)
                    .cmp(right.iter().map(|node| &self.nodes[*node as usize].label))
            });
        }
        classes
    }

    fn class_label(&self, class: &[u32]) -> CompactString {
        let mut label = String::new();
        let actual_limit = Self::NODE_LIMIT - Self::RESERVED_LABEL_CHARS;
        for (count, node) in class.iter().enumerate() {
            let item = &self.nodes[*node as usize].label;
            if count != 0 {
                label.push_str("\\n");
                if label.len() + item.len() > actual_limit {
                    label.push_str("...and ");
                    label.push_str(&(class.len() - count).to_string());
                    label.push_str(" more items");
                    break;
                }
            }
            label.push_str(item);
        }
        CompactString::new(label)
    }
}

fn topological_order(successors: &[Vec<u32>]) -> Vec<u32> {
    let starts = (0..successors.len())
        .map(|index| {
            index
                .try_into()
                .expect("query graph exceeds u32 node capacity")
        })
        .collect::<Vec<u32>>();
    let mut visited = vec![false; successors.len()];
    let mut postorder = Vec::with_capacity(successors.len());
    for start in starts {
        if visited[start as usize] {
            continue;
        }
        visited[start as usize] = true;
        let mut stack = vec![(start, 0_usize)];
        while let Some((node, next_child)) = stack.last_mut() {
            let children = &successors[*node as usize];
            if let Some(child) = children.get(*next_child).copied() {
                *next_child += 1;
                if !visited[child as usize] {
                    visited[child as usize] = true;
                    stack.push((child, 0));
                }
            } else {
                let (node, _) = stack.pop().expect("query DFS stack is non-empty");
                postorder.push(node);
            }
        }
    }
    postorder.reverse();
    postorder
}

#[cfg(test)]
mod graph_output_tests {
    use compact_str::CompactString;

    use super::SelectedQueryGraph;
    use super::SelectedQueryGraphNode;

    fn graph(nodes: &[(&str, &[u32])]) -> SelectedQueryGraph {
        SelectedQueryGraph {
            nodes: nodes
                .iter()
                .map(|(label, successors)| SelectedQueryGraphNode {
                    label: CompactString::new(*label),
                    kind: Some(CompactString::const_new("source file")),
                    successors: successors.to_vec(),
                })
                .collect(),
            generated_file_labels: Default::default(),
        }
    }

    #[test]
    fn full_factored_dot_matches_bazel_node_then_outgoing_edge_layout() {
        let output = graph(&[("//a:root", &[1, 2]), ("//a:left", &[]), ("//a:right", &[])])
            .stdout(true, true);
        assert_eq!(
            output,
            concat!(
                "digraph mygraph {\n",
                "  node [shape=box];\n",
                "  \"//a:root\"\n",
                "  \"//a:root\" -> \"//a:left\\n//a:right\"\n",
                "  \"//a:left\\n//a:right\"\n",
                "}\n",
            )
        );
    }

    #[test]
    fn unfactored_dot_keeps_equivalent_nodes_separate() {
        let output = graph(&[("//a:root", &[1, 2]), ("//a:left", &[]), ("//a:right", &[])])
            .stdout(false, true);
        assert_eq!(
            output,
            concat!(
                "digraph mygraph {\n",
                "  node [shape=box];\n",
                "  \"//a:root\"\n",
                "  \"//a:root\" -> \"//a:left\"\n",
                "  \"//a:root\" -> \"//a:right\"\n",
                "  \"//a:right\"\n",
                "  \"//a:left\"\n",
                "}\n",
            )
        );
    }

    #[test]
    fn factoring_requires_matching_predecessors_and_deduplicates_quotient_edges() {
        let different_predecessors = graph(&[
            ("//a:left_parent", &[2]),
            ("//a:right_parent", &[3]),
            ("//a:left", &[4]),
            ("//a:right", &[4]),
            ("//a:leaf", &[]),
        ]);
        assert!(
            !different_predecessors
                .factored_classes(true)
                .iter()
                .map(|class| different_predecessors.class_label(class))
                .any(|label| label == "//a:left\\n//a:right"),
            "equal successors alone must not factor nodes"
        );

        let duplicate_quotient_edges = graph(&[
            ("//a:root", &[1, 2]),
            ("//a:left", &[3]),
            ("//a:right", &[3]),
            ("//a:leaf", &[]),
        ])
        .stdout(true, true);
        assert_eq!(
            duplicate_quotient_edges
                .matches("\"//a:root\" -> \"//a:left\\n//a:right\"")
                .count(),
            1,
            "{duplicate_quotient_edges}"
        );
    }

    #[test]
    fn factored_order_compares_member_label_sequences_not_joined_dot_labels() {
        let output = graph(&[
            ("//a:a", &[]),
            ("//z:z", &[]),
            ("//a:a0", &[3]),
            ("//x:leaf", &[]),
        ])
        .stdout(true, true);
        assert_eq!(
            output,
            concat!(
                "digraph mygraph {\n",
                "  node [shape=box];\n",
                "  \"//a:a0\"\n",
                "  \"//a:a0\" -> \"//x:leaf\"\n",
                "  \"//x:leaf\"\n",
                "  \"//a:a\\n//z:z\"\n",
                "}\n",
            )
        );
    }
}
