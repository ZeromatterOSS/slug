//! Exact, cancellable comparison of foreign immutable workspace observations.
//! Pointer pairs only route a phase-local memo; a miss compares every byte.
use std::collections::HashMap;

use slug_bzlmod_v2::RepositoryMaterializationRequest;
use slug_bzlmod_v2::RepositorySourceSnapshotState;
use slug_bzlmod_v2::RepositoryWorkspaceNode;
use slug_bzlmod_v2::RepositoryWorkspaceReadViewStateRef;
use slug_bzlmod_v2::RepositoryWorkspaceTranscript;
use starlark_map::StarlarkHasherBuilder;

use super::*;

const MAX_PAIRS: usize = 131_072;
type StateKey = (u8, usize, usize);
enum ComparedState {
    Workspace {
        _state: Arc<RepositoryWorkspaceState>,
    },
    Source {
        _state: Arc<RepositorySourceSnapshotState>,
    },
}
type BodyKey = (
    RepositoryWorkspaceBodyAllocation,
    RepositoryWorkspaceBodyAllocation,
);
type StateValue = (ComparedState, ComparedState, bool);
type BodyValue = (RepositoryWorkspaceBody, RepositoryWorkspaceBody, bool);
type Memo<K, V> = HashMap<K, V, StarlarkHasherBuilder>;

struct NativeComparisonCoverage {
    _bodies: NativeBodyCandidates,
    _dependencies: Arc<NativeReadViewDependencies>,
    _credit: ResourceCredit,
}

