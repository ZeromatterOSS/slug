/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::hash::Hash;
use std::hash::Hasher;
use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use dupe::Dupe;
use starlark_map::small_map::SmallMap;

use crate::actions::ActionOutputKind;
use crate::analysis_value::AnalysisArtifact;
use crate::analysis_value::AnalysisDepset;
use crate::analysis_value::AnalysisValue;
use crate::analysis_value::AnalysisValueType;
use crate::analysis_value::ProviderIdentity;
use crate::analysis_value::ProviderOccurrence;
use crate::analysis_value::PublicationEqState;
use crate::depset::Depset;
use crate::depset::DepsetOrder;

pub type FileDepset = Depset<String>;

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative)]
pub struct ProviderName(CompactString);

impl ProviderName {
    pub fn new(name: impl Into<String>) -> Result<Self, ProviderError> {
        let name = name.into();
        if name.is_empty() {
            return Err(ProviderError::EmptyProviderName);
        }
        Ok(Self(name.into()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ProviderName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<&str> for ProviderName {
    type Error = ProviderError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ProviderError {
    EmptyProviderName,
    DuplicateProvider { name: ProviderName },
    MissingDefaultInfo,
    InvalidDefaultInfoFiles { element_type: AnalysisValueType },
    InvalidDefaultInfoArtifactKind { kind: ActionOutputKind },
    MissingExecutable,
    FilesToRunAlreadyComplete,
    RunfilesMissingExecutable,
}

impl fmt::Display for ProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyProviderName => f.write_str("provider name must not be empty"),
            Self::DuplicateProvider { name } => write!(f, "provider {name} specified twice"),
            Self::MissingDefaultInfo => {
                f.write_str("collection did not receive a `DefaultInfo` provider")
            }
            Self::InvalidDefaultInfoFiles { element_type } => write!(
                f,
                "DefaultInfo.files must be a depset of Files, got {element_type:?}"
            ),
            Self::InvalidDefaultInfoArtifactKind { kind } => write!(
                f,
                "DefaultInfo.files contains unsupported {kind:?} artifact"
            ),
            Self::MissingExecutable => {
                f.write_str("runfiles support requires an executable FilesToRun provider")
            }
            Self::FilesToRunAlreadyComplete => {
                f.write_str("runfiles support cannot replace a complete FilesToRun provider")
            }
            Self::RunfilesMissingExecutable => {
                f.write_str("runfiles support must contain its owning executable")
            }
        }
    }
}

impl Error for ProviderError {}

/// Structural identity of one exported user provider constructor.
///
/// The source label and exported variable name are both semantic. The shared
/// allocation makes graph-value copies pointer-sized while equality and
/// hashing remain structural.
#[derive(Debug, Clone, Dupe, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative)]
pub struct ProviderId(Arc<ProviderIdData>);

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative)]
struct ProviderIdData {
    source_label: CompactString,
    exported_name: CompactString,
}

impl ProviderId {
    pub fn new(
        source_label: impl Into<CompactString>,
        exported_name: impl Into<CompactString>,
    ) -> Result<Self, ProviderError> {
        let source_label = source_label.into();
        let exported_name = exported_name.into();
        if exported_name.is_empty() {
            return Err(ProviderError::EmptyProviderName);
        }
        Ok(Self(Arc::new(ProviderIdData {
            source_label,
            exported_name,
        })))
    }

    pub fn unqualified(exported_name: impl Into<CompactString>) -> Result<Self, ProviderError> {
        Self::new("<unqualified>", exported_name)
    }

    pub fn source_label(&self) -> &str {
        &self.0.source_label
    }

    pub fn exported_name(&self) -> &str {
        &self.0.exported_name
    }
}

impl fmt::Display for ProviderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}%{}", self.source_label(), self.exported_name())
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Allocative)]
pub enum RunfilesConflictPolicy {
    Warn,
    Error,
}

