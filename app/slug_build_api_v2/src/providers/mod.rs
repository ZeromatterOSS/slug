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

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub struct Runfiles {
    pub files: FileDepset,
    pub symlinks: BTreeMap<String, String>,
    pub empty_filenames: FileDepset,
}

impl Runfiles {
    pub fn empty() -> Self {
        Self {
            files: Depset::empty(),
            symlinks: BTreeMap::new(),
            empty_filenames: Depset::empty(),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub struct FilesToRunProvider {
    pub executable: Option<String>,
    pub runfiles_manifest: Option<String>,
    pub repo_mapping_manifest: Option<String>,
}

impl FilesToRunProvider {
    pub fn empty() -> Self {
        Self {
            executable: None,
            runfiles_manifest: None,
            repo_mapping_manifest: None,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub struct DefaultInfo {
    files: AnalysisDepset,
    pub default_runfiles: Runfiles,
    pub data_runfiles: Runfiles,
    pub executable: Option<String>,
    pub files_to_run: FilesToRunProvider,
}

impl DefaultInfo {
    pub fn empty() -> Self {
        Self {
            files: AnalysisDepset::empty(DepsetOrder::Default),
            default_runfiles: Runfiles::empty(),
            data_runfiles: Runfiles::empty(),
            executable: None,
            files_to_run: FilesToRunProvider::empty(),
        }
    }

    pub fn from_files(files: AnalysisDepset) -> Result<Self, ProviderError> {
        Self::ensure_artifact_files(&files)?;
        Ok(Self {
            files,
            ..Self::empty()
        })
    }

    pub fn files(&self) -> &AnalysisDepset {
        &self.files
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
        executable: String,
        executable_artifact: AnalysisArtifact,
        files: Option<AnalysisDepset>,
    ) -> Result<Self, ProviderError> {
        if let Some(files) = &files {
            Self::ensure_artifact_files(files)?;
        }
        let executable_files = AnalysisDepset::new(
            DepsetOrder::Default,
            vec![AnalysisValue::artifact(executable_artifact)],
            Vec::new(),
        )
        .expect("a singleton executable artifact depset is valid");
        let executable_runfiles =
            Depset::from_direct(DepsetOrder::Default, vec![executable.clone()])
                .expect("a singleton executable runfiles depset is valid");
        let runfiles = Runfiles {
            files: executable_runfiles,
            ..Runfiles::empty()
        };
        Ok(Self {
            files: files.unwrap_or(executable_files),
            default_runfiles: runfiles.clone(),
            data_runfiles: runfiles,
            executable: Some(executable.clone()),
            files_to_run: FilesToRunProvider {
                executable: Some(executable),
                ..FilesToRunProvider::empty()
            },
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
            && self.default_runfiles == other.default_runfiles
            && self.data_runfiles == other.data_runfiles
            && self.executable == other.executable
            && self.files_to_run == other.files_to_run
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
            (Self::FilesToRunProvider(left), Self::FilesToRunProvider(right)) => left == right,
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
