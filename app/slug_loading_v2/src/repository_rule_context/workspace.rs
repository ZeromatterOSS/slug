//! Pure prefix replay for the general original repository-rule route.
use slug_bzlmod_v2::RepositoryMaterializationEpochEntry;
use slug_bzlmod_v2::RepositoryMaterializationKind;
use slug_bzlmod_v2::RepositoryMaterializationResultEpoch;
use slug_bzlmod_v2::RepositoryWorkspaceArgument;
use slug_bzlmod_v2::RepositoryWorkspaceEnvironmentValue;
use slug_bzlmod_v2::RepositoryWorkspaceEpoch;
use slug_bzlmod_v2::RepositoryWorkspaceError;
use slug_bzlmod_v2::RepositoryWorkspaceExecute;
use slug_bzlmod_v2::RepositoryWorkspaceOperation;
use slug_bzlmod_v2::RepositoryWorkspaceOperationDemand;
use slug_bzlmod_v2::RepositoryWorkspaceReadView;
use slug_bzlmod_v2::RepositoryWorkspaceReadViewManifest;
use slug_bzlmod_v2::RepositoryWorkspaceRealization;
use slug_bzlmod_v2::RepositoryWorkspaceReservation;
use slug_bzlmod_v2::RepositoryWorkspaceReservationDemand;
use slug_bzlmod_v2::RepositoryWorkspaceResult;
use slug_bzlmod_v2::RepositoryWorkspaceState;
use slug_bzlmod_v2::RepositoryWorkspaceTranscript;

