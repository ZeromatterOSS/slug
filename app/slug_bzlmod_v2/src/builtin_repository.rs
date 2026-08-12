/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory or the Apache License, Version 2.0
 * found in the LICENSE-APACHE file in the root directory. You may select,
 * at your option, one of the above-listed licenses.
 */

use std::fmt;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use compact_str::CompactString;
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use sha2::Digest;
use sha2::Sha256;

const MANIFEST_DOMAIN: &[u8] = b"slug-v2:builtin-bazel-tools-manifest:v1\0";

#[derive(Debug, Clone, Copy, Dupe, PartialEq, Eq, Hash, Allocative)]
pub enum BuiltinBazelToolsSnapshot {
    Bazel9_2,
}

#[derive(Debug, Clone, Copy, Dupe, PartialEq, Eq, Hash, Allocative)]
pub struct BuiltinBazelToolsRouteIdentity {
    snapshot: BuiltinBazelToolsSnapshot,
    manifest_sha256: [u8; 32],
}

impl BuiltinBazelToolsRouteIdentity {
    pub fn snapshot(self) -> BuiltinBazelToolsSnapshot {
        self.snapshot
    }

    pub fn manifest_sha256(self) -> [u8; 32] {
        self.manifest_sha256
    }
}

impl BuiltinBazelToolsSnapshot {
    pub const CURRENT: Self = Self::Bazel9_2;

    fn tag(self) -> &'static str {
        match self {
            Self::Bazel9_2 => "bazel-9.2.0-8220c6198837d5c13d53fea211cf3282aa12408a",
        }
    }

    pub fn route_identity(self) -> BuiltinBazelToolsRouteIdentity {
        BuiltinBazelToolsRouteIdentity {
            snapshot: self,
            manifest_sha256: manifest_sha256(self),
        }
    }
}

struct CatalogEntry {
    path: &'static str,
    bytes: &'static [u8],
    expected_sha256: &'static str,
    executable: bool,
}

const CATALOG: &[CatalogEntry] = &[
    CatalogEntry {
        path: "MODULE.bazel",
        bytes: include_bytes!("../builtin/bazel_tools/MODULE.bazel"),
        expected_sha256: "a51e647c77be3c7dcb861131e339f2b65301bb572d2a9ac3d7eef30ca5b8a523",
        executable: true,
    },
    CatalogEntry {
        path: "src/conditions/BUILD",
        bytes: include_bytes!("../builtin/bazel_tools/src/conditions/BUILD"),
        expected_sha256: "7a2d956c2c38092b93276b6cb11492f0ef7ce401c879d68a57e40b45f9163f16",
        executable: true,
    },
    CatalogEntry {
        path: "tools/test/BUILD",
        bytes: include_bytes!("../builtin/bazel_tools/tools/test/BUILD"),
        expected_sha256: "81db88f41f7a9a07af246a42cfa7a8b6e118012b4f41830aaee9ffe4a4a9ee17",
        executable: true,
    },
    CatalogEntry {
        path: "tools/test/default_test_toolchain.bzl",
        bytes: include_bytes!("../builtin/bazel_tools/tools/test/default_test_toolchain.bzl"),
        expected_sha256: "c013158dde96f9b4699af24806fab64e4574e398fe94f612e25a16b1fa4f16f6",
        executable: true,
    },
    CatalogEntry {
        path: "tools/test/dummy.sh",
        bytes: include_bytes!("../builtin/bazel_tools/tools/test/dummy.sh"),
        expected_sha256: "14a80dd0456a276c4707b36d8fb39cd180bb436c965fe13c79541fc8613d397c",
        executable: true,
    },
    CatalogEntry {
        path: "tools/test/generate-xml.sh",
        bytes: include_bytes!("../builtin/bazel_tools/tools/test/generate-xml.sh"),
        expected_sha256: "368e50ceca617b237c60adf70105cf6e1d33427f232c78239a3e7c10a4d93ebf",
        executable: true,
    },
    CatalogEntry {
        path: "tools/test/test-setup.sh",
        bytes: include_bytes!("../builtin/bazel_tools/tools/test/test-setup.sh"),
        expected_sha256: "49ba08927c3c556c52c6f771eaca362a0dbd1b6e19fd2667c61d92c33a32278a",
        executable: true,
    },
];

fn file_sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn frame(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

fn manifest_sha256(snapshot: BuiltinBazelToolsSnapshot) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(MANIFEST_DOMAIN);
    frame(&mut hasher, snapshot.tag().as_bytes());
    hasher.update((CATALOG.len() as u64).to_be_bytes());
    for entry in CATALOG {
        frame(&mut hasher, entry.path.as_bytes());
        hasher.update(file_sha256(entry.bytes));
        hasher.update([u8::from(entry.executable)]);
        hasher.update((entry.bytes.len() as u64).to_be_bytes());
    }
    hasher.finalize().into()
}

