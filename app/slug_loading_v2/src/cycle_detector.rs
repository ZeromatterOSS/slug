/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Request-scoped lazy detection for recursive `.bzl` loads.
//!
//! This is a deliberately small V2 adaptation of Buck2's
//! `buck2_util::cycle_detector::LazyCycleDetector`. Modern DICE exposes user
//! cycle-detector events, but does not resolve this loading-key cycle itself.
//! The detector records only `BzlModuleEvalKey` nodes, waits for the active
//! graph to become idle, and then releases blocked dependency waits with the
//! discovered cycle.

use std::fmt;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use dice::DynKey;
use dice::UserCycleDetector;
use dice::UserCycleDetectorGuard;
use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use crate::keys::BzlModuleEvalKey;

/// Create the detector that must be installed in one loading-capable DICE
/// request's `UserComputationData`.
///
/// Construct it inside an active Tokio runtime and never reuse it across
/// requests.
pub fn bzl_load_cycle_detector() -> Arc<dyn UserCycleDetector> {
    Arc::new(BzlLoadCycleDetector::new())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BzlLoadCycle {
    pub(crate) path: Arc<[BzlModuleEvalKey]>,
    pub(crate) keys: Arc<[BzlModuleEvalKey]>,
}

impl BzlLoadCycle {
    fn new(path: Vec<BzlModuleEvalKey>, keys: Vec<BzlModuleEvalKey>) -> Self {
        Self {
            path: path.into(),
            keys: keys.into(),
        }
    }
}

/// The typed guard retrieved from `DiceComputations` while evaluating a bzl
/// module. It is intentionally not a process-global singleton.
pub(crate) struct BzlLoadCycleGuard {
    key: BzlModuleEvalKey,
    sender: mpsc::UnboundedSender<Event>,
    receiver: Mutex<oneshot::Receiver<BzlLoadCycle>>,
}

impl BzlLoadCycleGuard {
    /// Race a single child-module wait against a detected cycle. A module
    /// evaluates its direct loads sequentially, so each guard has at most one
    /// outstanding call at a time.
    pub(crate) async fn guard_this<R, F>(&self, future: F) -> Result<R, BzlLoadCycle>
    where
        F: Future<Output = R>,
    {
        let mut receiver = self.receiver.lock().await;
        tokio::select! {
            value = future => Ok(value),
            cycle = &mut *receiver => Err(cycle.unwrap_or_else(|_| BzlLoadCycle::new(Vec::new(), Vec::new()))),
        }
    }
}

impl UserCycleDetectorGuard for BzlLoadCycleGuard {
    fn add_edge(&self, key: &DynKey) {
        if let Some(key) = key.downcast_ref::<BzlModuleEvalKey>() {
            let _ignored = self.sender.send(Event::Edge(self.key.clone(), key.clone()));
        }
    }

    fn type_name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

/// Buck2-derived evented detector. It receives DICE key start/finish events
/// and explicitly records only `.bzl`-to-`.bzl` dependency edges.
pub(crate) struct BzlLoadCycleDetector {
    sender: mpsc::UnboundedSender<Event>,
    #[allow(dead_code)]
    task: JoinHandle<()>,
}

impl fmt::Debug for BzlLoadCycleDetector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BzlLoadCycleDetector")
            .finish_non_exhaustive()
    }
}

impl BzlLoadCycleDetector {
    fn new() -> Self {
        Self::new_with_delay(Duration::from_millis(10))
    }

    fn new_with_delay(idle_delay: Duration) -> Self {
        let (sender, mut receiver) = mpsc::unbounded_channel();
        let task = tokio::spawn(async move {
            let mut state = CycleDetectorState::new();
            while let Some(event) = receiver.recv().await {
                state.handle_event(event);
                loop {
                    while let Ok(event) = receiver.try_recv() {
                        state.handle_event(event);
                    }
                    tokio::select! {
                        _ = tokio::time::sleep(idle_delay) => {
                            state.check_for_cycles();
                            break;
                        }
                        event = receiver.recv() => match event {
                            Some(event) => state.handle_event(event),
                            None => {
                                state.check_for_cycles();
                                return;
                            }
                        },
                    }
                }
            }
        });
        Self { sender, task }
    }

    fn start(&self, key: BzlModuleEvalKey) -> Arc<BzlLoadCycleGuard> {
        let (sender, receiver) = oneshot::channel();
        // The receiver lives for as long as DICE holds the computation guard.
        let _ignored = self.sender.send(Event::Started(key.clone(), sender));
        Arc::new(BzlLoadCycleGuard {
            key,
            sender: self.sender.clone(),
            receiver: Mutex::new(receiver),
        })
    }

    fn finish(&self, key: BzlModuleEvalKey) {
        let _ignored = self.sender.send(Event::Finished(key));
    }
}

impl UserCycleDetector for BzlLoadCycleDetector {
    fn start_computing_key(&self, key: &DynKey) -> Option<Arc<dyn UserCycleDetectorGuard>> {
        key.downcast_ref::<BzlModuleEvalKey>()
            .map(|key| self.start(key.clone()) as Arc<dyn UserCycleDetectorGuard>)
    }

