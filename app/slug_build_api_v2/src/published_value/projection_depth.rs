//! Iterative native containment heights, distinct from typed depset depth.
use super::*;

#[derive(Clone, Copy)]
struct WeightedEdge {
    edge: PublishedEdge,
    weight: usize,
}
struct Frame {
    node: usize,
    next: usize,
    end: usize,
    enclosing: usize,
    intrinsic: usize,
    complete: usize,
    incoming: usize,
}

/// All four arrays coexist with the seal's canonical source scratch and final
/// row arrays. The builder charges this conservative upper bound before seal.
pub(super) fn scratch_bytes(rows: usize, references: usize) -> Option<usize> {
    let adjacency = references.checked_mul(std::mem::size_of::<WeightedEdge>())?;
    let offsets = rows
        .checked_add(1)?
        .checked_mul(std::mem::size_of::<usize>())?;
    let colors = rows.checked_mul(std::mem::size_of::<(u16, u16)>())?;
    let frames = rows.checked_mul(std::mem::size_of::<Frame>())?;
    adjacency
        .checked_add(offsets)?
        .checked_add(colors)?
        .checked_add(frames)?
        .checked_add(4 * std::mem::size_of::<Vec<usize>>())
}
pub(in crate::published_value) fn draft_scratch_bytes(
    rows: &[Option<PublishedNode>],
    edges: &[PublishedEdge],
) -> Result<usize, PublishedValueError> {
    // Several rows may reuse one stored range. Count actual containment
    // references before allocating expanded adjacency, rather than assuming
    // that the stored-edge array bounds its multiplicity.
    let mut references = 0usize;
    for row in rows {
        let row = row
            .as_ref()
            .ok_or(PublishedValueError::Invalid("incomplete row"))?;
        visit_edges(row, edges, true, |_| {
            references = references
                .checked_add(1)
                .filter(|n| *n <= EDGE_LIMIT)
                .ok_or(PublishedValueError::Limit("projection references"))?;
            Ok(())
        })?;
    }
    scratch_bytes(rows.len(), references).ok_or(PublishedValueError::Limit("projection scratch"))
}
fn resolve<'a>(
    edge: PublishedEdge,
    rows: &'a [PublishedNode],
    imports: &'a [PublishedValueArena],
) -> Result<&'a PublishedNode, PublishedValueError> {
    match edge {
        PublishedEdge::Local(id) => rows
            .get(id.index())
            .ok_or(PublishedValueError::Invalid("node")),
        PublishedEdge::Imported { arena, node } => imports
            .get(arena.index())
            .ok_or(PublishedValueError::Invalid("import"))?
            .node(node),
    }
}
fn weighted_edges(
    row: &PublishedNode,
    rows: &[PublishedNode],
    edges: &[PublishedEdge],
    imports: &[PublishedValueArena],
    mut visit: impl FnMut(WeightedEdge) -> Result<(), PublishedValueError>,
) -> Result<(), PublishedValueError> {
    if let PublishedNode::Depset(depset) = row {
        if depset.depth > crate::depset::MAX_DEPTH {
            return Err(PublishedValueError::Limit("depset depth"));
        }
        for input in &*depset.successors {
            let (edge, weight) = match input {
                PublishedDepsetInput::Direct(edge) => (*edge, 1),
                PublishedDepsetInput::Transitive(edge) => {
                    // Zero containment weight belongs only to a typed DAG edge.
                    match resolve(*edge, rows, imports)? {
                        PublishedNode::Depset(child) if child.depth <= crate::depset::MAX_DEPTH => {
                        }
                        PublishedNode::Native(child)
                            if matches!(child.value().kind(),
                            AnalysisValueKind::Depset(d) if d.depth() <= crate::depset::MAX_DEPTH) =>
                            {}
                        _ => return Err(PublishedValueError::Invalid("transitive depset")),
                    }
                    (*edge, 0)
                }
            };
            visit(WeightedEdge { edge, weight })?;
        }
        Ok(())
    } else {
        visit_edges(row, edges, true, |edge| {
            visit(WeightedEdge { edge, weight: 1 })
        })
    }
}
fn adjacency(
    rows: &[PublishedNode],
    edges: &[PublishedEdge],
    imports: &[PublishedValueArena],
) -> Result<(Vec<WeightedEdge>, Vec<usize>), PublishedValueError> {
    // Count without growing storage. This separate bounded validator does not
    // recharge the producer's already-paid reference construction work.
    let mut count = 0usize;
    for row in rows {
        visit_edges(row, edges, true, |_| {
            count = count
                .checked_add(1)
                .filter(|n| *n <= EDGE_LIMIT)
                .ok_or(PublishedValueError::Limit("projection references"))?;
            Ok(())
        })?;
    }
    let mut adjacency = Vec::with_capacity(count);
    let mut offsets = Vec::with_capacity(rows.len() + 1);
    offsets.push(0);
    for row in rows {
        weighted_edges(row, rows, edges, imports, |edge| {
            adjacency.push(edge);
            Ok(())
        })?;
        offsets.push(adjacency.len());
    }
    Ok((adjacency, offsets))
}
#[path = "projection_depth_native.rs"]
mod native;
use native::base;

