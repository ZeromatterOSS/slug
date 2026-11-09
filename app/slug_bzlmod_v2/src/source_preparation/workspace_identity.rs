//! Immutable structural spelling key and full borrowed request storage bounds.
//! Candidate bytes route native reservations; full equality remains authority.
use std::hash::Hash;
use std::hash::Hasher;

use sha2::Digest;
use sha2::Sha256;

use super::*;
use crate::HostSelectedExtensionOwner;
use crate::RepositoryWorkspaceError;
use crate::RepositoryWorkspaceRealization;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
pub struct RepositoryWorkspaceStableKey {
    workspace: NormalizedAbsolutePath,
    owner: Arc<HostSelectedExtensionOwner>,
    repository: CanonicalRepoName,
    ordinal: usize,
}
impl RepositoryWorkspaceStableKey {
    pub fn new(
        workspace: NormalizedAbsolutePath,
        owner: Arc<HostSelectedExtensionOwner>,
        repository: CanonicalRepoName,
        ordinal: usize,
    ) -> Result<Self, RepositoryWorkspaceError> {
        if repository.is_root() {
            return Err(RepositoryWorkspaceError::InvalidReservation);
        }
        Ok(Self {
            workspace,
            owner,
            repository,
            ordinal,
        })
    }
    pub fn workspace(&self) -> &NormalizedAbsolutePath {
        &self.workspace
    }
    pub fn owner(&self) -> &Arc<HostSelectedExtensionOwner> {
        &self.owner
    }
    pub fn repository(&self) -> &CanonicalRepoName {
        &self.repository
    }
    pub fn ordinal(&self) -> usize {
        self.ordinal
    }
    /// Version1 uses the pinned Rust Hash stream of these immutable structural
    /// fields, with fixed little-endian integer encoding. Never Debug/display or
    /// owner.unique_name alone. Native collision guards compare the whole key.
    pub fn candidate(&self) -> [u8; 32] {
        struct Writer(Sha256);
        impl Hasher for Writer {
            fn write(&mut self, bytes: &[u8]) {
                self.0.update(bytes);
            }
            fn finish(&self) -> u64 {
                u64::from_le_bytes(self.0.clone().finalize()[..8].try_into().unwrap())
            }
            fn write_usize(&mut self, value: usize) {
                self.write_u64(value as u64)
            }
            fn write_isize(&mut self, value: isize) {
                self.write_i64(value as i64)
            }
            fn write_u16(&mut self, v: u16) {
                self.write(&v.to_le_bytes())
            }
            fn write_u32(&mut self, v: u32) {
                self.write(&v.to_le_bytes())
            }
            fn write_u64(&mut self, v: u64) {
                self.write(&v.to_le_bytes())
            }
            fn write_u128(&mut self, v: u128) {
                self.write(&v.to_le_bytes())
            }
            fn write_i16(&mut self, v: i16) {
                self.write(&v.to_le_bytes())
            }
            fn write_i32(&mut self, v: i32) {
                self.write(&v.to_le_bytes())
            }
            fn write_i64(&mut self, v: i64) {
                self.write(&v.to_le_bytes())
            }
            fn write_i128(&mut self, v: i128) {
                self.write(&v.to_le_bytes())
            }
        }
        let mut writer = Writer(Sha256::new());
        writer.write(b"slug-repository-spelling-v1\0");
        self.hash(&mut writer);
        writer.0.finalize().into()
    }
    pub fn retained_storage_bytes(&self) -> Result<usize, RepositoryWorkspaceError> {
        let mut n = std::mem::size_of::<Self>() + 2 * std::mem::size_of::<usize>();
        n = add(
            n,
            self.workspace
                .retained_storage_bytes()
                .ok_or(RepositoryWorkspaceError::RetentionLimit)?,
        )?;
        n = add(
            n,
            self.owner
                .retained_storage_bytes()
                .ok_or(RepositoryWorkspaceError::RetentionLimit)?,
        )?;
        add(n, self.repository.retained_capacity_bytes())
    }
}
fn add(a: usize, b: usize) -> Result<usize, RepositoryWorkspaceError> {
    a.checked_add(b)
        .ok_or(RepositoryWorkspaceError::RetentionLimit)
}
fn arc<T>(count: usize) -> Result<usize, RepositoryWorkspaceError> {
    let tail = std::alloc::Layout::array::<T>(count)
        .map_err(|_| RepositoryWorkspaceError::RetentionLimit)?;
    let (layout, _) = std::alloc::Layout::new::<[usize; 2]>()
        .extend(tail)
        .map_err(|_| RepositoryWorkspaceError::RetentionLimit)?;
    Ok(layout.pad_to_align().size())
}
fn label(value: &CanonicalLabel) -> Result<usize, RepositoryWorkspaceError> {
    let mut n = value.package().repo().retained_capacity_bytes();
    n = add(n, value.package().package().retained_capacity_bytes())?;
    n = add(n, value.target().retained_capacity_bytes())?;
    add(
        n,
        value
            .mapping_id()
            .map_or(0, |v| v.retained_capacity_bytes()),
    )
}
#[derive(Default)]
pub(crate) struct WorkspaceSpecStorageWalk {
    nodes: usize,
}
impl WorkspaceSpecStorageWalk {
    fn step(&mut self, depth: usize) -> Result<(), RepositoryWorkspaceError> {
        self.nodes = self
            .nodes
            .checked_add(1)
            .filter(|n| *n <= 131072)
            .ok_or(RepositoryWorkspaceError::RetentionLimit)?;
        if depth > 64 {
            return Err(RepositoryWorkspaceError::RetentionLimit);
        }
        Ok(())
    }
    fn key(
        &mut self,
        key: &crate::OverrideAttributeKey,
        depth: usize,
    ) -> Result<usize, RepositoryWorkspaceError> {
        self.step(depth)?;
        match key {
            crate::OverrideAttributeKey::String(s) => Ok(s.capacity()),
            crate::OverrideAttributeKey::Label(l) => label(l),
        }
    }
    fn value(
        &mut self,
        value: &OverrideAttributeValue,
        depth: usize,
    ) -> Result<usize, RepositoryWorkspaceError> {
        self.step(depth)?;
        match value {
            OverrideAttributeValue::None
            | OverrideAttributeValue::Bool(_)
            | OverrideAttributeValue::Int(_) => Ok(0),
            OverrideAttributeValue::String(v) => Ok(v.capacity()),
            OverrideAttributeValue::Label(v) => label(v),
            OverrideAttributeValue::Iterable(values) => {
                let mut n = arc::<OverrideAttributeValue>(values.len())?;
                for value in values.iter() {
                    n = add(n, self.value(value, depth + 1)?)?;
                }
                Ok(n)
            }
            OverrideAttributeValue::Map(values) => {
                let mut n = map_bytes(values.as_ref())?;
                for (key, value) in values.iter() {
                    n = add(n, self.key(key, depth + 1)?)?;
                    n = add(n, self.value(value, depth + 1)?)?;
                }
                Ok(n)
            }
        }
    }
}
fn map_bytes<K, V>(
    map: &starlark_map::small_map::SmallMap<K, V>,
) -> Result<usize, RepositoryWorkspaceError> {
    add(
        arc::<starlark_map::small_map::SmallMap<K, V>>(1)?,
        map.retained_storage_bytes_upper_bound()
            .ok_or(RepositoryWorkspaceError::RetentionLimit)?,
    )
}

