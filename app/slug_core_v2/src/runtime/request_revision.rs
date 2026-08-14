/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory.
 */

//! Private, callerless first vertical for a request-owned Host file certificate.
//!
//! The mutex owns only the short publication linearization. DICE computation,
//! terminal formatting, and the initial Host observation all happen without it.

use std::fmt;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use dice::Dice;
use dice::DiceComputations;
use dice::DiceTransaction;
use dice::InjectedKey;
use dice::Key;
#[cfg(test)]
use dice::UserComputationData;
use dupe::Dupe;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathObservationDemand;
use slug_workspace_v2::PathObservationEpoch;
use slug_workspace_v2::PathObservationEpochKey;
use slug_workspace_v2::PathObservationKey;
use slug_workspace_v2::PathObservationNamespace;
use slug_workspace_v2::PathObservationOperation;
use slug_workspace_v2::PathObservationResult;
use slug_workspace_v2::PathOutcome;

const MAX_ATTEMPTS: usize = 8;

#[cfg(test)]
#[derive(Default)]
struct RequestTestAudit {
    root_starts: std::sync::atomic::AtomicUsize,
    observations: std::sync::atomic::AtomicUsize,
    commits: std::sync::atomic::AtomicUsize,
    retries: std::sync::atomic::AtomicUsize,
    accepts: std::sync::atomic::AtomicUsize,
}

#[cfg(test)]
struct RequestTestGate {
    block_once: std::sync::atomic::AtomicBool,
    entered: tokio::sync::Semaphore,
    release: tokio::sync::Semaphore,
}

#[cfg(test)]
impl RequestTestGate {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            block_once: std::sync::atomic::AtomicBool::new(true),
            entered: tokio::sync::Semaphore::new(0),
            release: tokio::sync::Semaphore::new(0),
        })
    }

    async fn enter(&self) {
        use std::sync::atomic::Ordering;
        if self.block_once.swap(false, Ordering::SeqCst) {
            self.entered.add_permits(1);
            self.release.acquire().await.unwrap().forget();
        }
    }
}

#[cfg(test)]
#[derive(Default)]
struct RequestTestFaults {
    observation: std::sync::atomic::AtomicBool,
    injection: std::sync::atomic::AtomicBool,
    publication: std::sync::atomic::AtomicBool,
    nonprogress: std::sync::atomic::AtomicBool,
}

#[cfg(test)]
struct RequestTestData {
    audit: Arc<RequestTestAudit>,
    faults: RequestTestFaults,
    gates: std::sync::Mutex<std::collections::HashMap<Arc<str>, Arc<RequestTestGate>>>,
    compute_entries: tokio::sync::Semaphore,
}

#[derive(Debug, Clone, Allocative)]
pub(super) struct RequestOverlay {
    semantic: Arc<str>,
    presentation: Arc<str>,
}

