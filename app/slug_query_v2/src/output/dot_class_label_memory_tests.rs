use super::*;

fn graph(labels: &[String], edges: &[&[u32]]) -> SelectedQueryGraph {
    SelectedQueryGraph {
        nodes: labels
            .iter()
            .zip(edges)
            .map(|(label, successors)| SelectedQueryGraphNode {
                label: CompactString::new(label),
                kind: Some(CompactString::const_new("source file")),
                successors: successors.to_vec(),
            })
            .collect(),
        generated_file_labels: Default::default(),
    }
}

fn original_label(labels: &[String]) -> (CompactString, bool) {
    let mut label = String::new();
    let limit = SelectedQueryGraph::NODE_LIMIT - SelectedQueryGraph::RESERVED_LABEL_CHARS;
    for (count, item) in labels.iter().enumerate() {
        if count != 0 {
            label.push_str("\\n");
            if label.len() + item.len() > limit {
                label.push_str("...and ");
                label.push_str(&(labels.len() - count).to_string());
                label.push_str(" more items");
                break;
            }
        }
        label.push_str(item);
    }
    let tight = label.capacity() == label.len();
    (CompactString::new(label), tight)
}

#[test]
fn dot_class_labels_preserve_bytes_capacity_and_truncation_boundaries() {
    let limit = SelectedQueryGraph::NODE_LIMIT - SelectedQueryGraph::RESERVED_LABEL_CHARS;
    let cases = [
        vec![],
        vec![String::new()],
        vec!["x".into()],
        vec!["x".repeat(24)],
        vec!["x".repeat(25)],
        vec!["x".repeat(31)],
        vec!["x".repeat(32)],
        vec!["x".repeat(8193)],
        vec!["a".repeat(limit - 3), "b".into()],
        vec!["a".repeat(limit - 3), "bb".into()],
        vec!["a".repeat(limit + 1), "b".into()],
        vec!["é世界".repeat(64), "🌍".repeat(32)],
        vec!["//p:t".into(); 101],
        vec!["//p:first".into(), "//p:second".into(), "//p:third".into()],
    ];
    let mut tight_count = 0;
    let mut spare_count = 0;
    for (index, labels) in cases.iter().enumerate() {
        let edges = vec![&[][..]; labels.len()];
        let selected = graph(labels, &edges);
        let class: Vec<_> = (0..labels.len() as u32).collect();
        let (expected, tight) = original_label(labels);
        let actual = selected.class_label(&class);
        assert_eq!(actual, expected, "case {index}");
        assert_eq!(actual.capacity(), expected.capacity(), "case {index}");
        assert_eq!(actual.is_heap_allocated(), expected.is_heap_allocated());
        if tight {
            tight_count += 1;
        } else {
            spare_count += 1;
        }
        println!(
            "DOT_CLASS case={index} tight={tight} bytes={} capacity={}",
            actual.len(),
            actual.capacity()
        );
    }
    assert!(tight_count > 0 && spare_count > 0);
}

#[test]
fn dot_outputs_preserve_modes_graph_owners_and_retained_text() {
    let mut outputs = Vec::new();
    for cyclic in [false, true] {
        let labels = ["//é:root", "//p:left", "//p:right", "//p:leaf"].map(String::from);
        let acyclic: [&[u32]; 4] = [&[2, 1, 2], &[3], &[3], &[]];
        let cycle: [&[u32]; 4] = [&[1, 1], &[0, 2], &[3], &[]];
        let mut selected = graph(&labels, if cyclic { &cycle } else { &acyclic });
        selected
            .generated_file_labels
            .insert(CompactString::new(&labels[2]));
        selected
            .generated_file_labels
            .insert(CompactString::new(&labels[1]));
        let query = QueryOutput {
            labels: Arc::from(labels.iter().map(CompactString::new).collect::<Vec<_>>()),
            graph: selected,
        };
        let clone = query.clone();
        drop(labels);
        for factored in [false, true] {
            for sorted in [false, true] {
                let output = query.graph_stdout(factored, sorted);
                assert_eq!(output, clone.graph_stdout(factored, sorted));
                assert!(output.starts_with("digraph mygraph {\n  node [shape=box];\n"));
                assert!(output.contains("//é:root") && output.contains("//p:leaf"));
                assert!(output.ends_with("}\n"));
                outputs.push(output);
            }
        }
        assert_eq!(query, clone);
        drop(query);
        drop(clone);
    }
    assert_eq!(outputs.len(), 8);
    assert!(outputs.iter().all(|output| output.contains("//é:root")));
    println!("DOT_OUTPUTS={outputs:?}");
}
