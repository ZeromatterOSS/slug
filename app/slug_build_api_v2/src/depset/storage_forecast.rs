//! Checked peak forecast for the concrete local dense-construction arrays.
use super::*;

/// Three conservative capacity credits cover old/new growth allocations and
/// Vec-to-Box transfer together. Eight slots dominate Vec's initial minimum;
/// twice the final length dominates its geometric capacity for these sized types.
pub(crate) fn vector_peak<T>(len: usize) -> Option<usize> {
    let header = std::mem::size_of::<Vec<T>>();
    if len == 0 {
        return Some(header);
    }
    len.checked_mul(2)?
        .max(8)
        .checked_mul(std::mem::size_of::<T>())?
        .checked_mul(3)?
        .checked_add(header)
}
/// Input successor arrays live in separate per-row Vecs. Credit each nonempty
/// segment's initial capacity independently, not just one aggregate Vec.
pub(crate) fn input_edges_peak<T>(nodes: usize, edges: usize) -> Option<usize> {
    let capacity = edges
        .checked_mul(2)?
        .checked_add(nodes.min(edges).checked_mul(8)?)?;
    capacity
        .checked_mul(std::mem::size_of::<T>())?
        .checked_mul(3)
}
fn index_peak<K>(len: usize) -> Option<usize> {
    let header = std::mem::size_of::<FxHashMap<K, u32>>();
    if len == 0 {
        return Some(header);
    }
    // Power-of-two buckets at <=7/8 occupancy need fewer than4*len+8
    // buckets. Include controls/alignment and old/new-table coexistence.
    let buckets = len.checked_mul(4)?.checked_add(8)?;
    buckets
        .checked_mul(std::mem::size_of::<(K, u32)>().checked_add(1)?)?
        .checked_add(64)?
        .checked_mul(3)?
        .checked_add(header)
}
pub(super) fn dense<T, M>(nodes: usize, edges: usize) -> Option<usize> {
    let mut bytes = std::mem::size_of::<DenseDepsetStore<T, M>>()
        .checked_add(2 * std::mem::size_of::<usize>())?;
    // Assume every reference independently needs a leaf, successor and external
    // handle; every row may additionally own an external canonical row.
    for array in [
        vector_peak::<DenseDepsetNodeInput<T, M>>(nodes)?,
        input_edges_peak::<DenseDepsetInput<T, M>>(nodes, edges)?,
        vector_peak::<DenseNode<M>>(nodes)?,
        vector_peak::<DenseSuccessor>(edges)?,
        vector_peak::<T>(edges)?,
        vector_peak::<Depset<T, M>>(edges)?,
        vector_peak::<DenseExternalRow<T, M>>(nodes)?,
        vector_peak::<Depset<T, M>>(nodes)?,
        index_peak::<(usize, u32)>(edges)?,
        index_peak::<(usize, DenseRange)>(nodes)?,
    ] {
        bytes = bytes.checked_add(array)?;
    }
    Some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dense_forecast_covers_empty_direct_external_and_checked_growth() {
        let empty = dense::<u64, ()>(0, 0).unwrap();
        let direct = dense::<u64, ()>(1, 1).unwrap();
        let external = dense::<u64, ()>(2, 2).unwrap();
        assert!(
            empty
                >= std::mem::size_of::<DenseDepsetStore<u64, ()>>()
                    + 2 * std::mem::size_of::<usize>()
        );
        assert!(direct > empty);
        assert!(external > direct);
        // Input arrays, native output arrays, both indexes and returned guards
        // all exist in the conservative sum, even when one semantic case would
        // use fewer categories. No element is inspected or cloned by forecast.
        let rows = vec![DenseDepsetNodeInput {
            order: DepsetOrder::Default,
            row: DenseDepsetRowSource::Successors(vec![DenseDepsetInput::Direct(7u64)]),
            depth: 1,
            metadata: (),
        }];
        let original = Depset::from_dense_nodes(rows).unwrap().pop().unwrap();
        let rows = vec![DenseDepsetNodeInput {
            order: DepsetOrder::Default,
            row: DenseDepsetRowSource::External(original),
            depth: 1,
            metadata: (),
        }];
        let projected = Depset::from_dense_nodes(rows).unwrap();
        let store = &projected[0].store;
        let actual_retained = std::mem::size_of::<DenseDepsetStore<u64, ()>>()
            + 2 * std::mem::size_of::<usize>()
            + std::mem::size_of_val(store.nodes.as_ref())
            + std::mem::size_of_val(store.successors.as_ref())
            + std::mem::size_of_val(store.leaves.as_ref())
            + std::mem::size_of_val(store.external_depsets.as_ref())
            + std::mem::size_of_val(store.external_rows.as_ref())
            + std::mem::size_of_val(projected.as_slice());
        assert!(dense::<u64, ()>(1, 0).unwrap() >= actual_retained);
        assert!(
            input_edges_peak::<DenseDepsetInput<u64, ()>>(8, 8).unwrap()
                >= 8 * 8 * std::mem::size_of::<DenseDepsetInput<u64, ()>>()
        );
        assert!(dense::<u64, ()>(usize::MAX, 1).is_none());
        assert!(dense::<u64, ()>(1, usize::MAX).is_none());
        assert!(vector_peak::<u64>(usize::MAX).is_none());
    }
    #[test]
    fn analysis_frontend_forecast_precedes_empty_direct_and_external_construction() {
        use crate::analysis_value::AnalysisDepsetGraphInput;
        use crate::analysis_value::AnalysisDepsetGraphNode;
        use crate::analysis_value::AnalysisDepsetGraphRow;
        let empty = crate::AnalysisDepset::local_graph_storage_upper_bound(0, 0).unwrap();
        assert!(empty > 0);
        assert!(
            crate::AnalysisDepset::from_local_graph(vec![])
                .unwrap()
                .is_empty()
        );
        let forecast = crate::AnalysisDepset::local_graph_storage_upper_bound(1, 1).unwrap();
        assert!(forecast > empty);
        let leaf = AnalysisDepsetGraphNode::new(
            crate::AnalysisDepsetOccurrence::new(),
            DepsetOrder::Default,
            1,
            AnalysisDepsetGraphRow::Successors(vec![AnalysisDepsetGraphInput::Direct(
                crate::AnalysisValue::string("leaf"),
            )]),
        );
        let direct = crate::AnalysisDepset::from_local_graph(vec![leaf])
            .unwrap()
            .pop()
            .unwrap();
        let mut physical_bytes = 0usize;
        direct
            .visit_storage_bounded(|event| {
                if let DepsetStorageEvent::Charge(charge) = event {
                    physical_bytes += charge.bytes;
                }
                Ok::<_, std::convert::Infallible>(())
            })
            .unwrap();
        assert!(forecast >= physical_bytes);
        let external = AnalysisDepsetGraphNode::new(
            crate::AnalysisDepsetOccurrence::new(),
            DepsetOrder::Default,
            direct.depth(),
            AnalysisDepsetGraphRow::External(direct),
        );
        assert_eq!(
            crate::AnalysisDepset::from_local_graph(vec![external])
                .unwrap()
                .len(),
            1
        );
        assert!(crate::AnalysisDepset::local_graph_storage_upper_bound(usize::MAX, 0).is_none());
        assert!(crate::AnalysisDepset::local_graph_storage_upper_bound(0, usize::MAX).is_none());
    }
}
