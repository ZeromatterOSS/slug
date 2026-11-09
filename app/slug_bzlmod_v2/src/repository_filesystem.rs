/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select either.
 */

//! Historical verified filesystem-call facts. Core owns physical mutation and
//! provisional roots; these immutable observations never authorize source reads.

use std::fmt;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use compact_str::CompactString;
use dice::DiceComputations;
use dice::InjectedKey;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use slug_workspace_v2::NormalizedAbsolutePath;

use crate::GeneratedRepositoryArchiveEffect;
use crate::GeneratedRepositoryFileEffectPlan;
use crate::HostSelectedExtensionOwner;
use crate::generated_repository_archive_sha256_hex;
use crate::generated_repository_archive_sha256_sri;

/// Slug-native filesystem-call bound shared by all invocations in one command.
pub const REPOSITORY_FILESYSTEM_MAX_CALLS: usize = 128;

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct RepositoryFilesystemCallId {
    owner: Arc<HostSelectedExtensionOwner>,
    ordinal: usize,
    call_index: usize,
}

impl RepositoryFilesystemCallId {
    pub fn new(owner: Arc<HostSelectedExtensionOwner>, ordinal: usize, call_index: usize) -> Self {
        Self {
            owner,
            ordinal,
            call_index,
        }
    }

    pub fn owner(&self) -> &Arc<HostSelectedExtensionOwner> {
        &self.owner
    }
    pub fn ordinal(&self) -> usize {
        self.ordinal
    }
    pub fn call_index(&self) -> usize {
        self.call_index
    }
}

/// One general repository filesystem operation. Deletion addresses a non-root,
/// normalized relative path; transcript admission revalidates public variants.
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub enum RepositoryFilesystemOperation {
    Archive(GeneratedRepositoryArchiveEffect),
    Delete(CompactString),
}

impl RepositoryFilesystemOperation {
    pub fn delete(
        path: CompactString,
    ) -> Result<Self, crate::GeneratedRepositoryFileEffectPlanError> {
        if !crate::generated_repository_file_effect::valid_delete_path(&path) {
            return Err(crate::GeneratedRepositoryFileEffectPlanError::InvalidPath(
                path,
            ));
        }
        Ok(Self::Delete(path))
    }
    pub fn archive(&self) -> Option<&GeneratedRepositoryArchiveEffect> {
        match self {
            Self::Archive(archive) => Some(archive),
            Self::Delete(_) => None,
        }
    }
    pub fn delete_path(&self) -> Option<&str> {
        match self {
            Self::Delete(path) => Some(path),
            Self::Archive(_) => None,
        }
    }
}

/// Complete replay descriptor, including every normalized archive option and
/// every preceding ordered effect. Enclosing source and policy remain tracked.
#[doc(hidden)]
#[derive(Clone, PartialEq, Eq, Hash, Allocative)]
pub struct RepositoryFilesystemRequest {
    id: RepositoryFilesystemCallId,
    generated_repository: CompactString,
    operation: RepositoryFilesystemOperation,
    prefix: GeneratedRepositoryFileEffectPlan,
}

impl RepositoryFilesystemRequest {
    pub fn new(
        id: RepositoryFilesystemCallId,
        generated_repository: CompactString,
        operation: RepositoryFilesystemOperation,
        prefix: GeneratedRepositoryFileEffectPlan,
    ) -> Self {
        Self {
            id,
            generated_repository,
            operation,
            prefix,
        }
    }

    pub fn id(&self) -> &RepositoryFilesystemCallId {
        &self.id
    }
    pub fn generated_repository(&self) -> &str {
        &self.generated_repository
    }
    pub fn operation(&self) -> &RepositoryFilesystemOperation {
        &self.operation
    }
    pub fn archive(&self) -> Option<&GeneratedRepositoryArchiveEffect> {
        self.operation.archive()
    }
    pub fn prefix(&self) -> &GeneratedRepositoryFileEffectPlan {
        &self.prefix
    }
}

impl fmt::Debug for RepositoryFilesystemRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RepositoryFilesystemRequest")
            .field("id", &self.id)
            .field("generated_repository", &self.generated_repository)
            .field("operation", &self.operation)
            .field(
                "prefix_effect_count",
                &self.prefix.ordered_effects().count(),
            )
            .finish()
    }
}

