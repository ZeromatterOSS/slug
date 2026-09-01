/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the above-listed
 * licenses.
 */

//! Evaluator-local configured-analysis and subrule call ABI.
//!
//! Frozen subrule callables live in the loading crate, while their configured
//! values and sole action registry are prepared by analysis. This module is
//! the small synchronous seam between them; it owns no graph lookup or DICE
//! state, and every value here dies with the evaluator.

use std::fmt;
use std::hash::Hash;
use std::sync::Arc;
use std::sync::Mutex;

use allocative::Allocative;
use compact_str::CompactString;
use slug_build_api_v2::ActionOutput;
use slug_build_api_v2::AnalysisArtifact;
use slug_build_api_v2::AnalysisConfiguredTargetKey;
use slug_build_api_v2::RetainedParamFileFormat;
use slug_build_api_v2::RetainedScalarArg;
use slug_build_api_v2::RetainedScalarValue;
use slug_build_api_v2::RetainedVectorOptions;
use slug_identity_v2::CanonicalLabel;
use starlark::any::ProvidesStaticType;
use starlark::environment::Methods;
use starlark::environment::MethodsBuilder;
use starlark::environment::MethodsStatic;
use starlark::eval::Arguments;
use starlark::eval::Evaluator;
use starlark::starlark_complex_value;
use starlark::starlark_module;
use starlark::values::Coerce;
use starlark::values::Freeze;
use starlark::values::FreezeError;
use starlark::values::FreezeResult;
use starlark::values::Freezer;
use starlark::values::FrozenValue;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::StarlarkValue;
use starlark::values::Trace;
use starlark::values::UnpackValue;
use starlark::values::Value;
use starlark::values::ValueLike;
use starlark::values::dict::DictRef;
use starlark::values::list::ListRef;
use starlark::values::none::NoneType;
use starlark::values::starlark_value;
use starlark::values::tuple::TupleRef;
use starlark::values::typing::StarlarkCallable;
use starlark_map::StarlarkHasher;
use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;

use crate::BzlModuleIdentity;
use crate::analysis_fragments::SubruleFragmentCollection;
use crate::provider::alloc_starlark_label;
use crate::starlark_label::StarlarkLabel;
use crate::subrule::SubruleIdentity;

#[derive(Debug, Clone, Copy)]
pub struct AnalysisRunRequest<'v> {
    pub outputs: Value<'v>,
    pub executable: Value<'v>,
    pub arguments: Option<Value<'v>>,
    pub inputs: Option<Value<'v>>,
    pub tools: Option<Value<'v>>,
    pub env: Option<Value<'v>>,
    pub mnemonic: Option<&'v str>,
    pub progress_message: Option<&'v str>,
    pub use_default_shell_env: bool,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative)]
pub enum AnalysisActionCallScope {
    Root,
    Subrule(Arc<SubruleIdentity>),
}

#[derive(Debug, Clone, Copy)]
pub enum AnalysisSpawnInvocation<'v> {
    Executable(Value<'v>),
    Shell(Value<'v>),
}

#[derive(Debug, Clone)]
pub struct AnalysisSpawnRequest<'v> {
    pub scope: AnalysisActionCallScope,
    pub invocation: AnalysisSpawnInvocation<'v>,
    pub outputs: Value<'v>,
    pub inputs: Option<Value<'v>>,
    pub unused_inputs_list: Option<Value<'v>>,
    pub tools: Option<Value<'v>>,
    pub arguments: Option<Value<'v>>,
    pub mnemonic: Option<&'v str>,
    pub progress_message: Option<&'v str>,
    pub use_default_shell_env: bool,
    pub env: Option<Value<'v>>,
    pub execution_requirements: Option<Value<'v>>,
    pub exec_group: Option<&'v str>,
    pub shadowed_action: Option<Value<'v>>,
    pub has_resource_set: bool,
    pub toolchain: Option<Value<'v>>,
}

pub trait AnalysisActionSink: fmt::Debug + Send + Sync {
    fn declare_file(&self, path: &str) -> anyhow::Result<AnalysisArtifactValue>;
    fn write(
        &self,
        output: Value<'_>,
        content: Value<'_>,
        is_executable: bool,
    ) -> anyhow::Result<()>;
    fn run_shell(
        &self,
        outputs: Value<'_>,
        command: &str,
        arguments: Value<'_>,
    ) -> anyhow::Result<()>;
    fn run(&self, request: AnalysisRunRequest<'_>) -> anyhow::Result<()>;
    fn spawn(&self, _request: AnalysisSpawnRequest<'_>) -> anyhow::Result<()> {
        anyhow::bail!("typed Spawn actions are unavailable in this analysis context")
    }
    fn is_files_to_run_provider(&self, _value: Value<'_>) -> bool {
        false
    }
    fn artifact_symlink(
        &self,
        output: Value<'_>,
        target_file: Value<'_>,
        is_executable: bool,
        progress_message: Option<&str>,
    ) -> anyhow::Result<()>;
    fn absolute_symlink(
        &self,
        output: Value<'_>,
        target_path: &str,
        progress_message: Option<&str>,
    ) -> anyhow::Result<()>;
}

#[derive(Debug, Clone, Allocative, Trace)]
pub enum EvaluatorVectorSourceGen<V> {
    Sequence(Vec<V>),
    Depset(V),
}

#[derive(Debug, Clone, Allocative, Trace)]
pub struct EvaluatorVectorArgGen<V> {
    pub source: EvaluatorVectorSourceGen<V>,
    #[trace(unsafe_ignore)]
    pub options: RetainedVectorOptions,
}

#[derive(Debug, Clone, Allocative, Trace)]
pub enum EvaluatorArgCallGen<V> {
    Scalar(#[trace(unsafe_ignore)] RetainedScalarArg),
    AddAll(EvaluatorVectorArgGen<V>),
    AddJoined(EvaluatorVectorArgGen<V>),
}

#[derive(Debug, Clone, Allocative, Trace)]
struct StarlarkArgsStateGen<V> {
    calls: Vec<EvaluatorArgCallGen<V>>,
    #[trace(unsafe_ignore)]
    format: Option<RetainedParamFileFormat>,
    #[trace(unsafe_ignore)]
    param_file: Option<(CompactString, bool)>,
}

#[derive(Debug, Clone)]
pub struct EvaluatorArgsSnapshot<'v> {
    pub calls: Vec<EvaluatorArgCallGen<Value<'v>>>,
    pub format: RetainedParamFileFormat,
    pub param_file: Option<(CompactString, bool)>,
}

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative, Trace)]
#[repr(C)]
pub struct StarlarkArgsGen<V> {
    state: Arc<Mutex<StarlarkArgsStateGen<V>>>,
}

