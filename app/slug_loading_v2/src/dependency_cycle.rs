//! Immediate mixed Loading dependencies, scoped to one actual live DICE context.
//! Graph locks cover only synchronous bookkeeping; waits use cloned watch receivers.

use std::fmt;
use std::future::Future;
use std::sync::Arc;
use std::sync::Mutex;

use allocative::Allocative;
use dice::DiceComputations;
use dice::DynKey;
use dice::Key;
use dice::UserCycleDetectorGuard;
use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;
use tokio::sync::watch;

/// Every variant stores the complete concrete key, including mode and source route.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
enum Node {
    SelectedLabelSource(Arc<slug_bzlmod_v2::SelectedLabelSourceKey>),
    LegacyBzl(Arc<crate::keys::BzlModuleEvalKey>),
    HostBzl(Arc<crate::bzl_module::HostBzlModuleEvalKey>),
    ObservedHostBzl(Arc<crate::bzl_module::HostBzlModuleObservationKey>),
    ExternalBzl(Arc<crate::bzl_module::ExternalBzlModuleEvalKey>),
    ObservedExternalBzl(Arc<crate::bzl_module::ExternalBzlModuleObservationKey>),
    PureOwner(Arc<crate::module_extension::HostSelectedExtensionOwnerPureKey>),
    ObservedPureOwner(Arc<crate::module_extension::HostSelectedExtensionOwnerPureObservationKey>),
    InnateOwner(Arc<crate::module_extension_innate_repository::HostPureInnateRepositoryOwnerKey>),
    ObservedInnateOwner(Arc<crate::module_extension_innate_repository::HostPureInnateRepositoryOwnerObservationKey>),
    Certificate(Arc<crate::module_extension_repository_validation::HostSelectedExtensionOwnerCertificateKey>),
    ObservedCertificate(Arc<crate::module_extension_repository_validation::HostSelectedExtensionOwnerCertificateObservationKey>),
    Definition(Arc<crate::generated_repository_definition::HostGeneratedRepositoryDefinitionKey>),
    ObservedDefinition(Arc<crate::generated_repository_definition::HostGeneratedRepositoryDefinitionObservationKey>),
    Route(Arc<crate::canonical_repository_route::HostCanonicalRepositoryRouteKey>),
    ObservedRoute(Arc<crate::canonical_repository_route::HostCanonicalRepositoryRouteObservationKey>),
    LoadRoute(Arc<crate::canonical_repository_load_route::HostCanonicalRepositoryLoadRouteKey>),
    ObservedLoadRoute(Arc<crate::canonical_repository_load_route::HostCanonicalRepositoryLoadRouteObservationKey>),
    FileEffect(Arc<crate::module_extension_repository_file_effect::HostSelectedRepositoryFileEffectKey>),
    ObservedFileEffect(Arc<crate::module_extension_repository_file_effect::HostSelectedRepositoryFileEffectObservationKey>),
}