#[derive(Debug, Clone, Allocative)]
pub struct RunfilesSymlink {
    pub path: CompactString,
    pub artifact: AnalysisArtifact,
    occurrence: Arc<()>,
}

impl RunfilesSymlink {
    pub fn new(path: impl Into<CompactString>, artifact: AnalysisArtifact) -> Self {
        Self {
            path: path.into(),
            artifact,
            occurrence: Arc::new(()),
        }
    }

    pub(crate) fn publication_eq(&self, other: &Self) -> bool {
        self.path == other.path && self.artifact == other.artifact
    }
}

impl PartialEq for RunfilesSymlink {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.occurrence, &other.occurrence)
    }
}

impl Eq for RunfilesSymlink {}

impl Hash for RunfilesSymlink {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (Arc::as_ptr(&self.occurrence) as usize).hash(state);
    }
}

pub type RunfilesSymlinkDepset = Depset<RunfilesSymlink>;

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub struct RetainedRunfiles {
    pub files: AnalysisDepset,
    pub symlinks: RunfilesSymlinkDepset,
    pub root_symlinks: RunfilesSymlinkDepset,
    pub empty_filenames: FileDepset,
    pub conflict_policy: RunfilesConflictPolicy,
    pub repository_prefix: CompactString,
}

#[derive(Debug, Clone, Allocative)]
pub struct RunfilesSupport {
    pub runfiles: RetainedRunfiles,
    pub tree: AnalysisArtifact,
    pub input_manifest: AnalysisArtifact,
    pub manifest: Option<AnalysisArtifact>,
    pub repo_mapping_manifest: Option<AnalysisArtifact>,
}

impl RunfilesSupport {
    pub(crate) fn publication_eq_with(&self, other: &Self, state: &mut PublicationEqState) -> bool {
        self.runfiles.publication_eq_with(&other.runfiles, state)
            && self.tree == other.tree
            && self.input_manifest == other.input_manifest
            && self.manifest == other.manifest
            && self.repo_mapping_manifest == other.repo_mapping_manifest
    }
}

impl PartialEq for RunfilesSupport {
    fn eq(&self, other: &Self) -> bool {
        self.publication_eq_with(other, &mut PublicationEqState::default())
    }
}

impl Eq for RunfilesSupport {}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub struct FilesToRunProvider {
    files: AnalysisDepset,
    pub executable: Option<AnalysisArtifact>,
    pub support: Option<Arc<RunfilesSupport>>,
}

impl FilesToRunProvider {
    pub fn empty() -> Self {
        Self {
            files: AnalysisDepset::empty(DepsetOrder::Default),
            executable: None,
            support: None,
        }
    }

    pub fn files(&self) -> &AnalysisDepset {
        &self.files
    }

    pub fn runfiles_manifest(&self) -> Option<&AnalysisArtifact> {
        self.support
            .as_ref()
            .and_then(|support| support.manifest.as_ref())
    }

    pub fn repo_mapping_manifest(&self) -> Option<&AnalysisArtifact> {
        self.support
            .as_ref()
            .and_then(|support| support.repo_mapping_manifest.as_ref())
    }

    pub fn to_occurrence(&self) -> ProviderOccurrence {
        ProviderOccurrence::new(
            ProviderIdentity::builtin("FilesToRunProvider"),
            [
                (
                    "executable",
                    self.executable
                        .clone()
                        .map(AnalysisValue::artifact)
                        .unwrap_or_else(AnalysisValue::none),
                ),
                ("_files_to_run", AnalysisValue::depset(self.files.clone())),
                (
                    "runfiles_manifest",
                    self.runfiles_manifest()
                        .cloned()
                        .map(AnalysisValue::artifact)
                        .unwrap_or_else(AnalysisValue::none),
                ),
                (
                    "repo_mapping_manifest",
                    self.repo_mapping_manifest()
                        .cloned()
                        .map(AnalysisValue::artifact)
                        .unwrap_or_else(AnalysisValue::none),
                ),
            ],
        )
        .with_files_to_run(Arc::new(self.clone()))
    }