impl RequestOverlay {
    #[cfg(test)]
    fn new(semantic: impl Into<Arc<str>>, presentation: impl Into<Arc<str>>) -> Self {
        Self {
            semantic: semantic.into(),
            presentation: presentation.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(super) struct SourceCertificate {
    demand: PathObservationDemand,
    observation: Arc<PathObservationResult>,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
struct RootTerminal {
    semantic: Arc<str>,
    certificate: SourceCertificate,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(super) struct RootHostRequestResult {
    terminal: RootTerminal,
    presentation: Arc<str>,
    revision: u64,
}

impl RootHostRequestResult {
    #[cfg(test)]
    fn certificate(&self) -> &SourceCertificate {
        &self.terminal.certificate
    }

    #[cfg(test)]
    fn semantic(&self) -> &str {
        &self.terminal.semantic
    }

    #[cfg(test)]
    fn presentation(&self) -> &str {
        &self.presentation
    }

    #[cfg(test)]
    fn bytes(&self) -> Option<&[u8]> {
        match self.terminal.certificate.observation.as_ref() {
            PathObservationResult::FileBytes(slug_workspace_v2::PathOperationResult::Present(
                bytes,
            )) => Some(bytes.as_ref()),
            _ => None,
        }
    }

    #[cfg(test)]
    fn revision(&self) -> u64 {
        self.revision
    }
}

#[derive(Debug)]
#[allow(dead_code)]
pub(super) enum RequestRevisionError {
    PathOutsideWorkspace(PathBuf),
    Observation(String),
    Injection(String),
    Publication(String),
    Computation(String),
    RevisionExhausted,
    RetryNonProgress,
}

impl fmt::Display for RequestRevisionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PathOutsideWorkspace(path) => {
                write!(f, "request path escapes workspace: {}", path.display())
            }
            Self::Observation(error) => write!(f, "observing Host file: {error}"),
            Self::Publication(error) => write!(f, "publishing path epoch: {error}"),
            Self::Injection(error) => write!(f, "injecting path epoch: {error}"),
            Self::Computation(error) => write!(f, "computing request root: {error}"),
            Self::RevisionExhausted => f.write_str("request revision allocator exhausted"),
            Self::RetryNonProgress => f.write_str("request revision made no bounded progress"),
        }
    }
}

impl std::error::Error for RequestRevisionError {}

#[derive(Debug, Allocative)]
struct RevisionOwner {
    initialized: bool,
    next_revision: u64,
    published_revision: Option<RequestRevision>,
}

pub(super) struct RequestRevisionRuntime {
    dice: Arc<Dice>,
    workspace: NormalizedAbsolutePath,
    owner: tokio::sync::Mutex<RevisionOwner>,
    #[cfg(test)]
    test_data: Arc<RequestTestData>,
}

impl RequestRevisionRuntime {
    pub(super) fn new(dice: Arc<Dice>, workspace: NormalizedAbsolutePath) -> Arc<Self> {
        Arc::new(Self {
            dice,
            workspace,
            owner: tokio::sync::Mutex::new(RevisionOwner {
                initialized: false,
                next_revision: 1,
                published_revision: None,
            }),
            #[cfg(test)]
            test_data: Arc::new(RequestTestData {
                audit: Arc::new(RequestTestAudit::default()),
                faults: RequestTestFaults::default(),
                gates: std::sync::Mutex::new(std::collections::HashMap::new()),
                compute_entries: tokio::sync::Semaphore::new(0),
            }),
        })
    }

    fn updater(&self) -> dice::DiceTransactionUpdater {
        #[cfg(test)]
        {
            let mut data = UserComputationData::default();
            data.data.set(self.test_data.dupe());
            self.dice.updater_with_data(data)
        }
        #[cfg(not(test))]
        {
            self.dice.updater()
        }
    }

    /// A leaf used by every legacy production publisher sharing this DICE.
    /// It holds the async owner only for `commit`, never for later compute.
    pub(super) async fn commit(&self, updater: dice::DiceTransactionUpdater) -> DiceTransaction {
        let owner = self.owner.lock().await;
        let transaction = updater.commit().await;
        drop(owner);
        transaction
    }

    #[allow(dead_code)]
    pub(super) async fn read_host_file(
        &self,
        relative: &Path,
        overlay: RequestOverlay,
    ) -> Result<RootHostRequestResult, RequestRevisionError> {
        let relative = contained_relative_path(relative)?;
        let key = RootHostRequestKey {
            workspace: self.workspace.dupe(),
            relative,
            semantic: overlay.semantic.dupe(),
        };

        for _ in 0..MAX_ATTEMPTS {
            let mut base = self.base_transaction().await?;
            #[cfg(test)]
            self.test_data.compute_entries.add_permits(1);
            let provisional = base
                .compute(&key)
                .await
                .map_err(|error| RequestRevisionError::Computation(error.to_string()))?;
            match &provisional {
                RootHostOutcome::Need(demand) => {
                    let observed = self.observe_exact(demand)?;
                    if self.publish_if_current(&base, demand, observed).await? {
                        #[cfg(test)]
                        self.test_data
                            .audit
                            .retries
                            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        drop(base);
                        continue;
                    }
                }
                RootHostOutcome::Complete(terminal) => {
                    let terminal = terminal.clone();
                    if let Some(revision) = self.validate_terminal(&base, &terminal).await? {
                        #[cfg(test)]
                        self.test_data
                            .audit
                            .accepts
                            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        drop(base);
                        return Ok(RootHostRequestResult {
                            terminal,
                            presentation: overlay.presentation,
                            revision,
                        });
                    }
                }
                RootHostOutcome::Failure(error) => {
                    return Err(RequestRevisionError::Computation(error.to_string()));
                }
            }
            drop(base);
            #[cfg(test)]
            self.test_data
                .audit
                .retries
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
        Err(RequestRevisionError::RetryNonProgress)
    }

    async fn base_transaction(&self) -> Result<DiceTransaction, RequestRevisionError> {
        let mut owner = self.owner.lock().await;
        let mut updater = self.updater();
        let transaction = if owner.initialized {
            updater.existing_state().await
        } else {
            let revision = Self::allocate_revision(&mut owner)?;
            updater
                .changed_to(vec![(
                    RequestRevisionKey {
                        workspace: self.workspace.dupe(),
                    },
                    revision,
                )])
                .map_err(|error| RequestRevisionError::Injection(error.to_string()))?;
            updater
                .changed_to(vec![(
                    PathObservationEpochKey,
                    PathObservationEpoch::empty(),
                )])
                .map_err(|error| RequestRevisionError::Injection(error.to_string()))?;
            let transaction = updater.commit().await;
            owner.published_revision = Some(revision);
            owner.initialized = true;
            #[cfg(test)]
            self.test_data
                .audit
                .commits
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            transaction
        };
        drop(owner);
        Ok(transaction)
    }

    fn allocate_revision(
        owner: &mut RevisionOwner,
    ) -> Result<RequestRevision, RequestRevisionError> {
        let revision = RequestRevision(owner.next_revision);
        owner.next_revision = owner
            .next_revision
            .checked_add(1)
            .ok_or(RequestRevisionError::RevisionExhausted)?;
        Ok(revision)
    }

    fn observe_exact(
        &self,
        demand: &PathObservationDemand,
    ) -> Result<Arc<PathObservationResult>, RequestRevisionError> {
        #[cfg(test)]
        {
            use std::sync::atomic::Ordering;
            self.test_data
                .audit
                .observations
                .fetch_add(1, Ordering::SeqCst);
            if self
                .test_data
                .faults
                .observation
                .swap(false, Ordering::SeqCst)
            {
                return Err(RequestRevisionError::Observation(
                    "forced test failure".to_owned(),
                ));
            }
        }
        let epoch = super::path_observation::observe_native(
            &(),
            std::iter::empty::<(
                slug_workspace_v2::PathObservationInstanceId,
                NormalizedAbsolutePath,
            )>(),
            [demand.dupe()],
        )
        .map_err(|error| RequestRevisionError::Observation(format!("{error:?}")))?;
        epoch.get(demand).cloned().ok_or_else(|| {
            RequestRevisionError::Observation("kernel omitted demanded result".to_owned())
        })
    }

    /// Commit one typed observation while the caller continuously owns publication.
    /// This leaf performs no DICE compute, callback, or mutex acquisition.
    async fn commit_observation_under_owner(
        &self,
        owner: &mut RevisionOwner,
        mut updater: dice::DiceTransactionUpdater,
        demand: &PathObservationDemand,
        observation: Arc<PathObservationResult>,
    ) -> Result<(), RequestRevisionError> {
        #[cfg(test)]
        if self
            .test_data
            .faults
            .injection
            .swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            return Err(RequestRevisionError::Injection(
                "forced test failure".to_owned(),
            ));
        }
        let epoch = PathObservationEpoch::new([(demand.dupe(), observation.as_ref().clone())])
            .map_err(|error| RequestRevisionError::Injection(error.to_string()))?;
        let revision = Self::allocate_revision(owner)?;
        updater
            .changed_to(vec![(
                RequestRevisionKey {
                    workspace: self.workspace.dupe(),
                },
                revision,
            )])
            .map_err(|error| RequestRevisionError::Injection(error.to_string()))?;
        updater
            .changed_to(vec![(PathObservationEpochKey, epoch)])
            .map_err(|error| RequestRevisionError::Injection(error.to_string()))?;
        #[cfg(test)]
        if self
            .test_data
            .faults
            .publication
            .swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            return Err(RequestRevisionError::Publication(
                "forced test failure".to_owned(),
            ));
        }
        let transaction = updater.commit().await;
        owner.published_revision = Some(revision);
        #[cfg(test)]
        self.test_data
            .audit
            .commits
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        drop(transaction);
        Ok(())
    }

    async fn publish_if_current(
        &self,
        base: &DiceTransaction,
        demand: &PathObservationDemand,
        observation: Arc<PathObservationResult>,
    ) -> Result<bool, RequestRevisionError> {
        let mut owner = self.owner.lock().await;
        let updater = self.updater();
        let current = updater.existing_state().await;
        if !current.equivalent(base) {
            return Ok(false);
        }
        #[cfg(test)]
        if self
            .test_data
            .faults
            .nonprogress
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            return Ok(false);
        }
        self.commit_observation_under_owner(&mut owner, updater, demand, observation)
            .await?;
        drop(owner);
        Ok(true)
    }

