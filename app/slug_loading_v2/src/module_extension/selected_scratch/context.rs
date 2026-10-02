//! Private synchronous recorder. Its method table never changes ordinary Pure.
use slug_bzlmod_v2::SelectedLabelSourceRecipe;
use slug_bzlmod_v2::SelectedScratchOperandSpec;
use slug_bzlmod_v2::SelectedScratchPath;
use slug_bzlmod_v2::SelectedScratchPrefixBuilder;
use slug_bzlmod_v2::SelectedScratchProcessSpec;
use slug_bzlmod_v2::SelectedScratchUnsupported;

use super::*;
use crate::module_extension::selected_path::PreparedSelectedLabelPath;

pub(super) struct DiscardPrint;
impl PrintHandler for DiscardPrint {
    fn println(&self, _: PrintLocation, _: &str) -> starlark::Result<()> {
        Ok(())
    }
}
#[derive(Debug, Allocative)]
pub(super) struct IssuedLabel {
    pub(super) event: SelectedScratchExternalPath,
    pub(super) recipe: SelectedLabelSourceRecipe,
    pub(super) spelling: ImageChargedPath,
}
pub(super) struct RecordedCheckpoint {
    pub(super) prefix: SelectedScratchPrefix,
    pub(super) labels: BudgetedImageBlock<Vec<IssuedLabel>>,
}
#[derive(Debug, Allocative)]
struct RecorderState {
    builder: Option<SelectedScratchPrefixBuilder>,
    prefix: Option<SelectedScratchPrefix>,
    labels: Vec<IssuedLabel>,
    credit: Option<ImageDescriptorReservation>,
    poison: Option<SelectedScratchPrefixError>,
    boundary: bool,
}
#[derive(Debug, Allocative)]
pub(super) struct CheckpointRecorder {
    state: Mutex<RecorderState>,
    inherited: RepositoryEnvironmentSnapshot,
    policy: SelectedScratchPolicy,
    // Recorder/environment allocations outlive a transferred label block.
    _fixed_credit: ImageDescriptorReservation,
}
impl CheckpointRecorder {
    pub(super) fn new(
        key: &HostSelectedExtensionScratchPrefixKey,
        resources: &SelectedScratchResources,
        inherited: &RepositoryEnvironmentSnapshot,
    ) -> Result<Arc<Self>, SelectedScratchPrefixError> {
        let env_bytes = inherited
            .iter()
            .try_fold(
                std::mem::size_of::<Self>() + 2 * std::mem::size_of::<usize>(),
                |n, e| {
                    n.checked_add(e.name().len())?
                        .checked_add(e.value().len())?
                        .checked_add(std::mem::size_of_val(e))
                },
            )
            .ok_or_else(SelectedScratchPrefixError::operational)?;
        let fixed_credit = resources
            .descriptor_budget()
            .try_reserve(env_bytes)
            .map_err(|_| SelectedScratchPrefixError::operational())?;
        let credit = resources
            .descriptor_budget()
            .try_reserve(0)
            .map_err(|_| SelectedScratchPrefixError::operational())?;
        let workspace =
            ImageChargedPath::pin_existing(key.workspace.dupe(), resources.descriptor_budget())
                .map_err(|_| SelectedScratchPrefixError::operational())?;
        let builder = SelectedScratchPrefixBuilder::new(
            workspace,
            key.owner.clone(),
            key.policy.clone(),
            resources,
        )?;
        Ok(Arc::new(Self {
            state: Mutex::new(RecorderState {
                builder: Some(builder),
                prefix: None,
                labels: Vec::new(),
                credit: Some(credit),
                poison: None,
                boundary: false,
            }),
            inherited: inherited.clone(),
            policy: key.policy.clone(),
            _fixed_credit: fixed_credit,
        }))
    }
    fn poison(&self, error: SelectedScratchPrefixError) {
        if let Ok(mut state) = self.state.lock() {
            if state.poison.is_none() {
                state.poison = Some(error);
            }
        }
    }
    fn unsupported(&self) -> anyhow::Error {
        self.poison(SelectedScratchPrefixError::unsupported());
        anyhow::anyhow!("unsupported private source preparation operation")
    }
    fn with_builder<T>(
        &self,
        f: impl FnOnce(&mut SelectedScratchPrefixBuilder) -> Result<T, SelectedScratchError>,
    ) -> anyhow::Result<T> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("private preparation recorder unavailable"))?;
        if state.poison.is_some() || state.boundary {
            anyhow::bail!("private preparation recorder refused");
        }
        let builder = state
            .builder
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("private preparation recorder unavailable"))?;
        match f(builder) {
            Ok(value) => Ok(value),
            Err(error) => {
                state.poison = Some(error.into());
                anyhow::bail!("private preparation operation refused")
            }
        }
    }
    fn label(
        &self,
        prepared: &PreparedSelectedLabelPath,
    ) -> anyhow::Result<SelectedScratchExternalPath> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("private preparation recorder unavailable"))?;
        if state.poison.is_some() || state.boundary {
            anyhow::bail!("private preparation recorder refused");
        }
        let result = (|| -> Result<SelectedScratchExternalPath, SelectedScratchPrefixError> {
            let event = state
                .builder
                .as_mut()
                .ok_or_else(SelectedScratchPrefixError::operational)?
                .record_external_path(
                    prepared
                        .path()
                        .as_path()
                        .to_str()
                        .ok_or_else(SelectedScratchPrefixError::unsupported)?,
                )?;
            if state.labels.len() == state.labels.capacity() {
                let capacity = state
                    .labels
                    .capacity()
                    .max(1)
                    .checked_mul(2)
                    .ok_or_else(SelectedScratchPrefixError::operational)?;
                let bytes = capacity
                    .checked_add(state.labels.capacity())
                    .and_then(|n| n.checked_mul(std::mem::size_of::<IssuedLabel>()))
                    .ok_or_else(SelectedScratchPrefixError::operational)?;
                state
                    .credit
                    .as_mut()
                    .ok_or_else(SelectedScratchPrefixError::operational)?
                    .try_grow(bytes)
                    .map_err(|_| SelectedScratchPrefixError::operational())?;
                let additional = capacity - state.labels.len();
                state
                    .labels
                    .try_reserve_exact(additional)
                    .map_err(|_| SelectedScratchPrefixError::operational())?;
            }
            state.labels.push(IssuedLabel {
                event: event.dupe(),
                recipe: prepared.recipe().dupe(),
                spelling: prepared.path().dupe(),
            });
            Ok(event)
        })();
        match result {
            Ok(event) => Ok(event),
            Err(error) => {
                state.poison = Some(error);
                anyhow::bail!("private Label source recording refused")
            }
        }
    }
    pub(super) fn take(
        &self,
        failed: bool,
    ) -> Result<RecordedCheckpoint, SelectedScratchPrefixError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| SelectedScratchPrefixError::operational())?;
        if let Some(error) = &state.poison {
            return Err(error.clone());
        }
        if !failed || !state.boundary {
            return Err(SelectedScratchPrefixError::unsupported());
        }
        let prefix = state
            .prefix
            .take()
            .ok_or_else(SelectedScratchPrefixError::unsupported)?;
        let labels = std::mem::take(&mut state.labels);
        let credit = state
            .credit
            .take()
            .ok_or_else(SelectedScratchPrefixError::operational)?;
        let labels = credit
            .freeze(labels)
            .map_err(|_| SelectedScratchPrefixError::operational())?;
        Ok(RecordedCheckpoint { prefix, labels })
    }
}

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub(in crate::module_extension) struct CheckpointContext {
    pub(crate) base: InvocationContext,
    recorder: Arc<CheckpointRecorder>,
}
impl CheckpointContext {
    pub(super) fn new(base: InvocationContext, recorder: Arc<CheckpointRecorder>) -> Self {
        Self { base, recorder }
    }
    pub(crate) fn path<'v>(
        &self,
        path: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        if let Some(literal) = path.unpack_str() {
            let path = self
                .recorder
                .with_builder(|builder| builder.path(literal))?;
            return Ok(eval.heap().alloc_simple(ScratchPathValue {
                path,
                recorder: self.recorder.clone(),
            }));
        }
        if let Some(path) = ScratchPathValue::from_value(path) {
            if !Arc::ptr_eq(&path.recorder, &self.recorder) {
                return Err(self.recorder.unsupported());
            }
            return Ok(eval.heap().alloc_simple(ScratchPathValue {
                path: path.path.dupe(),
                recorder: self.recorder.clone(),
            }));
        }
        let label = StarlarkLabel::from_value(path).ok_or_else(|| self.recorder.unsupported())?;
        let selected = self
            .base
            .selected_path
            .as_ref()
            .ok_or_else(|| self.recorder.unsupported())?;
        let prepared = selected.call(label.canonical())?;
        let event = self.recorder.label(&prepared)?;
        Ok(eval.heap().alloc_simple(LabelPathValue {
            prepared,
            event,
            recorder: self.recorder.clone(),
        }))
    }
    fn output(&self, path: Value) -> anyhow::Result<SelectedScratchPath> {
        if let Some(path) = ScratchPathValue::from_value(path) {
            if Arc::ptr_eq(&path.recorder, &self.recorder) {
                return Ok(path.path.dupe());
            }
        }
        if let Some(path) = path.unpack_str() {
            return self.recorder.with_builder(|builder| builder.path(path));
        }
        Err(self.recorder.unsupported())
    }
    pub(crate) fn download<'v>(
        &self,
        url: &str,
        output: Value<'v>,
        sha256: &str,
        executable: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        let path = self.output(output)?;
        let download = self
            .base
            .selected_download
            .as_ref()
            .ok_or_else(|| self.recorder.unsupported())?;
        let relative = selected_scratch_relative(path.relative())?;
        let request = Arc::new(SelectedExtensionDownloadRequest::new(
            download.workspace.dupe(),
            download.selected_owner.clone(),
            relative,
            url,
            selected_download_sha256(sha256)?,
            executable,
        )?);
        if let Err(error) = download.call(request.clone()) {
            if download.need().is_ok_and(|need| need.is_none()) {
                self.recorder
                    .poison(SelectedScratchPrefixError::unsupported());
            }
            return Err(error);
        }
        let verified = download
            .ready
            .as_ref()
            .ok_or_else(|| self.recorder.unsupported())?;
        self.recorder
            .with_builder(|builder| builder.download(&path, verified))?;
        let heap = eval.heap();
        Ok(heap.alloc(AllocStruct([
            ("success", heap.alloc(true)),
            ("sha256", heap.alloc(request.sha256_hex())),
            ("integrity", heap.alloc(request.sha256_sri())),
        ])))
    }
}
impl fmt::Display for CheckpointContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<private_source_preparation_ctx>")
    }
}
starlark::starlark_simple_value!(CheckpointContext);
#[starlark_value(type = "private_source_preparation_ctx")]
impl<'v> StarlarkValue<'v> for CheckpointContext {
    fn get_attr(&self, name: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        let result = self.base.get_attr(name, heap);
        if result.is_none() {
            self.recorder
                .poison(SelectedScratchPrefixError::unsupported());
        }
        result
    }
    fn get_methods() -> Option<&'static Methods> {
        static METHODS: MethodsStatic = MethodsStatic::new();
        METHODS.methods(checkpoint_methods)
    }
}
fn checkpoint_methods(builder: &mut MethodsBuilder) {
    invocation_context_methods(builder);
    checkpoint_extra_methods(builder);
}

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct ScratchPathValue {
    path: SelectedScratchPath,
    recorder: Arc<CheckpointRecorder>,
}
impl fmt::Display for ScratchPathValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.path.guest_spelling())
    }
}
starlark::starlark_simple_value!(ScratchPathValue);
#[starlark_value(type = "private_scratch_path")]
impl<'v> StarlarkValue<'v> for ScratchPathValue {
    fn get_attr(&self, name: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        if name != "dirname" {
            self.recorder
                .poison(SelectedScratchPrefixError::unsupported());
            return None;
        }
        match self
            .recorder
            .with_builder(|builder| builder.parent(&self.path))
        {
            Ok(path) => Some(heap.alloc_simple(Self {
                path,
                recorder: self.recorder.clone(),
            })),
            Err(_) => None,
        }
    }
    fn get_methods() -> Option<&'static Methods> {
        static METHODS: MethodsStatic = MethodsStatic::new();
        METHODS.methods(scratch_path_methods)
    }
}
#[starlark_module]
fn scratch_path_methods(builder: &mut MethodsBuilder) {
    fn get_child<'v>(
        this: Value<'v>,
        #[starlark(require = pos)] child: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        let this = ScratchPathValue::from_value(this)
            .ok_or_else(|| anyhow::anyhow!("invalid private path receiver"))?;
        let path = this
            .recorder
            .with_builder(|builder| builder.child(&this.path, child))?;
        Ok(eval.heap().alloc_simple(ScratchPathValue {
            path,
            recorder: this.recorder.clone(),
        }))
    }
}
#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct LabelPathValue {
    prepared: PreparedSelectedLabelPath,
    event: SelectedScratchExternalPath,
    recorder: Arc<CheckpointRecorder>,
}
impl LabelPathValue {
    fn derive(&self, child: Option<&str>) -> anyhow::Result<Self> {
        let prepared = match self.prepared.derive(child) {
            Ok(value) => value,
            Err(error) => {
                self.recorder.poison(error.into());
                anyhow::bail!("private Label derivation refused")
            }
        };
        let event = self.recorder.label(&prepared)?;
        Ok(Self {
            prepared,
            event,
            recorder: self.recorder.clone(),
        })
    }
}
impl fmt::Display for LabelPathValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.prepared.path().as_path().display().fmt(f)
    }
}
starlark::starlark_simple_value!(LabelPathValue);
#[starlark_value(type = "private_label_path")]
impl<'v> StarlarkValue<'v> for LabelPathValue {
    fn get_attr(&self, name: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        if name != "dirname" {
            self.recorder
                .poison(SelectedScratchPrefixError::unsupported());
            return None;
        }
        self.derive(None).ok().map(|path| heap.alloc_simple(path))
    }
    fn get_methods() -> Option<&'static Methods> {
        static METHODS: MethodsStatic = MethodsStatic::new();
        METHODS.methods(label_path_methods)
    }
}
#[starlark_module]
fn label_path_methods(builder: &mut MethodsBuilder) {
    fn get_child<'v>(
        this: Value<'v>,
        #[starlark(require = pos)] child: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        let this = LabelPathValue::from_value(this)
            .ok_or_else(|| anyhow::anyhow!("invalid private Label receiver"))?;
        Ok(eval.heap().alloc_simple(this.derive(Some(child))?))
    }
}

