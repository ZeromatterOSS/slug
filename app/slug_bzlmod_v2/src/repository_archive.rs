/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select either.
 */

//! Historical verified archive-call facts. Core owns capture, extraction and
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
use crate::GeneratedRepositoryOrderedEffectRef;
use crate::HostSelectedExtensionOwner;
use crate::generated_repository_archive_sha256_hex;
use crate::generated_repository_archive_sha256_sri;

/// Slug-native archive-call bound shared by all invocations in one command.
pub const REPOSITORY_ARCHIVE_MAX_CALLS: usize = 128;

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct RepositoryArchiveCallId {
    owner: Arc<HostSelectedExtensionOwner>,
    ordinal: usize,
    call_index: usize,
}

impl RepositoryArchiveCallId {
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

/// Complete replay descriptor, including every normalized archive option and
/// every preceding ordered effect. Enclosing source and policy remain tracked.
#[doc(hidden)]
#[derive(Clone, PartialEq, Eq, Hash, Allocative)]
pub struct RepositoryArchiveRequest {
    id: RepositoryArchiveCallId,
    generated_repository: CompactString,
    archive: GeneratedRepositoryArchiveEffect,
    prefix: GeneratedRepositoryFileEffectPlan,
}

impl RepositoryArchiveRequest {
    pub fn new(
        id: RepositoryArchiveCallId,
        generated_repository: CompactString,
        archive: GeneratedRepositoryArchiveEffect,
        prefix: GeneratedRepositoryFileEffectPlan,
    ) -> Self {
        Self {
            id,
            generated_repository,
            archive,
            prefix,
        }
    }

    pub fn id(&self) -> &RepositoryArchiveCallId {
        &self.id
    }
    pub fn generated_repository(&self) -> &str {
        &self.generated_repository
    }
    pub fn archive(&self) -> &GeneratedRepositoryArchiveEffect {
        &self.archive
    }
    pub fn prefix(&self) -> &GeneratedRepositoryFileEffectPlan {
        &self.prefix
    }
}

impl fmt::Debug for RepositoryArchiveRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RepositoryArchiveRequest")
            .field("id", &self.id)
            .field("generated_repository", &self.generated_repository)
            .field("archive", &self.archive)
            .field(
                "prefix_effect_count",
                &self.prefix.ordered_effects().count(),
            )
            .finish()
    }
}

/// Core constructs this only after successful capture, checksum verification
/// and extraction. It carries the actual digest, without a mutable physical path.
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct RepositoryArchiveResult {
    sha256: [u8; 32],
}

