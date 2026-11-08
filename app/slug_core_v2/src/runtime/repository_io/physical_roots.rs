//! Checked selected-root leases. This extends the materializer's sole registry;
//! no path lookup or native owner is installed into DICE.
use super::*;
use crate::runtime::SourceArtifactInput;
use crate::runtime::native_source_access::NativeReaderCredit;
use crate::runtime::native_source_access::NativeReaderResources;
#[cfg(target_os = "linux")]
use crate::runtime::repository_physical_root::RepositoryPhysicalRoot;

/// Data-only legacy lifetime. Dependencies name already-existing owners, so
/// retaining a generated symlink cannot retain a request/history or form cycles.
#[derive(Debug)]
pub(in crate::runtime) struct LegacyRootOwner {
    directory: tempfile::TempDir,
    _dependencies: LegacyRootDependencies,
}
impl LegacyRootOwner {
    pub(super) fn new(directory: tempfile::TempDir, dependencies: LegacyRootDependencies) -> Self {
        Self {
            directory,
            _dependencies: dependencies,
        }
    }
    pub(in crate::runtime) fn path(&self) -> &Path {
        self.directory.path()
    }
}
#[derive(Debug)]
pub(super) struct LegacyRootDependencies {
    _owners: Vec<RetainedRootOwner>,
    // Owners and their vector retire before this metadata allowance.
    _credit: Option<NativeReaderCredit>,
}