    async fn validate_terminal(
        &self,
        base: &DiceTransaction,
        terminal: &RootTerminal,
    ) -> Result<Option<u64>, RequestRevisionError> {
        let mut owner = self.owner.lock().await;
        let updater = self.updater();
        let current = updater.existing_state().await;
        if !current.equivalent(base) {
            return Ok(None);
        }
        #[cfg(test)]
        if self
            .test_data
            .faults
            .nonprogress
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            return Ok(None);
        }
        let observed = self.observe_exact(&terminal.certificate.demand)?;
        if observed.as_ref() == terminal.certificate.observation.as_ref() {
            let revision = owner
                .published_revision
                .ok_or(RequestRevisionError::RetryNonProgress)?;
            drop(owner);
            return Ok(Some(revision.0));
        }
        self.commit_observation_under_owner(
            &mut owner,
            updater,
            &terminal.certificate.demand,
            observed,
        )
        .await?;
        drop(owner);
        Ok(None)
    }
}

fn contained_relative_path(path: &Path) -> Result<PathBuf, RequestRevisionError> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::CurDir
                    | Component::ParentDir
                    | Component::RootDir
                    | Component::Prefix(_)
            )
        })
    {
        return Err(RequestRevisionError::PathOutsideWorkspace(
            path.to_path_buf(),
        ));
    }
    Ok(path.components().collect())
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
struct RequestRevisionKey {
    workspace: NormalizedAbsolutePath,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative, Dupe)]
