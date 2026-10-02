//! Normal selected Label lineage and bounded lexical path handles.
use slug_bzlmod_v2::HostRepositoryLabelPathSource;
use slug_bzlmod_v2::SelectedLabelSourceError;
use slug_bzlmod_v2::SelectedLabelSourcePolicy;
use slug_bzlmod_v2::SelectedLabelSourceRecipe;
use slug_workspace_v2::BudgetedImageBlock;
use slug_workspace_v2::ImageChargedPath;
use slug_workspace_v2::ImageDescriptorBudget;
use slug_workspace_v2::ImageDescriptorReservation;

use super::*;

async fn selected_path_source(
    ctx: &mut DiceComputations<'_>,
    key: &HostSelectedExtensionOwnerPureKey,
    inputs: &Arc<HostSelectedExtensionOwnerInputs>,
    first: &Arc<OwnerProjection>,
    calls: &Arc<[RepositoryRuleCallRecord]>,
    label: &CanonicalLabel,
    mut observations: PathObservationEpoch,
) -> Result<(HostRepositoryLabelPathSource, PathObservationEpoch), OwnerPureDriver> {
    macro_rules! merge {
        ($incoming:expr) => {
            observations = union_pure_observations(&observations, $incoming).map_err(|error| {
                SourcePreparationOutcome::Complete(Err(
                    HostSelectedExtensionOwnerPureObservationError::Merge {
                        inputs: inputs.clone(),
                        first: Some(first.clone()),
                        error,
                    },
                ))
            })?;
        };
    }
    macro_rules! fail {
        ($message:expr) => {
            return Err(owner_complete(
                Err(owner_after(inputs, calls.clone(), $message)),
                observations,
            ))
        };
    }
    let repo = label.package().repo();
    let route = if repo.is_root() {
        None
    } else {
        // Resolve ownership without evaluating the producer. Self reads fail
        // here; independent owners use the guarded, completed load route.
        let demand = match ctx
            .compute(&HostSelectedExtensionDemandObservationKey::new(
                key.workspace.dupe(),
                repo.clone(),
            ))
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return Err(SourcePreparationOutcome::Need(need));
            }
            Ok(SourcePreparationOutcome::Complete(Err(error))) => {
                return Err(SourcePreparationOutcome::Complete(Err(
                    HostSelectedExtensionOwnerPureObservationError::Demand {
                        inputs: inputs.clone(),
                        first: first.clone(),
                        error,
                    },
                )));
            }
            Ok(SourcePreparationOutcome::Complete(Ok(observed))) => observed,
            Err(error) => fail!(format!(
                "module_ctx.path generated-owner preflight failed: {error}"
            )),
        };
        merge!(demand.observations());
        let generated_owner = match demand.result().as_ref() {
            Ok(demand) if demand.owner() == &key.owner => {
                fail!(generated_repository_label_refusal(
                    label,
                    "generated-owner-preflight"
                ));
            }
            Ok(demand) => {
                // Owner inequality is only a precondition. The actual shared
                // DICE graph must cover all waits before admitting this path.
                match ctx.cycle_guard::<DependencyCycleGuard>() {
                    Ok(Some(_)) => {}
                    Ok(None) => fail!(format!(
                        "module_ctx.path generated Label requires the dependency cycle detector (label={label})"
                    )),
                    Err(error) => fail!(format!("module_ctx.path cycle guard failed: {error}")),
                }
                Some(demand.owner().clone())
            }
            Err(error)
                if error.disposition() == HostSelectedExtensionDemandErrorDisposition::Missing =>
            {
                None
            }
            Err(error) => fail!(format!(
                "module_ctx.path generated-owner preflight failed: {error}"
            )),
        };
        let route = match compute_guarded(
            ctx,
            &HostCanonicalRepositoryLoadRouteObservationKey::new(
                key.workspace.dupe(),
                repo.clone(),
            ),
        )
        .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return Err(SourcePreparationOutcome::Need(need));
            }
            Ok(SourcePreparationOutcome::Complete(Err(error))) => {
                return Err(SourcePreparationOutcome::Complete(Err(
                    HostSelectedExtensionOwnerPureObservationError::CanonicalRoute {
                        inputs: inputs.clone(),
                        first: first.clone(),
                        error: Arc::new(error),
                    },
                )));
            }
            Ok(SourcePreparationOutcome::Complete(Ok(observed))) => observed,
            Err(error) => fail!(format!("module_ctx.path canonical route failed: {error}")),
        };
        merge!(route.observations());
        let route = match route.result().as_ref() {
            Ok(value) => value,
            Err(error) => fail!(format!("module_ctx.path canonical route failed: {error:?}")),
        };
        match (
            generated_owner.as_ref(),
            route.route().view().generated_effect_seed(),
        ) {
            (Some(owner), Some(seed)) if seed.owner() == owner => {}
            (None, None) => {}
            _ => fail!(generated_repository_label_refusal(
                label,
                "generated-effect-route"
            )),
        }
        Some(HostRepositorySourceRoute::canonical(route.input().clone()))
    };
    let source = match route {
        None => HostRepositoryLabelPathSource::Root(key.workspace.dupe()),
        Some(route) => HostRepositoryLabelPathSource::External(route),
    };
    Ok((source, observations))
}