#[derive(Clone, Debug)]
pub(super) enum RetainedRootOwner {
    Legacy(Arc<LegacyRootOwner>),
    #[cfg(target_os = "linux")]
    Physical(Arc<RepositoryPhysicalRoot>),
}
impl RetainedRootOwner {
    pub(super) fn path(&self) -> &Path {
        match self {
            Self::Legacy(root) => root.path(),
            #[cfg(target_os = "linux")]
            Self::Physical(root) => root.reservation().display_root().as_path(),
        }
    }
    pub(super) fn is_physical(&self) -> bool {
        match self {
            Self::Legacy(_) => false,
            #[cfg(target_os = "linux")]
            Self::Physical(_) => true,
        }
    }
    pub(super) fn same_owner(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Legacy(a), Self::Legacy(b)) => Arc::ptr_eq(a, b),
            #[cfg(target_os = "linux")]
            (Self::Physical(a), Self::Physical(b)) => Arc::ptr_eq(a, b),
            #[allow(unreachable_patterns)]
            _ => false,
        }
    }
    pub(super) fn legacy(&self) -> Option<&Arc<LegacyRootOwner>> {
        match self {
            Self::Legacy(root) => Some(root),
            #[cfg(target_os = "linux")]
            Self::Physical(_) => None,
        }
    }
}
#[derive(Debug)]
struct SelectedRootData {
    repository: CanonicalRepoName,
    instance: PathObservationInstanceId,
    root: RetainedRootOwner,
    // A source lease deliberately keeps no complete terminal request/history.
    _credit: NativeReaderCredit,
}
#[derive(Clone, Debug)]
pub(in crate::runtime) struct SelectedRootLease {
    data: Arc<SelectedRootData>,
}
impl SelectedRootLease {
    #[cfg(target_os = "linux")]
    pub(in crate::runtime) fn physical(&self) -> Option<&Arc<RepositoryPhysicalRoot>> {
        match &self.data.root {
            RetainedRootOwner::Physical(root) => Some(root),
            _ => None,
        }
    }
    pub(in crate::runtime) fn namespace(&self) -> PathObservationNamespace {
        PathObservationNamespace::Materialization(self.data.instance)
    }
    pub(in crate::runtime) fn generation_root(&self) -> &Path {
        self.data.root.path()
    }
    fn validate(&self, source: &SourceArtifactInput) -> Result<(), Arc<str>> {
        if source.label().package().repo() != &self.data.repository
            || source.namespace() != self.namespace()
            || !source
                .requested_path()
                .as_path()
                .starts_with(self.generation_root())
        {
            return Err("source does not belong to the selected native root".into());
        }
        #[cfg(target_os = "linux")]
        match self.physical() {
            Some(root) => {
                if !source
                    .real_path()
                    .as_path()
                    .starts_with(self.generation_root())
                {
                    return Err("physical real path escapes selected root".into());
                }
                if source.anchor() != Some(root.anchor()) {
                    return Err("physical source anchor differs from selected root".into());
                }
                let relative = source
                    .real_path()
                    .as_path()
                    .strip_prefix(self.generation_root())
                    .map_err(|_| Arc::<str>::from("physical source is outside selected root"))?;
                let relative = relative
                    .to_str()
                    .ok_or_else(|| Arc::<str>::from("physical source path is not Unicode"))?;
                let Some(entry) = root.state().entry(relative) else {
                    return Err("physical source is absent from sealed state".into());
                };
                let slug_bzlmod_v2::RepositoryWorkspaceNode::File(bytes) = entry.node() else {
                    return Err("physical source is not a sealed regular file".into());
                };
                if source.digest().size_bytes() != bytes.len() as u64
                    || source.digest().sha256() != &<[u8; 32]>::from(Sha256::digest(bytes))
                {
                    return Err("physical source digest differs from sealed state".into());
                }
            }
            None => {
                if source.anchor().is_some() {
                    return Err("legacy root cannot authorize a physical source anchor".into());
                }
            }
        }
        #[cfg(not(target_os = "linux"))]
        if source.anchor().is_some() {
            return Err("physical source unsupported on this platform".into());
        }
        Ok(())
    }
}
#[derive(Debug)]
struct SelectionCredits {
    _base: NativeReaderCredit,
    _dynamic: Option<NativeReaderCredit>,
}
#[derive(Clone, Debug, Default)]
pub(in crate::runtime) struct SelectedRepositoryRoots {
    leases: Arc<[SelectedRootLease]>,
    _credit: Option<Arc<SelectionCredits>>,
}
impl SelectedRepositoryRoots {
    pub(in crate::runtime) fn lease_for(
        &self,
        source: &SourceArtifactInput,
    ) -> Result<Option<SelectedRootLease>, Arc<str>> {
        if source.namespace() == PathObservationNamespace::Host {
            if self
                .leases
                .iter()
                .any(|lease| &lease.data.repository == source.label().package().repo())
            {
                return Err(
                    "selected immutable repository cannot use Host source authority".into(),
                );
            }
            if source.anchor().is_some() {
                return Err("Host source has a physical anchor".into());
            }
            return Ok(None);
        }
        let mut found = self
            .leases
            .iter()
            .filter(|lease| lease.namespace() == source.namespace());
        let lease = found.next().ok_or_else(|| {
            Arc::<str>::from("materialized source has no exact selected native lease")
        })?;
        if found.next().is_some() {
            return Err("ambiguous selected native root instance".into());
        }
        lease.validate(source)?;
        Ok(Some(lease.clone()))
    }
    pub(super) fn retain_sources<'a>(
        &self,
        sources: impl Iterator<Item = &'a SourceArtifactInput>,
        resources: &Arc<NativeReaderResources>,
    ) -> Result<Self, Arc<str>> {
        let bytes = self
            .leases
            .len()
            .checked_mul(2 * std::mem::size_of::<SelectedRootLease>())
            .and_then(|n| n.checked_add(std::mem::size_of::<Self>() + 64))
            .ok_or_else(|| Arc::<str>::from("source lease metadata overflow"))?;
        let credit = resources
            .reserve(bytes, 0, 0)
            .map_err(|_| Arc::<str>::from("source lease metadata refused"))?;
        let mut leases = Vec::with_capacity(self.leases.len());
        for source in sources {
            if let Some(lease) = self.lease_for(source)? {
                if !leases
                    .iter()
                    .any(|prior: &SelectedRootLease| prior.namespace() == lease.namespace())
                {
                    leases.push(lease);
                }
            }
        }
        Ok(Self {
            leases: leases.into(),
            _credit: Some(Arc::new(SelectionCredits {
                _base: credit,
                _dynamic: None,
            })),
        })
    }
    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.leases.len()
    }
    #[cfg(test)]
    pub(super) fn is_empty(&self) -> bool {
        self.leases.is_empty()
    }
    #[cfg(test)]
    pub(super) fn same_set(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.leases, &other.leases)
    }
}
impl RepositoryMaterializer {
    pub(super) fn legacy_dependencies(
        &self,
        token: RepositorySessionToken,
        request: &RepositoryMaterializationRequest,
    ) -> Result<LegacyRootDependencies, RepositorySessionError> {
        let plan = match &request.kind {
            RepositoryMaterializationKind::GeneratedFileEffects(plan) => Some(plan),
            RepositoryMaterializationKind::GeneratedFilesystemEffects(realization) => {
                Some(realization.terminal_plan())
            }
            _ => None,
        };
        let state = self
            .state
            .lock()
            .expect("repository materializer mutex poisoned");
        let active = matching_validated_active(&state, token)?;
        let sources = plan.map_or(0, |plan| plan.ordered_effects().filter(|effect| {
            matches!(effect, GeneratedRepositoryOrderedEffectRef::Symlink(link)
                if matches!(link.source().namespace(), PathObservationNamespace::Materialization(_)))
        }).count());
        let capacity = sources.min(
            state
                .accepted_roots
                .len()
                .checked_add(active.provisional_roots.len())
                .ok_or_else(|| {
                    RepositorySessionError::NativeWorkspace(
                        "legacy dependency count overflow".into(),
                    )
                })?,
        );
        // Same finite metadata scale as the admitted native observation batch.
        if capacity > 131_072 {
            return Err(RepositorySessionError::NativeWorkspace(
                "legacy dependency count exceeds native metadata bound".into(),
            ));
        }
        #[cfg(target_os = "linux")]
        let credit = state
            .claim_domain
            .as_ref()
            .map(|domain| {
                let bytes = capacity
                    .checked_mul(2 * std::mem::size_of::<RetainedRootOwner>())
                    .and_then(|n| n.checked_add(std::mem::size_of::<LegacyRootOwner>() + 256))
                    .ok_or_else(|| {
                        RepositorySessionError::NativeWorkspace(
                            "legacy dependency storage overflow".into(),
                        )
                    })?;
                domain.reserve_observation(bytes)
            })
            .transpose()?;
        #[cfg(not(target_os = "linux"))]
        let credit = None;
        let mut owners = Vec::with_capacity(capacity);
        if owners.capacity() > capacity {
            return Err(RepositorySessionError::NativeWorkspace(
                "legacy dependency allocation exceeded allowance".into(),
            ));
        }
        if let Some(plan) = plan {
            for effect in plan.ordered_effects() {
                let GeneratedRepositoryOrderedEffectRef::Symlink(link) = effect else {
                    continue;
                };
                let PathObservationNamespace::Materialization(instance) = link.source().namespace()
                else {
                    continue;
                };
                let mut rows = state
                    .accepted_roots
                    .iter()
                    .chain(&active.provisional_roots)
                    .filter(|row| row.observation_instance == instance);
                let row = rows.next().ok_or_else(|| {
                    RepositorySessionError::NativeWorkspace(
                        "generated symlink lacks current native source owner".into(),
                    )
                })?;
                if rows.next().is_some()
                    || row.root.is_physical()
                    || !link.source().path().as_path().starts_with(row.root.path())
                    || !active.entries.iter().any(|entry| {
                        Arc::ptr_eq(&entry.request, &row.request)
                            && matches!(&entry.result,
                                RepositoryMaterializationResult::Success(
                                    RepositoryMaterializationSuccess::Immutable {
                                        observation_instance, generation_root, source_identity,
                                    }
                                ) if *observation_instance == instance
                                    && generation_root == row.root.path()
                                    && source_identity == &row.source_identity)
                    })
                {
                    return Err(RepositorySessionError::NativeWorkspace(
                        "generated symlink native source binding refused".into(),
                    ));
                }
                if !owners
                    .iter()
                    .any(|owner: &RetainedRootOwner| owner.same_owner(&row.root))
                {
                    owners.push(row.root.clone());
                }
            }
        }
        Ok(LegacyRootDependencies {
            _owners: owners,
            _credit: credit,
        })
    }
    pub(in crate::runtime) fn pin_selected_roots(
        &self,
        token: RepositorySessionToken,
        selected: &[Arc<RepositoryMaterializationRequest>],
        results: &RepositoryMaterializationResultEpoch,
        resources: &Arc<NativeReaderResources>,
    ) -> Result<SelectedRepositoryRoots, Arc<str>> {
        self.check_source_scope(self.source_scope.as_ref())
            .map_err(|_| Arc::<str>::from("selected root scope refused"))?;
        let bytes = selected
            .len()
            .checked_mul(
                16 * std::mem::size_of::<RepositoryMaterializationEpochEntry>()
                    + std::mem::size_of::<RetainedRepositoryRoot>() * 4
                    + 4096,
            )
            .and_then(|n| n.checked_add(std::mem::size_of::<SelectedRepositoryRoots>() + 128))
            .ok_or_else(|| Arc::<str>::from("selected root storage overflow"))?;
        let control = resources
            .reserve(bytes, 0, 0)
            .map_err(|_| Arc::<str>::from("selected root storage unavailable"))?;
        let dynamic_credit;
        let (rows, entries) = {
            let state = self
                .state
                .lock()
                .expect("repository materializer mutex poisoned");
            let active = matching_validated_active(&state, token)
                .map_err(|_| Arc::<str>::from("selected root session refused"))?;
            // Forecast dynamically owned results and canonical map keys from
            // BORROWED current entries before cloning any PathBuf/string/key.
            let mut dynamic_bytes = 0usize;
            for request in selected {
                let entry = active
                    .entries
                    .iter()
                    .find(|entry| entry.request.id == request.id)
                    .ok_or_else(|| Arc::<str>::from("selected repository absent"))?;
                dynamic_bytes = dynamic_bytes
                    .checked_add(
                        cloned_epoch_dynamic_storage(std::iter::once(entry))
                            .map_err(|_| Arc::<str>::from("selected dynamic storage overflow"))?,
                    )
                    .ok_or_else(|| Arc::<str>::from("selected dynamic storage overflow"))?;
            }
            dynamic_credit = resources
                .reserve(dynamic_bytes, 0, 0)
                .map_err(|_| Arc::<str>::from("selected dynamic storage unavailable"))?;
            let mut rows = Vec::with_capacity(selected.len());
            let mut entries = Vec::with_capacity(selected.len());
            for request in selected {
                if entries
                    .iter()
                    .any(|entry: &RepositoryMaterializationEpochEntry| {
                        entry.request.id == request.id
                    })
                {
                    return Err("duplicate selected repository".into());
                }
                let entry = active
                    .entries
                    .iter()
                    .find(|entry| entry.request.id == request.id)
                    .ok_or_else(|| Arc::<str>::from("selected repository absent"))?;
                if let RepositoryMaterializationResult::Success(
                    RepositoryMaterializationSuccess::Immutable {
                        observation_instance,
                        ..
                    },
                ) = &entry.result
                {
                    let mut found = state
                        .accepted_roots
                        .iter()
                        .chain(&active.provisional_roots)
                        .filter(|row| row.observation_instance == *observation_instance);
                    let row = found
                        .next()
                        .ok_or_else(|| Arc::<str>::from("selected root lacks native owner"))?;
                    if found.next().is_some() {
                        return Err("ambiguous native root instance".into());
                    }
                    rows.push(row.clone());
                }
                entries.push(entry.clone());
            }
            (rows, entries)
        };
        // Complete foreign semantic comparisons and epoch construction happen
        // outside the actor mutex. Cloned row guards keep their request/history
        // prepaid even if a concurrent discard retires the original row.
        let active = || self.session_is_active(token);
        let mut comparison = RepositoryRequestComparison::new(
            Some(resources),
            self.source_scope.as_deref(),
            &active,
        )
        .map_err(|_| Arc::<str>::from("selected request comparison refused"))?;
        for (entry, request) in entries.iter().zip(selected) {
            if !matches!(entry.result, RepositoryMaterializationResult::Success(_)) {
                return Err("selected repository lacks successful materialization".into());
            }
            if !comparison
                .equal(&entry.request, request)
                .map_err(|_| Arc::<str>::from("selected request comparison refused"))?
            {
                return Err("selected request differs from current request".into());
            }
        }
        if !comparison
            .epoch_equal(
                &complete_epoch(&self.workspace, &entries)
                    .map_err(|_| Arc::<str>::from("selected epoch refused"))?,
                results,
            )
            .map_err(|_| Arc::<str>::from("selected epoch comparison refused"))?
        {
            return Err("selected result epoch differs from current results".into());
        }
        let mut leases = Vec::with_capacity(rows.len());
        for row in &rows {
            let entry = entries
                .iter()
                .find(|entry| entry.request.id == row.request.id)
                .ok_or_else(|| Arc::<str>::from("root request absent"))?;
            let RepositoryMaterializationResult::Success(
                RepositoryMaterializationSuccess::Immutable {
                    observation_instance,
                    generation_root,
                    source_identity,
                },
            ) = &entry.result
            else {
                return Err("selected native root lacks immutable success".into());
            };
            if !Arc::ptr_eq(&row.request, &entry.request)
                || row.observation_instance != *observation_instance
                || row.root.path() != generation_root
                || &row.source_identity != source_identity
                || matches!(
                    entry.request.kind,
                    RepositoryMaterializationKind::GeneralWorkspace(_)
                ) != row.root.is_physical()
            {
                return Err("selected native root association differs".into());
            }
            let credit = resources
                .reserve(
                    std::mem::size_of::<SelectedRootData>()
                        + 64
                        + entry.request.id.canonical_repo.retained_capacity_bytes(),
                    0,
                    0,
                )
                .map_err(|_| Arc::<str>::from("selected root lease unavailable"))?;
            leases.push(SelectedRootLease {
                data: Arc::new(SelectedRootData {
                    repository: entry.request.id.canonical_repo.clone(),
                    instance: *observation_instance,
                    root: row.root.clone(),
                    _credit: credit,
                }),
            });
        }
        self.check_source_scope(self.source_scope.as_ref())
            .map_err(|_| Arc::<str>::from("selected root scope changed"))?;
        let state = self
            .state
            .lock()
            .expect("repository materializer mutex poisoned");
        let active = matching_validated_active(&state, token)
            .map_err(|_| Arc::<str>::from("selected root session changed"))?;
        for entry in &entries {
            if !active
                .entries
                .iter()
                .any(|now| Arc::ptr_eq(&now.request, &entry.request) && now.result == entry.result)
            {
                return Err("selected result changed during lease construction".into());
            }
        }
        for row in &rows {
            if !association_current(&state, active, row) {
                return Err("native root changed during lease construction".into());
            }
        }
        Ok(SelectedRepositoryRoots {
            leases: leases.into(),
            _credit: Some(Arc::new(SelectionCredits {
                _base: control,
                _dynamic: Some(dynamic_credit),
            })),
        })
    }
}
pub(super) fn association_current(
    state: &RepositoryMaterializerState,
    active: &ActiveRepositorySession,
    expected: &RetainedRepositoryRoot,
) -> bool {
    state
        .accepted_roots
        .iter()
        .chain(&active.provisional_roots)
        .any(|row| {
            row.observation_instance == expected.observation_instance
                && row.generation == expected.generation
                && Arc::ptr_eq(&row.request, &expected.request)
                && row.source_identity == expected.source_identity
                && row.root.same_owner(&expected.root)
        })
}

