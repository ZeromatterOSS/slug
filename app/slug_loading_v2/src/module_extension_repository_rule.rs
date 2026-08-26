/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::cell::OnceCell;
use std::cell::RefCell;
use std::fmt;
use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use slug_identity_v2::CanonicalLabel;
use starlark::any::ProvidesStaticType;
use starlark::eval::Arguments;
use starlark::eval::Evaluator;
use starlark::values::Freeze;
use starlark::values::FreezeResult;
use starlark::values::Freezer;
use starlark::values::FrozenValue;
use starlark::values::NoSerialize;
use starlark::values::StarlarkValue;
use starlark::values::Trace;
use starlark::values::Value;
use starlark::values::starlark_value;

use crate::attrs::AttributeKind;
use crate::attrs::CoercedAttributeValue;
use crate::starlark_label::StarlarkLabel;

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct RepositoryRuleAttribute {
    pub(crate) name: CompactString,
    pub(crate) kind: AttributeKind,
    pub(crate) mandatory: bool,
    pub(crate) default: Option<CoercedAttributeValue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct RepositoryRuleDefinitionProjection {
    pub(crate) defining_label: CanonicalLabel,
    pub(crate) exported_name: CompactString,
    pub(crate) attributes: Arc<[RepositoryRuleAttribute]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) enum RepositoryRuleCallValue {
    None,
    Bool(bool),
    Int(i32),
    String(CompactString),
    Label(CanonicalLabel),
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct RepositoryRuleCallSpan {
    pub(crate) file: CompactString,
    pub(crate) start_line: u32,
    pub(crate) start_column: u32,
    pub(crate) end_line: u32,
    pub(crate) end_column: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct RepositoryRuleCallFrame {
    pub(crate) function: CompactString,
    pub(crate) location: Option<RepositoryRuleCallSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct RepositoryRuleCallRecord {
    pub(crate) definition: RepositoryRuleDefinitionProjection,
    pub(crate) name: CompactString,
    pub(crate) kwargs: Arc<[(CompactString, RepositoryRuleCallValue)]>,
    pub(crate) caller: RepositoryRuleCallSpan,
    pub(crate) stack: Arc<[RepositoryRuleCallFrame]>,
}

#[derive(Debug, ProvidesStaticType)]
pub(crate) struct RepositoryRuleInvocationState {
    records: RefCell<Vec<RepositoryRuleCallRecord>>,
}

impl RepositoryRuleInvocationState {
    pub(crate) fn new() -> Self {
        Self {
            records: RefCell::new(Vec::new()),
        }
    }

    pub(crate) fn records(&self) -> Arc<[RepositoryRuleCallRecord]> {
        self.records.borrow().clone().into()
    }

    fn from_evaluator<'a>(
        eval: &'a Evaluator<'_, '_, '_>,
    ) -> anyhow::Result<&'a RepositoryRuleInvocationState> {
        eval.extra
            .and_then(|extra| extra.downcast_ref::<Self>())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "repo rules can only be called from within module extension impl functions"
                )
            })
    }

    fn contains_name(&self, name: &str) -> bool {
        self.records
            .borrow()
            .iter()
            .any(|record| record.name == name)
    }

    fn push(&self, record: RepositoryRuleCallRecord) {
        self.records.borrow_mut().push(record);
    }
}

#[derive(Debug, Trace, ProvidesStaticType, NoSerialize, Allocative)]
pub(crate) struct RepositoryRuleDefinitionGen<V> {
    implementation: V,
    #[trace(unsafe_ignore)]
    defining_label: CanonicalLabel,
    #[trace(unsafe_ignore)]
    attributes: Arc<[RepositoryRuleAttribute]>,
    #[trace(unsafe_ignore)]
    exported_name: OnceCell<CompactString>,
}

pub(crate) type RepositoryRuleDefinition<'v> = RepositoryRuleDefinitionGen<Value<'v>>;

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub(crate) struct FrozenRepositoryRuleDefinition {
    #[allow(dead_code)] // The repository implementation is a later lifetime-only consumer.
    #[allocative(skip)]
    implementation: FrozenValue,
    defining_label: CanonicalLabel,
    attributes: Arc<[RepositoryRuleAttribute]>,
    exported_name: Option<CompactString>,
}

starlark::starlark_complex_values!(RepositoryRuleDefinition);