impl Node {
    fn from_dyn(key: &DynKey) -> Option<Self> {
        if let Some(key) = key.downcast_ref::<slug_bzlmod_v2::SelectedLabelSourceKey>() {
            return Some(Self::SelectedLabelSource(Arc::new(key.clone())));
        }
        if let Some(key) = key.downcast_ref::<crate::keys::BzlModuleEvalKey>() {
            return Some(Self::LegacyBzl(Arc::new(key.clone())));
        }
        if let Some(key) = key.downcast_ref::<crate::bzl_module::HostBzlModuleEvalKey>() {
            return Some(Self::HostBzl(Arc::new(key.clone())));
        }
        if let Some(key) = key.downcast_ref::<crate::bzl_module::HostBzlModuleObservationKey>() {
            return Some(Self::ObservedHostBzl(Arc::new(key.clone())));
        }
        if let Some(key) = key.downcast_ref::<crate::bzl_module::ExternalBzlModuleEvalKey>() {
            return Some(Self::ExternalBzl(Arc::new(key.clone())));
        }
        if let Some(key) = key.downcast_ref::<crate::bzl_module::ExternalBzlModuleObservationKey>()
        {
            return Some(Self::ObservedExternalBzl(Arc::new(key.clone())));
        }
        if let Some(key) =
            key.downcast_ref::<crate::module_extension::HostSelectedExtensionOwnerPureKey>()
        {
            return Some(Self::PureOwner(Arc::new(key.clone())));
        }
        if let Some(key) = key
            .downcast_ref::<crate::module_extension::HostSelectedExtensionOwnerPureObservationKey>()
        {
            return Some(Self::ObservedPureOwner(Arc::new(key.clone())));
        }
        if let Some(key) = key.downcast_ref::<crate::module_extension_innate_repository::HostPureInnateRepositoryOwnerKey>() {
            return Some(Self::InnateOwner(Arc::new(key.clone())));
        }
        if let Some(key) = key.downcast_ref::<crate::module_extension_innate_repository::HostPureInnateRepositoryOwnerObservationKey>() {
            return Some(Self::ObservedInnateOwner(Arc::new(key.clone())));
        }
        if let Some(key) = key.downcast_ref::<crate::module_extension_repository_validation::HostSelectedExtensionOwnerCertificateKey>() {
            return Some(Self::Certificate(Arc::new(key.clone())));
        }
        if let Some(key) = key.downcast_ref::<crate::module_extension_repository_validation::HostSelectedExtensionOwnerCertificateObservationKey>() {
            return Some(Self::ObservedCertificate(Arc::new(key.clone())));
        }
        if let Some(key) = key.downcast_ref::<crate::generated_repository_definition::HostGeneratedRepositoryDefinitionKey>() {
            return Some(Self::Definition(Arc::new(key.clone())));
        }
        if let Some(key) = key.downcast_ref::<crate::generated_repository_definition::HostGeneratedRepositoryDefinitionObservationKey>() {
            return Some(Self::ObservedDefinition(Arc::new(key.clone())));
        }
        if let Some(key) =
            key.downcast_ref::<crate::canonical_repository_route::HostCanonicalRepositoryRouteKey>()
        {
            return Some(Self::Route(Arc::new(key.clone())));
        }
        if let Some(key) = key.downcast_ref::<crate::canonical_repository_route::HostCanonicalRepositoryRouteObservationKey>() {
            return Some(Self::ObservedRoute(Arc::new(key.clone())));
        }
        if let Some(key) = key.downcast_ref::<crate::canonical_repository_load_route::HostCanonicalRepositoryLoadRouteKey>() {
            return Some(Self::LoadRoute(Arc::new(key.clone())));
        }
        if let Some(key) = key.downcast_ref::<crate::canonical_repository_load_route::HostCanonicalRepositoryLoadRouteObservationKey>() {
            return Some(Self::ObservedLoadRoute(Arc::new(key.clone())));
        }
        if let Some(key) = key.downcast_ref::<crate::module_extension_repository_file_effect::HostSelectedRepositoryFileEffectKey>() {
            return Some(Self::FileEffect(Arc::new(key.clone())));
        }
        if let Some(key) = key.downcast_ref::<crate::module_extension_repository_file_effect::HostSelectedRepositoryFileEffectObservationKey>() {
            return Some(Self::ObservedFileEffect(Arc::new(key.clone())));
        }
        None
    }

    fn pure_bzl_family(&self) -> Option<u8> {
        match self {
            Self::LegacyBzl(_) => Some(0),
            Self::HostBzl(_) => Some(1),
            Self::ObservedHostBzl(_) => Some(2),
            Self::ExternalBzl(_) => Some(3),
            Self::ObservedExternalBzl(_) => Some(4),
            _ => None,
        }
    }
}