starlark_complex_value!(pub StarlarkArgs);

unsafe impl<'v> Coerce<StarlarkArgsGen<Value<'v>>> for StarlarkArgsGen<FrozenValue> {}

impl<'v> Freeze for StarlarkArgs<'v> {
    type Frozen = FrozenStarlarkArgs;

    fn freeze(self, _freezer: &Freezer) -> FreezeResult<Self::Frozen> {
        Err(FreezeError::new(
            "Args values are evaluator-local and cannot be frozen".to_owned(),
        ))
    }
}

impl<'v> StarlarkArgs<'v> {
    pub fn snapshot(value: Value<'v>) -> Option<EvaluatorArgsSnapshot<'v>> {
        Self::from_value(value).map(|args| {
            let state = args
                .state
                .lock()
                .expect("Args mutation lock is not poisoned");
            EvaluatorArgsSnapshot {
                calls: state.calls.clone(),
                format: state.format.unwrap_or(RetainedParamFileFormat::Shell),
                param_file: state.param_file.clone(),
            }
        })
    }
}

impl<V> fmt::Display for StarlarkArgsGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Args")
    }
}

#[starlark_value(type = "Args")]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for StarlarkArgsGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    type Canonical = FrozenStarlarkArgs;

    fn get_methods() -> Option<&'static Methods> {
        static METHODS: MethodsStatic = MethodsStatic::new();
        METHODS.methods(starlark_args_methods)
    }
}

#[starlark_module]
fn starlark_args_methods(builder: &mut MethodsBuilder) {
    fn add<'v>(
        this: Value<'v>,
        arg_name_or_value: Value<'v>,
        value: Option<Value<'v>>,
        #[starlark(require = named)] format: Option<&str>,
    ) -> anyhow::Result<Value<'v>> {
        let args = StarlarkArgs::from_value(this)
            .ok_or_else(|| anyhow::anyhow!("Args.add receiver is invalid"))?;
        let (arg_name, value) = match value {
            Some(value) => (
                Some(
                    arg_name_or_value
                        .unpack_str()
                        .ok_or_else(|| anyhow::anyhow!("Args.add arg name must be a string"))?,
                ),
                value,
            ),
            None => (None, arg_name_or_value),
        };
        let value = scalar_arg_value(value)?;
        if let Some(format) = format {
            validate_scalar_format(format)?;
        }
        args.state
            .lock()
            .map_err(|_| anyhow::anyhow!("Args mutation lock is poisoned"))?
            .calls
            .push(EvaluatorArgCallGen::Scalar(RetainedScalarArg::new(
                arg_name, value, format,
            )));
        Ok(this)
    }

    #[allow(clippy::too_many_arguments)]
    fn add_all<'v>(
        this: Value<'v>,
        arg_name_or_values: Value<'v>,
        values: Option<Value<'v>>,
        #[starlark(require = named)] map_each: Option<Value<'v>>,
        #[starlark(require = named)] format_each: Option<&str>,
        #[starlark(require = named)] before_each: Option<&str>,
        #[starlark(require = named, default = true)] omit_if_empty: bool,
        #[starlark(require = named, default = false)] uniquify: bool,
        #[starlark(require = named, default = true)] expand_directories: bool,
        #[starlark(require = named)] terminate_with: Option<&str>,
        #[starlark(require = named, default = false)] allow_closure: bool,
    ) -> anyhow::Result<Value<'v>> {
        let args = StarlarkArgs::from_value(this)
            .ok_or_else(|| anyhow::anyhow!("Args.add_all receiver is invalid"))?;
        let (arg_name, source) = vector_positionals(arg_name_or_values, values, "add_all")?;
        reject_callback_options(map_each, allow_closure, "add_all")?;
        if let Some(format) = format_each {
            validate_named_format("format_each", format)?;
        }
        if omit_if_empty && vector_source_is_empty(&source) {
            return Ok(this);
        }
        args.state
            .lock()
            .map_err(|_| anyhow::anyhow!("Args mutation lock is poisoned"))?
            .calls
            .push(EvaluatorArgCallGen::AddAll(EvaluatorVectorArgGen {
                source,
                options: RetainedVectorOptions {
                    arg_name: arg_name.map(Into::into),
                    format_each: format_each.map(Into::into),
                    before_each: before_each.map(Into::into),
                    join_with: None,
                    format_joined: None,
                    omit_if_empty,
                    uniquify,
                    expand_directories,
                    terminate_with: terminate_with.map(Into::into),
                },
            }));
        Ok(this)
    }

    #[allow(clippy::too_many_arguments)]
    fn add_joined<'v>(
        this: Value<'v>,
        arg_name_or_values: Value<'v>,
        values: Option<Value<'v>>,
        #[starlark(require = named)] join_with: &str,
        #[starlark(require = named)] map_each: Option<Value<'v>>,
        #[starlark(require = named)] format_each: Option<&str>,
        #[starlark(require = named)] format_joined: Option<&str>,
        #[starlark(require = named, default = true)] omit_if_empty: bool,
        #[starlark(require = named, default = false)] uniquify: bool,
        #[starlark(require = named, default = true)] expand_directories: bool,
        #[starlark(require = named, default = false)] allow_closure: bool,
    ) -> anyhow::Result<Value<'v>> {
        let args = StarlarkArgs::from_value(this)
            .ok_or_else(|| anyhow::anyhow!("Args.add_joined receiver is invalid"))?;
        let (arg_name, source) = vector_positionals(arg_name_or_values, values, "add_joined")?;
        reject_callback_options(map_each, allow_closure, "add_joined")?;
        if let Some(format) = format_each {
            validate_named_format("format_each", format)?;
        }
        if let Some(format) = format_joined {
            validate_named_format("format_joined", format)?;
        }
        if omit_if_empty && vector_source_is_empty(&source) {
            return Ok(this);
        }
        args.state
            .lock()
            .map_err(|_| anyhow::anyhow!("Args mutation lock is poisoned"))?
            .calls
            .push(EvaluatorArgCallGen::AddJoined(EvaluatorVectorArgGen {
                source,
                options: RetainedVectorOptions {
                    arg_name: arg_name.map(Into::into),
                    format_each: format_each.map(Into::into),
                    before_each: None,
                    join_with: Some(join_with.into()),
                    format_joined: format_joined.map(Into::into),
                    omit_if_empty,
                    uniquify,
                    expand_directories,
                    terminate_with: None,
                },
            }));
        Ok(this)
    }

    fn use_param_file<'v>(
        this: Value<'v>,
        param_file_arg: &str,
        #[starlark(require = named, default = false)] use_always: bool,
    ) -> anyhow::Result<Value<'v>> {
        let args = StarlarkArgs::from_value(this)
            .ok_or_else(|| anyhow::anyhow!("Args.use_param_file receiver is invalid"))?;
        validate_named_format("param_file_arg", param_file_arg)?;
        args.state
            .lock()
            .map_err(|_| anyhow::anyhow!("Args mutation lock is poisoned"))?
            .param_file = Some((param_file_arg.into(), use_always));
        Ok(this)
    }

    fn set_param_file_format<'v>(this: Value<'v>, format: &str) -> anyhow::Result<Value<'v>> {
        let args = StarlarkArgs::from_value(this)
            .ok_or_else(|| anyhow::anyhow!("Args.set_param_file_format receiver is invalid"))?;
        let format = match format {
            "shell" => RetainedParamFileFormat::Shell,
            "multiline" => RetainedParamFileFormat::Multiline,
            "flag_per_line" => RetainedParamFileFormat::FlagPerLine,
            _ => anyhow::bail!(
                "Invalid value for parameter format: expected shell, multiline, or flag_per_line"
            ),
        };
        let mut state = args
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("Args mutation lock is poisoned"))?;
        if state.format.is_some() {
            anyhow::bail!("set_param_file_format() may only be called once")
        }
        state.format = Some(format);
        Ok(this)
    }
}