struct RequestRevision(u64);

impl fmt::Display for RequestRevisionKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "root-host-request-revision:{}", self.workspace)
    }
}

#[async_trait]
impl InjectedKey for RequestRevisionKey {
    type Value = RequestRevision;

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
struct RootHostRequestKey {
    workspace: NormalizedAbsolutePath,
    relative: PathBuf,
    semantic: Arc<str>,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
enum RootHostOutcome {
    Need(PathObservationDemand),
    Complete(RootTerminal),
    Failure(Arc<str>),
}

impl fmt::Display for RootHostRequestKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "root-host-request:{}", self.relative.display())
    }
}

#[async_trait]
impl Key for RootHostRequestKey {
    type Value = RootHostOutcome;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &dice_futures::cancellation::CancellationContext,
    ) -> Self::Value {
        #[cfg(test)]
        let test_data = ctx
            .per_transaction_data()
            .data
            .get::<Arc<RequestTestData>>()
            .ok()
            .cloned();
        #[cfg(test)]
        if let Some(data) = &test_data {
            data.audit
                .root_starts
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
        if let Err(error) = ctx
            .compute(&RequestRevisionKey {
                workspace: self.workspace.dupe(),
            })
            .await
        {
            return RootHostOutcome::Failure(Arc::from(error.to_string()));
        }
        let demand = PathObservationDemand::new(
            PathObservationNamespace::Host,
            NormalizedAbsolutePath::new(self.workspace.as_path().join(&self.relative))
                .expect("contained workspace request remains absolute"),
            PathObservationOperation::FileBytes,
        );
        match ctx.compute(&PathObservationKey::new(demand.dupe())).await {
            Ok(PathOutcome::Need(_)) => RootHostOutcome::Need(demand),
            Ok(PathOutcome::Complete(observation)) => {
                #[cfg(test)]
                if let Some(data) = &test_data {
                    let gate = {
                        let mut gates = data.gates.lock().expect("request test gate mutex");
                        gates.remove(&self.semantic)
                    };
                    if let Some(gate) = gate {
                        gate.enter().await;
                    }
                }
                RootHostOutcome::Complete(RootTerminal {
                    semantic: self.semantic.dupe(),
                    certificate: SourceCertificate {
                        demand,
                        observation,
                    },
                })
            }
            Err(error) => RootHostOutcome::Failure(Arc::from(error.to_string())),
        }
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        matches!((x, y), (RootHostOutcome::Complete(x), RootHostOutcome::Complete(y)) if x == y)
    }

    fn validity(value: &Self::Value) -> bool {
        matches!(value, RootHostOutcome::Complete(_))
    }
}
#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::atomic::Ordering;
    use std::time::Duration;

    use dice::DetectCycles;

    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct Audit {
        root_starts: usize,
        observations: usize,
        commits: usize,
        retries: usize,
        accepts: usize,
    }

    fn runtime(directory: &tempfile::TempDir) -> Arc<RequestRevisionRuntime> {
        let workspace =
            NormalizedAbsolutePath::new(directory.path().canonicalize().unwrap()).unwrap();
        let dice = Dice::builder().build(DetectCycles::Enabled);
        RequestRevisionRuntime::new(dice, workspace)
    }

    fn overlay(semantic: &str, presentation: &str) -> RequestOverlay {
        RequestOverlay::new(semantic, presentation)
    }

    async fn read(
        runtime: &Arc<RequestRevisionRuntime>,
        path: &str,
        semantic: &str,
        presentation: &str,
    ) -> Result<RootHostRequestResult, RequestRevisionError> {
        runtime
            .read_host_file(Path::new(path), overlay(semantic, presentation))
            .await
    }

    fn audit(runtime: &RequestRevisionRuntime) -> Audit {
        let audit = &runtime.test_data.audit;
        Audit {
            root_starts: audit.root_starts.load(Ordering::SeqCst),
            observations: audit.observations.load(Ordering::SeqCst),
            commits: audit.commits.load(Ordering::SeqCst),
            retries: audit.retries.load(Ordering::SeqCst),
            accepts: audit.accepts.load(Ordering::SeqCst),
        }
    }

    fn delta(after: Audit, before: Audit) -> Audit {
        Audit {
            root_starts: after.root_starts - before.root_starts,
            observations: after.observations - before.observations,
            commits: after.commits - before.commits,
            retries: after.retries - before.retries,
            accepts: after.accepts - before.accepts,
        }
    }

    fn gate(runtime: &RequestRevisionRuntime, semantic: &str) -> Arc<RequestTestGate> {
        let gate = RequestTestGate::new();
        assert!(
            runtime
                .test_data
                .gates
                .lock()
                .expect("request test gate mutex")
                .insert(Arc::from(semantic), gate.clone())
                .is_none()
        );
        gate
    }

    fn drain_compute_entries(runtime: &RequestRevisionRuntime) {
        let available = runtime.test_data.compute_entries.available_permits();
        if available != 0 {
            runtime
                .test_data
                .compute_entries
                .try_acquire_many(available.try_into().unwrap())
                .unwrap()
                .forget();
        }
    }

    async fn wait_entries(runtime: &RequestRevisionRuntime, count: u32) {
        tokio::time::timeout(
            Duration::from_secs(5),
            runtime.test_data.compute_entries.acquire_many(count),
        )
        .await
        .expect("request attempts reached compute")
        .unwrap()
        .forget();
    }

    async fn wait_gate(gate: &RequestTestGate) {
        tokio::time::timeout(Duration::from_secs(5), gate.entered.acquire())
            .await
            .expect("root reached post-demand gate")
            .unwrap()
            .forget();
    }

    async fn wait_idle(runtime: &RequestRevisionRuntime) {
        tokio::time::timeout(Duration::from_secs(5), runtime.dice.wait_for_idle())
            .await
            .expect("DICE became idle");
    }

    #[test]
    fn path_containment_and_revision_overflow_fail_closed() {
        for path in ["", "../outside", "./not-normalized", "/absolute"] {
            assert!(matches!(
                contained_relative_path(Path::new(path)),
                Err(RequestRevisionError::PathOutsideWorkspace(_))
            ));
        }
        assert_eq!(
            contained_relative_path(Path::new("pkg//defs.bzl")).unwrap(),
            Path::new("pkg/defs.bzl")
        );
        assert_eq!(
            contained_relative_path(Path::new("pkg/defs.bzl")).unwrap(),
            Path::new("pkg/defs.bzl")
        );

        let mut owner = RevisionOwner {
            initialized: true,
            next_revision: u64::MAX,
            published_revision: None,
        };
        assert!(matches!(
            RequestRevisionRuntime::allocate_revision(&mut owner),
            Err(RequestRevisionError::RevisionExhausted)
        ));
        assert_eq!(owner.next_revision, u64::MAX);
    }

    #[tokio::test]
    async fn serial_reuse_relevant_identity_mutation_and_restoration_are_exact() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("defs.bzl"), b"V1").unwrap();
        let runtime = runtime(&directory);

        let first = read(&runtime, "defs.bzl", "A", "first").await.unwrap();
        assert_eq!(first.bytes(), Some(&b"V1"[..]));
        assert_eq!(first.semantic(), "A");
        assert_eq!(first.presentation(), "first");
        assert_eq!(first.revision(), 2);
        assert_eq!(
            audit(&runtime),
            Audit {
                root_starts: 2,
                observations: 2,
                commits: 2,
                retries: 1,
                accepts: 1,
            }
        );

        let presentation_only = read(&runtime, "defs.bzl", "A", "second").await.unwrap();
        assert_eq!(presentation_only.presentation(), "second");
        assert_eq!(presentation_only.certificate(), first.certificate());

        let relevant_b = read(&runtime, "defs.bzl", "B", "B").await.unwrap();
        let restored_a = read(&runtime, "defs.bzl", "A", "A-again").await.unwrap();
        assert_eq!(relevant_b.semantic(), "B");
        assert_eq!(restored_a.certificate(), first.certificate());
        assert_eq!(
            audit(&runtime),
            Audit {
                root_starts: 3,
                observations: 5,
                commits: 2,
                retries: 1,
                accepts: 4,
            }
        );

        std::fs::write(directory.path().join("defs.bzl"), b"V2").unwrap();
        let changed = read(&runtime, "defs.bzl", "A", "changed").await.unwrap();
        assert_eq!(changed.bytes(), Some(&b"V2"[..]));
        assert_eq!(changed.revision(), 3);
        let warm = read(&runtime, "defs.bzl", "A", "warm").await.unwrap();
        assert_eq!(warm.bytes(), Some(&b"V2"[..]));

        std::fs::write(directory.path().join("defs.bzl"), b"V1").unwrap();
        let restored = read(&runtime, "defs.bzl", "A", "restored").await.unwrap();
        assert_eq!(restored.bytes(), Some(&b"V1"[..]));
        assert_eq!(restored.revision(), 4);
        assert_eq!(
            audit(&runtime),
            Audit {
                root_starts: 5,
                observations: 10,
                commits: 4,
                retries: 3,
                accepts: 7,
            }
        );
    }

    #[tokio::test]
    async fn missing_file_has_an_exact_absence_certificate() {
        let directory = tempfile::tempdir().unwrap();
        let runtime = runtime(&directory);
        let missing = read(&runtime, "missing.bzl", "missing", "missing")
            .await
            .unwrap();
        assert!(matches!(
            missing.certificate().observation.as_ref(),
            PathObservationResult::FileBytes(slug_workspace_v2::PathOperationResult::Missing)
        ));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn overlapping_post_demand_requests_discard_v1_and_accept_only_v2() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("defs.bzl"), b"V1").unwrap();
        let runtime = runtime(&directory);
        read(&runtime, "defs.bzl", "seed", "seed").await.unwrap();
        drain_compute_entries(&runtime);
        let before = audit(&runtime);

        let gate_a = gate(&runtime, "A");
        let gate_b = gate(&runtime, "B");
        let task_a = {
            let runtime = runtime.clone();
            tokio::spawn(async move { read(&runtime, "defs.bzl", "A", "A").await })
        };
        let task_b = {
            let runtime = runtime.clone();
            tokio::spawn(async move { read(&runtime, "defs.bzl", "B", "B").await })
        };

        wait_entries(&runtime, 2).await;
        wait_gate(&gate_a).await;
        assert!(runtime.owner.try_lock().is_ok());
        wait_gate(&gate_b).await;
        assert!(runtime.owner.try_lock().is_ok());
        assert_eq!(audit(&runtime).commits, before.commits);

        std::fs::write(directory.path().join("defs.bzl"), b"V2").unwrap();
        gate_a.release.add_permits(1);
        gate_b.release.add_permits(1);

        let result_a = tokio::time::timeout(Duration::from_secs(5), task_a)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let result_b = tokio::time::timeout(Duration::from_secs(5), task_b)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(result_a.bytes(), Some(&b"V2"[..]));
        assert_eq!(result_b.bytes(), Some(&b"V2"[..]));
        assert_eq!(
            delta(audit(&runtime), before),
            Audit {
                root_starts: 4,
                observations: 3,
                commits: 1,
                retries: 2,
                accepts: 2,
            }
        );
        wait_idle(&runtime).await;
        assert_eq!(Arc::strong_count(&gate_a), 1);
        assert_eq!(Arc::strong_count(&gate_b), 1);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn one_waiter_and_last_waiter_cancellation_release_request_state() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("defs.bzl"), b"V1").unwrap();
        let runtime = runtime(&directory);
        read(&runtime, "defs.bzl", "seed", "seed").await.unwrap();
        drain_compute_entries(&runtime);

        let before_shared = audit(&runtime);
        let shared_gate = gate(&runtime, "shared");
        let first = {
            let runtime = runtime.clone();
            tokio::spawn(async move { read(&runtime, "defs.bzl", "shared", "first").await })
        };
        wait_gate(&shared_gate).await;
        let second = {
            let runtime = runtime.clone();
            tokio::spawn(async move { read(&runtime, "defs.bzl", "shared", "second").await })
        };
        wait_entries(&runtime, 2).await;
        assert!(runtime.owner.try_lock().is_ok());

        first.abort();
        assert!(first.await.unwrap_err().is_cancelled());
        shared_gate.release.add_permits(1);
        let surviving = tokio::time::timeout(Duration::from_secs(5), second)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(surviving.bytes(), Some(&b"V1"[..]));
        wait_idle(&runtime).await;
        assert_eq!(Arc::strong_count(&shared_gate), 1);
        assert_eq!(
            delta(audit(&runtime), before_shared),
            Audit {
                root_starts: 1,
                observations: 1,
                commits: 0,
                retries: 0,
                accepts: 1,
            }
        );

        drain_compute_entries(&runtime);
        let before_last = audit(&runtime);
        let last_gate = gate(&runtime, "last");
        let last = {
            let runtime = runtime.clone();
            tokio::spawn(async move { read(&runtime, "defs.bzl", "last", "last").await })
        };
        wait_entries(&runtime, 1).await;
        wait_gate(&last_gate).await;
        assert!(runtime.owner.try_lock().is_ok());
        last.abort();
        assert!(last.await.unwrap_err().is_cancelled());
        wait_idle(&runtime).await;
        assert_eq!(Arc::strong_count(&last_gate), 1);
        assert_eq!(
            delta(audit(&runtime), before_last),
            Audit {
                root_starts: 1,
                observations: 0,
                commits: 0,
                retries: 0,
                accepts: 0,
            }
        );

        let after_cancel = audit(&runtime);
        let recovered = read(&runtime, "defs.bzl", "last", "recovered")
            .await
            .unwrap();
        assert_eq!(recovered.bytes(), Some(&b"V1"[..]));
        assert_eq!(
            delta(audit(&runtime), after_cancel),
            Audit {
                root_starts: 1,
                observations: 1,
                commits: 0,
                retries: 0,
                accepts: 1,
            }
        );
    }

    #[tokio::test]
    async fn forced_failures_and_nonprogress_never_publish_a_terminal() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("seed.bzl"), b"seed").unwrap();
        std::fs::write(directory.path().join("other.bzl"), b"other").unwrap();
        let runtime = runtime(&directory);
        read(&runtime, "seed.bzl", "seed", "seed").await.unwrap();
        let baseline = audit(&runtime);

        runtime
            .test_data
            .faults
            .observation
            .store(true, Ordering::SeqCst);
        assert!(matches!(
            read(&runtime, "other.bzl", "other", "observation").await,
            Err(RequestRevisionError::Observation(_))
        ));
        assert_eq!(audit(&runtime).commits, baseline.commits);
        assert_eq!(audit(&runtime).accepts, baseline.accepts);

        runtime
            .test_data
            .faults
            .injection
            .store(true, Ordering::SeqCst);
        assert!(matches!(
            read(&runtime, "other.bzl", "other", "injection").await,
            Err(RequestRevisionError::Injection(_))
        ));
        assert_eq!(audit(&runtime).commits, baseline.commits);
        assert_eq!(audit(&runtime).accepts, baseline.accepts);

        runtime
            .test_data
            .faults
            .publication
            .store(true, Ordering::SeqCst);
        assert!(matches!(
            read(&runtime, "other.bzl", "other", "publication").await,
            Err(RequestRevisionError::Publication(_))
        ));
        assert_eq!(audit(&runtime).commits, baseline.commits);
        assert_eq!(audit(&runtime).accepts, baseline.accepts);

        let before_nonprogress = audit(&runtime);
        runtime
            .test_data
            .faults
            .nonprogress
            .store(true, Ordering::SeqCst);
        assert!(matches!(
            read(&runtime, "other.bzl", "other", "nonprogress").await,
            Err(RequestRevisionError::RetryNonProgress)
        ));
        assert_eq!(
            delta(audit(&runtime), before_nonprogress),
            Audit {
                root_starts: MAX_ATTEMPTS,
                observations: MAX_ATTEMPTS,
                commits: 0,
                retries: MAX_ATTEMPTS,
                accepts: 0,
            }
        );

        runtime
            .test_data
            .faults
            .nonprogress
            .store(false, Ordering::SeqCst);
        let recovered = read(&runtime, "other.bzl", "other", "recovered")
            .await
            .unwrap();
        assert_eq!(recovered.bytes(), Some(&b"other"[..]));
        assert_eq!(audit(&runtime).commits, baseline.commits + 1);
        assert_eq!(audit(&runtime).accepts, baseline.accepts + 1);
    }
}