pub(super) async fn observe_selected_path_label(
    ctx: &mut DiceComputations<'_>,
    key: &HostSelectedExtensionOwnerPureKey,
    inputs: &Arc<HostSelectedExtensionOwnerInputs>,
    first: &Arc<OwnerProjection>,
    calls: &Arc<[RepositoryRuleCallRecord]>,
    label: &CanonicalLabel,
    mut observations: PathObservationEpoch,
) -> Result<(PreparedSelectedLabelPath, PathObservationEpoch), OwnerPureDriver> {
    macro_rules! merge {
        ($incoming:expr) => {
            observations = union_pure_observations(&observations, $incoming).map_err(|error| {
                SourcePreparationOutcome::Complete(Err(
                    HostSelectedExtensionOwnerPureObservationError::Merge {
                        inputs: inputs.clone(),
                        first: Some(first.clone()),
                        error,
                    },
                ))
            })?;
        };
    }
    macro_rules! fail {
        ($message:expr) => {
            return Err(owner_complete(
                Err(owner_after(inputs, calls.clone(), $message)),
                observations,
            ))
        };
    }
    let (source, next_observations) =
        selected_path_source(ctx, key, inputs, first, calls, label, observations).await?;
    observations = next_observations;
    let address = RepositoryLabelPathAddress::from_label(label);
    let path_key = match &source {
        HostRepositoryLabelPathSource::Root(workspace) => {
            HostRepositoryLabelPathObservationKey::new_root(workspace.dupe(), address)
        }
        HostRepositoryLabelPathSource::External(route) => {
            HostRepositoryLabelPathObservationKey::new_external(route.clone(), address)
        }
    };
    let path_key = match path_key {
        Ok(value) => value,
        Err(error) => fail!(format!("module_ctx.path Label is unsupported: {error:?}")),
    };
    let observed = match ctx.compute(&path_key).await {
        Ok(SourcePreparationOutcome::Need(need)) => {
            return Err(SourcePreparationOutcome::Need(need));
        }
        Ok(SourcePreparationOutcome::Complete(Err(error))) => {
            return Err(SourcePreparationOutcome::Complete(Err(
                HostSelectedExtensionOwnerPureObservationError::Merge {
                    inputs: inputs.clone(),
                    first: Some(first.clone()),
                    error,
                },
            )));
        }
        Ok(SourcePreparationOutcome::Complete(Ok(observed))) => observed,
        Err(error) => fail!(format!("module_ctx.path Label lookup failed: {error}")),
    };
    merge!(observed.observations());
    let path = match observed.result().as_ref() {
        Ok(value) => value.clone(),
        Err(error) => fail!(format!("module_ctx.path Label lookup failed: {error:?}")),
    };
    let prepared = ctx
        .per_transaction_data()
        .data
        .get::<ImageDescriptorBudget>()
        .map_err(|_| SelectedLabelSourceError::MissingBudget)
        .and_then(|budget| {
            PreparedSelectedLabelPath::new(&key.workspace, label, source, &path, budget)
        });
    match prepared {
        Ok(path) => Ok((path, observations)),
        Err(error) => Err(owner_complete(
            Err(source_owner_after(inputs, calls.clone(), error)),
            observations,
        )),
    }
}