impl fmt::Display for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SelectedLabelSource(key) => write!(f, "SelectedLabelSource({key})"),
            Self::LegacyBzl(key) => write!(f, "LegacyBzl({key})"),
            Self::HostBzl(key) => write!(f, "HostBzl({key})"),
            Self::ObservedHostBzl(key) => write!(f, "ObservedHostBzl({key})"),
            Self::ExternalBzl(key) => write!(f, "ExternalBzl({key})"),
            Self::ObservedExternalBzl(key) => write!(f, "ObservedExternalBzl({key})"),
            Self::PureOwner(key) => write!(f, "PureOwner({key})"),
            Self::ObservedPureOwner(key) => write!(f, "ObservedPureOwner({key})"),
            Self::InnateOwner(key) => write!(f, "InnateOwner({key})"),
            Self::ObservedInnateOwner(key) => write!(f, "ObservedInnateOwner({key})"),
            Self::Certificate(key) => write!(f, "Certificate({key})"),
            Self::ObservedCertificate(key) => write!(f, "ObservedCertificate({key})"),
            Self::Definition(key) => write!(f, "Definition({key})"),
            Self::ObservedDefinition(key) => write!(f, "ObservedDefinition({key})"),
            Self::Route(key) => write!(f, "Route({key})"),
            Self::ObservedRoute(key) => write!(f, "ObservedRoute({key})"),
            Self::LoadRoute(key) => write!(f, "LoadRoute({key})"),
            Self::ObservedLoadRoute(key) => write!(f, "ObservedLoadRoute({key})"),
            Self::FileEffect(key) => write!(f, "FileEffect({key})"),
            Self::ObservedFileEffect(key) => write!(f, "ObservedFileEffect({key})"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct DependencyCycle {
    nodes: Arc<[Arc<Node>]>,
}

struct BoundedWriter<'a, W> {
    writer: &'a mut W,
    remaining: usize,
    overflow: bool,
}

impl<W: fmt::Write> fmt::Write for BoundedWriter<'_, W> {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        if let Some((end, _)) = text.char_indices().nth(self.remaining) {
            self.writer.write_str(&text[..end])?;
            self.remaining = 0;
            self.overflow = true;
            // Stop the source formatter before it visits the rest of the key.
            Err(fmt::Error)
        } else {
            self.writer.write_str(text)?;
            self.remaining -= text.chars().count();
            Ok(())
        }
    }
}

fn write_bounded(
    writer: &mut impl fmt::Write,
    value: &impl fmt::Display,
    limit: usize,
) -> fmt::Result {
    let mut bounded = BoundedWriter {
        writer,
        remaining: limit,
        overflow: false,
    };
    let result = fmt::write(&mut bounded, format_args!("{value}"));
    if bounded.overflow {
        bounded.writer.write_str("...")
    } else {
        result
    }
}

impl fmt::Display for DependencyCycle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("cycle detected in generated repository dependencies: ")?;
        for (index, node) in self.nodes.iter().take(12).enumerate() {
            if index != 0 {
                f.write_str(" -> ")?;
            }
            // Diagnostic presentation is bounded; node equality retains the complete key.
            write_bounded(f, node, 256)?;
        }
        if self.nodes.len() > 12 {
            f.write_str(" -> ...")?;
        }
        Ok(())
    }
}

impl std::error::Error for DependencyCycle {}

#[derive(Debug)]
pub(crate) enum DependencyComputeError {
    // Preserve the concrete DiceError as a source, without a new dependency.
    Dice(anyhow::Error),
    Cycle(DependencyCycle),
}

impl fmt::Display for DependencyComputeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Dice(error) => error.fmt(f),
            Self::Cycle(cycle) => cycle.fmt(f),
        }
    }
}

impl std::error::Error for DependencyComputeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Dice(error) => Some(error.as_ref()),
            Self::Cycle(cycle) => Some(cycle),
        }
    }
}

struct Attempt {
    id: u64,
    edges: SmallSet<Arc<Node>>,
    notification: watch::Sender<Option<DependencyCycle>>,
}

#[derive(Default)]
struct State {
    next_id: u64,
    active: SmallMap<Arc<Node>, Attempt>,
}

