/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License found in the LICENSE-APACHE file in the root directory of this
 * source tree. You may select the license that applies to you.
 */

use std::ffi::OsString;
use std::fs::File;
use std::io::Write;
use std::ops::Range;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::sync::Mutex;

use async_trait::async_trait;
use dice::DiceDataBuilder;
use sha2::Digest;
use sha2::Sha256;
use slug_bzlmod_v2::OverrideAttributeValue;
use slug_bzlmod_v2::RepoSpec;
use slug_bzlmod_v2::RepositoryIo;
use slug_bzlmod_v2::RepositoryIoOutcome;
use slug_bzlmod_v2::RepositoryMaterializationEpochEntry;
use slug_bzlmod_v2::RepositoryMaterializationGeneration;
use slug_bzlmod_v2::RepositoryMaterializationKind;
use slug_bzlmod_v2::RepositoryMaterializationRequest;
#[cfg(test)]
use slug_bzlmod_v2::RepositoryMaterializationRequestId;
use slug_bzlmod_v2::RepositoryMaterializationResult;
use slug_bzlmod_v2::RepositoryMaterializationResultEpoch;
use slug_bzlmod_v2::RepositoryMaterializationSuccess;
use slug_bzlmod_v2::RepositoryTransportError;
use slug_bzlmod_v2::install_repository_io;
use slug_bzlmod_v2::source_identity;
use slug_identity_v2::CanonicalRepoName;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathLstat;
use slug_workspace_v2::PathNodeKind;
use slug_workspace_v2::PathObservationDemand;
use slug_workspace_v2::PathObservationEpoch;
use slug_workspace_v2::PathObservationInstanceId;
use slug_workspace_v2::PathObservationNamespace;
use slug_workspace_v2::PathObservationOperation;
use slug_workspace_v2::PathObservationResult;
use slug_workspace_v2::PathOperationResult;

struct LocalRepositoryIo {
    immutable_roots: Mutex<RetainedImmutableRoots>,
}

struct RetainedImmutableRoots {
    next_instance: u64,
    roots: Vec<(PathObservationInstanceId, tempfile::TempDir)>,
}

impl LocalRepositoryIo {
    fn new() -> Self {
        Self {
            immutable_roots: Mutex::new(RetainedImmutableRoots {
                next_instance: 1,
                roots: Vec::new(),
            }),
        }
    }

    fn retain(
        &self,
        root: tempfile::TempDir,
    ) -> Result<(PathBuf, PathObservationInstanceId), RepositoryTransportError> {
        let mut retained = self
            .immutable_roots
            .lock()
            .expect("immutable repository root mutex poisoned");
        let instance = allocate_observation_instance(&mut retained.next_instance)?;
        let path = root.path().to_path_buf();
        retained.roots.push((instance, root));
        Ok((path, instance))
    }
}

fn allocate_observation_instance(
    next_instance: &mut u64,
) -> Result<PathObservationInstanceId, RepositoryTransportError> {
    let current = *next_instance;
    if current == 0 {
        return Err(observation_instance_exhausted());
    }
    let successor = current
        .checked_add(1)
        .ok_or_else(observation_instance_exhausted)?;
    *next_instance = successor;
    Ok(PathObservationInstanceId::new(current))
}

fn observation_instance_exhausted() -> RepositoryTransportError {
    RepositoryTransportError {
        message: "repository materialization observation instance is invalid or exhausted".into(),
    }
}

#[allow(dead_code)] // The bridge is dormant until the retry-driver packet.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RepositorySessionToken(u64);

#[allow(dead_code)]
#[derive(Debug, Eq, PartialEq)]
pub(super) enum RepositorySessionError {
    Busy,
    StaleToken {
        active: Option<RepositorySessionToken>,
        supplied: RepositorySessionToken,
    },
    TokenExhausted,
    InstanceExhausted,
    WrongWorkspace,
    KindMismatch,
    ConflictingRequest(CanonicalRepoName),
    NonSuccessSelection(CanonicalRepoName),
    UnknownSelection(CanonicalRepoName),
    DuplicateSelection(CanonicalRepoName),
    InvalidValidation(CanonicalRepoName),
    ValidationAlreadyStarted,
    ValidationIncomplete,
    InvalidRetainedRoot(PathBuf),
    NativeObservation(super::path_observation::PathObservationKernelError),
}

#[allow(dead_code)]
#[derive(Clone)]
pub(super) struct RepositoryValidation {
    request: Arc<RepositoryMaterializationRequest>,
    observations: Arc<[(PathObservationDemand, PathObservationResult)]>,
}

#[allow(dead_code)]
impl RepositoryValidation {
    pub(super) fn new(
        request: Arc<RepositoryMaterializationRequest>,
        observations: impl Into<Arc<[(PathObservationDemand, PathObservationResult)]>>,
    ) -> Self {
        Self {
            request,
            observations: observations.into(),
        }
    }

    pub(super) fn request(&self) -> &Arc<RepositoryMaterializationRequest> {
        &self.request
    }

    pub(super) fn observations(&self) -> &[(PathObservationDemand, PathObservationResult)] {
        &self.observations
    }
}

#[allow(dead_code)]
#[derive(Clone)]
struct AcceptedRepository {
    request: Arc<RepositoryMaterializationRequest>,
    success: RepositoryMaterializationSuccess,
    validation: Arc<[(PathObservationDemand, PathObservationResult)]>,
}

#[allow(dead_code)]
struct RetainedRepositoryRoot {
    observation_instance: PathObservationInstanceId,
    root: Arc<tempfile::TempDir>,
}

#[allow(dead_code)]
struct ActiveRepositorySession {
    token: RepositorySessionToken,
    validation: RepositorySessionValidation,
    reusable: Vec<AcceptedRepository>,
    entries: Vec<RepositoryMaterializationEpochEntry>,
    provisional_roots: Vec<RetainedRepositoryRoot>,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RepositorySessionValidation {
    Pending,
    InProgress,
    Complete,
}

#[allow(dead_code)]
struct RepositoryMaterializerState {
    next_token: u64,
    next_instance: u64,
    active: Option<ActiveRepositorySession>,
    accepted: Vec<AcceptedRepository>,
    accepted_roots: Vec<RetainedRepositoryRoot>,
}

#[allow(dead_code)]
pub(super) struct RepositoryMaterializer {
    workspace: NormalizedAbsolutePath,
    state: Mutex<RepositoryMaterializerState>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct RepositoryNativePreflight {
    path_observations: PathObservationEpoch,
    repository_results: RepositoryMaterializationResultEpoch,
    reusable_requests: Arc<[Arc<RepositoryMaterializationRequest>]>,
}

#[allow(dead_code)]
impl RepositoryNativePreflight {
    pub(super) fn path_observations(&self) -> &PathObservationEpoch {
        &self.path_observations
    }

    pub(super) fn repository_results(&self) -> &RepositoryMaterializationResultEpoch {
        &self.repository_results
    }

    pub(super) fn reusable_requests(&self) -> &[Arc<RepositoryMaterializationRequest>] {
        &self.reusable_requests
    }
}

#[allow(dead_code)]
enum RepositoryMaterializationAttempt {
    Local,
    Immutable {
        bytes: Vec<u8>,
        root: tempfile::TempDir,
    },
    SpecError(String),
    TransportError(String),
    MaterializationError(String),
}

#[allow(dead_code)]
impl RepositoryMaterializer {
    pub(super) fn new(workspace: NormalizedAbsolutePath) -> Self {
        Self {
            workspace,
            state: Mutex::new(RepositoryMaterializerState {
                next_token: 1,
                next_instance: 1,
                active: None,
                accepted: Vec::new(),
                accepted_roots: Vec::new(),
            }),
        }
    }

    #[cfg(test)]
    pub(super) fn workspace(&self) -> &NormalizedAbsolutePath {
        &self.workspace
    }

    pub(super) fn begin(&self) -> Result<RepositorySessionToken, RepositorySessionError> {
        let mut state = self
            .state
            .lock()
            .expect("repository materializer mutex poisoned");
        if state.active.is_some() {
            return Err(RepositorySessionError::Busy);
        }
        let token = allocate_session_token(&mut state.next_token)?;
        state.active = Some(ActiveRepositorySession {
            token,
            validation: RepositorySessionValidation::Pending,
            reusable: Vec::new(),
            entries: Vec::new(),
            provisional_roots: Vec::new(),
        });
        Ok(token)
    }

    fn validate(
        &self,
        token: RepositorySessionToken,
        observe: &mut impl FnMut(&PathObservationDemand, Option<&Path>) -> PathObservationResult,
    ) -> Result<(), RepositorySessionError> {
        let (accepted, roots) = self.start_validation(token)?;
        let root_paths = roots
            .iter()
            .map(|(instance, root)| (*instance, root.path().to_path_buf()))
            .collect::<Vec<_>>();
        let reusable = accepted
            .into_iter()
            .filter(|accepted| {
                !validation_is_dirty(&accepted.validation, &root_paths, |demand, root| {
                    observe(demand, root)
                })
            })
            .collect::<Vec<_>>();
        self.complete_validation(token, reusable).map(|_| ())
    }

    fn start_validation(
        &self,
        token: RepositorySessionToken,
    ) -> Result<
        (
            Vec<AcceptedRepository>,
            Vec<(PathObservationInstanceId, Arc<tempfile::TempDir>)>,
        ),
        RepositorySessionError,
    > {
        let mut state = self
            .state
            .lock()
            .expect("repository materializer mutex poisoned");
        let active = matching_active_mut(&mut state, token)?;
        if active.validation != RepositorySessionValidation::Pending {
            return Err(RepositorySessionError::ValidationAlreadyStarted);
        }
        active.validation = RepositorySessionValidation::InProgress;
        let accepted = state.accepted.clone();
        let roots = state
            .accepted_roots
            .iter()
            .map(|root| (root.observation_instance, root.root.clone()))
            .collect::<Vec<_>>();
        Ok((accepted, roots))
    }

    fn complete_validation(
        &self,
        token: RepositorySessionToken,
        reusable: Vec<AcceptedRepository>,
    ) -> Result<RepositoryMaterializationResultEpoch, RepositorySessionError> {
        let mut state = self
            .state
            .lock()
            .expect("repository materializer mutex poisoned");
        let active = matching_active_mut(&mut state, token)?;
        for accepted in &reusable {
            upsert_epoch_entry(
                &mut active.entries,
                RepositoryMaterializationEpochEntry {
                    request: accepted.request.clone(),
                    result: RepositoryMaterializationResult::Success(accepted.success.clone()),
                },
            );
        }
        active.reusable = reusable;
        active.validation = RepositorySessionValidation::Complete;
        complete_epoch(&self.workspace, &active.entries)
    }

    pub(super) fn preflight_native(
        &self,
        token: RepositorySessionToken,
        demands: impl IntoIterator<Item = PathObservationDemand>,
    ) -> Result<RepositoryNativePreflight, RepositorySessionError> {
        self.preflight_impl(token, demands, |root_owners, roots, demands| {
            super::path_observation::observe_native(&root_owners, roots, demands)
        })
    }

    #[cfg(test)]
    fn preflight_with(
        &self,
        token: RepositorySessionToken,
        demands: impl IntoIterator<Item = PathObservationDemand>,
        observe: impl FnOnce(
            &[(PathObservationInstanceId, Arc<tempfile::TempDir>)],
            Vec<(PathObservationInstanceId, NormalizedAbsolutePath)>,
            Vec<PathObservationDemand>,
        ) -> Result<
            PathObservationEpoch,
            super::path_observation::PathObservationKernelError,
        >,
    ) -> Result<RepositoryNativePreflight, RepositorySessionError> {
        self.preflight_impl(token, demands, observe)
    }

    fn preflight_impl(
        &self,
        token: RepositorySessionToken,
        demands: impl IntoIterator<Item = PathObservationDemand>,
        observe: impl FnOnce(
            &[(PathObservationInstanceId, Arc<tempfile::TempDir>)],
            Vec<(PathObservationInstanceId, NormalizedAbsolutePath)>,
            Vec<PathObservationDemand>,
        ) -> Result<
            PathObservationEpoch,
            super::path_observation::PathObservationKernelError,
        >,
    ) -> Result<RepositoryNativePreflight, RepositorySessionError> {
        let (accepted, root_owners) = self.start_validation(token)?;
        let roots = normalize_retained_roots(&root_owners)?;
        let mut demands = accepted
            .iter()
            .flat_map(|accepted| accepted.validation.iter().map(|(demand, _)| demand.clone()))
            .chain(demands)
            .collect::<Vec<_>>();
        demands.sort_unstable();
        demands.dedup();
        let observed = observe(&root_owners, roots, demands)
            .map_err(RepositorySessionError::NativeObservation)?;
        let reusable = accepted
            .into_iter()
            .filter(|accepted| !validation_epoch_is_dirty(&accepted.validation, &observed))
            .collect::<Vec<_>>();
        let reusable_requests = reusable
            .iter()
            .map(|accepted| accepted.request.clone())
            .collect::<Arc<[_]>>();
        let repository_results = self.complete_validation(token, reusable)?;
        Ok(RepositoryNativePreflight {
            path_observations: observed,
            repository_results,
            reusable_requests,
        })
    }

    #[cfg(test)]
    fn validate_native(&self, token: RepositorySessionToken) -> Result<(), RepositorySessionError> {
        self.preflight_native(token, std::iter::empty()).map(|_| ())
    }

    fn materialize_with(
        &self,
        token: RepositorySessionToken,
        request: Arc<RepositoryMaterializationRequest>,
        generation: RepositoryMaterializationGeneration,
        materialize: impl FnOnce() -> RepositoryMaterializationAttempt,
    ) -> Result<RepositoryMaterializationResultEpoch, RepositorySessionError> {
        if request.id.workspace != self.workspace {
            return Err(RepositorySessionError::WrongWorkspace);
        }
        {
            let mut state = self
                .state
                .lock()
                .expect("repository materializer mutex poisoned");
            let active = matching_validated_active_mut(&mut state, token)?;
            if let Some(existing) = active
                .entries
                .iter()
                .find(|entry| entry.request.id == request.id)
            {
                let inherited = active
                    .reusable
                    .iter()
                    .position(|accepted| accepted.request.id == request.id);
                if let Some(index) = inherited {
                    if *existing.request == *request {
                        active.reusable.remove(index);
                        return complete_epoch(&self.workspace, &active.entries);
                    }
                } else if *existing.request != *request {
                    return Err(RepositorySessionError::ConflictingRequest(
                        request.id.canonical_repo.clone(),
                    ));
                } else {
                    return complete_epoch(&self.workspace, &active.entries);
                }
            }
        }

        let attempt = materialize();
        let prepared = match attempt {
            RepositoryMaterializationAttempt::Immutable { bytes, root } => {
                let identity = source_identity(&bytes);
                PreparedMaterializationAttempt::Immutable {
                    source_identity: identity,
                    root,
                }
            }
            RepositoryMaterializationAttempt::Local => PreparedMaterializationAttempt::Local,
            RepositoryMaterializationAttempt::SpecError(message) => {
                PreparedMaterializationAttempt::Result(RepositoryMaterializationResult::SpecError(
                    message.into(),
                ))
            }
            RepositoryMaterializationAttempt::TransportError(message) => {
                PreparedMaterializationAttempt::Result(
                    RepositoryMaterializationResult::TransportError {
                        generation,
                        message: message.into(),
                    },
                )
            }
            RepositoryMaterializationAttempt::MaterializationError(message) => {
                PreparedMaterializationAttempt::Result(
                    RepositoryMaterializationResult::MaterializationError {
                        generation,
                        message: message.into(),
                    },
                )
            }
        };

        let kind_matches = matches!(
            (&request.kind, &prepared),
            (
                RepositoryMaterializationKind::Local { .. },
                PreparedMaterializationAttempt::Local
            ) | (
                RepositoryMaterializationKind::Immutable,
                PreparedMaterializationAttempt::Immutable { .. }
            ) | (_, PreparedMaterializationAttempt::Result(_))
        );
        if !kind_matches {
            return Err(RepositorySessionError::KindMismatch);
        }

        let mut state = self
            .state
            .lock()
            .expect("repository materializer mutex poisoned");
        let active = matching_validated_active(&state, token)?;
        let mut inherited = None;
        if let Some(existing) = active
            .entries
            .iter()
            .find(|entry| entry.request.id == request.id)
        {
            inherited = active
                .reusable
                .iter()
                .position(|accepted| accepted.request.id == request.id);
            if inherited.is_none() && *existing.request != *request {
                return Err(RepositorySessionError::ConflictingRequest(
                    request.id.canonical_repo.clone(),
                ));
            } else if inherited.is_none() {
                return complete_epoch(&self.workspace, &active.entries);
            }
        }
        let result = match (request.kind.clone(), prepared) {
            (
                RepositoryMaterializationKind::Local { .. },
                PreparedMaterializationAttempt::Local,
            ) => RepositoryMaterializationResult::Success(RepositoryMaterializationSuccess::Local),
            (
                RepositoryMaterializationKind::Immutable,
                PreparedMaterializationAttempt::Immutable {
                    source_identity,
                    root,
                },
            ) => {
                let mut candidate = state.next_instance;
                let observation_instance = match allocate_observation_instance(&mut candidate) {
                    Ok(observation_instance) => observation_instance,
                    Err(_) => {
                        drop(state);
                        drop(root);
                        return Err(RepositorySessionError::InstanceExhausted);
                    }
                };
                state.next_instance = candidate;
                let generation_root = root.path().to_path_buf();
                matching_validated_active_mut(&mut state, token)?
                    .provisional_roots
                    .push(RetainedRepositoryRoot {
                        observation_instance,
                        root: Arc::new(root),
                    });
                RepositoryMaterializationResult::Success(
                    RepositoryMaterializationSuccess::Immutable {
                        source_identity,
                        generation_root,
                        observation_instance,
                    },
                )
            }
            (_, PreparedMaterializationAttempt::Result(result)) => result,
            _ => unreachable!("materialization kind was validated before locking"),
        };
        let entry = RepositoryMaterializationEpochEntry {
            request: request.clone(),
            result,
        };
        let active = matching_validated_active_mut(&mut state, token)?;
        if let Some(index) = inherited {
            active.reusable.remove(index);
        }
        upsert_epoch_entry(&mut active.entries, entry);
        complete_epoch(&self.workspace, &active.entries)
    }

    pub(super) fn materialize_native(
        &self,
        token: RepositorySessionToken,
        request: Arc<RepositoryMaterializationRequest>,
        generation: RepositoryMaterializationGeneration,
    ) -> Result<RepositoryMaterializationResultEpoch, RepositorySessionError> {
        {
            let state = self
                .state
                .lock()
                .expect("repository materializer mutex poisoned");
            matching_validated_active(&state, token)?;
        }
        validate_native_request(&self.workspace, &request)?;
        let workspace = self.workspace.as_path();
        let request_for_io = request.clone();
        self.materialize_with(token, request, generation, || {
            materialize_native_attempt(workspace, &request_for_io)
        })
    }

    fn epoch(
        &self,
        token: RepositorySessionToken,
    ) -> Result<RepositoryMaterializationResultEpoch, RepositorySessionError> {
        let state = self
            .state
            .lock()
            .expect("repository materializer mutex poisoned");
        let active = matching_validated_active(&state, token)?;
        complete_epoch(&self.workspace, &active.entries)
    }

    pub(super) fn selected_epoch(
        &self,
        token: RepositorySessionToken,
        selected: &[Arc<RepositoryMaterializationRequest>],
    ) -> Result<RepositoryMaterializationResultEpoch, RepositorySessionError> {
        let state = self
            .state
            .lock()
            .expect("repository materializer mutex poisoned");
        let active = matching_validated_active(&state, token)?;
        let mut entries = Vec::with_capacity(selected.len());
        for request in selected {
            let entry = active
                .entries
                .iter()
                .find(|entry| entry.request.id == request.id)
                .ok_or_else(|| {
                    RepositorySessionError::UnknownSelection(request.id.canonical_repo.clone())
                })?;
            if entry.request != *request {
                return Err(RepositorySessionError::ConflictingRequest(
                    request.id.canonical_repo.clone(),
                ));
            }
            entries.push(entry.clone());
        }
        complete_epoch(&self.workspace, &entries)
    }

    pub(super) fn observe_native(
        &self,
        token: RepositorySessionToken,
        demands: impl IntoIterator<Item = PathObservationDemand>,
    ) -> Result<PathObservationEpoch, RepositorySessionError> {
        let root_owners = {
            let state = self
                .state
                .lock()
                .expect("repository materializer mutex poisoned");
            let active = matching_validated_active(&state, token)?;
            let mut roots = state
                .accepted_roots
                .iter()
                .map(|root| (root.observation_instance, root.root.clone()))
                .collect::<Vec<_>>();
            roots.extend(
                active
                    .provisional_roots
                    .iter()
                    .map(|root| (root.observation_instance, root.root.clone())),
            );
            roots
        };
        let roots = normalize_retained_roots(&root_owners)?;
        let observed = super::path_observation::observe_native(&root_owners, roots, demands)
            .map_err(RepositorySessionError::NativeObservation);
        let state = self
            .state
            .lock()
            .expect("repository materializer mutex poisoned");
        matching_validated_active(&state, token)?;
        observed
    }