pub(in crate::runtime) struct ViewComparison<'a> {
    states: Memo<StateKey, StateValue>,
    bodies: Memo<BodyKey, BodyValue>,
    state_capacity: usize,
    body_capacity: usize,
    scope: &'a SourceServiceScope,
    active: &'a dyn Fn() -> bool,
    #[cfg(test)]
    compared_bytes: usize,
    // Memo payloads above retire before incoming graph/control coverage. The
    // caller can retain this memo after a frontier-validation call returns.
    coverage: Vec<NativeComparisonCoverage>,
    credit: ResourceCredit,
}
impl<'a> ViewComparison<'a> {
    pub(in crate::runtime) fn new(
        resources: &Arc<RepositoryRootResources>,
        scope: &'a SourceServiceScope,
        active: &'a dyn Fn() -> bool,
    ) -> Result<Self, PhysicalRootError> {
        check(scope, active)?;
        // Operation/Read-result equality may decode without calling body_equal.
        // Pay the complete semantic-comparison workspace at the outer entry.
        let credit = resources.reserve(
            add(
                std::mem::size_of::<Self>() + 256,
                2 * RepositoryWorkspaceBody::decode_scratch_bytes(),
            )?,
            0,
            0,
        )?;
        Ok(Self {
            states: Memo::default(),
            bodies: Memo::default(),
            state_capacity: 0,
            body_capacity: 0,
            scope,
            active,
            #[cfg(test)]
            compared_bytes: 0,
            coverage: Vec::new(),
            credit,
        })
    }
    pub(super) fn prepare_workspace_coverage(&mut self) -> Result<(), PhysicalRootError> {
        self.check()?;
        if self.coverage.len() == self.coverage.capacity() {
            let next = self
                .coverage
                .capacity()
                .checked_mul(2)
                .ok_or(PhysicalRootError::Resource)?
                .max(4);
            let bytes = next
                .checked_mul(std::mem::size_of::<NativeComparisonCoverage>())
                .ok_or(PhysicalRootError::Resource)?;
            self.credit
                .bytes
                .try_grow(bytes)
                .map_err(|_| PhysicalRootError::Resource)?;
            self.coverage
                .try_reserve_exact(next - self.coverage.len())
                .map_err(|_| PhysicalRootError::Resource)?;
            if self.coverage.capacity() > next {
                return Err(PhysicalRootError::Resource);
            }
        }
        Ok(())
    }
    /// Called only after prepare, before any memo retains the incoming graph.
    /// This handoff cannot fail and retire guards ahead of caller-held payloads.
    pub(super) fn hold_workspace_coverage(
        &mut self,
        bodies: NativeBodyCandidates,
        dependencies: Arc<NativeReadViewDependencies>,
        credit: ResourceCredit,
    ) {
        assert!(self.coverage.len() < self.coverage.capacity());
        self.coverage.push(NativeComparisonCoverage {
            _bodies: bodies,
            _dependencies: dependencies,
            _credit: credit,
        });
    }
    fn check(&self) -> Result<(), PhysicalRootError> {
        check(self.scope, self.active)
    }
    fn allocation_bound(
        state_capacity: usize,
        body_capacity: usize,
    ) -> Result<usize, PhysicalRootError> {
        let states = state_capacity
            .checked_mul(4 * (std::mem::size_of::<(StateKey, StateValue)>() + 1))
            .ok_or(PhysicalRootError::Resource)?;
        let bodies = body_capacity
            .checked_mul(4 * (std::mem::size_of::<(BodyKey, BodyValue)>() + 1))
            .ok_or(PhysicalRootError::Resource)?;
        add(add(std::mem::size_of::<Self>() + 256, states)?, bodies)
    }
    fn state_slot(&mut self) -> Result<(), PhysicalRootError> {
        if self.states.len() >= MAX_PAIRS {
            return Err(PhysicalRootError::Resource);
        }
        if self.states.len() == self.states.capacity() {
            let next = self
                .state_capacity
                .checked_mul(2)
                .ok_or(PhysicalRootError::Resource)?
                .max(16)
                .min(MAX_PAIRS);
            self.credit
                .bytes
                .try_grow(Self::allocation_bound(next, self.body_capacity)?)
                .map_err(|_| PhysicalRootError::Resource)?;
            self.states
                .try_reserve(next.saturating_sub(self.states.len()))
                .map_err(|_| PhysicalRootError::Resource)?;
            if self.states.capacity() > next.checked_mul(4).ok_or(PhysicalRootError::Resource)? {
                return Err(PhysicalRootError::Resource);
            }
            self.state_capacity = next;
        }
        Ok(())
    }
    fn body_slot(&mut self) -> Result<(), PhysicalRootError> {
        if self.bodies.len() >= MAX_PAIRS {
            return Err(PhysicalRootError::Resource);
        }
        if self.bodies.len() == self.bodies.capacity() {
            let next = self
                .body_capacity
                .checked_mul(2)
                .ok_or(PhysicalRootError::Resource)?
                .max(16)
                .min(MAX_PAIRS);
            self.credit
                .bytes
                .try_grow(Self::allocation_bound(self.state_capacity, next)?)
                .map_err(|_| PhysicalRootError::Resource)?;
            self.bodies
                .try_reserve(next.saturating_sub(self.bodies.len()))
                .map_err(|_| PhysicalRootError::Resource)?;
            if self.bodies.capacity() > next.checked_mul(4).ok_or(PhysicalRootError::Resource)? {
                return Err(PhysicalRootError::Resource);
            }
            self.body_capacity = next;
        }
        Ok(())
    }
    fn body_equal(
        &mut self,
        left: &RepositoryWorkspaceBody,
        right: &RepositoryWorkspaceBody,
    ) -> Result<bool, PhysicalRootError> {
        self.check()?;
        if left.ptr_eq(right) {
            return Ok(true);
        }
        let key = (left.allocation_identity(), right.allocation_identity());
        if let Some((_, _, equal)) = self.bodies.get(&key) {
            return Ok(*equal);
        }
        self.body_slot()?;
        let mut equal = left.len() == right.len();
        if equal {
            let mut offset = 0;
            let compared = left.try_for_each_chunk(&mut |a| {
                self.check().map_err(Some)?;
                #[cfg(test)]
                {
                    self.compared_bytes += a.len();
                }
                if !right.range_eq(offset, a) {
                    return Err(None);
                }
                offset += a.len();
                Ok(())
            });
            match compared {
                Ok(()) => {}
                Err(None) => equal = false,
                Err(Some(error)) => return Err(error),
            }
        }
        self.check()?;
        self.bodies
            .insert(key, (left.clone(), right.clone(), equal));
        Ok(equal)
    }
    pub(in crate::runtime) fn state_equal(
        &mut self,
        left: &Arc<RepositoryWorkspaceState>,
        right: &Arc<RepositoryWorkspaceState>,
    ) -> Result<bool, PhysicalRootError> {
        self.check()?;
        if Arc::ptr_eq(left, right) {
            return Ok(true);
        }
        let key = (0, Arc::as_ptr(left) as usize, Arc::as_ptr(right) as usize);
        if let Some((_, _, equal)) = self.states.get(&key) {
            return Ok(*equal);
        }
        self.state_slot()?;
        let mut equal = left.reservation() == right.reservation()
            && left.mode() == right.mode()
            && left.source_bindings().len() == right.source_bindings().len()
            && left.entries().len() == right.entries().len();
        if equal {
            for (a, b) in left.source_bindings().iter().zip(right.source_bindings()) {
                self.check()?;
                equal = match (a.source_state(), b.source_state()) {
                    (Some(a), Some(b)) => self.source_state_equal(a, b)?,
                    _ => false,
                };
                if !equal {
                    break;
                }
            }
        }
        if equal {
            for (a, b) in left.entries().iter().zip(right.entries()) {
                self.check()?;
                if a.path() != b.path() || a.mode() != b.mode() {
                    equal = false;
                    break;
                }
                equal = match (a.node(), b.node()) {
                    (RepositoryWorkspaceNode::Directory, RepositoryWorkspaceNode::Directory) => {
                        true
                    }
                    (RepositoryWorkspaceNode::File(a), RepositoryWorkspaceNode::File(b)) => {
                        self.body_equal(a, b)?
                    }
                    (RepositoryWorkspaceNode::Link(a), RepositoryWorkspaceNode::Link(b)) => a == b,
                    _ => false,
                };
                if !equal {
                    break;
                }
            }
        }
        self.check()?;
        self.states.insert(
            key,
            (
                ComparedState::Workspace {
                    _state: left.clone(),
                },
                ComparedState::Workspace {
                    _state: right.clone(),
                },
                equal,
            ),
        );
        Ok(equal)
    }
    /// Source files already use the explicit SHA256/size content-identity
    /// domain. Every tree field and full source reservation still compares.
    pub(in crate::runtime) fn source_state_equal(
        &mut self,
        left: &Arc<RepositorySourceSnapshotState>,
        right: &Arc<RepositorySourceSnapshotState>,
    ) -> Result<bool, PhysicalRootError> {
        self.check()?;
        if Arc::ptr_eq(left, right) {
            return Ok(true);
        }
        let key = (1, Arc::as_ptr(left) as usize, Arc::as_ptr(right) as usize);
        if let Some((_, _, equal)) = self.states.get(&key) {
            return Ok(*equal);
        }
        self.state_slot()?;
        let mut equal = left.reservation() == right.reservation()
            && left.entries().len() == right.entries().len();
        if equal {
            for (a, b) in left.entries().iter().zip(right.entries()) {
                self.check()?;
                if a != b {
                    equal = false;
                    break;
                }
            }
        }
        self.check()?;
        self.states.insert(
            key,
            (
                ComparedState::Source {
                    _state: left.clone(),
                },
                ComparedState::Source {
                    _state: right.clone(),
                },
                equal,
            ),
        );
        Ok(equal)
    }
    pub(in crate::runtime) fn state_ref_equal(
        &mut self,
        left: RepositoryWorkspaceReadViewStateRef<'_>,
        right: RepositoryWorkspaceReadViewStateRef<'_>,
    ) -> Result<bool, PhysicalRootError> {
        self.check()?;
        match (left, right) {
            (
                RepositoryWorkspaceReadViewStateRef::Workspace(a),
                RepositoryWorkspaceReadViewStateRef::Workspace(b),
            ) => self.state_equal(a, b),
            (
                RepositoryWorkspaceReadViewStateRef::Source(a),
                RepositoryWorkspaceReadViewStateRef::Source(b),
            ) => self.source_state_equal(a, b),
            _ => Ok(false),
        }
    }
    pub(in crate::runtime) fn request_equal(
        &mut self,
        left: &RepositoryMaterializationRequest,
        right: &RepositoryMaterializationRequest,
    ) -> Result<bool, PhysicalRootError> {
        self.check()?;
        let equal = left.try_eq_with_states(right, &mut |a, b| self.state_ref_equal(a, b))?;
        self.check()?;
        Ok(equal)
    }
    pub(in crate::runtime) fn epoch_equal(
        &mut self,
        left: &slug_bzlmod_v2::RepositoryMaterializationResultEpoch,
        right: &slug_bzlmod_v2::RepositoryMaterializationResultEpoch,
    ) -> Result<bool, PhysicalRootError> {
        self.check()?;
        let equal = left.try_eq_with_states(right, &mut |a, b| self.state_ref_equal(a, b))?;
        self.check()?;
        Ok(equal)
    }
    pub(super) fn transcript_equal(
        &mut self,
        left: &RepositoryWorkspaceTranscript,
        right: &RepositoryWorkspaceTranscript,
    ) -> Result<bool, PhysicalRootError> {
        self.check()?;
        let equal = left.try_eq_with_states(right, &mut |a, b| self.state_ref_equal(a, b))?;
        self.check()?;
        Ok(equal)
    }
}