fn vector_source<'v>(
    values: Value<'v>,
    operation: &str,
) -> anyhow::Result<EvaluatorVectorSourceGen<Value<'v>>> {
    let sequence = ListRef::from_value(values)
        .map(|values| values.iter().collect())
        .or_else(|| TupleRef::from_value(values).map(|values| values.iter().collect()));
    if let Some(values) = sequence {
        return Ok(EvaluatorVectorSourceGen::Sequence(values));
    }
    if crate::provider::StarlarkDepset::parts_from_value(values).is_some() {
        return Ok(EvaluatorVectorSourceGen::Depset(values));
    }
    anyhow::bail!("Args.{operation} values must be a sequence or depset")
}

fn vector_positionals<'v>(
    arg_name_or_values: Value<'v>,
    values: Option<Value<'v>>,
    operation: &str,
) -> anyhow::Result<(Option<&'v str>, EvaluatorVectorSourceGen<Value<'v>>)> {
    if let Some(values) = values {
        let arg_name = arg_name_or_values
            .unpack_str()
            .ok_or_else(|| anyhow::anyhow!("Args.{operation} arg name must be a string"))?;
        return Ok((Some(arg_name), vector_source(values, operation)?));
    }
    Ok((None, vector_source(arg_name_or_values, operation)?))
}

fn reject_callback_options(
    map_each: Option<Value<'_>>,
    allow_closure: bool,
    operation: &str,
) -> anyhow::Result<()> {
    if map_each.is_some_and(|value| !value.is_none()) || allow_closure {
        anyhow::bail!("Args.{operation} callback forms are not supported")
    }
    Ok(())
}

fn vector_source_is_empty(source: &EvaluatorVectorSourceGen<Value<'_>>) -> bool {
    match source {
        EvaluatorVectorSourceGen::Sequence(values) => values.is_empty(),
        EvaluatorVectorSourceGen::Depset(root) => {
            crate::provider::StarlarkDepset::parts_from_value(*root)
                .is_some_and(|(_, _, _, _, depth, _)| depth == 0)
        }
    }
}

fn validate_named_format(name: &str, format: &str) -> anyhow::Result<()> {
    validate_scalar_format(format)
        .map_err(|_| anyhow::anyhow!("Args {name} must contain exactly one %s placeholder"))
}

fn scalar_arg_value(value: Value<'_>) -> anyhow::Result<RetainedScalarValue> {
    if let Some(value) = value.unpack_str() {
        return Ok(RetainedScalarValue::String(value.into()));
    }
    if value.get_type() == "int" {
        return Ok(RetainedScalarValue::Integer(value.to_str().into()));
    }
    if let Some(file) = AnalysisArtifactValue::from_value(value) {
        if matches!(
            file.artifact(),
            AnalysisArtifact::Derived { output, .. }
                if output.kind() == slug_build_api_v2::ActionOutputKind::Directory
        ) {
            anyhow::bail!("Args.add does not support directory Files in this category")
        }
        return Ok(RetainedScalarValue::Artifact(file.artifact().clone()));
    }
    anyhow::bail!(
        "Args.add supports only strings, integers, and regular Files, got {}",
        value.get_type()
    )
}

fn validate_scalar_format(format: &str) -> anyhow::Result<()> {
    let mut placeholders = 0;
    let mut chars = format.chars();
    while let Some(character) = chars.next() {
        if character != '%' {
            continue;
        }
        match chars.next() {
            Some('%') => {}
            Some('s') => placeholders += 1,
            _ => anyhow::bail!("Args.add format must contain exactly one %s placeholder"),
        }
    }
    if placeholders != 1 {
        anyhow::bail!("Args.add format must contain exactly one %s placeholder")
    }
    Ok(())
}

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
pub struct AnalysisArtifactValue {
    artifact: AnalysisArtifact,
}

impl AnalysisArtifactValue {
    pub fn new(artifact: AnalysisArtifact) -> Self {
        Self { artifact }
    }

    pub fn from_starlark(value: Value<'_>) -> Option<&Self> {
        Self::from_value(value)
    }