/// The normal Label producer owns this constructor. Lexical values are requests,
/// never certificates of file contents or current native IO authority.
#[derive(Debug, Clone, Allocative, Dupe)]
pub(super) struct PreparedSelectedLabelPath {
    path: ImageChargedPath,
    namespace: PathObservationNamespace,
    recipe: SelectedLabelSourceRecipe,
    budget: ImageDescriptorBudget,
}

impl PreparedSelectedLabelPath {
    fn new(
        workspace: &NormalizedAbsolutePath,
        label: &CanonicalLabel,
        source: HostRepositoryLabelPathSource,
        value: &HostRepositoryLabelPathValue,
        budget: &ImageDescriptorBudget,
    ) -> Result<Self, SelectedLabelSourceError> {
        let workspace = ImageChargedPath::pin_existing(workspace.dupe(), budget)
            .map_err(SelectedLabelSourceError::Budget)?;
        let recipe = SelectedLabelSourceRecipe::new(
            workspace,
            label,
            source,
            SelectedLabelSourcePolicy::plain(),
            budget,
        )?;
        if value.path_str().len() > recipe.policy().limits().max_path_bytes {
            return Err(SelectedLabelSourceError::LimitExceeded(
                slug_bzlmod_v2::SelectedLabelSourceLimit::PathBytes,
            ));
        }
        let path = ImageChargedPath::pin_existing(value.path().dupe(), budget)
            .map_err(SelectedLabelSourceError::Budget)?;
        Ok(Self {
            path,
            namespace: value.namespace(),
            recipe,
            budget: budget.dupe(),
        })
    }

    fn derive(&self, child: Option<&str>) -> Result<Self, SelectedLabelSourceError> {
        // Validate and reserve the ordered transform before allocating a path.
        let recipe = match child {
            Some(child) => self.recipe.child(child, &self.budget)?,
            None => self.recipe.parent(&self.budget)?,
        };
        let parent = match child {
            Some(_) => self.path.as_path(),
            None => self
                .path
                .as_path()
                .parent()
                .ok_or(SelectedLabelSourceError::InvalidPath)?,
        };
        let length = parent
            .as_os_str()
            .len()
            .checked_add(child.map_or(0, |s| s.len() + 1))
            .ok_or(SelectedLabelSourceError::Budget(
                slug_workspace_v2::ImageDescriptorBudgetError::ChargeOverflow,
            ))?;
        if length > recipe.policy().limits().max_path_bytes {
            return Err(SelectedLabelSourceError::LimitExceeded(
                slug_bzlmod_v2::SelectedLabelSourceLimit::PathBytes,
            ));
        }
        let bytes = length
            .checked_mul(5)
            .and_then(|n| n.checked_add(128))
            .ok_or(SelectedLabelSourceError::Budget(
                slug_workspace_v2::ImageDescriptorBudgetError::ChargeOverflow,
            ))?;
        // Cover input, normalization replacement and headers before allocation.
        let _scratch = self
            .budget
            .try_reserve(bytes)
            .map_err(SelectedLabelSourceError::Budget)?;
        let mut path = PathBuf::with_capacity(length);
        path.push(parent);
        if let Some(child) = child {
            path.push(child);
        }
        let path =
            NormalizedAbsolutePath::new(path).map_err(|_| SelectedLabelSourceError::InvalidPath)?;
        let path = ImageChargedPath::pin_existing(path, &self.budget)
            .map_err(SelectedLabelSourceError::Budget)?;
        Ok(Self {
            path,
            namespace: self.namespace,
            recipe,
            budget: self.budget.dupe(),
        })
    }
}