impl RepositoryMaterializationRequest {
    /// The independent terminal selection must match the pre-evaluation binding.
    #[doc(hidden)]
    pub fn validate_general_workspace_binding(&self) -> Result<(), RepositoryWorkspaceError> {
        let RepositoryMaterializationKind::GeneralWorkspace(realization) = &self.kind else {
            return Err(RepositoryWorkspaceError::InvalidOperation);
        };
        let reservation = realization.reservation();
        if self.id.workspace != *reservation.workspace()
            || self.id.canonical_repo != *reservation.repository()
            || &self.repo_spec != reservation.expected_repo_spec().as_ref()
        {
            return Err(RepositoryWorkspaceError::ForeignReservation);
        }
        Ok(())
    }
    /// Compare complete current selected facts while letting Core bound actual
    /// state/body work. Non-general kinds retain their ordinary full equality.
    #[doc(hidden)]
    pub fn try_eq_with_states<E>(
        &self,
        other: &Self,
        compare: &mut impl FnMut(
            crate::RepositoryWorkspaceReadViewStateRef<'_>,
            crate::RepositoryWorkspaceReadViewStateRef<'_>,
        ) -> Result<bool, E>,
    ) -> Result<bool, E> {
        let Self {
            id,
            repo_spec,
            kind,
        } = self;
        let Self {
            id: other_id,
            repo_spec: other_spec,
            kind: other_kind,
        } = other;
        if id != other_id || repo_spec != other_spec {
            return Ok(false);
        }
        match (kind, other_kind) {
            (
                RepositoryMaterializationKind::GeneralWorkspace(left),
                RepositoryMaterializationKind::GeneralWorkspace(right),
            ) => left.try_eq_with_states(right, compare),
            _ => Ok(kind == other_kind),
        }
    }

