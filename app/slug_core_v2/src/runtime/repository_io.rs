/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License found in the LICENSE-APACHE file in the root directory of this
 * source tree. You may select the license that applies to you.
 */

use std::fs::File;
use std::fs::OpenOptions;
use std::io::Write;
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
use slug_bzlmod_v2::GeneratedRepositoryFileEffectPlan;
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
    GeneratedImmutable {
        source_identity: Arc<str>,
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

    #[cfg(test)]
    pub(super) fn active_result_for_test(
        &self,
        token: RepositorySessionToken,
        canonical_repo: &str,
    ) -> RepositoryMaterializationResult {
        self.state
            .lock()
            .expect("repository materializer mutex poisoned")
            .active
            .as_ref()
            .filter(|active| active.token == token)
            .and_then(|active| {
                active
                    .entries
                    .iter()
                    .find(|entry| entry.request.id.canonical_repo.as_str() == canonical_repo)
            })
            .expect("active repository materialization result")
            .result
            .clone()
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
            RepositoryMaterializationAttempt::GeneratedImmutable {
                source_identity,
                root,
            } => PreparedMaterializationAttempt::Immutable {
                source_identity,
                root,
            },
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
            ) | (
                RepositoryMaterializationKind::GeneratedFileEffects(_),
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
            (
                RepositoryMaterializationKind::GeneratedFileEffects(_),
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

pub(super) enum Materialized {
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

trait GeneratedRepositoryFileEffectsIo {
    fn temporary_root(&mut self) -> Result<tempfile::TempDir, String>;
    fn create_parent(&mut self, path: &Path) -> Result<(), String>;
    fn create_file(&mut self, path: &Path) -> Result<File, String>;
    fn write_file(&mut self, file: &mut File, content: &[u8]) -> Result<(), String>;
    fn flush_file(&mut self, file: &mut File) -> Result<(), String>;
    fn set_mode(&mut self, path: &Path, executable: bool) -> Result<(), String>;
}

struct NativeGeneratedRepositoryFileEffectsIo;

impl GeneratedRepositoryFileEffectsIo for NativeGeneratedRepositoryFileEffectsIo {
    fn temporary_root(&mut self) -> Result<tempfile::TempDir, String> {
        tempfile::tempdir().map_err(|error| error.to_string())
    }

    fn create_parent(&mut self, path: &Path) -> Result<(), String> {
        std::fs::create_dir_all(path).map_err(|error| error.to_string())
    }

    fn create_file(&mut self, path: &Path) -> Result<File, String> {
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|error| error.to_string())
    }

    fn write_file(&mut self, file: &mut File, content: &[u8]) -> Result<(), String> {
        file.write_all(content).map_err(|error| error.to_string())
    }

    fn flush_file(&mut self, file: &mut File) -> Result<(), String> {
        file.flush().map_err(|error| error.to_string())
    }

    fn set_mode(&mut self, path: &Path, executable: bool) -> Result<(), String> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            std::fs::set_permissions(
                path,
                std::fs::Permissions::from_mode(if executable { 0o755 } else { 0o644 }),
            )
            .map_err(|error| error.to_string())?;
        }
        #[cfg(not(unix))]
        {
            let _ = (path, executable);
            Err("generated repository executable mode is unsupported on this platform".into())
        }
        #[cfg(unix)]
        Ok(())
    }
}

fn generated_repository_file_effect_source_association(
    plan: &GeneratedRepositoryFileEffectPlan,
) -> Arc<str> {
    let mut digest = Sha256::new();
    digest.update(b"slug.generated-repository-file-effects.v1\\0");
    for effect in plan.effects() {
        for bytes in [effect.path().as_bytes(), effect.content()] {
            digest.update((bytes.len() as u64).to_be_bytes());
            digest.update(bytes);
        }
        digest.update([u8::from(effect.executable())]);
    }
    Arc::from(hex::encode(digest.finalize()))
}

fn generated_repository_file_effect_paths_are_valid(
    plan: &GeneratedRepositoryFileEffectPlan,
) -> bool {
    let paths = plan
        .effects()
        .iter()
        .map(|effect| effect.path())
        .collect::<Vec<_>>();
    paths.iter().all(|path| {
        !path.is_empty()
            && !path.ends_with('/')
            && !path.contains('\\')
            && !path.contains('\0')
            && path
                .split('/')
                .all(|component| !component.is_empty() && component != "." && component != "..")
    }) && paths.iter().enumerate().all(|(index, path)| {
        paths.iter().skip(index + 1).all(|other| {
            path != other
                && !other
                    .strip_prefix(path)
                    .is_some_and(|suffix| suffix.starts_with('/'))
                && !path
                    .strip_prefix(other)
                    .is_some_and(|suffix| suffix.starts_with('/'))
        })
    })
}

fn materialize_generated_repository_file_effects(
    plan: &GeneratedRepositoryFileEffectPlan,
    io: &mut impl GeneratedRepositoryFileEffectsIo,
) -> RepositoryMaterializationAttempt {
    if !generated_repository_file_effect_paths_are_valid(plan) {
        return RepositoryMaterializationAttempt::SpecError(
            "invalid generated repository file-effect plan".into(),
        );
    }
    let source_identity = generated_repository_file_effect_source_association(plan);
    let root = match io.temporary_root() {
        Ok(root) => root,
        Err(error) => return RepositoryMaterializationAttempt::MaterializationError(error),
    };
    for effect in plan.effects() {
        let target = root.path().join(effect.path());
        if let Some(parent) = target.parent()
            && let Err(error) = io.create_parent(parent)
        {
            return RepositoryMaterializationAttempt::MaterializationError(error);
        }
        let mut file = match io.create_file(&target) {
            Ok(file) => file,
            Err(error) => return RepositoryMaterializationAttempt::MaterializationError(error),
        };
        if let Err(error) = io.write_file(&mut file, effect.content()) {
            return RepositoryMaterializationAttempt::MaterializationError(error);
        }
        if let Err(error) = io.flush_file(&mut file) {
            return RepositoryMaterializationAttempt::MaterializationError(error);
        }
        if let Err(error) = io.set_mode(&target, effect.executable()) {
            return RepositoryMaterializationAttempt::MaterializationError(error);
        }
    }
    RepositoryMaterializationAttempt::GeneratedImmutable {
        source_identity,
        root,
    }
}

fn materialize_native_attempt(
    workspace: &Path,
    request: &RepositoryMaterializationRequest,
) -> RepositoryMaterializationAttempt {
    if let RepositoryMaterializationKind::GeneratedFileEffects(plan) = &request.kind {
        return materialize_generated_repository_file_effects(
            plan,
            &mut NativeGeneratedRepositoryFileEffectsIo,
        );
    }
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
        ("@@bazel_tools//tools/build_defs/repo:http.bzl", "http_archive") => {
            materialized_attempt(materialize_archive_plan(&request.repo_spec))
        }
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
            RepositoryMaterializationKind::GeneratedFileEffects(_),
        )
        | (
            "@@bazel_tools//tools/build_defs/repo:http.bzl",
            "http_archive",
            RepositoryMaterializationKind::GeneratedFileEffects(_),
        )
        | (
            "@@bazel_tools//tools/build_defs/repo:git.bzl",
            "git_repository",
            RepositoryMaterializationKind::GeneratedFileEffects(_),
        ) => Err(RepositorySessionError::KindMismatch),
        (_, _, RepositoryMaterializationKind::GeneratedFileEffects(_)) => Ok(()),
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
    materialize_archive_plan(repo_spec).map_err(ArchiveMaterializationError::into_repository)
}