impl<'v> RepositoryRuleDefinition<'v> {
    pub(crate) fn new(
        implementation: Value<'v>,
        defining_label: CanonicalLabel,
        attributes: Arc<[RepositoryRuleAttribute]>,
    ) -> Self {
        Self {
            implementation,
            defining_label,
            attributes,
            exported_name: OnceCell::new(),
        }
    }
}

impl<V> fmt::Display for RepositoryRuleDefinitionGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.exported_name.get() {
            Some(name) => write!(f, "<starlark repository rule {name}>"),
            None => f.write_str("<anonymous starlark repository rule>"),
        }
    }
}

impl fmt::Display for FrozenRepositoryRuleDefinition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.exported_name {
            Some(name) => write!(f, "<starlark repository rule {name}>"),
            None => f.write_str("<anonymous starlark repository rule>"),
        }
    }
}

impl FrozenRepositoryRuleDefinition {
    pub(crate) fn projection(&self) -> Option<RepositoryRuleDefinitionProjection> {
        self.exported_name
            .as_ref()
            .map(|exported_name| RepositoryRuleDefinitionProjection {
                defining_label: self.defining_label.clone(),
                exported_name: exported_name.clone(),
                attributes: self.attributes.clone(),
            })
    }

    pub(crate) fn implementation(&self) -> FrozenValue {
        self.implementation
    }
}

impl<'v> Freeze for RepositoryRuleDefinition<'v> {
    type Frozen = FrozenRepositoryRuleDefinition;

    fn freeze(self, freezer: &Freezer) -> FreezeResult<Self::Frozen> {
        Ok(FrozenRepositoryRuleDefinition {
            implementation: self.implementation.freeze(freezer)?,
            defining_label: self.defining_label,
            attributes: self.attributes,
            exported_name: self.exported_name.into_inner(),
        })
    }
}

#[starlark_value(type = "repository_rule")]
impl<'v> StarlarkValue<'v> for RepositoryRuleDefinition<'v> {
    type Canonical = FrozenRepositoryRuleDefinition;

    fn export_as(
        &self,
        variable_name: &str,
        _eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<()> {
        if self.exported_name.get().is_none() {
            let _ = self.exported_name.set(variable_name.into());
        }
        Ok(())
    }

    fn invoke(
        &self,
        _me: Value<'v>,
        _args: &Arguments<'v, '_>,
        _eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        Err(starlark::Error::new_other(anyhow::anyhow!(
            "repository_rule definitions may only be called after their .bzl module is frozen"
        )))
    }
}

#[starlark_value(type = "repository_rule")]
impl<'v> StarlarkValue<'v> for FrozenRepositoryRuleDefinition {
    type Canonical = Self;

    fn invoke(
        &self,
        _me: Value<'v>,
        args: &Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        args.no_positional_args(eval.heap())?;
        let state = RepositoryRuleInvocationState::from_evaluator(eval)
            .map_err(starlark::Error::new_other)?;
        let exported_name = self.exported_name.as_ref().ok_or_else(|| {
            starlark::Error::new_other(anyhow::anyhow!(
                "attempting to instantiate a non-exported repository rule"
            ))
        })?;
        let names = args.names_map()?;
        let name_value = names
            .iter()
            .find_map(|(key, value)| (key.as_str() == "name").then_some(*value))
            .unwrap_or_else(Value::new_none);
        let name = name_value.unpack_str().ok_or_else(|| {
            starlark::Error::new_other(anyhow::anyhow!(
                "expected string for attribute 'name', got '{}'",
                name_value.get_type()
            ))
        })?;
        validate_repository_name(name).map_err(starlark::Error::new_other)?;
        if state.contains_name(name) {
            return Err(starlark::Error::new_other(anyhow::anyhow!(
                "A repo named {name} is already generated by this module extension"
            )));
        }
        let caller = eval.call_stack_top_location().ok_or_else(|| {
            starlark::Error::new_other(anyhow::anyhow!(
                "repository-rule call has no Starlark caller location"
            ))
        })?;
        let caller = project_span(&caller).map_err(starlark::Error::new_other)?;
        let mut stack = eval.call_stack().into_frames();
        stack.pop();
        let stack = stack
            .into_iter()
            .map(|frame| {
                anyhow::Ok(RepositoryRuleCallFrame {
                    function: frame.name.into(),
                    location: frame.location.as_ref().map(project_span).transpose()?,
                })
            })
            .collect::<anyhow::Result<Vec<_>>>()
            .map_err(starlark::Error::new_other)?
            .into();
        let kwargs = names
            .iter()
            .map(|(key, value)| {
                Ok((
                    CompactString::new(key.as_str()),
                    project_call_value(*value)?,
                ))
            })
            .collect::<anyhow::Result<Vec<_>>>()
            .map_err(starlark::Error::new_other)?;
        state.push(RepositoryRuleCallRecord {
            definition: RepositoryRuleDefinitionProjection {
                defining_label: self.defining_label.clone(),
                exported_name: exported_name.clone(),
                attributes: self.attributes.clone(),
            },
            name: name.into(),
            kwargs: kwargs.into(),
            caller,
            stack,
        });
        Ok(Value::new_none())
    }
}

