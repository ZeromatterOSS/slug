/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory or the Apache License, Version 2.0
 * found in the LICENSE-APACHE file in the root directory of this source tree.
 * You may select, at your option, one of the above-listed licenses.
 */

#![allow(dead_code)] // Dormant until the later Host registry-file packet.

use std::fmt;
use std::net::Ipv6Addr;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use compact_str::CompactString;
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::ObservedPathFrontierError;
use slug_workspace_v2::PathObservationEpoch;
use slug_workspace_v2::PathOutcome;

use crate::BazelLockfile;
use crate::LockfileMode;
use crate::RegistryFileExpectation;
use crate::RootPackagePolicyProjectionError;
use crate::host_lockfile::HostVisibleLockfileError;
use crate::host_lockfile::HostVisibleLockfileKey;
use crate::host_lockfile::HostVisibleLockfileObservationKey;
use crate::host_lockfile::HostVisibleLockfileValue;
use crate::host_lockfile::ObservedHostVisibleLockfile;
use crate::host_registry_inputs::HostModuleMirrorsInputKey;
use crate::host_registry_inputs::HostRegistryRefreshToken;
use crate::host_registry_inputs::HostRegistryRefreshTokenKey;
use crate::module_eval::RootModuleLockfileModeKey;
use crate::package_policy::RootVendorDirectoryProjectionKey;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Allocative, Dupe)]
pub(crate) enum HostRegistryScheme {
    Http,
    Https,
    File,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Allocative, Dupe)]
pub(crate) enum RegistryKnownFileHashesMode {
    Ignore,
    UseAndUpdate,
    UseImmutableAndUpdate,
    Enforce,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Allocative, Dupe)]
pub(crate) enum HostRegistryUriErrorKind {
    InvalidSyntax,
    MissingScheme,
    MissingPath,
    UnrecognizedProtocol,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) enum HostRegistryFunctionError {
    LockfileModeInput {
        workspace: NormalizedAbsolutePath,
    },
    VendorDirectoryInput {
        error: RootPackagePolicyProjectionError,
    },
    RefreshTokenInput {
        workspace: NormalizedAbsolutePath,
    },
    VisibleLockfile {
        error: HostVisibleLockfileError,
    },
    ModuleMirrorsInput {
        workspace: NormalizedAbsolutePath,
    },
    InvalidPrimaryRegistryUri {
        original_registry: CompactString,
        resolved_registry: CompactString,
        reason: HostRegistryUriErrorKind,
    },
    InvalidModuleMirrorUri {
        original_registry: CompactString,
        mirror: CompactString,
        ordinal: u32,
    },
}

#[derive(Debug, Clone, Allocative)]
pub(crate) struct HostRegistryFunctionValue {
    original_registry: CompactString,
    resolved_registry: CompactString,
    scheme: HostRegistryScheme,
    known_file_hashes_mode: RegistryKnownFileHashesMode,
    lockfile: Arc<BazelLockfile>,
    vendor_directory: Option<NormalizedAbsolutePath>,
    module_mirrors: Arc<[CompactString]>,
    refresh_token: Option<HostRegistryRefreshToken>,
}

impl PartialEq for HostRegistryFunctionValue {
    fn eq(&self, other: &Self) -> bool {
        self.original_registry == other.original_registry
            && self.resolved_registry == other.resolved_registry
            && self.scheme == other.scheme
            && self.known_file_hashes_mode == other.known_file_hashes_mode
            && self.vendor_directory == other.vendor_directory
            && self.module_mirrors == other.module_mirrors
            && self.refresh_token == other.refresh_token
            && self.lockfile.registry_file_hashes == other.lockfile.registry_file_hashes
            && self.lockfile.selected_yanked_versions == other.lockfile.selected_yanked_versions
    }
}

impl Eq for HostRegistryFunctionValue {}

impl HostRegistryFunctionValue {
    pub(crate) fn original_registry(&self) -> &str {
        &self.original_registry
    }

    pub(crate) fn resolved_registry(&self) -> &str {
        &self.resolved_registry
    }

    pub(crate) fn scheme(&self) -> HostRegistryScheme {
        self.scheme
    }

    pub(crate) fn known_file_hashes_mode(&self) -> RegistryKnownFileHashesMode {
        self.known_file_hashes_mode
    }

    pub(crate) fn vendor_directory(&self) -> Option<&NormalizedAbsolutePath> {
        self.vendor_directory.as_ref()
    }

    pub(crate) fn module_mirrors(&self) -> &[CompactString] {
        &self.module_mirrors
    }

    pub(crate) fn registry_file_expectation(
        &self,
        url: &str,
    ) -> Result<RegistryFileExpectation, String> {
        self.lockfile.registry_file_expectation(url)
    }