#[cfg(test)]
#[path = "repository_view_comparison/body_coverage_tests.rs"]
mod body_coverage_tests;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repeated_exact_body_pairs_are_compared_once_and_cancelled_hits_refuse() {
        let bank = super::super::tests::resources();
        let scope = super::super::tests::scope();
        let baseline = bank.snapshot().retained_bytes;
        let active = || true;
        {
            let mut comparison = ViewComparison::new(&bank, &scope, &active).unwrap();
            let a: RepositoryWorkspaceBody = vec![7; 128 * 1024].into();
            let b: RepositoryWorkspaceBody = vec![7; 128 * 1024].into();
            for _ in 0..128 {
                assert!(comparison.body_equal(&a, &b).unwrap());
            }
            assert_eq!(comparison.compared_bytes, a.len());
            assert_eq!(comparison.bodies.len(), 1);
            scope.cancel();
            assert!(matches!(
                comparison.body_equal(&a, &b),
                Err(PhysicalRootError::SourceService(
                    SourceServiceError::Cancelled
                ))
            ));
        }
        assert_eq!(bank.snapshot().retained_bytes, baseline);
    }
    #[test]
    fn body_comparison_checks_activity_between_chunks_and_refunds_failed_growth() {
        use std::cell::Cell;
        let bank = super::super::tests::resources();
        let scope = super::super::tests::scope();
        let baseline = bank.snapshot().retained_bytes;
        let polls = Cell::new(0usize);
        let active = || {
            let count = polls.get();
            polls.set(count + 1);
            count < 4
        };
        {
            let mut comparison = ViewComparison::new(&bank, &scope, &active).unwrap();
            let a: RepositoryWorkspaceBody = vec![7; 256 * 1024].into();
            let b: RepositoryWorkspaceBody = vec![7; 256 * 1024].into();
            assert!(matches!(
                comparison.body_equal(&a, &b),
                Err(PhysicalRootError::Inactive)
            ));
            assert!(comparison.compared_bytes < a.len());
            assert!(comparison.bodies.is_empty());
        }
        assert_eq!(bank.snapshot().retained_bytes, baseline);
        let working = 2 * RepositoryWorkspaceBody::decode_scratch_bytes();
        let tiny = RepositoryRootResources::new(
            RepositoryRootResourceLimits::new(working + 2048, 1, 1).unwrap(),
        )
        .unwrap();
        let initial = tiny.snapshot().retained_bytes;
        {
            let mut comparison = ViewComparison::new(&tiny, &scope, &|| true).unwrap();
            let admitted = tiny.snapshot().retained_bytes;
            assert_eq!(
                admitted,
                initial + working + std::mem::size_of::<ViewComparison<'_>>() + 256
            );
            let a: RepositoryWorkspaceBody = Arc::<[u8]>::from(b"a".as_slice()).into();
            let b: RepositoryWorkspaceBody = Arc::<[u8]>::from(b"a".as_slice()).into();
            assert!(matches!(
                comparison.body_equal(&a, &b),
                Err(PhysicalRootError::Resource)
            ));
            assert!(comparison.bodies.is_empty());
            assert_eq!(tiny.snapshot().retained_bytes, admitted);
            assert_eq!(tiny.snapshot().peak_retained_bytes, admitted);
        }
        assert_eq!(tiny.snapshot().retained_bytes, initial);
    }
    #[test]
    fn full_state_pairs_reuse_exact_body_proof_and_refuse_changed_metadata() {
        use slug_bzlmod_v2::RepositoryWorkspaceEntry;
        use slug_bzlmod_v2::RepositoryWorkspaceOperation;
        use slug_bzlmod_v2::RepositoryWorkspaceResult;
        let bank = super::super::tests::resources();
        let scope = super::super::tests::scope();
        let reservation = super::super::native_tests::reservation(
            std::path::Path::new("/workspace/compare"),
            3411,
        );
        let make = |mode| {
            let initial = RepositoryWorkspaceTranscript::new(reservation.clone(), 0o755).unwrap();
            let body: RepositoryWorkspaceBody = vec![8; 128 * 1024].into();
            let request = initial
                .request(RepositoryWorkspaceOperation::File {
                    path: "data".into(),
                    body: body.clone(),
                    mode,
                })
                .unwrap();
            initial
                .complete(
                    request,
                    RepositoryWorkspaceResult::Unit,
                    RepositoryWorkspaceMode::Archive,
                    vec![
                        RepositoryWorkspaceEntry::new(
                            "".into(),
                            0o755,
                            RepositoryWorkspaceNode::Directory,
                        )
                        .unwrap(),
                        RepositoryWorkspaceEntry::new(
                            "data".into(),
                            mode,
                            RepositoryWorkspaceNode::File(body),
                        )
                        .unwrap(),
                    ],
                )
                .unwrap()
                .current()
                .clone()
        };
        let a = make(0o644);
        let b = make(0o644);
        let changed = make(0o600);
        let baseline = bank.snapshot().retained_bytes;
        {
            let mut comparison = ViewComparison::new(&bank, &scope, &|| true).unwrap();
            for _ in 0..128 {
                assert!(comparison.state_equal(&a, &b).unwrap());
            }
            assert_eq!(comparison.states.len(), 1);
            assert_eq!(comparison.bodies.len(), 1);
            assert_eq!(comparison.compared_bytes, 128 * 1024);
            assert!(!comparison.state_equal(&a, &changed).unwrap());
            assert_eq!(comparison.compared_bytes, 128 * 1024);
        }
        assert_eq!(bank.snapshot().retained_bytes, baseline);
    }
}