fn project_span(span: &starlark::codemap::FileSpan) -> anyhow::Result<RepositoryRuleCallSpan> {
    let resolved = span.resolve_span_for_reporting();
    Ok(RepositoryRuleCallSpan {
        file: span.filename().into(),
        start_line: u32::try_from(resolved.begin.line + 1)?,
        start_column: u32::try_from(resolved.begin.column + 1)?,
        end_line: u32::try_from(resolved.end.line + 1)?,
        end_column: u32::try_from(resolved.end.column + 1)?,
    })
}

fn project_call_value(value: Value<'_>) -> anyhow::Result<RepositoryRuleCallValue> {
    if value.is_none() {
        return Ok(RepositoryRuleCallValue::None);
    }
    if let Some(value) = value.unpack_bool() {
        return Ok(RepositoryRuleCallValue::Bool(value));
    }
    if let Some(value) = value.unpack_i32() {
        return Ok(RepositoryRuleCallValue::Int(value));
    }
    if let Some(value) = value.unpack_str() {
        return Ok(RepositoryRuleCallValue::String(value.into()));
    }
    if let Some(label) = StarlarkLabel::from_value(value) {
        return Ok(RepositoryRuleCallValue::Label(label.canonical().clone()));
    }
    anyhow::bail!(
        "unexpected Starlark value: {} (of type {})",
        value.to_repr(),
        value.get_type()
    )
}