    pub fn artifact(&self) -> &AnalysisArtifact {
        &self.artifact
    }

    pub fn output_for_owner(
        &self,
        expected: &AnalysisConfiguredTargetKey,
    ) -> Option<&ActionOutput> {
        match &self.artifact {
            AnalysisArtifact::Derived { owner, output } if owner == expected => Some(output),
            AnalysisArtifact::Source(_) | AnalysisArtifact::Derived { .. } => None,
        }
    }

    fn path(&self) -> String {
        self.artifact.path().into_owned()
    }

    fn basename(&self) -> String {
        self.path().rsplit('/').next().unwrap().to_owned()
    }

    fn dirname(&self) -> String {
        self.path()
            .rsplit_once('/')
            .map_or_else(String::new, |(dirname, _)| dirname.to_owned())
    }
}

impl fmt::Display for AnalysisArtifactValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.path())
    }
}

starlark::starlark_simple_value!(AnalysisArtifactValue);

#[starlark_value(type = "File")]
impl<'v> StarlarkValue<'v> for AnalysisArtifactValue {
    fn write_hash(&self, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        self.artifact.hash(hasher);
        Ok(())
    }

    fn equals(&self, other: Value<'v>) -> starlark::Result<bool> {
        Ok(Self::from_value(other).is_some_and(|other| self.artifact == other.artifact))
    }

    fn get_attr(&self, name: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match name {
            "path" => Some(heap.alloc_str(&self.path()).to_value()),
            "short_path" => Some(heap.alloc_str(&self.path()).to_value()),
            "basename" => Some(heap.alloc_str(&self.basename()).to_value()),
            "dirname" => Some(heap.alloc_str(&self.dirname()).to_value()),
            "label" => Some(alloc_starlark_label(
                heap,
                match &self.artifact {
                    AnalysisArtifact::Source(label) => label.clone(),
                    AnalysisArtifact::Derived { owner, .. } => owner.label().clone(),
                },
            )),
            _ => None,
        }
    }

    fn dir_attr(&self) -> Vec<String> {
        ["basename", "dirname", "label", "path", "short_path"]
            .map(str::to_owned)
            .to_vec()
    }
}

#[derive(Debug, Clone)]
struct CallFrame {
    token: u64,
    caller: Option<Arc<SubruleIdentity>>,
    direct: Arc<[Arc<SubruleIdentity>]>,
}

#[derive(Debug)]
struct AnalysisCallStack {
    next: u64,
    frames: Vec<CallFrame>,
}

#[derive(Debug, Clone)]
pub struct AnalysisCallToken {
    token: u64,
    stack: Arc<Mutex<AnalysisCallStack>>,
}

impl AnalysisCallToken {
    pub fn require_active(&self, field: &str, context: &str) -> anyhow::Result<()> {
        let active = self
            .stack
            .lock()
            .expect("analysis call stack lock is not poisoned")
            .frames
            .last()
            .is_some_and(|frame| frame.token == self.token);
        if active {
            Ok(())
        } else {
            anyhow::bail!(
                "cannot access field or method '{field}' of {context} outside of its own implementation function"
            )
        }
    }
}

struct CallFrameGuard {
    token: AnalysisCallToken,
}

impl Drop for CallFrameGuard {
    fn drop(&mut self) {
        let mut stack = self
            .token
            .stack
            .lock()
            .expect("analysis call stack lock is not poisoned");
        let frame = stack
            .frames
            .pop()
            .expect("subrule call frame remains installed until return");
        debug_assert_eq!(frame.token, self.token.token);
    }
}

#[derive(Debug, Clone)]
pub struct PreparedSubruleInvocation {
    identity: Arc<SubruleIdentity>,
    hidden: Arc<[(CompactString, FrozenValue)]>,
    fragments: Arc<SmallSet<CompactString>>,
}

impl PreparedSubruleInvocation {
    pub fn new(
        identity: Arc<SubruleIdentity>,
        hidden: impl Into<Arc<[(CompactString, FrozenValue)]>>,
        fragments: Arc<SmallSet<CompactString>>,
    ) -> Self {
        Self {
            identity,
            hidden: hidden.into(),
            fragments,
        }
    }
}

#[derive(Debug, Clone, ProvidesStaticType)]
pub struct AnalysisEvaluationContext {
    stack: Arc<Mutex<AnalysisCallStack>>,
    payload: Arc<AnalysisEvaluationPayload>,
}

#[derive(Debug)]
struct AnalysisEvaluationPayload {
    prepared: SmallMap<Arc<SubruleIdentity>, PreparedSubruleInvocation>,
    target_label: CanonicalLabel,
    action_sink: Arc<dyn AnalysisActionSink>,
    cpp_fragment: FrozenValue,
    coverage_fragment: FrozenValue,
    source_identities_by_filename: Arc<[(CompactString, BzlModuleIdentity)]>,
}

impl AnalysisEvaluationContext {
    pub fn new(
        direct: Arc<[Arc<SubruleIdentity>]>,
        prepared: impl IntoIterator<Item = PreparedSubruleInvocation>,
        target_label: CanonicalLabel,
        action_sink: Arc<dyn AnalysisActionSink>,
        cpp_fragment: FrozenValue,
        source_identities_by_filename: Arc<[(CompactString, BzlModuleIdentity)]>,
    ) -> Self {
        Self::new_with_coverage(
            direct,
            prepared,
            target_label,
            action_sink,
            cpp_fragment,
            FrozenValue::new_none(),
            source_identities_by_filename,
        )
    }

    pub fn new_with_coverage(
        direct: Arc<[Arc<SubruleIdentity>]>,
        prepared: impl IntoIterator<Item = PreparedSubruleInvocation>,
        target_label: CanonicalLabel,
        action_sink: Arc<dyn AnalysisActionSink>,
        cpp_fragment: FrozenValue,
        coverage_fragment: FrozenValue,
        source_identities_by_filename: Arc<[(CompactString, BzlModuleIdentity)]>,
    ) -> Self {
        let stack = Arc::new(Mutex::new(AnalysisCallStack {
            next: 1,
            frames: vec![CallFrame {
                token: 0,
                caller: None,
                direct,
            }],
        }));
        Self {
            stack,
            payload: Arc::new(AnalysisEvaluationPayload {
                prepared: prepared
                    .into_iter()
                    .map(|row| (row.identity.clone(), row))
                    .collect(),
                target_label,
                action_sink,
                cpp_fragment,
                coverage_fragment,
                source_identities_by_filename,
            }),
        }
    }

