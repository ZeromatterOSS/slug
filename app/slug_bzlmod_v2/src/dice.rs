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
use std::fmt;

use sha2::Digest;
use sha2::Sha256;

use crate::ModuleKey;
use crate::YankedVersionPolicy;

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct BzlmodModuleFileDigest {
    path: String,
    digest: String,
}

impl BzlmodModuleFileDigest {
    pub fn new(path: impl Into<String>, digest: impl Into<String>) -> Result<Self, String> {
        let input = Self {
            path: path.into(),
            digest: digest.into(),
        };
        input.validate()?;
        Ok(input)
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn digest(&self) -> &str {
        &self.digest
    }

    fn validate(&self) -> Result<(), String> {
        if self.path.is_empty() {
            return Err("module file digest path must not be empty".to_owned());
        }
        if self.path.starts_with('/') || self.path.contains('\\') {
            return Err(format!(
                "module file digest path must be a normalized relative path: {}",
                self.path
            ));
        }
        if self
            .path
            .split('/')
            .any(|segment| segment.is_empty() || segment == "." || segment == "..")
        {
            return Err(format!(
                "module file digest path must be a normalized relative path: {}",
                self.path
            ));
        }
        validate_key_digest("module_file_digest", &self.digest)
    }
}

pub fn digest_module_file_content(content: impl AsRef<[u8]>) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_ref());
    hex::encode(hasher.finalize())
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct BzlmodVisibleLockfileDigest {
    digest: String,
}

impl BzlmodVisibleLockfileDigest {
    pub fn absent() -> Self {
        Self {
            digest: "absent".to_owned(),
        }
    }

    pub fn from_content(content: impl AsRef<[u8]>) -> Self {
        Self {
            digest: format!("present_{}", digest_module_file_content(content)),
        }
    }

    pub fn stable_serialize(&self) -> &str {
        &self.digest
    }
}

impl fmt::Display for BzlmodVisibleLockfileDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.stable_serialize())
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct BzlmodHiddenLockfileDigest {
    digest: String,
}

impl BzlmodHiddenLockfileDigest {
    pub fn absent() -> Self {
        Self {
            digest: "absent".to_owned(),
        }
    }

    pub fn from_content(content: impl AsRef<[u8]>) -> Self {
        Self {
            digest: format!("present_{}", digest_module_file_content(content)),
        }
    }

    pub fn stable_serialize(&self) -> &str {
        &self.digest
    }
}

impl fmt::Display for BzlmodHiddenLockfileDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.stable_serialize())
    }
}

