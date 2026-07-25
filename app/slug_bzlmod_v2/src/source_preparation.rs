/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License found in the LICENSE-APACHE file in the root directory of this
 * source tree. You may select the license that applies to you.
 */

use std::fmt;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use compact_str::CompactString;
use dice::DiceComputations;
use dice::DiceDataBuilder;
use dice::InjectedKey;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use sha2::Digest;
use sha2::Sha256;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::CanonicalRepoName;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathLstat;
use slug_workspace_v2::PathNodeKind;
use slug_workspace_v2::PathObservationDemand;
use slug_workspace_v2::PathObservationError;
use slug_workspace_v2::PathObservationKey;
use slug_workspace_v2::PathObservationNamespace;
use slug_workspace_v2::PathObservationOperation;
use slug_workspace_v2::PathObservationResult;
use slug_workspace_v2::PathOperationResult;
use slug_workspace_v2::PathOutcome;
use slug_workspace_v2::PathResolutionError;
use slug_workspace_v2::PathResult;
use slug_workspace_v2::ResolvedPathKey;
use slug_workspace_v2::ResolvedPathState;

use crate::ModuleKey;
use crate::RegistryBaseUrl;
use crate::RegistryFileError;
use crate::RegistryFileKey;
use crate::RegistryFileUrl;
use crate::RegistryFileValue;
use crate::RegistryPolicyKey;
use crate::RepoSpec;
use crate::RootModuleFilesKey;
use crate::RootModuleOverride;
use crate::apply_unified_patch;
use crate::registry_module_file_url;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct RepositoryMaterializationKey {
    pub workspace: PathBuf,
    pub module_name: CompactString,
}

/// Prepares one module's raw MODULE.bazel bytes. `version` is already the
/// effective version chosen by the upstream owner; this key never resolves or
/// rewrites it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct ModuleSourcePreparationKey {
    pub workspace: PathBuf,
    pub module_name: CompactString,
    pub version: CompactString,
}

impl fmt::Display for ModuleSourcePreparationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "module-source-preparation:{}@{}",
            self.module_name, self.version
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum ModuleSourcePreparation {
    NonRegistry {
        bytes: Arc<[u8]>,
    },
    Registry {
        bytes: Arc<[u8]>,
        selected_registry: RegistryBaseUrl,
        module_file_attempts: Arc<[RegistryModuleFileAttempt]>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct RegistryModuleFileAttempt {
    pub url: RegistryFileUrl,
    pub sha256: Option<[u8; 32]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum ModuleSourcePreparationError {
    RootModuleFiles(CompactString),
    RegistryPolicy(RegistryFileError),
    RegistryFileCompute {
        url: RegistryFileUrl,
        prior_not_found_attempts: Arc<[RegistryModuleFileAttempt]>,
        message: CompactString,
    },
    RegistryFile {
        url: RegistryFileUrl,
        prior_not_found_attempts: Arc<[RegistryModuleFileAttempt]>,
        error: RegistryFileError,
    },
    RegistryPolicyCompute(CompactString),
    Source(RepositorySourceFileError),
    SourceCompute(Arc<str>),
    InvalidPatchPath {
        path: PathBuf,
    },
    PatchMissing {
        logical_path: NormalizedAbsolutePath,
    },
    PatchWrongKind {
        logical_path: NormalizedAbsolutePath,
        actual: PathNodeKind,
    },
    PatchResolution(PathResolutionError),
    PatchResolutionCompute {
        logical_path: NormalizedAbsolutePath,
        message: CompactString,
    },
    PatchFileObservation {
        demand: PathObservationDemand,
        error: PathObservationError,
    },
    PatchFileInconsistentState {
        demand: PathObservationDemand,
        before: Option<PathLstat>,
        after: Option<PathLstat>,
    },
    PatchFileCompute {
        demand: PathObservationDemand,
        message: CompactString,
    },
    Patch(CompactString),
    MissingVersion,
    ModuleNotFound {
        module_file_attempts: Arc<[RegistryModuleFileAttempt]>,
    },
}

impl fmt::Display for RepositoryMaterializationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "repository-materialization:{}", self.module_name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct RepositorySourceFileKey {
    pub workspace: PathBuf,
    pub module_name: CompactString,
    pub repo_relative_path: PathBuf,
}

impl fmt::Display for RepositorySourceFileKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "repository-source-file:{}:{}",
            self.module_name,
            self.repo_relative_path.display()
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Allocative, Dupe)]
pub struct RepositoryMaterializationGeneration(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct RepositoryMaterializationGenerationKey {
    pub workspace: PathBuf,
}

impl fmt::Display for RepositoryMaterializationGenerationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "repository-materialization-generation:{}",
            self.workspace.display()
        )
    }
}