    pub(super) fn accept(
        &self,
        token: RepositorySessionToken,
        selected: &[Arc<RepositoryMaterializationRequest>],
        validation: Vec<RepositoryValidation>,
    ) -> Result<(), RepositorySessionError> {
        let released = {
            let mut state = self
                .state
                .lock()
                .expect("repository materializer mutex poisoned");
            let active = matching_validated_active(&state, token)?;
            let accepted = selected_acceptance(active, selected, validation)?;
            let selected_instances = accepted
                .iter()
                .filter_map(|accepted| match &accepted.success {
                    RepositoryMaterializationSuccess::Local => None,
                    RepositoryMaterializationSuccess::Immutable {
                        observation_instance,
                        ..
                    } => Some(*observation_instance),
                })
                .collect::<Vec<_>>();
            let mut active = state.active.take().expect("matching active session exists");
            let mut retained = Vec::new();
            let mut released = Vec::new();
            for root in active.provisional_roots.drain(..) {
                if selected_instances.contains(&root.observation_instance) {
                    retained.push(root);
                } else {
                    released.push(root);
                }
            }
            state.accepted_roots.extend(retained);
            state.accepted = accepted;
            released
        };
        drop(released);
        Ok(())
    }

    pub(super) fn discard(
        &self,
        token: RepositorySessionToken,
    ) -> Result<(), RepositorySessionError> {
        let released = {
            let mut state = self
                .state
                .lock()
                .expect("repository materializer mutex poisoned");
            matching_active(&state, token)?;
            state
                .active
                .take()
                .expect("matching active session exists")
                .provisional_roots
        };
        drop(released);
        Ok(())
    }
}

#[allow(dead_code)]
enum PreparedMaterializationAttempt {
    Local,
    Immutable {
        source_identity: Arc<str>,
        root: tempfile::TempDir,
    },
    Result(RepositoryMaterializationResult),
}

#[allow(dead_code)]
fn allocate_session_token(
    next_token: &mut u64,
) -> Result<RepositorySessionToken, RepositorySessionError> {
    let current = *next_token;
    if current == 0 {
        return Err(RepositorySessionError::TokenExhausted);
    }
    let successor = current
        .checked_add(1)
        .ok_or(RepositorySessionError::TokenExhausted)?;
    *next_token = successor;
    Ok(RepositorySessionToken(current))
}

#[allow(dead_code)]
fn matching_active(
    state: &RepositoryMaterializerState,
    token: RepositorySessionToken,
) -> Result<&ActiveRepositorySession, RepositorySessionError> {
    match state.active.as_ref() {
        Some(active) if active.token == token => Ok(active),
        active => Err(RepositorySessionError::StaleToken {
            active: active.map(|active| active.token),
            supplied: token,
        }),
    }
}

#[allow(dead_code)]
fn matching_active_mut(
    state: &mut RepositoryMaterializerState,
    token: RepositorySessionToken,
) -> Result<&mut ActiveRepositorySession, RepositorySessionError> {
    match state.active.as_mut() {
        Some(active) if active.token == token => Ok(active),
        active => Err(RepositorySessionError::StaleToken {
            active: active.as_deref().map(|active| active.token),
            supplied: token,
        }),
    }
}

#[allow(dead_code)]
fn matching_validated_active(
    state: &RepositoryMaterializerState,
    token: RepositorySessionToken,
) -> Result<&ActiveRepositorySession, RepositorySessionError> {
    let active = matching_active(state, token)?;
    if active.validation != RepositorySessionValidation::Complete {
        return Err(RepositorySessionError::ValidationIncomplete);
    }
    Ok(active)
}

#[allow(dead_code)]
fn matching_validated_active_mut(
    state: &mut RepositoryMaterializerState,
    token: RepositorySessionToken,
) -> Result<&mut ActiveRepositorySession, RepositorySessionError> {
    let active = matching_active_mut(state, token)?;
    if active.validation != RepositorySessionValidation::Complete {
        return Err(RepositorySessionError::ValidationIncomplete);
    }
    Ok(active)
}

#[allow(dead_code)]
fn upsert_epoch_entry(
    entries: &mut Vec<RepositoryMaterializationEpochEntry>,
    entry: RepositoryMaterializationEpochEntry,
) {
    let repo = &entry.request.id.canonical_repo;
    match entries.binary_search_by(|candidate| candidate.request.id.canonical_repo.cmp(repo)) {
        Ok(index) => entries[index] = entry,
        Err(index) => entries.insert(index, entry),
    }
}

#[allow(dead_code)]
fn complete_epoch(
    workspace: &NormalizedAbsolutePath,
    entries: &[RepositoryMaterializationEpochEntry],
) -> Result<RepositoryMaterializationResultEpoch, RepositorySessionError> {
    RepositoryMaterializationResultEpoch::new(workspace.clone(), entries.iter().cloned())
        .map_err(|_| RepositorySessionError::WrongWorkspace)
}

#[allow(dead_code)]
fn selected_acceptance(
    active: &ActiveRepositorySession,
    selected: &[Arc<RepositoryMaterializationRequest>],
    validation: Vec<RepositoryValidation>,
) -> Result<Vec<AcceptedRepository>, RepositorySessionError> {
    let mut selected = selected.to_vec();
    selected.sort_by(|left, right| left.id.canonical_repo.cmp(&right.id.canonical_repo));
    if let Some(pair) = selected.windows(2).find(|pair| pair[0].id == pair[1].id) {
        return Err(if *pair[0] == *pair[1] {
            RepositorySessionError::DuplicateSelection(pair[0].id.canonical_repo.clone())
        } else {
            RepositorySessionError::ConflictingRequest(pair[0].id.canonical_repo.clone())
        });
    }
    let mut validation = validation;
    validation.sort_by(|left, right| {
        left.request
            .id
            .canonical_repo
            .cmp(&right.request.id.canonical_repo)
    });
    if let Some(pair) = validation
        .windows(2)
        .find(|pair| pair[0].request.id == pair[1].request.id)
    {
        return Err(if *pair[0].request == *pair[1].request {
            RepositorySessionError::InvalidValidation(pair[0].request.id.canonical_repo.clone())
        } else {
            RepositorySessionError::ConflictingRequest(pair[0].request.id.canonical_repo.clone())
        });
    }
    let mut accepted = Vec::new();
    for request in selected {
        let entry = active
            .entries
            .iter()
            .find(|entry| entry.request.id == request.id)
            .ok_or_else(|| {
                RepositorySessionError::UnknownSelection(request.id.canonical_repo.clone())
            })?;
        if *entry.request != *request {
            return Err(RepositorySessionError::ConflictingRequest(
                request.id.canonical_repo.clone(),
            ));
        }
        let RepositoryMaterializationResult::Success(success) = &entry.result else {
            return Err(RepositorySessionError::NonSuccessSelection(
                request.id.canonical_repo.clone(),
            ));
        };
        let observations = validation
            .iter()
            .find(|candidate| candidate.request.id == request.id)
            .map(|candidate| {
                if *candidate.request != *request {
                    Err(RepositorySessionError::ConflictingRequest(
                        request.id.canonical_repo.clone(),
                    ))
                } else {
                    Ok(candidate.observations.clone())
                }
            })
            .transpose()?
            .unwrap_or_else(|| Arc::from([]));
        if !valid_validation(success, &observations) {
            return Err(RepositorySessionError::InvalidValidation(
                request.id.canonical_repo.clone(),
            ));
        }
        accepted.push(AcceptedRepository {
            request: entry.request.clone(),
            success: success.clone(),
            validation: observations,
        });
    }
    if let Some(unselected) = validation.iter().find(|candidate| {
        !accepted
            .iter()
            .any(|entry| entry.request.id == candidate.request.id)
    }) {
        return Err(RepositorySessionError::InvalidValidation(
            unselected.request.id.canonical_repo.clone(),
        ));
    }
    Ok(accepted)
}

#[allow(dead_code)]
fn valid_validation(
    success: &RepositoryMaterializationSuccess,
    observations: &[(PathObservationDemand, PathObservationResult)],
) -> bool {
    observations
        .iter()
        .enumerate()
        .all(|(index, (demand, result))| {
            !observations[..index]
                .iter()
                .any(|(prior, _)| prior == demand)
                && demand.operation() == result.operation()
                && match success {
                    RepositoryMaterializationSuccess::Local => {
                        demand.namespace() == PathObservationNamespace::Host
                    }
                    RepositoryMaterializationSuccess::Immutable {
                        observation_instance,
                        ..
                    } => {
                        demand.namespace()
                            == PathObservationNamespace::Materialization(*observation_instance)
                    }
                }
        })
}

#[allow(dead_code)]
fn normalize_retained_roots(
    roots: &[(PathObservationInstanceId, Arc<tempfile::TempDir>)],
) -> Result<Vec<(PathObservationInstanceId, NormalizedAbsolutePath)>, RepositorySessionError> {
    roots
        .iter()
        .map(|(instance, root)| {
            let path = root.path().to_path_buf();
            NormalizedAbsolutePath::new(path.clone())
                .map(|root| (*instance, root))
                .map_err(|_| RepositorySessionError::InvalidRetainedRoot(path))
        })
        .collect()
}

#[allow(dead_code)]
fn validation_epoch_is_dirty(
    previous: &[(PathObservationDemand, PathObservationResult)],
    observed: &PathObservationEpoch,
) -> bool {
    previous.iter().any(|(demand, prior_result)| {
        observed.get(demand).is_none_or(|observed| {
            observation_is_dirty(
                demand,
                prior_result,
                observed,
                previous_has_file_bytes(previous, demand),
            )
        })
    })
}

#[allow(dead_code)]
fn validation_is_dirty(
    previous: &[(PathObservationDemand, PathObservationResult)],
    roots: &[(PathObservationInstanceId, PathBuf)],
    mut observe: impl FnMut(&PathObservationDemand, Option<&Path>) -> PathObservationResult,
) -> bool {
    let mut observed = Vec::with_capacity(previous.len());
    for (demand, _) in previous {
        let root = match demand.namespace() {
            PathObservationNamespace::Host => None,
            PathObservationNamespace::Materialization(instance) => {
                let Some((_, root)) = roots.iter().find(|(candidate, _)| *candidate == instance)
                else {
                    return true;
                };
                Some(root.as_path())
            }
        };
        observed.push(observe(demand, root));
    }
    previous
        .iter()
        .zip(&observed)
        .any(|((demand, prior_result), observed)| {
            observation_is_dirty(
                demand,
                prior_result,
                observed,
                previous_has_file_bytes(previous, demand),
            )
        })
}

#[allow(dead_code)]
fn previous_has_file_bytes(
    previous: &[(PathObservationDemand, PathObservationResult)],
    demand: &PathObservationDemand,
) -> bool {
    previous.iter().any(|(candidate, _)| {
        candidate.namespace() == demand.namespace()
            && candidate.path() == demand.path()
            && candidate.operation() == PathObservationOperation::FileBytes
    })
}

#[allow(dead_code)]
fn observation_is_dirty(
    demand: &PathObservationDemand,
    previous: &PathObservationResult,
    observed: &PathObservationResult,
    has_file_bytes: bool,
) -> bool {
    if observed.operation() != demand.operation() || observation_has_error(observed) {
        return true;
    }
    match (previous, observed) {
        (PathObservationResult::Lstat(previous), PathObservationResult::Lstat(observed)) => {
            lstat_is_dirty(previous, observed, has_file_bytes)
        }
        (
            PathObservationResult::FileBytes(previous),
            PathObservationResult::FileBytes(observed),
        ) => previous != observed,
        (PathObservationResult::ReadLink(previous), PathObservationResult::ReadLink(observed)) => {
            previous != observed
        }
        (
            PathObservationResult::DirectoryEntries(previous),
            PathObservationResult::DirectoryEntries(observed),
        ) => previous != observed,
        (
            PathObservationResult::WindowsLongPath(previous),
            PathObservationResult::WindowsLongPath(observed),
        ) => previous != observed,
        (
            PathObservationResult::WindowsOptionPathLongName(previous),
            PathObservationResult::WindowsOptionPathLongName(observed),
        ) => previous != observed,
        _ => true,
    }
}

#[allow(dead_code)]
fn observation_has_error(result: &PathObservationResult) -> bool {
    match result {
        PathObservationResult::Lstat(result) => {
            matches!(result, PathOperationResult::Error(_))
        }
        PathObservationResult::ReadLink(result) => {
            matches!(result, PathOperationResult::Error(_))
        }
        PathObservationResult::FileBytes(result) => {
            matches!(result, PathOperationResult::Error(_))
        }
        PathObservationResult::DirectoryEntries(result) => {
            matches!(result, PathOperationResult::Error(_))
        }
        PathObservationResult::WindowsLongPath(_) => false,
        PathObservationResult::WindowsOptionPathLongName(_) => false,
    }
}

#[allow(dead_code)]
fn lstat_is_dirty(
    previous: &PathOperationResult<PathLstat>,
    observed: &PathOperationResult<PathLstat>,
    has_file_bytes: bool,
) -> bool {
    match (previous, observed) {
        (PathOperationResult::Missing, PathOperationResult::Missing) => false,
        (PathOperationResult::Present(previous), PathOperationResult::Present(observed)) => {
            if previous.kind() != observed.kind() {
                return true;
            }
            match previous.kind() {
                PathNodeKind::RegularFile | PathNodeKind::SpecialFile => {
                    previous.size() != observed.size()
                        || (!has_file_bytes
                            && (previous.node_id() != observed.node_id()
                                || previous.mtime_millis() != observed.mtime_millis()))
                }
                PathNodeKind::Directory | PathNodeKind::Symlink => false,
            }
        }
        _ => true,
    }
}

#[async_trait]
impl RepositoryIo for LocalRepositoryIo {
    async fn materialize(
        &self,
        workspace: &Path,
        repo_spec: &RepoSpec,
    ) -> Result<RepositoryIoOutcome, RepositoryTransportError> {
        let workspace = workspace.to_path_buf();
        let repo_spec = repo_spec.clone();
        let result = tokio::task::spawn_blocking(move || materialize(&workspace, &repo_spec))
            .await
            .map_err(|error| RepositoryTransportError {
                message: format!("joining repository materializer: {error}").into(),
            })??;
        match result {
            Materialized::Local { source_root } => Ok(RepositoryIoOutcome::Local { source_root }),
            Materialized::Immutable { bytes, root } => {
                let source_identity = source_identity(&bytes);
                let (generation_root, observation_instance) = self.retain(root)?;
                Ok(RepositoryIoOutcome::Immutable {
                    source_identity,
                    generation_root,
                    observation_instance,
                })
            }
        }
    }
}

enum Materialized {
    Local {
        source_root: PathBuf,
    },
    Immutable {
        bytes: Vec<u8>,
        root: tempfile::TempDir,
    },
}

fn materialize(
    workspace: &Path,
    repo_spec: &RepoSpec,
) -> Result<Materialized, RepositoryTransportError> {
    let bzl_file = repo_spec.rule_id.bzl_file.to_string();
    match (bzl_file.as_str(), repo_spec.rule_id.rule_name.as_str()) {
        ("@@bazel_tools//tools/build_defs/repo:local.bzl", "local_repository") => {
            materialize_local(workspace, repo_spec)
        }
        ("@@bazel_tools//tools/build_defs/repo:http.bzl", "http_archive") => {
            materialize_archive(repo_spec)
        }
        ("@@bazel_tools//tools/build_defs/repo:git.bzl", "git_repository") => {
            materialize_git(repo_spec)
        }
        _ => Err(unsupported("unsupported repository override rule")),
    }
}

fn materialize_native_attempt(
    workspace: &Path,
    request: &RepositoryMaterializationRequest,
) -> RepositoryMaterializationAttempt {
    let bzl_file = request.repo_spec.rule_id.bzl_file.to_string();
    match (
        bzl_file.as_str(),
        request.repo_spec.rule_id.rule_name.as_str(),
    ) {
        ("@@bazel_tools//tools/build_defs/repo:local.bzl", "local_repository") => {
            match validate_local_spec(workspace, &request.repo_spec) {
                Ok(()) => RepositoryMaterializationAttempt::Local,
                Err(error) => RepositoryMaterializationAttempt::SpecError(error.message.into()),
            }
        }
        ("@@bazel_tools//tools/build_defs/repo:http.bzl", "http_archive") => materialized_attempt(
            materialize_archive_with(&request.repo_spec, &mut NativeArchiveIo),
        ),
        ("@@bazel_tools//tools/build_defs/repo:git.bzl", "git_repository") => {
            materialized_attempt(materialize_git_staged(&request.repo_spec))
        }
        _ => RepositoryMaterializationAttempt::SpecError(
            "unsupported repository override rule".into(),
        ),
    }
}

fn validate_native_request(
    workspace: &NormalizedAbsolutePath,
    request: &RepositoryMaterializationRequest,
) -> Result<(), RepositorySessionError> {
    if request.id.workspace != *workspace {
        return Err(RepositorySessionError::WrongWorkspace);
    }
    let bzl_file = request.repo_spec.rule_id.bzl_file.to_string();
    match (
        bzl_file.as_str(),
        request.repo_spec.rule_id.rule_name.as_str(),
        &request.kind,
    ) {
        (
            "@@bazel_tools//tools/build_defs/repo:local.bzl",
            "local_repository",
            RepositoryMaterializationKind::Local { logical_root },
        ) => {
            if let Ok(relative) = local_relative_path(&request.repo_spec) {
                let expected = NormalizedAbsolutePath::new(workspace.as_path().join(relative))
                    .map_err(|_| RepositorySessionError::KindMismatch)?;
                if *logical_root != expected {
                    return Err(RepositorySessionError::KindMismatch);
                }
            }
            Ok(())
        }
        (
            "@@bazel_tools//tools/build_defs/repo:http.bzl",
            "http_archive",
            RepositoryMaterializationKind::Immutable,
        )
        | (
            "@@bazel_tools//tools/build_defs/repo:git.bzl",
            "git_repository",
            RepositoryMaterializationKind::Immutable,
        ) => Ok(()),
        ("@@bazel_tools//tools/build_defs/repo:local.bzl", "local_repository", _)
        | ("@@bazel_tools//tools/build_defs/repo:http.bzl", "http_archive", _)
        | ("@@bazel_tools//tools/build_defs/repo:git.bzl", "git_repository", _) => {
            Err(RepositorySessionError::KindMismatch)
        }
        _ => Ok(()),
    }
}

fn materialized_attempt(
    materialized: Result<Materialized, ArchiveMaterializationError>,
) -> RepositoryMaterializationAttempt {
    match materialized {
        Ok(Materialized::Local { .. }) => RepositoryMaterializationAttempt::Local,
        Ok(Materialized::Immutable { bytes, root }) => {
            RepositoryMaterializationAttempt::Immutable { bytes, root }
        }
        Err(error) => match error.stage {
            ArchiveFailureStage::Spec => RepositoryMaterializationAttempt::SpecError(error.message),
            ArchiveFailureStage::Transport => {
                RepositoryMaterializationAttempt::TransportError(error.message)
            }
            ArchiveFailureStage::Materialization => {
                RepositoryMaterializationAttempt::MaterializationError(error.message)
            }
        },
    }
}

fn validate_local_spec(
    _workspace: &Path,
    repo_spec: &RepoSpec,
) -> Result<(), RepositoryTransportError> {
    local_relative_path(repo_spec).map(|_| ())
}

fn local_relative_path(repo_spec: &RepoSpec) -> Result<&Path, RepositoryTransportError> {
    reject_extra_attributes(repo_spec, &["path"])?;
    let path = Path::new(required_string(repo_spec, "path")?);
    if path.is_absolute() || !normalized_relative(path) {
        return Err(unsupported(
            "local_repository path must be normalized and workspace-relative",
        ));
    }
    Ok(path)
}

fn materialize_local(
    workspace: &Path,
    repo_spec: &RepoSpec,
) -> Result<Materialized, RepositoryTransportError> {
    validate_local_spec(workspace, repo_spec)?;
    let path = Path::new(required_string(repo_spec, "path")?);
    let source_root =
        workspace
            .join(path)
            .canonicalize()
            .map_err(|error| RepositoryTransportError {
                message: format!(
                    "canonicalizing local_repository path {}: {error}",
                    path.display()
                )
                .into(),
            })?;
    if !source_root.starts_with(workspace) {
        return Err(unsupported("local_repository path escapes the workspace"));
    }
    Ok(Materialized::Local { source_root })
}

fn materialize_archive(repo_spec: &RepoSpec) -> Result<Materialized, RepositoryTransportError> {
    materialize_archive_with(repo_spec, &mut NativeArchiveIo)
        .map_err(ArchiveMaterializationError::into_repository)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ArchiveFailureStage {
    Spec,
    Transport,
    Materialization,
}

#[derive(Debug)]
struct ArchiveMaterializationError {
    stage: ArchiveFailureStage,
    message: String,
}

impl ArchiveMaterializationError {
    fn new(stage: ArchiveFailureStage, message: impl Into<String>) -> Self {
        Self {
            stage,
            message: message.into(),
        }
    }

    fn spec(message: impl Into<String>) -> Self {
        Self::new(ArchiveFailureStage::Spec, message)
    }

    fn transport(message: impl Into<String>) -> Self {
        Self::new(ArchiveFailureStage::Transport, message)
    }

    fn materialization(message: impl Into<String>) -> Self {
        Self::new(ArchiveFailureStage::Materialization, message)
    }

    fn into_repository(self) -> RepositoryTransportError {
        let _stage = self.stage;
        RepositoryTransportError {
            message: self.message.into(),
        }
    }
}

enum SavedChecksum {
    Valid(String),
    Malformed,
}

struct CapturedArchive {
    bytes: Vec<u8>,
    _artifact: tempfile::NamedTempFile,
}

trait ArchiveIo: ArchiveDestination {
    fn create_root(&mut self) -> std::io::Result<tempfile::TempDir>;
    fn create_capture(&mut self) -> std::io::Result<tempfile::NamedTempFile>;
    fn read_source(&mut self, source: &Path) -> std::io::Result<Vec<u8>>;
    fn write_capture(
        &mut self,
        capture: &mut tempfile::NamedTempFile,
        bytes: &[u8],
    ) -> std::io::Result<()>;
    fn flush_capture(&mut self, capture: &mut tempfile::NamedTempFile) -> std::io::Result<()>;
}

struct NativeArchiveIo;

impl ArchiveIo for NativeArchiveIo {
    fn create_root(&mut self) -> std::io::Result<tempfile::TempDir> {
        tempfile::tempdir()
    }

    fn create_capture(&mut self) -> std::io::Result<tempfile::NamedTempFile> {
        tempfile::NamedTempFile::new()
    }

    fn read_source(&mut self, source: &Path) -> std::io::Result<Vec<u8>> {
        std::fs::read(source)
    }

    fn write_capture(
        &mut self,
        capture: &mut tempfile::NamedTempFile,
        bytes: &[u8],
    ) -> std::io::Result<()> {
        capture.write_all(bytes)
    }

    fn flush_capture(&mut self, capture: &mut tempfile::NamedTempFile) -> std::io::Result<()> {
        capture.flush()
    }
}

impl ArchiveDestination for NativeArchiveIo {
    fn create_parent(&mut self, path: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(path)
    }

    fn create_directory(&mut self, path: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(path)
    }

    fn write_regular(&mut self, path: &Path, bytes: &[u8]) -> std::io::Result<()> {
        let mut output = File::create(path)?;
        output.write_all(bytes)
    }
}

fn materialize_archive_with(
    repo_spec: &RepoSpec,
    io: &mut impl ArchiveIo,
) -> Result<Materialized, ArchiveMaterializationError> {
    reject_extra_attributes(repo_spec, &["urls", "sha256", "type", "strip_prefix"])
        .map_err(|error| ArchiveMaterializationError::spec(error.message))?;
    let urls = repo_spec.attributes.get("urls").ok_or_else(|| {
        ArchiveMaterializationError::spec("http_archive requires exactly one file URL")
    })?;
    let OverrideAttributeValue::Iterable(urls) = urls else {
        return Err(ArchiveMaterializationError::spec(
            "http_archive urls must contain exactly one file URL",
        ));
    };
    let [OverrideAttributeValue::String(url)] = urls.as_ref() else {
        return Err(ArchiveMaterializationError::spec(
            "http_archive urls must contain exactly one file URL",
        ));
    };
    let archive =
        local_file_uri(url).map_err(|error| ArchiveMaterializationError::spec(error.message))?;
    if optional_string(repo_spec, "type")
        .map_err(|error| ArchiveMaterializationError::spec(error.message))?
        != Some("tar")
    {
        return Err(ArchiveMaterializationError::spec(
            "http_archive type must be exactly tar",
        ));
    }
    let strip_prefix = optional_string(repo_spec, "strip_prefix")
        .map_err(|error| ArchiveMaterializationError::spec(error.message))?
        .map(latin1_bytes);
    if strip_prefix
        .as_ref()
        .is_some_and(|prefix| prefix.contains(&0))
    {
        return Err(ArchiveMaterializationError::spec(
            "http_archive strip_prefix contains a NUL byte",
        ));
    }
    let prefix = strip_prefix
        .as_deref()
        .map(|value| normalize_raw_tar_path(value, native_path_flavor()))
        .transpose()?;
    if let Some(prefix) = prefix.as_deref() {
        validate_strip_prefix(prefix, native_path_flavor())?;
    }
    let expected_sha256 = required_string(repo_spec, "sha256")
        .map_err(|error| ArchiveMaterializationError::spec(error.message))?;
    let saved_checksum = if expected_sha256.len() == 64
        && expected_sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        SavedChecksum::Valid(expected_sha256.to_owned())
    } else {
        SavedChecksum::Malformed
    };

    let root = io.create_root().map_err(|error| {
        ArchiveMaterializationError::materialization(format!(
            "creating archive materialization root: {error}"
        ))
    })?;
    let mut artifact = io.create_capture().map_err(|error| {
        ArchiveMaterializationError::materialization(format!(
            "creating temporary http_archive capture: {error}"
        ))
    })?;
    let bytes = io.read_source(&archive).map_err(|error| {
        ArchiveMaterializationError::transport(format!(
            "reading http_archive {}: {error}",
            archive.display()
        ))
    })?;
    io.write_capture(&mut artifact, &bytes).map_err(|error| {
        ArchiveMaterializationError::transport(format!(
            "writing temporary http_archive capture: {error}"
        ))
    })?;
    io.flush_capture(&mut artifact).map_err(|error| {
        ArchiveMaterializationError::transport(format!(
            "flushing temporary http_archive capture: {error}"
        ))
    })?;
    let captured = CapturedArchive {
        bytes,
        _artifact: artifact,
    };

    let SavedChecksum::Valid(expected_sha256) = saved_checksum else {
        return Err(ArchiveMaterializationError::spec(
            "http_archive sha256 must be an exact 64-character hexadecimal digest",
        ));
    };
    let actual_sha256 = format!("{:x}", Sha256::digest(&captured.bytes));
    if !actual_sha256.eq_ignore_ascii_case(&expected_sha256) {
        return Err(ArchiveMaterializationError::transport(
            "http_archive sha256 does not match the local tar",
        ));
    }

    let plan = inspect_and_plan_ustar(&captured.bytes, prefix.as_deref(), root.path())?;
    extract_ustar_plan(&captured.bytes, &plan, io)?;
    Ok(Materialized::Immutable {
        bytes: captured.bytes,
        root,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(dead_code)] // Both flavors are exercised by host-pure tests.
enum PathFlavor {
    Unix,
    Windows,
}

#[cfg(windows)]
fn native_path_flavor() -> PathFlavor {
    PathFlavor::Windows
}

#[cfg(not(windows))]
fn native_path_flavor() -> PathFlavor {
    PathFlavor::Unix
}

fn latin1_bytes(value: &str) -> Vec<u8> {
    value
        .chars()
        .map(|character| u8::try_from(u32::from(character)).unwrap_or(b'?'))
        .collect()
}

fn validate_strip_prefix(
    prefix: &[Vec<u8>],
    flavor: PathFlavor,
) -> Result<(), ArchiveMaterializationError> {
    if prefix.is_empty() || join_raw_components(Path::new(""), prefix, flavor).is_err() {
        return Err(ArchiveMaterializationError::spec(
            "http_archive strip_prefix must normalize to a safe relative path",
        ));
    }
    Ok(())
}

fn normalize_raw_tar_path(
    value: &[u8],
    flavor: PathFlavor,
) -> Result<Vec<Vec<u8>>, ArchiveMaterializationError> {
    let is_separator = |byte| byte == b'/' || (flavor == PathFlavor::Windows && byte == b'\\');
    let drive_absolute = flavor == PathFlavor::Windows
        && value.len() >= 3
        && value[0].is_ascii_alphabetic()
        && value[1] == b':'
        && is_separator(value[2]);
    let absolute = value.first().is_some_and(|byte| is_separator(*byte)) || drive_absolute;
    let mut start = if drive_absolute { 3 } else { 0 };
    while start < value.len() && is_separator(value[start]) {
        start += 1;
    }
    let mut components: Vec<Vec<u8>> = Vec::new();
    for component in value[start..].split(|byte| is_separator(*byte)) {
        if component.is_empty() || component == b"." {
            continue;
        }
        if component == b".." {
            if components
                .last()
                .is_some_and(|last| last.as_slice() != b"..")
            {
                components.pop();
            } else if !absolute {
                components.push(component.to_vec());
            }
        } else {
            components.push(component.to_vec());
        }
    }
    Ok(components)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PlannedUstarKind {
    Regular,
    Directory,
}

#[derive(Debug)]
struct PlannedUstarEntry {
    components: Vec<Vec<u8>>,
    path: PathBuf,
    payload: Range<usize>,
    kind: PlannedUstarKind,
}

#[derive(Debug, Default)]
struct UstarExtractionPlan {
    entries: Vec<PlannedUstarEntry>,
}

fn inspect_and_plan_ustar(
    bytes: &[u8],
    prefix: Option<&[Vec<u8>]>,
    root: &Path,
) -> Result<UstarExtractionPlan, ArchiveMaterializationError> {
    let flavor = native_path_flavor();
    inspect_and_plan_ustar_for_flavor(bytes, prefix, flavor, root)
}

fn inspect_and_plan_ustar_for_flavor(
    bytes: &[u8],
    prefix: Option<&[Vec<u8>]>,
    flavor: PathFlavor,
    root: &Path,
) -> Result<UstarExtractionPlan, ArchiveMaterializationError> {
    let mut plan = UstarExtractionPlan::default();
    let mut offset = 0usize;
    let mut found_prefix = false;
    while offset < bytes.len() {
        if bytes.len() - offset < 512 {
            break;
        }
        let header = &bytes[offset..offset + 512];
        if header.iter().all(|byte| *byte == 0) {
            break;
        }
        let size = parse_ustar_octal(&header[124..136])?;
        parse_ustar_octal(&header[148..156]).map_err(|_| {
            ArchiveMaterializationError::materialization(
                "http_archive tar entry has a malformed checksum field",
            )
        })?;
        reject_non_ustar_layout(header)?;
        let payload_start = offset + 512;
        let payload_end = payload_start.checked_add(size).ok_or_else(|| {
            ArchiveMaterializationError::materialization(
                "http_archive tar entry payload length overflows",
            )
        })?;
        if payload_end > bytes.len() {
            return Err(ArchiveMaterializationError::materialization(
                "http_archive tar entry payload is truncated",
            ));
        }
        let padding = (512 - size % 512) % 512;
        let next_offset = payload_end.checked_add(padding).ok_or_else(|| {
            ArchiveMaterializationError::materialization(
                "http_archive tar entry padding length overflows",
            )
        })?;
        if next_offset > bytes.len() {
            return Err(ArchiveMaterializationError::materialization(
                "http_archive tar entry padding is truncated",
            ));
        }

        let name = nul_terminated(&header[..100]);
        let raw_prefix = nul_terminated(&header[345..500]);
        let mut raw_path =
            Vec::with_capacity(raw_prefix.len() + usize::from(!raw_prefix.is_empty()) + name.len());
        if !raw_prefix.is_empty() {
            raw_path.extend_from_slice(raw_prefix);
            raw_path.push(b'/');
        }
        raw_path.extend_from_slice(name);
        let normalized = normalize_raw_tar_path(&raw_path, flavor)?;
        let selected = match prefix {
            None => Some(normalized.as_slice()),
            Some(prefix) if normalized.starts_with(prefix) => {
                found_prefix = true;
                Some(&normalized[prefix.len()..])
            }
            Some(_) => None,
        };
        if let Some(selected) = selected {
            let kind = match header[156] {
                b'5' => PlannedUstarKind::Directory,
                0 | b'0' if raw_path.ends_with(b"/") => PlannedUstarKind::Directory,
                0 | b'0' => PlannedUstarKind::Regular,
                _ => {
                    return Err(ArchiveMaterializationError::materialization(
                        "http_archive contains an unsupported tar entry type",
                    ));
                }
            };
            if !selected.is_empty() {
                let path = join_raw_components(root, selected, flavor)?;
                reject_namespace_collision(&plan.entries, selected, kind)?;
                plan.entries.push(PlannedUstarEntry {
                    components: selected.to_vec(),
                    path,
                    payload: payload_start..payload_end,
                    kind,
                });
            }
        }
        offset = next_offset;
    }
    if prefix.is_some() && !found_prefix {
        return Err(ArchiveMaterializationError::materialization(
            "http_archive strip_prefix was not found",
        ));
    }
    Ok(plan)
}

fn reject_non_ustar_layout(header: &[u8]) -> Result<(), ArchiveMaterializationError> {
    if &header[257..263] == b"ustar " || (&header[257..263] == b"ustar\0" && is_xstar(header)) {
        return Err(ArchiveMaterializationError::materialization(
            "http_archive contains an unsupported tar header layout",
        ));
    }
    Ok(())
}

fn is_xstar(header: &[u8]) -> bool {
    if &header[508..512] == b"tar\0" {
        return true;
    }
    if header[475] != 0
        && (header[156] != b'M' || ((header[464] & 0x80) == 0 && header[475] != b' '))
    {
        return false;
    }
    xstar_time_is_valid(&header[476..488]) && xstar_time_is_valid(&header[488..500])
}

fn xstar_time_is_valid(field: &[u8]) -> bool {
    field[0] & 0x80 != 0
        || (field[..field.len() - 1]
            .iter()
            .all(|byte| matches!(byte, b'0'..=b'7'))
            && matches!(field[field.len() - 1], 0 | b' '))
}

fn parse_ustar_octal(field: &[u8]) -> Result<usize, ArchiveMaterializationError> {
    if field.first() == Some(&0) {
        return Ok(0);
    }
    if field.first().is_some_and(|byte| byte & 0x80 != 0) {
        return Err(ArchiveMaterializationError::materialization(
            "http_archive tar entry uses an unsupported base-256 size",
        ));
    }
    let mut start = 0;
    while field.get(start) == Some(&b' ') {
        start += 1;
    }
    let mut end = field.len();
    while end > start && matches!(field[end - 1], 0 | b' ') {
        end -= 1;
    }
    let mut value = 0usize;
    for byte in &field[start..end] {
        if !matches!(byte, b'0'..=b'7') {
            return Err(ArchiveMaterializationError::materialization(
                "http_archive tar entry has a malformed size",
            ));
        }
        value = value
            .checked_mul(8)
            .and_then(|value| value.checked_add(usize::from(*byte - b'0')))
            .ok_or_else(|| {
                ArchiveMaterializationError::materialization(
                    "http_archive tar entry size overflows",
                )
            })?;
    }
    Ok(value)
}

fn nul_terminated(field: &[u8]) -> &[u8] {
    &field[..field
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(field.len())]
}

fn reject_namespace_collision(
    entries: &[PlannedUstarEntry],
    components: &[Vec<u8>],
    kind: PlannedUstarKind,
) -> Result<(), ArchiveMaterializationError> {
    for entry in entries {
        if entry.components == components {
            if entry.kind != kind {
                return Err(ArchiveMaterializationError::materialization(
                    "http_archive tar entries collide as file and directory",
                ));
            }
        } else if entry.kind == PlannedUstarKind::Regular
            && components.starts_with(&entry.components)
            || kind == PlannedUstarKind::Regular && entry.components.starts_with(components)
        {
            return Err(ArchiveMaterializationError::materialization(
                "http_archive tar entry collides with a regular-file ancestor",
            ));
        }
    }
    Ok(())
}

fn join_raw_components(
    root: &Path,
    components: &[Vec<u8>],
    flavor: PathFlavor,
) -> Result<PathBuf, ArchiveMaterializationError> {
    let mut result = root.to_path_buf();
    for component in components {
        if component.is_empty()
            || matches!(component.as_slice(), b"." | b"..")
            || component.contains(&0)
            || (flavor == PathFlavor::Windows
                && component
                    .iter()
                    .any(|byte| matches!(byte, b'/' | b'\\' | b':')))
        {
            return Err(ArchiveMaterializationError::materialization(
                "http_archive tar entry contains a non-normal path component",
            ));
        }
        let component = raw_os_string(component);
        let mut parsed = Path::new(&component).components();
        if !matches!(parsed.next(), Some(Component::Normal(value)) if value == component)
            || parsed.next().is_some()
        {
            return Err(ArchiveMaterializationError::materialization(
                "http_archive tar entry contains a non-normal OS component",
            ));
        }
        result.push(component);
    }
    if !result.starts_with(root) {
        return Err(ArchiveMaterializationError::materialization(
            "http_archive tar entry escapes the destination directory",
        ));
    }
    Ok(result)
}

#[cfg(unix)]
fn raw_os_string(bytes: &[u8]) -> OsString {
    use std::os::unix::ffi::OsStringExt;

    OsString::from_vec(bytes.to_vec())
}

#[cfg(windows)]
fn raw_os_string(bytes: &[u8]) -> OsString {
    OsString::from(
        bytes
            .iter()
            .map(|byte| char::from(*byte))
            .collect::<String>(),
    )
}

trait ArchiveDestination {
    fn create_parent(&mut self, path: &Path) -> std::io::Result<()>;
    fn create_directory(&mut self, path: &Path) -> std::io::Result<()>;
    fn write_regular(&mut self, path: &Path, bytes: &[u8]) -> std::io::Result<()>;
}

fn extract_ustar_plan(
    bytes: &[u8],
    plan: &UstarExtractionPlan,
    destination: &mut impl ArchiveDestination,
) -> Result<(), ArchiveMaterializationError> {
    for entry in &plan.entries {
        if let Some(parent) = entry.path.parent() {
            destination.create_parent(parent).map_err(|error| {
                ArchiveMaterializationError::materialization(format!(
                    "creating http_archive tar entry parent: {error}"
                ))
            })?;
        }
        let result = match entry.kind {
            PlannedUstarKind::Directory => destination.create_directory(&entry.path),
            PlannedUstarKind::Regular => {
                destination.write_regular(&entry.path, &bytes[entry.payload.clone()])
            }
        };
        result.map_err(|error| {
            ArchiveMaterializationError::materialization(format!(
                "extracting http_archive tar entry: {error}"
            ))
        })?;
    }
    Ok(())
}

fn materialize_git(repo_spec: &RepoSpec) -> Result<Materialized, RepositoryTransportError> {
    materialize_git_staged(repo_spec).map_err(ArchiveMaterializationError::into_repository)
}

fn materialize_git_staged(
    repo_spec: &RepoSpec,
) -> Result<Materialized, ArchiveMaterializationError> {
    materialize_git_staged_with(repo_spec, |archive, root| extract_tar(archive, root, None))
}

fn materialize_git_staged_with(
    repo_spec: &RepoSpec,
    extract: impl FnOnce(&Path, &Path) -> Result<(), RepositoryTransportError>,
) -> Result<Materialized, ArchiveMaterializationError> {
    reject_extra_attributes(repo_spec, &["remote", "commit"])
        .map_err(|error| ArchiveMaterializationError::spec(error.message))?;
    let remote = local_file_uri(
        required_string(repo_spec, "remote")
            .map_err(|error| ArchiveMaterializationError::spec(error.message))?,
    )
    .map_err(|error| ArchiveMaterializationError::spec(error.message))?;
    let commit = required_string(repo_spec, "commit")
        .map_err(|error| ArchiveMaterializationError::spec(error.message))?;
    if commit.len() != 40 || !commit.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(ArchiveMaterializationError::spec(
            "git_repository commit must be an exact 40-character hexadecimal commit",
        ));
    }
    if !remote.join("HEAD").is_file() {
        return Err(ArchiveMaterializationError::transport(
            "git_repository remote must be a local bare repository",
        ));
    }
    let output = Command::new("git")
        .arg(format!("--git-dir={}", remote.display()))
        .args(["archive", "--format=tar", commit])
        .output()
        .map_err(|error| {
            ArchiveMaterializationError::transport(format!(
                "running git archive for {}: {error}",
                remote.display()
            ))
        })?;
    if !output.status.success() {
        return Err(ArchiveMaterializationError::transport(format!(
            "git archive for {} at {} failed: {}",
            remote.display(),
            commit,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let root = tempfile::tempdir().map_err(|error| {
        ArchiveMaterializationError::materialization(format!(
            "creating git materialization root: {error}"
        ))
    })?;
    let archive = tempfile::NamedTempFile::new().map_err(|error| {
        ArchiveMaterializationError::materialization(format!(
            "creating temporary git archive: {error}"
        ))
    })?;
    std::fs::write(archive.path(), &output.stdout).map_err(|error| {
        ArchiveMaterializationError::materialization(format!(
            "writing temporary git archive: {error}"
        ))
    })?;
    extract(archive.path(), root.path())
        .map_err(|error| ArchiveMaterializationError::materialization(error.message))?;
    Ok(Materialized::Immutable {
        bytes: output.stdout,
        root,
    })
}

fn extract_tar(
    archive: &Path,
    root: &Path,
    strip_prefix: Option<&Path>,
) -> Result<(), RepositoryTransportError> {
    let strip_components = match strip_prefix {
        Some(path) if !path.as_os_str().is_empty() && normalized_relative(path) => {
            path.components().count().to_string()
        }
        Some(_) => {
            return Err(unsupported(
                "http_archive strip_prefix must be normalized and relative",
            ));
        }
        None => "0".to_owned(),
    };
    let listing = Command::new("tar")
        .args(["-tf"])
        .arg(archive)
        .output()
        .map_err(|error| RepositoryTransportError {
            message: format!("listing archive {}: {error}", archive.display()).into(),
        })?;
    if !listing.status.success()
        || String::from_utf8_lossy(&listing.stdout)
            .lines()
            .any(|line| !normalized_relative(Path::new(line.trim_end_matches('/'))))
    {
        return Err(unsupported("http_archive contains a non-normalized path"));
    }
    let detailed_listing = Command::new("tar")
        .args(["-tvf"])
        .arg(archive)
        .output()
        .map_err(|error| RepositoryTransportError {
            message: format!("inspecting archive {}: {error}", archive.display()).into(),
        })?;
    if !detailed_listing.status.success()
        || String::from_utf8_lossy(&detailed_listing.stdout)
            .lines()
            .any(|line| !matches!(line.as_bytes().first(), Some(b'-' | b'd')))
    {
        return Err(unsupported(
            "http_archive contains an unsupported tar entry type",
        ));
    }
    let output = Command::new("tar")
        .args(["-xf"])
        .arg(archive)
        .args(["-C"])
        .arg(root)
        .arg(format!("--strip-components={strip_components}"))
        .output()
        .map_err(|error| RepositoryTransportError {
            message: format!("extracting archive {}: {error}", archive.display()).into(),
        })?;
    if !output.status.success() {
        return Err(RepositoryTransportError {
            message: format!(
                "extracting archive {} failed: {}",
                archive.display(),
                String::from_utf8_lossy(&output.stderr).trim()
            )
            .into(),
        });
    }
    Ok(())
}

fn local_file_uri(value: &str) -> Result<PathBuf, RepositoryTransportError> {
    let url = url::Url::parse(value)
        .map_err(|_| unsupported("repository source must use an absolute file:// URI"))?;
    if url.scheme() != "file"
        || url.host_str().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(unsupported(
            "repository source must use an absolute file:// URI",
        ));
    }
    url.to_file_path()
        .map_err(|_| unsupported("repository source must use an absolute file:// URI"))
}

fn required_string<'a>(
    repo_spec: &'a RepoSpec,
    name: &str,
) -> Result<&'a str, RepositoryTransportError> {
    match repo_spec.attributes.get(name) {
        Some(OverrideAttributeValue::String(value)) => Ok(value),
        _ => Err(unsupported(&format!(
            "repository override requires string attribute {name}"
        ))),
    }
}

fn optional_string<'a>(
    repo_spec: &'a RepoSpec,
    name: &str,
) -> Result<Option<&'a str>, RepositoryTransportError> {
    match repo_spec.attributes.get(name) {
        None => Ok(None),
        Some(OverrideAttributeValue::String(value)) => Ok(Some(value)),
        Some(_) => Err(unsupported(&format!(
            "repository override attribute {name} must be a string"
        ))),
    }
}

fn reject_extra_attributes(
    repo_spec: &RepoSpec,
    allowed: &[&str],
) -> Result<(), RepositoryTransportError> {
    if repo_spec
        .attributes
        .keys()
        .any(|name| !allowed.contains(&name.as_str()))
    {
        return Err(unsupported(
            "repository override has unsupported attributes",
        ));
    }
    Ok(())
}

fn normalized_relative(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn unsupported(message: &str) -> RepositoryTransportError {
    RepositoryTransportError {
        message: message.into(),
    }
}

pub(crate) fn install(builder: &mut DiceDataBuilder) {
    install_repository_io(builder, Arc::new(LocalRepositoryIo::new()));
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use compact_str::CompactString;
    use sha2::Digest;
    use slug_bzlmod_v2::RepoRuleId;
    use slug_identity_v2::CanonicalLabel;
    use starlark_map::small_map::SmallMap;

    use super::*;

    fn archive_spec(url: String, sha256: String) -> RepoSpec {
        let attributes: [(CompactString, OverrideAttributeValue); 3] = [
            (
                "urls".into(),
                OverrideAttributeValue::Iterable(Arc::new([OverrideAttributeValue::String(
                    url.into(),
                )])),
            ),
            (
                "sha256".into(),
                OverrideAttributeValue::String(sha256.into()),
            ),
            ("type".into(), OverrideAttributeValue::String("tar".into())),
        ];
        RepoSpec {
            rule_id: RepoRuleId {
                bzl_file: CanonicalLabel::parse("@@bazel_tools//tools/build_defs/repo:http.bzl")
                    .unwrap(),
                rule_name: "http_archive".into(),
            },
            attributes: Arc::new(SmallMap::from_iter(attributes)),
        }
    }

    fn archive_spec_with_prefix(url: String, sha256: String, prefix: &str) -> RepoSpec {
        let mut spec = archive_spec(url, sha256);
        Arc::make_mut(&mut spec.attributes).insert(
            "strip_prefix".into(),
            OverrideAttributeValue::String(prefix.into()),
        );
        spec
    }

    #[derive(Clone, Copy)]
    struct TarEntry<'a> {
        name: &'a [u8],
        prefix: &'a [u8],
        typeflag: u8,
        data: &'a [u8],
    }

    fn ustar(entries: &[TarEntry<'_>], terminator: bool) -> Vec<u8> {
        let mut archive = Vec::new();
        for entry in entries {
            assert!(entry.name.len() <= 100);
            assert!(entry.prefix.len() <= 155);
            let mut header = [0u8; 512];
            header[..entry.name.len()].copy_from_slice(entry.name);
            write_octal(&mut header[100..108], 0o644);
            write_octal(&mut header[108..116], 0);
            write_octal(&mut header[116..124], 0);
            write_octal(&mut header[124..136], entry.data.len());
            write_octal(&mut header[136..148], 0);
            header[148..156].fill(b' ');
            header[156] = entry.typeflag;
            header[257..263].copy_from_slice(b"ustar\0");
            header[263..265].copy_from_slice(b"00");
            header[345..345 + entry.prefix.len()].copy_from_slice(entry.prefix);
            let checksum = header.iter().map(|byte| u32::from(*byte)).sum::<u32>();
            write_octal(&mut header[148..156], checksum as usize);
            archive.extend_from_slice(&header);
            archive.extend_from_slice(entry.data);
            let padding = (512 - entry.data.len() % 512) % 512;
            archive.resize(archive.len() + padding, 0);
        }
        if terminator {
            archive.resize(archive.len() + 1024, 0);
        }
        archive
    }

    fn write_octal(field: &mut [u8], value: usize) {
        let digits = format!("{value:0width$o}", width = field.len() - 1);
        assert!(digits.len() < field.len());
        field.fill(0);
        field[..digits.len()].copy_from_slice(digits.as_bytes());
    }

    #[derive(Default)]
    struct RecordingDestination {
        calls: Vec<(PlannedUstarKind, PathBuf, Vec<u8>)>,
        fail: bool,
    }

    impl ArchiveDestination for RecordingDestination {
        fn create_parent(&mut self, _path: &Path) -> std::io::Result<()> {
            if self.fail {
                return Err(std::io::Error::other("scripted parent failure"));
            }
            Ok(())
        }

        fn create_directory(&mut self, path: &Path) -> std::io::Result<()> {
            if self.fail {
                return Err(std::io::Error::other("scripted extraction failure"));
            }
            self.calls
                .push((PlannedUstarKind::Directory, path.to_owned(), Vec::new()));
            Ok(())
        }

        fn write_regular(&mut self, path: &Path, bytes: &[u8]) -> std::io::Result<()> {
            if self.fail {
                return Err(std::io::Error::other("scripted extraction failure"));
            }
            self.calls
                .push((PlannedUstarKind::Regular, path.to_owned(), bytes.to_vec()));
            Ok(())
        }
    }

    #[derive(Clone, Copy)]
    enum ScriptedFailure {
        None,
        Root,
        Capture,
        Read,
        Write,
        Flush,
    }

    struct ScriptedArchiveIo {
        source: Vec<u8>,
        failure: ScriptedFailure,
        reads: usize,
        root_path: Option<PathBuf>,
        capture_path: Option<PathBuf>,
        replace_source: Option<Vec<u8>>,
        delete_source: bool,
        destination_calls: usize,
        destination_failure: Option<&'static str>,
    }

    impl ScriptedArchiveIo {
        fn new(source: Vec<u8>, failure: ScriptedFailure) -> Self {
            Self {
                source,
                failure,
                reads: 0,
                root_path: None,
                capture_path: None,
                replace_source: None,
                delete_source: false,
                destination_calls: 0,
                destination_failure: None,
            }
        }
    }

    impl ArchiveIo for ScriptedArchiveIo {
        fn create_root(&mut self) -> std::io::Result<tempfile::TempDir> {
            if matches!(self.failure, ScriptedFailure::Root) {
                return Err(std::io::Error::other("scripted root failure"));
            }
            let root = tempfile::tempdir()?;
            self.root_path = Some(root.path().to_owned());
            Ok(root)
        }

        fn create_capture(&mut self) -> std::io::Result<tempfile::NamedTempFile> {
            if matches!(self.failure, ScriptedFailure::Capture) {
                return Err(std::io::Error::other("scripted capture failure"));
            }
            let capture = tempfile::NamedTempFile::new()?;
            self.capture_path = Some(capture.path().to_owned());
            Ok(capture)
        }

        fn read_source(&mut self, source: &Path) -> std::io::Result<Vec<u8>> {
            self.reads += 1;
            if matches!(self.failure, ScriptedFailure::Read) {
                return Err(std::io::Error::other("scripted read failure"));
            }
            if let Some(replacement) = self.replace_source.take() {
                std::fs::write(source, replacement)?;
            } else if self.delete_source {
                std::fs::remove_file(source)?;
            }
            Ok(self.source.clone())
        }

        fn write_capture(
            &mut self,
            capture: &mut tempfile::NamedTempFile,
            bytes: &[u8],
        ) -> std::io::Result<()> {
            if matches!(self.failure, ScriptedFailure::Write) {
                return Err(std::io::Error::other("scripted write failure"));
            }
            capture.write_all(bytes)
        }

        fn flush_capture(&mut self, capture: &mut tempfile::NamedTempFile) -> std::io::Result<()> {
            if matches!(self.failure, ScriptedFailure::Flush) {
                return Err(std::io::Error::other("scripted flush failure"));
            }
            capture.flush()
        }
    }

    impl ArchiveDestination for ScriptedArchiveIo {
        fn create_parent(&mut self, _path: &Path) -> std::io::Result<()> {
            self.destination_calls += 1;
            if self.destination_failure == Some("parent") {
                return Err(std::io::Error::other("scripted parent failure"));
            }
            Ok(())
        }

        fn create_directory(&mut self, path: &Path) -> std::io::Result<()> {
            self.destination_calls += 1;
            if self.destination_failure == Some("directory") {
                return Err(std::io::Error::other("scripted directory failure"));
            }
            std::fs::create_dir_all(path)
        }

        fn write_regular(&mut self, path: &Path, bytes: &[u8]) -> std::io::Result<()> {
            self.destination_calls += 1;
            if self.destination_failure == Some("write") {
                return Err(std::io::Error::other("scripted file failure"));
            }
            let mut output = File::create(path)?;
            output.write_all(bytes)
        }
    }

    fn git_spec(remote: String, commit: String) -> RepoSpec {
        let attributes: [(CompactString, OverrideAttributeValue); 2] = [
            (
                "remote".into(),
                OverrideAttributeValue::String(remote.into()),
            ),
            (
                "commit".into(),
                OverrideAttributeValue::String(commit.into()),
            ),
        ];
        RepoSpec {
            rule_id: RepoRuleId {
                bzl_file: CanonicalLabel::parse("@@bazel_tools//tools/build_defs/repo:git.bzl")
                    .unwrap(),
                rule_name: "git_repository".into(),
            },
            attributes: Arc::new(SmallMap::from_iter(attributes)),
        }
    }

    fn local_spec(path: &str) -> RepoSpec {
        let attributes: [(CompactString, OverrideAttributeValue); 1] =
            [("path".into(), OverrideAttributeValue::String(path.into()))];
        RepoSpec {
            rule_id: RepoRuleId {
                bzl_file: CanonicalLabel::parse("@@bazel_tools//tools/build_defs/repo:local.bzl")
                    .unwrap(),
                rule_name: "local_repository".into(),
            },
            attributes: Arc::new(SmallMap::from_iter(attributes)),
        }
    }

    fn native_request(
        workspace: &NormalizedAbsolutePath,
        repo: &str,
        repo_spec: RepoSpec,
        kind: RepositoryMaterializationKind,
    ) -> Arc<RepositoryMaterializationRequest> {
        Arc::new(RepositoryMaterializationRequest {
            id: RepositoryMaterializationRequestId {
                workspace: workspace.clone(),
                canonical_repo: CanonicalRepoName::new(repo).unwrap(),
            },
            repo_spec,
            kind,
        })
    }

    fn immutable_archive_fixture() -> (tempfile::TempDir, RepoSpec) {
        let source = tempfile::tempdir().unwrap();
        let content = source.path().join("content");
        std::fs::create_dir(&content).unwrap();
        std::fs::write(content.join("MODULE.bazel"), b"module(name = 'archive')").unwrap();
        let archive = source.path().join("source.tar");
        assert!(
            Command::new("tar")
                .args(["--format=ustar", "-cf"])
                .arg(&archive)
                .args(["-C"])
                .arg(source.path())
                .arg("content")
                .status()
                .unwrap()
                .success()
        );
        let bytes = std::fs::read(&archive).unwrap();
        let spec = archive_spec(
            url::Url::from_file_path(&archive).unwrap().to_string(),
            format!("{:x}", Sha256::digest(&bytes)),
        );
        (source, spec)
    }

    #[test]
    fn archive_path_flavors_and_latin1_prefix_are_exact() {
        assert_eq!(latin1_bytes("\u{ff}root"), b"\xffroot");
        assert_eq!(latin1_bytes("\u{100}root"), b"?root");
        assert_eq!(
            normalize_raw_tar_path(br"root\f", PathFlavor::Unix).unwrap(),
            vec![br"root\f".to_vec()]
        );
        for (input, expected) in [
            (&b"C:/root/f"[..], vec![b"root".to_vec(), b"f".to_vec()]),
            (&br"C:\root\f"[..], vec![b"root".to_vec(), b"f".to_vec()]),
            (
                &br"\\server\share\f"[..],
                vec![b"server".to_vec(), b"share".to_vec(), b"f".to_vec()],
            ),
            (&br"root\f"[..], vec![b"root".to_vec(), b"f".to_vec()]),
            (&b"C:foo"[..], vec![b"C:foo".to_vec()]),
            (&br"a\..\b"[..], vec![b"b".to_vec()]),
        ] {
            assert_eq!(
                normalize_raw_tar_path(input, PathFlavor::Windows).unwrap(),
                expected
            );
        }
        assert!(
            join_raw_components(Path::new("root"), &[b"C:foo".to_vec()], PathFlavor::Windows)
                .is_err()
        );
        assert!(
            join_raw_components(
                Path::new("root"),
                &[b"..".to_vec(), b"escape".to_vec()],
                PathFlavor::Unix
            )
            .is_err()
        );
        for (prefix, flavor) in [
            (Vec::<Vec<u8>>::new(), PathFlavor::Unix),
            (vec![b"..".to_vec()], PathFlavor::Unix),
            (vec![b"C:foo".to_vec()], PathFlavor::Windows),
        ] {
            let error = validate_strip_prefix(&prefix, flavor).err().unwrap();
            assert_eq!(error.stage, ArchiveFailureStage::Spec);
        }
        assert!(validate_strip_prefix(&[b"safe".to_vec()], PathFlavor::Unix).is_ok());
    }

    #[test]
    fn archive_short_headers_and_declared_bounds_match_commons() {
        for length in [0, 1, 511] {
            let short = vec![b'x'; length];
            assert!(
                inspect_and_plan_ustar_for_flavor(&short, None, PathFlavor::Unix, Path::new(""))
                    .unwrap()
                    .entries
                    .is_empty()
            );
            assert!(
                inspect_and_plan_ustar_for_flavor(
                    &short,
                    Some(&[b"missing".to_vec()]),
                    PathFlavor::Unix,
                    Path::new("")
                )
                .is_err()
            );
        }
        let entry = TarEntry {
            name: b"file",
            prefix: b"",
            typeflag: b'0',
            data: b"x",
        };
        let complete = ustar(&[entry], false);
        assert_eq!(
            inspect_and_plan_ustar_for_flavor(&complete, None, PathFlavor::Unix, Path::new(""))
                .unwrap()
                .entries
                .len(),
            1
        );
        for length in [1, 511] {
            let mut trailing = complete.clone();
            trailing.extend(std::iter::repeat_n(b'x', length));
            assert_eq!(
                inspect_and_plan_ustar_for_flavor(&trailing, None, PathFlavor::Unix, Path::new(""))
                    .unwrap()
                    .entries
                    .len(),
                1
            );
        }
        let mut truncated_payload = ustar(
            &[TarEntry {
                name: b"file",
                prefix: b"",
                typeflag: b'0',
                data: &[1; 513],
            }],
            false,
        );
        truncated_payload.truncate(512 + 512);
        assert!(
            inspect_and_plan_ustar_for_flavor(
                &truncated_payload,
                None,
                PathFlavor::Unix,
                Path::new("")
            )
            .is_err()
        );
        let mut truncated_padding = complete.clone();
        truncated_padding.truncate(513);
        assert!(
            inspect_and_plan_ustar_for_flavor(
                &truncated_padding,
                None,
                PathFlavor::Unix,
                Path::new("")
            )
            .is_err()
        );
    }

    #[test]
    fn archive_numeric_and_header_format_boundaries_are_explicit() {
        assert_eq!(parse_ustar_octal(b"\x001          ").unwrap(), 0);
        assert_eq!(parse_ustar_octal(b"       17\0 ").unwrap(), 15);
        assert!(parse_ustar_octal(b"000000008\0  ").is_err());
        assert!(parse_ustar_octal(&[b'7'; 30]).is_err());
        let mut binary = [0u8; 12];
        binary[0] = 0x80;
        assert!(parse_ustar_octal(&binary).is_err());

        let mut leading_nul = ustar(
            &[TarEntry {
                name: b"empty",
                prefix: b"",
                typeflag: b'0',
                data: b"",
            }],
            false,
        );
        leading_nul[124..136].copy_from_slice(b"\x001          ");
        let plan =
            inspect_and_plan_ustar_for_flavor(&leading_nul, None, PathFlavor::Unix, Path::new(""))
                .unwrap();
        assert!(plan.entries[0].payload.is_empty());

        for selected in [true, false] {
            let mut binary_archive = ustar(
                &[TarEntry {
                    name: b"file",
                    prefix: if selected {
                        b"wanted".as_slice()
                    } else {
                        b"other".as_slice()
                    },
                    typeflag: b'0',
                    data: b"",
                }],
                false,
            );
            binary_archive[124] = 0x80;
            assert!(
                inspect_and_plan_ustar_for_flavor(
                    &binary_archive,
                    Some(&[b"wanted".to_vec()]),
                    PathFlavor::Unix,
                    Path::new("")
                )
                .is_err()
            );
        }

        let entry = TarEntry {
            name: b"file",
            prefix: b"prefix",
            typeflag: b'0',
            data: b"x",
        };
        let mut legacy = ustar(&[entry], false);
        legacy[257..265].fill(0);
        let plan = inspect_and_plan_ustar_for_flavor(
            &legacy,
            Some(&[b"prefix".to_vec()]),
            PathFlavor::Unix,
            Path::new(""),
        )
        .unwrap();
        assert_eq!(plan.entries[0].components, vec![b"file".to_vec()]);

        let mut odd_version = ustar(&[entry], false);
        odd_version[263..265].copy_from_slice(b"!?");
        assert!(
            inspect_and_plan_ustar_for_flavor(&odd_version, None, PathFlavor::Unix, Path::new(""))
                .is_ok()
        );
        let mut gnu = ustar(&[entry], false);
        gnu[257..263].copy_from_slice(b"ustar ");
        assert!(
            inspect_and_plan_ustar_for_flavor(&gnu, None, PathFlavor::Unix, Path::new("")).is_err()
        );
        let mut xstar = ustar(&[entry], false);
        xstar[508..512].copy_from_slice(b"tar\0");
        assert!(
            inspect_and_plan_ustar_for_flavor(&xstar, None, PathFlavor::Unix, Path::new(""))
                .is_err()
        );
        let long_prefix = [b'p'; 140];
        let mut discriminating_xstar = ustar(
            &[TarEntry {
                name: b"file",
                prefix: &long_prefix,
                typeflag: b'0',
                data: b"x",
            }],
            false,
        );
        discriminating_xstar[508..512].copy_from_slice(b"tar\0");
        assert!(
            inspect_and_plan_ustar_for_flavor(
                &discriminating_xstar,
                Some(&[long_prefix.to_vec()]),
                PathFlavor::Unix,
                Path::new("")
            )
            .is_err()
        );
        let mut xustar = ustar(&[entry], false);
        xustar[476..488].copy_from_slice(b"00000000000 ");
        xustar[488..500].copy_from_slice(b"00000000000 ");
        assert!(
            inspect_and_plan_ustar_for_flavor(&xustar, None, PathFlavor::Unix, Path::new(""))
                .is_err()
        );
        let mut checksum_corrupt = ustar(&[entry], false);
        checksum_corrupt[148..156].fill(b'7');
        assert!(
            inspect_and_plan_ustar_for_flavor(
                &checksum_corrupt,
                None,
                PathFlavor::Unix,
                Path::new("")
            )
            .is_ok()
        );
        let mut checksum_leading_nul = ustar(&[entry], false);
        checksum_leading_nul[148..156].copy_from_slice(b"\0xxxxxxx");
        assert!(
            inspect_and_plan_ustar_for_flavor(
                &checksum_leading_nul,
                None,
                PathFlavor::Unix,
                Path::new("")
            )
            .is_ok()
        );
        let mut checksum_invalid = ustar(&[entry], false);
        checksum_invalid[148..156].copy_from_slice(b"000x000\0");
        assert!(
            inspect_and_plan_ustar_for_flavor(
                &checksum_invalid,
                None,
                PathFlavor::Unix,
                Path::new("")
            )
            .is_err()
        );
    }

    #[test]
    fn archive_raw_prefix_normalization_and_type_rows_are_discriminating() {
        let archive = ustar(
            &[
                TarEntry {
                    name: b"raw-\xff",
                    prefix: b"\xffroot",
                    typeflag: b'0',
                    data: b"raw",
                },
                TarEntry {
                    name: b"./dir/",
                    prefix: b"\xffroot",
                    typeflag: b'0',
                    data: b"",
                },
                TarEntry {
                    name: b"typed",
                    prefix: b"\xffroot",
                    typeflag: b'5',
                    data: b"",
                },
                TarEntry {
                    name: b"/absolute/../normalized",
                    prefix: b"\xffroot",
                    typeflag: 0,
                    data: b"normalized",
                },
            ],
            false,
        );
        let plan = inspect_and_plan_ustar_for_flavor(
            &archive,
            Some(&[b"\xffroot".to_vec()]),
            PathFlavor::Unix,
            Path::new(""),
        )
        .unwrap();
        assert_eq!(plan.entries.len(), 4);
        assert_eq!(plan.entries[0].components[0], b"raw-\xff");
        assert_eq!(plan.entries[1].kind, PlannedUstarKind::Directory);
        assert_eq!(plan.entries[2].kind, PlannedUstarKind::Directory);
        assert_eq!(plan.entries[3].components, vec![b"normalized".to_vec()]);

        let source = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(source.path(), &archive).unwrap();
        let url = url::Url::from_file_path(source.path()).unwrap().to_string();
        let digest = format!("{:x}", Sha256::digest(&archive));
        let mut io = ScriptedArchiveIo::new(archive, ScriptedFailure::None);
        let Materialized::Immutable { root, .. } = materialize_archive_with(
            &archive_spec_with_prefix(url, digest, "\u{ff}root"),
            &mut io,
        )
        .unwrap() else {
            panic!("archive must be immutable");
        };
        assert_eq!(
            std::fs::read(root.path().join("normalized")).unwrap(),
            b"normalized"
        );

        let question_archive = ustar(
            &[TarEntry {
                name: b"file",
                prefix: b"?root",
                typeflag: b'0',
                data: b"question",
            }],
            false,
        );
        let question_source = tempfile::NamedTempFile::new().unwrap();
        let question_url = url::Url::from_file_path(question_source.path())
            .unwrap()
            .to_string();
        let question_digest = format!("{:x}", Sha256::digest(&question_archive));
        let mut io = ScriptedArchiveIo::new(question_archive, ScriptedFailure::None);
        assert!(
            materialize_archive_with(
                &archive_spec_with_prefix(question_url, question_digest, "\u{100}root"),
                &mut io
            )
            .is_ok()
        );
    }

    #[test]
    fn archive_selection_types_and_planning_are_atomic() {
        let entries = [
            TarEntry {
                name: b"root/",
                prefix: b"",
                typeflag: b'0',
                data: b"",
            },
            TarEntry {
                name: b"file",
                prefix: b"root",
                typeflag: 0,
                data: b"first",
            },
            TarEntry {
                name: b"file",
                prefix: b"root",
                typeflag: b'0',
                data: b"last",
            },
            TarEntry {
                name: b"directory",
                prefix: b"root",
                typeflag: b'5',
                data: b"",
            },
            TarEntry {
                name: b"ignored",
                prefix: b"other",
                typeflag: b'3',
                data: b"",
            },
        ];
        let archive = ustar(&entries, true);
        let plan = inspect_and_plan_ustar_for_flavor(
            &archive,
            Some(&[b"root".to_vec()]),
            PathFlavor::Unix,
            Path::new(""),
        )
        .unwrap();
        assert_eq!(plan.entries.len(), 3);
        let mut destination = RecordingDestination::default();
        extract_ustar_plan(&archive, &plan, &mut destination).unwrap();
        assert_eq!(destination.calls[0].2, b"first");
        assert_eq!(destination.calls[1].2, b"last");

        let mut late_failure = ustar(&entries[..2], false);
        late_failure.extend(ustar(
            &[TarEntry {
                name: b"bad",
                prefix: b"root",
                typeflag: b'3',
                data: b"",
            }],
            false,
        ));
        let destination = RecordingDestination::default();
        assert!(
            inspect_and_plan_ustar_for_flavor(
                &late_failure,
                Some(&[b"root".to_vec()]),
                PathFlavor::Unix,
                Path::new("")
            )
            .is_err()
        );
        assert!(destination.calls.is_empty());

        let collision = ustar(
            &[
                TarEntry {
                    name: b"same",
                    prefix: b"",
                    typeflag: b'5',
                    data: b"",
                },
                TarEntry {
                    name: b"same",
                    prefix: b"",
                    typeflag: b'0',
                    data: b"x",
                },
            ],
            false,
        );
        assert!(
            inspect_and_plan_ustar_for_flavor(&collision, None, PathFlavor::Unix, Path::new(""))
                .is_err()
        );
        let reverse_collision = ustar(
            &[
                TarEntry {
                    name: b"same",
                    prefix: b"",
                    typeflag: b'0',
                    data: b"x",
                },
                TarEntry {
                    name: b"same",
                    prefix: b"",
                    typeflag: b'5',
                    data: b"",
                },
            ],
            false,
        );
        assert!(
            inspect_and_plan_ustar_for_flavor(
                &reverse_collision,
                None,
                PathFlavor::Unix,
                Path::new("")
            )
            .is_err()
        );
        for entries in [
            [
                TarEntry {
                    name: b"a",
                    prefix: b"",
                    typeflag: b'0',
                    data: b"x",
                },
                TarEntry {
                    name: b"a/b",
                    prefix: b"",
                    typeflag: b'0',
                    data: b"y",
                },
            ],
            [
                TarEntry {
                    name: b"a/b",
                    prefix: b"",
                    typeflag: b'0',
                    data: b"y",
                },
                TarEntry {
                    name: b"a",
                    prefix: b"",
                    typeflag: b'0',
                    data: b"x",
                },
            ],
        ] {
            let ancestor_collision = ustar(&entries, false);
            assert!(
                inspect_and_plan_ustar_for_flavor(
                    &ancestor_collision,
                    None,
                    PathFlavor::Unix,
                    Path::new("")
                )
                .is_err()
            );
        }
        for entries in [
            [
                TarEntry {
                    name: b"a",
                    prefix: b"",
                    typeflag: b'5',
                    data: b"",
                },
                TarEntry {
                    name: b"a",
                    prefix: b"",
                    typeflag: b'5',
                    data: b"",
                },
            ],
            [
                TarEntry {
                    name: b"a",
                    prefix: b"",
                    typeflag: b'5',
                    data: b"",
                },
                TarEntry {
                    name: b"a/b",
                    prefix: b"",
                    typeflag: b'0',
                    data: b"x",
                },
            ],
            [
                TarEntry {
                    name: b"a/b",
                    prefix: b"",
                    typeflag: b'0',
                    data: b"x",
                },
                TarEntry {
                    name: b"a",
                    prefix: b"",
                    typeflag: b'5',
                    data: b"",
                },
            ],
        ] {
            let compatible = ustar(&entries, false);
            assert!(
                inspect_and_plan_ustar_for_flavor(
                    &compatible,
                    None,
                    PathFlavor::Unix,
                    Path::new("")
                )
                .is_ok()
            );
        }

        for (typeflag, name, expected) in [
            (0, b"nul".as_slice(), PlannedUstarKind::Regular),
            (b'0', b"zero".as_slice(), PlannedUstarKind::Regular),
            (b'5', b"typed".as_slice(), PlannedUstarKind::Directory),
            (b'5', b"typed/".as_slice(), PlannedUstarKind::Directory),
            (b'0', b"implicit/".as_slice(), PlannedUstarKind::Directory),
        ] {
            let typed = ustar(
                &[TarEntry {
                    name,
                    prefix: b"",
                    typeflag,
                    data: b"",
                }],
                false,
            );
            let typed =
                inspect_and_plan_ustar_for_flavor(&typed, None, PathFlavor::Unix, Path::new(""))
                    .unwrap();
            assert_eq!(typed.entries[0].kind, expected);
        }
        for typeflag in [b'x', b'g', b'L', b'K', b'1', b'2', b'3', b'4', b'6'] {
            for name in [b"bad".as_slice(), b"bad/".as_slice()] {
                let selected = ustar(
                    &[TarEntry {
                        name,
                        prefix: b"root",
                        typeflag,
                        data: b"",
                    }],
                    false,
                );
                let error = inspect_and_plan_ustar_for_flavor(
                    &selected,
                    Some(&[b"root".to_vec()]),
                    PathFlavor::Unix,
                    Path::new(""),
                )
                .err()
                .unwrap();
                assert!(error.message.contains("unsupported tar entry type"));
                let outside = ustar(
                    &[TarEntry {
                        name,
                        prefix: b"rooted",
                        typeflag,
                        data: b"",
                    }],
                    false,
                );
                let error = inspect_and_plan_ustar_for_flavor(
                    &outside,
                    Some(&[b"wanted".to_vec()]),
                    PathFlavor::Unix,
                    Path::new(""),
                )
                .err()
                .unwrap();
                assert!(error.message.contains("strip_prefix"));
            }
            let prefix_root = ustar(
                &[TarEntry {
                    name: b"root",
                    prefix: b"",
                    typeflag,
                    data: b"",
                }],
                false,
            );
            let error = inspect_and_plan_ustar_for_flavor(
                &prefix_root,
                Some(&[b"root".to_vec()]),
                PathFlavor::Unix,
                Path::new(""),
            )
            .err()
            .unwrap();
            assert!(error.message.contains("unsupported tar entry type"));
        }

        let escaping = TarEntry {
            name: b"../escape",
            prefix: b"",
            typeflag: b'0',
            data: b"x",
        };
        let outside_escape = ustar(
            &[
                escaping,
                TarEntry {
                    name: b"good",
                    prefix: b"wanted",
                    typeflag: b'0',
                    data: b"good",
                },
            ],
            false,
        );
        assert_eq!(
            inspect_and_plan_ustar_for_flavor(
                &outside_escape,
                Some(&[b"wanted".to_vec()]),
                PathFlavor::Unix,
                Path::new("")
            )
            .unwrap()
            .entries
            .len(),
            1
        );
        let selected_escape = ustar(&[escaping], false);
        assert!(
            inspect_and_plan_ustar_for_flavor(
                &selected_escape,
                None,
                PathFlavor::Unix,
                Path::new("")
            )
            .is_err()
        );

        let mut failed_destination = RecordingDestination {
            fail: true,
            ..RecordingDestination::default()
        };
        let error = extract_ustar_plan(&archive, &plan, &mut failed_destination)
            .err()
            .unwrap();
        assert_eq!(error.stage, ArchiveFailureStage::Materialization);
    }

    #[test]
    fn archive_capture_stage_precedence_and_mutation_barrier_are_exact() {
        let archive = ustar(
            &[TarEntry {
                name: b"file",
                prefix: b"",
                typeflag: b'0',
                data: b"captured",
            }],
            false,
        );
        let source = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(source.path(), b"caller").unwrap();
        let url = url::Url::from_file_path(source.path()).unwrap().to_string();
        for (failure, stage, reads) in [
            (
                ScriptedFailure::Root,
                ArchiveFailureStage::Materialization,
                0,
            ),
            (
                ScriptedFailure::Capture,
                ArchiveFailureStage::Materialization,
                0,
            ),
            (ScriptedFailure::Read, ArchiveFailureStage::Transport, 1),
            (ScriptedFailure::Write, ArchiveFailureStage::Transport, 1),
            (ScriptedFailure::Flush, ArchiveFailureStage::Transport, 1),
        ] {
            let mut io = ScriptedArchiveIo::new(archive.clone(), failure);
            let error = materialize_archive_with(&archive_spec(url.clone(), "bad".into()), &mut io)
                .err()
                .unwrap();
            assert_eq!(error.stage, stage);
            assert_eq!(io.reads, reads);
            if let Some(path) = io.root_path {
                assert!(!path.exists());
            }
            if let Some(path) = io.capture_path {
                assert!(!path.exists());
            }
        }
        let mut io = ScriptedArchiveIo::new(archive.clone(), ScriptedFailure::None);
        let error = materialize_archive_with(&archive_spec(url.clone(), "bad".into()), &mut io)
            .err()
            .unwrap();
        assert_eq!(error.stage, ArchiveFailureStage::Spec);
        assert_eq!(io.reads, 1);
        assert!(!io.root_path.unwrap().exists());
        assert!(!io.capture_path.unwrap().exists());

        let mut io = ScriptedArchiveIo::new(archive.clone(), ScriptedFailure::Root);
        let error =
            materialize_archive_with(&archive_spec("not a URL".into(), "bad".into()), &mut io)
                .err()
                .unwrap();
        assert_eq!(error.stage, ArchiveFailureStage::Spec);
        assert!(io.root_path.is_none());
        assert_eq!(io.reads, 0);
        for prefix in ["", ".."] {
            let mut io = ScriptedArchiveIo::new(archive.clone(), ScriptedFailure::Root);
            let error = materialize_archive_with(
                &archive_spec_with_prefix(url.clone(), "bad".into(), prefix),
                &mut io,
            )
            .err()
            .unwrap();
            assert_eq!(error.stage, ArchiveFailureStage::Spec);
            assert!(io.root_path.is_none());
            assert_eq!(io.reads, 0);
        }

        let mut io = ScriptedArchiveIo::new(archive.clone(), ScriptedFailure::None);
        let error = materialize_archive_with(&archive_spec(url.clone(), "0".repeat(64)), &mut io)
            .err()
            .unwrap();
        assert_eq!(error.stage, ArchiveFailureStage::Transport);

        let malformed = vec![b'x'; 512];
        let malformed_digest = format!("{:x}", Sha256::digest(&malformed));
        let mut io = ScriptedArchiveIo::new(malformed, ScriptedFailure::None);
        let error = materialize_archive_with(&archive_spec(url.clone(), malformed_digest), &mut io)
            .err()
            .unwrap();
        assert_eq!(error.stage, ArchiveFailureStage::Materialization);
        assert!(!io.root_path.unwrap().exists());
        assert!(!io.capture_path.unwrap().exists());

        let mut io = ScriptedArchiveIo::new(vec![b'x'; 512], ScriptedFailure::None);
        let error = materialize_archive_with(&archive_spec(url.clone(), "0".repeat(64)), &mut io)
            .err()
            .unwrap();
        assert_eq!(error.stage, ArchiveFailureStage::Transport);
        assert_eq!(io.destination_calls, 0);

        let late_failure = ustar(
            &[
                TarEntry {
                    name: b"early",
                    prefix: b"",
                    typeflag: b'0',
                    data: b"early",
                },
                TarEntry {
                    name: b"bad",
                    prefix: b"",
                    typeflag: b'3',
                    data: b"",
                },
            ],
            false,
        );
        let late_digest = format!("{:x}", Sha256::digest(&late_failure));
        let mut io = ScriptedArchiveIo::new(late_failure, ScriptedFailure::None);
        let error = materialize_archive_with(&archive_spec(url.clone(), late_digest), &mut io)
            .err()
            .unwrap();
        assert_eq!(error.stage, ArchiveFailureStage::Materialization);
        assert_eq!(io.destination_calls, 0);
        assert!(!io.root_path.unwrap().exists());
        assert!(!io.capture_path.unwrap().exists());

        let extraction_archive = ustar(
            &[
                TarEntry {
                    name: b"file",
                    prefix: b"",
                    typeflag: b'0',
                    data: b"file",
                },
                TarEntry {
                    name: b"directory",
                    prefix: b"",
                    typeflag: b'5',
                    data: b"",
                },
            ],
            false,
        );
        let extraction_digest = format!("{:x}", Sha256::digest(&extraction_archive));
        for failure in ["parent", "write", "directory"] {
            let mut io = ScriptedArchiveIo::new(extraction_archive.clone(), ScriptedFailure::None);
            io.destination_failure = Some(failure);
            let error = materialize_archive_with(
                &archive_spec(url.clone(), extraction_digest.clone()),
                &mut io,
            )
            .err()
            .unwrap();
            assert_eq!(error.stage, ArchiveFailureStage::Materialization);
            assert!(io.destination_calls > 0);
            assert!(!io.root_path.unwrap().exists());
            assert!(!io.capture_path.unwrap().exists());
        }

        let digest = format!("{:x}", Sha256::digest(&archive));
        let mut io = ScriptedArchiveIo::new(archive, ScriptedFailure::None);
        io.replace_source = Some(b"changed after capture".to_vec());
        let Materialized::Immutable { bytes, root } =
            materialize_archive_with(&archive_spec(url, digest), &mut io).unwrap()
        else {
            panic!("archive must be immutable");
        };
        assert_eq!(io.reads, 1);
        assert_eq!(bytes.len(), 1024);
        assert_eq!(
            std::fs::read(root.path().join("file")).unwrap(),
            b"captured"
        );

        let source = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(source.path(), b"caller").unwrap();
        let url = url::Url::from_file_path(source.path()).unwrap().to_string();
        let archive = ustar(
            &[TarEntry {
                name: b"file",
                prefix: b"",
                typeflag: b'0',
                data: b"deleted source",
            }],
            false,
        );
        let digest = format!("{:x}", Sha256::digest(&archive));
        let mut io = ScriptedArchiveIo::new(archive, ScriptedFailure::None);
        io.delete_source = true;
        let Materialized::Immutable { root, .. } =
            materialize_archive_with(&archive_spec(url, digest), &mut io).unwrap()
        else {
            panic!("archive must be immutable");
        };
        assert!(!source.path().exists());
        assert_eq!(
            std::fs::read(root.path().join("file")).unwrap(),
            b"deleted source"
        );
    }

    fn materialization_request(
        workspace: &NormalizedAbsolutePath,
        repo: &str,
        salt: &str,
        kind: RepositoryMaterializationKind,
    ) -> Arc<RepositoryMaterializationRequest> {
        Arc::new(RepositoryMaterializationRequest {
            id: RepositoryMaterializationRequestId {
                workspace: workspace.clone(),
                canonical_repo: CanonicalRepoName::new(repo).unwrap(),
            },
            repo_spec: git_spec(format!("file:///{salt}"), salt.into()),
            kind,
        })
    }

    fn begin_empty(materializer: &RepositoryMaterializer) -> RepositorySessionToken {
        begin_validated(materializer, |_, _| {
            panic!("empty accepted state must not observe")
        })
    }

    fn begin_validated(
        materializer: &RepositoryMaterializer,
        mut observe: impl FnMut(&PathObservationDemand, Option<&Path>) -> PathObservationResult,
    ) -> RepositorySessionToken {
        let token = materializer.begin().unwrap();
        materializer.validate(token, &mut observe).unwrap();
        token
    }

    fn immutable_result(
        materializer: &RepositoryMaterializer,
        repo: &str,
    ) -> (PathBuf, PathObservationInstanceId) {
        let state = materializer
            .state
            .lock()
            .expect("repository materializer mutex poisoned");
        let entry = state
            .active
            .as_ref()
            .unwrap()
            .entries
            .iter()
            .find(|entry| entry.request.id.canonical_repo.as_str() == repo)
            .unwrap();
        let RepositoryMaterializationResult::Success(RepositoryMaterializationSuccess::Immutable {
            generation_root,
            observation_instance,
            ..
        }) = &entry.result
        else {
            panic!("expected immutable success");
        };
        (generation_root.clone(), *observation_instance)
    }

    fn active_result(
        materializer: &RepositoryMaterializer,
        repo: &str,
    ) -> RepositoryMaterializationResult {
        materializer
            .state
            .lock()
            .unwrap()
            .active
            .as_ref()
            .unwrap()
            .entries
            .iter()
            .find(|entry| entry.request.id.canonical_repo.as_str() == repo)
            .unwrap()
            .result
            .clone()
    }

    #[test]
    fn retained_native_preflight_batches_deduplicated_demands_and_returns_complete_epochs() {
        let workspace_root = tempfile::tempdir().unwrap();
        let workspace = NormalizedAbsolutePath::new(workspace_root.path().to_path_buf()).unwrap();
        let materializer = RepositoryMaterializer::new(workspace.clone());
        let shared_path = workspace_root.path().join("shared");
        let path_a = workspace_root.path().join("a");
        let path_b = workspace_root.path().join("b");
        std::fs::write(&shared_path, b"shared").unwrap();
        std::fs::write(&path_a, b"a").unwrap();
        std::fs::write(&path_b, b"b").unwrap();
        let demand = |path: &Path| {
            PathObservationDemand::new(
                PathObservationNamespace::Host,
                NormalizedAbsolutePath::new(path.to_path_buf()).unwrap(),
                PathObservationOperation::FileBytes,
            )
        };
        let shared = demand(&shared_path);
        let only_a = demand(&path_a);
        let only_b = demand(&path_b);
        let request_a = native_request(
            &workspace,
            "a",
            local_spec("missing-a"),
            RepositoryMaterializationKind::Local {
                logical_root: NormalizedAbsolutePath::new(workspace_root.path().join("missing-a"))
                    .unwrap(),
            },
        );
        let request_b = native_request(
            &workspace,
            "b",
            local_spec("missing-b"),
            RepositoryMaterializationKind::Local {
                logical_root: NormalizedAbsolutePath::new(workspace_root.path().join("missing-b"))
                    .unwrap(),
            },
        );
        let token = begin_empty(&materializer);
        for request in [&request_a, &request_b] {
            materializer
                .materialize_native(
                    token,
                    request.clone(),
                    RepositoryMaterializationGeneration(1),
                )
                .unwrap();
        }
        let observed = materializer
            .observe_native(token, [shared.clone(), only_a.clone(), only_b.clone()])
            .unwrap();
        materializer
            .accept(
                token,
                &[request_a.clone(), request_b.clone()],
                vec![
                    RepositoryValidation::new(
                        request_a.clone(),
                        vec![
                            (shared.clone(), epoch_result(&observed, &shared)),
                            (only_a.clone(), epoch_result(&observed, &only_a)),
                        ],
                    ),
                    RepositoryValidation::new(
                        request_b.clone(),
                        vec![
                            (shared.clone(), epoch_result(&observed, &shared)),
                            (only_b.clone(), epoch_result(&observed, &only_b)),
                        ],
                    ),
                ],
            )
            .unwrap();

        std::fs::write(&path_b, b"changed").unwrap();
        let token = materializer.begin().unwrap();
        let mut calls = 0;
        let preflight = materializer
            .preflight_with(
                token,
                [shared.clone(), shared.clone()],
                |root_owners, roots, demands| {
                    calls += 1;
                    assert_eq!(demands, [only_a.clone(), only_b.clone(), shared.clone()]);
                    super::super::path_observation::observe_native(&root_owners, roots, demands)
                },
            )
            .unwrap();
        assert_eq!(calls, 1);
        assert_eq!(preflight.path_observations().observations().len(), 3);
        assert_eq!(
            epoch_result(preflight.path_observations(), &only_b),
            PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(
                &b"changed"[..],
            )))
        );
        let expected = RepositoryMaterializationResultEpoch::new(
            workspace.clone(),
            [RepositoryMaterializationEpochEntry {
                request: request_a,
                result: RepositoryMaterializationResult::Success(
                    RepositoryMaterializationSuccess::Local,
                ),
            }],
        )
        .unwrap();
        assert_eq!(preflight.repository_results(), &expected);
        assert_eq!(materializer.epoch(token).unwrap(), expected);
        materializer.discard(token).unwrap();

        let empty = RepositoryMaterializer::new(workspace.clone());
        let token = empty.begin().unwrap();
        let mut empty_calls = 0;
        let preflight = empty
            .preflight_with(token, [], |root_owners, roots, demands| {
                empty_calls += 1;
                assert!(root_owners.is_empty());
                assert!(roots.is_empty());
                assert!(demands.is_empty());
                Ok(PathObservationEpoch::empty())
            })
            .unwrap();
        assert_eq!(empty_calls, 1);
        assert_eq!(
            preflight.path_observations(),
            &PathObservationEpoch::empty()
        );
        assert_eq!(
            preflight.repository_results(),
            &RepositoryMaterializationResultEpoch::new(workspace, []).unwrap()
        );
        empty.discard(token).unwrap();
    }

    #[test]
    fn retained_native_materialization_classifies_stable_spec_and_generation_scoped_http_failures()
    {
        let workspace_root = tempfile::tempdir().unwrap();
        let workspace = NormalizedAbsolutePath::new(workspace_root.path().to_path_buf()).unwrap();
        let good_bytes = ustar(
            &[TarEntry {
                name: b"file",
                prefix: b"",
                typeflag: b'0',
                data: b"good",
            }],
            false,
        );
        let good_path = workspace_root.path().join("good.tar");
        std::fs::write(&good_path, &good_bytes).unwrap();
        let malformed_bytes = vec![b'x'; 512];
        let malformed_path = workspace_root.path().join("malformed.tar");
        std::fs::write(&malformed_path, &malformed_bytes).unwrap();
        let url = |path: &Path| url::Url::from_file_path(path).unwrap().to_string();
        let request_spec = native_request(
            &workspace,
            "spec",
            archive_spec(url(&good_path), "bad".into()),
            RepositoryMaterializationKind::Immutable,
        );
        let request_transport = native_request(
            &workspace,
            "transport",
            archive_spec(url(&good_path), "0".repeat(64)),
            RepositoryMaterializationKind::Immutable,
        );
        let request_materialization = native_request(
            &workspace,
            "materialization",
            archive_spec(
                url(&malformed_path),
                format!("{:x}", Sha256::digest(&malformed_bytes)),
            ),
            RepositoryMaterializationKind::Immutable,
        );
        let materializer = RepositoryMaterializer::new(workspace.clone());
        let mut first_spec = None;
        for generation in [7, 8] {
            let token = begin_empty(&materializer);
            for request in [&request_spec, &request_transport, &request_materialization] {
                materializer
                    .materialize_native(
                        token,
                        request.clone(),
                        RepositoryMaterializationGeneration(generation),
                    )
                    .unwrap();
            }
            let spec = active_result(&materializer, "spec");
            assert!(matches!(
                &spec,
                RepositoryMaterializationResult::SpecError(message)
                    if message.as_str()
                        == "http_archive sha256 must be an exact 64-character hexadecimal digest"
            ));
            if let Some(first) = &first_spec {
                assert_eq!(&spec, first);
            } else {
                first_spec = Some(spec);
            }
            assert!(matches!(
                active_result(&materializer, "transport"),
                RepositoryMaterializationResult::TransportError {
                    generation: RepositoryMaterializationGeneration(current),
                    ..
                } if current == generation
            ));
            assert!(matches!(
                active_result(&materializer, "materialization"),
                RepositoryMaterializationResult::MaterializationError {
                    generation: RepositoryMaterializationGeneration(current),
                    ..
                } if current == generation
            ));
            materializer.discard(token).unwrap();
        }

        let token = begin_empty(&materializer);
        let mismatched = native_request(
            &workspace,
            "wrong-kind",
            archive_spec(url(&workspace_root.path().join("absent.tar")), "bad".into()),
            RepositoryMaterializationKind::Local {
                logical_root: NormalizedAbsolutePath::new(workspace_root.path().join("local"))
                    .unwrap(),
            },
        );
        assert_eq!(
            materializer
                .materialize_native(token, mismatched, RepositoryMaterializationGeneration(9),)
                .unwrap_err(),
            RepositorySessionError::KindMismatch
        );
        assert!(
            materializer
                .epoch(token)
                .unwrap()
                .eq(&RepositoryMaterializationResultEpoch::new(workspace, []).unwrap())
        );
        materializer.discard(token).unwrap();
    }

    #[test]
    fn retained_native_local_success_is_rootless_for_a_missing_logical_path() {
        let workspace_root = tempfile::tempdir().unwrap();
        let workspace = NormalizedAbsolutePath::new(workspace_root.path().to_path_buf()).unwrap();
        let materializer = RepositoryMaterializer::new(workspace.clone());
        let request = native_request(
            &workspace,
            "local",
            local_spec("missing"),
            RepositoryMaterializationKind::Local {
                logical_root: NormalizedAbsolutePath::new(workspace_root.path().join("missing"))
                    .unwrap(),
            },
        );
        let token = begin_empty(&materializer);
        let mismatched = native_request(
            &workspace,
            "mismatched",
            local_spec("missing"),
            RepositoryMaterializationKind::Local {
                logical_root: NormalizedAbsolutePath::new(workspace_root.path().join("other"))
                    .unwrap(),
            },
        );
        assert_eq!(
            materializer
                .materialize_native(token, mismatched, RepositoryMaterializationGeneration(1),)
                .unwrap_err(),
            RepositorySessionError::KindMismatch
        );
        materializer
            .materialize_native(
                token,
                request.clone(),
                RepositoryMaterializationGeneration(1),
            )
            .unwrap();
        {
            let state = materializer.state.lock().unwrap();
            assert_eq!(state.next_instance, 1);
            assert!(state.accepted_roots.is_empty());
            assert!(state.active.as_ref().unwrap().provisional_roots.is_empty());
        }
        materializer
            .accept(token, std::slice::from_ref(&request), Vec::new())
            .unwrap();
        let token = materializer.begin().unwrap();
        let preflight = materializer
            .preflight_native(token, std::iter::empty())
            .unwrap();
        assert_eq!(
            preflight.path_observations(),
            &PathObservationEpoch::empty()
        );
        assert_eq!(
            preflight.repository_results(),
            &RepositoryMaterializationResultEpoch::new(
                workspace,
                [RepositoryMaterializationEpochEntry {
                    request,
                    result: RepositoryMaterializationResult::Success(
                        RepositoryMaterializationSuccess::Local,
                    ),
                }],
            )
            .unwrap()
        );
        materializer.discard(token).unwrap();
    }

    #[test]
    fn retained_native_validation_and_materialization_reject_post_io_stale_tokens_without_mutation()
    {
        let workspace_root = tempfile::tempdir().unwrap();
        let workspace = NormalizedAbsolutePath::new(workspace_root.path().to_path_buf()).unwrap();
        let materializer = RepositoryMaterializer::new(workspace.clone());
        let request = materialization_request(
            &workspace,
            "accepted",
            "accepted",
            RepositoryMaterializationKind::Immutable,
        );
        let token = begin_empty(&materializer);
        materializer
            .materialize_with(
                token,
                request.clone(),
                RepositoryMaterializationGeneration(1),
                || {
                    let root = tempfile::tempdir().unwrap();
                    std::fs::write(root.path().join("file"), b"accepted").unwrap();
                    RepositoryMaterializationAttempt::Immutable {
                        bytes: b"accepted".to_vec(),
                        root,
                    }
                },
            )
            .unwrap();
        let (accepted_root, instance) = immutable_result(&materializer, "accepted");
        let demand = PathObservationDemand::new(
            PathObservationNamespace::Materialization(instance),
            NormalizedAbsolutePath::new(accepted_root.join("file")).unwrap(),
            PathObservationOperation::FileBytes,
        );
        let observed = materializer
            .observe_native(token, [demand.clone()])
            .unwrap();
        materializer
            .accept(
                token,
                std::slice::from_ref(&request),
                vec![RepositoryValidation::new(
                    request.clone(),
                    vec![(demand.clone(), epoch_result(&observed, &demand))],
                )],
            )
            .unwrap();

        let token = materializer.begin().unwrap();
        let error = materializer
            .preflight_with(token, [], |root_owners, roots, demands| {
                assert!(accepted_root.exists());
                let observed =
                    super::super::path_observation::observe_native(&root_owners, roots, demands)?;
                materializer.discard(token).unwrap();
                assert!(accepted_root.exists());
                Ok(observed)
            })
            .unwrap_err();
        assert_eq!(
            error,
            RepositorySessionError::StaleToken {
                active: None,
                supplied: token,
            }
        );
        assert_eq!(materializer.state.lock().unwrap().accepted.len(), 1);
        assert!(accepted_root.exists());

        let token = materializer.begin().unwrap();
        materializer
            .preflight_native(token, std::iter::empty())
            .unwrap();
        let next_instance = materializer.state.lock().unwrap().next_instance;
        let new_request = materialization_request(
            &workspace,
            "new",
            "new",
            RepositoryMaterializationKind::Immutable,
        );
        let mut provisional = None;
        let error = materializer
            .materialize_with(
                token,
                new_request,
                RepositoryMaterializationGeneration(2),
                || {
                    let root = tempfile::tempdir().unwrap();
                    provisional = Some(root.path().to_path_buf());
                    materializer.discard(token).unwrap();
                    assert!(root.path().exists());
                    RepositoryMaterializationAttempt::Immutable {
                        bytes: b"new".to_vec(),
                        root,
                    }
                },
            )
            .unwrap_err();
        assert_eq!(
            error,
            RepositorySessionError::StaleToken {
                active: None,
                supplied: token,
            }
        );
        assert!(!provisional.unwrap().exists());
        let state = materializer.state.lock().unwrap();
        assert_eq!(state.next_instance, next_instance);
        assert_eq!(state.accepted.len(), 1);
        assert!(state.active.is_none());
    }

    #[test]
    fn retained_session_lease_allocators_cancellation_and_lock_scope_are_exact() {
        for invalid in [0, u64::MAX] {
            let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
            let materializer = RepositoryMaterializer::new(workspace);
            materializer.state.lock().unwrap().next_token = invalid;
            let error = materializer.begin().unwrap_err();
            assert_eq!(error, RepositorySessionError::TokenExhausted);
            let state = materializer.state.lock().unwrap();
            assert_eq!(state.next_token, invalid);
            assert!(state.active.is_none());
        }

        let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
        let materializer = RepositoryMaterializer::new(workspace.clone());
        let pending = materializer.begin().unwrap();
        assert_eq!(
            materializer.epoch(pending).unwrap_err(),
            RepositorySessionError::ValidationIncomplete
        );
        assert_eq!(
            materializer.accept(pending, &[], Vec::new()).unwrap_err(),
            RepositorySessionError::ValidationIncomplete
        );
        let pending_request = materialization_request(
            &workspace,
            "pending",
            "pending",
            RepositoryMaterializationKind::Local {
                logical_root: NormalizedAbsolutePath::new("/logical/pending").unwrap(),
            },
        );
        let mut pending_materializations = 0;
        assert_eq!(
            materializer
                .materialize_with(
                    pending,
                    pending_request,
                    RepositoryMaterializationGeneration(1),
                    || {
                        pending_materializations += 1;
                        RepositoryMaterializationAttempt::Local
                    },
                )
                .unwrap_err(),
            RepositorySessionError::ValidationIncomplete
        );
        assert_eq!(pending_materializations, 0);
        materializer.discard(pending).unwrap();

        let token = begin_empty(&materializer);
        assert_eq!(token, RepositorySessionToken(2));
        assert_eq!(
            materializer
                .validate(token, &mut |_, _| unreachable!())
                .unwrap_err(),
            RepositorySessionError::ValidationAlreadyStarted
        );
        assert_eq!(
            materializer.begin().unwrap_err(),
            RepositorySessionError::Busy
        );
        assert_eq!(materializer.state.lock().unwrap().next_token, 3);

        let stale = RepositorySessionToken(0);
        assert!(matches!(
            materializer.epoch(stale),
            Err(RepositorySessionError::StaleToken { .. })
        ));
        assert!(matches!(
            materializer.accept(stale, &[], Vec::new()),
            Err(RepositorySessionError::StaleToken { .. })
        ));
        assert!(matches!(
            materializer.discard(stale),
            Err(RepositorySessionError::StaleToken { .. })
        ));
        assert_eq!(materializer.state.lock().unwrap().next_token, 3);

        let request = materialization_request(
            &workspace,
            "repo",
            "one",
            RepositoryMaterializationKind::Immutable,
        );
        let mut provisional_path = None;
        materializer
            .materialize_with(
                token,
                request,
                RepositoryMaterializationGeneration(1),
                || {
                    assert_eq!(
                        materializer.begin().unwrap_err(),
                        RepositorySessionError::Busy
                    );
                    let root = tempfile::tempdir().unwrap();
                    std::fs::write(root.path().join("value"), b"one").unwrap();
                    provisional_path = Some(root.path().to_path_buf());
                    RepositoryMaterializationAttempt::Immutable {
                        bytes: b"one".to_vec(),
                        root,
                    }
                },
            )
            .unwrap();
        let provisional_path = provisional_path.unwrap();
        assert!(provisional_path.exists());
        let copied_token = token;
        let _ = copied_token;
        assert_eq!(
            materializer.begin().unwrap_err(),
            RepositorySessionError::Busy
        );
        assert!(provisional_path.exists());

        materializer.discard(token).unwrap();
        assert!(!provisional_path.exists());
        assert_eq!(
            materializer.epoch(token).unwrap_err(),
            RepositorySessionError::StaleToken {
                active: None,
                supplied: token,
            }
        );
        let next = begin_empty(&materializer);
        assert_eq!(next, RepositorySessionToken(3));
        assert_eq!(materializer.state.lock().unwrap().next_instance, 2);
        assert_eq!(
            materializer.discard(token).unwrap_err(),
            RepositorySessionError::StaleToken {
                active: Some(next),
                supplied: token,
            }
        );
        let request = materialization_request(
            &workspace,
            "repo",
            "two",
            RepositoryMaterializationKind::Immutable,
        );
        materializer
            .materialize_with(
                next,
                request,
                RepositoryMaterializationGeneration(2),
                || RepositoryMaterializationAttempt::Immutable {
                    bytes: b"two".to_vec(),
                    root: tempfile::tempdir().unwrap(),
                },
            )
            .unwrap();
        assert_eq!(
            immutable_result(&materializer, "repo").1,
            PathObservationInstanceId::new(2)
        );
        materializer.discard(next).unwrap();

        for invalid in [0, u64::MAX] {
            let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
            let materializer = RepositoryMaterializer::new(workspace.clone());
            materializer.state.lock().unwrap().next_instance = invalid;
            let token = begin_empty(&materializer);
            let request = materialization_request(
                &workspace,
                "repo",
                "invalid",
                RepositoryMaterializationKind::Immutable,
            );
            let mut root_path = None;
            let error = materializer
                .materialize_with(
                    token,
                    request,
                    RepositoryMaterializationGeneration(1),
                    || {
                        let root = tempfile::tempdir().unwrap();
                        root_path = Some(root.path().to_path_buf());
                        RepositoryMaterializationAttempt::Immutable {
                            bytes: Vec::new(),
                            root,
                        }
                    },
                )
                .unwrap_err();
            assert_eq!(error, RepositorySessionError::InstanceExhausted);
            assert!(!root_path.unwrap().exists());
            let state = materializer.state.lock().unwrap();
            assert_eq!(state.next_instance, invalid);
            assert!(state.active.as_ref().unwrap().entries.is_empty());
            assert!(state.active.as_ref().unwrap().provisional_roots.is_empty());
            drop(state);
            materializer.discard(token).unwrap();
        }
    }

    #[test]
    fn retained_session_epochs_acceptance_reuse_and_old_roots_are_exact() {
        let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
        let materializer = RepositoryMaterializer::new(workspace.clone());
        let token = begin_empty(&materializer);
        let request_b = materialization_request(
            &workspace,
            "b",
            "b",
            RepositoryMaterializationKind::Immutable,
        );
        let request_a = materialization_request(
            &workspace,
            "a",
            "a",
            RepositoryMaterializationKind::Immutable,
        );
        let request_local = materialization_request(
            &workspace,
            "local",
            "local",
            RepositoryMaterializationKind::Local {
                logical_root: NormalizedAbsolutePath::new("/logical/local").unwrap(),
            },
        );
        let mut paths = Vec::new();
        for (request, value) in [(&request_b, b"b".as_slice()), (&request_a, b"a".as_slice())] {
            materializer
                .materialize_with(
                    token,
                    request.clone(),
                    RepositoryMaterializationGeneration(1),
                    || {
                        let root = tempfile::tempdir().unwrap();
                        std::fs::write(root.path().join("value"), value).unwrap();
                        paths.push(root.path().to_path_buf());
                        RepositoryMaterializationAttempt::Immutable {
                            bytes: value.to_vec(),
                            root,
                        }
                    },
                )
                .unwrap();
        }
        let instance_after_immutable = materializer.state.lock().unwrap().next_instance;
        materializer
            .materialize_with(
                token,
                request_local.clone(),
                RepositoryMaterializationGeneration(1),
                || RepositoryMaterializationAttempt::Local,
            )
            .unwrap();
        {
            let state = materializer.state.lock().unwrap();
            assert_eq!(state.next_instance, instance_after_immutable);
            let active = state.active.as_ref().unwrap();
            assert_eq!(
                active
                    .entries
                    .iter()
                    .map(|entry| entry.request.id.canonical_repo.as_str())
                    .collect::<Vec<_>>(),
                ["a", "b", "local"]
            );
        }
        let (path_a, instance_a) = immutable_result(&materializer, "a");
        let (path_b, _) = immutable_result(&materializer, "b");
        let validation_a = RepositoryValidation::new(
            request_a.clone(),
            vec![(
                PathObservationDemand::new(
                    PathObservationNamespace::Materialization(instance_a),
                    NormalizedAbsolutePath::new("/MODULE.bazel").unwrap(),
                    PathObservationOperation::FileBytes,
                ),
                PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(
                    &b"a"[..],
                ))),
            )],
        );
        let validation_local = RepositoryValidation::new(
            request_local.clone(),
            vec![(
                PathObservationDemand::new(
                    PathObservationNamespace::Host,
                    NormalizedAbsolutePath::new("/logical/local").unwrap(),
                    PathObservationOperation::Lstat,
                ),
                PathObservationResult::Lstat(PathOperationResult::Missing),
            )],
        );
        materializer
            .accept(
                token,
                &[request_a.clone(), request_local.clone()],
                vec![validation_a.clone(), validation_local],
            )
            .unwrap();
        assert!(path_a.exists());
        assert!(!path_b.exists());
        {
            let state = materializer.state.lock().unwrap();
            assert_eq!(state.accepted_roots.len(), 1);
            assert_eq!(
                state
                    .accepted
                    .iter()
                    .map(|entry| entry.request.id.canonical_repo.as_str())
                    .collect::<Vec<_>>(),
                ["a", "local"]
            );
        }

