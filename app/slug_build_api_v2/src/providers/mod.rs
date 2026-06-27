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

use crate::depset::Depset;

pub type FileDepset = Depset<String>;

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ProviderName(String);

impl ProviderName {
    pub fn new(name: impl Into<String>) -> Result<Self, ProviderError> {
        let name = name.into();
        if name.is_empty() {
            return Err(ProviderError::EmptyProviderName);
        }
        Ok(Self(name))
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

#[derive(Debug, Clone, Eq, PartialEq)]
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

#[derive(Debug, Clone, Eq, PartialEq)]
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

#[derive(Debug, Clone, Eq, PartialEq)]
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

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct OutputGroupInfo {
    pub groups: BTreeMap<String, FileDepset>,
}

impl OutputGroupInfo {
    pub fn new(groups: BTreeMap<String, FileDepset>) -> Self {
        Self { groups }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
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

#[derive(Debug, Clone, Eq, PartialEq)]
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

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct UserProvider {
    pub name: ProviderName,
    pub fields: BTreeMap<String, String>,
}

impl UserProvider {
    pub fn new(
        name: impl Into<String>,
        fields: BTreeMap<String, String>,
    ) -> Result<Self, ProviderError> {
        Ok(Self {
            name: ProviderName::new(name)?,
            fields,
        })
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
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
            Self::DefaultInfo(_) => ProviderName("DefaultInfo".to_owned()),
            Self::OutputGroupInfo(_) => ProviderName("OutputGroupInfo".to_owned()),
            Self::RunEnvironmentInfo(_) => ProviderName("RunEnvironmentInfo".to_owned()),
            Self::FilesToRunProvider(_) => ProviderName("FilesToRunProvider".to_owned()),
            Self::PlatformInfo(_) => ProviderName("PlatformInfo".to_owned()),
            Self::User(provider) => provider.name.clone(),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ProviderCollection {
    providers: BTreeMap<ProviderName, ProviderValue>,
}

impl ProviderCollection {
    pub fn new(values: Vec<ProviderValue>) -> Result<Self, ProviderError> {
        Self::from_values(values, true)
    }

    pub fn from_values(
        values: Vec<ProviderValue>,
        require_default_info: bool,
    ) -> Result<Self, ProviderError> {
        let mut providers = BTreeMap::new();
        for value in values {
            let name = value.name();
            if providers.insert(name.clone(), value).is_some() {
                return Err(ProviderError::DuplicateProvider { name });
            }
        }
        let default_info = ProviderName("DefaultInfo".to_owned());
        if require_default_info && !providers.contains_key(&default_info) {
            return Err(ProviderError::MissingDefaultInfo);
        }
        Ok(Self { providers })
    }

    pub fn contains(&self, name: &ProviderName) -> bool {
        self.providers.contains_key(name)
    }

    pub fn get(&self, name: &ProviderName) -> Option<&ProviderValue> {
        self.providers.get(name)
    }

    pub fn names(&self) -> impl Iterator<Item = &ProviderName> {
        self.providers.keys()
    }

    pub fn default_info(&self) -> Option<&DefaultInfo> {
        match self.get(&ProviderName("DefaultInfo".to_owned())) {
            Some(ProviderValue::DefaultInfo(info)) => Some(info),
            _ => None,
        }
    }
}