impl RepositoryArchiveResult {
    pub fn verified(sha256: [u8; 32]) -> Self {
        Self { sha256 }
    }
    pub fn sha256(&self) -> [u8; 32] {
        self.sha256
    }
    pub fn sha256_hex(&self) -> String {
        generated_repository_archive_sha256_hex(self.sha256)
    }
    pub fn sha256_sri(&self) -> String {
        generated_repository_archive_sha256_sri(self.sha256)
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct RepositoryArchiveRecord {
    request: Arc<RepositoryArchiveRequest>,
    result: RepositoryArchiveResult,
}

impl RepositoryArchiveRecord {
    pub fn new(request: Arc<RepositoryArchiveRequest>, result: RepositoryArchiveResult) -> Self {
        Self { request, result }
    }
    pub fn request(&self) -> &RepositoryArchiveRequest {
        &self.request
    }
    pub fn result(&self) -> &RepositoryArchiveResult {
        &self.result
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum RepositoryArchiveTranscriptError {
    DuplicateCall(RepositoryArchiveCallId),
    ConflictingRequest(RepositoryArchiveCallId),
    NoncontiguousCall {
        id: RepositoryArchiveCallId,
        expected: usize,
    },
    InvalidPrefix(RepositoryArchiveCallId),
    ChecksumMismatch(RepositoryArchiveCallId),
    TooManyCalls,
}

/// Ordered historical successes across a command, with contiguous indices and
/// monotone complete effect prefixes within each repository invocation.
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative, Dupe)]
pub struct RepositoryArchiveTranscript {
    records: Arc<[RepositoryArchiveRecord]>,
}

impl RepositoryArchiveTranscript {
    pub fn empty() -> Self {
        Self {
            records: Arc::from([]),
        }
    }
    pub fn records(&self) -> &[RepositoryArchiveRecord] {
        &self.records
    }

    pub fn find(&self, id: &RepositoryArchiveCallId) -> Option<&RepositoryArchiveRecord> {
        self.records.iter().find(|record| record.request.id() == id)
    }

    pub fn matching_record(
        &self,
        request: &RepositoryArchiveRequest,
    ) -> Result<Option<&RepositoryArchiveRecord>, RepositoryArchiveTranscriptError> {
        match self.find(request.id()) {
            Some(record) if record.request() != request => Err(
                RepositoryArchiveTranscriptError::ConflictingRequest(request.id().clone()),
            ),
            record => Ok(record),
        }
    }

    pub fn records_for<'a>(
        &'a self,
        owner: &'a HostSelectedExtensionOwner,
        ordinal: usize,
    ) -> impl Iterator<Item = &'a RepositoryArchiveRecord> {
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
        record: RepositoryArchiveRecord,
    ) -> Result<Self, RepositoryArchiveTranscriptError> {
        let request = record.request();
        let id = request.id();
        if self.matching_record(request)?.is_some() {
            return Err(RepositoryArchiveTranscriptError::DuplicateCall(id.clone()));
        }
        if self.records.len() >= REPOSITORY_ARCHIVE_MAX_CALLS {
            return Err(RepositoryArchiveTranscriptError::TooManyCalls);
        }
        let expected = self.records_for(id.owner(), id.ordinal()).count();
        if id.call_index() != expected {
            return Err(RepositoryArchiveTranscriptError::NoncontiguousCall {
                id: id.clone(),
                expected,
            });
        }
        if request.prefix().archive_count() != expected {
            return Err(RepositoryArchiveTranscriptError::InvalidPrefix(id.clone()));
        }
        if let Some(previous) = self.records_for(id.owner(), id.ordinal()).last() {
            if previous.request().generated_repository() != request.generated_repository()
                || !extends_verified_prefix(previous.request(), request.prefix())
            {
                return Err(RepositoryArchiveTranscriptError::InvalidPrefix(id.clone()));
            }
        }
        if record.result().sha256() != request.archive().sha256() {
            return Err(RepositoryArchiveTranscriptError::ChecksumMismatch(
                id.clone(),
            ));
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
    previous: &RepositoryArchiveRequest,
    prefix: &GeneratedRepositoryFileEffectPlan,
) -> bool {
    let mut actual = prefix.ordered_effects();
    previous
        .prefix()
        .ordered_effects()
        .chain(std::iter::once(
            GeneratedRepositoryOrderedEffectRef::Archive(previous.archive()),
        ))
        .all(|expected| match (expected, actual.next()) {
            (
                GeneratedRepositoryOrderedEffectRef::File(a),
                Some(GeneratedRepositoryOrderedEffectRef::File(b)),
            ) => a == b,
            (
                GeneratedRepositoryOrderedEffectRef::Archive(a),
                Some(GeneratedRepositoryOrderedEffectRef::Archive(b)),
            ) => a == b,
            (
                GeneratedRepositoryOrderedEffectRef::Symlink(a),
                Some(GeneratedRepositoryOrderedEffectRef::Symlink(b)),
            ) => a == b,
            _ => false,
        })
}

impl Default for RepositoryArchiveTranscript {
    fn default() -> Self {
        Self::empty()
    }
}

/// Terminal immutable handoff binding. Core still owns and validates the live
/// provisional root; this reference is neither a path nor a source certificate.
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct RepositoryArchiveRealization {
    lease_nonce: u64,
    owner: Arc<HostSelectedExtensionOwner>,
    ordinal: usize,
    transcript: RepositoryArchiveTranscript,
    terminal_plan: GeneratedRepositoryFileEffectPlan,
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum RepositoryArchiveRealizationError {
    EmptyTranscript,
    ForeignInvocation,
    InvalidTerminalPlan,
}

impl RepositoryArchiveRealization {
    pub fn new(
        lease_nonce: u64,
        owner: Arc<HostSelectedExtensionOwner>,
        ordinal: usize,
        transcript: RepositoryArchiveTranscript,
        terminal_plan: GeneratedRepositoryFileEffectPlan,
    ) -> Result<Self, RepositoryArchiveRealizationError> {
        let last = transcript
            .records()
            .last()
            .ok_or(RepositoryArchiveRealizationError::EmptyTranscript)?;
        if transcript.records_for(&owner, ordinal).count() != transcript.records().len() {
            return Err(RepositoryArchiveRealizationError::ForeignInvocation);
        }
        if terminal_plan.archive_count() != transcript.records().len()
            || !extends_verified_prefix(last.request(), &terminal_plan)
        {
            return Err(RepositoryArchiveRealizationError::InvalidTerminalPlan);
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
    pub fn transcript(&self) -> &RepositoryArchiveTranscript {
        &self.transcript
    }
    pub fn terminal_plan(&self) -> &GeneratedRepositoryFileEffectPlan {
        &self.terminal_plan
    }
}

/// The nonce scopes historical call observations to the active command lease.
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub enum RepositoryArchiveEpoch {
    Revoked,
    Active {
        lease_nonce: u64,
        transcript: RepositoryArchiveTranscript,
    },
}

impl RepositoryArchiveEpoch {
    pub fn revoked() -> Self {
        Self::Revoked
    }
    pub fn active(lease_nonce: u64, transcript: RepositoryArchiveTranscript) -> Self {
        Self::Active {
            lease_nonce,
            transcript,
        }
    }
    pub fn transcript(&self) -> Option<&RepositoryArchiveTranscript> {
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
pub struct RepositoryArchiveEpochKey {
    workspace: NormalizedAbsolutePath,
}

impl RepositoryArchiveEpochKey {
    pub fn new(workspace: NormalizedAbsolutePath) -> Self {
        Self { workspace }
    }
    pub fn workspace(&self) -> &NormalizedAbsolutePath {
        &self.workspace
    }
}

impl fmt::Display for RepositoryArchiveEpochKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "repository-archive-epoch:{}", self.workspace)
    }
}

impl InjectedKey for RepositoryArchiveEpochKey {
    type Value = RepositoryArchiveEpoch;
    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct RepositoryArchiveObservationKey {
    workspace: NormalizedAbsolutePath,
}

impl RepositoryArchiveObservationKey {
    pub fn new(workspace: NormalizedAbsolutePath) -> Self {
        Self { workspace }
    }
}

impl fmt::Display for RepositoryArchiveObservationKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "observed-repository-archive:{}", self.workspace)
    }
}

#[async_trait]
impl Key for RepositoryArchiveObservationKey {
    type Value = RepositoryArchiveEpoch;
    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        ctx.compute(&RepositoryArchiveEpochKey::new(self.workspace.clone()))
            .await
            .expect("Core injects a repository archive epoch before every command attempt")
    }
    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
    fn validity(value: &Self::Value) -> bool {
        matches!(value, RepositoryArchiveEpoch::Active { .. })
    }
}

#[cfg(test)]
mod tests {
    use dice::DetectCycles;
    use dice::Dice;

