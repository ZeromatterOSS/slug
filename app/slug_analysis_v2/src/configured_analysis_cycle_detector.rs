/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::fmt;
use std::future::Future;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use dice::DynKey;
use dice::UserCycleDetector;
use dice::UserCycleDetectorGuard;
use dupe::Dupe;
use slug_loading_v2::bzl_load_cycle_detector;
use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use crate::dice::ConfiguredConditionKey;
use crate::dice::ConfiguredNodeAnalysisKey;
use crate::dice::ConfiguredNodeAnalysisObservationKey;

pub fn analysis_cycle_detector() -> Arc<dyn UserCycleDetector> {
    Arc::new(CombinedCycleDetector {
        bzl: bzl_load_cycle_detector(),
        configured: Arc::new(ConfiguredAnalysisCycleDetector::new()),
    })
}

struct CombinedCycleDetector {
    bzl: Arc<dyn UserCycleDetector>,
    configured: Arc<ConfiguredAnalysisCycleDetector>,
}

impl UserCycleDetector for CombinedCycleDetector {
    fn start_computing_key(&self, key: &DynKey) -> Option<Arc<dyn UserCycleDetectorGuard>> {
        let bzl = self.bzl.start_computing_key(key);
        let configured = self.configured.start_computing_key(key);
        assert!(
            bzl.is_none() || configured.is_none(),
            "cycle detector key families must be disjoint"
        );
        bzl.or(configured)
    }

    fn finished_computing_key(&self, key: &DynKey) {
        self.bzl.finished_computing_key(key);
        self.configured.finished_computing_key(key);
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
enum CycleNode {
    Legacy(ConfiguredNodeAnalysisKey),
    Observed(ConfiguredNodeAnalysisKey),
    Condition(ConfiguredConditionKey),
}

impl fmt::Display for CycleNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Legacy(key) | Self::Observed(key) => write!(f, "{}", key.node()),
            Self::Condition(key) => write!(f, "condition {}", key.target()),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ConfiguredAnalysisCycle(Arc<[CycleNode]>);

impl fmt::Display for ConfiguredAnalysisCycle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("configured alias cycle")?;
        for node in self.0.iter() {
            write!(formatter, " -> {node}")?;
        }
        Ok(())
    }
}

pub(crate) struct ConfiguredAnalysisCycleGuard {
    key: CycleNode,
    sender: mpsc::UnboundedSender<Event>,
    receiver: Mutex<Option<oneshot::Receiver<DetectedCycle>>>,
}

impl ConfiguredAnalysisCycleGuard {
    pub(crate) async fn guard_this<R, F>(&self, future: F) -> Result<R, ConfiguredAnalysisCycle>
    where
        F: Future<Output = R>,
    {
        let receiver = self
            .receiver
            .lock()
            .expect("configured cycle receiver lock poisoned")
            .take()
            .expect("configured alias guard is used once");
        tokio::pin!(receiver);
        tokio::select! {
            value = future => Ok(value),
            cycle = &mut receiver => Err(ConfiguredAnalysisCycle(
                cycle.map(|cycle| cycle.keys).unwrap_or_else(|_| Arc::from([])),
            )),
        }
    }
}

impl UserCycleDetectorGuard for ConfiguredAnalysisCycleGuard {
    fn add_edge(&self, key: &DynKey) {
        if let Some(child) = cycle_node(key) {
            let _ignored = self.sender.send(Event::Edge(self.key.clone(), child));
        }
    }

    fn type_name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

struct ConfiguredAnalysisCycleDetector {
    sender: mpsc::UnboundedSender<Event>,
    _task: JoinHandle<()>,
}

impl ConfiguredAnalysisCycleDetector {
    fn new() -> Self {
        let (sender, mut receiver) = mpsc::unbounded_channel();
        let task = tokio::spawn(async move {
            let mut state = DetectorState::default();
            while let Some(event) = receiver.recv().await {
                state.handle(event);
                loop {
                    while let Ok(event) = receiver.try_recv() {
                        state.handle(event);
                    }
                    tokio::select! {
                        _ = tokio::time::sleep(Duration::from_millis(10)) => {
                            state.check();
                            break;
                        }
                        event = receiver.recv() => match event {
                            Some(event) => state.handle(event),
                            None => {
                                state.check();
                                return;
                            }
                        },
                    }
                }
            }
        });
        Self {
            sender,
            _task: task,
        }
    }