impl State {
    fn retire(&mut self, node: &Arc<Node>, id: u64) {
        if self
            .active
            .get(node)
            .is_some_and(|attempt| attempt.id == id)
        {
            self.active.shift_remove(node);
            // Incoming edges belong to their live source attempts. A child finish
            // may be a cancelled predecessor handoff, after its parent has already
            // registered the replacement wait. Keep those logical targets until
            // the source retires; path traversal still requires an active child.
        }
    }

    fn path(&self, start: &Arc<Node>, goal: &Arc<Node>) -> Option<Vec<Arc<Node>>> {
        let mut visited = SmallSet::new();
        let mut parents: SmallMap<Arc<Node>, Arc<Node>> = SmallMap::new();
        let mut pending = vec![start.clone()];
        visited.insert(start.clone());
        while let Some(node) = pending.pop() {
            let Some(attempt) = self.active.get(&node) else {
                continue;
            };
            if &node == goal {
                let mut path = vec![node.clone()];
                let mut current = node;
                while &current != start {
                    current = parents
                        .get(&current)
                        .expect("visited node has a parent")
                        .clone();
                    path.push(current.clone());
                }
                path.reverse();
                return Some(path);
            }
            for child in &attempt.edges {
                if visited.insert(child.clone()) {
                    parents.insert(child.clone(), node.clone());
                    pending.push(child.clone());
                }
            }
        }
        None
    }

    fn notifications(
        &self,
        source: &Arc<Node>,
    ) -> Vec<(watch::Sender<Option<DependencyCycle>>, DependencyCycle)> {
        let mut notifications = Vec::new();
        let Some(attempt) = self.active.get(source) else {
            return notifications;
        };
        for child in &attempt.edges {
            let Some(mut nodes) = self.path(child, source) else {
                continue;
            };
            let family = source.pure_bzl_family();
            if family.is_some() && nodes.iter().all(|node| node.pure_bzl_family() == family) {
                continue; // Existing isolated .bzl detector owns these projections.
            }
            nodes.insert(0, source.clone());
            let cycle = DependencyCycle {
                nodes: nodes.into(),
            };
            let mut sent = SmallSet::new();
            for node in cycle.nodes.iter() {
                if sent.insert(node.clone()) {
                    notifications.push((
                        self.active.get(node).unwrap().notification.clone(),
                        cycle.clone(),
                    ));
                }
            }
        }
        notifications
    }
}

#[derive(Default)]
pub(crate) struct DependencyCycleGraph {
    state: Mutex<State>,
}

impl DependencyCycleGraph {
    pub(crate) fn start(self: &Arc<Self>, key: &DynKey) -> Option<DependencyCycleGuard> {
        let node = Arc::new(Node::from_dyn(key)?);
        let (notification, receiver) = watch::channel(None);
        let id = {
            let mut state = self.state.lock().expect("dependency graph lock poisoned");
            // DICE serializes same-key/version workers (worker awaits any cancelled predecessor).
            // Exact IDs also make a retained old guard harmless after callback/replacement.
            if let Some(old_id) = state.active.get(&node).map(|attempt| attempt.id) {
                state.retire(&node, old_id);
            }
            let id = state.next_id;
            state.next_id = id.checked_add(1).expect("dependency attempt ID exhausted");
            state.active.insert(
                node.clone(),
                Attempt {
                    id,
                    edges: SmallSet::new(),
                    notification,
                },
            );
            // Incoming pending edges now resolve to this attempt. Its empty outgoing set
            // cannot close a cycle until its first immediate add_edge call.
            id
        };
        Some(DependencyCycleGuard {
            graph: self.clone(),
            node,
            id,
            receiver,
        })
    }

    pub(crate) fn finish(&self, key: &DynKey) {
        let Some(node) = Node::from_dyn(key).map(Arc::new) else {
            return;
        };
        let mut state = self.state.lock().expect("dependency graph lock poisoned");
        if let Some(id) = state.active.get(&node).map(|attempt| attempt.id) {
            state.retire(&node, id);
        }
    }