    pub fn root_token(&self) -> AnalysisCallToken {
        AnalysisCallToken {
            token: 0,
            stack: self.stack.clone(),
        }
    }

    pub fn from_evaluator<'a>(eval: &'a Evaluator<'_, '_, '_>) -> anyhow::Result<&'a Self> {
        eval.extra
            .and_then(|extra| extra.downcast_ref::<Self>())
            .ok_or_else(|| anyhow::anyhow!("subrules may only be called from configured analysis"))
    }

    pub fn cloned_from_evaluator(eval: &Evaluator<'_, '_, '_>) -> anyhow::Result<Self> {
        Ok(Self::from_evaluator(eval)?.clone())
    }

    pub(crate) fn source_identities_by_filename(
        &self,
    ) -> &Arc<[(CompactString, BzlModuleIdentity)]> {
        &self.payload.source_identities_by_filename
    }

    pub(crate) fn invoke<'v>(
        &self,
        identity: &Arc<SubruleIdentity>,
        direct: Arc<[Arc<SubruleIdentity>]>,
        implementation: FrozenValue,
        args: &Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        {
            let stack = self
                .stack
                .lock()
                .expect("analysis call stack lock is not poisoned");
            let caller = stack
                .frames
                .last()
                .expect("configured analysis keeps a root call frame");
            if !caller.direct.contains(identity) {
                let message = caller.caller.as_ref().map_or_else(
                    || {
                        format!(
                            "rule must declare '{}' in 'subrules'",
                            identity.exported_name
                        )
                    },
                    |caller| {
                        format!(
                            "subrule {} must declare {} in 'subrules'",
                            caller.exported_name, identity.exported_name
                        )
                    },
                );
                return Err(starlark::Error::new_other(anyhow::anyhow!(message)));
            }
        }
        let prepared = self.payload.prepared.get(identity).ok_or_else(|| {
            starlark::Error::new_other(anyhow::anyhow!(
                "rule must declare '{}' in 'subrules'",
                identity.exported_name
            ))
        })?;
        let mut names = args.names_map()?;
        for (name, _) in prepared.hidden.iter() {
            if names.keys().any(|candidate| candidate.as_str() == name) {
                return Err(starlark::Error::new_other(anyhow::anyhow!(
                    "got invalid named argument: '{name}' is an implicit dependency and cannot be overridden"
                )));
            }
        }
        let token = {
            let mut stack = self
                .stack
                .lock()
                .expect("analysis call stack lock is not poisoned");
            let token = stack.next;
            stack.next = stack
                .next
                .checked_add(1)
                .expect("subrule call token overflow");
            stack.frames.push(CallFrame {
                token,
                caller: Some(identity.clone()),
                direct,
            });
            AnalysisCallToken {
                token,
                stack: self.stack.clone(),
            }
        };
        let _guard = CallFrameGuard {
            token: token.clone(),
        };
        let fragments = eval.frozen_heap().alloc(SubruleFragmentCollection::new(
            token.clone(),
            prepared.fragments.clone(),
            self.payload.cpp_fragment,
            self.payload.coverage_fragment,
        ));
        let context = eval.heap().alloc(SubruleContext {
            token: token.clone(),
            target_label: self.payload.target_label.clone(),
            action_sink: self.payload.action_sink.clone(),
            fragments,
            name: identity.exported_name.clone(),
            identity: identity.clone(),
        });
        let mut positions = Vec::with_capacity(args.len()? + 1);
        positions.push(context);
        positions.extend(args.positions(eval.heap())?);
        for (name, value) in prepared.hidden.iter() {
            names.insert(eval.heap().alloc_str(name), value.to_value());
        }
        let kwargs = eval.heap().alloc(names);
        implementation
            .to_value()
            .invoke_pos_kwargs(&positions, Some(kwargs), eval)
    }
}

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
struct SubruleContext {
    #[allocative(skip)]
    token: AnalysisCallToken,
    target_label: CanonicalLabel,
    #[allocative(skip)]
    action_sink: Arc<dyn AnalysisActionSink>,
    #[allocative(skip)]
    fragments: FrozenValue,
    name: CompactString,
    identity: Arc<SubruleIdentity>,
}

impl fmt::Display for SubruleContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<{} context for {}>", self.name, self.target_label)
    }
}

starlark::starlark_simple_value!(SubruleContext);

#[starlark_value(type = "subrule_ctx")]
impl<'v> StarlarkValue<'v> for SubruleContext {
    fn get_methods() -> Option<&'static Methods> {
        static METHODS: MethodsStatic = MethodsStatic::new();
        METHODS.methods(subrule_context_methods)
    }
}

#[starlark_module]
fn subrule_context_methods(builder: &mut MethodsBuilder) {
    #[starlark(attribute)]
    fn label<'v>(this: &SubruleContext, heap: Heap<'v>) -> anyhow::Result<Value<'v>> {
        this.token.require_active("label", "subrule context")?;
        Ok(alloc_starlark_label(heap, this.target_label.clone()))
    }

    #[starlark(attribute)]
    fn actions<'v>(this: &SubruleContext, heap: Heap<'v>) -> anyhow::Result<Value<'v>> {
        this.token.require_active("actions", "subrule context")?;
        Ok(heap.alloc_simple(AnalysisActions {
            action_sink: this.action_sink.clone(),
            token: this.token.clone(),
            context_name: "subrule context",
            scope: AnalysisActionCallScope::Subrule(this.identity.clone()),
        }))
    }

    #[starlark(attribute)]
    fn fragments<'v>(this: &SubruleContext) -> anyhow::Result<Value<'v>> {
        this.token.require_active("fragments", "subrule context")?;
        Ok(this.fragments.to_value())
    }

    #[starlark(attribute)]
    fn toolchains(this: &SubruleContext) -> anyhow::Result<NoneType> {
        this.token.require_active("toolchains", "subrule context")?;
        anyhow::bail!("configured subrule toolchains are deferred")
    }
}

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
pub struct AnalysisActions {
    #[allocative(skip)]
    action_sink: Arc<dyn AnalysisActionSink>,
    #[allocative(skip)]
    token: AnalysisCallToken,
    context_name: &'static str,
    scope: AnalysisActionCallScope,
}

