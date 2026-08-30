/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select either.
 */

use std::cell::RefCell;
use std::fmt;
use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use dupe::Dupe;
use slug_bzlmod_v2::GeneratedRepositoryFileEffectPlan;
use slug_bzlmod_v2::GeneratedRepositoryFileEffectPlanBuilder;
use slug_bzlmod_v2::GeneratedRepositoryFileEffectPlanError;
use slug_bzlmod_v2::RepositoryEnvironmentSnapshot;
use slug_bzlmod_v2::RepositoryPlatform;
use starlark::PrintHandler;
use starlark::any::ProvidesStaticType;
use starlark::environment::Methods;
use starlark::environment::MethodsBuilder;
use starlark::environment::MethodsStatic;
use starlark::environment::Module;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::StarlarkValue;
use starlark::values::Value;
use starlark::values::dict::AllocDict;
use starlark::values::none::NoneType;
use starlark::values::starlark_value;
#[doc(hidden)]
#[rustfmt::skip]
#[derive(Clone, PartialEq, Eq, Allocative)]
pub struct RepositoryRuleHostObservation { platform: RepositoryPlatform, environment: Arc<[(CompactString, Option<Arc<str>>)]> }

#[rustfmt::skip]
impl fmt::Debug for RepositoryRuleHostObservation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.debug_struct("RepositoryRuleHostObservation").field("platform", &self.platform).field("environment", &self.environment.iter().map(|(name, value)| (name, value.as_ref().map(|_| "<redacted>"))).collect::<Vec<_>>()).finish() }
}

#[rustfmt::skip]
impl RepositoryRuleHostObservation {
    pub(crate) fn new(platform: RepositoryPlatform, environment: impl IntoIterator<Item = (CompactString, Option<Arc<str>>)>) -> Self {
        let mut environment = environment.into_iter().collect::<Vec<_>>();
        environment.sort_by(|left, right| left.0.cmp(&right.0)); environment.dedup_by(|left, right| left.0 == right.0);
        Self { platform, environment: environment.into() }
    }
    pub fn platform(&self) -> &RepositoryPlatform { &self.platform }
    pub fn environment(&self) -> impl ExactSizeIterator<Item = (&str, Option<&Arc<str>>)> { self.environment.iter().map(|(name, value)| (name.as_str(), value.as_ref())) }
}

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RepositoryRuleInvocationError { PathArgument, Plan(GeneratedRepositoryFileEffectPlanError), Evaluation(CompactString), Result(CompactString) }

#[rustfmt::skip]
pub(crate) struct RepositoryRuleInvocation { pub(crate) plan: GeneratedRepositoryFileEffectPlan, pub(crate) dynamic_environment: Arc<[CompactString]> }

#[rustfmt::skip]
impl RepositoryRuleInvocation {
    pub(crate) fn dynamic_environment(&self) -> &[CompactString] { &self.dynamic_environment }
    pub(crate) fn into_plan(self) -> GeneratedRepositoryFileEffectPlan { self.plan }
}

#[rustfmt::skip]
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
struct RepositoryRuleContext { platform: RepositoryPlatform, snapshot: RepositoryEnvironmentSnapshot }

#[rustfmt::skip]
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
struct RepositoryOs { platform: RepositoryPlatform, snapshot: RepositoryEnvironmentSnapshot }

#[rustfmt::skip]
#[derive(Debug, ProvidesStaticType)]
struct RepositoryRuleInvocationState { effects: RefCell<Option<GeneratedRepositoryFileEffectPlanBuilder>>, dynamic_environment: RefCell<Vec<CompactString>>, error: RefCell<Option<RepositoryRuleInvocationError>> }

#[rustfmt::skip]
impl RepositoryRuleInvocationState {
    fn new() -> Self { Self { effects: RefCell::new(Some(GeneratedRepositoryFileEffectPlan::builder())), dynamic_environment: RefCell::new(Vec::new()), error: RefCell::new(None) } }
    fn from_evaluator<'a>(eval: &'a Evaluator<'_, '_, '_>) -> anyhow::Result<&'a Self> { eval.extra.and_then(|extra| extra.downcast_ref::<Self>()).ok_or_else(|| anyhow::anyhow!("repository_ctx is outside repository-rule execution")) }
    fn fail(&self, error: RepositoryRuleInvocationError) -> anyhow::Error {
        *self.error.borrow_mut() = Some(error);
        anyhow::anyhow!("unsupported repository_ctx.file argument")
    }
    fn record_environment(&self, name: &str) { self.dynamic_environment.borrow_mut().push(name.into()); }
    fn finish(&self) -> RepositoryRuleInvocation {
        let mut dynamic_environment = self.dynamic_environment.borrow().clone();
        dynamic_environment.sort(); dynamic_environment.dedup();
        RepositoryRuleInvocation { plan: self.effects.borrow_mut().take().expect("repository context completes at most once").finish(), dynamic_environment: dynamic_environment.into() }
    }
}