        let token = begin_validated(&materializer, |demand, root| match demand.namespace() {
            PathObservationNamespace::Materialization(_) => {
                assert_eq!(root, Some(path_a.as_path()));
                validation_a.observations[0].1.clone()
            }
            PathObservationNamespace::Host => {
                assert!(root.is_none());
                PathObservationResult::Lstat(PathOperationResult::Missing)
            }
        });
        let epoch = materializer
            .materialize_with(
                token,
                request_a.clone(),
                RepositoryMaterializationGeneration(2),
                || panic!("clean exact request must reuse offline"),
            )
            .unwrap();
        assert_eq!(epoch, materializer.epoch(token).unwrap());
        let (reused_path, reused_instance) = immutable_result(&materializer, "a");
        assert_eq!(reused_path, path_a);
        assert_eq!(reused_instance, instance_a);
        materializer.discard(token).unwrap();

        let token = begin_validated(&materializer, |demand, root| match demand.namespace() {
            PathObservationNamespace::Materialization(_) => {
                assert_eq!(root, Some(path_a.as_path()));
                validation_a.observations[0].1.clone()
            }
            PathObservationNamespace::Host => {
                assert!(root.is_none());
                PathObservationResult::Lstat(PathOperationResult::Missing)
            }
        });
        let changed_request = materialization_request(
            &workspace,
            "a",
            "changed",
            RepositoryMaterializationKind::Immutable,
        );
        let mut replacement_path = None;
        materializer
            .materialize_with(
                token,
                changed_request.clone(),
                RepositoryMaterializationGeneration(2),
                || {
                    let root = tempfile::tempdir().unwrap();
                    std::fs::write(root.path().join("value"), b"changed").unwrap();
                    replacement_path = Some(root.path().to_path_buf());
                    RepositoryMaterializationAttempt::Immutable {
                        bytes: b"changed".to_vec(),
                        root,
                    }
                },
            )
            .unwrap();
        let changed_local = materialization_request(
            &workspace,
            "local",
            "local",
            RepositoryMaterializationKind::Local {
                logical_root: NormalizedAbsolutePath::new("/logical/other").unwrap(),
            },
        );
        let mut local_materializations = 0;
        materializer
            .materialize_with(
                token,
                changed_local,
                RepositoryMaterializationGeneration(2),
                || {
                    local_materializations += 1;
                    RepositoryMaterializationAttempt::Local
                },
            )
            .unwrap();
        assert_eq!(local_materializations, 1);
        let replacement_path = replacement_path.unwrap();
        materializer
            .accept(token, std::slice::from_ref(&changed_request), Vec::new())
            .unwrap();
        assert!(path_a.exists());
        assert!(replacement_path.exists());
        let state = materializer.state.lock().unwrap();
        assert_eq!(state.accepted_roots.len(), 2);
        assert_eq!(state.accepted.len(), 1);
        assert_eq!(*state.accepted[0].request, *changed_request);
    }