impl AnalysisActions {
    pub fn new(
        action_sink: Arc<dyn AnalysisActionSink>,
        token: AnalysisCallToken,
        context_name: &'static str,
        scope: AnalysisActionCallScope,
    ) -> Self {
        Self {
            action_sink,
            token,
            context_name,
            scope,
        }
    }

    pub fn register_absolute_symlink(
        &self,
        output: Value<'_>,
        target_path: &str,
        progress_message: Option<&str>,
    ) -> anyhow::Result<()> {
        self.token
            .require_active("absolute_symlink", self.context_name)?;
        self.action_sink
            .absolute_symlink(output, target_path, progress_message)
    }
}

impl fmt::Display for AnalysisActions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<ctx.actions>")
    }
}

starlark::starlark_simple_value!(AnalysisActions);

fn action_sequence<'v>(value: Value<'v>, name: &str) -> anyhow::Result<Vec<Value<'v>>> {
    ListRef::from_value(value)
        .map(|values| values.iter().collect())
        .or_else(|| TupleRef::from_value(value).map(|values| values.iter().collect()))
        .ok_or_else(|| anyhow::anyhow!("{name} must be a sequence"))
}

fn bind_action_outputs(value: Value<'_>, operation: &str) -> anyhow::Result<()> {
    let values = action_sequence(value, &format!("ctx.actions.{operation} outputs"))?;
    if values
        .iter()
        .any(|value| AnalysisArtifactValue::from_starlark(*value).is_none())
    {
        anyhow::bail!("ctx.actions.{operation} outputs must contain Files")
    }
    Ok(())
}

fn bind_action_inputs<'v>(
    value: Option<Value<'v>>,
    operation: &str,
) -> anyhow::Result<Option<Value<'v>>> {
    let Some(value) = value else {
        return Ok(None);
    };
    if let Ok(values) = action_sequence(value, "inputs") {
        if values
            .iter()
            .any(|value| AnalysisArtifactValue::from_starlark(*value).is_none())
        {
            anyhow::bail!("ctx.actions.{operation} inputs must contain Files")
        }
        return Ok(Some(value));
    }
    if crate::provider::StarlarkDepset::parts_from_value(value).is_some() {
        return Ok(Some(value));
    }
    anyhow::bail!("ctx.actions.{operation} inputs must be a sequence or depset")
}

fn bind_action_tools<'v>(
    value: Option<Value<'v>>,
    operation: &str,
) -> anyhow::Result<Option<Value<'v>>> {
    let Some(value) = value else {
        return Ok(None);
    };
    if action_sequence(value, "tools").is_ok()
        || crate::provider::StarlarkDepset::parts_from_value(value).is_some()
    {
        return Ok(Some(value));
    }
    anyhow::bail!("ctx.actions.{operation} tools must be a sequence or depset")
}

fn bind_action_arguments<'v>(
    value: Option<Value<'v>>,
    operation: &str,
) -> anyhow::Result<Option<Value<'v>>> {
    let Some(value) = value else {
        return Ok(None);
    };
    action_sequence(value, &format!("ctx.actions.{operation} arguments"))?;
    Ok(Some(value))
}

fn bind_optional_file<'v>(
    value: Option<Value<'v>>,
    name: &str,
) -> anyhow::Result<Option<Value<'v>>> {
    let Some(value) = value.filter(|value| !value.is_none()) else {
        return Ok(None);
    };
    if AnalysisArtifactValue::from_starlark(value).is_none() {
        anyhow::bail!("{name} must be a File or None")
    }
    Ok(Some(value))
}

fn bind_optional_string<'v>(
    value: Option<Value<'v>>,
    name: &str,
) -> anyhow::Result<Option<&'v str>> {
    let Some(value) = value.filter(|value| !value.is_none()) else {
        return Ok(None);
    };
    value
        .unpack_str()
        .map(Some)
        .ok_or_else(|| anyhow::anyhow!("{name} must be a string or None"))
}

fn bind_optional_bool(value: Option<Value<'_>>, name: &str) -> anyhow::Result<bool> {
    let Some(value) = value else {
        return Ok(false);
    };
    value
        .unpack_bool()
        .ok_or_else(|| anyhow::anyhow!("{name} must be a bool"))
}

fn bind_optional_dict<'v>(
    value: Option<Value<'v>>,
    name: &str,
) -> anyhow::Result<Option<Value<'v>>> {
    let Some(value) = value.filter(|value| !value.is_none()) else {
        return Ok(None);
    };
    DictRef::from_value(value)
        .map(|_| Some(value))
        .ok_or_else(|| anyhow::anyhow!("{name} must be a dictionary or None"))
}

fn bind_input_manifests(value: Option<Value<'_>>, operation: &str) -> anyhow::Result<()> {
    let Some(value) = value.filter(|value| !value.is_none()) else {
        return Ok(());
    };
    action_sequence(value, &format!("ctx.actions.{operation} input_manifests"))?;
    Ok(())
}

fn bind_executable(value: Value<'_>, sink: &dyn AnalysisActionSink) -> anyhow::Result<()> {
    if AnalysisArtifactValue::from_starlark(value).is_some()
        || value.unpack_str().is_some()
        || sink.is_files_to_run_provider(value)
    {
        return Ok(());
    }
    anyhow::bail!("ctx.actions.run executable must be a File, string, or FilesToRunProvider")
}

fn bind_shell_command(value: Value<'_>) -> anyhow::Result<()> {
    if value.unpack_str().is_some() {
        return Ok(());
    }
    let values = action_sequence(value, "ctx.actions.run_shell command")?;
    if values.iter().any(|value| value.unpack_str().is_none()) {
        anyhow::bail!("ctx.actions.run_shell command sequence must contain strings")
    }
    Ok(())
}

fn bind_resource_set(value: Option<Value<'_>>, operation: &str) -> anyhow::Result<bool> {
    let Some(value) = value.filter(|value| !value.is_none()) else {
        return Ok(false);
    };
    let callable: Option<StarlarkCallable<'_>> = StarlarkCallable::unpack_value(value)
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    if callable.is_some() {
        return Ok(true);
    }
    anyhow::bail!("ctx.actions.{operation} resource_set must be callable or None")
}