/// Core publishes the actual result only after completing the physical operation.
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub enum RepositoryFilesystemResult {
    Archive([u8; 32]),
    Delete(bool),
}

impl RepositoryFilesystemResult {
    pub fn sha256(&self) -> Option<[u8; 32]> {
        match self {
            Self::Archive(sha256) => Some(*sha256),
            Self::Delete(_) => None,
        }
    }
    pub fn sha256_hex(&self) -> Option<String> {
        self.sha256().map(generated_repository_archive_sha256_hex)
    }
    pub fn sha256_sri(&self) -> Option<String> {
        self.sha256().map(generated_repository_archive_sha256_sri)
    }
    pub fn deleted(&self) -> Option<bool> {
        match self {
            Self::Delete(deleted) => Some(*deleted),
            Self::Archive(_) => None,
        }
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct RepositoryFilesystemRecord {
    request: Arc<RepositoryFilesystemRequest>,
    result: RepositoryFilesystemResult,
}

impl RepositoryFilesystemRecord {
    pub fn new(
        request: Arc<RepositoryFilesystemRequest>,
        result: RepositoryFilesystemResult,
    ) -> Self {
        Self { request, result }
    }
    pub fn request(&self) -> &RepositoryFilesystemRequest {
        &self.request
    }
    pub fn result(&self) -> &RepositoryFilesystemResult {
        &self.result
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum RepositoryFilesystemTranscriptError {
    DuplicateCall(RepositoryFilesystemCallId),
    ConflictingRequest(RepositoryFilesystemCallId),
    NoncontiguousCall {
        id: RepositoryFilesystemCallId,
        expected: usize,
    },
    InvalidPrefix(RepositoryFilesystemCallId),
    ChecksumMismatch(RepositoryFilesystemCallId),
    ResultKindMismatch(RepositoryFilesystemCallId),
    InvalidOperation(RepositoryFilesystemCallId),
    TooManyCalls,
}

/// Ordered historical successes across a command, with contiguous indices and
/// monotone complete effect prefixes within each repository invocation.
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative, Dupe)]
pub struct RepositoryFilesystemTranscript {
    records: Arc<[RepositoryFilesystemRecord]>,
}

impl RepositoryFilesystemTranscript {
    pub fn empty() -> Self {
        Self {
            records: Arc::from([]),
        }
    }
    pub fn records(&self) -> &[RepositoryFilesystemRecord] {
        &self.records
    }

    pub fn find(&self, id: &RepositoryFilesystemCallId) -> Option<&RepositoryFilesystemRecord> {
        self.records.iter().find(|record| record.request.id() == id)
    }

    pub fn matching_record(
        &self,
        request: &RepositoryFilesystemRequest,
    ) -> Result<Option<&RepositoryFilesystemRecord>, RepositoryFilesystemTranscriptError> {
        match self.find(request.id()) {
            Some(record) if record.request() != request => Err(
                RepositoryFilesystemTranscriptError::ConflictingRequest(request.id().clone()),
            ),
            record => Ok(record),
        }
    }

    pub fn records_for<'a>(
        &'a self,
        owner: &'a HostSelectedExtensionOwner,
        ordinal: usize,
    ) -> impl Iterator<Item = &'a RepositoryFilesystemRecord> {
        self.records.iter().filter(move |record| {
            record.request.id().owner().as_ref() == owner
                && record.request.id().ordinal() == ordinal
        })
    }

    pub fn for_owner(&self, owner: &HostSelectedExtensionOwner, ordinal: usize) -> Self {
        Self {
            records: self.records_for(owner, ordinal).cloned().collect(),
        }
    }