#[starlark_module]
fn checkpoint_extra_methods(builder: &mut MethodsBuilder) {
    fn file<'v>(
        this: Value<'v>,
        #[starlark(require = pos)] path: Value<'v>,
        #[starlark(default = "")] content: &str,
        #[starlark(require = named, default = true)] executable: bool,
    ) -> anyhow::Result<Value<'v>> {
        let this = CheckpointContext::from_value(this)
            .ok_or_else(|| anyhow::anyhow!("invalid private context receiver"))?;
        let path = this.output(path)?;
        this.recorder
            .with_builder(|builder| builder.file(&path, content.as_bytes(), executable))?;
        Ok(Value::new_none())
    }
    fn read<'v>(
        this: Value<'v>,
        #[starlark(require = pos)] _path: Value<'v>,
    ) -> anyhow::Result<Value<'v>> {
        let this = CheckpointContext::from_value(this)
            .ok_or_else(|| anyhow::anyhow!("invalid private context receiver"))?;
        this.recorder.poison(
            SelectedScratchError::Unsupported(SelectedScratchUnsupported::ReadBeforeFirstProcess)
                .into(),
        );
        Err(this.recorder.unsupported())
    }
    fn execute<'v>(
        this: Value<'v>,
        #[starlark(require = pos)] arguments: Value<'v>,
        #[starlark(require = named, default = 600)] timeout: i64,
        #[starlark(require = named)] environment: Option<Value<'v>>,
        #[starlark(require = named, default = true)] quiet: bool,
        #[starlark(require = named, default = "")] working_directory: &str,
    ) -> anyhow::Result<Value<'v>> {
        let this = CheckpointContext::from_value(this)
            .ok_or_else(|| anyhow::anyhow!("invalid private context receiver"))?;
        let argument_values = if let Some(list) = ListRef::from_value(arguments) {
            list.content()
        } else if let Some(tuple) = TupleRef::from_value(arguments) {
            tuple.content()
        } else {
            return Err(this.recorder.unsupported());
        };
        let environment = environment.filter(|value| !value.is_none());
        let environment = environment
            .map(|value| DictRef::from_value(value).ok_or_else(|| this.recorder.unsupported()))
            .transpose()?;
        let mut state = this
            .recorder
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("private preparation recorder unavailable"))?;
        if state.boundary || state.poison.is_some() {
            anyhow::bail!("private preparation recorder refused");
        }
        let result = (|| -> Result<(), SelectedScratchPrefixError> {
            let limits = this.recorder.policy.limits();
            if argument_values.len() > limits.max_argv_entries
                || environment.as_ref().map_or(0, |e| e.len()) > limits.max_environment_entries
            {
                return Err(SelectedScratchPrefixError::unsupported());
            }
            let bytes = argument_values
                .len()
                .checked_mul(std::mem::size_of::<SelectedScratchOperandSpec<'_>>())
                .and_then(|n| {
                    n.checked_add(
                        environment.as_ref().map_or(0, |e| e.len())
                            * std::mem::size_of::<(&str, Option<&str>)>(),
                    )
                })
                .ok_or_else(SelectedScratchPrefixError::operational)?;
            let _temporary = state
                .credit
                .as_mut()
                .ok_or_else(SelectedScratchPrefixError::operational)?;
            _temporary
                .try_grow(bytes)
                .map_err(|_| SelectedScratchPrefixError::operational())?;
            let mut argv = Vec::new();
            argv.try_reserve_exact(argument_values.len())
                .map_err(|_| SelectedScratchPrefixError::operational())?;
            for value in argument_values {
                let operand = if let Some(literal) = value.unpack_str() {
                    SelectedScratchOperandSpec::Literal(literal)
                } else if let Some(path) = ScratchPathValue::from_value(*value) {
                    if !Arc::ptr_eq(&path.recorder, &this.recorder) {
                        return Err(SelectedScratchPrefixError::unsupported());
                    }
                    SelectedScratchOperandSpec::Scratch(&path.path)
                } else if let Some(path) = LabelPathValue::from_value(*value) {
                    if !Arc::ptr_eq(&path.recorder, &this.recorder) {
                        return Err(SelectedScratchPrefixError::unsupported());
                    }
                    SelectedScratchOperandSpec::ExternalPath(&path.event)
                } else {
                    return Err(SelectedScratchPrefixError::unsupported());
                };
                argv.push(operand);
            }
            let mut env = Vec::new();
            env.try_reserve_exact(environment.as_ref().map_or(0, |e| e.len()))
                .map_err(|_| SelectedScratchPrefixError::operational())?;
            if let Some(environment) = &environment {
                for (name, value) in environment.iter() {
                    let name = name
                        .unpack_str()
                        .ok_or_else(SelectedScratchPrefixError::unsupported)?;
                    let value = if value.is_none() {
                        None
                    } else {
                        Some(
                            value
                                .unpack_str()
                                .ok_or_else(SelectedScratchPrefixError::unsupported)?,
                        )
                    };
                    env.push((name, value));
                }
            }
            let mut builder = state
                .builder
                .take()
                .ok_or_else(SelectedScratchPrefixError::operational)?;
            let cwd_path;
            let cwd = if working_directory.is_empty() {
                "/__slug_module"
            } else if working_directory.starts_with("/__slug_module") {
                working_directory
            } else {
                cwd_path = builder.path(working_directory)?;
                cwd_path.guest_spelling()
            };
            let prefix = builder.finish(
                SelectedScratchProcessSpec {
                    arguments: &argv,
                    cwd,
                    environment: &env,
                    timeout_seconds: u64::try_from(timeout)
                        .map_err(|_| SelectedScratchPrefixError::unsupported())?,
                    quiet,
                },
                &this.recorder.inherited,
            )?;
            state.prefix = Some(prefix);
            state.boundary = true;
            Ok(())
        })();
        if let Err(error) = result {
            state.poison = Some(error);
            anyhow::bail!("private process checkpoint refused");
        }
        anyhow::bail!("private first process checkpoint boundary")
    }
}