/// At most eight prepared handles. Each append owns a credited immutable vector;
/// the origin Label is borrowed from the shared recipe rather than cloned here.
#[derive(Debug, Clone, Default, Allocative, Dupe)]
pub(super) struct SelectedPathReady {
    paths: Option<BudgetedImageBlock<Vec<PreparedSelectedLabelPath>>>,
    budget: Option<ImageDescriptorBudget>,
}

impl SelectedPathReady {
    fn paths(&self) -> &[PreparedSelectedLabelPath] {
        self.paths
            .as_ref()
            .map_or(&[], |paths| paths.get().as_slice())
    }

    pub(super) fn append(
        &self,
        path: PreparedSelectedLabelPath,
    ) -> Result<Self, SelectedLabelSourceError> {
        if self.paths().len() >= MAX_SELECTED_PATH_LABELS {
            return Err(SelectedLabelSourceError::LimitExceeded(
                slug_bzlmod_v2::SelectedLabelSourceLimit::Transforms,
            ));
        }
        if self
            .budget
            .as_ref()
            .is_some_and(|budget| !budget.same_instance(&path.budget))
        {
            return Err(SelectedLabelSourceError::Budget(
                slug_workspace_v2::ImageDescriptorBudgetError::ForeignBudget,
            ));
        }
        let budget = path.budget.dupe();
        let length = self.paths().len() + 1;
        let mut reservation = budget
            .try_reserve(length * std::mem::size_of::<PreparedSelectedLabelPath>())
            .map_err(SelectedLabelSourceError::Budget)?;
        let mut paths = Vec::with_capacity(length);
        if paths.capacity() > length {
            reservation
                .try_grow(
                    (paths.capacity() - length) * std::mem::size_of::<PreparedSelectedLabelPath>(),
                )
                .map_err(SelectedLabelSourceError::Budget)?;
        }
        paths.extend(self.paths().iter().map(Dupe::dupe));
        paths.push(path);
        let paths = reservation
            .freeze(paths)
            .map_err(SelectedLabelSourceError::Budget)?;
        Ok(Self {
            paths: Some(paths),
            budget: Some(budget),
        })
    }
}

#[derive(Debug, Allocative)]
pub(super) struct SelectedPathInvocation {
    ready: SelectedPathReady,
    pending: Mutex<Option<CanonicalLabel>>,
    operation_error: Mutex<Option<SelectedLabelSourceError>>,
    // Drop the new storage before releasing its reservation.
    _reservation: Option<ImageDescriptorReservation>,
}

impl SelectedPathInvocation {
    pub(super) fn new(ready: SelectedPathReady) -> Result<Arc<Self>, SelectedLabelSourceError> {
        let reservation = ready
            .budget
            .as_ref()
            .map(|budget| {
                budget
                    .try_reserve(
                        std::mem::size_of::<Self>()
                            + 2 * std::mem::size_of::<usize>()
                            + std::mem::align_of::<Self>(),
                    )
                    .map_err(SelectedLabelSourceError::Budget)
            })
            .transpose()?;
        Ok(Arc::new(Self {
            ready,
            pending: Mutex::new(None),
            operation_error: Mutex::new(None),
            _reservation: reservation,
        }))
    }