    pub fn append(
        &self,
        record: RepositoryFilesystemRecord,
    ) -> Result<Self, RepositoryFilesystemTranscriptError> {
        let request = record.request();
        let id = request.id();
        if self.matching_record(request)?.is_some() {
            return Err(RepositoryFilesystemTranscriptError::DuplicateCall(
                id.clone(),
            ));
        }
        if self.records.len() >= REPOSITORY_FILESYSTEM_MAX_CALLS {
            return Err(RepositoryFilesystemTranscriptError::TooManyCalls);
        }
        let expected = self.records_for(id.owner(), id.ordinal()).count();
        if id.call_index() != expected {
            return Err(RepositoryFilesystemTranscriptError::NoncontiguousCall {
                id: id.clone(),
                expected,
            });
        }
        if request.prefix().filesystem_operation_count() != expected {
            return Err(RepositoryFilesystemTranscriptError::InvalidPrefix(
                id.clone(),
            ));
        }
        if let Some(previous) = self.records_for(id.owner(), id.ordinal()).last() {
            if previous.request().generated_repository() != request.generated_repository()
                || !extends_verified_prefix(previous.request(), request.prefix())
            {
                return Err(RepositoryFilesystemTranscriptError::InvalidPrefix(
                    id.clone(),
                ));
            }
        }
        if matches!(request.operation(), RepositoryFilesystemOperation::Archive(archive) if archive.expected_sha256().is_none())
        {
            return Err(RepositoryFilesystemTranscriptError::InvalidOperation(
                id.clone(),
            ));
        }
        request
            .prefix()
            .with_operation(request.operation())
            .map_err(|_| RepositoryFilesystemTranscriptError::InvalidOperation(id.clone()))?;
        match (request.operation(), record.result()) {
            (
                RepositoryFilesystemOperation::Archive(archive),
                RepositoryFilesystemResult::Archive(digest),
            ) => {
                if archive.expected_sha256() != Some(*digest) {
                    return Err(RepositoryFilesystemTranscriptError::ChecksumMismatch(
                        id.clone(),
                    ));
                }
            }
            (RepositoryFilesystemOperation::Delete(_), RepositoryFilesystemResult::Delete(_)) => {}
            _ => {
                return Err(RepositoryFilesystemTranscriptError::ResultKindMismatch(
                    id.clone(),
                ));
            }
        }
        let mut records = Vec::with_capacity(self.records.len() + 1);
        records.extend_from_slice(&self.records);
        records.push(record);
        Ok(Self {
            records: records.into(),
        })
    }
}

fn extends_verified_prefix(
    previous: &RepositoryFilesystemRequest,
    prefix: &GeneratedRepositoryFileEffectPlan,
) -> bool {
    previous
        .prefix()
        .with_operation(previous.operation())
        .is_ok_and(|completed| completed.is_prefix_of(prefix))
}

impl Default for RepositoryFilesystemTranscript {
    fn default() -> Self {
        Self::empty()
    }
}

/// Terminal immutable handoff binding. Core still owns and validates the live
/// provisional root; this reference is neither a path nor a source certificate.
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct RepositoryFilesystemRealization {
    lease_nonce: u64,
    owner: Arc<HostSelectedExtensionOwner>,
    ordinal: usize,
    transcript: RepositoryFilesystemTranscript,
    terminal_plan: GeneratedRepositoryFileEffectPlan,
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum RepositoryFilesystemRealizationError {
    EmptyTranscript,
    ForeignInvocation,
    InvalidTerminalPlan,
}

impl RepositoryFilesystemRealization {
    pub fn new(
        lease_nonce: u64,
        owner: Arc<HostSelectedExtensionOwner>,
        ordinal: usize,
        transcript: RepositoryFilesystemTranscript,
        terminal_plan: GeneratedRepositoryFileEffectPlan,
    ) -> Result<Self, RepositoryFilesystemRealizationError> {
        let last = transcript
            .records()
            .last()
            .ok_or(RepositoryFilesystemRealizationError::EmptyTranscript)?;
        if transcript.records_for(&owner, ordinal).count() != transcript.records().len() {
            return Err(RepositoryFilesystemRealizationError::ForeignInvocation);
        }
        if terminal_plan.filesystem_operation_count() != transcript.records().len()
            || !extends_verified_prefix(last.request(), &terminal_plan)
        {
            return Err(RepositoryFilesystemRealizationError::InvalidTerminalPlan);
        }
        Ok(Self {
            lease_nonce,
            owner,
            ordinal,
            transcript,
            terminal_plan,
        })
    }

    pub fn lease_nonce(&self) -> u64 {
        self.lease_nonce
    }
    pub fn owner(&self) -> &Arc<HostSelectedExtensionOwner> {
        &self.owner
    }
    pub fn ordinal(&self) -> usize {
        self.ordinal
    }
    pub fn transcript(&self) -> &RepositoryFilesystemTranscript {
        &self.transcript
    }
    pub fn terminal_plan(&self) -> &GeneratedRepositoryFileEffectPlan {
        &self.terminal_plan
    }
}

/// The nonce scopes historical call observations to the active command lease.
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub enum RepositoryFilesystemEpoch {
    Revoked,
    Active {
        lease_nonce: u64,
        transcript: RepositoryFilesystemTranscript,
    },
}

impl RepositoryFilesystemEpoch {
    pub fn revoked() -> Self {
        Self::Revoked
    }
    pub fn active(lease_nonce: u64, transcript: RepositoryFilesystemTranscript) -> Self {
        Self::Active {
            lease_nonce,
            transcript,
        }
    }
    pub fn transcript(&self) -> Option<&RepositoryFilesystemTranscript> {
        match self {
            Self::Revoked => None,
            Self::Active { transcript, .. } => Some(transcript),
        }
    }
    pub fn lease_nonce(&self) -> Option<u64> {
        match self {
            Self::Revoked => None,
            Self::Active { lease_nonce, .. } => Some(*lease_nonce),
        }
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct RepositoryFilesystemEpochKey {
    workspace: NormalizedAbsolutePath,
}

impl RepositoryFilesystemEpochKey {
    pub fn new(workspace: NormalizedAbsolutePath) -> Self {
        Self { workspace }
    }
    pub fn workspace(&self) -> &NormalizedAbsolutePath {
        &self.workspace
    }
}

impl fmt::Display for RepositoryFilesystemEpochKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "repository-filesystem-epoch:{}", self.workspace)
    }
}

