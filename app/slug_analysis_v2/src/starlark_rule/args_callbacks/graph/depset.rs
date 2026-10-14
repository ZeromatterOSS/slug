//! Typed transitive depset links use explicit frames, not Rust call depth.
use slug_build_api_v2::AnalysisValueKind;
use slug_build_api_v2::AnalysisValueType;
use slug_build_api_v2::published_value::*;
use slug_loading_v2::provider::StarlarkDepset;
use slug_loading_v2::provider::StarlarkDepsetSuccessorGen as Successor;

use super::*;

enum Frame<'v> {
    Enter(Value<'v>, Option<PublishedNodeId>),
    Exit {
        value: Value<'v>,
        id: PublishedNodeId,
        order: slug_build_api_v2::DepsetOrder,
        occurrence: slug_build_api_v2::AnalysisDepsetOccurrence,
        element_type: AnalysisValueType,
        logical_depth: usize,
        successors: Vec<Successor<Value<'v>>>,
    },
}
impl<'v> PublicationBuilder<'v> {
    fn depset_push(
        &mut self,
        frames: &mut Vec<Frame<'v>>,
        credit: &mut storage::PeakCredit,
        frame: Frame<'v>,
    ) -> Result<(), String> {
        credit.vector::<Frame<'v>>(&mut self.graph, frames.len().saturating_add(1))?;
        frames.push(frame);
        Ok(())
    }
    pub(super) fn depset(
        &mut self,
        root: Value<'v>,
        root_id: PublishedNodeId,
        lowerer: &mut AnalysisValueLowerer<'v>,
        depth: usize,
    ) -> Result<bool, String> {
        if StarlarkDepset::depth_from_value(root).is_none() {
            return Ok(false);
        }
        let mut frames = Vec::new();
        let mut active = UnorderedMap::new();
        let mut frame_credit = storage::PeakCredit::default();
        let mut active_credit = storage::PeakCredit::default();
        self.depset_push(
            &mut frames,
            &mut frame_credit,
            Frame::Enter(root, Some(root_id)),
        )?;
        while let Some(frame) = frames.pop() {
            self.step(depth)?;
            match frame {
                Frame::Enter(value, root_id) => {
                    if active.contains_key(&value.identity()) {
                        return Err("cyclic typed depset graph".into());
                    }
                    let id = if let Some(id) = root_id {
                        id
                    } else if let Some(id) = self.values.get(&value.identity()) {
                        self.graph
                            .node(*id)
                            .map_err(|_| "incomplete typed depset successor")?;
                        continue;
                    } else {
                        self.reserve_value(value)?
                    };
                    let logical_depth = StarlarkDepset::depth_from_value(value)
                        .ok_or("transitive successor is not an actual depset")?;
                    if logical_depth > slug_build_api_v2::depset::MAX_DEPTH {
                        return Err("typed depset logical depth exceeds 3500".into());
                    }
                    let mut successor_count = 0;
                    StarlarkDepset::visit_metadata_from_value(value, &mut |name, count, _| {
                        successor_count = count;
                        self.membership(count)?;
                        self.graph
                            .charge_bytes(
                                count.saturating_mul(std::mem::size_of::<Successor<Value<'v>>>()),
                            )
                            .map_err(|e| e.to_string())?;
                        if let Some(name) = name {
                            self.graph
                                .charge_bytes(name.len())
                                .map_err(|e| e.to_string())?;
                        }
                        Ok(())
                    })?;
                    // Hidden native payloads are audited before even cloning
                    // the retained handle or allocating an AnalysisValue wrapper.
                    if successor_count == 0 {
                        if let Some(retained) = StarlarkDepset::retained_from_value(value) {
                            if !retained.is_empty() {
                                let atom = self
                                    .graph
                                    .depset_atom_borrowed(retained)
                                    .map_err(|e| e.to_string())?;
                                self.set(id, PublishedNode::Native(atom))?;
                                continue;
                            }
                        }
                    }
                    let (order, _, occurrence, retained, logical_depth, successors) =
                        StarlarkDepset::parts_from_value(value)
                            .ok_or("missing actual depset parts")?;
                    let element_type = retained
                        .as_ref()
                        .map_or(AnalysisValueType::Empty, |v| v.element_type());
                    active_credit.map::<ValueIdentity<'v>, ()>(
                        &mut self.graph,
                        active.len().saturating_add(1),
                    )?;
                    active.insert(value.identity(), ());
                    // Descendant frames borrow the already admitted snapshot.
                    // Charge the temporary child work list before copying handles.
                    self.graph
                        .charge_bytes(
                            successors
                                .len()
                                .saturating_mul(std::mem::size_of::<Value<'v>>()),
                        )
                        .map_err(|e| e.to_string())?;
                    let mut children = Vec::with_capacity(successors.len());
                    for successor in successors.iter().rev() {
                        if let Successor::Transitive(child) = successor {
                            children.push(*child);
                        }
                    }
                    self.depset_push(
                        &mut frames,
                        &mut frame_credit,
                        Frame::Exit {
                            value,
                            id,
                            order,
                            occurrence,
                            element_type,
                            logical_depth,
                            successors,
                        },
                    )?;
                    for child in children {
                        self.depset_push(
                            &mut frames,
                            &mut frame_credit,
                            Frame::Enter(child, None),
                        )?;
                    }
                }
                Frame::Exit {
                    value,
                    id,
                    order,
                    occurrence,
                    mut element_type,
                    logical_depth,
                    successors,
                } => {
                    self.graph
                        .charge_bytes(
                            successors
                                .len()
                                .saturating_mul(std::mem::size_of::<PublishedDepsetInput>()),
                        )
                        .map_err(|e| e.to_string())?;
                    let mut edges = Vec::with_capacity(successors.len());
                    for successor in successors {
                        self.step(depth)?;
                        let (node, transitive) = match successor {
                            Successor::Direct(child) => {
                                (self.value(child, lowerer, depth + 1)?, false)
                            }
                            Successor::Transitive(child) => (
                                *self
                                    .values
                                    .get(&child.identity())
                                    .ok_or("missing completed depset child")?,
                                true,
                            ),
                        };
                        if element_type == AnalysisValueType::Empty {
                            element_type = if transitive {
                                match self.graph.node(node).map_err(|e| e.to_string())? {
                                    PublishedNode::Depset(row) => row.element_type,
                                    PublishedNode::Native(atom) => match atom.value().kind() {
                                        AnalysisValueKind::Depset(child) => child.element_type(),
                                        _ => return Err("invalid native transitive depset".into()),
                                    },
                                    _ => return Err("invalid transitive depset".into()),
                                }
                            } else {
                                self.graph.value_type(node).map_err(|e| e.to_string())?
                            };
                        }
                        edges.push(if transitive {
                            PublishedDepsetInput::Transitive(PublishedEdge::Local(node))
                        } else {
                            PublishedDepsetInput::Direct(PublishedEdge::Local(node))
                        });
                    }
                    self.set(
                        id,
                        PublishedNode::Depset(PublishedDepset {
                            order,
                            element_type,
                            depth: logical_depth,
                            occurrence,
                            successors: edges.into(),
                        }),
                    )?;
                    active.remove(&value.identity());
                }
            }
        }
        Ok(true)
    }
}
