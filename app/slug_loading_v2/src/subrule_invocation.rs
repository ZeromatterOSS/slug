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
use slug_build_api_v2::CtxActions;
use slug_identity_v2::CanonicalLabel;
use starlark::any::ProvidesStaticType;
use starlark::environment::Methods;
use starlark::environment::MethodsBuilder;
use starlark::environment::MethodsStatic;
use starlark::eval::Arguments;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::values::FrozenValue;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::StarlarkValue;
use starlark::values::Value;
use starlark::values::none::NoneType;
use starlark::values::starlark_value;
use starlark_map::StarlarkHasher;
use starlark_map::small_map::SmallMap;

use crate::provider::alloc_starlark_label;
use crate::subrule::SubruleIdentity;

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
        match &self.artifact {
            AnalysisArtifact::Source(label) => {
                let package = label.package().package().as_str();
                if package.is_empty() {
                    label.target().as_str().to_owned()
                } else {
                    format!("{package}/{}", label.target())
                }
            }
            AnalysisArtifact::Derived { output, .. } => output.path().to_owned(),
        }
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
        vec!["label".to_owned(), "path".to_owned()]
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
}

impl PreparedSubruleInvocation {
    pub fn new(
        identity: Arc<SubruleIdentity>,
        hidden: impl Into<Arc<[(CompactString, FrozenValue)]>>,
    ) -> Self {
        Self {
            identity,
            hidden: hidden.into(),
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
    package_path: String,
    owner: AnalysisConfiguredTargetKey,
    actions: Arc<Mutex<CtxActions>>,
}

impl AnalysisEvaluationContext {
    pub fn new(
        direct: Arc<[Arc<SubruleIdentity>]>,
        prepared: impl IntoIterator<Item = PreparedSubruleInvocation>,
        target_label: CanonicalLabel,
        package_path: String,
        owner: AnalysisConfiguredTargetKey,
        actions: Arc<Mutex<CtxActions>>,
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
                package_path,
                owner,
                actions,
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
        let context = eval.heap().alloc(SubruleContext {
            token: token.clone(),
            target_label: self.payload.target_label.clone(),
            package_path: self.payload.package_path.clone(),
            owner: self.payload.owner.clone(),
            actions: self.payload.actions.clone(),
            name: identity.exported_name.clone(),
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
    package_path: String,
    owner: AnalysisConfiguredTargetKey,
    #[allocative(skip)]
    actions: Arc<Mutex<CtxActions>>,
    name: CompactString,
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
            actions: this.actions.clone(),
            package_path: this.package_path.clone(),
            owner: this.owner.clone(),
            token: this.token.clone(),
            context_name: "subrule context",
        }))
    }

    #[starlark(attribute)]
    fn fragments(this: &SubruleContext) -> anyhow::Result<NoneType> {
        this.token.require_active("fragments", "subrule context")?;
        anyhow::bail!("configured subrule fragments are deferred")
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
    actions: Arc<Mutex<CtxActions>>,
    package_path: String,
    owner: AnalysisConfiguredTargetKey,
    #[allocative(skip)]
    token: AnalysisCallToken,
    context_name: &'static str,
}

impl AnalysisActions {
    pub fn new(
        actions: Arc<Mutex<CtxActions>>,
        package_path: String,
        owner: AnalysisConfiguredTargetKey,
        token: AnalysisCallToken,
        context_name: &'static str,
    ) -> Self {
        Self {
            actions,
            package_path,
            owner,
            token,
            context_name,
        }
    }
}

impl fmt::Display for AnalysisActions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<ctx.actions>")
    }
}

starlark::starlark_simple_value!(AnalysisActions);

#[starlark_module]
fn analysis_actions_methods(builder: &mut MethodsBuilder) {
    fn declare_file(this: Value, path: &str) -> anyhow::Result<AnalysisArtifactValue> {
        let actions = AnalysisActions::from_value(this)
            .ok_or_else(|| anyhow::anyhow!("ctx.actions receiver is invalid"))?;
        actions
            .token
            .require_active("declare_file", actions.context_name)?;
        let path = if actions.package_path.is_empty() {
            path.to_owned()
        } else {
            format!("{}/{}", actions.package_path, path)
        };
        let output = actions
            .actions
            .lock()
            .map_err(|_| anyhow::anyhow!("ctx.actions state lock is poisoned"))?
            .declare_file(path)
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        Ok(AnalysisArtifactValue::new(AnalysisArtifact::Derived {
            owner: actions.owner.clone(),
            output,
        }))
    }

    fn write(
        this: Value,
        output: Value,
        content: &str,
        #[starlark(default = false)] is_executable: bool,
    ) -> anyhow::Result<NoneType> {
        let actions = AnalysisActions::from_value(this)
            .ok_or_else(|| anyhow::anyhow!("ctx.actions receiver is invalid"))?;
        actions
            .token
            .require_active("write", actions.context_name)?;
        let output = AnalysisArtifactValue::from_value(output)
            .ok_or_else(|| anyhow::anyhow!("ctx.actions.write requires a declared file"))?;
        let output = output
            .output_for_owner(&actions.owner)
            .ok_or_else(|| anyhow::anyhow!("ctx.actions.write requires a declared file"))?;
        actions
            .actions
            .lock()
            .map_err(|_| anyhow::anyhow!("ctx.actions state lock is poisoned"))?
            .write(output.clone(), content, is_executable)
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        Ok(NoneType)
    }

    fn run_shell<'v>(
        this: Value<'v>,
        outputs: Value<'v>,
        command: &str,
        arguments: Value<'v>,
        heap: Heap<'v>,
    ) -> anyhow::Result<NoneType> {
        let actions = AnalysisActions::from_value(this)
            .ok_or_else(|| anyhow::anyhow!("ctx.actions receiver is invalid"))?;
        actions
            .token
            .require_active("run_shell", actions.context_name)?;
        let mut declared = Vec::new();
        for item in outputs
            .iterate(heap)
            .map_err(|error| anyhow::anyhow!(error.to_string()))?
        {
            let file = AnalysisArtifactValue::from_value(item).ok_or_else(|| {
                anyhow::anyhow!("ctx.actions.run_shell outputs must be declared files")
            })?;
            declared.push(
                file.output_for_owner(&actions.owner)
                    .ok_or_else(|| {
                        anyhow::anyhow!("ctx.actions.run_shell outputs must be declared files")
                    })?
                    .clone(),
            );
        }
        let output = declared
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("ctx.actions.run_shell requires at least one output"))?;
        let mut args = Vec::new();
        for item in arguments
            .iterate(heap)
            .map_err(|error| anyhow::anyhow!(error.to_string()))?
        {
            args.push(item.to_str());
        }
        actions
            .actions
            .lock()
            .map_err(|_| anyhow::anyhow!("ctx.actions state lock is poisoned"))?
            .run_shell(output, command, args, Vec::new())
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
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