    use super::*;

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
    ) -> RepositoryArchiveRequest {
        let mut prefix = GeneratedRepositoryFileEffectPlan::builder();
        prefix
            .push("before.txt".into(), Arc::from(content), false)
            .unwrap();
        for _ in 0..index {
            let archive = archive([7; 32]);
            prefix
                .push_archive(
                    Arc::from(archive.urls()),
                    archive.sha256(),
                    archive.strip_prefix().into(),
                )
                .unwrap();
        }
        RepositoryArchiveRequest::new(
            RepositoryArchiveCallId::new(owner.clone(), ordinal, index),
            "unrelated+repo".into(),
            archive([7; 32]),
            prefix.finish(),
        )
    }

    fn record(request: RepositoryArchiveRequest) -> RepositoryArchiveRecord {
        RepositoryArchiveRecord::new(
            Arc::new(request),
            RepositoryArchiveResult::verified([7; 32]),
        )
    }

    #[test]
    fn transcript_rejects_reordered_conflicting_and_changed_prefix_calls() {
        let owner = Arc::new(HostSelectedExtensionOwner::testing("unrelated"));
        let first = request(&owner, 3, 0, b"before");
        let second = request(&owner, 3, 1, b"before");
        let empty = RepositoryArchiveTranscript::empty();
        assert!(matches!(
            empty.append(record(second.clone())),
            Err(RepositoryArchiveTranscriptError::NoncontiguousCall { expected: 0, .. })
        ));
        let one = empty.append(record(first.clone())).unwrap();
        assert_eq!(
            one.matching_record(&first).unwrap().unwrap().request(),
            &first
        );
        assert!(matches!(
            one.append(record(first.clone())),
            Err(RepositoryArchiveTranscriptError::DuplicateCall(_))
        ));
        let changed = request(&owner, 3, 0, b"changed");
        assert!(matches!(
            one.matching_record(&changed),
            Err(RepositoryArchiveTranscriptError::ConflictingRequest(_))
        ));
        assert!(matches!(
            one.append(record(changed)),
            Err(RepositoryArchiveTranscriptError::ConflictingRequest(_))
        ));
        assert!(matches!(
            one.append(record(request(&owner, 3, 1, b"changed"))),
            Err(RepositoryArchiveTranscriptError::InvalidPrefix(_))
        ));
        let mut changed_repository = second.clone();
        changed_repository.generated_repository = "different+repo".into();
        assert!(matches!(
            one.append(record(changed_repository)),
            Err(RepositoryArchiveTranscriptError::InvalidPrefix(_))
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
            two.find(&RepositoryArchiveCallId::new(owner, 4, 0))
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
        ordered.archive = effect.clone();
        let mut reversed = GeneratedRepositoryFileEffectPlan::builder();
        reversed
            .push_archive(
                Arc::from([effect.urls()[1].clone(), effect.urls()[0].clone()]),
                effect.sha256(),
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
        reordered.archive = effect.clone();
        assert_ne!(ordered, reordered);
        let transcript = RepositoryArchiveTranscript::empty()
            .append(record(ordered))
            .unwrap();
        assert!(matches!(
            transcript.matching_record(&reordered),
            Err(RepositoryArchiveTranscriptError::ConflictingRequest(_))
        ));
        let mut changed = original.clone();
        changed.archive = archive([9; 32]);
        assert_ne!(original, changed);
        let bad = RepositoryArchiveRecord::new(
            Arc::new(original),
            RepositoryArchiveResult::verified([9; 32]),
        );
        assert!(matches!(
            RepositoryArchiveTranscript::empty().append(bad),
            Err(RepositoryArchiveTranscriptError::ChecksumMismatch(_))
        ));
        let result = RepositoryArchiveResult::verified([7; 32]);
        assert_eq!(
            result.sha256_hex(),
            generated_repository_archive_sha256_hex([7; 32])
        );
        assert_eq!(
            result.sha256_sri(),
            generated_repository_archive_sha256_sri([7; 32])
        );
    }

    #[test]
    fn transcript_call_cap_is_shared_across_invocations() {
        let owner = Arc::new(HostSelectedExtensionOwner::testing("owner"));
        let mut transcript = RepositoryArchiveTranscript::empty();
        for ordinal in 0..REPOSITORY_ARCHIVE_MAX_CALLS {
            transcript = transcript
                .append(record(request(&owner, ordinal, 0, b"")))
                .unwrap();
        }
        assert!(matches!(
            transcript.append(record(request(
                &owner,
                REPOSITORY_ARCHIVE_MAX_CALLS,
                0,
                b""
            ))),
            Err(RepositoryArchiveTranscriptError::TooManyCalls)
        ));
    }

    #[test]
    fn terminal_realization_binds_complete_plan_invocation_and_lease() {
        let owner = Arc::new(HostSelectedExtensionOwner::testing("owner"));
        let transcript = RepositoryArchiveTranscript::empty()
            .append(record(request(&owner, 0, 0, b"before")))
            .unwrap();
        let plan = request(&owner, 0, 1, b"before").prefix().clone();
        let realization = RepositoryArchiveRealization::new(
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
                effect.sha256(),
                effect.strip_prefix().into(),
            )
            .unwrap();
        suffix
            .push("after.txt".into(), Arc::from(&b"after"[..]), true)
            .unwrap();
        let with_suffix = RepositoryArchiveRealization::new(
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
            RepositoryArchiveRealization::new(
                2,
                owner.clone(),
                0,
                transcript.clone(),
                plan.clone()
            )
            .unwrap()
        );
        assert!(matches!(
            RepositoryArchiveRealization::new(
                1,
                owner.clone(),
                1,
                transcript.clone(),
                plan.clone()
            ),
            Err(RepositoryArchiveRealizationError::ForeignInvocation)
        ));
        assert!(matches!(
            RepositoryArchiveRealization::new(
                1,
                owner.clone(),
                0,
                transcript.clone(),
                request(&owner, 0, 1, b"changed").prefix().clone()
            ),
            Err(RepositoryArchiveRealizationError::InvalidTerminalPlan)
        ));
        assert!(matches!(
            RepositoryArchiveRealization::new(
                1,
                owner,
                0,
                RepositoryArchiveTranscript::empty(),
                plan
            ),
            Err(RepositoryArchiveRealizationError::EmptyTranscript)
        ));
    }

    #[tokio::test]
    async fn archive_epoch_tracks_nonce_transcript_and_revocation_in_dice() {
        let workspace = NormalizedAbsolutePath::new(std::path::Path::new("/archive-test")).unwrap();
        let injected = RepositoryArchiveEpochKey::new(workspace.clone());
        let observed = RepositoryArchiveObservationKey::new(workspace);
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let owner = Arc::new(HostSelectedExtensionOwner::testing("owner"));
        let transcript = RepositoryArchiveTranscript::empty()
            .append(record(request(&owner, 0, 0, b"")))
            .unwrap();
        let original = RepositoryArchiveEpoch::active(1, RepositoryArchiveTranscript::empty());
        for epoch in [
            original.clone(),
            RepositoryArchiveEpoch::active(1, transcript),
            RepositoryArchiveEpoch::active(2, RepositoryArchiveTranscript::empty()),
            original,
            RepositoryArchiveEpoch::revoked(),
        ] {
            let mut updater = dice.updater();
            updater
                .changed_to(vec![(injected.clone(), epoch.clone())])
                .unwrap();
            let actual = updater.commit().await.compute(&observed).await.unwrap();
            assert_eq!(actual, epoch);
            assert_eq!(
                RepositoryArchiveObservationKey::validity(&actual),
                actual.lease_nonce().is_some()
            );
        }
        assert!(RepositoryArchiveEpoch::revoked().transcript().is_none());
    }
}