impl InjectedKey for RepositoryMaterializationGenerationKey {
    type Value = RepositoryMaterializationGeneration;

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum RepositoryIoOutcome {
    Local {
        source_root: PathBuf,
    },
    Immutable {
        source_identity: Arc<str>,
        generation_root: PathBuf,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct RepositoryTransportError {
    pub message: CompactString,
}

#[async_trait]
pub trait RepositoryIo: Send + Sync + 'static {
    async fn materialize(
        &self,
        workspace: &Path,
        repo_spec: &RepoSpec,
    ) -> Result<RepositoryIoOutcome, RepositoryTransportError>;
}

struct RepositoryIoHandle(Arc<dyn RepositoryIo>);

pub fn install_repository_io(builder: &mut DiceDataBuilder, io: Arc<dyn RepositoryIo>) {
    builder.set(RepositoryIoHandle(io));
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum RepositoryMaterialization {
    Local {
        canonical_repo: CanonicalRepoName,
        repo_spec: RepoSpec,
        source_root: PathBuf,
    },
    Immutable {
        canonical_repo: CanonicalRepoName,
        repo_spec: RepoSpec,
        source_identity: Arc<str>,
        generation_root: PathBuf,
    },
}

impl RepositoryMaterialization {
    fn equivalent_to(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::Local {
                    canonical_repo: left_repo,
                    repo_spec: left_spec,
                    source_root: left_root,
                },
                Self::Local {
                    canonical_repo: right_repo,
                    repo_spec: right_spec,
                    source_root: right_root,
                },
            ) => left_repo == right_repo && left_spec == right_spec && left_root == right_root,
            (
                Self::Immutable {
                    canonical_repo: left_repo,
                    repo_spec: left_spec,
                    source_identity: left_identity,
                    ..
                },
                Self::Immutable {
                    canonical_repo: right_repo,
                    repo_spec: right_spec,
                    source_identity: right_identity,
                    ..
                },
            ) => {
                left_repo == right_repo
                    && left_spec == right_spec
                    && left_identity == right_identity
            }
            _ => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum RepositoryMaterializationError {
    RootModuleFiles(CompactString),
    MissingOverride(CompactString),
    UnsupportedOverride(CompactString),
    InvalidCanonicalRepository(CompactString),
    MissingIoCapability,
    MissingGeneration(CompactString),
    Transport(CompactString),
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub enum RepositorySourceFileValue {
    Present(Arc<[u8]>),
    Absent,
}

/// Semantic failure from reading a repository source file. Operational resolver
/// paths, namespaces, and symlink provenance deliberately remain below this
/// DICE boundary.
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub enum RepositorySourceFileError {
    InvalidRepoRelativePath {
        requested_path: Arc<PathBuf>,
    },
    MaterializationCompute {
        repo_relative_path: Arc<PathBuf>,
        message: Arc<str>,
    },
    Materialization {
        repo_relative_path: Arc<PathBuf>,
        error: Arc<RepositoryMaterializationError>,
    },
    MaterializationGenerationCompute {
        repo_relative_path: Arc<PathBuf>,
        message: Arc<str>,
    },
    InvalidMaterializedPath {
        repo_relative_path: Arc<PathBuf>,
    },
    Observation {
        repo_relative_path: Arc<PathBuf>,
        operation: PathObservationOperation,
        error: PathObservationError,
    },
    InconsistentState {
        repo_relative_path: Arc<PathBuf>,
        operation: PathObservationOperation,
        before: Option<PathLstat>,
        after: Option<PathLstat>,
    },
    WrongKind {
        repo_relative_path: Arc<PathBuf>,
        actual: PathNodeKind,
    },
    Cycle {
        repo_relative_path: Arc<PathBuf>,
    },
    InfiniteExpansion {
        repo_relative_path: Arc<PathBuf>,
    },
    ResolutionCompute {
        repo_relative_path: Arc<PathBuf>,
        message: Arc<str>,
    },
    FileCompute {
        repo_relative_path: Arc<PathBuf>,
        message: Arc<str>,
    },
    ImmutableRead {
        repo_relative_path: Arc<PathBuf>,
        message: Arc<str>,
    },
}

#[async_trait]
impl Key for RepositoryMaterializationKey {
    type Value = Arc<Result<RepositoryMaterialization, RepositoryMaterializationError>>;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let root_files = match ctx
            .compute(&RootModuleFilesKey {
                workspace: self.workspace.clone(),
            })
            .await
        {
            Ok(root_files) => root_files,
            Err(error) => {
                return Arc::new(Err(RepositoryMaterializationError::RootModuleFiles(
                    error.to_string().into(),
                )));
            }
        };
        let root_files = match root_files.as_ref() {
            Ok(root_files) => root_files,
            Err(error) => {
                return Arc::new(Err(RepositoryMaterializationError::RootModuleFiles(
                    error.clone(),
                )));
            }
        };
        let repo_spec = match root_files.overrides.get(self.module_name.as_str()) {
            Some(RootModuleOverride::NonRegistry(repo_spec)) => repo_spec.clone(),
            Some(_) => {
                return Arc::new(Err(RepositoryMaterializationError::UnsupportedOverride(
                    format!(
                        "module {} does not have a non-registry override",
                        self.module_name
                    )
                    .into(),
                )));
            }
            None => {
                return Arc::new(Err(RepositoryMaterializationError::MissingOverride(
                    self.module_name.clone(),
                )));
            }
        };
        let canonical_repo = match CanonicalRepoName::new(format!("{}+", self.module_name)) {
            Ok(repo) => repo,
            Err(error) => {
                return Arc::new(Err(
                    RepositoryMaterializationError::InvalidCanonicalRepository(error.into()),
                ));
            }
        };
        let io = match ctx.global_data().get::<RepositoryIoHandle>() {
            Ok(handle) => handle.0.clone(),
            Err(_) => return Arc::new(Err(RepositoryMaterializationError::MissingIoCapability)),
        };
        let result = match io.materialize(&self.workspace, &repo_spec).await {
            Ok(RepositoryIoOutcome::Local { source_root }) => {
                Ok(RepositoryMaterialization::Local {
                    canonical_repo,
                    repo_spec,
                    source_root,
                })
            }
            Ok(RepositoryIoOutcome::Immutable {
                source_identity,
                generation_root,
            }) => Ok(RepositoryMaterialization::Immutable {
                canonical_repo,
                repo_spec,
                source_identity,
                generation_root,
            }),
            Err(error) => Err(RepositoryMaterializationError::Transport(error.message)),
        };
        if result.is_err() {
            if let Err(error) = ctx
                .compute(&RepositoryMaterializationGenerationKey {
                    workspace: self.workspace.clone(),
                })
                .await
            {
                return Arc::new(Err(RepositoryMaterializationError::MissingGeneration(
                    error.to_string().into(),
                )));
            }
        }
        Arc::new(result)
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x.as_ref(), y.as_ref()) {
            (Ok(left), Ok(right)) => left.equivalent_to(right),
            (Err(left), Err(right)) => left == right,
            _ => false,
        }
    }
}

fn project_resolution_error(
    repo_relative_path: Arc<PathBuf>,
    error: PathResolutionError,
) -> RepositorySourceFileError {
    match error {
        PathResolutionError::Observation { demand, error, .. } => {
            RepositorySourceFileError::Observation {
                repo_relative_path,
                operation: demand.operation(),
                error,
            }
        }
        PathResolutionError::InconsistentState {
            demand,
            before,
            after,
            ..
        } => RepositorySourceFileError::InconsistentState {
            repo_relative_path,
            operation: demand.operation(),
            before,
            after,
        },
        PathResolutionError::Cycle { .. } => {
            RepositorySourceFileError::Cycle { repo_relative_path }
        }
        PathResolutionError::InfiniteExpansion { .. } => {
            RepositorySourceFileError::InfiniteExpansion { repo_relative_path }
        }
    }
}

#[async_trait]
impl Key for RepositorySourceFileKey {
    type Value = PathResult<RepositorySourceFileValue, RepositorySourceFileError>;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let relative = match checked_relative_path(&self.repo_relative_path) {
            Ok(relative) => relative,
            Err(_) => {
                return PathOutcome::Complete(Err(
                    RepositorySourceFileError::InvalidRepoRelativePath {
                        requested_path: Arc::new(self.repo_relative_path.clone()),
                    },
                ));
            }
        };
        let repo_relative_path = Arc::new(relative.to_owned());
        let materialization = match ctx
            .compute(&RepositoryMaterializationKey {
                workspace: self.workspace.clone(),
                module_name: self.module_name.clone(),
            })
            .await
        {
            Ok(value) => value,
            Err(error) => {
                return PathOutcome::Complete(Err(
                    RepositorySourceFileError::MaterializationCompute {
                        repo_relative_path,
                        message: Arc::from(error.to_string()),
                    },
                ));
            }
        };
        let materialization = match materialization.as_ref() {
            Ok(value) => value,
            Err(error) => {
                if let Err(generation_error) = ctx
                    .compute(&RepositoryMaterializationGenerationKey {
                        workspace: self.workspace.clone(),
                    })
                    .await
                {
                    return PathOutcome::Complete(Err(
                        RepositorySourceFileError::MaterializationGenerationCompute {
                            repo_relative_path,
                            message: Arc::from(generation_error.to_string()),
                        },
                    ));
                }
                return PathOutcome::Complete(Err(RepositorySourceFileError::Materialization {
                    repo_relative_path,
                    error: Arc::new(error.clone()),
                }));
            }
        };
        match materialization {
            RepositoryMaterialization::Local { source_root, .. } => {
                let logical_path = match NormalizedAbsolutePath::new(source_root.join(relative)) {
                    Ok(path) => path,
                    Err(_) => {
                        return PathOutcome::Complete(Err(
                            RepositorySourceFileError::InvalidMaterializedPath {
                                repo_relative_path,
                            },
                        ));
                    }
                };
                let resolved = match ctx
                    .compute(&ResolvedPathKey::new(
                        PathObservationNamespace::Host,
                        logical_path,
                    ))
                    .await
                {
                    Ok(PathOutcome::Need(need)) => return PathOutcome::Need(need),
                    Ok(PathOutcome::Complete(Err(error))) => {
                        return PathOutcome::Complete(Err(project_resolution_error(
                            repo_relative_path,
                            error,
                        )));
                    }
                    Ok(PathOutcome::Complete(Ok(resolved))) => resolved,
                    Err(error) => {
                        return PathOutcome::Complete(Err(
                            RepositorySourceFileError::ResolutionCompute {
                                repo_relative_path,
                                message: Arc::from(error.to_string()),
                            },
                        ));
                    }
                };
                let lstat = match resolved.state() {
                    ResolvedPathState::Missing => {
                        return PathOutcome::Complete(Ok(RepositorySourceFileValue::Absent));
                    }
                    ResolvedPathState::Present(lstat)
                        if matches!(
                            lstat.kind(),
                            PathNodeKind::RegularFile | PathNodeKind::SpecialFile
                        ) =>
                    {
                        lstat
                    }
                    ResolvedPathState::Present(lstat) => {
                        return PathOutcome::Complete(Err(RepositorySourceFileError::WrongKind {
                            repo_relative_path,
                            actual: lstat.kind(),
                        }));
                    }
                };
                let demand = PathObservationDemand::new(
                    PathObservationNamespace::Host,
                    resolved.real_path().dupe(),
                    PathObservationOperation::FileBytes,
                );
                let observed = match ctx.compute(&PathObservationKey::new(demand)).await {
                    Ok(PathOutcome::Need(need)) => return PathOutcome::Need(need),
                    Ok(PathOutcome::Complete(result)) => result,
                    Err(error) => {
                        return PathOutcome::Complete(Err(
                            RepositorySourceFileError::FileCompute {
                                repo_relative_path,
                                message: Arc::from(error.to_string()),
                            },
                        ));
                    }
                };
                match observed.as_ref() {
                    PathObservationResult::FileBytes(PathOperationResult::Present(bytes)) => {
                        PathOutcome::Complete(Ok(RepositorySourceFileValue::Present(bytes.dupe())))
                    }
                    PathObservationResult::FileBytes(PathOperationResult::Missing) => {
                        PathOutcome::Complete(Err(RepositorySourceFileError::InconsistentState {
                            repo_relative_path,
                            operation: PathObservationOperation::FileBytes,
                            before: Some(lstat),
                            after: None,
                        }))
                    }
                    PathObservationResult::FileBytes(PathOperationResult::Error(error)) => {
                        PathOutcome::Complete(Err(RepositorySourceFileError::Observation {
                            repo_relative_path,
                            operation: PathObservationOperation::FileBytes,
                            error: *error,
                        }))
                    }
                    PathObservationResult::Lstat(_)
                    | PathObservationResult::ReadLink(_)
                    | PathObservationResult::DirectoryEntries(_) => {
                        unreachable!("FileBytes demand must return a FileBytes observation")
                    }
                }
            }
            RepositoryMaterialization::Immutable {
                generation_root, ..
            } => match std::fs::read(generation_root.join(relative)) {
                Ok(bytes) => {
                    PathOutcome::Complete(Ok(RepositorySourceFileValue::Present(Arc::from(bytes))))
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    PathOutcome::Complete(Ok(RepositorySourceFileValue::Absent))
                }
                Err(error) => {
                    PathOutcome::Complete(Err(RepositorySourceFileError::ImmutableRead {
                        repo_relative_path,
                        message: Arc::from(error.to_string()),
                    }))
                }
            },
        }
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[async_trait]
impl Key for ModuleSourcePreparationKey {
    type Value = PathOutcome<Arc<Result<ModuleSourcePreparation, ModuleSourcePreparationError>>>;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let root = match ctx
            .compute(&RootModuleFilesKey {
                workspace: self.workspace.clone(),
            })
            .await
        {
            Ok(value) => value,
            Err(error) => {
                return PathOutcome::Complete(Arc::new(Err(
                    ModuleSourcePreparationError::RootModuleFiles(error.to_string().into()),
                )));
            }
        };
        let root = match root.as_ref() {
            Ok(value) => value,
            Err(error) => {
                return PathOutcome::Complete(Arc::new(Err(
                    ModuleSourcePreparationError::RootModuleFiles(error.clone()),
                )));
            }
        };
        let override_ = root.overrides.get(self.module_name.as_str()).cloned();
        if matches!(override_, Some(RootModuleOverride::NonRegistry(_))) {
            let value = match ctx
                .compute(&RepositorySourceFileKey {
                    workspace: self.workspace.clone(),
                    module_name: self.module_name.clone(),
                    repo_relative_path: PathBuf::from("MODULE.bazel"),
                })
                .await
            {
                Ok(PathOutcome::Need(need)) => return PathOutcome::Need(need),
                Ok(PathOutcome::Complete(Ok(RepositorySourceFileValue::Present(bytes)))) => {
                    Ok(ModuleSourcePreparation::NonRegistry { bytes })
                }
                Ok(PathOutcome::Complete(Ok(RepositorySourceFileValue::Absent))) => {
                    Err(ModuleSourcePreparationError::ModuleNotFound {
                        module_file_attempts: Arc::from([]),
                    })
                }
                Ok(PathOutcome::Complete(Err(error))) => {
                    Err(ModuleSourcePreparationError::Source(error))
                }
                Err(error) => Err(ModuleSourcePreparationError::SourceCompute(Arc::from(
                    error.to_string(),
                ))),
            };
            return PathOutcome::Complete(Arc::new(value));
        }
        if self.version.is_empty() {
            return PathOutcome::Complete(Arc::new(Err(
                ModuleSourcePreparationError::MissingVersion,
            )));
        }
        let policy = match ctx
            .compute(&RegistryPolicyKey {
                workspace: self.workspace.clone(),
            })
            .await
        {
            Ok(value) => value,
            Err(error) => {
                return PathOutcome::Complete(Arc::new(Err(
                    ModuleSourcePreparationError::RegistryPolicyCompute(error.to_string().into()),
                )));
            }
        };
        let policy = match policy.as_ref() {
            Ok(value) => value,
            Err(error) => {
                return PathOutcome::Complete(Arc::new(Err(
                    ModuleSourcePreparationError::RegistryPolicy(error.clone()),
                )));
            }
        };
        let override_registry = match override_.as_ref() {
            Some(RootModuleOverride::RegistrySingle(value)) if !value.registry.is_empty() => {
                Some(value.registry.as_str())
            }
            Some(RootModuleOverride::RegistryMultiple(value)) if !value.registry.is_empty() => {
                Some(value.registry.as_str())
            }
            _ => None,
        };
        let module = ModuleKey::new(self.module_name.as_str(), self.version.as_str());
        if let Some(registry) = override_registry {
            let mut attempts = Vec::new();
            return match self
                .prepare_from_registry(ctx, override_.as_ref(), registry, &module, &mut attempts)
                .await
            {
                PathOutcome::Need(need) => PathOutcome::Need(need),
                PathOutcome::Complete(result) => PathOutcome::Complete(Arc::new(match result {
                    Ok(Some(value)) => Ok(value),
                    Ok(None) => Err(ModuleSourcePreparationError::ModuleNotFound {
                        module_file_attempts: Arc::from(attempts),
                    }),
                    Err(error) => Err(error),
                })),
            };
        }
        let mut attempts = Vec::new();
        for registry in policy.urls().as_slice() {
            match self
                .prepare_from_registry(
                    ctx,
                    override_.as_ref(),
                    registry.as_str(),
                    &module,
                    &mut attempts,
                )
                .await
            {
                PathOutcome::Need(need) => return PathOutcome::Need(need),
                PathOutcome::Complete(Ok(Some(value))) => {
                    return PathOutcome::Complete(Arc::new(Ok(value)));
                }
                PathOutcome::Complete(Ok(None)) => {}
                PathOutcome::Complete(Err(error)) => {
                    return PathOutcome::Complete(Arc::new(Err(error)));
                }
            }
        }
        PathOutcome::Complete(Arc::new(Err(
            ModuleSourcePreparationError::ModuleNotFound {
                module_file_attempts: Arc::from(attempts),
            },
        )))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

impl ModuleSourcePreparationKey {
    async fn prepare_from_registry(
        &self,
        ctx: &mut DiceComputations<'_>,
        override_: Option<&RootModuleOverride>,
        registry: &str,
        module: &ModuleKey,
        attempts: &mut Vec<RegistryModuleFileAttempt>,
    ) -> PathOutcome<Result<Option<ModuleSourcePreparation>, ModuleSourcePreparationError>> {
        let url = RegistryFileUrl::new(registry_module_file_url(registry, module));
        let file = match ctx
            .compute(&RegistryFileKey {
                workspace: self.workspace.clone(),
                url: url.clone(),
            })
            .await
        {
            Ok(file) => file,
            Err(error) => {
                return PathOutcome::Complete(Err(
                    ModuleSourcePreparationError::RegistryFileCompute {
                        url: url.clone(),
                        prior_not_found_attempts: Arc::from(attempts.as_slice()),
                        message: error.to_string().into(),
                    },
                ));
            }
        };
        match file.as_ref() {
            Ok(RegistryFileValue::NotFound { .. }) => {
                attempts.push(RegistryModuleFileAttempt { url, sha256: None });
                PathOutcome::Complete(Ok(None))
            }
            Ok(RegistryFileValue::Found { bytes, sha256, .. }) => {
                let selected_registry = RegistryBaseUrl::new(registry);
                let bytes = match self.apply_root_patches(ctx, override_, bytes.clone()).await {
                    PathOutcome::Need(need) => return PathOutcome::Need(need),
                    PathOutcome::Complete(Ok(bytes)) => bytes,
                    PathOutcome::Complete(Err(error)) => return PathOutcome::Complete(Err(error)),
                };
                attempts.push(RegistryModuleFileAttempt {
                    url,
                    sha256: Some(*sha256),
                });
                PathOutcome::Complete(Ok(Some(ModuleSourcePreparation::Registry {
                    bytes,
                    selected_registry,
                    module_file_attempts: Arc::from(attempts.as_slice()),
                })))
            }
            Err(error) => PathOutcome::Complete(Err(ModuleSourcePreparationError::RegistryFile {
                url,
                prior_not_found_attempts: Arc::from(attempts.as_slice()),
                error: error.clone(),
            })),
        }
    }

    async fn apply_root_patches(
        &self,
        ctx: &mut DiceComputations<'_>,
        override_: Option<&RootModuleOverride>,
        mut bytes: Arc<[u8]>,
    ) -> PathOutcome<Result<Arc<[u8]>, ModuleSourcePreparationError>> {
        let Some(RootModuleOverride::RegistrySingle(override_)) = override_ else {
            return PathOutcome::Complete(Ok(bytes));
        };
        // PatchUtil filters this list to main-repository labels. `patch_cmds`
        // are deliberately inactive for module-file patching.
        let mut patches = Vec::new();
        for label in override_.patches.iter() {
            let Some(path) = main_repo_patch_path(label) else {
                continue;
            };
            let logical_path = match NormalizedAbsolutePath::new(self.workspace.join(path)) {
                Ok(path) => path,
                Err(error) => {
                    return PathOutcome::Complete(Err(
                        ModuleSourcePreparationError::InvalidPatchPath {
                            path: error.path().to_owned(),
                        },
                    ));
                }
            };
            let resolved = match ctx
                .compute(&ResolvedPathKey::new(
                    PathObservationNamespace::Host,
                    logical_path.dupe(),
                ))
                .await
            {
                Ok(PathOutcome::Need(need)) => return PathOutcome::Need(need),
                Ok(PathOutcome::Complete(Err(error))) => {
                    return PathOutcome::Complete(Err(
                        ModuleSourcePreparationError::PatchResolution(error),
                    ));
                }
                Ok(PathOutcome::Complete(Ok(resolved))) => resolved,
                Err(error) => {
                    return PathOutcome::Complete(Err(
                        ModuleSourcePreparationError::PatchResolutionCompute {
                            logical_path,
                            message: error.to_string().into(),
                        },
                    ));
                }
            };
            match resolved.state() {
                ResolvedPathState::Missing => {
                    return PathOutcome::Complete(Err(
                        ModuleSourcePreparationError::PatchMissing { logical_path },
                    ));
                }
                ResolvedPathState::Present(lstat)
                    if matches!(
                        lstat.kind(),
                        PathNodeKind::RegularFile | PathNodeKind::SpecialFile
                    ) =>
                {
                    patches.push((logical_path, resolved));
                }
                ResolvedPathState::Present(lstat) => {
                    return PathOutcome::Complete(Err(
                        ModuleSourcePreparationError::PatchWrongKind {
                            logical_path,
                            actual: lstat.kind(),
                        },
                    ));
                }
            }
        }

        for (_logical_path, resolved) in patches {
            let demand = PathObservationDemand::new(
                PathObservationNamespace::Host,
                resolved.real_path().dupe(),
                PathObservationOperation::FileBytes,
            );
            let observed = match ctx.compute(&PathObservationKey::new(demand.dupe())).await {
                Ok(PathOutcome::Need(need)) => return PathOutcome::Need(need),
                Ok(PathOutcome::Complete(result)) => result,
                Err(error) => {
                    return PathOutcome::Complete(Err(
                        ModuleSourcePreparationError::PatchFileCompute {
                            demand,
                            message: error.to_string().into(),
                        },
                    ));
                }
            };
            let patch = match observed.as_ref() {
                PathObservationResult::FileBytes(PathOperationResult::Present(bytes)) => {
                    bytes.dupe()
                }
                PathObservationResult::FileBytes(PathOperationResult::Missing) => {
                    let before = match resolved.state() {
                        ResolvedPathState::Present(lstat) => Some(lstat),
                        ResolvedPathState::Missing => None,
                    };
                    return PathOutcome::Complete(Err(
                        ModuleSourcePreparationError::PatchFileInconsistentState {
                            demand,
                            before,
                            after: None,
                        },
                    ));
                }
                PathObservationResult::FileBytes(PathOperationResult::Error(error)) => {
                    return PathOutcome::Complete(Err(
                        ModuleSourcePreparationError::PatchFileObservation {
                            demand,
                            error: *error,
                        },
                    ));
                }
                PathObservationResult::Lstat(_)
                | PathObservationResult::ReadLink(_)
                | PathObservationResult::DirectoryEntries(_) => {
                    unreachable!("FileBytes demand must return a FileBytes observation")
                }
            };
            if !patch.is_empty() {
                bytes = match apply_unified_patch(bytes, &patch, override_.patch_strip) {
                    Ok(bytes) => bytes,
                    Err(error) => {
                        return PathOutcome::Complete(Err(ModuleSourcePreparationError::Patch(
                            error.0,
                        )));
                    }
                };
            }
        }
        PathOutcome::Complete(Ok(bytes))
    }
}

fn main_repo_patch_path(label: &CanonicalLabel) -> Option<PathBuf> {
    if !label.package().repo().as_str().is_empty() {
        return None;
    }
    let mut path = PathBuf::new();
    let package = label.package().package().as_str();
    if !package.is_empty() {
        path.push(package);
    }
    path.push(label.target().as_str());
    (!path
        .components()
        .any(|component| !matches!(component, Component::Normal(_))))
    .then_some(path)
}

fn checked_relative_path(path: &Path) -> Result<&Path, CompactString> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(format!(
            "repository source path is not a normalized relative path: {}",
            path.display()
        )
        .into());
    }
    Ok(path)
}

pub fn source_identity(bytes: &[u8]) -> Arc<str> {
    Arc::from(hex::encode(Sha256::digest(bytes)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn immutable(root: &str) -> RepositoryMaterialization {
        RepositoryMaterialization::Immutable {
            canonical_repo: CanonicalRepoName::new("dep+").unwrap(),
            repo_spec: RepoSpec {
                rule_id: crate::RepoRuleId {
                    bzl_file: slug_identity_v2::CanonicalLabel::parse(
                        "@@bazel_tools//tools/build_defs/repo:http.bzl",
                    )
                    .unwrap(),
                    rule_name: "http_archive".into(),
                },
                attributes: Arc::default(),
            },
            source_identity: Arc::from("fixed-content"),
            generation_root: PathBuf::from(root),
        }
    }

    #[test]
    fn immutable_materialization_equality_excludes_generation_root() {
        let left = Arc::new(Ok(immutable("/tmp/generation-a")));
        let right = Arc::new(Ok(immutable("/tmp/generation-b")));

        assert_ne!(left, right);
        assert!(RepositoryMaterializationKey::equality(&left, &right));
    }
}