    pub(super) fn call(&self, label: &CanonicalLabel) -> anyhow::Result<PreparedSelectedLabelPath> {
        if let Some(path) = self
            .ready
            .paths()
            .iter()
            .find(|path| path.recipe.label() == label)
        {
            return Ok(path.dupe());
        }
        if self.ready.paths().len() == MAX_SELECTED_PATH_LABELS {
            anyhow::bail!(
                "module_ctx.path supports at most {MAX_SELECTED_PATH_LABELS} distinct Labels"
            );
        }
        let mut pending = self
            .pending
            .lock()
            .map_err(|_| anyhow::anyhow!("selected module extension path scratch unavailable"))?;
        *pending = Some(label.clone());
        anyhow::bail!("selected module extension Label path requires preparation")
    }

    pub(super) fn pending(&self) -> Result<Option<CanonicalLabel>, CompactString> {
        self.pending
            .lock()
            .map(|pending| pending.clone())
            .map_err(|_| "selected module extension path scratch unavailable".into())
    }

    fn record_error(&self, error: SelectedLabelSourceError) {
        if let Ok(mut state) = self.operation_error.lock() {
            if state.is_none() {
                *state = Some(error);
            }
        }
    }

    pub(super) fn operation_error(&self) -> Option<SelectedLabelSourceError> {
        self.operation_error
            .lock()
            .map(|state| state.clone())
            .unwrap_or(Some(SelectedLabelSourceError::Compute))
    }
}

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
pub(super) struct SelectedModuleLabelPath(PreparedSelectedLabelPath, Arc<SelectedPathInvocation>);

impl SelectedModuleLabelPath {
    pub(super) fn new(path: PreparedSelectedLabelPath, owner: Arc<SelectedPathInvocation>) -> Self {
        Self(path, owner)
    }
}
impl fmt::Display for SelectedModuleLabelPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(
            self.0
                .path
                .as_path()
                .to_str()
                .expect("prepared Unicode Label path"),
        )
    }
}
starlark::starlark_simple_value!(SelectedModuleLabelPath);

#[starlark_value(type = "path")]
impl<'v> StarlarkValue<'v> for SelectedModuleLabelPath {
    fn get_attr(&self, name: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        if name != "dirname" {
            return None;
        }
        match self.0.derive(None) {
            Ok(path) => Some(heap.alloc_simple(Self(path, self.1.clone()))),
            Err(error) => {
                self.1.record_error(error);
                None
            }
        }
    }
    fn get_methods() -> Option<&'static Methods> {
        static METHODS: MethodsStatic = MethodsStatic::new();
        METHODS.methods(selected_module_label_path_methods)
    }
}

#[starlark_module]
fn selected_module_label_path_methods(builder: &mut MethodsBuilder) {
    fn get_child<'v>(
        this: Value<'v>,
        #[starlark(require = pos)] child: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        let this = SelectedModuleLabelPath::from_value(this)
            .ok_or_else(|| anyhow::anyhow!("invalid selected Label path receiver"))?;
        match this.0.derive(Some(child)) {
            Ok(path) => Ok(eval
                .heap()
                .alloc_simple(SelectedModuleLabelPath(path, this.1.clone()))),
            Err(error) => {
                let diagnostic = format!(
                    "module_ctx.path requires a normalized relative string or bounded source path: {error:?}"
                );
                this.1.record_error(error);
                anyhow::bail!(diagnostic)
            }
        }
    }
}

pub(super) fn source_owner_after(
    inputs: &Arc<HostSelectedExtensionOwnerInputs>,
    calls: Arc<[RepositoryRuleCallRecord]>,
    error: SelectedLabelSourceError,
) -> HostSelectedExtensionOwnerPureError {
    let message = format!("module_ctx.path source preparation failed: {error:?}");
    HostSelectedExtensionOwnerPureError::SourceOperational {
        error,
        prior: Arc::new(owner_after(inputs, calls, message)),
    }
}

impl HostSelectedExtensionOwnerPureError {
    pub(crate) fn is_transient(&self) -> bool {
        match self {
            Self::Factored { error, .. } => error.is_transient(),
            Self::SourceOperational { error, prior } => {
                error.is_transient() || prior.is_transient()
            }
            _ => false,
        }
    }
}