fn validate_repository_name(name: &str) -> anyhow::Result<()> {
    let valid = name.bytes().enumerate().all(|(index, byte)| {
        byte.is_ascii_alphanumeric() || (index > 0 && matches!(byte, b'-' | b'_' | b'.'))
    });
    if !valid || name.is_empty() {
        anyhow::bail!(
            "invalid user-provided repo name '{name}': valid names may contain only A-Z, a-z, 0-9, '-', '_', '.', and must start with a letter or a number"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use starlark::environment::FrozenModule;
    use starlark::environment::Module;
    use starlark::syntax::AstModule;
    use starlark::syntax::Dialect;

    use super::*;
    use crate::package::loading_globals;
    use crate::provider::BzlEvaluationContext;

    fn load(source: &str) -> Result<FrozenModule, String> {
        let ast = AstModule::parse("//:ext.bzl", source.to_owned(), &Dialect::Standard)
            .map_err(|error| error.to_string())?;
        let module = Module::new();
        let context = BzlEvaluationContext::new("//:ext.bzl");
        let mut evaluator = Evaluator::new(&module);
        evaluator.extra = Some(&context);
        evaluator
            .eval_module(ast, &loading_globals())
            .map_err(|error| error.to_string())?;
        drop(evaluator);
        module.freeze().map_err(|error| format!("{error:?}"))
    }

    fn invoke(
        loaded: &FrozenModule,
        function: &str,
        arguments: impl FnOnce(&Module) -> Vec<Value<'_>>,
    ) -> (Result<String, String>, Arc<[RepositoryRuleCallRecord]>) {
        let function = loaded.get(function).unwrap();
        let module = Module::new();
        let function = function.owned_value(module.frozen_heap());
        let arguments = arguments(&module);
        let state = RepositoryRuleInvocationState::new();
        let mut evaluator = Evaluator::new(&module);
        evaluator.extra = Some(&state);
        let result = evaluator
            .eval_function(function, &arguments, &[])
            .map(|value| value.to_repr())
            .map_err(|error| error.to_string());
        drop(evaluator);
        (result, state.records())
    }

    const BASE: &str = r#"
def _repo_impl(ctx):
    pass
_repo = repository_rule(
    _repo_impl,
    attrs = {
        "text": attr.string(mandatory = True),
        "enabled": attr.bool(default = True),
        "count": attr.int(default = 3),
        "target": attr.label(default = None),
    },
)
"#;

    #[test]
    fn definition_and_scalar_capture_preserve_order_identity_and_provenance() {
        let loaded = load(&format!(
            "{BASE}\ndef run(label):\n    _repo(name='first', extra='x', enabled=False, count=7, target=label)\n"
        ))
        .unwrap();
        let (result, records) = invoke(&loaded, "run", |module| {
            vec![module.heap().alloc_simple(StarlarkLabel::new(
                CanonicalLabel::parse("@@dep+//pkg:item").unwrap(),
            ))]
        });
        assert_eq!(result.unwrap(), "None");
        assert_eq!(records.len(), 1);
        let record = &records[0];
        assert_eq!(record.name, "first");
        assert_eq!(
            record
                .kwargs
                .iter()
                .map(|(name, _)| name.as_str())
                .collect::<Vec<_>>(),
            ["name", "extra", "enabled", "count", "target"]
        );
        assert_eq!(
            record.kwargs[4].1,
            RepositoryRuleCallValue::Label(CanonicalLabel::parse("@@dep+//pkg:item").unwrap())
        );
        assert_eq!(record.definition.defining_label.to_string(), "@@//:ext.bzl");
        assert_eq!(record.definition.exported_name, "_repo");
        assert_eq!(
            record
                .definition
                .attributes
                .iter()
                .map(|attribute| attribute.name.as_str())
                .collect::<Vec<_>>(),
            ["text", "enabled", "count", "target"]
        );
        assert_eq!(record.caller.file, "//:ext.bzl");
        assert!(record.caller.start_line > 0);
        assert!(record.stack.iter().any(|frame| frame.function == "run"));
        assert!(
            record
                .stack
                .iter()
                .all(|frame| frame.function != "<native>")
        );
    }

    #[test]
    fn calls_are_atomic_ordered_namespaced_and_retain_error_prefix() {
        let loaded = load(&format!(
            "{BASE}\ndef ordered():\n    _repo(name='one', value=None)\n    _repo(name='two', value=True)\n\ndef duplicate():\n    _repo(name='same')\n    _repo(name='same', bad=[])\n\ndef throws():\n    _repo(name='before')\n    fail('boom')\n"
        ))
        .unwrap();
        let (ordered, records) = invoke(&loaded, "ordered", |_| Vec::new());
        assert_eq!(ordered.unwrap(), "None");
        assert_eq!(
            records
                .iter()
                .map(|record| record.name.as_str())
                .collect::<Vec<_>>(),
            ["one", "two"]
        );
        assert_eq!(records[0].kwargs[1].1, RepositoryRuleCallValue::None);
        assert_eq!(records[1].kwargs[1].1, RepositoryRuleCallValue::Bool(true));

        let (duplicate, records) = invoke(&loaded, "duplicate", |_| Vec::new());
        assert!(duplicate.unwrap_err().contains("already generated"));
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].name, "same");

        let (throws, records) = invoke(&loaded, "throws", |_| Vec::new());
        assert!(throws.unwrap_err().contains("boom"));
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].name, "before");

        let (_, fresh) = invoke(&loaded, "ordered", |_| Vec::new());
        assert_eq!(
            fresh.len(),
            2,
            "a fresh extension context owns a fresh namespace"
        );
    }

    #[test]
    fn context_export_name_and_values_fail_in_pinned_order() {
        let loaded = load(&format!(
            "{BASE}\ndef positional():\n    _repo('x')\n\ndef missing():\n    _repo()\n\ndef typed():\n    _repo(name=1)\n\ndef bad_name():\n    _repo(name='+bad')\n\ndef bad_value():\n    _repo(name='valid', bad=[])\n"
        ))
        .unwrap();
        for (function, expected) in [
            ("positional", "positional"),
            ("missing", "expected string for attribute 'name'"),
            ("typed", "expected string for attribute 'name'"),
        ] {
            let (error, records) = invoke(&loaded, function, |_| Vec::new());
            assert!(error.unwrap_err().contains(expected));
            assert!(records.is_empty());
        }
        let (bad_name, records) = invoke(&loaded, "bad_name", |_| Vec::new());
        assert!(
            bad_name
                .unwrap_err()
                .contains("invalid user-provided repo name")
        );
        assert!(records.is_empty());
        let (bad_value, records) = invoke(&loaded, "bad_value", |_| Vec::new());
        assert!(bad_value.unwrap_err().contains("unexpected Starlark value"));
        assert!(records.is_empty());

        let anonymous = load(
            "def impl(ctx):\n  pass\nrules=[repository_rule(impl)]\ndef run():\n  rules[0](name='x')\n",
        )
        .unwrap();
        let (error, records) = invoke(&anonymous, "run", |_| Vec::new());
        assert!(error.unwrap_err().contains("non-exported repository rule"));
        assert!(records.is_empty());

        let public = load("def impl(ctx): pass\nrepo=repository_rule(impl)\n").unwrap();
        let repo = public.get("repo").unwrap();
        let module = Module::new();
        let name = module.heap().alloc_str("x").to_value();
        let mut evaluator = Evaluator::new(&module);
        let error = evaluator
            .eval_function(
                repo.owned_value(module.frozen_heap()),
                &[],
                &[("name", name)],
            )
            .unwrap_err()
            .to_string();
        assert!(error.contains("only be called from within module extension"));

        let rejected = load(&format!(
            "{BASE}\ndef run():\n    value=lambda: None\n    _repo(name='valid', bad=value)\n"
        ))
        .unwrap();
        for source in [
            "def run():\n  _repo(name='valid', bad=(1,))\n",
            "def run():\n  _repo(name='valid', bad={'x':1})\n",
            "def run():\n  _repo(name='valid', bad=123456789012345678901234567890)\n",
        ] {
            let loaded = load(&format!("{BASE}\n{source}")).unwrap();
            let (error, records) = invoke(&loaded, "run", |_| Vec::new());
            assert!(error.unwrap_err().contains("unexpected Starlark value"));
            assert!(records.is_empty());
        }
        let (error, records) = invoke(&rejected, "run", |_| Vec::new());
        assert!(error.unwrap_err().contains("unexpected Starlark value"));
        assert!(records.is_empty());
    }

    #[test]
    fn definition_surface_accepts_defaults_and_rejects_deferred_families() {
        for source in [
            "def impl(ctx): pass\nr=repository_rule(impl)\n",
            "def impl(ctx): pass\nr=repository_rule(implementation=impl, attrs=None, local=False, configure=False, environ=[], doc=None)\n",
        ] {
            load(source).unwrap();
        }
        for (source, expected) in [
            (
                "r=repository_rule(1)\n",
                "repository_rule implementation must be callable",
            ),
            (
                "def impl(ctx): pass\nr=repository_rule(impl, local=True)\n",
                "unsupported repository_rule option",
            ),
            (
                "def impl(ctx): pass\nr=repository_rule(impl, configure=True)\n",
                "unsupported repository_rule option",
            ),
            (
                "def impl(ctx): pass\nr=repository_rule(impl, environ=['X'])\n",
                "unsupported repository_rule option",
            ),
            (
                "def impl(ctx): pass\nr=repository_rule(impl, doc='x')\n",
                "unsupported repository_rule option",
            ),
            (
                "def impl(ctx): pass\nr=repository_rule(impl, attrs={'name':attr.string()})\n",
                "built-in attribute 'name'",
            ),
            (
                "def impl(ctx): pass\nr=repository_rule(impl, attrs={'tags':attr.string()})\n",
                "built-in attribute 'tags'",
            ),
            (
                "def impl(ctx): pass\nr=repository_rule(impl, attrs={'deprecation':attr.string()})\n",
                "built-in attribute 'deprecation'",
            ),
            (
                "def impl(ctx): pass\nr=repository_rule(impl, attrs={'visibility':attr.string()})\n",
                "built-in attribute 'visibility'",
            ),
            (
                "def impl(ctx): pass\nr=repository_rule(impl, attrs={1:attr.string()})\n",
                "attr names must be strings",
            ),
            (
                "def impl(ctx): pass\nr=repository_rule(impl, attrs={'x':'bad'})\n",
                "must use attr.*()",
            ),
            (
                "def impl(ctx): pass\nr=repository_rule(impl, attrs={'_private':attr.string()})\n",
                "unsupported repository_rule attribute name",
            ),
            (
                "def impl(ctx): pass\nr=repository_rule(impl, attrs={'xs':attr.string_list()})\n",
                "unsupported repository_rule attribute schema",
            ),
            (
                "def impl(ctx): pass\nr=repository_rule(impl, attrs={'x':attr.string(configurable=False)})\n",
                "unsupported repository_rule attribute schema",
            ),
            (
                "def impl(ctx): pass\nr=repository_rule(impl, attrs={'x':attr.label(allow_single_file=True)})\n",
                "unsupported repository_rule attribute schema",
            ),
        ] {
            let error = load(source).unwrap_err();
            assert!(error.contains(expected), "{error}");
        }
    }
}