    fn start(&self, key: CycleNode) -> Arc<ConfiguredAnalysisCycleGuard> {
        let (sender, receiver) = oneshot::channel();
        let _ignored = self.sender.send(Event::Started(key.clone(), sender));
        Arc::new(ConfiguredAnalysisCycleGuard {
            key,
            sender: self.sender.clone(),
            receiver: Mutex::new(Some(receiver)),
        })
    }

    fn finish(&self, key: CycleNode) {
        let _ignored = self.sender.send(Event::Finished(key));
    }
}

impl UserCycleDetector for ConfiguredAnalysisCycleDetector {
    fn start_computing_key(&self, key: &DynKey) -> Option<Arc<dyn UserCycleDetectorGuard>> {
        cycle_node(key).map(|key| self.start(key) as Arc<dyn UserCycleDetectorGuard>)
    }

    fn finished_computing_key(&self, key: &DynKey) {
        if let Some(key) = cycle_node(key) {
            self.finish(key);
        }
    }
}

fn cycle_node(key: &DynKey) -> Option<CycleNode> {
    key.downcast_ref::<ConfiguredNodeAnalysisKey>()
        .cloned()
        .map(CycleNode::Legacy)
        .or_else(|| {
            key.downcast_ref::<ConfiguredNodeAnalysisObservationKey>()
                .map(|key| {
                    CycleNode::Observed(
                        ConfiguredNodeAnalysisKey::new(key.workspace().dupe(), key.node().clone())
                            .expect("observed configured key retains structural identity"),
                    )
                })
        })
        .or_else(|| {
            key.downcast_ref::<ConfiguredConditionKey>()
                .cloned()
                .map(CycleNode::Condition)
        })
}

enum Event {
    Started(CycleNode, oneshot::Sender<DetectedCycle>),
    Finished(CycleNode),
    Edge(CycleNode, CycleNode),
}

enum NodeState {
    Known,
    Finished,
    Cycle(DetectedCycle),
    Working {
        edges: SmallSet<u32>,
        sender: oneshot::Sender<DetectedCycle>,
    },
}

#[derive(Clone)]
struct DetectedCycle {
    keys: Arc<[CycleNode]>,
}

#[derive(Default)]
struct DetectorState {
    ids: SmallMap<CycleNode, u32>,
    nodes: Vec<(CycleNode, NodeState)>,
    dirty: SmallSet<u32>,
}

impl DetectorState {
    fn id(&mut self, key: &CycleNode) -> u32 {
        if let Some(id) = self.ids.get(key) {
            return *id;
        }
        let id = u32::try_from(self.nodes.len()).expect("too many configured cycle nodes");
        self.ids.insert(key.clone(), id);
        self.nodes.push((key.clone(), NodeState::Known));
        id
    }

    fn handle(&mut self, event: Event) {
        match event {
            Event::Started(key, sender) => {
                let id = self.id(&key);
                if let NodeState::Cycle(cycle) = &self.nodes[id as usize].1 {
                    let _ignored = sender.send(cycle.clone());
                } else {
                    self.nodes[id as usize].1 = NodeState::Working {
                        edges: SmallSet::new(),
                        sender,
                    };
                }
            }
            Event::Finished(key) => {
                if let Some(id) = self.ids.get(&key).copied() {
                    self.nodes[id as usize].1 = NodeState::Finished;
                    self.prune_finished();
                }
            }
            Event::Edge(from, to) => {
                let from = self.id(&from);
                let to = self.id(&to);
                if let NodeState::Working { edges, .. } = &mut self.nodes[from as usize].1 {
                    edges.insert(to);
                    self.dirty.insert(from);
                }
            }
        }
    }