    pub fn general_workspace_storage_partition(
        &self,
    ) -> Result<crate::RepositoryWorkspaceStoragePartition, RepositoryWorkspaceError> {
        self.validate_general_workspace_binding()?;
        let RepositoryMaterializationKind::GeneralWorkspace(realization) = &self.kind else {
            return Err(RepositoryWorkspaceError::InvalidOperation);
        };
        let mut n = arc::<Self>(1)?;
        n = add(
            n,
            self.id
                .workspace
                .retained_storage_bytes()
                .ok_or(RepositoryWorkspaceError::RetentionLimit)?,
        )?;
        n = add(n, self.id.canonical_repo.retained_capacity_bytes())?;
        n = add(n, self.repo_spec.workspace_retained_storage_bytes()?)?;
        realization.storage_partition()?.add_owned(n)
    }
    pub fn general_workspace_storage_bytes(&self) -> Result<usize, RepositoryWorkspaceError> {
        self.general_workspace_storage_partition()?.total_bytes()
    }

    /// Requires separately retained exact body-allocation coverage. This pure
    /// forecast grants neither that coverage nor native IO authority.
    #[doc(hidden)]
    pub fn general_workspace_body_covered_storage_partition(
        &self,
    ) -> Result<crate::RepositoryWorkspaceStoragePartition, RepositoryWorkspaceError> {
        let full = self.general_workspace_storage_partition()?;
        let RepositoryMaterializationKind::GeneralWorkspace(realization) = &self.kind else {
            return Err(RepositoryWorkspaceError::InvalidOperation);
        };
        let payload = realization
            .storage_partition()?
            .owned_bytes()
            .checked_sub(realization.body_covered_storage_partition()?.owned_bytes())
            .ok_or(RepositoryWorkspaceError::RetentionLimit)?;
        crate::RepositoryWorkspaceStoragePartition::new(
            full.owned_bytes()
                .checked_sub(payload)
                .ok_or(RepositoryWorkspaceError::RetentionLimit)?,
            full.source_state_bytes(),
        )
    }

