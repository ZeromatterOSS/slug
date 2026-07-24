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
use slug_workspace_v2::WorkspaceRawFileKey;
use slug_workspace_v2::WorkspaceRawFileValue;

use crate::ModuleKey;
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
    Found(Arc<[u8]>),
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum ModuleSourcePreparationError {
    RootModuleFiles(CompactString),
    RegistryPolicy(CompactString),
    Registry(CompactString),
    Source(Arc<str>),
    Patch(CompactString),
    MissingVersion,
    ModuleNotFound,
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
    ReadError(Arc<str>),
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

#[async_trait]
impl Key for RepositorySourceFileKey {
    type Value = RepositorySourceFileValue;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let relative = match checked_relative_path(&self.repo_relative_path) {
            Ok(relative) => relative,
            Err(error) => return RepositorySourceFileValue::ReadError(error.into()),
        };
        let materialization = match ctx
            .compute(&RepositoryMaterializationKey {
                workspace: self.workspace.clone(),
                module_name: self.module_name.clone(),
            })
            .await
        {
            Ok(value) => value,
            Err(error) => return RepositorySourceFileValue::ReadError(error.to_string().into()),
        };
        let materialization = match materialization.as_ref() {
            Ok(value) => value,
            Err(error) => {
                let error = error.clone();
                if let Err(generation_error) = ctx
                    .compute(&RepositoryMaterializationGenerationKey {
                        workspace: self.workspace.clone(),
                    })
                    .await
                {
                    return RepositorySourceFileValue::ReadError(
                        generation_error.to_string().into(),
                    );
                }
                return RepositorySourceFileValue::ReadError(format!("{error:?}").into());
            }
        };
        match materialization {
            RepositoryMaterialization::Local { source_root, .. } => match ctx
                .compute(&WorkspaceRawFileKey {
                    workspace: self.workspace.clone(),
                    path: source_root.join(relative),
                })
                .await
            {
                Ok(value) => match value {
                    WorkspaceRawFileValue::Present(bytes) => {
                        RepositorySourceFileValue::Present(bytes)
                    }
                    WorkspaceRawFileValue::Absent => RepositorySourceFileValue::Absent,
                    WorkspaceRawFileValue::ReadError(error) => {
                        RepositorySourceFileValue::ReadError(error.to_string().into())
                    }
                },
                Err(error) => RepositorySourceFileValue::ReadError(error.to_string().into()),
            },
            RepositoryMaterialization::Immutable {
                generation_root, ..
            } => match std::fs::read(generation_root.join(relative)) {
                Ok(bytes) => RepositorySourceFileValue::Present(Arc::from(bytes)),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    RepositorySourceFileValue::Absent
                }
                Err(error) => RepositorySourceFileValue::ReadError(error.to_string().into()),
            },
        }
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[async_trait]
impl Key for ModuleSourcePreparationKey {
    type Value = Arc<Result<ModuleSourcePreparation, ModuleSourcePreparationError>>;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let root = match ctx
            .compute(&RootModuleFilesKey {
                workspace: self.workspace.clone(),
            })
            .await
        {
            Ok(value) => value,
            Err(error) => {
                return Arc::new(Err(ModuleSourcePreparationError::RootModuleFiles(
                    error.to_string().into(),
                )));
            }
        };
        let root = match root.as_ref() {
            Ok(value) => value,
            Err(error) => {
                return Arc::new(Err(ModuleSourcePreparationError::RootModuleFiles(
                    error.clone(),
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
                Ok(RepositorySourceFileValue::Present(bytes)) => {
                    Ok(ModuleSourcePreparation::Found(bytes))
                }
                Ok(RepositorySourceFileValue::Absent) => {
                    Err(ModuleSourcePreparationError::ModuleNotFound)
                }
                Ok(RepositorySourceFileValue::ReadError(error)) => {
                    Err(ModuleSourcePreparationError::Source(error))
                }
                Err(error) => Err(ModuleSourcePreparationError::Source(
                    error.to_string().into(),
                )),
            };
            return Arc::new(value);
        }
        if self.version.is_empty() {
            return Arc::new(Err(ModuleSourcePreparationError::MissingVersion));
        }
        let policy = match ctx
            .compute(&RegistryPolicyKey {
                workspace: self.workspace.clone(),
            })
            .await
        {
            Ok(value) => value,
            Err(error) => {
                return Arc::new(Err(ModuleSourcePreparationError::RegistryPolicy(
                    error.to_string().into(),
                )));
            }
        };
        let policy = match policy.as_ref() {
            Ok(value) => value,
            Err(error) => {
                return Arc::new(Err(ModuleSourcePreparationError::RegistryPolicy(
                    format!("{error:?}").into(),
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
            return Arc::new(
                match self
                    .prepare_from_registry(ctx, override_.as_ref(), registry, &module)
                    .await
                {
                    Ok(Some(value)) => Ok(value),
                    Ok(None) => Err(ModuleSourcePreparationError::ModuleNotFound),
                    Err(error) => Err(error),
                },
            );
        }
        for registry in policy.urls().as_slice() {
            match self
                .prepare_from_registry(ctx, override_.as_ref(), registry.as_str(), &module)
                .await
            {
                Ok(Some(value)) => return Arc::new(Ok(value)),
                Ok(None) => {}
                Err(error) => return Arc::new(Err(error)),
            }
        }
        Arc::new(Err(ModuleSourcePreparationError::ModuleNotFound))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

impl ModuleSourcePreparationKey {
    async fn prepare_from_registry(
        &self,
        ctx: &mut DiceComputations<'_>,
        override_: Option<&RootModuleOverride>,
        registry: &str,
        module: &ModuleKey,
    ) -> Result<Option<ModuleSourcePreparation>, ModuleSourcePreparationError> {
        let url = RegistryFileUrl::new(registry_module_file_url(registry, module));
        let file = ctx
            .compute(&RegistryFileKey {
                workspace: self.workspace.clone(),
                url,
            })
            .await
            .map_err(|error| ModuleSourcePreparationError::Registry(error.to_string().into()))?;
        match file.as_ref() {
            Ok(RegistryFileValue::NotFound { .. }) => Ok(None),
            Ok(RegistryFileValue::Found { bytes, .. }) => Ok(Some(ModuleSourcePreparation::Found(
                self.apply_root_patches(ctx, override_, bytes.clone())
                    .await?,
            ))),
            Err(error) => Err(ModuleSourcePreparationError::Registry(
                format!("{error:?}").into(),
            )),
        }
    }

    async fn apply_root_patches(
        &self,
        ctx: &mut DiceComputations<'_>,
        override_: Option<&RootModuleOverride>,
        mut bytes: Arc<[u8]>,
    ) -> Result<Arc<[u8]>, ModuleSourcePreparationError> {
        let Some(RootModuleOverride::RegistrySingle(override_)) = override_ else {
            return Ok(bytes);
        };
        // PatchUtil filters this list to main-repository labels. `patch_cmds`
        // are deliberately inactive for module-file patching.
        for label in override_.patches.iter() {
            let Some(path) = main_repo_patch_path(label) else {
                continue;
            };
            let patch = match ctx
                .compute(&WorkspaceRawFileKey {
                    workspace: self.workspace.clone(),
                    path: self.workspace.join(path),
                })
                .await
            {
                Ok(WorkspaceRawFileValue::Present(bytes)) => bytes,
                Ok(WorkspaceRawFileValue::Absent) => {
                    return Err(ModuleSourcePreparationError::Patch(
                        "patch file is absent".into(),
                    ));
                }
                Ok(WorkspaceRawFileValue::ReadError(error)) => {
                    return Err(ModuleSourcePreparationError::Patch(
                        error.to_string().into(),
                    ));
                }
                Err(error) => {
                    return Err(ModuleSourcePreparationError::Patch(
                        error.to_string().into(),
                    ));
                }
            };
            bytes = apply_unified_patch(bytes, &patch, override_.patch_strip)
                .map_err(|error| ModuleSourcePreparationError::Patch(error.0))?;
        }
        Ok(bytes)
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