    // A live-cache coordinator can serve continuously overlapping requests.
    // Keep only active computations and their unresolved immediate targets;
    // compaction also bounds storage when the graph never becomes fully idle.
    fn prune_finished(&mut self) {
        let mut referenced = SmallSet::new();
        for (_, state) in &self.nodes {
            if let NodeState::Working { edges, .. } = state {
                referenced.extend(edges.iter().copied());
            }
        }
        let mut remap = SmallMap::new();
        for (old, (_, state)) in self.nodes.iter().enumerate() {
            let retain = match state {
                NodeState::Working { .. } | NodeState::Cycle(_) => true,
                NodeState::Known => referenced.contains(&(old as u32)),
                NodeState::Finished => referenced.contains(&(old as u32)),
            };
            if retain {
                remap.insert(old as u32, remap.len() as u32);
            }
        }
        let old_nodes = std::mem::take(&mut self.nodes);
        self.ids.clear();
        for (old, (key, mut state)) in old_nodes.into_iter().enumerate() {
            let Some(new) = remap.get(&(old as u32)).copied() else {
                continue;
            };
            if matches!(state, NodeState::Finished) {
                // This placeholder belongs to a still-active parent's logical
                // dependency, which may already await a cancelled replacement.
                state = NodeState::Known;
            }
            if let NodeState::Working { edges, .. } = &mut state {
                *edges = edges
                    .iter()
                    .filter_map(|id| remap.get(id).copied())
                    .collect();
            }
            self.ids.insert(key.clone(), new);
            self.nodes.push((key, state));
        }
        self.dirty = self
            .dirty
            .iter()
            .filter_map(|id| remap.get(id).copied())
            .collect();
    }

    fn working(&self, id: u32) -> bool {
        matches!(self.nodes[id as usize].1, NodeState::Working { .. })
    }

    fn check(&mut self) {
        let roots = std::mem::take(&mut self.dirty);
        let mut visited = SmallSet::new();
        for root in roots {
            let mut active = SmallSet::new();
            let mut stack = Vec::new();
            if self.working(root)
                && let Some(cycle) = self.find(root, &mut visited, &mut active, &mut stack)
            {
                self.notify(cycle);
            }
        }
    }

    fn find(
        &self,
        node: u32,
        visited: &mut SmallSet<u32>,
        active: &mut SmallSet<u32>,
        stack: &mut Vec<u32>,
    ) -> Option<Vec<u32>> {
        if !self.working(node) {
            return None;
        }
        if active.contains(&node) {
            return Some(stack[stack.iter().position(|id| *id == node).unwrap()..].to_vec());
        }
        if !visited.insert(node) {
            return None;
        }
        active.insert(node);
        stack.push(node);
        let edges = match &self.nodes[node as usize].1 {
            NodeState::Working { edges, .. } => edges.iter().copied().collect::<Vec<_>>(),
            _ => Vec::new(),
        };
        for child in edges {
            if let Some(cycle) = self.find(child, visited, active, stack) {
                return Some(cycle);
            }
        }
        stack.pop();
        active.shift_remove(&node);
        None
    }

    fn notify(&mut self, ids: Vec<u32>) {
        let cycle = DetectedCycle {
            keys: ids
                .iter()
                .map(|id| self.nodes[*id as usize].0.clone())
                .collect(),
        };
        for id in ids {
            let state = std::mem::replace(
                &mut self.nodes[id as usize].1,
                NodeState::Cycle(cycle.clone()),
            );
            if let NodeState::Working { sender, .. } = state {
                let _ignored = sender.send(cycle.clone());
            }
        }
    }
}

#[cfg(test)]
mod lifecycle_tests {
    use slug_identity_v2::CanonicalLabel;
    use slug_workspace_v2::NormalizedAbsolutePath;

    use super::*;
    use crate::key::ConfiguredNodeKey;

    fn node(name: &str) -> CycleNode {
        CycleNode::Legacy(
            ConfiguredNodeAnalysisKey::new(
                NormalizedAbsolutePath::new("/configured-cycle-test").unwrap(),
                ConfiguredNodeKey::null(CanonicalLabel::parse(&format!("@@//:{name}")).unwrap()),
            )
            .unwrap(),
        )
    }

