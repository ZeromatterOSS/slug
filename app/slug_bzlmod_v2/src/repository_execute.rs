/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select either.
 */

//! Immutable, request-scoped observations for selected repository execution.
//! Loading owns the call sequence; Core owns epoch injection and child launch.

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

use crate::GeneratedRepositoryFileEffectPlan;
use crate::HostRepositoryLabelPathValue;
use crate::HostSelectedExtensionOwner;
use crate::RepositoryEnvironmentSnapshot;

/// A Slug-native bound on repeated selected repository probes in one command.
pub const REPOSITORY_EXECUTE_MAX_CALLS: usize = 128;

/// The first argument retains `which` provenance instead of reducing it to a
/// string. Other admitted arguments are Starlark strings, including a generated
/// path after `str(repository_ctx.path(...))`.
#[doc(hidden)]
#[derive(Clone, PartialEq, Eq, Hash, Allocative)]
pub enum RepositoryExecuteArg {
    Which(HostRepositoryLabelPathValue),
    String(CompactString),
}

impl fmt::Debug for RepositoryExecuteArg {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Which(_) => formatter.write_str("Which(<redacted>)"),
            Self::String(_) => formatter.write_str("String(<redacted>)"),
        }
    }
}

/// The sole admitted local execute option and working-directory combination.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Allocative)]
pub enum RepositoryExecuteOptions {
    DefaultLocalRepositoryCwd,
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct RepositoryExecuteCallId {
    owner: Arc<HostSelectedExtensionOwner>,
    ordinal: usize,
    call_index: usize,
}

impl RepositoryExecuteCallId {
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

/// The complete descriptor checked on each invocation replay. The full raw
/// request environment is structural DICE state, not the child environment.
#[doc(hidden)]
#[derive(Clone, PartialEq, Eq, Allocative)]
pub struct RepositoryExecuteRequest {
    id: RepositoryExecuteCallId,
    generated_repository: CompactString,
    argv: Arc<[RepositoryExecuteArg]>,
    environment: RepositoryEnvironmentSnapshot,
    options: RepositoryExecuteOptions,
    prefix: GeneratedRepositoryFileEffectPlan,
}

impl RepositoryExecuteRequest {
    pub fn new(
        id: RepositoryExecuteCallId,
        generated_repository: CompactString,
        argv: Arc<[RepositoryExecuteArg]>,
        environment: RepositoryEnvironmentSnapshot,
        prefix: GeneratedRepositoryFileEffectPlan,
    ) -> Self {
        Self {
            id,
            generated_repository,
            argv,
            environment,
            options: RepositoryExecuteOptions::DefaultLocalRepositoryCwd,
            prefix,
        }
    }

    pub fn id(&self) -> &RepositoryExecuteCallId {
        &self.id
    }

    pub fn generated_repository(&self) -> &str {
        &self.generated_repository
    }

    pub fn argv(&self) -> &[RepositoryExecuteArg] {
        &self.argv
    }

    pub fn environment(&self) -> &RepositoryEnvironmentSnapshot {
        &self.environment
    }

    pub fn options(&self) -> RepositoryExecuteOptions {
        self.options
    }

    pub fn prefix(&self) -> &GeneratedRepositoryFileEffectPlan {
        &self.prefix
    }
}

impl fmt::Debug for RepositoryExecuteRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RepositoryExecuteRequest")
            .field("id", &self.id)
            .field("generated_repository", &self.generated_repository)
            .field("argv_count", &self.argv.len())
            .field("environment_entry_count", &self.environment.len())
            .field("options", &self.options)
            .field(
                "prefix_effect_count",
                &self.prefix.ordered_effects().count(),
            )
            .finish()
    }
}

/// Raw captured bytes are retained exactly and decoded as Latin-1 by Loading.
#[doc(hidden)]
#[derive(Clone, PartialEq, Eq, Allocative)]
pub struct RepositoryExecuteResult {
    return_code: i32,
    stdout: Arc<[u8]>,
    stderr: Arc<[u8]>,
}

impl RepositoryExecuteResult {
    pub fn new(return_code: i32, stdout: Arc<[u8]>, stderr: Arc<[u8]>) -> Self {
        Self {
            return_code,
            stdout,
            stderr,
        }
    }

    pub fn return_code(&self) -> i32 {
        self.return_code
    }

    pub fn stdout(&self) -> &[u8] {
        &self.stdout
    }

    pub fn stderr(&self) -> &[u8] {
        &self.stderr
    }
}