pub fn digest_included_module_files(
    files: impl IntoIterator<Item = BzlmodModuleFileDigest>,
) -> Result<String, String> {
    let mut by_path = BTreeMap::new();
    for file in files {
        if by_path.insert(file.path, file.digest).is_some() {
            return Err("duplicate included module file digest path".to_owned());
        }
    }

    let mut hasher = Sha256::new();
    for (path, digest) in by_path {
        hasher.update(path.as_bytes());
        hasher.update([0]);
        hasher.update(digest.as_bytes());
        hasher.update([0]);
    }
    Ok(hex::encode(hasher.finalize()))
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct BzlmodRegistryModuleFileDigest {
    registry_url: String,
    module: ModuleKey,
    digest: String,
}

impl BzlmodRegistryModuleFileDigest {
    pub fn new(
        registry_url: impl Into<String>,
        module: ModuleKey,
        digest: impl Into<String>,
    ) -> Result<Self, String> {
        let input = Self {
            registry_url: registry_url.into(),
            module,
            digest: digest.into(),
        };
        input.validate()?;
        Ok(input)
    }

    pub fn registry_url(&self) -> &str {
        &self.registry_url
    }

    pub fn module(&self) -> &ModuleKey {
        &self.module
    }

    pub fn digest(&self) -> &str {
        &self.digest
    }

    fn validate(&self) -> Result<(), String> {
        if self.registry_url.is_empty() {
            return Err("registry module file digest URL must not be empty".to_owned());
        }
        if self.registry_url.contains('\0') {
            return Err("registry module file digest URL must not contain NUL bytes".to_owned());
        }
        if self.module.name.is_empty() {
            return Err("registry module file digest module name must not be empty".to_owned());
        }
        if self.module.version.is_empty() {
            return Err("registry module file digest module version must not be empty".to_owned());
        }
        if self.module.name.contains('\0') || self.module.version.contains('\0') {
            return Err(
                "registry module file digest module key must not contain NUL bytes".to_owned(),
            );
        }
        validate_key_digest("registry_module_file_digest", &self.digest)
    }
}

pub fn digest_registry_module_files(
    files: impl IntoIterator<Item = BzlmodRegistryModuleFileDigest>,
) -> Result<String, String> {
    let mut by_identity = BTreeMap::new();
    for file in files {
        let identity = (file.registry_url, file.module);
        if by_identity.insert(identity, file.digest).is_some() {
            return Err("duplicate registry module file digest identity".to_owned());
        }
    }

    let mut hasher = Sha256::new();
    for ((registry_url, module), digest) in by_identity {
        hasher.update(registry_url.as_bytes());
        hasher.update([0]);
        hasher.update(module.to_string().as_bytes());
        hasher.update([0]);
        hasher.update(digest.as_bytes());
        hasher.update([0]);
    }
    Ok(hex::encode(hasher.finalize()))
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct BzlmodRegistrySourceSpecDigest {
    registry_url: String,
    module: ModuleKey,
    digest: String,
}

impl BzlmodRegistrySourceSpecDigest {
    pub fn new(
        registry_url: impl Into<String>,
        module: ModuleKey,
        digest: impl Into<String>,
    ) -> Result<Self, String> {
        let input = Self {
            registry_url: registry_url.into(),
            module,
            digest: digest.into(),
        };
        input.validate()?;
        Ok(input)
    }

    pub fn registry_url(&self) -> &str {
        &self.registry_url
    }

    pub fn module(&self) -> &ModuleKey {
        &self.module
    }

    pub fn digest(&self) -> &str {
        &self.digest
    }

    fn validate(&self) -> Result<(), String> {
        if self.registry_url.is_empty() {
            return Err("registry source spec digest URL must not be empty".to_owned());
        }
        if self.registry_url.contains('\0') {
            return Err("registry source spec digest URL must not contain NUL bytes".to_owned());
        }
        if self.module.name.is_empty() {
            return Err("registry source spec digest module name must not be empty".to_owned());
        }
        if self.module.version.is_empty() {
            return Err("registry source spec digest module version must not be empty".to_owned());
        }
        if self.module.name.contains('\0') || self.module.version.contains('\0') {
            return Err(
                "registry source spec digest module key must not contain NUL bytes".to_owned(),
            );
        }
        validate_key_digest("registry_source_spec_digest", &self.digest)
    }
}

pub fn digest_registry_source_specs(
    files: impl IntoIterator<Item = BzlmodRegistrySourceSpecDigest>,
) -> Result<String, String> {
    let mut by_identity = BTreeMap::new();
    for file in files {
        let identity = (file.registry_url, file.module);
        if by_identity.insert(identity, file.digest).is_some() {
            return Err("duplicate registry source spec digest identity".to_owned());
        }
    }

    let mut hasher = Sha256::new();
    for ((registry_url, module), digest) in by_identity {
        hasher.update(registry_url.as_bytes());
        hasher.update([0]);
        hasher.update(module.to_string().as_bytes());
        hasher.update([0]);
        hasher.update(digest.as_bytes());
        hasher.update([0]);
    }
    Ok(hex::encode(hasher.finalize()))
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct BzlmodExtensionDefinitionDigest {
    extension_id: String,
    digest: String,
}

impl BzlmodExtensionDefinitionDigest {
    pub fn new(extension_id: impl Into<String>, digest: impl Into<String>) -> Result<Self, String> {
        let definition = Self {
            extension_id: extension_id.into(),
            digest: digest.into(),
        };
        definition.validate()?;
        Ok(definition)
    }

    pub fn extension_id(&self) -> &str {
        &self.extension_id
    }

    pub fn digest(&self) -> &str {
        &self.digest
    }

    fn validate(&self) -> Result<(), String> {
        if self.extension_id.is_empty() {
            return Err("module extension definition id must not be empty".to_owned());
        }
        if self.extension_id.contains('\0') {
            return Err("module extension definition id must not contain NUL bytes".to_owned());
        }
        validate_key_digest("extension_definition_digest", &self.digest)
    }
}

pub fn digest_module_extension_definitions(
    definitions: impl IntoIterator<Item = BzlmodExtensionDefinitionDigest>,
) -> Result<String, String> {
    let mut by_extension = BTreeMap::new();
    for definition in definitions {
        if by_extension
            .insert(definition.extension_id, definition.digest)
            .is_some()
        {
            return Err("duplicate module extension definition digest id".to_owned());
        }
    }

    let mut hasher = Sha256::new();
    for (extension_id, digest) in by_extension {
        hasher.update(extension_id.as_bytes());
        hasher.update([0]);
        hasher.update(digest.as_bytes());
        hasher.update([0]);
    }
    Ok(hex::encode(hasher.finalize()))
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct BzlmodExtensionUsageDigest {
    extension_id: String,
    digest: String,
}

impl BzlmodExtensionUsageDigest {
    pub fn new(extension_id: impl Into<String>, digest: impl Into<String>) -> Result<Self, String> {
        let usage = Self {
            extension_id: extension_id.into(),
            digest: digest.into(),
        };
        usage.validate()?;
        Ok(usage)
    }

    pub fn extension_id(&self) -> &str {
        &self.extension_id
    }

    pub fn digest(&self) -> &str {
        &self.digest
    }

    fn validate(&self) -> Result<(), String> {
        if self.extension_id.is_empty() {
            return Err("module extension usage id must not be empty".to_owned());
        }
        if self.extension_id.contains('\0') {
            return Err("module extension usage id must not contain NUL bytes".to_owned());
        }
        validate_key_digest("extension_usage_digest", &self.digest)
    }
}

pub fn digest_module_extension_usages(
    usages: impl IntoIterator<Item = BzlmodExtensionUsageDigest>,
) -> Result<String, String> {
    let mut by_extension = BTreeMap::new();
    for usage in usages {
        if by_extension
            .insert(usage.extension_id, usage.digest)
            .is_some()
        {
            return Err("duplicate module extension usage digest id".to_owned());
        }
    }

    let mut hasher = Sha256::new();
    for (extension_id, digest) in by_extension {
        hasher.update(extension_id.as_bytes());
        hasher.update([0]);
        hasher.update(digest.as_bytes());
        hasher.update([0]);
    }
    Ok(hex::encode(hasher.finalize()))
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct BzlmodGeneratedRepoSpecDigest {
    extension_id: String,
    repo_name: String,
    digest: String,
}

impl BzlmodGeneratedRepoSpecDigest {
    pub fn new(
        extension_id: impl Into<String>,
        repo_name: impl Into<String>,
        digest: impl Into<String>,
    ) -> Result<Self, String> {
        let spec = Self {
            extension_id: extension_id.into(),
            repo_name: repo_name.into(),
            digest: digest.into(),
        };
        spec.validate()?;
        Ok(spec)
    }

    pub fn extension_id(&self) -> &str {
        &self.extension_id
    }

    pub fn repo_name(&self) -> &str {
        &self.repo_name
    }

    pub fn digest(&self) -> &str {
        &self.digest
    }

    fn validate(&self) -> Result<(), String> {
        if self.extension_id.is_empty() {
            return Err("generated repo spec extension id must not be empty".to_owned());
        }
        if self.extension_id.contains('\0') {
            return Err("generated repo spec extension id must not contain NUL bytes".to_owned());
        }
        if self.repo_name.is_empty() {
            return Err("generated repo spec name must not be empty".to_owned());
        }
        if self.repo_name.contains('\0') {
            return Err("generated repo spec name must not contain NUL bytes".to_owned());
        }
        validate_key_digest("generated_repo_spec_digest", &self.digest)
    }
}

pub fn digest_generated_repo_specs(
    specs: impl IntoIterator<Item = BzlmodGeneratedRepoSpecDigest>,
) -> Result<String, String> {
    let mut by_repo = BTreeMap::new();
    for spec in specs {
        if by_repo
            .insert((spec.extension_id, spec.repo_name), spec.digest)
            .is_some()
        {
            return Err("duplicate generated repo spec digest id".to_owned());
        }
    }

    let mut hasher = Sha256::new();
    for ((extension_id, repo_name), digest) in by_repo {
        hasher.update(extension_id.as_bytes());
        hasher.update([0]);
        hasher.update(repo_name.as_bytes());
        hasher.update([0]);
        hasher.update(digest.as_bytes());
        hasher.update([0]);
    }
    Ok(hex::encode(hasher.finalize()))
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct BzlmodRegistryPolicyEntry {
    url: String,
    digest: String,
}

impl BzlmodRegistryPolicyEntry {
    pub fn new(url: impl Into<String>, digest: impl Into<String>) -> Result<Self, String> {
        let entry = Self {
            url: url.into(),
            digest: digest.into(),
        };
        entry.validate()?;
        Ok(entry)
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn digest(&self) -> &str {
        &self.digest
    }

    fn validate(&self) -> Result<(), String> {
        if self.url.is_empty() {
            return Err("registry policy entry URL must not be empty".to_owned());
        }
        if self.url.contains('\0') {
            return Err("registry policy entry URL must not contain NUL bytes".to_owned());
        }
        validate_key_digest("registry_policy_entry_digest", &self.digest)
    }
}

pub fn digest_registry_policy(
    entries: impl IntoIterator<Item = BzlmodRegistryPolicyEntry>,
) -> String {
    let mut hasher = Sha256::new();
    for (index, entry) in entries.into_iter().enumerate() {
        hasher.update(index.to_string().as_bytes());
        hasher.update([0]);
        hasher.update(entry.url.as_bytes());
        hasher.update([0]);
        hasher.update(entry.digest.as_bytes());
        hasher.update([0]);
    }
    hex::encode(hasher.finalize())
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum LockfileMode {
    Off,
    Update,
    Refresh,
    Error,
}

impl LockfileMode {
    pub fn from_bazel_flag_value(value: &str) -> Result<Self, String> {
        match value {
            "off" => Ok(Self::Off),
            "update" => Ok(Self::Update),
            "refresh" => Ok(Self::Refresh),
            "error" => Ok(Self::Error),
            other => Err(format!(
                "Not a valid Lockfile mode: '{other}' (should be off, update, refresh or error)"
            )),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Update => "update",
            Self::Refresh => "refresh",
            Self::Error => "error",
        }
    }

    pub fn reads_visible_lockfile(&self) -> bool {
        matches!(self, Self::Update | Self::Refresh | Self::Error)
    }

    pub fn writes_visible_lockfile(&self) -> bool {
        matches!(self, Self::Update | Self::Refresh)
    }
}

impl fmt::Display for LockfileMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct BzlmodCommandPolicyKey {
    yanked_versions_policy: YankedVersionPolicy,
}

impl BzlmodCommandPolicyKey {
    pub fn new(yanked_versions_policy: YankedVersionPolicy) -> Self {
        Self {
            yanked_versions_policy,
        }
    }

    pub fn from_allow_yanked_versions_flag(value: Option<&str>) -> Result<Self, String> {
        Ok(Self::new(YankedVersionPolicy::from_flag_value(value)?))
    }

    pub fn yanked_versions_policy(&self) -> &YankedVersionPolicy {
        &self.yanked_versions_policy
    }

    pub fn stable_serialize(&self) -> String {
        serialize_yanked_policy(&self.yanked_versions_policy)
    }
}

impl fmt::Display for BzlmodCommandPolicyKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.stable_serialize())
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct BzlmodEnvironmentPolicyKey {
    yanked_versions_policy: YankedVersionPolicy,
}

impl BzlmodEnvironmentPolicyKey {
    pub fn new(yanked_versions_policy: YankedVersionPolicy) -> Self {
        Self {
            yanked_versions_policy,
        }
    }

    pub fn from_bzlmod_allow_yanked_versions(value: Option<&str>) -> Result<Self, String> {
        Ok(Self::new(YankedVersionPolicy::from_env_value(value)?))
    }

    pub fn yanked_versions_policy(&self) -> &YankedVersionPolicy {
        &self.yanked_versions_policy
    }

    pub fn stable_serialize(&self) -> String {
        serialize_yanked_policy(&self.yanked_versions_policy)
    }
}

impl fmt::Display for BzlmodEnvironmentPolicyKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.stable_serialize())
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct BzlmodDiceInputs {
    root_module_digest: String,
    included_module_digest: String,
    registry_policy_digest: String,
    registry_module_digest: String,
    registry_source_digest: String,
    extension_definition_digest: String,
    extension_usage_digest: String,
    generated_repo_spec_digest: String,
    lockfile_digest: String,
    hidden_lockfile_digest: String,
    lockfile_mode: LockfileMode,
    command_policy: BzlmodCommandPolicyKey,
    environment_policy: BzlmodEnvironmentPolicyKey,
}

impl BzlmodDiceInputs {
    pub fn new(
        root_module_digest: impl Into<String>,
        included_module_digest: impl Into<String>,
        registry_policy_digest: impl Into<String>,
        registry_module_digest: impl Into<String>,
        registry_source_digest: impl Into<String>,
        extension_definition_digest: impl Into<String>,
        extension_usage_digest: impl Into<String>,
        lockfile_digest: impl Into<String>,
        lockfile_mode: LockfileMode,
        command_policy: BzlmodCommandPolicyKey,
        environment_policy: BzlmodEnvironmentPolicyKey,
    ) -> Result<Self, String> {
        Self::new_with_generated_repo_specs(
            root_module_digest,
            included_module_digest,
            registry_policy_digest,
            registry_module_digest,
            registry_source_digest,
            extension_definition_digest,
            extension_usage_digest,
            digest_generated_repo_specs(std::iter::empty::<BzlmodGeneratedRepoSpecDigest>())?,
            lockfile_digest,
            lockfile_mode,
            command_policy,
            environment_policy,
        )
    }

    pub fn new_with_hidden_lockfile(
        root_module_digest: impl Into<String>,
        included_module_digest: impl Into<String>,
        registry_policy_digest: impl Into<String>,
        registry_module_digest: impl Into<String>,
        registry_source_digest: impl Into<String>,
        extension_definition_digest: impl Into<String>,
        extension_usage_digest: impl Into<String>,
        lockfile_digest: impl Into<String>,
        hidden_lockfile_digest: impl Into<String>,
        lockfile_mode: LockfileMode,
        command_policy: BzlmodCommandPolicyKey,
        environment_policy: BzlmodEnvironmentPolicyKey,
    ) -> Result<Self, String> {
        Self::new_with_hidden_lockfile_and_generated_repo_specs(
            root_module_digest,
            included_module_digest,
            registry_policy_digest,
            registry_module_digest,
            registry_source_digest,
            extension_definition_digest,
            extension_usage_digest,
            digest_generated_repo_specs(std::iter::empty::<BzlmodGeneratedRepoSpecDigest>())?,
            lockfile_digest,
            hidden_lockfile_digest,
            lockfile_mode,
            command_policy,
            environment_policy,
        )
    }

    pub fn new_with_generated_repo_specs(
        root_module_digest: impl Into<String>,
        included_module_digest: impl Into<String>,
        registry_policy_digest: impl Into<String>,
        registry_module_digest: impl Into<String>,
        registry_source_digest: impl Into<String>,
        extension_definition_digest: impl Into<String>,
        extension_usage_digest: impl Into<String>,
        generated_repo_spec_digest: impl Into<String>,
        lockfile_digest: impl Into<String>,
        lockfile_mode: LockfileMode,
        command_policy: BzlmodCommandPolicyKey,
        environment_policy: BzlmodEnvironmentPolicyKey,
    ) -> Result<Self, String> {
        Self::new_with_hidden_lockfile_and_generated_repo_specs(
            root_module_digest,
            included_module_digest,
            registry_policy_digest,
            registry_module_digest,
            registry_source_digest,
            extension_definition_digest,
            extension_usage_digest,
            generated_repo_spec_digest,
            lockfile_digest,
            BzlmodHiddenLockfileDigest::absent()
                .stable_serialize()
                .to_owned(),
            lockfile_mode,
            command_policy,
            environment_policy,
        )
    }

    pub fn new_with_hidden_lockfile_and_generated_repo_specs(
        root_module_digest: impl Into<String>,
        included_module_digest: impl Into<String>,
        registry_policy_digest: impl Into<String>,
        registry_module_digest: impl Into<String>,
        registry_source_digest: impl Into<String>,
        extension_definition_digest: impl Into<String>,
        extension_usage_digest: impl Into<String>,
        generated_repo_spec_digest: impl Into<String>,
        lockfile_digest: impl Into<String>,
        hidden_lockfile_digest: impl Into<String>,
        lockfile_mode: LockfileMode,
        command_policy: BzlmodCommandPolicyKey,
        environment_policy: BzlmodEnvironmentPolicyKey,
    ) -> Result<Self, String> {
        let inputs = Self {
            root_module_digest: root_module_digest.into(),
            included_module_digest: included_module_digest.into(),
            registry_policy_digest: registry_policy_digest.into(),
            registry_module_digest: registry_module_digest.into(),
            registry_source_digest: registry_source_digest.into(),
            extension_definition_digest: extension_definition_digest.into(),
            extension_usage_digest: extension_usage_digest.into(),
            generated_repo_spec_digest: generated_repo_spec_digest.into(),
            lockfile_digest: lockfile_digest.into(),
            hidden_lockfile_digest: hidden_lockfile_digest.into(),
            lockfile_mode,
            command_policy,
            environment_policy,
        };
        inputs.validate()?;
        Ok(inputs)
    }

    pub fn lockfile_mode(&self) -> &LockfileMode {
        &self.lockfile_mode
    }

    pub fn command_policy(&self) -> &BzlmodCommandPolicyKey {
        &self.command_policy
    }

    pub fn environment_policy(&self) -> &BzlmodEnvironmentPolicyKey {
        &self.environment_policy
    }

    pub fn effective_yanked_versions_policy(&self) -> YankedVersionPolicy {
        self.command_policy
            .yanked_versions_policy()
            .union(self.environment_policy.yanked_versions_policy())
    }

    pub fn stable_serialize(&self) -> String {
        format!(
            "root={};includes={};registries={};registry_modules={};registry_sources={};extension_defs={};extensions={};generated_repos={};lockfile={};hidden_lockfile={};mode={};command={};env={}",
            self.root_module_digest,
            self.included_module_digest,
            self.registry_policy_digest,
            self.registry_module_digest,
            self.registry_source_digest,
            self.extension_definition_digest,
            self.extension_usage_digest,
            self.generated_repo_spec_digest,
            self.lockfile_digest,
            self.hidden_lockfile_digest,
            self.lockfile_mode,
            self.command_policy,
            self.environment_policy
        )
    }

    fn validate(&self) -> Result<(), String> {
        for (name, value) in [
            ("root_module_digest", &self.root_module_digest),
            ("included_module_digest", &self.included_module_digest),
            ("registry_policy_digest", &self.registry_policy_digest),
            ("registry_module_digest", &self.registry_module_digest),
            ("registry_source_digest", &self.registry_source_digest),
            (
                "extension_definition_digest",
                &self.extension_definition_digest,
            ),
            ("extension_usage_digest", &self.extension_usage_digest),
            (
                "generated_repo_spec_digest",
                &self.generated_repo_spec_digest,
            ),
            ("lockfile_digest", &self.lockfile_digest),
            ("hidden_lockfile_digest", &self.hidden_lockfile_digest),
        ] {
            validate_key_digest(name, value)?;
        }
        Ok(())
    }
}

impl fmt::Display for BzlmodDiceInputs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.stable_serialize())
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ResolvedBzlmodGraphDiceKey {
    root_module: ModuleKey,
    inputs: BzlmodDiceInputs,
}

impl ResolvedBzlmodGraphDiceKey {
    pub fn new(root_module: ModuleKey, inputs: BzlmodDiceInputs) -> Self {
        Self {
            root_module,
            inputs,
        }
    }

    pub fn root_module(&self) -> &ModuleKey {
        &self.root_module
    }

    pub fn inputs(&self) -> &BzlmodDiceInputs {
        &self.inputs
    }

    pub fn stable_serialize(&self) -> String {
        format!("{} {{{}}}", self.root_module, self.inputs)
    }
}

impl fmt::Display for ResolvedBzlmodGraphDiceKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.stable_serialize())
    }
}

fn validate_key_digest(name: &str, value: &str) -> Result<(), String> {
    if value.is_empty() {
        return Err(format!("{name} must not be empty"));
    }
    if !value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
    {
        return Err(format!("invalid {name}: {value}"));
    }
    Ok(())
}

fn serialize_yanked_policy(policy: &YankedVersionPolicy) -> String {
    match policy {
        YankedVersionPolicy::Reject => "allow_yanked=reject".to_owned(),
        YankedVersionPolicy::AllowAll => "allow_yanked=all".to_owned(),
        YankedVersionPolicy::AllowList(allowed) => {
            let entries = allowed
                .iter()
                .map(ModuleKey::to_string)
                .collect::<Vec<_>>()
                .join(",");
            format!("allow_yanked=[{entries}]")
        }
    }
}