fn materialize_archive_plan(
    repo_spec: &RepoSpec,
) -> Result<Materialized, ArchiveMaterializationError> {
    match super::repository_archive::parse_archive_plan(repo_spec) {
        Ok(super::repository_archive::ArchivePlan::LocalTar) => {
            super::repository_archive::materialize_local_tar(repo_spec)
        }
        Ok(super::repository_archive::ArchivePlan::SelectedBcrTarGz(plan)) => {
            let _complete_plan = (
                plan.urls,
                plan.integrity,
                plan.module_url,
                plan.module_integrity,
            );
            Err(ArchiveMaterializationError::transport(
                "selected-registry BCR archive transport is deferred",
            ))
        }
        Err(error) => Err(ArchiveMaterializationError::spec(error)),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ArchiveFailureStage {
    Spec,
    Transport,
    Materialization,
}

#[derive(Debug)]
pub(super) struct ArchiveMaterializationError {
    pub(super) stage: ArchiveFailureStage,
    pub(super) message: String,
}

impl ArchiveMaterializationError {
    pub(super) fn new(stage: ArchiveFailureStage, message: impl Into<String>) -> Self {
        Self {
            stage,
            message: message.into(),
        }
    }

    pub(super) fn spec(message: impl Into<String>) -> Self {
        Self::new(ArchiveFailureStage::Spec, message)
    }

    pub(super) fn transport(message: impl Into<String>) -> Self {
        Self::new(ArchiveFailureStage::Transport, message)
    }

    pub(super) fn materialization(message: impl Into<String>) -> Self {
        Self::new(ArchiveFailureStage::Materialization, message)
    }

    fn into_repository(self) -> RepositoryTransportError {
        let _stage = self.stage;
        RepositoryTransportError {
            message: self.message.into(),
        }
    }
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

pub(super) fn local_file_uri(value: &str) -> Result<PathBuf, RepositoryTransportError> {
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

pub(super) fn required_string<'a>(
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

pub(super) fn optional_string<'a>(
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

pub(super) fn reject_extra_attributes(
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
    use std::path::Path;
    use std::path::PathBuf;
    use std::sync::Arc;

    use compact_str::CompactString;
    use sha2::Digest;
    use slug_bzlmod_v2::GeneratedRepositoryFileEffectPlan;
    use slug_bzlmod_v2::RepoRuleId;
    use slug_identity_v2::CanonicalLabel;
    use starlark_map::small_map::SmallMap;

    use super::*;

    fn generated_plan(
        effects: impl IntoIterator<Item = (&'static str, &'static [u8], bool)>,
    ) -> GeneratedRepositoryFileEffectPlan {
        GeneratedRepositoryFileEffectPlan::build(effects.into_iter().map(
            |(path, content, executable)| {
                (
                    CompactString::new(path),
                    Arc::<[u8]>::from(content),
                    executable,
                )
            },
        ))
        .unwrap()
    }

    #[derive(Clone, Copy)]
    enum ScriptedGeneratedFileEffectsFailure {
        Root,
        Parent,
        Create,
        Write,
        Flush,
        Mode,
    }

    struct ScriptedGeneratedFileEffectsIo {
        failure: ScriptedGeneratedFileEffectsFailure,
        root_path: Option<PathBuf>,
        creates: usize,
    }

    impl GeneratedRepositoryFileEffectsIo for ScriptedGeneratedFileEffectsIo {
        fn temporary_root(&mut self) -> Result<tempfile::TempDir, String> {
            if matches!(self.failure, ScriptedGeneratedFileEffectsFailure::Root) {
                return Err("root".into());
            }
            let root = tempfile::tempdir().map_err(|error| error.to_string())?;
            self.root_path = Some(root.path().to_path_buf());
            Ok(root)
        }

        fn create_parent(&mut self, _path: &Path) -> Result<(), String> {
            if matches!(self.failure, ScriptedGeneratedFileEffectsFailure::Parent) {
                Err("parent".into())
            } else {
                Ok(())
            }
        }

        fn create_file(&mut self, path: &Path) -> Result<File, String> {
            self.creates += 1;
            if matches!(self.failure, ScriptedGeneratedFileEffectsFailure::Create) {
                return Err("create".into());
            }
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
                .map_err(|error| error.to_string())
        }

        fn write_file(&mut self, _file: &mut File, _content: &[u8]) -> Result<(), String> {
            if matches!(self.failure, ScriptedGeneratedFileEffectsFailure::Write) {
                Err("write".into())
            } else {
                Ok(())
            }
        }

        fn flush_file(&mut self, _file: &mut File) -> Result<(), String> {
            if matches!(self.failure, ScriptedGeneratedFileEffectsFailure::Flush) {
                Err("flush".into())
            } else {
                Ok(())
            }
        }

        fn set_mode(&mut self, _path: &Path, _executable: bool) -> Result<(), String> {
            if matches!(self.failure, ScriptedGeneratedFileEffectsFailure::Mode) {
                Err("mode".into())
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn generated_file_effects_preflight_and_atomic_native_application_are_exact() {
        let plan = generated_plan([
            ("BUILD.bazel", b"exports_files([])\n" as &[u8], true),
            ("nested/data.txt", b"exact\0bytes" as &[u8], false),
        ]);
        let first = generated_repository_file_effect_source_association(&plan);
        let reordered = generated_plan([
            ("nested/data.txt", b"exact\0bytes" as &[u8], false),
            ("BUILD.bazel", b"exports_files([])\n" as &[u8], true),
        ]);
        assert_ne!(
            first,
            generated_repository_file_effect_source_association(&reordered)
        );
        let RepositoryMaterializationAttempt::GeneratedImmutable {
            source_identity,
            root,
        } = materialize_generated_repository_file_effects(
            &plan,
            &mut NativeGeneratedRepositoryFileEffectsIo,
        )
        else {
            panic!("native generated plan must materialize immutably");
        };
        assert_eq!(source_identity, first);
        assert_eq!(
            std::fs::read(root.path().join("BUILD.bazel")).unwrap(),
            b"exports_files([])\n"
        );
        assert_eq!(
            std::fs::read(root.path().join("nested/data.txt")).unwrap(),
            b"exact\0bytes"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            assert_eq!(
                std::fs::metadata(root.path().join("BUILD.bazel"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o755
            );
            assert_eq!(
                std::fs::metadata(root.path().join("nested/data.txt"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o644
            );
        }

        let collision = generated_plan([
            ("a", b"one" as &[u8], true),
            ("a/b", b"two" as &[u8], false),
        ]);
        let mut io = ScriptedGeneratedFileEffectsIo {
            failure: ScriptedGeneratedFileEffectsFailure::Create,
            root_path: None,
            creates: 0,
        };
        assert!(matches!(
            materialize_generated_repository_file_effects(&collision, &mut io),
            RepositoryMaterializationAttempt::SpecError(_)
        ));
        assert!(io.root_path.is_none());
        assert_eq!(io.creates, 0);

        for failure in [
            ScriptedGeneratedFileEffectsFailure::Root,
            ScriptedGeneratedFileEffectsFailure::Parent,
            ScriptedGeneratedFileEffectsFailure::Create,
            ScriptedGeneratedFileEffectsFailure::Write,
            ScriptedGeneratedFileEffectsFailure::Flush,
            ScriptedGeneratedFileEffectsFailure::Mode,
        ] {
            let mut io = ScriptedGeneratedFileEffectsIo {
                failure,
                root_path: None,
                creates: 0,
            };
            assert!(matches!(
                materialize_generated_repository_file_effects(&plan, &mut io),
                RepositoryMaterializationAttempt::MaterializationError(_)
            ));
            if let Some(path) = io.root_path {
                assert!(!path.exists());
            }
        }
    }

    #[test]
    fn generated_file_effect_kind_is_custom_rule_only_and_nonposix_mode_fails_closed() {
        let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
        let plan = generated_plan([("BUILD.bazel", b"x" as &[u8], true)]);
        for spec in [
            local_spec("repo"),
            archive_spec("https://example.invalid/archive.tar".into(), "0".repeat(64)),
            git_spec("https://example.invalid/repo.git".into(), "0".repeat(40)),
        ] {
            let request = native_request(
                &workspace,
                "generated",
                spec,
                RepositoryMaterializationKind::GeneratedFileEffects(plan.clone()),
            );
            assert_eq!(
                validate_native_request(&workspace, &request),
                Err(RepositorySessionError::KindMismatch)
            );
        }
        let custom = RepoSpec {
            rule_id: RepoRuleId {
                bzl_file: CanonicalLabel::parse("@@extension+repo//:defs.bzl").unwrap(),
                rule_name: "generated_repository".into(),
            },
            attributes: Arc::default(),
        };
        let request = native_request(
            &workspace,
            "generated",
            custom,
            RepositoryMaterializationKind::GeneratedFileEffects(plan),
        );
        assert_eq!(validate_native_request(&workspace, &request), Ok(()));

        let source = include_str!("repository_io.rs");
        let production = source.split_once("\n#[cfg(test)]\nmod tests {").unwrap().0;
        let mode = &production[production
            .find("fn set_mode(&mut self, path: &Path, executable: bool)")
            .unwrap()
            ..production
                .find("fn generated_repository_file_effect_source_association")
                .unwrap()];
        assert!(mode.contains("#[cfg(not(unix))]"));
        assert!(mode.contains(
            "Err(\"generated repository executable mode is unsupported on this platform\".into())"
        ));
    }

    #[test]
    fn generated_file_effect_sessions_conflict_replace_reuse_restore_and_discard_post_io() {
        let workspace_root = tempfile::tempdir().unwrap();
        let workspace = NormalizedAbsolutePath::new(workspace_root.path().to_path_buf()).unwrap();
        let materializer = RepositoryMaterializer::new(workspace.clone());
        let spec = || RepoSpec {
            rule_id: RepoRuleId {
                bzl_file: CanonicalLabel::parse("@@extension+repo//:defs.bzl").unwrap(),
                rule_name: "generated_repository".into(),
            },
            attributes: Arc::default(),
        };
        let request = |repo: &str, content: &'static [u8]| {
            native_request(
                &workspace,
                repo,
                spec(),
                RepositoryMaterializationKind::GeneratedFileEffects(generated_plan([(
                    "BUILD.bazel",
                    content,
                    true,
                )])),
            )
        };
        let a = request("generated", b"a");
        let b = request("generated", b"b");
        let a_restore = request("generated", b"a");
        let sibling = request("sibling", b"sibling");

        let token = begin_empty(&materializer);
        materializer
            .materialize_native(token, a.clone(), RepositoryMaterializationGeneration(1))
            .unwrap();
        let (a_root, a_instance) = immutable_result(&materializer, "generated");
        materializer
            .materialize_native(token, sibling, RepositoryMaterializationGeneration(1))
            .unwrap();
        let (sibling_root, _) = immutable_result(&materializer, "sibling");
        assert_eq!(
            materializer
                .materialize_native(token, b.clone(), RepositoryMaterializationGeneration(1))
                .unwrap_err(),
            RepositorySessionError::ConflictingRequest(
                CanonicalRepoName::new("generated").unwrap()
            )
        );
        materializer
            .accept(token, std::slice::from_ref(&a), Vec::new())
            .unwrap();
        assert_eq!(materializer.state.lock().unwrap().accepted_roots.len(), 1);
        assert!(a_root.exists());
        assert!(!sibling_root.exists());

        let token = materializer.begin().unwrap();
        materializer
            .preflight_native(token, std::iter::empty())
            .unwrap();
        let before_reuse = materializer.state.lock().unwrap().next_instance;
        materializer
            .materialize_native(token, a.clone(), RepositoryMaterializationGeneration(2))
            .unwrap();
        assert_eq!(
            materializer.state.lock().unwrap().next_instance,
            before_reuse
        );
        materializer.discard(token).unwrap();

        let token = begin_empty(&materializer);
        materializer
            .materialize_native(token, b.clone(), RepositoryMaterializationGeneration(3))
            .unwrap();
        let (b_root, b_instance) = immutable_result(&materializer, "generated");
        assert_ne!(a_instance, b_instance);
        materializer
            .accept(token, std::slice::from_ref(&b), Vec::new())
            .unwrap();
        assert!(b_root.exists());
        assert!(a_root.exists());
        {
            let state = materializer.state.lock().unwrap();
            assert_eq!(state.accepted.len(), 1);
            assert_eq!(*state.accepted[0].request, *b);
            let RepositoryMaterializationSuccess::Immutable {
                source_identity,
                generation_root,
                ..
            } = &state.accepted[0].success
            else {
                panic!("generated acceptance must retain an immutable root")
            };
            assert_eq!(generation_root, &b_root);
            assert_eq!(
                source_identity,
                &generated_repository_file_effect_source_association(&generated_plan([(
                    "BUILD.bazel",
                    b"b" as &[u8],
                    true
                )]),)
            );
        }
        assert_eq!(std::fs::read(b_root.join("BUILD.bazel")).unwrap(), b"b");

        let token = begin_empty(&materializer);
        materializer
            .materialize_native(
                token,
                a_restore.clone(),
                RepositoryMaterializationGeneration(4),
            )
            .unwrap();
        let (restored_root, restored_instance) = immutable_result(&materializer, "generated");
        materializer
            .accept(token, std::slice::from_ref(&a_restore), Vec::new())
            .unwrap();
        assert_ne!(restored_instance, b_instance);
        assert!(restored_root.exists());
        assert!(b_root.exists());
        {
            let state = materializer.state.lock().unwrap();
            assert_eq!(state.accepted.len(), 1);
            assert_eq!(*state.accepted[0].request, *a_restore);
            let RepositoryMaterializationSuccess::Immutable {
                source_identity,
                generation_root,
                ..
            } = &state.accepted[0].success
            else {
                panic!("restored generated acceptance must retain an immutable root")
            };
            assert_eq!(generation_root, &restored_root);
            assert_eq!(
                source_identity,
                &generated_repository_file_effect_source_association(&generated_plan([(
                    "BUILD.bazel",
                    b"a" as &[u8],
                    true
                )]),)
            );
        }
        assert_eq!(
            std::fs::read(restored_root.join("BUILD.bazel")).unwrap(),
            b"a"
        );

        let token = begin_empty(&materializer);
        let stale_request = request("generated", b"stale");
        let mut provisional = None;
        let error = materializer
            .materialize_with(
                token,
                stale_request,
                RepositoryMaterializationGeneration(5),
                || {
                    let root = tempfile::tempdir().unwrap();
                    provisional = Some(root.path().to_path_buf());
                    materializer.discard(token).unwrap();
                    RepositoryMaterializationAttempt::GeneratedImmutable {
                        source_identity: generated_repository_file_effect_source_association(
                            &generated_plan([("BUILD.bazel", b"a" as &[u8], true)]),
                        ),
                        root,
                    }
                },
            )
            .unwrap_err();
        assert!(
            matches!(error, RepositorySessionError::StaleToken { active: None, supplied } if supplied == token)
        );
        assert!(!provisional.unwrap().exists());
        assert!(restored_root.exists());
    }

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