impl fmt::Debug for RepositoryExecuteResult {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RepositoryExecuteResult")
            .field("return_code", &self.return_code)
            .field("stdout_bytes", &self.stdout.len())
            .field("stderr_bytes", &self.stderr.len())
            .finish()
    }
}

#[doc(hidden)]
#[derive(Clone, PartialEq, Eq, Allocative)]
pub struct RepositoryExecuteRecord {
    request: Arc<RepositoryExecuteRequest>,
    result: RepositoryExecuteResult,
}

impl RepositoryExecuteRecord {
    pub fn new(request: Arc<RepositoryExecuteRequest>, result: RepositoryExecuteResult) -> Self {
        Self { request, result }
    }

    pub fn request(&self) -> &RepositoryExecuteRequest {
        &self.request
    }

    pub fn result(&self) -> &RepositoryExecuteResult {
        &self.result
    }
}

impl fmt::Debug for RepositoryExecuteRecord {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RepositoryExecuteRecord")
            .field("request", &self.request)
            .field("result", &self.result)
            .finish()
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum RepositoryExecuteTranscriptError {
    DuplicateCall(RepositoryExecuteCallId),
    TooManyCalls,
}

/// One command lease's ordered calls. Lookup is linear because the admitted
/// repository probe family is small; there is no second retained index/cache.
#[doc(hidden)]
#[derive(Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct RepositoryExecuteTranscript {
    records: Arc<[RepositoryExecuteRecord]>,
}

impl RepositoryExecuteTranscript {
    pub fn empty() -> Self {
        Self {
            records: Arc::from([]),
        }
    }

    pub fn records(&self) -> &[RepositoryExecuteRecord] {
        &self.records
    }

    pub fn find(&self, id: &RepositoryExecuteCallId) -> Option<&RepositoryExecuteRecord> {
        self.records.iter().find(|record| record.request.id() == id)
    }

    pub fn records_for<'a>(
        &'a self,
        owner: &'a HostSelectedExtensionOwner,
        ordinal: usize,
    ) -> impl Iterator<Item = &'a RepositoryExecuteRecord> {
        self.records.iter().filter(move |record| {
            record.request.id().owner().as_ref() == owner
                && record.request.id().ordinal() == ordinal
        })
    }

    /// Project the command transcript to one selected repository invocation.
    /// Result byte buffers remain shared with the command-scoped records.
    pub fn for_owner(&self, owner: &HostSelectedExtensionOwner, ordinal: usize) -> Self {
        Self {
            records: self.records_for(owner, ordinal).cloned().collect(),
        }
    }

    pub fn append(
        &self,
        record: RepositoryExecuteRecord,
    ) -> Result<Self, RepositoryExecuteTranscriptError> {
        if self.records.len() >= REPOSITORY_EXECUTE_MAX_CALLS {
            return Err(RepositoryExecuteTranscriptError::TooManyCalls);
        }
        if self.find(record.request.id()).is_some() {
            return Err(RepositoryExecuteTranscriptError::DuplicateCall(
                record.request.id().clone(),
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

impl Default for RepositoryExecuteTranscript {
    fn default() -> Self {
        Self::empty()
    }
}

impl fmt::Debug for RepositoryExecuteTranscript {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RepositoryExecuteTranscript")
            .field("records", &self.records)
            .finish()
    }
}

/// An epoch is injected on every attempt. Revocation prevents an old lease
/// from authorizing a later request, while the nonce prevents equal inputs
/// from reusing a prior process result.
#[doc(hidden)]
#[derive(Clone, PartialEq, Eq, Allocative, Dupe)]
pub enum RepositoryExecuteEpoch {
    Revoked,
    Active {
        lease_nonce: u64,
        transcript: RepositoryExecuteTranscript,
    },
}

impl RepositoryExecuteEpoch {
    pub fn revoked() -> Self {
        Self::Revoked
    }

    pub fn active(lease_nonce: u64, transcript: RepositoryExecuteTranscript) -> Self {
        Self::Active {
            lease_nonce,
            transcript,
        }
    }

    pub fn transcript(&self) -> Option<&RepositoryExecuteTranscript> {
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

impl fmt::Debug for RepositoryExecuteEpoch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Revoked => formatter.write_str("RepositoryExecuteEpoch::Revoked"),
            Self::Active {
                lease_nonce,
                transcript,
            } => formatter
                .debug_struct("RepositoryExecuteEpoch::Active")
                .field("lease_nonce", lease_nonce)
                .field("transcript", transcript)
                .finish(),
        }
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct RepositoryExecuteEpochKey {
    workspace: NormalizedAbsolutePath,
}

impl RepositoryExecuteEpochKey {
    pub fn new(workspace: NormalizedAbsolutePath) -> Self {
        Self { workspace }
    }

    pub fn workspace(&self) -> &NormalizedAbsolutePath {
        &self.workspace
    }
}

impl fmt::Display for RepositoryExecuteEpochKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "repository-execute-epoch:{}", self.workspace)
    }
}

impl InjectedKey for RepositoryExecuteEpochKey {
    type Value = RepositoryExecuteEpoch;

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

/// Only repository computations that actually call execute consume this key.
/// An active epoch is retained so command selection can inspect its complete
/// DICE closure. Its lease nonce and transcript are injected on every attempt;
/// revocation invalidates it after the command, preventing cross-request reuse.
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct RepositoryExecuteObservationKey {
    workspace: NormalizedAbsolutePath,
}

impl RepositoryExecuteObservationKey {
    pub fn new(workspace: NormalizedAbsolutePath) -> Self {
        Self { workspace }
    }
}

impl fmt::Display for RepositoryExecuteObservationKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "observed-repository-execute:{}", self.workspace)
    }
}