fn bind_toolchain<'v>(
    value: Option<Value<'v>>,
    operation: &str,
) -> anyhow::Result<Option<Value<'v>>> {
    let Some(value) = value.filter(|value| !value.is_none()) else {
        return Ok(None);
    };
    if value.unpack_str().is_some() || StarlarkLabel::from_value(value).is_some() {
        return Ok(Some(value));
    }
    anyhow::bail!("ctx.actions.{operation} toolchain must be a Label, string, or None")
}

fn bind_shadowed_action<'v>(
    value: Option<Value<'v>>,
    operation: &str,
) -> anyhow::Result<Option<Value<'v>>> {
    let Some(_value) = value.filter(|value| !value.is_none()) else {
        return Ok(None);
    };
    anyhow::bail!("ctx.actions.{operation} shadowed_action is not supported")
}

#[allow(clippy::too_many_arguments)]
fn bind_run_request<'v>(
    sink: &dyn AnalysisActionSink,
    scope: AnalysisActionCallScope,
    outputs: Value<'v>,
    inputs: Option<Value<'v>>,
    unused_inputs_list: Option<Value<'v>>,
    executable: Value<'v>,
    tools: Option<Value<'v>>,
    arguments: Option<Value<'v>>,
    mnemonic: Option<Value<'v>>,
    progress_message: Option<Value<'v>>,
    use_default_shell_env: Option<Value<'v>>,
    env: Option<Value<'v>>,
    execution_requirements: Option<Value<'v>>,
    input_manifests: Option<Value<'v>>,
    exec_group: Option<Value<'v>>,
    shadowed_action: Option<Value<'v>>,
    resource_set: Option<Value<'v>>,
    toolchain: Option<Value<'v>>,
) -> anyhow::Result<AnalysisSpawnRequest<'v>> {
    bind_action_outputs(outputs, "run")?;
    let inputs = bind_action_inputs(inputs, "run")?;
    let unused_inputs_list =
        bind_optional_file(unused_inputs_list, "ctx.actions.run unused_inputs_list")?;
    bind_executable(executable, sink)?;
    let tools = bind_action_tools(tools, "run")?;
    let arguments = bind_action_arguments(arguments, "run")?;
    let mnemonic = bind_optional_string(mnemonic, "ctx.actions.run mnemonic")?;
    let progress_message =
        bind_optional_string(progress_message, "ctx.actions.run progress_message")?;
    let use_default_shell_env = bind_optional_bool(
        use_default_shell_env,
        "ctx.actions.run use_default_shell_env",
    )?;
    let env = bind_optional_dict(env, "ctx.actions.run env")?;
    let execution_requirements = bind_optional_dict(
        execution_requirements,
        "ctx.actions.run execution_requirements",
    )?;
    bind_input_manifests(input_manifests, "run")?;
    let exec_group = bind_optional_string(exec_group, "ctx.actions.run exec_group")?;
    let shadowed_action = bind_shadowed_action(shadowed_action, "run")?;
    let has_resource_set = bind_resource_set(resource_set, "run")?;
    let toolchain = bind_toolchain(toolchain, "run")?;
    Ok(AnalysisSpawnRequest {
        scope,
        invocation: AnalysisSpawnInvocation::Executable(executable),
        outputs,
        inputs,
        unused_inputs_list,
        tools,
        arguments,
        mnemonic,
        progress_message,
        use_default_shell_env,
        env,
        execution_requirements,
        exec_group,
        shadowed_action,
        has_resource_set,
        toolchain,
    })
}

#[allow(clippy::too_many_arguments)]
fn bind_run_shell_request<'v>(
    scope: AnalysisActionCallScope,
    outputs: Value<'v>,
    inputs: Option<Value<'v>>,
    tools: Option<Value<'v>>,
    arguments: Option<Value<'v>>,
    mnemonic: Option<Value<'v>>,
    command: Value<'v>,
    progress_message: Option<Value<'v>>,
    use_default_shell_env: Option<Value<'v>>,
    env: Option<Value<'v>>,
    execution_requirements: Option<Value<'v>>,
    input_manifests: Option<Value<'v>>,
    exec_group: Option<Value<'v>>,
    shadowed_action: Option<Value<'v>>,
    resource_set: Option<Value<'v>>,
    toolchain: Option<Value<'v>>,
) -> anyhow::Result<AnalysisSpawnRequest<'v>> {
    bind_action_outputs(outputs, "run_shell")?;
    let inputs = bind_action_inputs(inputs, "run_shell")?;
    let tools = bind_action_tools(tools, "run_shell")?;
    let arguments = bind_action_arguments(arguments, "run_shell")?;
    let mnemonic = bind_optional_string(mnemonic, "ctx.actions.run_shell mnemonic")?;
    bind_shell_command(command)?;
    let progress_message =
        bind_optional_string(progress_message, "ctx.actions.run_shell progress_message")?;
    let use_default_shell_env = bind_optional_bool(
        use_default_shell_env,
        "ctx.actions.run_shell use_default_shell_env",
    )?;
    let env = bind_optional_dict(env, "ctx.actions.run_shell env")?;
    let execution_requirements = bind_optional_dict(
        execution_requirements,
        "ctx.actions.run_shell execution_requirements",
    )?;
    bind_input_manifests(input_manifests, "run_shell")?;
    let exec_group = bind_optional_string(exec_group, "ctx.actions.run_shell exec_group")?;
    let shadowed_action = bind_shadowed_action(shadowed_action, "run_shell")?;
    let has_resource_set = bind_resource_set(resource_set, "run_shell")?;
    let toolchain = bind_toolchain(toolchain, "run_shell")?;
    Ok(AnalysisSpawnRequest {
        scope,
        invocation: AnalysisSpawnInvocation::Shell(command),
        outputs,
        inputs,
        unused_inputs_list: None,
        tools,
        arguments,
        mnemonic,
        progress_message,
        use_default_shell_env,
        env,
        execution_requirements,
        exec_group,
        shadowed_action,
        has_resource_set,
        toolchain,
    })
}