fn valid_relative_path(path: &str) -> bool {
    if path.is_empty()
        || path.starts_with('/')
        || path.contains('\\')
        || path.contains('\0')
        || path
            .split('/')
            .any(|component| component.is_empty() || component == "." || component == "..")
    {
        return false;
    }
    let first = path.split('/').next().unwrap_or_default().as_bytes();
    !(first.len() >= 2 && first[0].is_ascii_alphabetic() && first[1] == b':')
}

#[derive(Debug, Clone, Copy, Dupe, PartialEq, Eq, Allocative)]
pub enum BuiltinBazelToolsSourceKind {
    Directory,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct BuiltinBazelToolsSourceFileValue {
    path: CompactString,
    bytes: Arc<[u8]>,
    sha256: [u8; 32],
    executable: bool,
}

impl BuiltinBazelToolsSourceFileValue {
    pub fn path(&self) -> &str {
        self.path.as_str()
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn sha256(&self) -> [u8; 32] {
        self.sha256
    }

    pub fn executable(&self) -> bool {
        self.executable
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum BuiltinBazelToolsSourceFileError {
    InvalidPath {
        path: CompactString,
    },
    WrongKind {
        path: CompactString,
        actual: BuiltinBazelToolsSourceKind,
    },
    UnsupportedCatalog {
        path: CompactString,
    },
    Integrity {
        path: CompactString,
        expected_sha256: CompactString,
        actual_sha256: CompactString,
    },
}

impl fmt::Display for BuiltinBazelToolsSourceFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPath { path } => {
                write!(f, "invalid built-in bazel_tools source-file path {path:?}")
            }
            Self::WrongKind { path, actual } => write!(
                f,
                "built-in bazel_tools source-file path {path:?} has wrong kind {actual:?}"
            ),
            Self::UnsupportedCatalog { path } => write!(
                f,
                "built-in bazel_tools source-file path {path:?} is outside the Bazel 9.2 catalog"
            ),
            Self::Integrity {
                path,
                expected_sha256,
                actual_sha256,
            } => write!(
                f,
                "built-in bazel_tools file {path:?} has SHA-256 {actual_sha256}, expected {expected_sha256}"
            ),
        }
    }
}

impl std::error::Error for BuiltinBazelToolsSourceFileError {}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct BuiltinBazelToolsSourceFileKey {
    snapshot: BuiltinBazelToolsSnapshot,
    path: CompactString,
}

impl BuiltinBazelToolsSourceFileKey {
    pub fn new(snapshot: BuiltinBazelToolsSnapshot, path: impl Into<CompactString>) -> Self {
        Self {
            snapshot,
            path: path.into(),
        }
    }
}

impl fmt::Display for BuiltinBazelToolsSourceFileKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "builtin-bazel-tools-source-file:{}", self.path)
    }
}

fn validated_file(
    path: CompactString,
    bytes: &'static [u8],
    expected_sha256: &'static str,
    executable: bool,
) -> Result<BuiltinBazelToolsSourceFileValue, BuiltinBazelToolsSourceFileError> {
    let sha256 = file_sha256(bytes);
    let actual_sha256 = hex::encode(sha256);
    if actual_sha256 != expected_sha256 {
        return Err(BuiltinBazelToolsSourceFileError::Integrity {
            path,
            expected_sha256: CompactString::new(expected_sha256),
            actual_sha256: CompactString::from(actual_sha256),
        });
    }
    Ok(BuiltinBazelToolsSourceFileValue {
        path,
        bytes: Arc::from(bytes),
        sha256,
        executable,
    })
}

fn lookup(
    path: CompactString,
) -> Result<BuiltinBazelToolsSourceFileValue, BuiltinBazelToolsSourceFileError> {
    if !valid_relative_path(path.as_str()) {
        return Err(BuiltinBazelToolsSourceFileError::InvalidPath { path });
    }
    if let Some(entry) = CATALOG.iter().find(|entry| entry.path == path.as_str()) {
        return validated_file(path, entry.bytes, entry.expected_sha256, entry.executable);
    }
    let prefix = format!("{path}/");
    if CATALOG.iter().any(|entry| entry.path.starts_with(&prefix)) {
        return Err(BuiltinBazelToolsSourceFileError::WrongKind {
            path,
            actual: BuiltinBazelToolsSourceKind::Directory,
        });
    }
    Err(BuiltinBazelToolsSourceFileError::UnsupportedCatalog { path })
}

#[async_trait]
impl Key for BuiltinBazelToolsSourceFileKey {
    type Value = Arc<Result<BuiltinBazelToolsSourceFileValue, BuiltinBazelToolsSourceFileError>>;

    async fn compute(
        &self,
        _ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        match self.snapshot {
            BuiltinBazelToolsSnapshot::Bazel9_2 => Arc::new(lookup(self.path.clone())),
        }
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }

    fn validity(_value: &Self::Value) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integrity_failure_is_exercised_through_validation() {
        let entry = &CATALOG[0];
        let error = validated_file(
            CompactString::new(entry.path),
            b"changed",
            entry.expected_sha256,
            entry.executable,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            BuiltinBazelToolsSourceFileError::Integrity { .. }
        ));
    }
}