    #[test]
    fn overlapping_completed_requests_do_not_retain_history() {
        let mut state = DetectorState::default();
        let anchor = node("anchor");
        let (sender, _anchor_receiver) = oneshot::channel();
        state.handle(Event::Started(anchor.clone(), sender));
        for index in 0..1000 {
            let current = node(&format!("request{index}"));
            let missing = node(&format!("target{index}"));
            let (sender, _receiver) = oneshot::channel();
            state.handle(Event::Started(current.clone(), sender));
            state.handle(Event::Edge(current.clone(), missing));
            state.handle(Event::Finished(current));
            assert_eq!(state.nodes.len(), 1);
            assert_eq!(state.ids.len(), 1);
            assert!(state.dirty.len() <= 1);
        }
        state.handle(Event::Finished(anchor));
        assert!(state.nodes.is_empty());
        assert!(state.ids.is_empty());
        assert!(state.dirty.is_empty());
    }

    #[tokio::test]
    async fn pending_replacement_keeps_incoming_dependency_but_retires_old_outgoing_edges() {
        let mut state = DetectorState::default();
        let parent = node("parent");
        let target = node("target");
        let retired_dependency = node("retired_dependency");
        let (sender, mut parent_receiver) = oneshot::channel();
        state.handle(Event::Started(parent.clone(), sender));
        let (sender, _old_receiver) = oneshot::channel();
        state.handle(Event::Started(target.clone(), sender));
        state.handle(Event::Edge(parent.clone(), target.clone()));
        state.handle(Event::Edge(target.clone(), retired_dependency));
        state.handle(Event::Finished(target.clone()));
        assert_eq!(state.nodes.len(), 2);
        state.check();
        assert!(matches!(
            parent_receiver.try_recv(),
            Err(oneshot::error::TryRecvError::Empty)
        ));
        let (sender, replacement_receiver) = oneshot::channel();
        state.handle(Event::Started(target.clone(), sender));
        state.handle(Event::Edge(target.clone(), parent.clone()));
        state.check();
        for receiver in [parent_receiver, replacement_receiver] {
            let cycle = receiver.await.unwrap();
            assert_eq!(cycle.keys.len(), 2);
            assert!(cycle.keys.contains(&parent) && cycle.keys.contains(&target));
        }
        state.handle(Event::Finished(target));
        state.handle(Event::Finished(parent));
        assert!(state.nodes.is_empty());
    }

    #[tokio::test]
    async fn compaction_preserves_live_cycles_and_replacement_is_not_poisoned() {
        let mut state = DetectorState::default();
        let a = node("a");
        let b = node("b");
        let removed = node("removed");
        let (sender, _receiver) = oneshot::channel();
        state.handle(Event::Started(removed.clone(), sender));
        let (sender, receiver_a) = oneshot::channel();
        state.handle(Event::Started(a.clone(), sender));
        state.handle(Event::Edge(a.clone(), b.clone()));
        let (sender, receiver_b) = oneshot::channel();
        state.handle(Event::Started(b.clone(), sender));
        state.handle(Event::Edge(b.clone(), a.clone()));
        state.handle(Event::Finished(removed));
        state.check();
        for receiver in [receiver_a, receiver_b] {
            let cycle = receiver.await.unwrap();
            assert_eq!(cycle.keys.len(), 2);
            assert!(cycle.keys.contains(&a) && cycle.keys.contains(&b));
        }
        state.handle(Event::Finished(a.clone()));
        state.handle(Event::Finished(b));
        assert!(state.nodes.is_empty());
        let (sender, mut replacement) = oneshot::channel();
        state.handle(Event::Started(a.clone(), sender));
        state.check();
        assert!(matches!(
            replacement.try_recv(),
            Err(oneshot::error::TryRecvError::Empty)
        ));
        state.handle(Event::Finished(a));
        assert!(state.nodes.is_empty());
    }
}