#[starlark_module]
fn analysis_actions_methods(builder: &mut MethodsBuilder) {
    fn declare_file(this: Value, path: &str) -> anyhow::Result<AnalysisArtifactValue> {
        let actions = AnalysisActions::from_value(this)
            .ok_or_else(|| anyhow::anyhow!("ctx.actions receiver is invalid"))?;
        actions
            .token
            .require_active("declare_file", actions.context_name)?;
        actions.action_sink.declare_file(path)
    }

    fn write(
        this: Value,
        output: Value,
        content: Value,
        #[starlark(default = false)] is_executable: bool,
    ) -> anyhow::Result<NoneType> {
        let actions = AnalysisActions::from_value(this)
            .ok_or_else(|| anyhow::anyhow!("ctx.actions receiver is invalid"))?;
        actions
            .token
            .require_active("write", actions.context_name)?;
        actions.action_sink.write(output, content, is_executable)?;
        Ok(NoneType)
    }

    fn run_shell<'v>(
        this: Value<'v>,
        #[starlark(require = named)] outputs: Value<'v>,
        #[starlark(require = named)] inputs: Option<Value<'v>>,
        #[starlark(require = named)] tools: Option<Value<'v>>,
        #[starlark(require = named)] arguments: Option<Value<'v>>,
        #[starlark(require = named)] mnemonic: Option<Value<'v>>,
        #[starlark(require = named)] command: Value<'v>,
        #[starlark(require = named)] progress_message: Option<Value<'v>>,
        #[starlark(require = named)] use_default_shell_env: Option<Value<'v>>,
        #[starlark(require = named)] env: Option<Value<'v>>,
        #[starlark(require = named)] execution_requirements: Option<Value<'v>>,
        #[starlark(require = named)] input_manifests: Option<Value<'v>>,
        #[starlark(require = named)] exec_group: Option<Value<'v>>,
        #[starlark(require = named)] shadowed_action: Option<Value<'v>>,
        #[starlark(require = named)] resource_set: Option<Value<'v>>,
        #[starlark(require = named)] toolchain: Option<Value<'v>>,
    ) -> anyhow::Result<NoneType> {
        let actions = AnalysisActions::from_value(this)
            .ok_or_else(|| anyhow::anyhow!("ctx.actions receiver is invalid"))?;
        let request = bind_run_shell_request(
            actions.scope.clone(),
            outputs,
            inputs,
            tools,
            arguments,
            mnemonic,
            command,
            progress_message,
            use_default_shell_env,
            env,
            execution_requirements,
            input_manifests,
            exec_group,
            shadowed_action,
            resource_set,
            toolchain,
        )?;
        actions
            .token
            .require_active("run_shell", actions.context_name)?;
        actions.action_sink.spawn(request)?;
        Ok(NoneType)
    }

    fn args<'v>(this: Value<'v>, heap: Heap<'v>) -> anyhow::Result<Value<'v>> {
        let actions = AnalysisActions::from_value(this)
            .ok_or_else(|| anyhow::anyhow!("ctx.actions receiver is invalid"))?;
        actions.token.require_active("args", actions.context_name)?;
        Ok(heap.alloc_complex(StarlarkArgs {
            state: Arc::new(Mutex::new(StarlarkArgsStateGen {
                calls: Vec::new(),
                format: None,
                param_file: None,
            })),
        }))
    }

    fn run<'v>(
        this: Value<'v>,
        #[starlark(require = named)] outputs: Value<'v>,
        #[starlark(require = named)] inputs: Option<Value<'v>>,
        #[starlark(require = named)] unused_inputs_list: Option<Value<'v>>,
        #[starlark(require = named)] executable: Value<'v>,
        #[starlark(require = named)] tools: Option<Value<'v>>,
        #[starlark(require = named)] arguments: Option<Value<'v>>,
        #[starlark(require = named)] mnemonic: Option<Value<'v>>,
        #[starlark(require = named)] progress_message: Option<Value<'v>>,
        #[starlark(require = named)] use_default_shell_env: Option<Value<'v>>,
        #[starlark(require = named)] env: Option<Value<'v>>,
        #[starlark(require = named)] execution_requirements: Option<Value<'v>>,
        #[starlark(require = named)] input_manifests: Option<Value<'v>>,
        #[starlark(require = named)] exec_group: Option<Value<'v>>,
        #[starlark(require = named)] shadowed_action: Option<Value<'v>>,
        #[starlark(require = named)] resource_set: Option<Value<'v>>,
        #[starlark(require = named)] toolchain: Option<Value<'v>>,
    ) -> anyhow::Result<NoneType> {
        let actions = AnalysisActions::from_value(this)
            .ok_or_else(|| anyhow::anyhow!("ctx.actions receiver is invalid"))?;
        let request = bind_run_request(
            actions.action_sink.as_ref(),
            actions.scope.clone(),
            outputs,
            inputs,
            unused_inputs_list,
            executable,
            tools,
            arguments,
            mnemonic,
            progress_message,
            use_default_shell_env,
            env,
            execution_requirements,
            input_manifests,
            exec_group,
            shadowed_action,
            resource_set,
            toolchain,
        )?;
        actions.token.require_active("run", actions.context_name)?;
        actions.action_sink.spawn(request)?;
        Ok(NoneType)
    }

    fn symlink<'v>(
        this: Value<'v>,
        #[starlark(require = named)] output: Value<'v>,
        #[starlark(require = named)] target_file: Value<'v>,
        #[starlark(require = named, default = false)] is_executable: bool,
        #[starlark(require = named)] progress_message: Option<&'v str>,
    ) -> anyhow::Result<NoneType> {
        let actions = AnalysisActions::from_value(this)
            .ok_or_else(|| anyhow::anyhow!("ctx.actions receiver is invalid"))?;
        actions
            .token
            .require_active("symlink", actions.context_name)?;
        actions.action_sink.artifact_symlink(
            output,
            target_file,
            is_executable,
            progress_message,
        )?;
        Ok(NoneType)
    }
}

#[starlark_value(type = "analysis_actions")]
impl<'v> StarlarkValue<'v> for AnalysisActions {
    fn get_methods() -> Option<&'static Methods> {
        static METHODS: MethodsStatic = MethodsStatic::new();
        METHODS.methods(analysis_actions_methods)
    }
}