    fn notify(notifications: Vec<(watch::Sender<Option<DependencyCycle>>, DependencyCycle)>) {
        for (sender, cycle) in notifications {
            sender.send_replace(Some(cycle));
        }
    }
}

pub(crate) struct DependencyCycleGuard {
    graph: Arc<DependencyCycleGraph>,
    node: Arc<Node>,
    id: u64,
    receiver: watch::Receiver<Option<DependencyCycle>>,
}

impl DependencyCycleGuard {
    pub(crate) async fn guard_this<R>(
        &self,
        future: impl Future<Output = R>,
    ) -> Result<R, DependencyCycle> {
        let mut receiver = self.receiver.clone();
        let notification = async {
            loop {
                if let Some(cycle) = receiver.borrow().clone() {
                    return cycle;
                }
                if receiver.changed().await.is_err() {
                    std::future::pending::<()>().await;
                }
            }
        };
        tokio::pin!(notification);
        tokio::select! { biased; cycle = &mut notification => Err(cycle), value = future => Ok(value) }
    }
}

impl UserCycleDetectorGuard for DependencyCycleGuard {
    fn add_edge(&self, key: &DynKey) {
        let Some(child) = Node::from_dyn(key).map(Arc::new) else {
            return;
        };
        let notifications = {
            let mut state = self
                .graph
                .state
                .lock()
                .expect("dependency graph lock poisoned");
            let Some(attempt) = state.active.get_mut(&self.node) else {
                return;
            };
            if attempt.id != self.id {
                return;
            }
            attempt.edges.insert(child);
            state.notifications(&self.node)
        };
        DependencyCycleGraph::notify(notifications);
    }

    fn type_name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

impl Drop for DependencyCycleGuard {
    fn drop(&mut self) {
        self.graph
            .state
            .lock()
            .expect("dependency graph lock poisoned")
            .retire(&self.node, self.id);
    }
}

pub(crate) async fn compute_guarded<K: Key>(
    ctx: &mut DiceComputations<'_>,
    key: &K,
) -> Result<K::Value, DependencyComputeError> {
    let guard = ctx
        .cycle_guard::<DependencyCycleGuard>()
        .map_err(|error| DependencyComputeError::Dice(anyhow::Error::new(error)))?;
    compute_with_guard(ctx, key, guard.as_deref()).await
}

pub(crate) async fn compute_with_guard<K: Key>(
    ctx: &mut DiceComputations<'_>,
    key: &K,
    guard: Option<&DependencyCycleGuard>,
) -> Result<K::Value, DependencyComputeError> {
    let result = match guard {
        Some(guard) => guard.guard_this(ctx.compute(key)).await,
        None => Ok(ctx.compute(key).await),
    };
    match result {
        Ok(result) => {
            result.map_err(|error| DependencyComputeError::Dice(anyhow::Error::new(error)))
        }
        Err(cycle) => {
            let _unused = ctx.compute(&crate::bzl_module::BzlLoadCyclePoisonKey).await;
            Err(DependencyComputeError::Cycle(cycle))
        }
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod handoff_tests;

#[cfg(test)]
mod warm_tests;

/// Private source preparation uses the same mixed graph guard as Loading waits.
#[doc(hidden)]
pub async fn compute_selected_label_source(
    ctx: &mut DiceComputations<'_>,
    key: &slug_bzlmod_v2::SelectedLabelSourceKey,
) -> Result<
    slug_bzlmod_v2::SourcePreparationOutcome<
        Result<
            slug_bzlmod_v2::ObservedSelectedLabelSource,
            slug_bzlmod_v2::SelectedLabelSourceError,
        >,
    >,
    slug_bzlmod_v2::SelectedLabelSourceError,
> {
    compute_guarded(ctx, key)
        .await
        .map_err(|_| slug_bzlmod_v2::SelectedLabelSourceError::Compute)
}