impl InjectedKey for RepositoryFilesystemEpochKey {
    type Value = RepositoryFilesystemEpoch;
    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct RepositoryFilesystemObservationKey {
    workspace: NormalizedAbsolutePath,
}

impl RepositoryFilesystemObservationKey {
    pub fn new(workspace: NormalizedAbsolutePath) -> Self {
        Self { workspace }
    }
}

impl fmt::Display for RepositoryFilesystemObservationKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "observed-repository-filesystem:{}",
            self.workspace
        )
    }
}

#[async_trait]
impl Key for RepositoryFilesystemObservationKey {
    type Value = RepositoryFilesystemEpoch;
    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        ctx.compute(&RepositoryFilesystemEpochKey::new(self.workspace.clone()))
            .await
            .expect("Core injects a repository filesystem epoch before every command attempt")
    }
    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
    fn validity(value: &Self::Value) -> bool {
        matches!(value, RepositoryFilesystemEpoch::Active { .. })
    }
}

#[cfg(test)]
mod tests {
    use dice::DetectCycles;
    use dice::Dice;

    use super::*;
    use crate::GeneratedRepositoryOrderedEffectRef;

    fn archive(sha256: [u8; 32]) -> GeneratedRepositoryArchiveEffect {
        let mut builder = GeneratedRepositoryFileEffectPlan::builder();
        builder
            .push_archive(
                Arc::from(["https://example.test/independent.tar.xz".into()]),
                sha256,
                "arbitrary/tree".into(),
            )
            .unwrap();
        let plan = builder.finish();
        let GeneratedRepositoryOrderedEffectRef::Archive(archive) =
            plan.ordered_effects().next().unwrap()
        else {
            panic!("archive effect missing")
        };
        archive.clone()
    }

    fn request(
        owner: &Arc<HostSelectedExtensionOwner>,
        ordinal: usize,
        index: usize,
        content: &'static [u8],
    ) -> RepositoryFilesystemRequest {
        let mut prefix = GeneratedRepositoryFileEffectPlan::builder();
        prefix
            .push("before.txt".into(), Arc::from(content), false)
            .unwrap();
        for _ in 0..index {
            let archive = archive([7; 32]);
            prefix
                .push_archive(
                    Arc::from(archive.urls()),
                    archive.expected_sha256().expect("pinned archive fixture"),
                    archive.strip_prefix().into(),
                )
                .unwrap();
        }
        RepositoryFilesystemRequest::new(
            RepositoryFilesystemCallId::new(owner.clone(), ordinal, index),
            "unrelated+repo".into(),
            RepositoryFilesystemOperation::Archive(archive([7; 32])),
            prefix.finish(),
        )
    }

