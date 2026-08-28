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

use crate::ConfiguredNodeKey;
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
}

impl CycleNode {
    fn configured(&self) -> &ConfiguredNodeKey {
        match self {
            Self::Legacy(key) | Self::Observed(key) => key.node(),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ConfiguredAnalysisCycle(Arc<[CycleNode]>);

impl fmt::Display for ConfiguredAnalysisCycle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("configured alias cycle")?;
        for node in self.0.iter().map(CycleNode::configured) {
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
                let id = self.id(&key);
                self.nodes[id as usize].1 = NodeState::Finished;
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