    pub fn from_occurrence(value: &ProviderOccurrence) -> Option<Self> {
        if !value.identity().is_builtin("FilesToRunProvider") {
            return None;
        }
        value.files_to_run().map(|value| value.as_ref().clone())
    }

    fn from_files(files: AnalysisDepset) -> Self {
        Self {
            files,
            ..Self::empty()
        }
    }

    fn executable_without_support(files: AnalysisDepset, executable: AnalysisArtifact) -> Self {
        Self {
            files,
            executable: Some(executable),
            support: None,
        }
    }

    pub fn single_executable_without_support(executable: AnalysisArtifact) -> Self {
        let files = AnalysisDepset::new(
            DepsetOrder::Default,
            vec![AnalysisValue::artifact(executable.clone())],
            Vec::new(),
        )
        .expect("a singleton executable artifact depset is valid");
        Self {
            files,
            executable: Some(executable),
            support: None,
        }
    }

    pub(crate) fn publication_eq_with(&self, other: &Self, state: &mut PublicationEqState) -> bool {
        self.files.publication_eq_with(&other.files, state)
            && self.executable == other.executable
            && match (&self.support, &other.support) {
                (Some(left), Some(right)) => left.publication_eq_with(right, state),
                (None, None) => true,
                _ => false,
            }
    }