    #[test]
    fn retained_session_dirtiness_is_repo_local_and_discard_preserves_cache() {
        let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
        let materializer = RepositoryMaterializer::new(workspace.clone());
        let token = begin_empty(&materializer);
        let request_a = materialization_request(
            &workspace,
            "a",
            "a",
            RepositoryMaterializationKind::Immutable,
        );
        let request_b = materialization_request(
            &workspace,
            "b",
            "b",
            RepositoryMaterializationKind::Immutable,
        );
        for request in [&request_a, &request_b] {
            materializer
                .materialize_with(
                    token,
                    request.clone(),
                    RepositoryMaterializationGeneration(1),
                    || {
                        let root = tempfile::tempdir().unwrap();
                        RepositoryMaterializationAttempt::Immutable {
                            bytes: request.id.canonical_repo.as_str().as_bytes().to_vec(),
                            root,
                        }
                    },
                )
                .unwrap();
        }
        let (accepted_path_a, instance_a) = immutable_result(&materializer, "a");
        let (_, instance_b) = immutable_result(&materializer, "b");
        let validation = [(&request_a, instance_a), (&request_b, instance_b)]
            .into_iter()
            .map(|(request, instance)| {
                RepositoryValidation::new(
                    request.clone(),
                    vec![(
                        PathObservationDemand::new(
                            PathObservationNamespace::Materialization(instance),
                            NormalizedAbsolutePath::new("/MODULE.bazel").unwrap(),
                            PathObservationOperation::Lstat,
                        ),
                        PathObservationResult::Lstat(PathOperationResult::Missing),
                    )],
                )
            })
            .collect::<Vec<_>>();
        materializer
            .accept(token, &[request_a.clone(), request_b.clone()], validation)
            .unwrap();

        let cancelled = materializer.begin().unwrap();
        let mut discarded = false;
        let error = materializer
            .validate(cancelled, &mut |_, _| {
                if !discarded {
                    discarded = true;
                    materializer.discard(cancelled).unwrap();
                }
                PathObservationResult::Lstat(PathOperationResult::Missing)
            })
            .unwrap_err();
        assert_eq!(
            error,
            RepositorySessionError::StaleToken {
                active: None,
                supplied: cancelled,
            }
        );
        assert!(discarded);
        assert_eq!(materializer.state.lock().unwrap().accepted.len(), 2);

        let token = begin_validated(&materializer, |demand, _| match demand.namespace() {
            PathObservationNamespace::Materialization(instance) if instance == instance_a => {
                PathObservationResult::Lstat(PathOperationResult::Missing)
            }
            PathObservationNamespace::Materialization(instance) if instance == instance_b => {
                lstat(PathNodeKind::RegularFile, 1, 1, 1, 1, 0o644)
            }
            _ => unreachable!(),
        });
        assert_eq!(
            materializer
                .state
                .lock()
                .unwrap()
                .active
                .as_ref()
                .unwrap()
                .entries
                .iter()
                .map(|entry| entry.request.id.canonical_repo.as_str())
                .collect::<Vec<_>>(),
            ["a"]
        );
        materializer
            .materialize_with(
                token,
                request_a.clone(),
                RepositoryMaterializationGeneration(2),
                || panic!("clean repository A must reuse"),
            )
            .unwrap();
        let mut dirty_materializations = 0;
        let mut dirty_path = None;
        materializer
            .materialize_with(
                token,
                request_b.clone(),
                RepositoryMaterializationGeneration(2),
                || {
                    dirty_materializations += 1;
                    let root = tempfile::tempdir().unwrap();
                    dirty_path = Some(root.path().to_path_buf());
                    RepositoryMaterializationAttempt::Immutable {
                        bytes: b"new-b".to_vec(),
                        root,
                    }
                },
            )
            .unwrap();
        assert_eq!(dirty_materializations, 1);
        let dirty_path = dirty_path.take().unwrap();
        let (_, replacement_instance_b) = immutable_result(&materializer, "b");
        let expected = RepositoryMaterializationResultEpoch::new(
            workspace.clone(),
            [
                RepositoryMaterializationEpochEntry {
                    request: request_b.clone(),
                    result: RepositoryMaterializationResult::Success(
                        RepositoryMaterializationSuccess::Immutable {
                            source_identity: source_identity(b"new-b"),
                            generation_root: dirty_path.clone(),
                            observation_instance: replacement_instance_b,
                        },
                    ),
                },
                RepositoryMaterializationEpochEntry {
                    request: request_a.clone(),
                    result: RepositoryMaterializationResult::Success(
                        RepositoryMaterializationSuccess::Immutable {
                            source_identity: source_identity(b"a"),
                            generation_root: accepted_path_a,
                            observation_instance: instance_a,
                        },
                    ),
                },
            ],
        )
        .unwrap();
        assert_eq!(materializer.epoch(token).unwrap(), expected);

        let local_a = materialization_request(
            &workspace,
            "a",
            "a",
            RepositoryMaterializationKind::Local {
                logical_root: NormalizedAbsolutePath::new("/different-logical-root").unwrap(),
            },
        );
        assert_eq!(
            materializer
                .materialize_with(
                    token,
                    local_a.clone(),
                    RepositoryMaterializationGeneration(2),
                    || {
                        dirty_materializations += 1;
                        RepositoryMaterializationAttempt::Local
                    },
                )
                .unwrap_err(),
            RepositorySessionError::ConflictingRequest(request_a.id.canonical_repo.clone())
        );
        assert_eq!(dirty_materializations, 1);
        assert!(dirty_path.exists());
        materializer.discard(token).unwrap();
        assert!(!dirty_path.exists());
        assert_eq!(materializer.state.lock().unwrap().accepted.len(), 2);

        let token = begin_validated(&materializer, |_, _| {
            PathObservationResult::Lstat(PathOperationResult::Missing)
        });
        materializer
            .materialize_with(
                token,
                request_a,
                RepositoryMaterializationGeneration(3),
                || panic!("discard must preserve the prior accepted cache"),
            )
            .unwrap();
        materializer.discard(token).unwrap();
    }