#[cfg(target_os = "linux")]
pub(super) struct PhysicalObservationBatch {
    pub(super) legacy: Vec<PathObservationDemand>,
    physical: Vec<(RetainedRepositoryRoot, Vec<PathObservationDemand>)>,
    _credit: Option<crate::runtime::repository_physical_root::ResourceCredit>,
}
#[cfg(target_os = "linux")]
impl PhysicalObservationBatch {
    pub(super) fn observe(
        &self,
        actor: &RepositoryMaterializer,
        token: RepositorySessionToken,
        legacy: PathObservationEpoch,
    ) -> Result<PathObservationEpoch, RepositorySessionError> {
        self.observe_impl(actor, token, legacy, || {})
    }
    fn observe_impl(
        &self,
        actor: &RepositoryMaterializer,
        token: RepositorySessionToken,
        legacy: PathObservationEpoch,
        after_io: impl FnOnce(),
    ) -> Result<PathObservationEpoch, RepositorySessionError> {
        let mut epochs = Vec::with_capacity(self.physical.len() + 1);
        epochs.push(legacy);
        for (row, demands) in &self.physical {
            let RetainedRootOwner::Physical(root) = &row.root else {
                unreachable!()
            };
            let scope = actor.source_scope.as_ref().ok_or_else(|| {
                RepositorySessionError::NativeWorkspace(
                    "physical observation requires original scope".into(),
                )
            })?;
            epochs.push(
                root.observe(demands, scope, &|| actor.session_is_active(token))
                    .map_err(|error| {
                        #[cfg(test)]
                        report_physical_observation_refusal(&error);
                        #[cfg(not(test))]
                        let _ = error;
                        RepositorySessionError::NativeWorkspace(
                            "physical root observation refused".into(),
                        )
                    })?,
            );
        }
        actor.check_source_scope(actor.source_scope.as_ref())?;
        let merged = PathObservationEpoch::from_shared(epochs.iter().flat_map(|epoch| {
            epoch
                .observations()
                .iter()
                .map(|(d, r)| (d.clone(), r.clone()))
        }))
        .map_err(|_| {
            RepositorySessionError::NativeWorkspace("mixed native observation epoch refused".into())
        })?;
        after_io();
        let state = actor
            .state
            .lock()
            .expect("repository materializer mutex poisoned");
        let active = matching_active(&state, token)?;
        for (row, _) in &self.physical {
            if !association_current(&state, active, row)
                || !active.entries.iter().any(|entry| {
                    Arc::ptr_eq(&entry.request, &row.request)
                        && row_matches_result(row, &entry.result)
                })
            {
                return Err(RepositorySessionError::NativeWorkspace(
                    "physical observation association changed".into(),
                ));
            }
        }
        Ok(merged)
    }
}
#[cfg(all(test, target_os = "linux"))]
fn report_physical_observation_refusal(
    error: &crate::runtime::repository_physical_root::PhysicalRootError,
) {
    use crate::runtime::path_observation::rooted::RootedObservationError as O;
    use crate::runtime::repository_physical_root::PhysicalRootError as E;
    let (category, errno) = match error {
        E::Io(error) => ("io", error.raw_os_error()),
        E::Reservation => ("reservation", None),
        E::Mode => ("mode", None),
        E::Backing => ("backing", None),
        E::RootIdentity => ("root_identity", None),
        E::Snapshot => ("snapshot", None),
        E::UnsupportedOperation => ("unsupported_operation", None),
        E::Mutation(_) => ("mutation", None),
        E::Protocol(_) => ("protocol", None),
        E::Resource => ("resource", None),
        E::SourceService(_) => ("source_service", None),
        E::Inactive => ("inactive", None),
        E::Observation(error) => match error {
            O::UnauthorizedDemand(_) => ("observation_unauthorized", None),
            O::DuplicateDemand => ("observation_duplicate", None),
            O::Bound => ("observation_bound", None),
            O::Unstable => ("observation_unstable", None),
            O::Inactive => ("observation_inactive", None),
            O::RootIo(error) => ("observation_root_io", error.raw_os_error()),
            O::SourceService(_) => ("observation_source_service", None),
            O::Epoch(_) => ("observation_epoch", None),
        },
    };
    eprintln!("physical observation refused: category={category} errno={errno:?}");
}
fn row_matches_result(
    row: &RetainedRepositoryRoot,
    result: &RepositoryMaterializationResult,
) -> bool {
    matches!(result,RepositoryMaterializationResult::Success(RepositoryMaterializationSuccess::Immutable{source_identity,generation_root,observation_instance})
        if source_identity==&row.source_identity&&generation_root==row.root.path()&&*observation_instance==row.observation_instance)
}
#[cfg(target_os = "linux")]
impl RepositoryMaterializer {
    #[cfg(test)]
    pub(in crate::runtime) fn observe_with_recheck_hook_for_test(
        &self,
        token: RepositorySessionToken,
        demands: Vec<PathObservationDemand>,
        after_io: impl FnOnce(),
    ) -> Result<PathObservationEpoch, RepositorySessionError> {
        let (demands, _credit) = self.collect_native_demands(token, demands)?;
        let mut batch = self.split_physical_demands(token, demands)?;
        let legacy = self.observe_native(token, std::mem::take(&mut batch.legacy))?;
        batch.observe_impl(self, token, legacy, after_io)
    }
    #[cfg(test)]
    pub(in crate::runtime) fn replace_source_identity_for_test(
        &self,
        token: RepositorySessionToken,
        repo: &CanonicalRepoName,
    ) {
        let mut state = self.state.lock().unwrap();
        let active = matching_validated_active_mut(&mut state, token).unwrap();
        let entry = active
            .entries
            .iter_mut()
            .find(|entry| &entry.request.id.canonical_repo == repo)
            .unwrap();
        let RepositoryMaterializationResult::Success(RepositoryMaterializationSuccess::Immutable {
            source_identity,
            ..
        }) = &mut entry.result
        else {
            panic!("immutable result expected")
        };
        *source_identity = "changed-after-io".into();
    }
    pub(super) fn collect_native_demands(
        &self,
        token: RepositorySessionToken,
        demands: impl IntoIterator<Item = PathObservationDemand>,
    ) -> Result<
        (
            Vec<PathObservationDemand>,
            Option<crate::runtime::repository_physical_root::ResourceCredit>,
        ),
        RepositorySessionError,
    > {
        let resources = {
            let state = self
                .state
                .lock()
                .expect("repository materializer mutex poisoned");
            matching_active(&state, token)?;
            state
                .claim_domain
                .as_ref()
                .map(|domain| domain.resources().clone())
        };
        let mut credit = None;
        let mut rows = Vec::new();
        for demand in demands {
            if rows.len() == 131072 {
                return Err(RepositorySessionError::NativeWorkspace(
                    "native observation demand limit".into(),
                ));
            }
            if let Some(resources) = &resources {
                super::reservations::reserve_capacity(&mut rows, &mut credit, resources, 131072)?;
            }
            rows.push(demand);
        }
        Ok((rows, credit))
    }
    pub(super) fn split_physical_demands(
        &self,
        token: RepositorySessionToken,
        demands: Vec<PathObservationDemand>,
    ) -> Result<PhysicalObservationBatch, RepositorySessionError> {
        let (credit, rows) = {
            let state = self
                .state
                .lock()
                .expect("repository materializer mutex poisoned");
            let active = matching_active(&state, token)?;
            let count = state
                .accepted_roots
                .iter()
                .chain(&active.provisional_roots)
                .filter(|row| {
                    row.root.is_physical()
                        && active.entries.iter().any(|entry| {
                            Arc::ptr_eq(&entry.request, &row.request)
                                && row_matches_result(row, &entry.result)
                        })
                })
                .count();
            if count == 0 {
                return Ok(PhysicalObservationBatch {
                    legacy: demands,
                    physical: Vec::new(),
                    _credit: None,
                });
            }
            let domain = state.claim_domain.as_ref().ok_or_else(|| {
                RepositorySessionError::NativeWorkspace("missing physical selection domain".into())
            })?;
            let bytes = demands
                .len()
                .checked_mul(4 * std::mem::size_of::<PathObservationDemand>() + 256)
                .and_then(|n| {
                    n.checked_add(count * (std::mem::size_of::<RetainedRepositoryRoot>() * 4 + 128))
                })
                .ok_or_else(|| {
                    RepositorySessionError::NativeWorkspace(
                        "physical observation storage overflow".into(),
                    )
                })?;
            let credit = domain.reserve_observation(bytes)?;
            let rows = state
                .accepted_roots
                .iter()
                .chain(&active.provisional_roots)
                .filter(|row| {
                    row.root.is_physical()
                        && active.entries.iter().any(|entry| {
                            Arc::ptr_eq(&entry.request, &row.request)
                                && row_matches_result(row, &entry.result)
                        })
                })
                .cloned()
                .collect::<Vec<_>>();
            (credit, rows)
        };
        let mut physical = Vec::with_capacity(rows.len());
        for row in rows {
            let RetainedRootOwner::Physical(root) = &row.root else {
                unreachable!()
            };
            if !matches!(&row.request.kind,RepositoryMaterializationKind::GeneralWorkspace(realization)
                if realization.reservation()==root.reservation()&&realization.final_state()==root.state())
                || row.observation_instance != root.reservation().instance()
                || !root
                    .source_claim()
                    .is_some_and(|claim| claim.matches_reservation(root.reservation()))
            {
                return Err(RepositorySessionError::NativeWorkspace(
                    "physical observation association differs".into(),
                ));
            }
            physical.push((row, Vec::new()));
        }
        let mut legacy = Vec::new();
        let mut paths = 0usize;
        let mut seen = starlark_map::small_set::SmallSet::new();
        for demand in demands {
            if let Some((row, batch)) = physical.iter_mut().find(|(row, _)| {
                PathObservationNamespace::Materialization(row.observation_instance)
                    == demand.namespace()
            }) {
                let path = demand.path().as_path();
                paths = paths
                    .checked_add(path.as_os_str().len())
                    .filter(|n| *n <= 4 * 1024 * 1024)
                    .ok_or_else(|| {
                        RepositorySessionError::NativeWorkspace(
                            "physical observation path limit".into(),
                        )
                    })?;
                if path.as_os_str().len() > 4096
                    || !path.starts_with(row.root.path())
                    || !matches!(
                        demand.operation(),
                        PathObservationOperation::Lstat
                            | PathObservationOperation::ReadLink
                            | PathObservationOperation::DirectoryEntries
                            | PathObservationOperation::FileBytes
                            | PathObservationOperation::FileDigest
                    )
                    || !seen.insert(demand.clone())
                {
                    return Err(RepositorySessionError::NativeWorkspace(
                        "physical demand refused before IO".into(),
                    ));
                }
                batch.push(demand);
            } else {
                legacy.push(demand);
            }
        }
        physical.retain(|(_, batch)| !batch.is_empty());
        for (row, batch) in &physical {
            let RetainedRootOwner::Physical(root) = &row.root else {
                unreachable!()
            };
            crate::runtime::path_observation::rooted::validate_rooted_demands(root.anchor(), batch)
                .map_err(|_| {
                    RepositorySessionError::NativeWorkspace(
                        "physical batch refused before IO".into(),
                    )
                })?;
            let depth = if root.state().mode() == slug_bzlmod_v2::RepositoryWorkspaceMode::Process {
                64
            } else {
                2048
            };
            for demand in batch {
                let relative = demand
                    .path()
                    .as_path()
                    .strip_prefix(root.anchor().root().as_path())
                    .map_err(|_| {
                        RepositorySessionError::NativeWorkspace(
                            "physical path escapes anchor".into(),
                        )
                    })?;
                if relative.components().count() > depth {
                    return Err(RepositorySessionError::NativeWorkspace(
                        "physical path exceeds root bounds".into(),
                    ));
                }
            }
        }
        Ok(PhysicalObservationBatch {
            legacy,
            physical,
            _credit: Some(credit),
        })
    }
}