#[rustfmt::skip]
impl fmt::Display for RepositoryRuleContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str("<repository_ctx>") }
}

#[rustfmt::skip]
impl fmt::Display for RepositoryOs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str("<repository_os>") }
}

starlark::starlark_simple_value!(RepositoryRuleContext);
starlark::starlark_simple_value!(RepositoryOs);

#[starlark_value(type = "repository_ctx")]
#[rustfmt::skip]
impl<'v> StarlarkValue<'v> for RepositoryRuleContext {
    fn get_attr(&self, name: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        (name == "os").then(|| heap.alloc_simple(RepositoryOs { platform: self.platform.clone(), snapshot: self.snapshot.dupe() }))
    }

    fn get_methods() -> Option<&'static Methods> {
        static METHODS: MethodsStatic = MethodsStatic::new();
        METHODS.methods(repository_rule_context_methods)
    }
}

#[starlark_value(type = "repository_os")]
#[rustfmt::skip]
impl<'v> StarlarkValue<'v> for RepositoryOs {
    fn get_attr(&self, name: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match name {
            "name" => Some(heap.alloc(self.platform.os_name())),
            "arch" => Some(heap.alloc(self.platform.arch())),
            "environ" => Some(heap.alloc(AllocDict(self.snapshot.iter().map(|entry| (entry.name(), entry.value().as_ref()))))),
            _ => None,
        }
    }
}

#[starlark_module]
fn repository_rule_context_methods(builder: &mut MethodsBuilder) {
    fn file<'v>(
        this: Value<'v>,
        #[starlark(require = pos)] path: Value<'v>,
        #[starlark(default = "")] content: &str,
        #[starlark(default = true)] executable: bool,
        #[starlark(default = false)] legacy_utf8: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<NoneType> {
        RepositoryRuleContext::from_value(this)
            .ok_or_else(|| anyhow::anyhow!("invalid repository_ctx receiver"))?;
        let _ = legacy_utf8;
        let state = RepositoryRuleInvocationState::from_evaluator(eval)?;
        let Some(path) = path.unpack_str() else {
            return Err(state.fail(RepositoryRuleInvocationError::PathArgument));
        };
        if let Err(error) = state
            .effects
            .borrow_mut()
            .as_mut()
            .expect("repository context has not completed")
            .push(
                CompactString::new(path),
                Arc::from(content.as_bytes()),
                executable,
            )
        {
            return Err(state.fail(RepositoryRuleInvocationError::Plan(error)));
        }
        Ok(NoneType)
    }

    fn getenv<'v>(
        this: Value<'v>,
        #[starlark(require = pos)] name: &str,
        #[starlark(require = pos)] default: Option<&str>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        let this = RepositoryRuleContext::from_value(this)
            .ok_or_else(|| anyhow::anyhow!("invalid repository_ctx receiver"))?;
        RepositoryRuleInvocationState::from_evaluator(eval)?.record_environment(name);
        Ok(this
            .snapshot
            .get(name)
            .map(|value| eval.heap().alloc(value.as_ref()))
            .or_else(|| default.map(|value| eval.heap().alloc(value)))
            .unwrap_or_else(Value::new_none))
    }
}

#[rustfmt::skip]
pub(crate) fn invoke_repository_rule(
    implementation: starlark::values::FrozenValue,
    platform: RepositoryPlatform,
    snapshot: RepositoryEnvironmentSnapshot,
    print_handler: Option<&dyn PrintHandler>,
) -> Result<RepositoryRuleInvocation, RepositoryRuleInvocationError> {
    let invocation_module = Module::new();
    let context = invocation_module.heap().alloc_simple(RepositoryRuleContext { platform, snapshot });
    let state = RepositoryRuleInvocationState::new();
    let returned = {
        let mut evaluator = Evaluator::new(&invocation_module);
        if let Some(print_handler) = print_handler {
            evaluator.set_print_handler(print_handler);
        }
        evaluator.extra = Some(&state);
        evaluator.eval_function(implementation.to_value(), &[context], &[])
    };
    let context_error = state.error.borrow_mut().take();
    match returned {
        Err(error) => Err(context_error.unwrap_or_else(|| RepositoryRuleInvocationError::Evaluation(error.to_string().into()))),
        Ok(value) if !value.is_none() => Err(RepositoryRuleInvocationError::Result(value.get_type().into())),
        Ok(_) => Ok(state.finish()),
    }
}