    #[test]
    fn retained_session_errors_and_full_request_misses_are_cumulative() {
        let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
        let materializer = RepositoryMaterializer::new(workspace.clone());
        let token = begin_empty(&materializer);
        let seed = materialization_request(
            &workspace,
            "seed",
            "seed",
            RepositoryMaterializationKind::Local {
                logical_root: NormalizedAbsolutePath::new("/logical/seed").unwrap(),
            },
        );
        materializer
            .materialize_with(
                token,
                seed.clone(),
                RepositoryMaterializationGeneration(1),
                || RepositoryMaterializationAttempt::Local,
            )
            .unwrap();
        materializer
            .accept(token, std::slice::from_ref(&seed), Vec::new())
            .unwrap();

        let token = begin_empty(&materializer);
        let request_a = materialization_request(
            &workspace,
            "a",
            "a",
            RepositoryMaterializationKind::Immutable,
        );
        let request_b = materialization_request(
            &workspace,
            "b",
            "b",
            RepositoryMaterializationKind::Immutable,
        );
        materializer
            .materialize_with(
                token,
                request_b.clone(),
                RepositoryMaterializationGeneration(1),
                || RepositoryMaterializationAttempt::TransportError("offline".into()),
            )
            .unwrap();
        materializer
            .materialize_with(
                token,
                request_a.clone(),
                RepositoryMaterializationGeneration(1),
                || RepositoryMaterializationAttempt::SpecError("bad spec".into()),
            )
            .unwrap();
        {
            let state = materializer.state.lock().unwrap();
            assert_eq!(
                state
                    .active
                    .as_ref()
                    .unwrap()
                    .entries
                    .iter()
                    .map(|entry| entry.request.id.canonical_repo.as_str())
                    .collect::<Vec<_>>(),
                ["a", "b", "seed"]
            );
        }
        materializer
            .materialize_with(
                token,
                request_b.clone(),
                RepositoryMaterializationGeneration(2),
                || RepositoryMaterializationAttempt::MaterializationError("bad tar".into()),
            )
            .unwrap();
        let expected = RepositoryMaterializationResultEpoch::new(
            workspace.clone(),
            [
                RepositoryMaterializationEpochEntry {
                    request: seed.clone(),
                    result: RepositoryMaterializationResult::Success(
                        RepositoryMaterializationSuccess::Local,
                    ),
                },
                RepositoryMaterializationEpochEntry {
                    request: request_b.clone(),
                    result: RepositoryMaterializationResult::TransportError {
                        generation: RepositoryMaterializationGeneration(1),
                        message: "offline".into(),
                    },
                },
                RepositoryMaterializationEpochEntry {
                    request: request_a.clone(),
                    result: RepositoryMaterializationResult::SpecError("bad spec".into()),
                },
            ],
        )
        .unwrap();
        assert_eq!(materializer.epoch(token).unwrap(), expected);
        assert_eq!(
            materializer
                .accept(token, &[request_a.clone(), request_b.clone()], Vec::new(),)
                .unwrap_err(),
            RepositorySessionError::NonSuccessSelection(request_a.id.canonical_repo.clone())
        );
        {
            let state = materializer.state.lock().unwrap();
            assert_eq!(state.accepted.len(), 1);
            assert_eq!(*state.accepted[0].request, *seed);
            assert!(state.active.is_some());
        }
        materializer.discard(token).unwrap();

        let token = begin_empty(&materializer);
        let local_a = materialization_request(
            &workspace,
            "a",
            "a",
            RepositoryMaterializationKind::Local {
                logical_root: NormalizedAbsolutePath::new("/logical/a").unwrap(),
            },
        );
        let mut calls = 0;
        materializer
            .materialize_with(
                token,
                local_a.clone(),
                RepositoryMaterializationGeneration(3),
                || {
                    calls += 1;
                    RepositoryMaterializationAttempt::Local
                },
            )
            .unwrap();
        assert_eq!(calls, 1);
        let conflicting = materialization_request(
            &workspace,
            "a",
            "a",
            RepositoryMaterializationKind::Local {
                logical_root: NormalizedAbsolutePath::new("/logical/other").unwrap(),
            },
        );
        assert_eq!(
            materializer
                .materialize_with(
                    token,
                    conflicting.clone(),
                    RepositoryMaterializationGeneration(3),
                    || {
                        calls += 1;
                        RepositoryMaterializationAttempt::Local
                    },
                )
                .unwrap_err(),
            RepositorySessionError::ConflictingRequest(local_a.id.canonical_repo.clone())
        );
        assert_eq!(calls, 1);
        assert_eq!(
            materializer
                .accept(token, std::slice::from_ref(&conflicting), Vec::new())
                .unwrap_err(),
            RepositorySessionError::ConflictingRequest(local_a.id.canonical_repo.clone())
        );
        assert_eq!(
            materializer
                .accept(
                    token,
                    std::slice::from_ref(&local_a),
                    vec![RepositoryValidation::new(
                        conflicting,
                        Vec::<(PathObservationDemand, PathObservationResult)>::new(),
                    )],
                )
                .unwrap_err(),
            RepositorySessionError::ConflictingRequest(local_a.id.canonical_repo.clone())
        );
        assert_eq!(materializer.state.lock().unwrap().accepted.len(), 1);
        let wrong_workspace = materialization_request(
            &NormalizedAbsolutePath::new("/other-workspace").unwrap(),
            "a",
            "a",
            RepositoryMaterializationKind::Local {
                logical_root: NormalizedAbsolutePath::new("/logical/a").unwrap(),
            },
        );
        assert_eq!(
            materializer
                .materialize_with(
                    token,
                    wrong_workspace,
                    RepositoryMaterializationGeneration(3),
                    || panic!("wrong workspace must fail before materialization")
                )
                .unwrap_err(),
            RepositorySessionError::WrongWorkspace
        );
        materializer.discard(token).unwrap();
    }