#[async_trait]
impl Key for RepositoryExecuteObservationKey {
    type Value = RepositoryExecuteEpoch;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        ctx.compute(&RepositoryExecuteEpochKey::new(self.workspace.clone()))
            .await
            .expect("Core injects a repository execution epoch before every command attempt")
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }

    fn validity(value: &Self::Value) -> bool {
        matches!(value, RepositoryExecuteEpoch::Active { .. })
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use slug_workspace_v2::PathObservationNamespace;

    use super::*;
    use crate::RepositoryEnvironmentEntry;
    use crate::source_preparation::SourcePreparationNeeds;

    fn request(
        owner: Arc<HostSelectedExtensionOwner>,
        call_index: usize,
        source: &'static [u8],
        environment: &str,
    ) -> RepositoryExecuteRequest {
        let gcc = HostRepositoryLabelPathValue::new(
            NormalizedAbsolutePath::new(Path::new("/usr/bin/gcc")).unwrap(),
            PathObservationNamespace::Host,
        )
        .unwrap();
        let mut prefix = GeneratedRepositoryFileEffectPlan::builder();
        prefix
            .push("tools/cpp/empty.cc".into(), Arc::from(source), false)
            .unwrap();
        RepositoryExecuteRequest::new(
            RepositoryExecuteCallId::new(owner, 2, call_index),
            "owner+repo".into(),
            Arc::from([
                RepositoryExecuteArg::Which(gcc),
                RepositoryExecuteArg::String("-v".into()),
            ]),
            RepositoryEnvironmentSnapshot::from_canonical(Arc::from([
                RepositoryEnvironmentEntry::new("SECRET", environment),
            ]))
            .unwrap(),
            prefix.finish(),
        )
    }

    #[test]
    fn result_debug_redacts_captured_bytes() {
        let result = RepositoryExecuteResult::new(
            1,
            Arc::from(&b"secret-stdout"[..]),
            Arc::from(&b"secret-stderr"[..]),
        );
        let debug = format!("{result:?}");
        assert!(!debug.contains("secret-stdout"));
        assert!(!debug.contains("secret-stderr"));
        assert_eq!(result.return_code(), 1);
        assert_eq!(result.stdout(), b"secret-stdout");
        assert_eq!(result.stderr(), b"secret-stderr");
    }

    #[test]
    fn descriptor_and_transcript_preserve_order_prefix_and_environment() {
        let owner = Arc::new(HostSelectedExtensionOwner::testing("owner"));
        let first = request(owner.clone(), 0, b"int main() {}", "secret-one");
        let second = request(owner.clone(), 1, b"int main() {}", "secret-one");
        assert_eq!(
            first.options(),
            RepositoryExecuteOptions::DefaultLocalRepositoryCwd
        );
        assert_ne!(first, second);
        assert_ne!(first, request(owner.clone(), 0, b"different", "secret-one"));
        assert_ne!(
            first,
            request(owner.clone(), 0, b"int main() {}", "secret-two")
        );

        let first_record = RepositoryExecuteRecord::new(
            Arc::new(first.clone()),
            RepositoryExecuteResult::new(0, Arc::from(&b""[..]), Arc::from(&b"first"[..])),
        );
        let second_record = RepositoryExecuteRecord::new(
            Arc::new(second.clone()),
            RepositoryExecuteResult::new(1, Arc::from(&b""[..]), Arc::from(&b"second"[..])),
        );
        let one = RepositoryExecuteTranscript::empty()
            .append(first_record.clone())
            .unwrap();
        let two = one.append(second_record.clone()).unwrap();
        assert_eq!(two.records().len(), 2);
        assert_eq!(two.find(first.id()).unwrap().result().stderr(), b"first");
        assert_eq!(two.find(second.id()).unwrap().result().stderr(), b"second");
        assert_eq!(two.records_for(owner.as_ref(), 2).count(), 2);
        let foreign_owner = Arc::new(HostSelectedExtensionOwner::testing("other"));
        let foreign = request(foreign_owner, 0, b"int main() {}", "secret-one");
        let with_foreign = two
            .append(RepositoryExecuteRecord::new(
                Arc::new(foreign),
                RepositoryExecuteResult::new(0, Arc::from(&b""[..]), Arc::from(&b"foreign"[..])),
            ))
            .unwrap();
        assert_eq!(with_foreign.for_owner(owner.as_ref(), 2), two);
        assert_eq!(with_foreign.records_for(owner.as_ref(), 2).count(), 2);
        assert!(
            matches!(two.append(first_record), Err(RepositoryExecuteTranscriptError::DuplicateCall(id)) if id == *first.id())
        );
        let reversed = RepositoryExecuteTranscript::empty()
            .append(second_record)
            .unwrap()
            .append(two.records()[0].clone())
            .unwrap();
        assert_ne!(two, reversed);
        let debug = format!("{:?}", RepositoryExecuteEpoch::active(7, two));
        assert!(!debug.contains("secret-one"));
        assert!(!debug.contains("first"));
    }

    #[test]
    fn execute_need_union_rejects_conflicting_descriptor_and_epoch_is_lease_scoped() {
        let owner = Arc::new(HostSelectedExtensionOwner::testing("owner"));
        let original = request(owner.clone(), 0, b"int main() {}", "secret-one");
        let equal = original.clone();
        let changed = request(owner, 0, b"int main() {}", "secret-two");
        let need = SourcePreparationNeeds::repository_execute(original);
        assert_eq!(
            need.try_union(&SourcePreparationNeeds::repository_execute(equal))
                .unwrap()
                .repository_executes()
                .len(),
            1
        );
        assert!(matches!(
            need.try_union(&SourcePreparationNeeds::repository_execute(changed)),
            Err(crate::source_preparation::SourcePreparationNeedsError::ConflictingRepositoryExecute { .. })
        ));
        let active = RepositoryExecuteEpoch::active(1, RepositoryExecuteTranscript::empty());
        let changed_nonce = RepositoryExecuteEpoch::active(2, RepositoryExecuteTranscript::empty());
        assert!(!<RepositoryExecuteEpochKey as InjectedKey>::equality(
            &active,
            &changed_nonce
        ));
        let recorded = RepositoryExecuteEpoch::active(
            1,
            RepositoryExecuteTranscript::empty()
                .append(RepositoryExecuteRecord::new(
                    need.repository_executes().values().next().unwrap().clone(),
                    RepositoryExecuteResult::new(0, Arc::from(&b""[..]), Arc::from(&b""[..])),
                ))
                .unwrap(),
        );
        assert!(!<RepositoryExecuteEpochKey as InjectedKey>::equality(
            &active, &recorded
        ));
        assert!(RepositoryExecuteObservationKey::validity(&active));
        assert!(!RepositoryExecuteObservationKey::validity(
            &RepositoryExecuteEpoch::revoked()
        ));
        assert!(RepositoryExecuteEpoch::revoked().transcript().is_none());
    }

    #[test]
    fn transcript_call_cap_fails_closed() {
        let owner = Arc::new(HostSelectedExtensionOwner::testing("owner"));
        let mut transcript = RepositoryExecuteTranscript::empty();
        for index in 0..REPOSITORY_EXECUTE_MAX_CALLS {
            transcript = transcript
                .append(RepositoryExecuteRecord::new(
                    Arc::new(request(owner.clone(), index, b"int main() {}", "")),
                    RepositoryExecuteResult::new(0, Arc::from(&b""[..]), Arc::from(&b""[..])),
                ))
                .unwrap();
        }
        assert_eq!(transcript.records().len(), REPOSITORY_EXECUTE_MAX_CALLS);
        let overflow = RepositoryExecuteRecord::new(
            Arc::new(request(
                owner,
                REPOSITORY_EXECUTE_MAX_CALLS,
                b"int main() {}",
                "",
            )),
            RepositoryExecuteResult::new(0, Arc::from(&b""[..]), Arc::from(&b""[..])),
        );
        assert!(matches!(
            transcript.append(overflow),
            Err(RepositoryExecuteTranscriptError::TooManyCalls)
        ));
    }
}