    fn record(request: RepositoryFilesystemRequest) -> RepositoryFilesystemRecord {
        RepositoryFilesystemRecord::new(
            Arc::new(request),
            RepositoryFilesystemResult::Archive([7; 32]),
        )
    }

    #[test]
    fn transcript_rejects_reordered_conflicting_and_changed_prefix_calls() {
        let owner = Arc::new(HostSelectedExtensionOwner::testing("unrelated"));
        let first = request(&owner, 3, 0, b"before");
        let second = request(&owner, 3, 1, b"before");
        let empty = RepositoryFilesystemTranscript::empty();
        assert!(matches!(
            empty.append(record(second.clone())),
            Err(RepositoryFilesystemTranscriptError::NoncontiguousCall { expected: 0, .. })
        ));
        let one = empty.append(record(first.clone())).unwrap();
        assert_eq!(
            one.matching_record(&first).unwrap().unwrap().request(),
            &first
        );
        assert!(matches!(
            one.append(record(first.clone())),
            Err(RepositoryFilesystemTranscriptError::DuplicateCall(_))
        ));
        let changed = request(&owner, 3, 0, b"changed");
        assert!(matches!(
            one.matching_record(&changed),
            Err(RepositoryFilesystemTranscriptError::ConflictingRequest(_))
        ));
        assert!(matches!(
            one.append(record(changed)),
            Err(RepositoryFilesystemTranscriptError::ConflictingRequest(_))
        ));
        assert!(matches!(
            one.append(record(request(&owner, 3, 1, b"changed"))),
            Err(RepositoryFilesystemTranscriptError::InvalidPrefix(_))
        ));
        let mut changed_repository = second.clone();
        changed_repository.generated_repository = "different+repo".into();
        assert!(matches!(
            one.append(record(changed_repository)),
            Err(RepositoryFilesystemTranscriptError::InvalidPrefix(_))
        ));
        let two = one.append(record(second)).unwrap();
        assert_eq!(two.records().len(), 2);
        let foreign = Arc::new(HostSelectedExtensionOwner::testing("relocated"));
        let shared = two
            .append(record(request(&foreign, 3, 0, b"foreign")))
            .unwrap();
        assert_eq!(shared.for_owner(&owner, 3), two);
        assert_eq!(shared.records_for(&foreign, 3).count(), 1);
        assert!(
            two.find(&RepositoryFilesystemCallId::new(owner, 4, 0))
                .is_none()
        );
    }

    #[test]
    fn archive_request_and_verified_digest_are_structural() {
        let owner = Arc::new(HostSelectedExtensionOwner::testing("owner"));
        let original = request(&owner, 0, 0, b"prefix");
        let mut mirrors = GeneratedRepositoryFileEffectPlan::builder();
        mirrors
            .push_archive(
                Arc::from([
                    "https://one.test/a.tar.xz".into(),
                    "https://two.test/b.tar.xz".into(),
                ]),
                [7; 32],
                "different/prefix".into(),
            )
            .unwrap();
        let plan = mirrors.finish();
        let GeneratedRepositoryOrderedEffectRef::Archive(effect) =
            plan.ordered_effects().next().unwrap()
        else {
            panic!("archive effect missing")
        };
        let mut ordered = original.clone();
        ordered.operation = RepositoryFilesystemOperation::Archive(effect.clone());
        let mut reversed = GeneratedRepositoryFileEffectPlan::builder();
        reversed
            .push_archive(
                Arc::from([effect.urls()[1].clone(), effect.urls()[0].clone()]),
                effect.expected_sha256().expect("pinned archive fixture"),
                effect.strip_prefix().into(),
            )
            .unwrap();
        let plan = reversed.finish();
        let GeneratedRepositoryOrderedEffectRef::Archive(effect) =
            plan.ordered_effects().next().unwrap()
        else {
            panic!("archive effect missing")
        };
        let mut reordered = ordered.clone();
        reordered.operation = RepositoryFilesystemOperation::Archive(effect.clone());
        assert_ne!(ordered, reordered);
        let transcript = RepositoryFilesystemTranscript::empty()
            .append(record(ordered))
            .unwrap();
        assert!(matches!(
            transcript.matching_record(&reordered),
            Err(RepositoryFilesystemTranscriptError::ConflictingRequest(_))
        ));
        let mut changed = original.clone();
        changed.operation = RepositoryFilesystemOperation::Archive(archive([9; 32]));
        assert_ne!(original, changed);
        let bad = RepositoryFilesystemRecord::new(
            Arc::new(original),
            RepositoryFilesystemResult::Archive([9; 32]),
        );
        assert!(matches!(
            RepositoryFilesystemTranscript::empty().append(bad),
            Err(RepositoryFilesystemTranscriptError::ChecksumMismatch(_))
        ));
        let result = RepositoryFilesystemResult::Archive([7; 32]);
        assert_eq!(
            result.sha256_hex().unwrap(),
            generated_repository_archive_sha256_hex([7; 32])
        );
        assert_eq!(
            result.sha256_sri().unwrap(),
            generated_repository_archive_sha256_sri([7; 32])
        );
    }