    fn with_support(
        &self,
        files_to_build: AnalysisDepset,
        support: Arc<RunfilesSupport>,
    ) -> Result<Self, ProviderError> {
        let Some(executable) = &self.executable else {
            return Err(ProviderError::MissingExecutable);
        };
        if self.support.is_some() {
            return Err(ProviderError::FilesToRunAlreadyComplete);
        }
        if !support.runfiles.files.to_list().iter().any(|value| {
            matches!(
                value.kind(),
                crate::analysis_value::AnalysisValueKind::Artifact(artifact)
                    if artifact == executable
            )
        }) {
            return Err(ProviderError::RunfilesMissingExecutable);
        }
        let runfiles_trees = AnalysisDepset::new(
            DepsetOrder::Default,
            vec![AnalysisValue::artifact(support.tree.clone())],
            Vec::new(),
        )
        .expect("a singleton runfiles-tree Artifact depset is valid");
        let files = AnalysisDepset::new(
            DepsetOrder::Default,
            vec![AnalysisValue::artifact(executable.clone())],
            vec![files_to_build, runfiles_trees],
        )
        .expect("typed files, runfiles tree, and executable compose in stable order");
        Ok(Self {
            files,
            executable: Some(executable.clone()),
            support: Some(support),
        })
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub struct DefaultInfo {
    files: AnalysisDepset,
    pub default_runfiles: RetainedRunfiles,
    pub data_runfiles: RetainedRunfiles,
    pub executable: Option<AnalysisArtifact>,
    pub files_to_run: FilesToRunProvider,
}

impl DefaultInfo {
    pub fn empty() -> Self {
        Self {
            files: AnalysisDepset::empty(DepsetOrder::Default),
            default_runfiles: RetainedRunfiles::empty(),
            data_runfiles: RetainedRunfiles::empty(),
            executable: None,
            files_to_run: FilesToRunProvider::empty(),
        }
    }

    pub fn from_files(files: AnalysisDepset) -> Result<Self, ProviderError> {
        Self::ensure_artifact_files(&files)?;
        Ok(Self {
            files: files.clone(),
            files_to_run: FilesToRunProvider::from_files(files),
            ..Self::empty()
        })
    }

    pub fn from_file_target(artifact: AnalysisArtifact) -> Self {
        let files_to_run = FilesToRunProvider::single_executable_without_support(artifact.clone());
        Self {
            files: files_to_run.files.clone(),
            executable: Some(artifact),
            files_to_run,
            ..Self::empty()
        }
    }

    pub fn files(&self) -> &AnalysisDepset {
        &self.files
    }

    pub fn with_runfiles_support(
        &self,
        support: Arc<RunfilesSupport>,
    ) -> Result<Self, ProviderError> {
        Ok(Self {
            files: self.files.clone(),
            default_runfiles: self.default_runfiles.clone(),
            data_runfiles: self.data_runfiles.clone(),
            executable: self.executable.clone(),
            files_to_run: self
                .files_to_run
                .with_support(self.files.clone(), support)?,
        })
    }

    pub fn file_artifacts(&self) -> Vec<AnalysisArtifact> {
        self.files
            .to_list()
            .into_iter()
            .map(|value| match value.kind() {
                crate::analysis_value::AnalysisValueKind::Artifact(artifact) => artifact.clone(),
                _ => unreachable!("DefaultInfo checked constructors retain only Files"),
            })
            .collect()
    }

    /// Creates the bounded analysis representation of an executable
    /// `DefaultInfo`. Explicit files retain the rule's declared file set;
    /// otherwise the executable is its sole default file and runfile.
    pub fn from_executable(
        executable_artifact: AnalysisArtifact,
        files: Option<AnalysisDepset>,
    ) -> Result<Self, ProviderError> {
        if let Some(files) = &files {
            Self::ensure_artifact_files(files)?;
        }
        let executable_files = AnalysisDepset::new(
            DepsetOrder::Default,
            vec![AnalysisValue::artifact(executable_artifact.clone())],
            Vec::new(),
        )
        .expect("a singleton executable artifact depset is valid");
        let effective_files = files.unwrap_or_else(|| executable_files.clone());
        let files_to_run = if effective_files == executable_files {
            executable_files
        } else {
            AnalysisDepset::new(
                DepsetOrder::Default,
                vec![AnalysisValue::artifact(executable_artifact.clone())],
                vec![effective_files.clone()],
            )
            .expect("typed DefaultInfo files compose with its executable")
        };
        let runfiles = RetainedRunfiles::empty()
            .with_artifact(executable_artifact.clone())
            .expect("an executable is a valid runfiles Artifact");
        Ok(Self {
            files: effective_files,
            default_runfiles: runfiles.clone(),
            data_runfiles: runfiles,
            executable: Some(executable_artifact.clone()),
            files_to_run: FilesToRunProvider::executable_without_support(
                files_to_run,
                executable_artifact,
            ),
        })
    }

    pub fn from_effective(
        files: AnalysisDepset,
        default_runfiles: RetainedRunfiles,
        data_runfiles: RetainedRunfiles,
        executable: Option<AnalysisArtifact>,
    ) -> Result<Self, ProviderError> {
        Self::ensure_artifact_files(&files)?;
        let files_to_run = match &executable {
            Some(executable) => {
                let files_to_run = AnalysisDepset::new(
                    DepsetOrder::Default,
                    vec![AnalysisValue::artifact(executable.clone())],
                    vec![files.clone()],
                )
                .expect("typed DefaultInfo files compose with its executable");
                FilesToRunProvider::executable_without_support(files_to_run, executable.clone())
            }
            None => FilesToRunProvider::from_files(files.clone()),
        };
        Ok(Self {
            files,
            default_runfiles,
            data_runfiles,
            executable,
            files_to_run,
        })
    }

    fn ensure_artifact_files(files: &AnalysisDepset) -> Result<(), ProviderError> {
        match files.element_type() {
            AnalysisValueType::Empty => return Ok(()),
            AnalysisValueType::Artifact => {}
            element_type => {
                return Err(ProviderError::InvalidDefaultInfoFiles { element_type });
            }
        }
        for value in files.to_list() {
            if let crate::analysis_value::AnalysisValueKind::Artifact(AnalysisArtifact::Derived {
                output,
                ..
            }) = value.kind()
                && output.kind() != ActionOutputKind::File
            {
                return Err(ProviderError::InvalidDefaultInfoArtifactKind {
                    kind: output.kind(),
                });
            }
        }
        Ok(())
    }

    fn publication_eq_with(&self, other: &Self, state: &mut PublicationEqState) -> bool {
        self.files.publication_eq_with(&other.files, state)
            && self
                .default_runfiles
                .publication_eq_with(&other.default_runfiles, state)
            && self
                .data_runfiles
                .publication_eq_with(&other.data_runfiles, state)
            && self.executable == other.executable
            && self
                .files_to_run
                .publication_eq_with(&other.files_to_run, state)
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub struct OutputGroupInfo {
    pub groups: BTreeMap<String, FileDepset>,
}

impl OutputGroupInfo {
    pub fn new(groups: BTreeMap<String, FileDepset>) -> Self {
        Self { groups }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub struct RunEnvironmentInfo {
    pub environment: BTreeMap<String, String>,
    pub inherited_environment: Vec<String>,
}

impl RunEnvironmentInfo {
    pub fn empty() -> Self {
        Self {
            environment: BTreeMap::new(),
            inherited_environment: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub struct PlatformInfo {
    pub label: String,
    pub constraints: BTreeMap<String, String>,
    pub exec_properties: BTreeMap<String, String>,
}

impl PlatformInfo {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            constraints: BTreeMap::new(),
            exec_properties: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub enum ProviderValue {
    DefaultInfo(DefaultInfo),
    OutputGroupInfo(OutputGroupInfo),
    RunEnvironmentInfo(RunEnvironmentInfo),
    FilesToRunProvider(FilesToRunProvider),
    PlatformInfo(PlatformInfo),
    Occurrence(ProviderOccurrence),
}

impl ProviderValue {
    pub fn name(&self) -> ProviderName {
        match self {
            Self::DefaultInfo(_) => ProviderName("DefaultInfo".into()),
            Self::OutputGroupInfo(_) => ProviderName("OutputGroupInfo".into()),
            Self::RunEnvironmentInfo(_) => ProviderName("RunEnvironmentInfo".into()),
            Self::FilesToRunProvider(_) => ProviderName("FilesToRunProvider".into()),
            Self::PlatformInfo(_) => ProviderName("PlatformInfo".into()),
            Self::Occurrence(provider) => ProviderName(provider.identity().name().into()),
        }
    }

    fn identity(&self) -> ProviderIdentity {
        match self {
            Self::DefaultInfo(_) => ProviderIdentity::builtin("DefaultInfo"),
            Self::OutputGroupInfo(_) => ProviderIdentity::builtin("OutputGroupInfo"),
            Self::RunEnvironmentInfo(_) => ProviderIdentity::builtin("RunEnvironmentInfo"),
            Self::FilesToRunProvider(_) => ProviderIdentity::builtin("FilesToRunProvider"),
            Self::PlatformInfo(_) => ProviderIdentity::builtin("PlatformInfo"),
            Self::Occurrence(provider) => provider.identity().clone(),
        }
    }

    fn publication_eq_with(&self, other: &Self, state: &mut PublicationEqState) -> bool {
        match (self, other) {
            (Self::DefaultInfo(left), Self::DefaultInfo(right)) => {
                left.publication_eq_with(right, state)
            }
            (Self::OutputGroupInfo(left), Self::OutputGroupInfo(right)) => left == right,
            (Self::RunEnvironmentInfo(left), Self::RunEnvironmentInfo(right)) => left == right,
            (Self::FilesToRunProvider(left), Self::FilesToRunProvider(right)) => {
                left.publication_eq_with(right, state)
            }
            (Self::PlatformInfo(left), Self::PlatformInfo(right)) => left == right,
            (Self::Occurrence(left), Self::Occurrence(right)) => {
                left.publication_eq_with(right, state)
            }
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Dupe, Allocative)]
pub struct ProviderCollection(Arc<ProviderCollectionData>);

#[derive(Debug, Allocative)]
struct ProviderCollectionData {
    providers: SmallMap<ProviderIdentity, ProviderValue>,
}

impl ProviderCollection {
    pub fn len(&self) -> usize {
        self.0.providers.len()
    }

    pub fn new(values: Vec<ProviderValue>) -> Result<Self, ProviderError> {
        Self::from_values(values, true)
    }

    pub fn from_values(
        values: Vec<ProviderValue>,
        require_default_info: bool,
    ) -> Result<Self, ProviderError> {
        let mut providers = SmallMap::with_capacity(values.len());
        for value in values {
            let name = value.name();
            if providers.insert(value.identity(), value).is_some() {
                return Err(ProviderError::DuplicateProvider { name });
            }
        }
        if require_default_info
            && !providers.contains_key(&ProviderIdentity::builtin("DefaultInfo"))
        {
            return Err(ProviderError::MissingDefaultInfo);
        }
        Ok(Self(Arc::new(ProviderCollectionData { providers })))
    }

    pub fn contains(&self, identity: &ProviderIdentity) -> bool {
        self.0.providers.contains_key(identity)
    }

    pub fn get(&self, identity: &ProviderIdentity) -> Option<&ProviderValue> {
        self.0.providers.get(identity)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&ProviderIdentity, &ProviderValue)> {
        self.0.providers.iter()
    }

    pub fn names(&self) -> impl Iterator<Item = ProviderName> + '_ {
        self.0.providers.values().map(ProviderValue::name)
    }

    pub fn occurrences(&self) -> impl Iterator<Item = &ProviderOccurrence> {
        self.0.providers.values().filter_map(|value| match value {
            ProviderValue::Occurrence(occurrence) => Some(occurrence),
            _ => None,
        })
    }

    pub fn user(&self, id: &ProviderId) -> Option<&ProviderOccurrence> {
        match self.0.providers.get(&ProviderIdentity::user(id.dupe())) {
            Some(ProviderValue::Occurrence(provider)) => Some(provider),
            _ => None,
        }
    }

    pub fn default_info(&self) -> Option<&DefaultInfo> {
        match self
            .0
            .providers
            .get(&ProviderIdentity::builtin("DefaultInfo"))
        {
            Some(ProviderValue::DefaultInfo(info)) => Some(info),
            _ => None,
        }
    }

    pub fn with_default_info(&self, info: DefaultInfo) -> Self {
        let mut providers = self.0.providers.clone();
        let previous = providers.insert(
            ProviderIdentity::builtin("DefaultInfo"),
            ProviderValue::DefaultInfo(info),
        );
        debug_assert!(matches!(previous, Some(ProviderValue::DefaultInfo(_))));
        Self(Arc::new(ProviderCollectionData { providers }))
    }

    /// Builtin-only lookup: user providers named `ToolchainInfo` never match.
    pub fn toolchain_info(&self) -> Option<&ProviderOccurrence> {
        match self
            .0
            .providers
            .get(&ProviderIdentity::builtin("ToolchainInfo"))
        {
            Some(ProviderValue::Occurrence(info)) => Some(info),
            _ => None,
        }
    }

    pub(crate) fn publication_eq_with(&self, other: &Self, state: &mut PublicationEqState) -> bool {
        self.0.providers.len() == other.0.providers.len()
            && self.0.providers.iter().all(|(identity, value)| {
                other
                    .0
                    .providers
                    .get(identity)
                    .is_some_and(|other| value.publication_eq_with(other, state))
            })
    }
}

impl PartialEq for ProviderCollection {
    fn eq(&self, other: &Self) -> bool {
        self.publication_eq_with(other, &mut PublicationEqState::default())
    }
}

impl Eq for ProviderCollection {}