    fn epoch_result(
        epoch: &PathObservationEpoch,
        demand: &PathObservationDemand,
    ) -> PathObservationResult {
        epoch.get(demand).unwrap().as_ref().clone()
    }

    #[test]
    fn retained_native_bridge_authority_lifecycle_and_structural_errors_are_exact() {
        let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
        let materializer = RepositoryMaterializer::new(workspace.clone());
        let token = begin_empty(&materializer);
        let request = materialization_request(
            &workspace,
            "repo",
            "native",
            RepositoryMaterializationKind::Immutable,
        );
        materializer
            .materialize_with(
                token,
                request.clone(),
                RepositoryMaterializationGeneration(1),
                || {
                    let root = tempfile::tempdir().unwrap();
                    std::fs::write(root.path().join("file"), b"one").unwrap();
                    RepositoryMaterializationAttempt::Immutable {
                        bytes: b"identity".to_vec(),
                        root,
                    }
                },
            )
            .unwrap();
        let (root, instance) = immutable_result(&materializer, "repo");
        let file = NormalizedAbsolutePath::new(root.join("file")).unwrap();
        let outside = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(outside.path(), b"outside").unwrap();
        let outside_path = NormalizedAbsolutePath::new(outside.path().to_path_buf()).unwrap();
        let lstat_demand = PathObservationDemand::new(
            PathObservationNamespace::Materialization(instance),
            file.clone(),
            PathObservationOperation::Lstat,
        );
        let bytes_demand = PathObservationDemand::new(
            PathObservationNamespace::Materialization(instance),
            file,
            PathObservationOperation::FileBytes,
        );
        let escaped_demand = PathObservationDemand::new(
            PathObservationNamespace::Materialization(instance),
            outside_path.clone(),
            PathObservationOperation::FileBytes,
        );
        let host_demand = PathObservationDemand::new(
            PathObservationNamespace::Host,
            outside_path,
            PathObservationOperation::FileBytes,
        );
        let observed = materializer
            .observe_native(
                token,
                [
                    bytes_demand.clone(),
                    escaped_demand.clone(),
                    host_demand.clone(),
                    lstat_demand.clone(),
                ],
            )
            .unwrap();
        assert_eq!(
            epoch_result(&observed, &escaped_demand),
            PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(
                &b"outside"[..]
            )))
        );
        assert_eq!(
            epoch_result(&observed, &host_demand),
            PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(
                &b"outside"[..]
            )))
        );

        for error in [
            materializer
                .observe_native(token, [bytes_demand.clone(), bytes_demand.clone()])
                .unwrap_err(),
            materializer
                .observe_native(
                    token,
                    [PathObservationDemand::new(
                        PathObservationNamespace::Materialization(PathObservationInstanceId::new(
                            0,
                        )),
                        lstat_demand.path().clone(),
                        PathObservationOperation::Lstat,
                    )],
                )
                .unwrap_err(),
            materializer
                .observe_native(
                    token,
                    [PathObservationDemand::new(
                        PathObservationNamespace::Materialization(PathObservationInstanceId::new(
                            999,
                        )),
                        lstat_demand.path().clone(),
                        PathObservationOperation::Lstat,
                    )],
                )
                .unwrap_err(),
        ] {
            assert!(matches!(
                error,
                RepositorySessionError::NativeObservation(_)
            ));
            assert!(root.exists());
            assert_eq!(
                materializer
                    .state
                    .lock()
                    .unwrap()
                    .active
                    .as_ref()
                    .unwrap()
                    .provisional_roots
                    .len(),
                1
            );
        }

        let validation = RepositoryValidation::new(
            request.clone(),
            vec![
                (lstat_demand.clone(), epoch_result(&observed, &lstat_demand)),
                (bytes_demand.clone(), epoch_result(&observed, &bytes_demand)),
            ],
        );
        materializer
            .accept(
                token,
                std::slice::from_ref(&request),
                vec![validation.clone()],
            )
            .unwrap();
        assert!(root.exists());

        let token = materializer.begin().unwrap();
        materializer.validate_native(token).unwrap();
        materializer
            .materialize_with(
                token,
                request.clone(),
                RepositoryMaterializationGeneration(2),
                || panic!("unchanged native validation must reuse"),
            )
            .unwrap();
        assert_eq!(immutable_result(&materializer, "repo").0, root);
        materializer.discard(token).unwrap();

        std::fs::write(root.join("file"), b"two").unwrap();
        let token = materializer.begin().unwrap();
        materializer.validate_native(token).unwrap();
        assert!(
            materializer
                .state
                .lock()
                .unwrap()
                .active
                .as_ref()
                .unwrap()
                .entries
                .is_empty()
        );
        materializer.discard(token).unwrap();

        std::fs::remove_file(root.join("file")).unwrap();
        let token = materializer.begin().unwrap();
        materializer.validate_native(token).unwrap();
        assert!(
            materializer
                .state
                .lock()
                .unwrap()
                .active
                .as_ref()
                .unwrap()
                .entries
                .is_empty()
        );
        materializer.discard(token).unwrap();

        std::fs::write(root.join("file"), b"one").unwrap();
        let token = materializer.begin().unwrap();
        materializer.validate_native(token).unwrap();
        materializer
            .materialize_with(
                token,
                request.clone(),
                RepositoryMaterializationGeneration(3),
                || panic!("exact byte restoration must reuse"),
            )
            .unwrap();
        let duplicate_validation = RepositoryValidation::new(
            request.clone(),
            vec![
                validation.observations()[0].clone(),
                validation.observations()[0].clone(),
            ],
        );
        assert!(matches!(
            materializer.accept(
                token,
                std::slice::from_ref(&request),
                vec![duplicate_validation],
            ),
            Err(RepositorySessionError::InvalidValidation(repo)) if repo == request.id.canonical_repo
        ));
        assert_eq!(materializer.state.lock().unwrap().accepted.len(), 1);
        assert!(root.exists());
        materializer.discard(token).unwrap();

        let token = materializer.begin().unwrap();
        materializer.validate_native(token).unwrap();
        materializer
            .materialize_with(
                token,
                request,
                RepositoryMaterializationGeneration(4),
                || panic!("rejected validation must leave accepted reuse intact"),
            )
            .unwrap();
        materializer.discard(token).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn retained_native_local_validation_is_host_only_and_tracks_symlink_target() {
        use std::os::unix::fs::symlink;

        let workspace_root = tempfile::tempdir().unwrap();
        let workspace = NormalizedAbsolutePath::new(workspace_root.path().to_path_buf()).unwrap();
        let materializer = RepositoryMaterializer::new(workspace.clone());
        let logical = workspace_root.path().join("logical");
        symlink("a", &logical).unwrap();
        let request = materialization_request(
            &workspace,
            "local",
            "local-native",
            RepositoryMaterializationKind::Local {
                logical_root: NormalizedAbsolutePath::new(logical.clone()).unwrap(),
            },
        );
        let token = begin_empty(&materializer);
        materializer
            .materialize_with(
                token,
                request.clone(),
                RepositoryMaterializationGeneration(1),
                || RepositoryMaterializationAttempt::Local,
            )
            .unwrap();
        let demand = PathObservationDemand::new(
            PathObservationNamespace::Host,
            NormalizedAbsolutePath::new(logical.clone()).unwrap(),
            PathObservationOperation::ReadLink,
        );
        let observed = materializer
            .observe_native(token, [demand.clone()])
            .unwrap();
        materializer
            .accept(
                token,
                std::slice::from_ref(&request),
                vec![RepositoryValidation::new(
                    request.clone(),
                    vec![(demand.clone(), epoch_result(&observed, &demand))],
                )],
            )
            .unwrap();
        assert_eq!(materializer.state.lock().unwrap().next_instance, 1);

        let token = materializer.begin().unwrap();
        materializer.validate_native(token).unwrap();
        materializer
            .materialize_with(
                token,
                request.clone(),
                RepositoryMaterializationGeneration(2),
                || panic!("unchanged logical Local target must reuse"),
            )
            .unwrap();
        materializer.discard(token).unwrap();

        std::fs::remove_file(&logical).unwrap();
        symlink("b", &logical).unwrap();
        let token = materializer.begin().unwrap();
        materializer.validate_native(token).unwrap();
        assert!(
            materializer
                .state
                .lock()
                .unwrap()
                .active
                .as_ref()
                .unwrap()
                .entries
                .is_empty()
        );
        assert_eq!(materializer.state.lock().unwrap().next_instance, 1);
        materializer.discard(token).unwrap();
    }

    fn lstat(
        kind: PathNodeKind,
        size: i64,
        mtime: i64,
        ctime: i64,
        node: i64,
        permissions: i32,
    ) -> PathObservationResult {
        PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
            kind,
            size,
            mtime,
            ctime,
            node,
            permissions,
        )))
    }

    fn validation_demand(operation: PathObservationOperation) -> PathObservationDemand {
        PathObservationDemand::new(
            PathObservationNamespace::Host,
            NormalizedAbsolutePath::new("/observed").unwrap(),
            operation,
        )
    }

    #[test]
    fn retained_session_validation_dirtiness_matrix_is_exact() {
        use slug_workspace_v2::PathDirectoryEntries;
        use slug_workspace_v2::PathDirectoryEntry;
        use slug_workspace_v2::PathDirectoryEntryKind;
        use slug_workspace_v2::PathDirectoryName;
        use slug_workspace_v2::PathIoErrorKind;
        use slug_workspace_v2::PathObservationError;

        let lstat_demand = validation_demand(PathObservationOperation::Lstat);
        let missing = vec![(
            lstat_demand.clone(),
            PathObservationResult::Lstat(PathOperationResult::Missing),
        )];
        assert!(!validation_is_dirty(&missing, &[], |_, _| missing[0]
            .1
            .clone()));
        assert!(validation_is_dirty(&missing, &[], |_, _| {
            lstat(PathNodeKind::RegularFile, 1, 1, 1, 1, 0o644)
        }));
        let io_error =
            PathObservationResult::Lstat(PathOperationResult::Error(PathObservationError::Io {
                kind: PathIoErrorKind::PermissionDenied,
                raw_os_error: Some(13),
            }));
        assert!(validation_is_dirty(
            &[(lstat_demand.clone(), io_error.clone())],
            &[],
            |_, _| io_error.clone()
        ));

        let regular = lstat(PathNodeKind::RegularFile, 4, 10, 20, 30, 0o644);
        let file_demand = validation_demand(PathObservationOperation::FileBytes);
        let bytes =
            PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(&b"same"[..])));
        let with_bytes = vec![
            (lstat_demand.clone(), regular.clone()),
            (file_demand, bytes.clone()),
        ];
        assert!(!validation_is_dirty(&with_bytes, &[], |demand, _| {
            match demand.operation() {
                PathObservationOperation::Lstat => {
                    lstat(PathNodeKind::RegularFile, 4, 999, 999, 999, 0)
                }
                PathObservationOperation::FileBytes => bytes.clone(),
                _ => unreachable!(),
            }
        }));
        assert!(validation_is_dirty(&with_bytes, &[], |demand, _| {
            match demand.operation() {
                PathObservationOperation::Lstat => {
                    lstat(PathNodeKind::RegularFile, 5, 10, 20, 30, 0o644)
                }
                PathObservationOperation::FileBytes => bytes.clone(),
                _ => unreachable!(),
            }
        }));
        let special_with_bytes = vec![
            (
                lstat_demand.clone(),
                lstat(PathNodeKind::SpecialFile, 4, 10, 20, 30, 0o644),
            ),
            (
                validation_demand(PathObservationOperation::FileBytes),
                bytes.clone(),
            ),
        ];
        assert!(!validation_is_dirty(
            &special_with_bytes,
            &[],
            |demand, _| match demand.operation() {
                PathObservationOperation::Lstat => {
                    lstat(PathNodeKind::SpecialFile, 4, 999, 999, 999, 0)
                }
                PathObservationOperation::FileBytes => bytes.clone(),
                _ => unreachable!(),
            }
        ));
        assert!(validation_is_dirty(
            &special_with_bytes,
            &[],
            |demand, _| match demand.operation() {
                PathObservationOperation::Lstat => {
                    lstat(PathNodeKind::SpecialFile, 5, 10, 20, 30, 0o644)
                }
                PathObservationOperation::FileBytes => bytes.clone(),
                _ => unreachable!(),
            }
        ));
        assert!(validation_is_dirty(&with_bytes, &[], |demand, _| {
            match demand.operation() {
                PathObservationOperation::Lstat => {
                    lstat(PathNodeKind::Directory, 4, 10, 20, 30, 0o644)
                }
                PathObservationOperation::FileBytes => bytes.clone(),
                _ => unreachable!(),
            }
        }));
        assert!(validation_is_dirty(&with_bytes, &[], |demand, _| {
            match demand.operation() {
                PathObservationOperation::Lstat => regular.clone(),
                PathObservationOperation::FileBytes => PathObservationResult::FileBytes(
                    PathOperationResult::Present(Arc::from(&b"changed"[..])),
                ),
                _ => unreachable!(),
            }
        }));

        let only_lstat = vec![(lstat_demand.clone(), regular.clone())];
        for changed in [
            lstat(PathNodeKind::RegularFile, 5, 10, 20, 30, 0o644),
            lstat(PathNodeKind::RegularFile, 4, 11, 20, 30, 0o644),
            lstat(PathNodeKind::RegularFile, 4, 10, 20, 31, 0o644),
        ] {
            assert!(validation_is_dirty(&only_lstat, &[], |_, _| changed.clone()));
        }
        for clean in [
            lstat(PathNodeKind::RegularFile, 4, 10, 999, 30, 0o644),
            lstat(PathNodeKind::RegularFile, 4, 10, 20, 30, 0o600),
            lstat(PathNodeKind::Directory, 999, 999, 999, 999, 0),
            lstat(PathNodeKind::Symlink, 999, 999, 999, 999, 0),
        ] {
            let prior = match &clean {
                PathObservationResult::Lstat(PathOperationResult::Present(value))
                    if matches!(
                        value.kind(),
                        PathNodeKind::Directory | PathNodeKind::Symlink
                    ) =>
                {
                    vec![(lstat_demand.clone(), lstat(value.kind(), 1, 1, 1, 1, 0o644))]
                }
                _ => only_lstat.clone(),
            };
            assert!(!validation_is_dirty(&prior, &[], |_, _| clean.clone()));
        }
        let special = vec![(
            lstat_demand.clone(),
            lstat(PathNodeKind::SpecialFile, 4, 10, 20, 30, 0o644),
        )];
        assert!(validation_is_dirty(&special, &[], |_, _| {
            lstat(PathNodeKind::SpecialFile, 5, 10, 20, 30, 0o644)
        }));

        let readlink_demand = validation_demand(PathObservationOperation::ReadLink);
        let target = PathObservationResult::ReadLink(PathOperationResult::Present(Arc::new(
            PathBuf::from("../target"),
        )));
        assert!(!validation_is_dirty(
            &[(readlink_demand.clone(), target.clone())],
            &[],
            |_, _| target.clone()
        ));
        assert!(validation_is_dirty(
            &[(readlink_demand, target)],
            &[],
            |_, _| PathObservationResult::ReadLink(PathOperationResult::Present(Arc::new(
                PathBuf::from("../other"),
            )))
        ));

        let entries_demand = validation_demand(PathObservationOperation::DirectoryEntries);
        let entry =
            |name, kind| PathDirectoryEntry::new(PathDirectoryName::new(name).unwrap(), kind);
        let entries = PathDirectoryEntries::new([
            entry("a", PathDirectoryEntryKind::File),
            entry("b", PathDirectoryEntryKind::Directory),
        ]);
        let entries_result =
            PathObservationResult::DirectoryEntries(PathOperationResult::Present(entries.clone()));
        assert!(!validation_is_dirty(
            &[(entries_demand.clone(), entries_result.clone())],
            &[],
            |_, _| PathObservationResult::DirectoryEntries(PathOperationResult::Present(
                PathDirectoryEntries::new([
                    entry("b", PathDirectoryEntryKind::Directory),
                    entry("a", PathDirectoryEntryKind::File),
                ])
            ))
        ));
        assert!(validation_is_dirty(
            &[(entries_demand.clone(), entries_result.clone())],
            &[],
            |_, _| PathObservationResult::DirectoryEntries(PathOperationResult::Present(
                PathDirectoryEntries::new([
                    entry("a", PathDirectoryEntryKind::Directory),
                    entry("b", PathDirectoryEntryKind::Directory),
                ])
            ))
        ));

        let long_path_demand = PathObservationDemand::windows_long_path(
            NormalizedAbsolutePath::new("/windows-long-path").unwrap(),
            Arc::from("C:/PROGRA~1".encode_utf16().collect::<Vec<_>>()),
        );
        let long_path = PathObservationResult::WindowsLongPath(Arc::from(
            "C:/Program Files".encode_utf16().collect::<Vec<_>>(),
        ));
        assert!(!validation_is_dirty(
            &[(long_path_demand.clone(), long_path.clone())],
            &[],
            |_, _| long_path.clone()
        ));
        assert!(validation_is_dirty(
            &[(long_path_demand, long_path)],
            &[],
            |_, _| PathObservationResult::WindowsLongPath(Arc::from(
                "C:/Programs".encode_utf16().collect::<Vec<_>>()
            ))
        ));
    }

    #[test]
    fn observation_instance_allocator_is_checked_and_preserves_invalid_state() {
        let mut next_instance = 1;
        assert_eq!(
            allocate_observation_instance(&mut next_instance).unwrap(),
            PathObservationInstanceId::new(1)
        );
        assert_eq!(next_instance, 2);
        assert_eq!(
            allocate_observation_instance(&mut next_instance).unwrap(),
            PathObservationInstanceId::new(2)
        );
        assert_eq!(next_instance, 3);

        for invalid in [0, u64::MAX] {
            let mut next_instance = invalid;
            let error = allocate_observation_instance(&mut next_instance).unwrap_err();
            assert_eq!(
                error.message,
                "repository materialization observation instance is invalid or exhausted"
            );
            assert_eq!(next_instance, invalid);
        }
    }

    #[test]
    fn failed_observation_instance_allocation_drops_unretained_root() {
        for invalid in [0, u64::MAX] {
            let io = LocalRepositoryIo {
                immutable_roots: Mutex::new(RetainedImmutableRoots {
                    next_instance: invalid,
                    roots: Vec::new(),
                }),
            };
            let root = tempfile::tempdir().unwrap();
            let path = root.path().to_owned();

            let error = io.retain(root).unwrap_err();

            assert_eq!(
                error.message,
                "repository materialization observation instance is invalid or exhausted"
            );
            assert!(!path.exists());
            let retained = io
                .immutable_roots
                .lock()
                .expect("immutable repository root mutex poisoned");
            assert_eq!(retained.next_instance, invalid);
            assert!(retained.roots.is_empty());
        }
    }

    #[test]
    fn archive_requires_the_fixed_tar_shape_and_decodes_file_uris() {
        let source = tempfile::tempdir().unwrap();
        let content = source.path().join("space name");
        std::fs::create_dir(&content).unwrap();
        std::fs::write(content.join("MODULE.bazel"), b"module(name = 'archive')").unwrap();
        let archive = source.path().join("source archive.tar");
        assert!(
            Command::new("tar")
                .args(["--format=ustar", "-cf"])
                .arg(&archive)
                .args(["-C"])
                .arg(source.path())
                .arg("space name")
                .status()
                .unwrap()
                .success()
        );
        let bytes = std::fs::read(&archive).unwrap();
        let digest = format!("{:x}", Sha256::digest(&bytes));
        let url = url::Url::from_file_path(&archive).unwrap().to_string();
        let Materialized::Immutable { root, .. } =
            materialize_archive(&archive_spec(url, digest)).unwrap()
        else {
            panic!("archive source must materialize immutably");
        };
        assert_eq!(
            std::fs::read(root.path().join("space name/MODULE.bazel")).unwrap(),
            b"module(name = 'archive')"
        );
    }

    #[tokio::test]
    async fn immutable_materializations_retain_prior_equal_generations() {
        let (source, spec) = immutable_archive_fixture();
        let io = LocalRepositoryIo::new();
        let RepositoryIoOutcome::Immutable {
            source_identity: first_identity,
            generation_root: first_root,
            observation_instance: first_instance,
        } = io.materialize(source.path(), &spec).await.unwrap()
        else {
            panic!("archive source must materialize immutably");
        };
        let RepositoryIoOutcome::Immutable {
            source_identity: second_identity,
            generation_root: second_root,
            observation_instance: second_instance,
        } = io.materialize(source.path(), &spec).await.unwrap()
        else {
            panic!("archive source must materialize immutably");
        };

        assert_eq!(first_identity, second_identity);
        assert_ne!(first_instance.value(), 0);
        assert_ne!(second_instance.value(), 0);
        assert_ne!(first_instance, second_instance);
        assert_ne!(first_root, second_root);
        assert_eq!(
            std::fs::read(first_root.join("content/MODULE.bazel")).unwrap(),
            b"module(name = 'archive')"
        );
        assert_eq!(
            std::fs::read(second_root.join("content/MODULE.bazel")).unwrap(),
            b"module(name = 'archive')"
        );
        let retained = io
            .immutable_roots
            .lock()
            .expect("immutable repository root mutex poisoned");
        assert_eq!(retained.roots.len(), 2);
        assert_eq!(retained.roots[0].0, first_instance);
        assert_eq!(retained.roots[0].1.path(), first_root);
        assert_eq!(retained.roots[1].0, second_instance);
        assert_eq!(retained.roots[1].1.path(), second_root);
    }

    #[tokio::test]
    async fn concurrent_immutable_materializations_get_unique_retained_instances() {
        let (source, spec) = immutable_archive_fixture();
        let io = LocalRepositoryIo::new();

        let (first, second) = tokio::join!(
            io.materialize(source.path(), &spec),
            io.materialize(source.path(), &spec)
        );
        let RepositoryIoOutcome::Immutable {
            source_identity: first_identity,
            generation_root: first_root,
            observation_instance: first_instance,
        } = first.unwrap()
        else {
            panic!("archive source must materialize immutably");
        };
        let RepositoryIoOutcome::Immutable {
            source_identity: second_identity,
            generation_root: second_root,
            observation_instance: second_instance,
        } = second.unwrap()
        else {
            panic!("archive source must materialize immutably");
        };

        assert_eq!(first_identity, second_identity);
        assert_ne!(first_instance.value(), 0);
        assert_ne!(second_instance.value(), 0);
        assert_ne!(first_instance, second_instance);
        assert_ne!(first_root, second_root);
        assert_eq!(
            std::fs::read(first_root.join("content/MODULE.bazel")).unwrap(),
            b"module(name = 'archive')"
        );
        assert_eq!(
            std::fs::read(second_root.join("content/MODULE.bazel")).unwrap(),
            b"module(name = 'archive')"
        );
        let retained = io
            .immutable_roots
            .lock()
            .expect("immutable repository root mutex poisoned");
        assert_eq!(retained.roots.len(), 2);
        for (instance, root) in [
            (first_instance, first_root.as_path()),
            (second_instance, second_root.as_path()),
        ] {
            assert!(
                retained
                    .roots
                    .iter()
                    .any(|retained| retained.0 == instance && retained.1.path() == root)
            );
        }
    }

    #[test]
    fn retained_native_git_preserves_external_tar_and_exact_stage_boundaries() {
        let directory = tempfile::tempdir().unwrap();
        let checkout = directory.path().join("checkout");
        assert!(
            Command::new("git")
                .args(["init"])
                .arg(&checkout)
                .status()
                .unwrap()
                .success()
        );
        std::fs::write(checkout.join("MODULE.bazel"), b"module(name = 'git')").unwrap();
        for args in [
            vec!["-C", checkout.to_str().unwrap(), "add", "MODULE.bazel"],
            vec![
                "-C",
                checkout.to_str().unwrap(),
                "-c",
                "user.name=Slug test",
                "-c",
                "user.email=slug@example.com",
                "commit",
                "-m",
                "source",
            ],
        ] {
            assert!(Command::new("git").args(args).status().unwrap().success());
        }
        let commit = String::from_utf8(
            Command::new("git")
                .args(["-C"])
                .arg(&checkout)
                .args(["rev-parse", "HEAD"])
                .output()
                .unwrap()
                .stdout,
        )
        .unwrap()
        .trim()
        .to_owned();
        let bare = directory.path().join("source.git");
        assert!(
            Command::new("git")
                .args(["clone", "--bare"])
                .arg(&checkout)
                .arg(&bare)
                .status()
                .unwrap()
                .success()
        );
        let remote = url::Url::from_file_path(&bare).unwrap().to_string();
        let Materialized::Immutable { root, .. } =
            materialize_git(&git_spec(remote.clone(), commit.clone())).unwrap()
        else {
            panic!("git source must materialize immutably");
        };
        assert_eq!(
            std::fs::read(root.path().join("MODULE.bazel")).unwrap(),
            b"module(name = 'git')"
        );

        let workspace = NormalizedAbsolutePath::new(directory.path().to_path_buf()).unwrap();
        let materializer = RepositoryMaterializer::new(workspace.clone());
        let token = begin_empty(&materializer);
        let request = native_request(
            &workspace,
            "git",
            git_spec(remote.clone(), commit.clone()),
            RepositoryMaterializationKind::Immutable,
        );
        materializer
            .materialize_native(token, request, RepositoryMaterializationGeneration(4))
            .unwrap();
        let (native_root, _) = immutable_result(&materializer, "git");
        assert_eq!(
            std::fs::read(native_root.join("MODULE.bazel")).unwrap(),
            b"module(name = 'git')"
        );

        let staged_spec = git_spec(remote.clone(), commit);
        let staged = native_request(
            &workspace,
            "git-stage",
            staged_spec.clone(),
            RepositoryMaterializationKind::Immutable,
        );
        materializer
            .materialize_with(
                token,
                staged,
                RepositoryMaterializationGeneration(5),
                || {
                    materialized_attempt(materialize_git_staged_with(&staged_spec, |_, _| {
                        Err(RepositoryTransportError {
                            message: "scripted external tar failure".into(),
                        })
                    }))
                },
            )
            .unwrap();
        assert!(matches!(
            active_result(&materializer, "git-stage"),
            RepositoryMaterializationResult::MaterializationError {
                generation: RepositoryMaterializationGeneration(5),
                ..
            }
        ));

        let invalid = native_request(
            &workspace,
            "invalid",
            git_spec(remote.clone(), "bad".into()),
            RepositoryMaterializationKind::Immutable,
        );
        materializer
            .materialize_native(token, invalid, RepositoryMaterializationGeneration(4))
            .unwrap();
        assert!(matches!(
            active_result(&materializer, "invalid"),
            RepositoryMaterializationResult::SpecError(_)
        ));

        let unavailable = native_request(
            &workspace,
            "unavailable",
            git_spec(remote, "0".repeat(40)),
            RepositoryMaterializationKind::Immutable,
        );
        materializer
            .materialize_native(token, unavailable, RepositoryMaterializationGeneration(4))
            .unwrap();
        assert!(matches!(
            active_result(&materializer, "unavailable"),
            RepositoryMaterializationResult::TransportError {
                generation: RepositoryMaterializationGeneration(4),
                ..
            }
        ));
        materializer.discard(token).unwrap();
    }
}