fn merge(
    frame: &mut Frame,
    child: (usize, usize),
    weight: usize,
) -> Result<(), PublishedValueError> {
    frame.intrinsic = frame.intrinsic.max(child.0.saturating_add(weight));
    frame.complete = frame.complete.max(child.1.saturating_add(weight));
    if frame.enclosing.saturating_add(frame.complete) > DEPTH_LIMIT {
        return Err(PublishedValueError::Limit("projection depth"));
    }
    Ok(())
}

pub(super) fn validate(
    rows: &[PublishedNode],
    edges: &[PublishedEdge],
    imports: &[PublishedValueArena],
) -> Result<(usize, usize), PublishedValueError> {
    let (adjacency, offsets) = adjacency(rows, edges, imports)?;
    // Each column: unseen0, active1, or completed2 + relative height.
    let mut colors = vec![(0u16, 0u16); rows.len()];
    let mut frames: Vec<Frame> = Vec::with_capacity(rows.len());
    let mut maximum = (0usize, 0usize);
    for root in 0..rows.len() {
        if colors[root].1 >= 2 {
            continue;
        }
        colors[root] = (1, 1);
        let root_base = base(&rows[root])?;
        frames.push(Frame {
            node: root,
            next: offsets[root],
            end: offsets[root + 1],
            enclosing: 0,
            intrinsic: root_base,
            complete: root_base,
            incoming: 0,
        });
        while let Some(frame) = frames.last_mut() {
            if frame.next == frame.end {
                let frame = frames.pop().unwrap();
                let heights = (frame.intrinsic, frame.complete);
                colors[frame.node] = ((heights.0 + 2) as u16, (heights.1 + 2) as u16);
                maximum.0 = maximum.0.max(heights.0);
                maximum.1 = maximum.1.max(heights.1);
                if let Some(parent) = frames.last_mut() {
                    merge(parent, heights, frame.incoming)?;
                }
                continue;
            }
            let edge = adjacency[frame.next];
            frame.next += 1;
            match edge.edge {
                PublishedEdge::Imported { arena, node } => {
                    let arena = imports
                        .get(arena.index())
                        .ok_or(PublishedValueError::Invalid("import"))?;
                    arena.node(node)?;
                    merge(frame, (0, arena.shape().depth), edge.weight)?;
                }
                PublishedEdge::Local(id) => {
                    let node = id.index();
                    let color = *colors
                        .get(node)
                        .ok_or(PublishedValueError::Invalid("node"))?;
                    if color.1 == 1 {
                        return Err(PublishedValueError::Unsupported("cyclic native projection"));
                    }
                    if color.1 >= 2 {
                        merge(
                            frame,
                            (usize::from(color.0 - 2), usize::from(color.1 - 2)),
                            edge.weight,
                        )?;
                    } else {
                        let enclosing = frame.enclosing.saturating_add(edge.weight);
                        if enclosing > DEPTH_LIMIT {
                            return Err(PublishedValueError::Limit("projection depth"));
                        }
                        let child_base = base(&rows[node])?;
                        if enclosing.saturating_add(child_base) > DEPTH_LIMIT {
                            return Err(PublishedValueError::Limit("projection depth"));
                        }
                        colors[node] = (1, 1);
                        frames.push(Frame {
                            node,
                            next: offsets[node],
                            end: offsets[node + 1],
                            enclosing,
                            intrinsic: child_base,
                            complete: child_base,
                            incoming: edge.weight,
                        });
                    }
                }
            }
        }
    }
    Ok(maximum)
}

#[cfg(test)]
#[path = "projection_depth_tests.rs"]
mod tests;
