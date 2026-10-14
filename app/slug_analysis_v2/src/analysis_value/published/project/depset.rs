//! Preserve typed DAG rows with iterative dense postorder projection.
use super::*;

impl Projector {
    fn reserve_depset_batch(
        &mut self,
        credit: &mut usize,
        nodes: usize,
        edges: usize,
    ) -> Result<(), String> {
        self.budget.reserve_forecast(credit, || {
            let native = AnalysisDepset::local_graph_storage_upper_bound(nodes, edges)
                .ok_or("typed projection native peak overflow")?;
            budget::caller_peak(nodes, edges)?
                .checked_add(native)
                .ok_or_else(|| "typed projection combined peak overflow".into())
        })
    }
    pub(super) fn depset(&mut self, root: &PublishedValueRef) -> Result<AnalysisValue, String> {
        let result = self.depset_inner(root);
        if result.is_err() {
            self.budget.poison();
        }
        result
    }
    fn depset_inner(&mut self, root: &PublishedValueRef) -> Result<AnalysisValue, String> {
        self.budget.admit_arena(root.arena())?;
        if let Some(value) = self.depsets.get(&root.identity()) {
            let value = AnalysisValue::with_original_origin(
                AnalysisValue::depset(value.clone()),
                root.clone(),
            )
            .map_err(err)?;
            self.values.insert(root.identity(), value.clone());
            return Ok(value);
        }
        let mut stack = Vec::new();
        let mut local = UnorderedMap::new();
        let mut active = UnorderedMap::new();
        let mut originals = Vec::new();
        let mut graph = Vec::new();
        let mut visits = 0usize;
        let mut nodes = 0usize;
        let mut edges = 0usize;
        let mut batch_credit = 0usize;
        self.budget
            .reserve_forecast(&mut batch_credit, || budget::caller_peak(0, 0))?;
        stack.push((root.clone(), false));
        while let Some((original, exiting)) = stack.pop() {
            self.budget.admit_arena(original.arena())?;
            visits = visits
                .checked_add(1)
                .filter(|n| *n <= NODE_LIMIT)
                .ok_or("typed depset projection visit limit")?;
            let identity = original.identity();
            if self.values.contains_key(&identity)
                || self.depsets.contains_key(&identity)
                || local.contains_key(&identity)
            {
                continue;
            }
            let PublishedNode::Depset(row) = original.node() else {
                return Err("typed projection starts from a depset row".into());
            };
            if row.depth > slug_build_api_v2::depset::MAX_DEPTH {
                return Err("typed depset projection logical depth exceeds 3500".into());
            }
            if !exiting {
                if active.contains_key(&identity) {
                    return Err("cyclic typed depset projection".into());
                }
                nodes = nodes.checked_add(1).ok_or("typed projection node limit")?;
                edges = edges
                    .checked_add(row.successors.len())
                    .ok_or("typed projection edge limit")?;
                if nodes > NODE_LIMIT || edges > EDGE_LIMIT {
                    return Err("typed projection shape limit".into());
                }
                self.reserve_depset_batch(&mut batch_credit, nodes, edges)?;
                active.insert(identity, ());
                stack.push((original.clone(), true));
                for child in row.successors.iter().rev() {
                    if let PublishedDepsetInput::Transitive(edge) = child {
                        let child = original.arena().resolve(*edge).map_err(err)?;
                        if matches!(child.node(), PublishedNode::Depset(_)) {
                            stack.push((child, false));
                        } else {
                            let projected = self.value(&child)?;
                            if !matches!(projected.kind(), AnalysisValueKind::Depset(_)) {
                                return Err("invalid transitive depset projection".into());
                            }
                        }
                    }
                }
                continue;
            }
            self.reserve_depset_batch(&mut batch_credit, nodes, edges)?;
            let mut successors = Vec::with_capacity(row.successors.len());
            for successor in &*row.successors {
                visits = visits
                    .checked_add(1)
                    .filter(|n| *n <= NODE_LIMIT)
                    .ok_or("typed depset projection visit limit")?;
                successors.push(match successor {
                    PublishedDepsetInput::Direct(edge) => {
                        AnalysisDepsetGraphInput::Direct(self.edge(&original, *edge)?)
                    }
                    PublishedDepsetInput::Transitive(edge) => {
                        let child = original.arena().resolve(*edge).map_err(err)?;
                        if let Some(id) = local.get(&child.identity()) {
                            AnalysisDepsetGraphInput::Local(*id)
                        } else if let Some(child) = self.depsets.get(&child.identity()) {
                            AnalysisDepsetGraphInput::External(child.clone())
                        } else {
                            let value = self.value(&child)?;
                            let AnalysisValueKind::Depset(child) = value.kind() else {
                                return Err("invalid completed transitive depset".into());
                            };
                            AnalysisDepsetGraphInput::External(child.clone())
                        }
                    }
                });
            }
            let source = match successors.as_slice() {
                [AnalysisDepsetGraphInput::Local(child)] => {
                    let child_original: &PublishedValueRef = &originals[*child];
                    let PublishedNode::Depset(child_row) = child_original.node() else {
                        return Err("invalid local depset row".into());
                    };
                    if row.order != child_row.order {
                        AnalysisDepsetGraphRow::Local(*child)
                    } else {
                        AnalysisDepsetGraphRow::Successors(successors)
                    }
                }
                [AnalysisDepsetGraphInput::External(child)] if row.order != child.order() => {
                    AnalysisDepsetGraphRow::External(child.clone())
                }
                _ => AnalysisDepsetGraphRow::Successors(successors),
            };
            local.insert(identity.clone(), graph.len());
            graph.push(AnalysisDepsetGraphNode::new(
                row.occurrence.clone(),
                row.order,
                row.depth,
                source,
            ));
            originals.push(original);
            active.remove(&identity);
        }
        self.reserve_depset_batch(&mut batch_credit, nodes, edges)?;
        let values = AnalysisDepset::from_local_graph(graph).map_err(err)?;
        for (original, value) in originals.into_iter().zip(values) {
            self.depsets.insert(original.identity(), value);
        }
        let native = self
            .depsets
            .get(&root.identity())
            .ok_or("missing projected typed root")?;
        let value = AnalysisValue::with_original_origin(
            AnalysisValue::depset(native.clone()),
            root.clone(),
        )
        .map_err(err)?;
        self.values.insert(root.identity(), value.clone());
        Ok(value)
    }
}