use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(super) struct WorkspaceInput {
    epoch: RepositoryWorkspaceEpoch,
    demand: RepositoryWorkspaceReservationDemand,
    selected: RepositoryMaterializationResultEpoch,
    pub(super) source_snapshots: Arc<slug_bzlmod_v2::RepositorySourceSnapshotEpoch>,
}
impl WorkspaceInput {
    pub(super) fn new(
        epoch: RepositoryWorkspaceEpoch,
        demand: RepositoryWorkspaceReservationDemand,
        selected: RepositoryMaterializationResultEpoch,
    ) -> Result<Self, RepositoryRuleInvocationError> {
        demand
            .find_in(&epoch)
            .map_err(RepositoryRuleInvocationError::Workspace)?
            .ok_or(RepositoryRuleInvocationError::WorkspaceReplay)?;
        Ok(Self {
            epoch,
            demand,
            selected,
            source_snapshots: Arc::new(slug_bzlmod_v2::RepositorySourceSnapshotEpoch::revoked()),
        })
    }
}
#[derive(Debug, Allocative)]
pub(super) struct WorkspaceReplay {
    input: WorkspaceInput,
    transcript: RepositoryWorkspaceTranscript,
    index: usize,
    state: Arc<RepositoryWorkspaceState>,
    declarations:
        SmallMap<slug_identity_v2::CanonicalRepoName, RepositoryMaterializationEpochEntry>,
    sources: SmallMap<
        slug_identity_v2::CanonicalRepoName,
        Arc<slug_bzlmod_v2::RepositorySourceSnapshotState>,
    >,
    #[allocative(skip)]
    pub(super) error: Option<RepositoryRuleInvocationError>,
}
impl WorkspaceReplay {
    pub(super) fn new(input: &WorkspaceInput) -> Self {
        let transcript = input
            .demand
            .find_in(&input.epoch)
            .expect("validated workspace input")
            .expect("validated reservation")
            .clone();
        Self {
            state: transcript.initial().clone(),
            transcript,
            input: input.clone(),
            index: 0,
            declarations: SmallMap::new(),
            sources: SmallMap::new(),
            error: None,
        }
    }
    pub(super) fn reservation(&self) -> &Arc<RepositoryWorkspaceReservation> {
        self.transcript.reservation()
    }
    pub(super) fn replay(
        &mut self,
        operation: RepositoryWorkspaceOperation,
    ) -> Result<RepositoryWorkspaceResult, RepositoryRuleInvocationError> {
        let request = Arc::new(
            self.input
                .epoch
                .request_at(self.reservation(), self.index, operation)
                .map_err(RepositoryRuleInvocationError::Workspace)?,
        );
        if request.predecessor() != &self.state {
            return Err(RepositoryRuleInvocationError::WorkspaceReplay);
        }
        let Some(record) = self.transcript.records().get(self.index) else {
            let demand = RepositoryWorkspaceOperationDemand::new_with_sources(
                request,
                self.declarations
                    .values()
                    .cloned()
                    .collect::<Vec<_>>()
                    .into(),
                self.sources.values().cloned().collect::<Vec<_>>().into(),
            )
            .map_err(RepositoryRuleInvocationError::Workspace)?;
            return Err(RepositoryRuleInvocationError::WorkspaceNeed(Arc::new(
                demand,
            )));
        };
        if record.request() != request.as_ref() {
            return Err(RepositoryRuleInvocationError::WorkspaceReplay);
        }
        self.state = record.next().clone();
        self.index += 1;
        Ok(record.result().clone())
    }
    pub(super) fn finish(
        &self,
    ) -> Result<RepositoryWorkspaceRealization, RepositoryRuleInvocationError> {
        if let Some(error) = &self.error {
            return Err(error.clone());
        }
        self.transcript
            .realize(self.index)
            .map_err(RepositoryRuleInvocationError::Workspace)
    }
    fn declare(
        &mut self,
        path: &HostRepositoryLabelPathValue,
        repository: Option<&slug_identity_v2::CanonicalRepoName>,
    ) -> Result<(), RepositoryRuleInvocationError> {
        let instance = match path.namespace() {
            PathObservationNamespace::Materialization(instance)
                if instance == self.reservation().instance() =>
            {
                return Ok(());
            }
            PathObservationNamespace::Materialization(instance) => Some(instance),
            PathObservationNamespace::Host => None,
        };
        let Some(repository) = repository else {
            return if instance.is_none() {
                Ok(())
            } else {
                Err(RepositoryRuleInvocationError::WorkspaceUnsupported)
            };
        };
        if let Some(source) = self.input.source_snapshots.states().and_then(|states| {
            states
                .iter()
                .find(|s| Some(s.reservation().instance()) == instance)
        }) {
            let reservation = source.reservation();
            if reservation.workspace() != self.reservation().workspace()
                || reservation.lease() != self.reservation().lease()
                || reservation.repository() != repository
                || !path
                    .path()
                    .as_path()
                    .starts_with(reservation.display_root().as_path())
            {
                return Err(RepositoryRuleInvocationError::WorkspaceReplay);
            }
            if self.declarations.contains_key(repository) {
                return Err(RepositoryRuleInvocationError::WorkspaceReplay);
            }
            if let Some(previous) = self.sources.get(repository) {
                if previous != source {
                    return Err(RepositoryRuleInvocationError::WorkspaceReplay);
                }
            } else {
                if self.declarations.len() + self.sources.len() == 128 {
                    return Err(RepositoryRuleInvocationError::ExecuteLimit);
                }
                self.sources.insert(repository.clone(), source.clone());
            }
            return Ok(());
        }
        if repository.is_root() {
            return Err(RepositoryRuleInvocationError::WorkspaceUnsupported);
        }
        let entry = self
            .input
            .selected
            .entry_for_repository(repository)
            .ok_or(RepositoryRuleInvocationError::WorkspaceReplay)?;
        if entry.request.id.workspace != *self.reservation().workspace() {
            return Err(RepositoryRuleInvocationError::WorkspaceReplay);
        }
        let valid = match &entry.result {
            slug_bzlmod_v2::RepositoryMaterializationResult::Success(
                slug_bzlmod_v2::RepositoryMaterializationSuccess::Immutable {
                    observation_instance,
                    ..
                },
            ) => instance == Some(*observation_instance),
            slug_bzlmod_v2::RepositoryMaterializationResult::Success(
                slug_bzlmod_v2::RepositoryMaterializationSuccess::Local,
            ) => instance.is_none(),
            _ => false,
        };
        if !valid {
            return Err(RepositoryRuleInvocationError::WorkspaceReplay);
        }
        if let Some(previous) = self.declarations.get(&entry.request.id.canonical_repo) {
            if previous != entry {
                return Err(RepositoryRuleInvocationError::WorkspaceReplay);
            }
        } else {
            if self.declarations.len() + self.sources.len() == 128 {
                return Err(RepositoryRuleInvocationError::ExecuteLimit);
            }
            self.declarations
                .insert(entry.request.id.canonical_repo.clone(), entry.clone());
        }
        Ok(())
    }
    fn manifest(
        &self,
    ) -> Result<RepositoryWorkspaceReadViewManifest, RepositoryRuleInvocationError> {
        let mut views = Vec::with_capacity(self.declarations.len() + self.sources.len());
        for source in self.sources.values() {
            views.push(
                RepositoryWorkspaceReadView::from_source(source.clone())
                    .map_err(RepositoryRuleInvocationError::Workspace)?,
            );
        }
        for entry in self.declarations.values() {
            let RepositoryMaterializationKind::GeneralWorkspace(realization) = &entry.request.kind
            else {
                return Err(RepositoryRuleInvocationError::WorkspaceUnsupported);
            };
            entry
                .request
                .validate_general_workspace_binding()
                .map_err(RepositoryRuleInvocationError::Workspace)?;
            views.push(
                RepositoryWorkspaceReadView::new(
                    entry.request.repo_spec.clone(),
                    realization.final_state().clone(),
                )
                .map_err(RepositoryRuleInvocationError::Workspace)?,
            );
        }
        RepositoryWorkspaceReadViewManifest::new(self.reservation(), views)
            .map_err(RepositoryRuleInvocationError::Workspace)
    }
}
pub(super) fn register(
    state: &RepositoryRuleInvocationState,
    path: &HostRepositoryLabelPathValue,
    repository: Option<&slug_identity_v2::CanonicalRepoName>,
) -> Result<(), RepositoryRuleInvocationError> {
    if let Some(replay) = &state.workspace {
        replay
            .lock()
            .expect("workspace replay poisoned")
            .declare(path, repository)?;
    }
    Ok(())
}
pub(super) fn replay(
    state: &RepositoryRuleInvocationState,
    operation: RepositoryWorkspaceOperation,
) -> anyhow::Result<RepositoryWorkspaceResult> {
    state
        .workspace
        .as_ref()
        .ok_or_else(|| state.fail(RepositoryRuleInvocationError::WorkspaceReplay))?
        .lock()
        .expect("workspace replay poisoned")
        .replay(operation)
        .map_err(|error| state.fail(error))
}
pub(super) fn path(
    state: &RepositoryRuleInvocationState,
    input: &str,
) -> Result<RepositoryStarlarkPath, RepositoryRuleInvocationError> {
    let replay = state
        .workspace
        .as_ref()
        .ok_or(RepositoryRuleInvocationError::WorkspaceReplay)?;
    let root = replay
        .lock()
        .expect("workspace replay poisoned")
        .reservation()
        .display_root()
        .clone();
    let absolute = if Path::new(input).is_absolute() {
        NormalizedAbsolutePath::new(input)
            .map_err(|_| RepositoryRuleInvocationError::PathArgument)?
    } else {
        NormalizedAbsolutePath::new(root.as_path().join(input))
            .map_err(|_| RepositoryRuleInvocationError::PathArgument)?
    };
    let relative = absolute
        .as_path()
        .strip_prefix(root.as_path())
        .map_err(|_| RepositoryRuleInvocationError::WorkspaceUnsupported)?;
    let relative = relative
        .to_str()
        .ok_or(RepositoryRuleInvocationError::PathArgument)?;
    if relative.len() + root.as_path().as_os_str().len() + 1 > 4096 {
        return Err(RepositoryRuleInvocationError::PathArgument);
    }
    let relative = CompactString::new(relative);
    Ok(RepositoryStarlarkPath::Workspace {
        absolute,
        relative,
        replay: replay.clone(),
    })
}
/// Own Labels resolve against the reserved root, without a terminal self-cycle.
pub(super) fn resolve_label(
    state: &RepositoryRuleInvocationState,
    address: &RepositoryLabelPathAddress,
) -> Result<HostRepositoryLabelPathValue, RepositoryRuleInvocationError> {
    let selected = state
        .workspace
        .as_ref()
        .ok_or(RepositoryRuleInvocationError::WorkspaceReplay)?
        .lock()
        .expect("workspace replay poisoned")
        .reservation()
        .clone();
    if address.repo() == selected.repository() {
        let absolute = NormalizedAbsolutePath::new(
            selected
                .display_root()
                .as_path()
                .join(address.package().as_str())
                .join(address.target().as_str()),
        )
        .map_err(|_| RepositoryRuleInvocationError::PathArgument)?;
        if absolute.as_path().as_os_str().len() > 4096 {
            return Err(RepositoryRuleInvocationError::PathArgument);
        }
        return HostRepositoryLabelPathValue::new(
            absolute,
            PathObservationNamespace::Materialization(selected.instance()),
        )
        .map_err(|_| RepositoryRuleInvocationError::PathArgument);
    }
    let path = state
        .prepared_paths
        .get(address)
        .ok_or_else(|| RepositoryRuleInvocationError::LabelPathNeed(address.clone()))?;
    register(state, path, Some(address.repo()))?;
    Ok(path.clone())
}
pub(super) fn own_relative(
    state: &RepositoryRuleInvocationState,
    value: Value<'_>,
) -> Result<CompactString, RepositoryRuleInvocationError> {
    if let Some(text) = value.unpack_str() {
        let RepositoryStarlarkPath::Workspace { relative, .. } = path(state, text)? else {
            unreachable!()
        };
        return Ok(relative);
    }
    if let Some(RepositoryStarlarkPath::Workspace {
        relative, replay, ..
    }) = RepositoryStarlarkPath::from_value(value)
    {
        if state
            .workspace
            .as_ref()
            .is_some_and(|current| Arc::ptr_eq(current, replay))
        {
            return Ok(relative.clone());
        }
    }
    if let Some(label) = StarlarkLabel::from_value(value) {
        let address = RepositoryLabelPathAddress::from_label(label.canonical());
        let resolved = resolve_label(state, &address)?;
        let instance = state
            .workspace
            .as_ref()
            .unwrap()
            .lock()
            .unwrap()
            .reservation()
            .instance();
        if resolved.namespace() != PathObservationNamespace::Materialization(instance) {
            return Err(RepositoryRuleInvocationError::WorkspaceUnsupported);
        }
        let own = path(state, resolved.path_str())?;
        if let RepositoryStarlarkPath::Workspace { relative, .. } = own {
            return Ok(relative);
        }
    }
    Err(RepositoryRuleInvocationError::PathArgument)
}
pub(super) fn read_bytes(
    state: &RepositoryRuleInvocationState,
    value: Value<'_>,
) -> anyhow::Result<(HostRepositoryLabelPathValue, Arc<[u8]>)> {
    let own = match own_relative(state, value) {
        Ok(relative) => Some(relative),
        Err(
            RepositoryRuleInvocationError::WorkspaceUnsupported
            | RepositoryRuleInvocationError::PathArgument,
        ) => None,
        Err(error) => return Err(state.fail(error)),
    };
    if let Some(relative) = own {
        let selected = state
            .workspace
            .as_ref()
            .unwrap()
            .lock()
            .unwrap()
            .reservation()
            .clone();
        let resolved = HostRepositoryLabelPathValue::new(
            NormalizedAbsolutePath::new(selected.display_root().as_path().join(relative.as_str()))
                .map_err(|_| state.fail(RepositoryRuleInvocationError::PathArgument))?,
            PathObservationNamespace::Materialization(selected.instance()),
        )
        .map_err(|_| state.fail(RepositoryRuleInvocationError::PathArgument))?;
        return match replay(state, RepositoryWorkspaceOperation::Read(relative))? {
            RepositoryWorkspaceResult::Read(body) => Ok((resolved, body)),
            _ => Err(state.fail(RepositoryRuleInvocationError::WorkspaceReplay)),
        };
    }
    let host = if let Some(text) = value.unpack_str() {
        Path::new(text)
            .is_absolute()
            .then(|| NormalizedAbsolutePath::new(text).ok())
            .flatten()
    } else {
        match RepositoryStarlarkPath::from_value(value) {
            Some(RepositoryStarlarkPath::Host { path, .. }) => Some(path.clone()),
            Some(RepositoryStarlarkPath::Which(path)) => Some(path.path().clone()),
            _ => None,
        }
    };
    if let Some(path) = host {
        if path.as_path().starts_with("/__slug_repositories__") {
            return Err(state.fail(RepositoryRuleInvocationError::WorkspaceUnsupported));
        }
        let bytes = state
            .prepared_host_reads
            .get(&path)
            .ok_or_else(|| state.fail(RepositoryRuleInvocationError::HostReadNeed(path.clone())))?;
        let resolved = HostRepositoryLabelPathValue::new(path, PathObservationNamespace::Host)
            .map_err(|_| state.fail(RepositoryRuleInvocationError::PathArgument))?;
        return Ok((resolved, bytes.clone()));
    }
    let address = if let Some(label) = StarlarkLabel::from_value(value) {
        RepositoryLabelPathAddress::from_label(label.canonical())
    } else if let Some(RepositoryStarlarkPath::Label { address, .. }) =
        RepositoryStarlarkPath::from_value(value)
    {
        address.clone()
    } else {
        return Err(state.fail(RepositoryRuleInvocationError::WorkspaceUnsupported));
    };
    let resolved = resolve_label(state, &address).map_err(|error| state.fail(error))?;
    let body = state
        .prepared_sources
        .get(&address)
        .ok_or_else(|| state.fail(RepositoryRuleInvocationError::RepositorySourceNeed(address)))?;
    Ok((resolved, body.clone()))
}
pub(super) fn execute<'v>(
    state: &RepositoryRuleInvocationState,
    context: &RepositoryRuleContext,
    arguments: Value<'v>,
    timeout: Value<'v>,
    environment: Value<'v>,
    quiet: bool,
    cwd: &str,
    heap: Heap<'v>,
) -> anyhow::Result<Value<'v>> {
    let values: Vec<_> = if let Some(list) = ListRef::from_value(arguments) {
        list.iter().collect()
    } else if let Some(tuple) = TupleRef::from_value(arguments) {
        tuple.iter().collect()
    } else {
        return Err(state.fail(RepositoryRuleInvocationError::ExecuteArgument));
    };
    if values.is_empty() {
        return Err(state.fail(RepositoryRuleInvocationError::ExecuteArgument));
    }
    if values.iter().any(|value| {
        value.unpack_str().is_none()
            && StarlarkLabel::from_value(*value).is_none()
            && RepositoryStarlarkPath::from_value(*value).is_none()
    }) {
        return Err(state.fail(RepositoryRuleInvocationError::ExecuteArgument));
    }
    let timeout = timeout
        .unpack_i32()
        .filter(|value| *value > 0)
        .ok_or_else(|| state.fail(RepositoryRuleInvocationError::ExecuteOptions))?;
    let environment = DictRef::from_value(environment)
        .ok_or_else(|| state.fail(RepositoryRuleInvocationError::ExecuteOptions))?;
    for (key, _) in environment.iter() {
        if key.unpack_str().is_none_or(|key| key.contains(['\0', '='])) {
            return Err(state.fail(RepositoryRuleInvocationError::ExecuteOptions));
        }
    }
    let mut overrides = SmallMap::new();
    for (key, value) in environment.iter() {
        let value = if value.is_none() {
            RepositoryWorkspaceEnvironmentValue::Remove
        } else {
            RepositoryWorkspaceEnvironmentValue::Override(
                value
                    .unpack_str()
                    .filter(|value| !value.contains('\0'))
                    .ok_or_else(|| state.fail(RepositoryRuleInvocationError::ExecuteOptions))?
                    .into(),
            )
        };
        overrides.insert(key.unpack_str().unwrap().into(), value);
    }
    let mut argv = Vec::with_capacity(values.len());
    for value in values {
        let argument = if let Some(text) = value.unpack_str() {
            RepositoryWorkspaceArgument::String(text.into())
        } else if let Some(label) = StarlarkLabel::from_value(value) {
            let address = RepositoryLabelPathAddress::from_label(label.canonical());
            let resolved = resolve_label(state, &address).map_err(|error| state.fail(error))?;
            RepositoryWorkspaceArgument::Label { address, resolved }
        } else if let Some(value) = RepositoryStarlarkPath::from_value(value) {
            let path = match value {
                RepositoryStarlarkPath::Label { address, path } => {
                    register(state, path, Some(address.repo()))
                        .map_err(|error| state.fail(error))?;
                    path.clone()
                }
                RepositoryStarlarkPath::Which(path) => path.clone(),
                RepositoryStarlarkPath::Host { path, .. } => {
                    HostRepositoryLabelPathValue::new(path.clone(), PathObservationNamespace::Host)
                        .map_err(|_| state.fail(RepositoryRuleInvocationError::ExecuteArgument))?
                }
                RepositoryStarlarkPath::Workspace {
                    absolute, replay, ..
                } => {
                    if !state
                        .workspace
                        .as_ref()
                        .is_some_and(|ours| Arc::ptr_eq(ours, replay))
                    {
                        return Err(state.fail(RepositoryRuleInvocationError::ExecuteArgument));
                    }
                    HostRepositoryLabelPathValue::new(
                        absolute.clone(),
                        PathObservationNamespace::Materialization(
                            replay.lock().unwrap().reservation().instance(),
                        ),
                    )
                    .map_err(|_| state.fail(RepositoryRuleInvocationError::ExecuteArgument))?
                }
                _ => return Err(state.fail(RepositoryRuleInvocationError::ExecuteArgument)),
            };
            RepositoryWorkspaceArgument::Path(path)
        } else {
            return Err(state.fail(RepositoryRuleInvocationError::ExecuteArgument));
        };
        if argument.text().contains('\0') {
            return Err(state.fail(RepositoryRuleInvocationError::ExecuteArgument));
        }
        argv.push(argument);
    }
    let RepositoryStarlarkPath::Workspace { absolute, .. } =
        path(state, cwd).map_err(|error| state.fail(error))?
    else {
        unreachable!()
    };
    let (reservation, manifest) = {
        let replay = state.workspace.as_ref().unwrap().lock().unwrap();
        (
            replay.reservation().clone(),
            replay.manifest().map_err(|error| state.fail(error))?,
        )
    };
    let cwd = HostRepositoryLabelPathValue::new(
        absolute,
        PathObservationNamespace::Materialization(reservation.instance()),
    )
    .map_err(|_| state.fail(RepositoryRuleInvocationError::ExecuteOptions))?;
    let operation = RepositoryWorkspaceExecute::new(
        argv.into(),
        context.snapshot.clone(),
        overrides,
        timeout,
        quiet,
        cwd,
        manifest,
    )
    .map_err(|error| state.fail(RepositoryRuleInvocationError::Workspace(error)))?;
    let operation_index = state.workspace.as_ref().unwrap().lock().unwrap().index;
    let result = match replay(state, RepositoryWorkspaceOperation::Execute(operation))? {
        RepositoryWorkspaceResult::Execute(result) => result,
        _ => return Err(state.fail(RepositoryRuleInvocationError::WorkspaceReplay)),
    };
    if !quiet {
        state.general_events.borrow_mut().push(
            slug_events_v2::EvaluationEvent::RepositoryExecuteOutput {
                command_lease: reservation.lease(),
                operation_index,
                stdout: result.stdout_arc().clone(),
                stderr: result.stderr_arc().clone(),
            },
        );
    }
    Ok(heap.alloc_simple(super::exec_result::ExecResult::new(result)))
}
