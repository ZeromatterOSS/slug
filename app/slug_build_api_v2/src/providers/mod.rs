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
use starlark_map::Equivalent;
use starlark_map::small_map::SmallMap;

use crate::depset::Depset;

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
}

impl fmt::Display for ProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyProviderName => f.write_str("provider name must not be empty"),
            Self::DuplicateProvider { name } => write!(f, "provider {name} specified twice"),
            Self::MissingDefaultInfo => {
                f.write_str("collection did not receive a `DefaultInfo` provider")
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
    pub files: FileDepset,
    pub default_runfiles: Runfiles,
    pub data_runfiles: Runfiles,
    pub executable: Option<String>,
    pub files_to_run: FilesToRunProvider,
}

impl DefaultInfo {
    pub fn empty() -> Self {
        Self {
            files: Depset::empty(),
            default_runfiles: Runfiles::empty(),
            data_runfiles: Runfiles::empty(),
            executable: None,
            files_to_run: FilesToRunProvider::empty(),
        }
    }

    pub fn from_files(files: FileDepset) -> Self {
        Self {
            files,
            ..Self::empty()
        }
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
pub struct UserProvider {
    pub id: ProviderId,
    pub fields: SmallMap<CompactString, CompactString>,
}

impl UserProvider {
    pub fn new(
        name: impl Into<String>,
        fields: BTreeMap<String, String>,
    ) -> Result<Self, ProviderError> {
        let id = ProviderId::unqualified(name.into())?;
        Self::with_id(id, fields)
    }

    pub fn with_id(
        id: ProviderId,
        fields: impl IntoIterator<Item = (impl Into<CompactString>, impl Into<CompactString>)>,
    ) -> Result<Self, ProviderError> {
        Ok(Self {
            id,
            fields: fields
                .into_iter()
                .map(|(name, value)| (name.into(), value.into()))
                .collect(),
        })
    }

    pub fn field(&self, name: &str) -> Option<&str> {
        self.fields.get(name).map(CompactString::as_str)
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub enum ProviderValue {
    DefaultInfo(DefaultInfo),
    OutputGroupInfo(OutputGroupInfo),
    RunEnvironmentInfo(RunEnvironmentInfo),
    FilesToRunProvider(FilesToRunProvider),
    PlatformInfo(PlatformInfo),
    User(UserProvider),
}

impl ProviderValue {
    pub fn name(&self) -> ProviderName {
        match self {
            Self::DefaultInfo(_) => ProviderName("DefaultInfo".into()),
            Self::OutputGroupInfo(_) => ProviderName("OutputGroupInfo".into()),
            Self::RunEnvironmentInfo(_) => ProviderName("RunEnvironmentInfo".into()),
            Self::FilesToRunProvider(_) => ProviderName("FilesToRunProvider".into()),
            Self::PlatformInfo(_) => ProviderName("PlatformInfo".into()),
            Self::User(provider) => ProviderName(provider.id.exported_name().into()),
        }
    }

    fn key(&self) -> ProviderKey {
        match self {
            Self::User(provider) => ProviderKey::User(provider.id.dupe()),
            _ => ProviderKey::Builtin(self.name()),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub struct ProviderCollection {
    providers: SmallMap<ProviderKey, ProviderValue>,
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
enum ProviderKey {
    Builtin(ProviderName),
    User(ProviderId),
}

impl Hash for ProviderKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Self::Builtin(name) => name.hash(state),
            Self::User(id) => id.hash(state),
        }
    }
}

impl Equivalent<ProviderKey> for ProviderId {
    fn equivalent(&self, key: &ProviderKey) -> bool {
        matches!(key, ProviderKey::User(id) if id == self)
    }
}

impl ProviderCollection {
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
            if providers.insert(value.key(), value).is_some() {
                return Err(ProviderError::DuplicateProvider { name });
            }
        }
        if require_default_info
            && !providers.keys().any(
                |key| matches!(key, ProviderKey::Builtin(name) if name.as_str() == "DefaultInfo"),
            )
        {
            return Err(ProviderError::MissingDefaultInfo);
        }
        Ok(Self { providers })
    }

    pub fn contains(&self, name: &ProviderName) -> bool {
        self.providers.values().any(|value| value.name() == *name)
    }

    pub fn get(&self, name: &ProviderName) -> Option<&ProviderValue> {
        self.providers.values().find(|value| value.name() == *name)
    }

    pub fn names(&self) -> impl Iterator<Item = ProviderName> + '_ {
        self.providers.values().map(ProviderValue::name)
    }

    pub fn user(&self, id: &ProviderId) -> Option<&UserProvider> {
        match self.providers.get(id) {
            Some(ProviderValue::User(provider)) => Some(provider),
            _ => None,
        }
    }

    pub fn default_info(&self) -> Option<&DefaultInfo> {
        match self.get(&ProviderName("DefaultInfo".into())) {
            Some(ProviderValue::DefaultInfo(info)) => Some(info),
            _ => None,
        }
    }
}