    pub(crate) fn selected_yanked_reason(
        &self,
        name: &str,
        canonical_version: &str,
    ) -> Option<&str> {
        self.lockfile
            .selected_yanked_versions
            .iter()
            .find_map(|(key, reason)| match key {
                crate::lockfile_v28::LockfileModuleKey::Module {
                    name: found,
                    version,
                } if found == name && version.canonical == canonical_version => {
                    Some(reason.as_str())
                }
                _ => None,
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(crate) struct HostRegistryFunctionKey {
    workspace: NormalizedAbsolutePath,
    original_registry: CompactString,
}

impl HostRegistryFunctionKey {
    pub(crate) fn new(
        workspace: NormalizedAbsolutePath,
        original_registry: impl Into<CompactString>,
    ) -> Self {
        Self {
            workspace,
            original_registry: original_registry.into(),
        }
    }
}

impl fmt::Display for HostRegistryFunctionKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "host-registry:{}:{}",
            self.workspace, self.original_registry
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(crate) struct ObservedHostRegistryFunction {
    result: Arc<Result<HostRegistryFunctionValue, HostRegistryFunctionError>>,
    observations: PathObservationEpoch,
}

impl ObservedHostRegistryFunction {
    pub(crate) fn result(
        &self,
    ) -> &Arc<Result<HostRegistryFunctionValue, HostRegistryFunctionError>> {
        &self.result
    }

    pub(crate) fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(crate) struct HostRegistryFunctionObservationKey(HostRegistryFunctionKey);

impl HostRegistryFunctionObservationKey {
    pub(crate) fn new(
        workspace: NormalizedAbsolutePath,
        original_registry: impl Into<CompactString>,
    ) -> Self {
        Self(HostRegistryFunctionKey::new(workspace, original_registry))
    }
}

impl fmt::Display for HostRegistryFunctionObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

#[derive(Clone, Copy)]
enum HostRegistryFunctionDriverMode {
    Legacy,
    Observed,
}

type HostRegistryFunctionDriverOutcome =
    PathOutcome<Result<ObservedHostRegistryFunction, ObservedPathFrontierError>>;

fn observed_complete(
    result: Result<HostRegistryFunctionValue, HostRegistryFunctionError>,
    observations: PathObservationEpoch,
) -> HostRegistryFunctionDriverOutcome {
    PathOutcome::Complete(Ok(ObservedHostRegistryFunction {
        result: Arc::new(result),
        observations,
    }))
}

#[track_caller]
fn dice_invariant<T, E: fmt::Debug>(result: Result<T, E>) -> T {
    result.unwrap_or_else(|error| panic!("Host RegistryFunction DICE invariant failed: {error:?}"))
}

fn project_legacy_host_registry(
    value: HostRegistryFunctionDriverOutcome,
) -> PathOutcome<Arc<Result<HostRegistryFunctionValue, HostRegistryFunctionError>>> {
    match value {
        PathOutcome::Need(need) => PathOutcome::Need(need),
        PathOutcome::Complete(Ok(value)) => PathOutcome::Complete(value.result),
        PathOutcome::Complete(Err(error)) => {
            panic!("legacy Host RegistryFunction produced observed outer: {error:?}")
        }
    }
}

fn finish_observed_visible_lockfile(
    value: PathOutcome<Result<ObservedHostVisibleLockfile, ObservedPathFrontierError>>,
) -> Result<
    (
        Arc<Result<HostVisibleLockfileValue, HostVisibleLockfileError>>,
        PathObservationEpoch,
    ),
    HostRegistryFunctionDriverOutcome,
> {
    match value {
        PathOutcome::Need(need) => Err(PathOutcome::Need(need)),
        PathOutcome::Complete(Err(error)) => Err(PathOutcome::Complete(Err(error))),
        PathOutcome::Complete(Ok(value)) => {
            Ok((value.result().dupe(), value.observations().dupe()))
        }
    }
}

async fn drive_host_registry_function(
    ctx: &mut DiceComputations<'_>,
    key: &HostRegistryFunctionKey,
    mode: HostRegistryFunctionDriverMode,
) -> HostRegistryFunctionDriverOutcome {
    let empty = PathObservationEpoch::empty();
    let lockfile_mode = match ctx
        .compute(&RootModuleLockfileModeKey {
            workspace: key.workspace.as_path().to_path_buf(),
        })
        .await
    {
        Ok(mode) => mode.semantic_mode(),
        Err(_) => {
            return observed_complete(
                Err(HostRegistryFunctionError::LockfileModeInput {
                    workspace: key.workspace.dupe(),
                }),
                empty,
            );
        }
    };
    let vendor_directory = match dice_invariant(
        ctx.compute(&RootVendorDirectoryProjectionKey::new(key.workspace.dupe()))
            .await,
    ) {
        Ok(vendor) => vendor,
        Err(error) => {
            return observed_complete(
                Err(HostRegistryFunctionError::VendorDirectoryInput { error }),
                empty,
            );
        }
    };
    let refresh_token = if lockfile_mode == LockfileMode::Refresh {
        match ctx
            .compute(&HostRegistryRefreshTokenKey::new(key.workspace.dupe()))
            .await
        {
            Ok(token) => Some(token),
            Err(_) => {
                return observed_complete(
                    Err(HostRegistryFunctionError::RefreshTokenInput {
                        workspace: key.workspace.dupe(),
                    }),
                    empty,
                );
            }
        }
    } else {
        None
    };
    let (visible, observations) = match mode {
        HostRegistryFunctionDriverMode::Legacy => match dice_invariant(
            ctx.compute(&HostVisibleLockfileKey::new(key.workspace.dupe()))
                .await,
        ) {
            PathOutcome::Need(need) => return PathOutcome::Need(need),
            PathOutcome::Complete(value) => (value, empty),
        },
        HostRegistryFunctionDriverMode::Observed => {
            match finish_observed_visible_lockfile(dice_invariant(
                ctx.compute(&HostVisibleLockfileObservationKey::new(
                    key.workspace.dupe(),
                ))
                .await,
            )) {
                Ok(visible) => visible,
                Err(terminal) => return terminal,
            }
        }
    };
    let lockfile = match visible.as_ref() {
        Ok(value) => value.lockfile().dupe(),
        Err(error) => {
            return observed_complete(
                Err(HostRegistryFunctionError::VisibleLockfile {
                    error: error.clone(),
                }),
                observations,
            );
        }
    };
    let resolved_registry = resolved_registry_spelling(&key.workspace, &key.original_registry);
    let module_mirrors = match ctx
        .compute(&HostModuleMirrorsInputKey::new(key.workspace.dupe()))
        .await
    {
        Ok(mirrors) => mirrors,
        Err(_) => {
            return observed_complete(
                Err(HostRegistryFunctionError::ModuleMirrorsInput {
                    workspace: key.workspace.dupe(),
                }),
                observations,
            );
        }
    };
    let selected_mirrors = module_mirrors
        .get(&key.original_registry)
        .map(|mirrors| mirrors.iter().cloned().collect::<Arc<[_]>>())
        .unwrap_or_else(|| Arc::from([]));
    let scheme = match parse_primary_registry_uri(&resolved_registry) {
        Ok(scheme) => scheme,
        Err(reason) => {
            return observed_complete(
                Err(HostRegistryFunctionError::InvalidPrimaryRegistryUri {
                    original_registry: key.original_registry.clone(),
                    resolved_registry,
                    reason,
                }),
                observations,
            );
        }
    };
    let known_file_hashes_mode = known_file_hashes_mode(scheme, &lockfile_mode);
    for (index, mirror) in selected_mirrors.iter().enumerate() {
        if validate_module_mirror_uri(mirror).is_err() {
            return observed_complete(
                Err(HostRegistryFunctionError::InvalidModuleMirrorUri {
                    original_registry: key.original_registry.clone(),
                    mirror: mirror.clone(),
                    ordinal: u32::try_from(index)
                        .expect("module mirror count must fit the converter's command vector"),
                }),
                observations,
            );
        }
    }
    observed_complete(
        Ok(HostRegistryFunctionValue {
            original_registry: key.original_registry.clone(),
            resolved_registry,
            scheme,
            known_file_hashes_mode,
            lockfile,
            vendor_directory,
            module_mirrors: selected_mirrors,
            refresh_token,
        }),
        observations,
    )
}

fn resolved_registry_spelling(
    workspace: &NormalizedAbsolutePath,
    original_registry: &str,
) -> CompactString {
    #[cfg(windows)]
    let workspace = workspace.as_path().to_string_lossy().replace('\\', "/");
    #[cfg(not(windows))]
    let workspace = workspace.as_path().to_string_lossy().into_owned();
    original_registry.replace("%workspace%", &workspace).into()
}

fn forbidden_uri_character(character: char) -> bool {
    character.is_whitespace()
        || character.is_control()
        || matches!(
            character,
            '"' | '<' | '>' | '\\' | '^' | '`' | '{' | '|' | '}'
        )
}

fn validate_percent_escapes(value: &str) -> bool {
    let bytes = value.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'%' {
            index += 1;
            continue;
        }
        if index + 2 >= bytes.len()
            || !bytes[index + 1].is_ascii_hexdigit()
            || !bytes[index + 2].is_ascii_hexdigit()
        {
            return false;
        }
        index += 3;
    }
    true
}

fn scheme_end(value: &str) -> Result<Option<usize>, ()> {
    let delimiter = value
        .char_indices()
        .find(|(_, character)| matches!(character, ':' | '/' | '?' | '#'));
    let Some((index, delimiter)) = delimiter else {
        return Ok(None);
    };
    if delimiter != ':' {
        return Ok(None);
    }
    let scheme = &value[..index];
    let mut characters = scheme.chars();
    if !characters
        .next()
        .is_some_and(|character| character.is_ascii_alphabetic())
        || !characters.all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '+' | '-' | '.')
        })
    {
        return Err(());
    }
    Ok(Some(index))
}

fn validate_bracket_authority(authority: &str) -> Result<Option<(usize, usize)>, ()> {
    let Some(open) = authority.find('[') else {
        return if authority.contains(']') {
            Err(())
        } else {
            Ok(None)
        };
    };
    let close = authority[open + 1..].find(']').ok_or(())? + open + 1;
    if authority[close + 1..].contains(['[', ']']) {
        return Err(());
    }
    let userinfo = &authority[..open];
    if !userinfo.is_empty()
        && (!userinfo.ends_with('@') || userinfo[..userinfo.len() - 1].contains(['@', '[', ']']))
    {
        return Err(());
    }
    let literal = &authority[open + 1..close];
    let (address, scope) = literal
        .split_once('%')
        .map_or((literal, None), |(address, scope)| (address, Some(scope)));
    if address.parse::<Ipv6Addr>().is_err()
        || scope.is_some_and(|scope| {
            scope.is_empty()
                || !scope
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.'))
        })
    {
        return Err(());
    }
    let suffix = &authority[close + 1..];
    if !suffix.is_empty() {
        let port = suffix.strip_prefix(':').ok_or(())?;
        if !port.is_empty()
            && (!port.bytes().all(|byte| byte.is_ascii_digit()) || port.parse::<i32>().is_err())
        {
            return Err(());
        }
    }
    Ok(Some((open, close)))
}

fn validate_brackets(value: &str, scheme_end: Option<usize>) -> Result<Option<(usize, usize)>, ()> {
    let hierarchy_base = scheme_end.map_or(0, |index| index + 1);
    let hierarchy = &value[hierarchy_base..];
    if scheme_end.is_some() && !hierarchy.starts_with('/') {
        return Ok(None);
    }
    let hierarchy_end = hierarchy.find(['?', '#']).unwrap_or(hierarchy.len());
    let hierarchy = &hierarchy[..hierarchy_end];
    let Some(authority) = hierarchy.strip_prefix("//") else {
        return if hierarchy.contains(['[', ']']) {
            Err(())
        } else {
            Ok(None)
        };
    };
    let authority_end = authority.find('/').unwrap_or(authority.len());
    let authority = &authority[..authority_end];
    let suffix = &hierarchy[2 + authority_end..];
    if suffix.contains(['[', ']']) {
        return Err(());
    }
    validate_bracket_authority(authority).map(|bounds| {
        bounds.map(|(open, close)| {
            let authority_offset = hierarchy_base + 2;
            (authority_offset + open, authority_offset + close)
        })
    })
}

fn validate_java_uri_syntax(value: &str) -> Result<Option<usize>, ()> {
    if value.chars().any(forbidden_uri_character) || value.matches('#').count() > 1 {
        return Err(());
    }
    let scheme_end = scheme_end(value)?;
    if scheme_end.is_some_and(|index| {
        let specific = &value[index + 1..value.find('#').unwrap_or(value.len())];
        specific.is_empty() || &value[index + 1..] == "//"
    }) || (scheme_end.is_none() && value == "//")
    {
        return Err(());
    }
    if let Some((open, close)) = validate_brackets(value, scheme_end)? {
        if !validate_percent_escapes(&value[..open])
            || !validate_percent_escapes(&value[close + 1..])
        {
            return Err(());
        }
    } else if !validate_percent_escapes(value) {
        return Err(());
    }
    Ok(scheme_end)
}

fn parse_primary_registry_uri(value: &str) -> Result<HostRegistryScheme, HostRegistryUriErrorKind> {
    let scheme_end =
        validate_java_uri_syntax(value).map_err(|_| HostRegistryUriErrorKind::InvalidSyntax)?;
    let scheme_end = scheme_end.ok_or(HostRegistryUriErrorKind::MissingScheme)?;
    let scheme = &value[..scheme_end];
    let scheme_specific = &value[scheme_end + 1..];
    if scheme_specific.is_empty() {
        return Err(HostRegistryUriErrorKind::InvalidSyntax);
    }
    if !scheme_specific.starts_with('/') {
        return Err(HostRegistryUriErrorKind::MissingPath);
    }
    match scheme {
        "http" => Ok(HostRegistryScheme::Http),
        "https" => Ok(HostRegistryScheme::Https),
        "file" => Ok(HostRegistryScheme::File),
        _ => Err(HostRegistryUriErrorKind::UnrecognizedProtocol),
    }
}

fn validate_module_mirror_uri(value: &str) -> Result<(), ()> {
    validate_java_uri_syntax(value).map(|_| ())
}

fn known_file_hashes_mode(
    scheme: HostRegistryScheme,
    lockfile_mode: &LockfileMode,
) -> RegistryKnownFileHashesMode {
    match scheme {
        HostRegistryScheme::File => RegistryKnownFileHashesMode::Ignore,
        HostRegistryScheme::Http | HostRegistryScheme::Https => match lockfile_mode {
            LockfileMode::Off | LockfileMode::Update => RegistryKnownFileHashesMode::UseAndUpdate,
            LockfileMode::Refresh => RegistryKnownFileHashesMode::UseImmutableAndUpdate,
            LockfileMode::Error => RegistryKnownFileHashesMode::Enforce,
        },
    }
}

#[async_trait]
impl Key for HostRegistryFunctionKey {
    type Value = PathOutcome<Arc<Result<HostRegistryFunctionValue, HostRegistryFunctionError>>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        project_legacy_host_registry(
            drive_host_registry_function(ctx, self, HostRegistryFunctionDriverMode::Legacy).await,
        )
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[async_trait]
impl Key for HostRegistryFunctionObservationKey {
    type Value = HostRegistryFunctionDriverOutcome;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        drive_host_registry_function(ctx, &self.0, HostRegistryFunctionDriverMode::Observed).await
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}
#[cfg(test)]
mod tests {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::Hash;
    use std::hash::Hasher;
    use std::sync::Mutex;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    use dice::ActivationData;
    use dice::ActivationTracker;
    use dice::DetectCycles;
    use dice::Dice;
    use dice::DiceTransaction;
    use dice::DiceTransactionUpdater;
    use dice::DynKey;
    use dice::InjectedKey;
    use dice::RichActivation;
    use dice::UserComputationData;
    use slug_workspace_v2::NeedPathObservations;
    use slug_workspace_v2::PathLstat;
    use slug_workspace_v2::PathNodeKind;
    use slug_workspace_v2::PathObservationDemand;
    use slug_workspace_v2::PathObservationEpoch;
    use slug_workspace_v2::PathObservationEpochKey;
    use slug_workspace_v2::PathObservationNamespace;
    use slug_workspace_v2::PathObservationOperation;
    use slug_workspace_v2::PathObservationResult;
    use slug_workspace_v2::PathOperationResult;