#[cfg(test)]
mod tests {
    use slug_bzlmod_v2::RepositoryEnvironmentEntry;
    use starlark::environment::Globals;
    use starlark::syntax::AstModule;
    use starlark::syntax::Dialect;

    use super::*;

    fn implementation(
        source: &str,
    ) -> (
        starlark::environment::FrozenModule,
        starlark::values::FrozenValue,
    ) {
        let module = Module::new();
        let ast =
            AstModule::parse("repository_rule.bzl", source.to_owned(), &Dialect::Bazel).unwrap();
        Evaluator::new(&module)
            .eval_module(ast, &Globals::standard())
            .unwrap();
        let module = module.freeze().unwrap();
        let implementation = unsafe {
            module
                .get("implementation")
                .unwrap()
                .unchecked_frozen_value()
        };
        (module, implementation)
    }

    fn invoke(source: &str) -> Result<RepositoryRuleInvocation, RepositoryRuleInvocationError> {
        let (_owner, implementation) = implementation(source);
        invoke_repository_rule(
            implementation,
            RepositoryPlatform::new("linux", "x86_64"),
            RepositoryEnvironmentSnapshot::empty(),
            None,
        )
    }

    #[test]
    fn context_exposes_exact_host_values_and_records_only_getenv_names() {
        let (_owner, implementation) = implementation(
            r#"
def implementation(ctx):
    ctx.file("values", repr([
        ctx.os.name,
        ctx.os.arch,
        ctx.os.environ,
        ctx.getenv("PRESENT"),
        ctx.getenv("MISSING"),
        ctx.getenv("MISSING", "fallback"),
        ctx.getenv("EMPTY"),
    ]), executable = False)
"#,
        );
        let snapshot = RepositoryEnvironmentSnapshot::from_canonical([
            RepositoryEnvironmentEntry::new("EMPTY", ""),
            RepositoryEnvironmentEntry::new("PRESENT", "value"),
            RepositoryEnvironmentEntry::new("UNOBSERVED", "ambient"),
        ])
        .unwrap();
        let invocation = invoke_repository_rule(
            implementation,
            RepositoryPlatform::new("linux", "x86_64"),
            snapshot,
            None,
        )
        .unwrap();
        assert_eq!(
            invocation.dynamic_environment(),
            ["EMPTY", "MISSING", "PRESENT"]
        );
        let effect = &invocation.plan.effects()[0];
        assert_eq!(effect.path(), "values");
        assert!(!effect.executable());
        assert_eq!(
            effect.content(),
            br#"["linux", "x86_64", {"EMPTY": "", "PRESENT": "value", "UNOBSERVED": "ambient"}, "value", None, "fallback", ""]"#
        );
    }

    #[test]
    #[rustfmt::skip]
    fn file_preserves_binding_order_modes_and_typed_path_failures() {
        let invocation = invoke(
            r#"
def implementation(ctx):
    ctx.file("BUILD.bazel", "one\n")
    ctx.file("generated", content = "two", executable = False, legacy_utf8 = True)
"#,
        )
        .unwrap();
        let effects = invocation.plan.effects();
        assert_eq!(effects.len(), 2);
        assert_eq!((effects[0].path(), effects[0].content(), effects[0].executable()), ("BUILD.bazel", b"one\n".as_slice(), true));
        assert_eq!((effects[1].path(), effects[1].content(), effects[1].executable()), ("generated", b"two".as_slice(), false));

        assert!(matches!(invoke(r#"
def implementation(ctx):
    ctx.file("same", "one")
    ctx.file("same", "two")
"#), Err(RepositoryRuleInvocationError::Plan(GeneratedRepositoryFileEffectPlanError::RepeatedPath(path))) if path == "same"));
        for path in ["", "/absolute", "a/../b", "a\\b", "a/"] {
            assert!(matches!(invoke(&format!("def implementation(ctx):\n    ctx.file({path:?})\n")), Err(RepositoryRuleInvocationError::Plan(GeneratedRepositoryFileEffectPlanError::InvalidPath(_)))));
        }
        assert!(matches!(invoke("def implementation(ctx):\n    ctx.file(1)\n"), Err(RepositoryRuleInvocationError::PathArgument)));
        for source in [
            "def implementation(ctx):\n    ctx.file(path='named')\n",
            "def implementation(ctx):\n    ctx.file('named', 'one', content='two')\n",
            "def implementation(ctx):\n    ctx.file('named', missing=True)\n",
            "def implementation(ctx):\n    ctx.unknown()\n",
        ] {
            assert!(matches!(invoke(source), Err(RepositoryRuleInvocationError::Evaluation(_))));
        }
    }
}