    #[doc(hidden)]
    pub fn try_visit_general_workspace_body_allocations<E>(
        &self,
        visit: &mut impl FnMut(&Arc<[u8]>) -> Result<(), E>,
    ) -> Result<Result<(), RepositoryWorkspaceError>, E> {
        if let Err(error) = self.validate_general_workspace_binding() {
            return Ok(Err(error));
        }
        let RepositoryMaterializationKind::GeneralWorkspace(realization) = &self.kind else {
            return Ok(Err(RepositoryWorkspaceError::InvalidOperation));
        };
        realization.try_visit_body_allocations(visit)?;
        Ok(Ok(()))
    }
    pub fn visit_read_views(&self, visit: &mut impl FnMut(&crate::RepositoryWorkspaceReadView)) {
        if let RepositoryMaterializationKind::GeneralWorkspace(realization) = &self.kind {
            realization.visit_read_views(visit)
        }
    }
}
impl RepoSpec {
    pub fn workspace_retained_storage_bytes(&self) -> Result<usize, RepositoryWorkspaceError> {
        self.workspace_retained_storage_bytes_with(&mut WorkspaceSpecStorageWalk::default())
    }
    pub(crate) fn workspace_retained_storage_bytes_with(
        &self,
        walk: &mut WorkspaceSpecStorageWalk,
    ) -> Result<usize, RepositoryWorkspaceError> {
        let mut n = arc::<Self>(1)?;
        n = add(n, label(&self.rule_id.bzl_file)?)?;
        n = add(n, self.rule_id.rule_name.capacity())?;
        n = add(n, map_bytes(self.attributes.as_ref())?)?;
        for (name, value) in self.attributes.iter() {
            walk.step(0)?;
            n = add(n, name.capacity())?;
            n = add(n, walk.value(value, 1)?)?;
        }
        Ok(n)
    }
}
/// Content projection only. Full terminal request/realization equality remains
/// semantic authority, including process history and reservation generation.
pub fn general_workspace_source_identity(realization: &RepositoryWorkspaceRealization) -> Arc<str> {
    realization.source_identity().clone()
}
pub(crate) fn derive_workspace_source_identity(
    state: &crate::RepositoryWorkspaceState,
) -> Arc<str> {
    let mut hash = Sha256::new();
    hash.update(b"slug-general-workspace-content-v2\0");
    hash.update((state.entries().len() as u64).to_le_bytes());
    for entry in state.entries() {
        hash.update((entry.path().len() as u64).to_le_bytes());
        hash.update(entry.path().as_bytes());
        hash.update(entry.mode().to_le_bytes());
        match entry.node() {
            crate::RepositoryWorkspaceNode::Directory => hash.update([0]),
            crate::RepositoryWorkspaceNode::File(bytes) => {
                hash.update([1]);
                hash.update((bytes.len() as u64).to_le_bytes());
                hash.update(bytes.as_ref());
            }
            crate::RepositoryWorkspaceNode::Link(target) => {
                hash.update([2]);
                hash.update((target.len() as u64).to_le_bytes());
                hash.update(target.as_bytes());
            }
        }
    }
    // These are image-content descriptors, not request or incarnation hashes.
    // Full provenance, mapping, lease and instance remain structural identity.
    hash.update((state.source_bindings().len() as u64).to_le_bytes());
    for view in state.source_bindings() {
        let root = view.display_root().as_path().as_os_str().as_encoded_bytes();
        hash.update((root.len() as u64).to_le_bytes());
        hash.update(root);
        let source = view
            .source_state()
            .expect("validated Source-only state binding");
        hash.update((source.entries().len() as u64).to_le_bytes());
        for entry in source.entries() {
            hash.update((entry.path().len() as u64).to_le_bytes());
            hash.update(entry.path().as_bytes());
            hash.update(entry.mode().to_le_bytes());
            match entry.node() {
                crate::RepositorySourceSnapshotNode::Directory => hash.update([0]),
                crate::RepositorySourceSnapshotNode::File(digest) => {
                    hash.update([1]);
                    hash.update(digest.size_bytes().to_le_bytes());
                    hash.update(digest.sha256());
                }
                crate::RepositorySourceSnapshotNode::Link(target) => {
                    hash.update([2]);
                    hash.update((target.len() as u64).to_le_bytes());
                    hash.update(target.as_bytes());
                }
            }
        }
    }
    let digest: [u8; 32] = hash.finalize().into();
    let mut text = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write;
        write!(&mut text, "{byte:02x}").unwrap();
    }
    text.into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RepositoryWorkspaceEntry;
    use crate::RepositoryWorkspaceMode;
    use crate::RepositoryWorkspaceNode;
    use crate::RepositoryWorkspaceOperation;
    use crate::RepositoryWorkspaceReservation;
    use crate::RepositoryWorkspaceResult;
    use crate::RepositoryWorkspaceRetentionLimits;
    use crate::RepositoryWorkspaceTranscript;
    fn owner() -> Arc<HostSelectedExtensionOwner> {
        Arc::new(HostSelectedExtensionOwner::testing("owner"))
    }
    fn key(owner: Arc<HostSelectedExtensionOwner>) -> RepositoryWorkspaceStableKey {
        RepositoryWorkspaceStableKey::new(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            owner,
            CanonicalRepoName::new("repo").unwrap(),
            0,
        )
        .unwrap()
    }
    fn initial(instance: u64, lease: u64) -> RepositoryWorkspaceTranscript {
        RepositoryWorkspaceTranscript::new(
            Arc::new(
                RepositoryWorkspaceReservation::new(
                    NormalizedAbsolutePath::new("/workspace").unwrap(),
                    owner(),
                    CanonicalRepoName::new("repo").unwrap(),
                    0,
                    Arc::new(spec(SmallMap::new())),
                    NormalizedAbsolutePath::new("/__slug_repositories__/test").unwrap(),
                    PathObservationInstanceId::new(instance),
                    lease,
                    RepositoryWorkspaceRetentionLimits::new(64, 1024 * 1024, 1024 * 1024).unwrap(),
                )
                .unwrap(),
            ),
            0o755,
        )
        .unwrap()
    }
    fn write(
        transcript: RepositoryWorkspaceTranscript,
        body: &[u8],
    ) -> RepositoryWorkspaceTranscript {
        let bytes: Arc<[u8]> = Arc::from(body);
        let request = transcript
            .request(RepositoryWorkspaceOperation::File {
                path: "out".into(),
                body: bytes.clone(),
                mode: 0o644,
            })
            .unwrap();
        transcript
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
                        "out".into(),
                        0o644,
                        RepositoryWorkspaceNode::File(bytes),
                    )
                    .unwrap(),
                ],
            )
            .unwrap()
    }
    #[test]
    fn stable_candidate_uses_complete_owner_identity_and_preserves_structural_authority() {
        let same_name_other_kind = Arc::new(HostSelectedExtensionOwner::testing_with_kind(
            "owner",
            crate::HostSelectedExtensionOwnerKind::InnateRepositoryRule,
        ));
        let a = key(owner());
        let a2 = key(owner());
        let b = key(same_name_other_kind);
        assert_eq!(a, a2);
        assert_eq!(a.candidate(), a2.candidate());
        // Recorded from the full version-one stream before Demand hash reuse.
        #[cfg(target_os = "linux")]
        assert_eq!(
            a.candidate(),
            [
                233, 116, 247, 202, 5, 244, 243, 9, 165, 175, 208, 52, 164, 103, 224, 35, 185, 211,
                130, 250, 237, 34, 52, 110, 246, 209, 232, 216, 71, 45, 127, 46
            ],
            "version-one spelling must retain its full path hash stream"
        );
        assert_eq!(a.owner().unique_name(), b.owner().unique_name());
        assert_ne!(a, b);
        assert_ne!(a.candidate(), b.candidate());
        let ordinal = RepositoryWorkspaceStableKey::new(
            a.workspace.clone(),
            a.owner.clone(),
            a.repository.clone(),
            1,
        )
        .unwrap();
        assert_ne!(a.candidate(), ordinal.candidate());
    }
    #[test]
    fn realization_memoizes_content_without_turning_equal_final_digest_into_history_identity() {
        let a = write(initial(1, 2), b"same").realize(1).unwrap();
        let b = write(write(initial(1, 2), b"prior"), b"same")
            .realize(2)
            .unwrap();
        let c = write(initial(3, 4), b"same").realize(1).unwrap();
        assert_eq!(a.source_identity(), b.source_identity());
        assert_eq!(a.source_identity(), c.source_identity());
        assert_ne!(a, b);
        assert_ne!(a, c);
        assert!(Arc::ptr_eq(
            &general_workspace_source_identity(&a),
            a.source_identity()
        ));
        assert!(
            a.transcript().realization_storage_bytes().unwrap()
                >= a.retained_storage_bytes().unwrap()
        );
        assert_ne!(
            a.source_identity(),
            write(initial(1, 2), b"changed")
                .realize(1)
                .unwrap()
                .source_identity()
        );
    }
    fn spec(attributes: SmallMap<CompactString, OverrideAttributeValue>) -> RepoSpec {
        RepoSpec {
            rule_id: crate::RepoRuleId {
                bzl_file: CanonicalLabel::parse("@@owner//:defs.bzl").unwrap(),
                rule_name: "rule".into(),
            },
            attributes: Arc::new(attributes),
        }
    }
    #[test]
    fn request_storage_counts_foreign_shared_spare_maps_and_bounds_recursive_values() {
        let dense = spec(SmallMap::new());
        let sparse = spec(SmallMap::with_capacity(128));
        assert_eq!(dense, sparse);
        assert!(
            sparse.workspace_retained_storage_bytes().unwrap()
                > dense.workspace_retained_storage_bytes().unwrap()
        );
        let shared = sparse.clone();
        assert_eq!(
            shared.workspace_retained_storage_bytes(),
            sparse.workspace_retained_storage_bytes()
        );
        let mut nested = OverrideAttributeValue::None;
        for _ in 0..65 {
            nested = OverrideAttributeValue::Iterable(Arc::from([nested]));
        }
        let deep = spec(SmallMap::from_iter([(
            CompactString::new("nested"),
            nested,
        )]));
        assert_eq!(
            deep.workspace_retained_storage_bytes(),
            Err(RepositoryWorkspaceError::RetentionLimit)
        );
        let realized = write(initial(1, 2), b"body").realize(1).unwrap();
        let request = RepositoryMaterializationRequest {
            id: RepositoryMaterializationRequestId {
                workspace: realized.reservation().workspace().clone(),
                canonical_repo: realized.reservation().repository().clone(),
            },
            repo_spec: sparse,
            kind: RepositoryMaterializationKind::GeneralWorkspace(realized.clone()),
        };
        assert!(
            request.general_workspace_storage_bytes().unwrap()
                >= realized.retained_storage_bytes().unwrap()
                    + request
                        .repo_spec
                        .workspace_retained_storage_bytes()
                        .unwrap()
        );
        let valid = RepositoryMaterializationEpochEntry {
            request: Arc::new(request),
            result: RepositoryMaterializationResult::Success(
                RepositoryMaterializationSuccess::Immutable {
                    path_policy: crate::RepositoryMaterializationPathPolicy::GenerationRoot,
                    source_identity: realized.source_identity().clone(),
                    generation_root: realized
                        .reservation()
                        .display_root()
                        .as_path()
                        .to_path_buf(),
                    observation_instance: realized.reservation().instance(),
                },
            ),
        };
        assert!(
            RepositoryMaterializationResultEpoch::new(
                realized.reservation().workspace().clone(),
                [valid.clone()]
            )
            .is_ok()
        );
        let mut forged = valid;
        let RepositoryMaterializationResult::Success(RepositoryMaterializationSuccess::Immutable {
            source_identity,
            ..
        }) = &mut forged.result
        else {
            unreachable!()
        };
        *source_identity = "forged".into();
        assert!(
            RepositoryMaterializationResultEpoch::new(
                realized.reservation().workspace().clone(),
                [forged]
            )
            .is_err()
        );
    }
}