#[cfg(test)]
pub(super) fn test_ready(
    paths: Vec<(CanonicalLabel, HostRepositoryLabelPathValue)>,
    budget: &ImageDescriptorBudget,
) -> SelectedPathReady {
    let workspace = NormalizedAbsolutePath::new("/fixture").unwrap();
    let mut ready = SelectedPathReady::default();
    for (label, value) in paths {
        let prepared = PreparedSelectedLabelPath::new(
            &workspace,
            &label,
            HostRepositoryLabelPathSource::Root(workspace.dupe()),
            &value,
            budget,
        )
        .unwrap();
        ready = ready.append(prepared).unwrap();
    }
    ready
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ready(budget: &ImageDescriptorBudget) -> SelectedPathReady {
        test_ready(
            vec![(
                CanonicalLabel::parse("@@//:marker").unwrap(),
                HostRepositoryLabelPathValue::new(
                    NormalizedAbsolutePath::new("/fixture/marker").unwrap(),
                    PathObservationNamespace::Host,
                )
                .unwrap(),
            )],
            budget,
        )
    }

    #[test]
    fn derived_handles_preserve_origin_transcript_and_last_drop_credit() {
        let budget = ImageDescriptorBudget::new(64 * 1024 * 1024);
        let ready = ready(&budget);
        let marker = ready.paths()[0].dupe();
        let parent = marker.derive(None).unwrap();
        let tool = parent.derive(Some("bin/cargo")).unwrap();
        assert_eq!(marker.recipe.label(), tool.recipe.label());
        assert_eq!(marker.recipe.source(), tool.recipe.source());
        assert_eq!(
            tool.path.as_path(),
            std::path::Path::new("/fixture/bin/cargo")
        );
        assert_eq!(tool.recipe.transforms().len(), 2);
        assert_eq!(tool.recipe.transforms()[1].child(), Some("bin/cargo"));
        let repeated = tool.derive(None).unwrap().derive(Some("cargo")).unwrap();
        assert_eq!(repeated.path, tool.path);
        assert_ne!(repeated.recipe, tool.recipe);
        // Lexical scope escape is displayed; normal source consumption refuses it.
        let escaped = parent
            .derive(None)
            .unwrap()
            .derive(Some("outside"))
            .unwrap();
        assert_eq!(escaped.path.as_path(), std::path::Path::new("/outside"));
        let history = tool.dupe();
        let charged = budget.used_bytes();
        drop((ready, marker, parent, tool, repeated, escaped));
        assert!(budget.used_bytes() > 0);
        assert!(budget.used_bytes() < charged);
        drop(history);
        assert_eq!(budget.used_bytes(), 0);
    }

    #[test]
    fn swallowed_dirname_budget_failure_remains_sticky() {
        let budget = ImageDescriptorBudget::new(64 * 1024 * 1024);
        let ready = ready(&budget);
        let invocation = SelectedPathInvocation::new(ready).unwrap();
        let path = invocation
            .call(&CanonicalLabel::parse("@@//:marker").unwrap())
            .unwrap();
        let blocker = budget
            .try_reserve(budget.limit_bytes() - budget.used_bytes())
            .unwrap();
        let module = Module::new();
        let value = SelectedModuleLabelPath::new(path.dupe(), invocation.clone());
        assert!(value.get_attr("dirname", module.heap()).is_none());
        let error = invocation.operation_error().unwrap();
        assert!(error.is_transient());
        assert!(matches!(error, SelectedLabelSourceError::Budget(_)));
        drop(blocker);
        assert!(value.get_attr("dirname", module.heap()).is_some());
        // A successful later operation cannot clear the earlier operational refusal.
        assert!(invocation.operation_error().unwrap().is_transient());
        drop((value, path, invocation, module));
        assert_eq!(budget.used_bytes(), 0);
    }
}