    use super::*;
    use crate::RootModuleLockfileMode;
    use crate::RootPackagePolicyInputs;
    use crate::host_registry_inputs::HostModuleMirrorOccurrence;
    use crate::host_registry_inputs::HostModuleMirrorsInput;
    use crate::host_registry_inputs::normalize_host_registry_inputs;
    use crate::inject_root_package_policy_inputs;

    const WORKSPACE: &str = "/workspace";
    const LOCKFILE: &str = "/workspace/MODULE.bazel.lock";
    const REGISTRY_A: &str = "https://a.example";
    const REGISTRY_B: &str = "https://b.example";

    fn path(value: &str) -> NormalizedAbsolutePath {
        NormalizedAbsolutePath::new(value).unwrap()
    }

    fn demand(value: &str, operation: PathObservationOperation) -> PathObservationDemand {
        PathObservationDemand::new(PathObservationNamespace::Host, path(value), operation)
    }

    fn lstat(
        value: &str,
        kind: PathNodeKind,
        variant: i64,
    ) -> (PathObservationDemand, PathObservationResult) {
        (
            demand(value, PathObservationOperation::Lstat),
            PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
                kind,
                variant,
                variant + 1,
                variant + 2,
                variant + 3,
                0o755,
            ))),
        )
    }

    fn complete_epoch(bytes: &[u8], variant: i64) -> PathObservationEpoch {
        PathObservationEpoch::new([
            lstat("/", PathNodeKind::Directory, 1),
            lstat(WORKSPACE, PathNodeKind::Directory, 2),
            lstat(LOCKFILE, PathNodeKind::RegularFile, variant),
            (
                demand(LOCKFILE, PathObservationOperation::FileBytes),
                PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(bytes))),
            ),
        ])
        .unwrap()
    }

    fn empty_epoch() -> PathObservationEpoch {
        PathObservationEpoch::new(std::iter::empty()).unwrap()
    }

    fn policy(vendor: Option<&str>) -> RootPackagePolicyInputs {
        RootPackagePolicyInputs::new(
            path(WORKSPACE),
            Arc::from([path(WORKSPACE)]),
            std::iter::empty::<&str>(),
            vendor.map(path),
            Some("warning"),
        )
        .unwrap()
    }

    fn mirrors(registries: &[&str], occurrences: &[(&str, &[&str])]) -> HostModuleMirrorsInput {
        let occurrences = occurrences
            .iter()
            .map(|(registry, mirrors)| {
                HostModuleMirrorOccurrence::new(
                    *registry,
                    mirrors
                        .iter()
                        .map(|mirror| CompactString::new(*mirror))
                        .collect::<Arc<[_]>>(),
                )
            })
            .collect::<Vec<_>>();
        normalize_host_registry_inputs(registries.iter().copied(), occurrences)
            .unwrap()
            .1
    }

    fn lockfile_source(
        hash_byte: u8,
        yanked_reason: &str,
        module_extensions: &str,
        facts: &str,
        facts_versions: &str,
    ) -> Vec<u8> {
        format!(
            r#"{{
              "lockFileVersion": 28,
              "registryFileHashes": {{"u": "{}"}},
              "selectedYankedVersions": {{
                "subject@1.0.0": "{yanked_reason}",
                "a_very_long_module_name_that_cannot_fit_inline@123456789.987654321.111111111": "long-reason"
              }},
              "moduleExtensions": {module_extensions},
              "facts": {facts},
              "factsVersions": {facts_versions}
            }}"#,
            format!("{hash_byte:02x}").repeat(32),
        )
        .into_bytes()
    }

    fn base_lockfile() -> Vec<u8> {
        lockfile_source(0xab, "reason-a", "{}", "{}", "{}")
    }

    #[derive(Clone)]
    struct Setup {
        mode: Option<LockfileMode>,
        vendor: Option<Option<CompactString>>,
        token: Option<u64>,
        mirrors: Option<HostModuleMirrorsInput>,
        epoch: PathObservationEpoch,
    }

    impl Setup {
        fn ready(mode: LockfileMode, registries: &[&str]) -> Self {
            Self {
                mode: Some(mode),
                vendor: Some(None),
                token: Some(1),
                mirrors: Some(mirrors(registries, &[])),
                epoch: complete_epoch(&base_lockfile(), 10),
            }
        }
    }

    fn apply_setup(
        updater: &mut DiceTransactionUpdater,
        workspace: &NormalizedAbsolutePath,
        setup: Setup,
    ) {
        updater
            .changed_to(vec![(PathObservationEpochKey, setup.epoch)])
            .unwrap();
        if let Some(mode) = setup.mode {
            updater
                .changed_to(vec![(
                    RootModuleLockfileModeKey {
                        workspace: workspace.as_path().to_path_buf(),
                    },
                    RootModuleLockfileMode::from(mode),
                )])
                .unwrap();
        }
        if let Some(vendor) = setup.vendor {
            inject_root_package_policy_inputs(updater, policy(vendor.as_deref())).unwrap();
        }
        if let Some(token) = setup.token {
            updater
                .changed_to(vec![(
                    HostRegistryRefreshTokenKey::new(workspace.dupe()),
                    HostRegistryRefreshToken::new(token),
                )])
                .unwrap();
        }
        if let Some(mirrors) = setup.mirrors {
            updater
                .changed_to(vec![(
                    HostModuleMirrorsInputKey::new(workspace.dupe()),
                    mirrors,
                )])
                .unwrap();
        }
    }

    async fn transaction(
        setup: Setup,
        tracker: Option<Arc<dyn ActivationTracker>>,
    ) -> DiceTransaction {
        let data = UserComputationData {
            activation_tracker: tracker,
            ..Default::default()
        };
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let mut updater = dice.updater_with_data(data);
        apply_setup(&mut updater, &path(WORKSPACE), setup);
        updater.commit().await
    }

    async fn outcome(
        setup: Setup,
        registry: &str,
    ) -> PathOutcome<Arc<Result<HostRegistryFunctionValue, HostRegistryFunctionError>>> {
        transaction(setup, None)
            .await
            .compute(&HostRegistryFunctionKey::new(path(WORKSPACE), registry))
            .await
            .unwrap()
    }

    fn terminal_error(
        error: HostRegistryFunctionError,
    ) -> PathOutcome<Arc<Result<HostRegistryFunctionValue, HostRegistryFunctionError>>> {
        PathOutcome::Complete(Arc::new(Err(error)))
    }

    fn complete_value(
        outcome: &PathOutcome<Arc<Result<HostRegistryFunctionValue, HostRegistryFunctionError>>>,
    ) -> &HostRegistryFunctionValue {
        let PathOutcome::Complete(value) = outcome else {
            panic!("expected a complete Host RegistryFunction value");
        };
        value.as_ref().as_ref().unwrap()
    }

    async fn observed(
        transaction: &mut DiceTransaction,
        key: &HostRegistryFunctionObservationKey,
    ) -> ObservedHostRegistryFunction {
        let PathOutcome::Complete(Ok(value)) = transaction.compute(key).await.unwrap() else {
            panic!("complete observed Host RegistryFunction script must be semantic");
        };
        value
    }

    async fn observed_once(setup: Setup, registry: &str) -> ObservedHostRegistryFunction {
        let mut transaction = transaction(setup, None).await;
        observed(
            &mut transaction,
            &HostRegistryFunctionObservationKey::new(path(WORKSPACE), registry),
        )
        .await
    }

    fn assert_exact_epoch(actual: &PathObservationEpoch, expected: &PathObservationEpoch) {
        assert_eq!(actual.observations().len(), expected.observations().len());
        for ((actual_demand, actual_result), (expected_demand, expected_result)) in
            actual.observations().iter().zip(expected.observations())
        {
            assert_eq!(actual_demand, expected_demand);
            assert!(Arc::ptr_eq(actual_result, expected_result));
        }
    }

    fn assert_no_upper(keys: &[String]) {
        let prefixes = "host-selected-registry-repo-specs: host-selected-module-routes: host-selected-extension- slug-command:";
        assert!(
            keys.iter()
                .all(|key| !prefixes.split(' ').any(|prefix| key.starts_with(prefix)))
        );
    }

    fn hash(value: &impl Hash) -> u64 {
        let mut hasher = DefaultHasher::new();
        value.hash(&mut hasher);
        hasher.finish()
    }

    fn complete_error(
        outcome: &PathOutcome<Arc<Result<HostRegistryFunctionValue, HostRegistryFunctionError>>>,
    ) -> &HostRegistryFunctionError {
        let PathOutcome::Complete(value) = outcome else {
            panic!("expected a complete Host RegistryFunction error");
        };
        value.as_ref().as_ref().unwrap_err()
    }

    #[test]
    fn java_uri_discriminators_match_registry_factory_boundaries() {
        for (uri, scheme) in [
            ("http:/registry", HostRegistryScheme::Http),
            ("http:///registry", HostRegistryScheme::Http),
            ("http://host", HostRegistryScheme::Http),
            ("http://?q", HostRegistryScheme::Http),
            ("http://#f", HostRegistryScheme::Http),
            ("http://bad_host", HostRegistryScheme::Http),
            ("http://user@host:bad", HostRegistryScheme::Http),
            ("http://host:", HostRegistryScheme::Http),
            ("https://[::1]", HostRegistryScheme::Https),
            (
                "https://user:pw@[::ffff:192.0.2.128]:65536",
                HostRegistryScheme::Https,
            ),
            ("https://[fe80::1%25eth0]", HostRegistryScheme::Https),
            ("https://[fe80::1%a.b_9]:", HostRegistryScheme::Https),
            ("https://[fe80::1%eth0]?q=%20", HostRegistryScheme::Https),
            ("https://[fe80::1%eth0]#f=%20", HostRegistryScheme::Https),
            ("file:/tmp", HostRegistryScheme::File),
            ("file://bad", HostRegistryScheme::File),
        ] {
            assert_eq!(parse_primary_registry_uri(uri), Ok(scheme), "{uri}");
        }
        for (uri, reason) in [
            ("relative", HostRegistryUriErrorKind::MissingScheme),
            ("file:c:/registry", HostRegistryUriErrorKind::MissingPath),
            ("http:registry", HostRegistryUriErrorKind::MissingPath),
            ("ftp:thing", HostRegistryUriErrorKind::MissingPath),
            (
                "ftp:/registry",
                HostRegistryUriErrorKind::UnrecognizedProtocol,
            ),
            (
                "HTTP:/registry",
                HostRegistryUriErrorKind::UnrecognizedProtocol,
            ),
            ("x:/", HostRegistryUriErrorKind::UnrecognizedProtocol),
            ("http:", HostRegistryUriErrorKind::InvalidSyntax),
            ("http:#f", HostRegistryUriErrorKind::InvalidSyntax),
            ("http://", HostRegistryUriErrorKind::InvalidSyntax),
            ("x:#f", HostRegistryUriErrorKind::InvalidSyntax),
            ("//", HostRegistryUriErrorKind::InvalidSyntax),
            ("//?q", HostRegistryUriErrorKind::MissingScheme),
            ("//#f", HostRegistryUriErrorKind::MissingScheme),
            ("a#b#c", HostRegistryUriErrorKind::InvalidSyntax),
            (
                "https://host/a path",
                HostRegistryUriErrorKind::InvalidSyntax,
            ),
            ("https://host/%zz", HostRegistryUriErrorKind::InvalidSyntax),
            ("https://[host", HostRegistryUriErrorKind::InvalidSyntax),
            ("https://host[::1]", HostRegistryUriErrorKind::InvalidSyntax),
            ("https://host/a[b", HostRegistryUriErrorKind::InvalidSyntax),
            (
                "https://[fe80::1%]",
                HostRegistryUriErrorKind::InvalidSyntax,
            ),
            (
                "https://[fe80::1%eth-0]",
                HostRegistryUriErrorKind::InvalidSyntax,
            ),
            (
                "https://[fe80::1%a%b]",
                HostRegistryUriErrorKind::InvalidSyntax,
            ),
            (
                "https://[fe80::1%é]",
                HostRegistryUriErrorKind::InvalidSyntax,
            ),
            ("https://u@v@[::1]", HostRegistryUriErrorKind::InvalidSyntax),
            (
                "https://[::1]:2147483648",
                HostRegistryUriErrorKind::InvalidSyntax,
            ),
            ("https://[::1]:x", HostRegistryUriErrorKind::InvalidSyntax),
        ] {
            assert_eq!(parse_primary_registry_uri(uri), Err(reason), "{uri}");
        }
        for mirror in [
            "",
            "relative/path",
            "../up",
            "#fragment[]",
            "?query=[]",
            "//?q",
            "//#f",
            "ftp://mirror",
            "HTTP:/mirror",
            "x:/",
            "opaque:[]",
            "mailto:owner",
            "https://mørror",
            "https://user@[2001:db8::192.0.2.1]",
            "relative%20path",
        ] {
            assert!(validate_module_mirror_uri(mirror).is_ok(), "{mirror}");
        }
        for mirror in [
            "bad path",
            "%zz",
            "//",
            "x:#f",
            "a#b#c",
            "relative[a",
            "https://[host",
        ] {
            assert!(validate_module_mirror_uri(mirror).is_err(), "{mirror}");
        }
    }

    #[test]
    fn key_identity_and_path_outcome_equality_are_exact() {
        assert_eq!(
            HostRegistryFunctionKey::new(path(WORKSPACE), REGISTRY_A),
            HostRegistryFunctionKey::new(path(WORKSPACE), REGISTRY_A)
        );
        assert_ne!(
            HostRegistryFunctionKey::new(path(WORKSPACE), REGISTRY_A),
            HostRegistryFunctionKey::new(path(WORKSPACE), REGISTRY_B)
        );
        assert_ne!(
            HostRegistryFunctionKey::new(path(WORKSPACE), REGISTRY_A),
            HostRegistryFunctionKey::new(path("/other"), REGISTRY_A)
        );
        let need = PathOutcome::Need(NeedPathObservations::singleton(demand(
            "/need",
            PathObservationOperation::Lstat,
        )));
        assert!(!HostRegistryFunctionKey::validity(&need));
        assert!(!HostRegistryFunctionKey::equality(&need, &need));
        let complete = terminal_error(HostRegistryFunctionError::LockfileModeInput {
            workspace: path(WORKSPACE),
        });
        let separately_allocated = terminal_error(HostRegistryFunctionError::LockfileModeInput {
            workspace: path(WORKSPACE),
        });
        assert!(HostRegistryFunctionKey::validity(&complete));
        assert!(HostRegistryFunctionKey::equality(
            &complete,
            &separately_allocated
        ));
        assert!(!HostRegistryFunctionKey::equality(
            &complete,
            &terminal_error(HostRegistryFunctionError::LockfileModeInput {
                workspace: path("/other"),
            })
        ));
        let (PathOutcome::Complete(left), PathOutcome::Complete(right)) =
            (&complete, &separately_allocated)
        else {
            unreachable!()
        };
        assert!(!Arc::ptr_eq(left, right));

        let observed_key = HostRegistryFunctionObservationKey::new(path(WORKSPACE), REGISTRY_A);
        let observed_other = HostRegistryFunctionObservationKey::new(path(WORKSPACE), REGISTRY_B);
        assert_eq!(
            observed_key,
            HostRegistryFunctionObservationKey::new(path(WORKSPACE), REGISTRY_A)
        );
        assert_ne!(observed_key, observed_other);
        assert_ne!(hash(&observed_key), hash(&observed_other));
        assert_eq!(
            observed_key.to_string(),
            "observed-host-registry:\"/workspace\":https://a.example"
        );

        let child_need =
            NeedPathObservations::singleton(demand("/need", PathObservationOperation::Lstat));
        let observed_need: HostRegistryFunctionDriverOutcome = PathOutcome::Need(child_need.dupe());
        assert!(!HostRegistryFunctionObservationKey::validity(
            &observed_need
        ));
        assert!(!HostRegistryFunctionObservationKey::equality(
            &observed_need,
            &observed_need
        ));
        let PathOutcome::Need(forwarded_need) =
            finish_observed_visible_lockfile(PathOutcome::Need(child_need.dupe())).unwrap_err()
        else {
            panic!("child Need must remain carrierless");
        };
        assert_eq!(forwarded_need, child_need);

        let result = Arc::new(Err(HostRegistryFunctionError::LockfileModeInput {
            workspace: path(WORKSPACE),
        }));
        let observed_complete = PathOutcome::Complete(Ok(ObservedHostRegistryFunction {
            result: result.dupe(),
            observations: PathObservationEpoch::empty(),
        }));
        assert!(HostRegistryFunctionObservationKey::validity(
            &observed_complete
        ));
        assert!(HostRegistryFunctionObservationKey::equality(
            &observed_complete,
            &observed_complete
        ));
        let PathOutcome::Complete(projected) =
            project_legacy_host_registry(observed_complete.clone())
        else {
            unreachable!()
        };
        assert!(Arc::ptr_eq(&projected, &result));

        let frontier: ObservedPathFrontierError =
            slug_workspace_v2::PathObservationEpochError::OperationMismatch {
                demand: demand("/need", PathObservationOperation::Lstat),
                result_operation: PathObservationOperation::FileBytes,
            }
            .into();
        let outer: HostRegistryFunctionDriverOutcome = PathOutcome::Complete(Err(frontier.dupe()));
        let PathOutcome::Complete(Err(forwarded_outer)) =
            finish_observed_visible_lockfile(PathOutcome::Complete(Err(frontier.dupe())))
                .unwrap_err()
        else {
            panic!("child outer must remain carrierless");
        };
        assert_eq!(forwarded_outer, frontier);
        assert!(HostRegistryFunctionObservationKey::validity(&outer));
        assert!(HostRegistryFunctionObservationKey::equality(&outer, &outer));
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum DirectDependency {
        Mode,
        Vendor,
        Refresh,
        LegacyVisible,
        ObservedVisible,
        Mirrors,
        Forbidden,
    }
    use DirectDependency::*;

    macro_rules! direct_dependency {
        ($key:expr; $($ty:ty => $value:expr),+ $(,)?) => {
            $(if $key.downcast_ref::<$ty>().is_some() { $value } else)+
            { Forbidden }
        };
    }

    const OFF_VISIBLE: &[DirectDependency] = &[Mode, Vendor, LegacyVisible];
    const OFF_COMPLETE: &[DirectDependency] = &[Mode, Vendor, LegacyVisible, Mirrors];
    const REFRESH_VISIBLE: &[DirectDependency] = &[Mode, Vendor, Refresh, LegacyVisible];
    const REFRESH_COMPLETE: &[DirectDependency] = &[Mode, Vendor, Refresh, LegacyVisible, Mirrors];
    const OBSERVED_OFF_COMPLETE: &[DirectDependency] = &[Mode, Vendor, ObservedVisible, Mirrors];
    const OBSERVED_REFRESH_COMPLETE: &[DirectDependency] =
        &[Mode, Vendor, Refresh, ObservedVisible, Mirrors];

    #[derive(Default)]
    struct RegistryTracker {
        dependencies: Mutex<Vec<Vec<DirectDependency>>>,
        evaluated: AtomicUsize,
        keys: Mutex<Vec<String>>,
        batches: AtomicUsize,
    }

    impl RegistryTracker {
        fn last(&self) -> Vec<DirectDependency> {
            self.dependencies.lock().unwrap().last().unwrap().clone()
        }

        fn evaluated(&self) -> usize {
            self.evaluated.load(Ordering::SeqCst)
        }

        fn take_keys(&self) -> Vec<String> {
            std::mem::take(&mut *self.keys.lock().unwrap())
        }

        fn batches(&self) -> usize {
            self.batches.load(Ordering::SeqCst)
        }

        fn clear_batches(&self) {
            self.batches.store(0, Ordering::SeqCst);
        }
    }
    impl ActivationTracker for RegistryTracker {
        fn key_activated(
            &self,
            key: &DynKey,
            dependencies: &mut dyn Iterator<Item = &DynKey>,
            activation: ActivationData,
        ) {
            self.keys.lock().unwrap().push(key.to_string());
            if key.downcast_ref::<HostRegistryFunctionKey>().is_none()
                && key
                    .downcast_ref::<HostRegistryFunctionObservationKey>()
                    .is_none()
            {
                return;
            }
            if matches!(activation, ActivationData::Evaluated(_)) {
                self.evaluated.fetch_add(1, Ordering::SeqCst);
            }
            self.dependencies.lock().unwrap().push(
                dependencies
                    .map(|dependency| {
                        direct_dependency! {
                            dependency;
                            RootModuleLockfileModeKey => Mode,
                            RootVendorDirectoryProjectionKey => Vendor,
                            HostRegistryRefreshTokenKey => Refresh,
                            HostVisibleLockfileKey => LegacyVisible,
                            HostVisibleLockfileObservationKey => ObservedVisible,
                            HostModuleMirrorsInputKey => Mirrors,
                        }
                    })
                    .collect(),
            );
        }

        fn tracks_rich_activations(&self) -> bool {
            true
        }

        fn key_activated_rich(&self, _key: &DynKey, activation: RichActivation<'_>) {
            if activation.evaluation_data().is_some() {
                self.batches.fetch_add(1, Ordering::SeqCst);
            }
        }
    }
    async fn tracked_outcome(
        setup: Setup,
        registry: &str,
        expected: &[DirectDependency],
    ) -> PathOutcome<Arc<Result<HostRegistryFunctionValue, HostRegistryFunctionError>>> {
        let tracker = Arc::new(RegistryTracker::default());
        let mut transaction =
            transaction(setup, Some(tracker.dupe() as Arc<dyn ActivationTracker>)).await;
        let value = transaction
            .compute(&HostRegistryFunctionKey::new(path(WORKSPACE), registry))
            .await
            .unwrap();
        assert_eq!(tracker.last(), expected);
        assert_eq!(tracker.evaluated(), 1);
        value
    }

    #[tokio::test]
    async fn source_order_needs_and_typed_errors_are_exact() {
        let mut missing_mode = Setup::ready(LockfileMode::Off, &[REGISTRY_A]);
        missing_mode.mode = None;
        let result = tracked_outcome(missing_mode, REGISTRY_A, &[]).await;
        assert!(matches!(
            complete_error(&result),
            HostRegistryFunctionError::LockfileModeInput { .. }
        ));

        let mut missing_vendor = Setup::ready(LockfileMode::Off, &[REGISTRY_A]);
        missing_vendor.vendor = None;
        let result = tracked_outcome(missing_vendor, REGISTRY_A, &[Mode, Vendor]).await;
        assert!(matches!(
            complete_error(&result),
            HostRegistryFunctionError::VendorDirectoryInput { .. }
        ));

        let mut missing_refresh = Setup::ready(LockfileMode::Refresh, &[REGISTRY_A]);
        missing_refresh.token = None;
        let result = tracked_outcome(missing_refresh, REGISTRY_A, &[Mode, Vendor]).await;
        assert!(matches!(
            complete_error(&result),
            HostRegistryFunctionError::RefreshTokenInput { .. }
        ));

        let mut need = Setup::ready(LockfileMode::Refresh, &[REGISTRY_A]);
        need.epoch = empty_epoch();
        need.mirrors = None;
        assert!(matches!(
            tracked_outcome(need, "not a uri", REFRESH_VISIBLE,).await,
            PathOutcome::Need(_)
        ));

        let mut bad_lockfile = Setup::ready(LockfileMode::Off, &[REGISTRY_A]);
        bad_lockfile.epoch = complete_epoch(b"{\"lockFileVersion\":28", 11);
        bad_lockfile.mirrors = None;
        let result = tracked_outcome(bad_lockfile, REGISTRY_A, OFF_VISIBLE).await;
        assert!(matches!(
            complete_error(&result),
            HostRegistryFunctionError::VisibleLockfile { .. }
        ));

        let mut missing_mirrors = Setup::ready(LockfileMode::Off, &["not a uri"]);
        missing_mirrors.mirrors = None;
        let result = tracked_outcome(missing_mirrors, "not a uri", OFF_VISIBLE).await;
        assert!(matches!(
            complete_error(&result),
            HostRegistryFunctionError::ModuleMirrorsInput { .. }
        ));

        let invalid_mirrors = mirrors(&["not-a-uri"], &[("not-a-uri", &["bad path"])]);
        let mut invalid_primary = Setup::ready(LockfileMode::Off, &["not-a-uri"]);
        invalid_primary.mirrors = Some(invalid_mirrors);
        let result = tracked_outcome(invalid_primary, "not-a-uri", OFF_COMPLETE).await;
        assert!(matches!(
            complete_error(&result),
            HostRegistryFunctionError::InvalidPrimaryRegistryUri {
                reason: HostRegistryUriErrorKind::MissingScheme,
                ..
            }
        ));

        let ordered = mirrors(
            &[REGISTRY_A],
            &[(
                REGISTRY_A,
                &["relative/ok", "bad path", "https://later/%zz"],
            )],
        );
        let mut invalid_second = Setup::ready(LockfileMode::Off, &[REGISTRY_A]);
        invalid_second.mirrors = Some(ordered);
        let result = tracked_outcome(invalid_second, REGISTRY_A, OFF_COMPLETE).await;
        assert!(matches!(
            complete_error(&result),
            HostRegistryFunctionError::InvalidModuleMirrorUri {
                mirror,
                ordinal: 1,
                ..
            } if mirror == "bad path"
        ));
    }

    #[tokio::test]
    async fn observed_owner_preserves_prefixes_families_terminals_and_silence() {
        let observed_key = HostRegistryFunctionObservationKey::new(path(WORKSPACE), REGISTRY_A);

        let tracker = Arc::new(RegistryTracker::default());
        let mut need_setup = Setup::ready(LockfileMode::Off, &[REGISTRY_A]);
        need_setup.epoch = PathObservationEpoch::empty();
        need_setup.mirrors = None;
        let mut need_transaction = transaction(
            need_setup,
            Some(tracker.dupe() as Arc<dyn ActivationTracker>),
        )
        .await;
        let need = need_transaction.compute(&observed_key).await.unwrap();
        assert!(matches!(need, PathOutcome::Need(_)));
        assert_eq!(tracker.last(), [Mode, Vendor, ObservedVisible].as_slice());
        assert_eq!(tracker.batches(), 0);
        let keys = tracker.take_keys();
        assert!(
            !keys
                .iter()
                .any(|key| key.starts_with("host-visible-lockfile:"))
        );
        assert_no_upper(&keys);

        let setup = Setup::ready(LockfileMode::Off, &[REGISTRY_A]);
        let selected = setup.epoch.dupe();
        let mut child_transaction = transaction(setup.clone(), None).await;
        let child = child_transaction
            .compute(&HostVisibleLockfileObservationKey::new(path(WORKSPACE)))
            .await
            .unwrap();
        let PathOutcome::Complete(Ok(expected_child)) = &child else {
            panic!("ready observed visible lockfile must complete");
        };
        let expected_result = expected_child.result().dupe();
        let expected_epoch = expected_child.observations().dupe();
        let (finished_result, finished_epoch) = finish_observed_visible_lockfile(child).unwrap();
        assert!(Arc::ptr_eq(&finished_result, &expected_result));
        assert_exact_epoch(&finished_epoch, &expected_epoch);

        let tracker = Arc::new(RegistryTracker::default());
        let mut observed_transaction =
            transaction(setup, Some(tracker.dupe() as Arc<dyn ActivationTracker>)).await;
        let observed_a = observed(&mut observed_transaction, &observed_key).await;
        assert_exact_epoch(observed_a.observations(), &selected);
        assert_eq!(tracker.last(), OBSERVED_OFF_COMPLETE);
        assert_eq!(tracker.batches(), 0);
        let keys = tracker.take_keys();
        assert!(
            !keys
                .iter()
                .any(|key| key.starts_with("host-visible-lockfile:"))
        );
        assert_no_upper(&keys);

        let warm = observed(&mut observed_transaction, &observed_key).await;
        assert_eq!(warm, observed_a);
        assert_eq!(tracker.batches(), 0);
        assert_no_upper(&tracker.take_keys());

        let legacy_key = HostRegistryFunctionKey::new(path(WORKSPACE), REGISTRY_A);
        let legacy = observed_transaction.compute(&legacy_key).await.unwrap();
        assert_eq!(
            observed_a.result().as_ref().as_ref().unwrap(),
            complete_value(&legacy)
        );
        assert_eq!(tracker.last(), OFF_COMPLETE);
        let keys = tracker.take_keys();
        assert!(
            !keys
                .iter()
                .any(|key| key.starts_with("observed-host-visible-lockfile:"))
        );
        assert_eq!(tracker.batches(), 0);
        assert_no_upper(&keys);

        let tracker = Arc::new(RegistryTracker::default());
        let mut refresh = transaction(
            Setup::ready(LockfileMode::Refresh, &[REGISTRY_A]),
            Some(tracker.dupe() as Arc<dyn ActivationTracker>),
        )
        .await;
        let refresh_value = observed(&mut refresh, &observed_key).await;
        assert!(!refresh_value.observations().observations().is_empty());
        assert_eq!(tracker.last(), OBSERVED_REFRESH_COMPLETE);
        assert_eq!(tracker.batches(), 0);
        assert_no_upper(&tracker.take_keys());

        let mut missing_mode = Setup::ready(LockfileMode::Off, &[REGISTRY_A]);
        missing_mode.mode = None;
        let value = observed_once(missing_mode, REGISTRY_A).await;
        assert!(value.observations().observations().is_empty());
        assert!(matches!(
            value.result().as_ref(),
            Err(HostRegistryFunctionError::LockfileModeInput { .. })
        ));

        let mut missing_vendor = Setup::ready(LockfileMode::Off, &[REGISTRY_A]);
        missing_vendor.vendor = None;
        let value = observed_once(missing_vendor, REGISTRY_A).await;
        assert!(value.observations().observations().is_empty());
        assert!(matches!(
            value.result().as_ref(),
            Err(HostRegistryFunctionError::VendorDirectoryInput { .. })
        ));

        let mut missing_refresh = Setup::ready(LockfileMode::Refresh, &[REGISTRY_A]);
        missing_refresh.token = None;
        let value = observed_once(missing_refresh, REGISTRY_A).await;
        assert!(value.observations().observations().is_empty());
        assert!(matches!(
            value.result().as_ref(),
            Err(HostRegistryFunctionError::RefreshTokenInput { .. })
        ));

        let mut bad_lockfile = Setup::ready(LockfileMode::Off, &[REGISTRY_A]);
        bad_lockfile.epoch = complete_epoch(b"{\"lockFileVersion\":28", 21);
        let mut missing_mirrors = Setup::ready(LockfileMode::Off, &[REGISTRY_A]);
        missing_mirrors.mirrors = None;
        let invalid_mirrors = mirrors(&["not-a-uri"], &[("not-a-uri", &["bad path"])]);
        let mut invalid_primary = Setup::ready(LockfileMode::Off, &["not-a-uri"]);
        invalid_primary.mirrors = Some(invalid_mirrors);
        let ordered = mirrors(
            &[REGISTRY_A],
            &[(REGISTRY_A, &["relative/ok", "bad path", "later"])],
        );
        let mut invalid_mirror = Setup::ready(LockfileMode::Off, &[REGISTRY_A]);
        invalid_mirror.mirrors = Some(ordered);
        for (index, setup, registry) in [
            (0, bad_lockfile, REGISTRY_A),
            (1, missing_mirrors, REGISTRY_A),
            (2, invalid_primary, "not-a-uri"),
            (3, invalid_mirror, REGISTRY_A),
        ] {
            let selected = setup.epoch.dupe();
            let value = observed_once(setup, registry).await;
            assert_exact_epoch(value.observations(), &selected);
            match index {
                0 => assert!(matches!(
                    value.result().as_ref(),
                    Err(HostRegistryFunctionError::VisibleLockfile { .. })
                )),
                1 => assert!(matches!(
                    value.result().as_ref(),
                    Err(HostRegistryFunctionError::ModuleMirrorsInput { .. })
                )),
                2 => assert!(matches!(
                    value.result().as_ref(),
                    Err(HostRegistryFunctionError::InvalidPrimaryRegistryUri {
                        reason: HostRegistryUriErrorKind::MissingScheme,
                        ..
                    })
                )),
                3 => assert!(matches!(
                    value.result().as_ref(),
                    Err(HostRegistryFunctionError::InvalidModuleMirrorUri { ordinal: 1, .. })
                )),
                _ => unreachable!(),
            }
        }
    }
    #[tokio::test]
    async fn all_mode_scheme_cells_and_direct_dependencies_are_exact() {
        for (registry, scheme) in [
            ("http://registry", HostRegistryScheme::Http),
            ("https://registry", HostRegistryScheme::Https),
            ("file://registry", HostRegistryScheme::File),
        ] {
            for (mode, remote_hash_mode) in [
                (LockfileMode::Off, RegistryKnownFileHashesMode::UseAndUpdate),
                (
                    LockfileMode::Update,
                    RegistryKnownFileHashesMode::UseAndUpdate,
                ),
                (
                    LockfileMode::Refresh,
                    RegistryKnownFileHashesMode::UseImmutableAndUpdate,
                ),
                (LockfileMode::Error, RegistryKnownFileHashesMode::Enforce),
            ] {
                let tracker = Arc::new(RegistryTracker::default());
                let mut transaction = transaction(
                    Setup::ready(mode.clone(), &[registry]),
                    Some(tracker.dupe() as Arc<dyn ActivationTracker>),
                )
                .await;
                let result = transaction
                    .compute(&HostRegistryFunctionKey::new(path(WORKSPACE), registry))
                    .await
                    .unwrap();
                let value = complete_value(&result);
                assert_eq!(value.scheme(), scheme);
                let expected_hash_mode = if scheme == HostRegistryScheme::File {
                    RegistryKnownFileHashesMode::Ignore
                } else {
                    remote_hash_mode
                };
                assert_eq!(value.known_file_hashes_mode(), expected_hash_mode);
                let expected_dependencies = if mode == LockfileMode::Refresh {
                    REFRESH_COMPLETE
                } else {
                    OFF_COMPLETE
                };
                assert_eq!(tracker.last(), expected_dependencies);
            }
        }
    }

    #[tokio::test]
    async fn original_spelling_drives_lookup_after_workspace_resolution() {
        let original = "file://%workspace%/registry";
        let resolved = "file:///workspace/registry";
        let selected = mirrors(
            &[original, resolved],
            &[
                (original, &["relative/mirror", "mailto:owner"]),
                (resolved, &["https://wrong.example"]),
            ],
        );
        let mut setup = Setup::ready(LockfileMode::Off, &[original, resolved]);
        setup.vendor = Some(Some("/vendor".into()));
        setup.mirrors = Some(selected);
        let result = outcome(setup, original).await;
        let value = complete_value(&result);
        assert_eq!(value.original_registry(), original);
        assert_eq!(value.resolved_registry(), resolved);
        assert_eq!(value.vendor_directory(), Some(&path("/vendor")));
        assert_eq!(
            value
                .module_mirrors()
                .iter()
                .map(CompactString::as_str)
                .collect::<Vec<_>>(),
            ["relative/mirror", "mailto:owner"]
        );
        assert_eq!(
            value.registry_file_expectation("u").unwrap(),
            RegistryFileExpectation::RecordedSha256([0xab; 32])
        );
        assert_eq!(
            value.selected_yanked_reason("subject", "1.0.0"),
            Some("reason-a")
        );
        assert_eq!(
            value.selected_yanked_reason(
                "a_very_long_module_name_that_cannot_fit_inline",
                "123456789.987654321.111111111"
            ),
            Some("long-reason")
        );
        assert_eq!(value.selected_yanked_reason("subject", "2.0.0"), None);

        let mut descriptors = Vec::new();
        for (input, expected) in [
            (
                mirrors(&[REGISTRY_A], &[("", &["relative/default"])]),
                &["relative/default"][..],
            ),
            (
                mirrors(
                    &[REGISTRY_A],
                    &[
                        ("", &["relative/default"]),
                        (REGISTRY_A, &["OPAQUE:value", "?query", "#fragment"]),
                    ],
                ),
                &["OPAQUE:value", "?query", "#fragment"][..],
            ),
            (mirrors(&[REGISTRY_A], &[(REGISTRY_A, &[])]), &[][..]),
            (mirrors(&[REGISTRY_A], &[]), &[][..]),
            (mirrors(&[REGISTRY_A], &[("", &[])]), &[][..]),
        ] {
            let mut setup = Setup::ready(LockfileMode::Off, &[REGISTRY_A]);
            setup.mirrors = Some(input);
            let result = outcome(setup, REGISTRY_A).await;
            descriptors.push(complete_value(&result).clone());
            assert_eq!(
                complete_value(&result)
                    .module_mirrors()
                    .iter()
                    .map(CompactString::as_str)
                    .collect::<Vec<_>>(),
                expected
            );
        }
        assert_ne!(descriptors[0], descriptors[1]);
        assert_eq!(descriptors[2], descriptors[3]);
        assert_eq!(descriptors[3], descriptors[4]);
    }

    #[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative, Dupe)]
    struct TestRegistrySelectionKey {
        workspace: NormalizedAbsolutePath,
    }

    impl fmt::Display for TestRegistrySelectionKey {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "test-registry-selection:{}", self.workspace)
        }
    }

    impl InjectedKey for TestRegistrySelectionKey {
        type Value = Arc<CompactString>;

        fn equality(x: &Self::Value, y: &Self::Value) -> bool {
            x == y
        }
    }

    static CONSUMER_EVALUATIONS: AtomicUsize = AtomicUsize::new(0);

    #[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative, Dupe)]
    struct RegistryConsumerKey {
        workspace: NormalizedAbsolutePath,
    }

    impl fmt::Display for RegistryConsumerKey {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "test-registry-consumer:{}", self.workspace)
        }
    }

    #[async_trait]
    impl Key for RegistryConsumerKey {
        type Value = PathOutcome<Arc<Result<HostRegistryFunctionValue, HostRegistryFunctionError>>>;

        async fn compute(
            &self,
            ctx: &mut DiceComputations,
            _cancellations: &CancellationContext,
        ) -> Self::Value {
            let registry = ctx
                .compute(&TestRegistrySelectionKey {
                    workspace: self.workspace.dupe(),
                })
                .await
                .unwrap();
            let value = ctx
                .compute(&HostRegistryFunctionKey::new(
                    self.workspace.dupe(),
                    registry.as_ref().clone(),
                ))
                .await
                .unwrap();
            CONSUMER_EVALUATIONS.fetch_add(1, Ordering::SeqCst);
            value
        }

        fn equality(x: &Self::Value, y: &Self::Value) -> bool {
            x.complete_eq(y)
        }

        fn validity(value: &Self::Value) -> bool {
            value.is_complete()
        }
    }

    async fn replace<T: InjectedKey>(
        transaction: DiceTransaction,
        key: T,
        value: T::Value,
    ) -> DiceTransaction {
        let mut updater = transaction.into_updater();
        updater.changed_to(vec![(key, value)]).unwrap();
        updater.commit().await
    }

    async fn replace_policy(transaction: DiceTransaction, vendor: Option<&str>) -> DiceTransaction {
        let mut updater = transaction.into_updater();
        inject_root_package_policy_inputs(&mut updater, policy(vendor)).unwrap();
        updater.commit().await
    }

    async fn replace_lockfile(
        transaction: DiceTransaction,
        bytes: &[u8],
        variant: i64,
    ) -> DiceTransaction {
        replace(
            transaction,
            PathObservationEpochKey,
            complete_epoch(bytes, variant),
        )
        .await
    }

    async fn assert_consumer(
        transaction: &mut DiceTransaction,
        consumer: &RegistryConsumerKey,
        expected: usize,
    ) {
        assert!(matches!(
            transaction.compute(consumer).await.unwrap(),
            PathOutcome::Complete(value) if value.is_ok()
        ));
        assert_eq!(CONSUMER_EVALUATIONS.load(Ordering::SeqCst), expected);
    }

    #[tokio::test]
    async fn retained_semantic_equality_prunes_only_unrelated_changes() {
        CONSUMER_EVALUATIONS.store(0, Ordering::SeqCst);
        let workspace = path(WORKSPACE);
        let consumer = RegistryConsumerKey {
            workspace: workspace.dupe(),
        };
        let mirror_a = mirrors(
            &[REGISTRY_A, REGISTRY_B],
            &[(REGISTRY_A, &["https://mirror-a"])],
        );
        let mirror_b = mirrors(
            &[REGISTRY_A, REGISTRY_B],
            &[(REGISTRY_A, &["https://mirror-b"])],
        );
        let mut setup = Setup::ready(LockfileMode::Off, &[REGISTRY_A, REGISTRY_B]);
        setup.mirrors = Some(mirror_a.dupe());
        let tracker = Arc::new(RegistryTracker::default());
        let mut transaction =
            transaction(setup, Some(tracker.dupe() as Arc<dyn ActivationTracker>)).await;
        transaction = replace(
            transaction,
            TestRegistrySelectionKey {
                workspace: workspace.dupe(),
            },
            Arc::new(REGISTRY_A.into()),
        )
        .await;
        assert_consumer(&mut transaction, &consumer, 1).await;
        assert_eq!(tracker.evaluated(), 1);

        let extension = r#"{
          "//:ext.bzl%x": {"general": {
            "bzlTransitiveDigest": "AQ==",
            "usagesDigest": "AgM=",
            "recordedInputs": [],
            "generatedRepoSpecs": {}
          }}
        }"#;
        let unrelated = [
            lockfile_source(0xab, "reason-a", extension, "{}", "{}"),
            base_lockfile(),
            lockfile_source(
                0xab,
                "reason-a",
                "{}",
                r#"{"//:ext.bzl%x":{"answer":42}}"#,
                "{}",
            ),
            base_lockfile(),
            lockfile_source(0xab, "reason-a", "{}", "{}", r#"{"//:ext.bzl%x":7}"#),
            base_lockfile(),
        ];
        for (index, bytes) in unrelated.iter().enumerate() {
            transaction = replace_lockfile(transaction, bytes, 20 + index as i64).await;
            assert_consumer(&mut transaction, &consumer, 1).await;
            assert_eq!(tracker.evaluated(), index + 2);
        }

        transaction = replace_lockfile(
            transaction,
            &lockfile_source(0xcd, "reason-a", "{}", "{}", "{}"),
            30,
        )
        .await;
        assert_consumer(&mut transaction, &consumer, 2).await;
        transaction = replace_lockfile(transaction, &base_lockfile(), 31).await;
        assert_consumer(&mut transaction, &consumer, 3).await;
        transaction = replace_lockfile(
            transaction,
            &lockfile_source(0xab, "reason-b", "{}", "{}", "{}"),
            32,
        )
        .await;
        assert_consumer(&mut transaction, &consumer, 4).await;
        transaction = replace_lockfile(transaction, &base_lockfile(), 33).await;
        assert_consumer(&mut transaction, &consumer, 5).await;

        transaction = replace(
            transaction,
            RootModuleLockfileModeKey {
                workspace: workspace.as_path().to_path_buf(),
            },
            RootModuleLockfileMode::from(LockfileMode::Update),
        )
        .await;
        assert_consumer(&mut transaction, &consumer, 5).await;
        transaction = replace(
            transaction,
            RootModuleLockfileModeKey {
                workspace: workspace.as_path().to_path_buf(),
            },
            RootModuleLockfileMode::from(LockfileMode::Error),
        )
        .await;
        assert_consumer(&mut transaction, &consumer, 6).await;
        transaction = replace(
            transaction,
            RootModuleLockfileModeKey {
                workspace: workspace.as_path().to_path_buf(),
            },
            RootModuleLockfileMode::from(LockfileMode::Off),
        )
        .await;
        assert_consumer(&mut transaction, &consumer, 7).await;

        transaction = replace_policy(transaction, Some("/vendor")).await;
        assert_consumer(&mut transaction, &consumer, 8).await;
        transaction = replace_policy(transaction, None).await;
        assert_consumer(&mut transaction, &consumer, 9).await;
        transaction = replace(
            transaction,
            HostModuleMirrorsInputKey::new(workspace.dupe()),
            mirror_b,
        )
        .await;
        assert_consumer(&mut transaction, &consumer, 10).await;
        transaction = replace(
            transaction,
            HostModuleMirrorsInputKey::new(workspace.dupe()),
            mirror_a,
        )
        .await;
        assert_consumer(&mut transaction, &consumer, 11).await;

        transaction = replace(
            transaction,
            RootModuleLockfileModeKey {
                workspace: workspace.as_path().to_path_buf(),
            },
            RootModuleLockfileMode::from(LockfileMode::Refresh),
        )
        .await;
        assert_consumer(&mut transaction, &consumer, 12).await;
        transaction = replace(
            transaction,
            HostRegistryRefreshTokenKey::new(workspace.dupe()),
            HostRegistryRefreshToken::new(1),
        )
        .await;
        assert_consumer(&mut transaction, &consumer, 12).await;
        for (token, expected) in [(2, 13), (1, 14)] {
            transaction = replace(
                transaction,
                HostRegistryRefreshTokenKey::new(workspace.dupe()),
                HostRegistryRefreshToken::new(token),
            )
            .await;
            assert_consumer(&mut transaction, &consumer, expected).await;
        }

        for (registry, expected) in [(REGISTRY_B, 15), (REGISTRY_A, 16)] {
            transaction = replace(
                transaction,
                TestRegistrySelectionKey {
                    workspace: workspace.dupe(),
                },
                Arc::new(registry.into()),
            )
            .await;
            assert_consumer(&mut transaction, &consumer, expected).await;
        }

        let current = complete_value(
            &transaction
                .compute(&HostRegistryFunctionKey::new(workspace.dupe(), REGISTRY_A))
                .await
                .unwrap(),
        )
        .clone();
        let mut different_version = (*current.lockfile).clone();
        different_version.lock_file_version = 27;
        let mut version_only = current.clone();
        version_only.lockfile = Arc::new(different_version);
        assert_eq!(current, version_only);
        let mut separately_allocated = current.clone();
        separately_allocated.lockfile = Arc::new((*current.lockfile).clone());
        assert!(!Arc::ptr_eq(
            &current.lockfile,
            &separately_allocated.lockfile
        ));
        assert_eq!(current, separately_allocated);
    }

    #[tokio::test]
    async fn observed_engine_restores_inputs_keys_and_recovers_after_poll_drop() {
        let workspace = path(WORKSPACE);
        let observed_key = HostRegistryFunctionObservationKey::new(workspace.dupe(), REGISTRY_A);
        let mirror_a = mirrors(
            &[REGISTRY_A, REGISTRY_B],
            &[(REGISTRY_A, &["https://mirror-a"])],
        );
        let mirror_b = mirrors(
            &[REGISTRY_A, REGISTRY_B],
            &[(REGISTRY_A, &["https://mirror-b"])],
        );
        let mut setup = Setup::ready(LockfileMode::Refresh, &[REGISTRY_A, REGISTRY_B]);
        setup.mirrors = Some(mirror_a.dupe());
        let tracker = Arc::new(RegistryTracker::default());
        let mut transaction =
            transaction(setup, Some(tracker.dupe() as Arc<dyn ActivationTracker>)).await;
        let a = observed(&mut transaction, &observed_key).await;
        let held_result = a.result().dupe();
        let held_epoch = a.observations().dupe();
        assert_eq!(tracker.batches(), 0);
        assert_eq!(observed(&mut transaction, &observed_key).await, a);

        transaction = replace(
            transaction,
            RootModuleLockfileModeKey {
                workspace: workspace.as_path().to_path_buf(),
            },
            RootModuleLockfileMode::from(LockfileMode::Error),
        )
        .await;
        let changed = observed(&mut transaction, &observed_key).await;
        assert_ne!(changed, a);
        assert_exact_epoch(changed.observations(), &held_epoch);
        transaction = replace(
            transaction,
            RootModuleLockfileModeKey {
                workspace: workspace.as_path().to_path_buf(),
            },
            RootModuleLockfileMode::from(LockfileMode::Refresh),
        )
        .await;
        let restored = observed(&mut transaction, &observed_key).await;
        assert_eq!(restored, a);
        assert_exact_epoch(restored.observations(), &held_epoch);

        transaction = replace_policy(transaction, Some("/vendor")).await;
        let changed = observed(&mut transaction, &observed_key).await;
        assert_ne!(changed, a);
        assert_exact_epoch(changed.observations(), &held_epoch);
        transaction = replace_policy(transaction, None).await;
        let restored = observed(&mut transaction, &observed_key).await;
        assert_eq!(restored, a);
        assert_exact_epoch(restored.observations(), &held_epoch);

        transaction = replace(
            transaction,
            HostRegistryRefreshTokenKey::new(workspace.dupe()),
            HostRegistryRefreshToken::new(2),
        )
        .await;
        let changed = observed(&mut transaction, &observed_key).await;
        assert_ne!(changed, a);
        assert_exact_epoch(changed.observations(), &held_epoch);
        transaction = replace(
            transaction,
            HostRegistryRefreshTokenKey::new(workspace.dupe()),
            HostRegistryRefreshToken::new(1),
        )
        .await;
        let restored = observed(&mut transaction, &observed_key).await;
        assert_eq!(restored, a);
        assert_exact_epoch(restored.observations(), &held_epoch);

        transaction = replace(
            transaction,
            HostModuleMirrorsInputKey::new(workspace.dupe()),
            mirror_b,
        )
        .await;
        let changed = observed(&mut transaction, &observed_key).await;
        assert_ne!(changed, a);
        assert_exact_epoch(changed.observations(), &held_epoch);
        transaction = replace(
            transaction,
            HostModuleMirrorsInputKey::new(workspace.dupe()),
            mirror_a,
        )
        .await;
        let restored = observed(&mut transaction, &observed_key).await;
        assert_eq!(restored, a);
        assert_exact_epoch(restored.observations(), &held_epoch);

        transaction = replace_lockfile(
            transaction,
            &lockfile_source(0xcd, "reason-b", "{}", "{}", "{}"),
            30,
        )
        .await;
        assert_ne!(observed(&mut transaction, &observed_key).await, a);
        transaction = replace_lockfile(transaction, &base_lockfile(), 10).await;
        let restored = observed(&mut transaction, &observed_key).await;
        assert_eq!(restored, a);
        assert_eq!(held_result.as_ref(), restored.result().as_ref());
        assert_eq!(held_epoch, *restored.observations());

        let registry_b = observed(
            &mut transaction,
            &HostRegistryFunctionObservationKey::new(workspace.dupe(), REGISTRY_B),
        )
        .await;
        assert_ne!(registry_b, a);
        assert_eq!(observed(&mut transaction, &observed_key).await, a);

        let cancel_epoch = complete_epoch(&lockfile_source(0xef, "reason-c", "{}", "{}", "{}"), 40);
        transaction = replace(transaction, PathObservationEpochKey, cancel_epoch).await;
        tracker.take_keys();
        tracker.clear_batches();
        let evaluated = tracker.evaluated();
        let mut future = Box::pin(transaction.compute(&observed_key));
        std::future::poll_fn(|context| {
            assert!(std::future::Future::poll(future.as_mut(), context).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        drop(future);
        let cancelled = tracker.take_keys();
        assert_eq!(tracker.evaluated(), evaluated);
        assert_eq!(tracker.batches(), 0);
        assert!(!cancelled.iter().any(|key| key == &observed_key.to_string()));
        assert_no_upper(&cancelled);

        let recovered = observed(&mut transaction, &observed_key).await;
        let PathOutcome::Complete(Ok(visible)) = transaction
            .compute(&HostVisibleLockfileObservationKey::new(workspace))
            .await
            .unwrap()
        else {
            panic!("recovery must complete the observed visible-lockfile child");
        };
        assert_exact_epoch(recovered.observations(), visible.observations());
        assert_ne!(recovered, a);
        assert_eq!(tracker.batches(), 0);
        assert_no_upper(&tracker.take_keys());
    }
    #[test]
    fn production_owner_names_no_forbidden_key_or_io_surface() {
        let source = include_str!("host_registry.rs");
        let source = source.split("#[cfg(test)]").next().unwrap();
        for forbidden in [
            "HostRegistryUrlsInputKey",
            "RegistryRequestGenerationKey",
            "HostRootModule",
            "RegistryIo",
            "HostFileBytesKey::new",
            "RootModuleGraph",
            "RepositoryMapping",
            "SourcePreparation",
            "VisibleLockfilePlan",
        ] {
            assert!(!source.contains(forbidden), "{forbidden}");
        }
    }
}