    #[test]
    fn delete_result_kind_path_prefix_and_boolean_are_structural() {
        let owner = Arc::new(HostSelectedExtensionOwner::testing("independent-delete"));
        let base = request(&owner, 0, 0, b"present");
        let mut delete = base.clone();
        delete.operation = RepositoryFilesystemOperation::delete("before.txt".into()).unwrap();
        let make = |request, deleted| {
            RepositoryFilesystemRecord::new(
                Arc::new(request),
                RepositoryFilesystemResult::Delete(deleted),
            )
        };
        let present = RepositoryFilesystemTranscript::empty()
            .append(make(delete.clone(), true))
            .unwrap();
        let absent = RepositoryFilesystemTranscript::empty()
            .append(make(delete.clone(), false))
            .unwrap();
        assert_ne!(present, absent);
        assert_eq!(present.records()[0].result().deleted(), Some(true));
        assert_eq!(absent.records()[0].result().deleted(), Some(false));
        assert_eq!(present.records()[0].result().sha256(), None);
        assert_eq!(present.records()[0].result().sha256_hex(), None);
        assert_eq!(present.records()[0].result().sha256_sri(), None);
        assert_eq!(RepositoryFilesystemResult::Archive([7; 32]).deleted(), None);
        assert!(matches!(
            RepositoryFilesystemTranscript::empty().append(record(delete.clone())),
            Err(RepositoryFilesystemTranscriptError::ResultKindMismatch(_))
        ));
        assert!(matches!(
            RepositoryFilesystemTranscript::empty().append(make(base, true)),
            Err(RepositoryFilesystemTranscriptError::ResultKindMismatch(_))
        ));
        let mut changed = delete.clone();
        changed.operation = RepositoryFilesystemOperation::Delete("other".into());
        assert!(matches!(
            present.matching_record(&changed),
            Err(RepositoryFilesystemTranscriptError::ConflictingRequest(_))
        ));
        changed = delete.clone();
        changed.prefix = request(&owner, 0, 0, b"changed").prefix;
        assert!(matches!(
            present.matching_record(&changed),
            Err(RepositoryFilesystemTranscriptError::ConflictingRequest(_))
        ));
        for path in [
            "",
            ".",
            "..",
            "/absolute",
            "C:/host",
            "a/../b",
            "a//b",
            "a/",
            "a\\b",
            "a\0b",
        ] {
            assert!(RepositoryFilesystemOperation::delete(path.into()).is_err());
            changed = delete.clone();
            changed.operation = RepositoryFilesystemOperation::Delete(path.into());
            assert!(matches!(
                RepositoryFilesystemTranscript::empty().append(make(changed, false)),
                Err(RepositoryFilesystemTranscriptError::InvalidOperation(_))
            ));
        }
        for path in [
            "x".repeat(4097),
            std::iter::repeat_n("x", 257).collect::<Vec<_>>().join("/"),
        ] {
            assert!(RepositoryFilesystemOperation::delete(path.clone().into()).is_err());
            let mut changed = delete.clone();
            changed.operation = RepositoryFilesystemOperation::Delete(path.into());
            assert!(matches!(
                RepositoryFilesystemTranscript::empty().append(make(changed, false)),
                Err(RepositoryFilesystemTranscriptError::InvalidOperation(_))
            ));
        }
        let terminal = delete.prefix().with_operation(delete.operation()).unwrap();
        let yes =
            RepositoryFilesystemRealization::new(1, owner.clone(), 0, present, terminal.clone())
                .unwrap();
        let no = RepositoryFilesystemRealization::new(1, owner, 0, absent, terminal).unwrap();
        assert_eq!(yes.terminal_plan().archive_count(), 0);
        assert!(yes.terminal_plan().has_filesystem_operations());
        assert_ne!(yes, no);
        use std::hash::Hash;
        use std::hash::Hasher;
        let mut a = std::collections::hash_map::DefaultHasher::new();
        let mut b = std::collections::hash_map::DefaultHasher::new();
        yes.hash(&mut a);
        no.hash(&mut b);
        assert_ne!(a.finish(), b.finish());
    }