    fn finished_computing_key(&self, key: &DynKey) {
        if let Some(key) = key.downcast_ref::<BzlModuleEvalKey>() {
            self.finish(key.clone());
        }
    }
}

enum Event {
    Started(BzlModuleEvalKey, oneshot::Sender<BzlLoadCycle>),
    Finished(BzlModuleEvalKey),
    Edge(BzlModuleEvalKey, BzlModuleEvalKey),
}

enum NodeState {
    Known,
    Finished,
    CycleDetected(BzlLoadCycle),
    Working {
        edges: SmallSet<u32>,
        sender: oneshot::Sender<BzlLoadCycle>,
    },
}

struct CycleDetectorState {
    node_ids: SmallMap<BzlModuleEvalKey, u32>,
    nodes: Vec<(BzlModuleEvalKey, NodeState)>,
    dirtied: SmallSet<u32>,
}

impl CycleDetectorState {
    fn new() -> Self {
        Self {
            node_ids: SmallMap::new(),
            nodes: Vec::new(),
            dirtied: SmallSet::new(),
        }
    }

    fn node_id(&mut self, key: &BzlModuleEvalKey) -> u32 {
        if let Some(id) = self.node_ids.get(key) {
            return *id;
        }
        let id = u32::try_from(self.nodes.len()).expect("too many active bzl load-cycle nodes");
        self.node_ids.insert(key.clone(), id);
        self.nodes.push((key.clone(), NodeState::Known));
        id
    }

    fn handle_event(&mut self, event: Event) {
        match event {
            Event::Started(key, sender) => {
                let id = self.node_id(&key);
                match &self.nodes[id as usize].1 {
                    NodeState::CycleDetected(cycle) => {
                        let _ignored = sender.send(cycle.clone());
                    }
                    NodeState::Known | NodeState::Finished | NodeState::Working { .. } => {
                        self.nodes[id as usize].1 = NodeState::Working {
                            edges: SmallSet::new(),
                            sender,
                        };
                    }
                }
            }
            Event::Finished(key) => {
                let id = self.node_id(&key);
                self.nodes[id as usize].1 = NodeState::Finished;
            }
            Event::Edge(from, to) => {
                let from = self.node_id(&from);
                let to = self.node_id(&to);
                if let NodeState::Working { edges, .. } = &mut self.nodes[from as usize].1 {
                    edges.insert(to);
                    self.dirtied.insert(from);
                }
            }
        }
    }

    fn is_working(&self, id: u32) -> bool {
        matches!(self.nodes[id as usize].1, NodeState::Working { .. })
    }

    fn edges(&self, id: u32) -> Vec<u32> {
        match &self.nodes[id as usize].1 {
            NodeState::Working { edges, .. } => edges.iter().copied().collect(),
            NodeState::Known | NodeState::Finished | NodeState::CycleDetected(_) => Vec::new(),
        }
    }

    fn check_for_cycles(&mut self) {
        let roots = std::mem::take(&mut self.dirtied);
        let mut visited = SmallSet::new();
        for root in roots {
            if !self.is_working(root) || visited.contains(&root) {
                continue;
            }
            let mut active = SmallSet::new();
            let mut stack = Vec::new();
            if let Some((path, cycle)) =
                self.find_cycle(root, &mut visited, &mut active, &mut stack)
            {
                self.notify_cycle(path, cycle);
            }
        }
    }

    fn find_cycle(
        &self,
        node: u32,
        visited: &mut SmallSet<u32>,
        active: &mut SmallSet<u32>,
        stack: &mut Vec<u32>,
    ) -> Option<(Vec<u32>, Vec<u32>)> {
        if !self.is_working(node) {
            return None;
        }
        if active.contains(&node) {
            let index = stack.iter().position(|id| *id == node).unwrap();
            return Some((stack[..index].to_vec(), stack[index..].to_vec()));
        }
        if !visited.insert(node) {
            return None;
        }
        active.insert(node);
        stack.push(node);
        for child in self.edges(node) {
            if let Some(cycle) = self.find_cycle(child, visited, active, stack) {
                return Some(cycle);
            }
        }
        stack.pop();
        active.shift_remove(&node);
        None
    }

    fn notify_cycle(&mut self, path: Vec<u32>, ids: Vec<u32>) {
        let cycle = BzlLoadCycle::new(
            path.iter()
                .map(|id| self.nodes[*id as usize].0.clone())
                .collect(),
            ids.iter()
                .map(|id| self.nodes[*id as usize].0.clone())
                .collect(),
        );
        for id in ids {
            let state = std::mem::replace(
                &mut self.nodes[id as usize].1,
                NodeState::CycleDetected(cycle.clone()),
            );
            if let NodeState::Working { sender, .. } = state {
                let _ignored = sender.send(cycle.clone());
            }
        }
    }
}