    #[test]
    fn mixed_filesystem_transcript_requires_exact_contiguous_prefix_and_terminal() {
        let owner = Arc::new(HostSelectedExtensionOwner::testing("mixed"));
        let first = request(&owner, 0, 0, b"before");
        let one = RepositoryFilesystemTranscript::empty()
            .append(record(first.clone()))
            .unwrap();
        let mut prefix = first.prefix().with_operation(first.operation()).unwrap();
        let delete_operation = RepositoryFilesystemOperation::delete("tree/nested".into()).unwrap();
        let second = RepositoryFilesystemRequest::new(
            RepositoryFilesystemCallId::new(owner.clone(), 0, 1),
            "unrelated+repo".into(),
            delete_operation.clone(),
            prefix.clone(),
        );
        let two = one
            .append(RepositoryFilesystemRecord::new(
                Arc::new(second.clone()),
                RepositoryFilesystemResult::Delete(true),
            ))
            .unwrap();
        prefix = prefix.with_operation(&delete_operation).unwrap();
        let third = RepositoryFilesystemRequest::new(
            RepositoryFilesystemCallId::new(owner.clone(), 0, 2),
            "unrelated+repo".into(),
            RepositoryFilesystemOperation::Archive(archive([7; 32])),
            prefix.clone(),
        );
        let three = two.append(record(third.clone())).unwrap();
        let terminal = prefix.with_operation(third.operation()).unwrap();
        assert_eq!(terminal.filesystem_operation_count(), 3);
        assert_eq!(terminal.archive_count(), 2);
        RepositoryFilesystemRealization::new(1, owner.clone(), 0, three.clone(), terminal).unwrap();
        assert!(matches!(
            RepositoryFilesystemRealization::new(1, owner.clone(), 0, three, prefix),
            Err(RepositoryFilesystemRealizationError::InvalidTerminalPlan)
        ));
        let mut changed = second.clone();
        changed.id = RepositoryFilesystemCallId::new(owner.clone(), 0, 2);
        assert!(matches!(
            one.append(RepositoryFilesystemRecord::new(
                Arc::new(changed),
                RepositoryFilesystemResult::Delete(true)
            )),
            Err(RepositoryFilesystemTranscriptError::NoncontiguousCall { expected: 1, .. })
        ));
        let mut changed = third;
        changed.prefix = first
            .prefix()
            .with_operation(first.operation())
            .unwrap()
            .with_operation(&RepositoryFilesystemOperation::Delete("other".into()))
            .unwrap();
        assert!(matches!(
            two.append(record(changed)),
            Err(RepositoryFilesystemTranscriptError::InvalidPrefix(_))
        ));
    }

    #[test]
    fn transcript_call_cap_is_shared_across_invocations() {
        let owner = Arc::new(HostSelectedExtensionOwner::testing("owner"));
        let mut transcript = RepositoryFilesystemTranscript::empty();
        for ordinal in 0..REPOSITORY_FILESYSTEM_MAX_CALLS {
            transcript = transcript
                .append(record(request(&owner, ordinal, 0, b"")))
                .unwrap();
        }
        assert!(matches!(
            transcript.append(record(request(
                &owner,
                REPOSITORY_FILESYSTEM_MAX_CALLS,
                0,
                b""
            ))),
            Err(RepositoryFilesystemTranscriptError::TooManyCalls)
        ));
    }

    #[test]
    fn terminal_realization_binds_complete_plan_invocation_and_lease() {
        let owner = Arc::new(HostSelectedExtensionOwner::testing("owner"));
        let transcript = RepositoryFilesystemTranscript::empty()
            .append(record(request(&owner, 0, 0, b"before")))
            .unwrap();
        let plan = request(&owner, 0, 1, b"before").prefix().clone();
        let realization = RepositoryFilesystemRealization::new(
            1,
            owner.clone(),
            0,
            transcript.clone(),
            plan.clone(),
        )
        .unwrap();
        assert_eq!(realization.lease_nonce(), 1);
        assert_eq!(realization.owner(), &owner);
        assert_eq!(realization.ordinal(), 0);
        assert_eq!(realization.transcript(), &transcript);
        assert_eq!(realization.terminal_plan(), &plan);
        let mut suffix = GeneratedRepositoryFileEffectPlan::builder();
        suffix
            .push("before.txt".into(), Arc::from(&b"before"[..]), false)
            .unwrap();
        let effect = archive([7; 32]);
        suffix
            .push_archive(
                Arc::from(effect.urls()),
                effect.expected_sha256().expect("pinned archive fixture"),
                effect.strip_prefix().into(),
            )
            .unwrap();
        suffix
            .push("after.txt".into(), Arc::from(&b"after"[..]), true)
            .unwrap();
        let with_suffix = RepositoryFilesystemRealization::new(
            1,
            owner.clone(),
            0,
            transcript.clone(),
            suffix.finish(),
        )
        .unwrap();
        assert_ne!(realization, with_suffix);
        assert_ne!(
            realization,
            RepositoryFilesystemRealization::new(
                2,
                owner.clone(),
                0,
                transcript.clone(),
                plan.clone()
            )
            .unwrap()
        );
        assert!(matches!(
            RepositoryFilesystemRealization::new(
                1,
                owner.clone(),
                1,
                transcript.clone(),
                plan.clone()
            ),
            Err(RepositoryFilesystemRealizationError::ForeignInvocation)
        ));
        assert!(matches!(
            RepositoryFilesystemRealization::new(
                1,
                owner.clone(),
                0,
                transcript.clone(),
                request(&owner, 0, 1, b"changed").prefix().clone()
            ),
            Err(RepositoryFilesystemRealizationError::InvalidTerminalPlan)
        ));
        assert!(matches!(
            RepositoryFilesystemRealization::new(
                1,
                owner,
                0,
                RepositoryFilesystemTranscript::empty(),
                plan
            ),
            Err(RepositoryFilesystemRealizationError::EmptyTranscript)
        ));
    }

    #[tokio::test]
    async fn archive_epoch_tracks_nonce_transcript_and_revocation_in_dice() {
        let workspace = NormalizedAbsolutePath::new(std::path::Path::new("/archive-test")).unwrap();
        let injected = RepositoryFilesystemEpochKey::new(workspace.clone());
        let observed = RepositoryFilesystemObservationKey::new(workspace);
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let owner = Arc::new(HostSelectedExtensionOwner::testing("owner"));
        let transcript = RepositoryFilesystemTranscript::empty()
            .append(record(request(&owner, 0, 0, b"")))
            .unwrap();
        let original =
            RepositoryFilesystemEpoch::active(1, RepositoryFilesystemTranscript::empty());
        for epoch in [
            original.clone(),
            RepositoryFilesystemEpoch::active(1, transcript),
            RepositoryFilesystemEpoch::active(2, RepositoryFilesystemTranscript::empty()),
            original,
            RepositoryFilesystemEpoch::revoked(),
        ] {
            let mut updater = dice.updater();
            updater
                .changed_to(vec![(injected.clone(), epoch.clone())])
                .unwrap();
            let actual = updater.commit().await.compute(&observed).await.unwrap();
            assert_eq!(actual, epoch);
            assert_eq!(
                RepositoryFilesystemObservationKey::validity(&actual),
                actual.lease_nonce().is_some()
            );
        }
        assert!(RepositoryFilesystemEpoch::revoked().transcript().is_none());
    }
}
