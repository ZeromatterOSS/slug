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
use starlark::values::ValueIdentity;
use starlark::values::dict::DictRef;
use starlark::values::list::ListRef;
use starlark::values::starlark_value;
use starlark::values::tuple::TupleRef;
use starlark_map::small_set::SmallSet;

use crate::attrs::AttributeKind;
use crate::attrs::CoercedAttributeValue;
use crate::attrs::FileAdmissibility;
use crate::starlark_label::StarlarkLabel;

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct RepositoryRuleAttribute {
    pub(crate) name: CompactString,
    pub(crate) kind: AttributeKind,
    pub(crate) mandatory: bool,
    pub(crate) default: Option<CoercedAttributeValue>,
    pub(crate) file_admissibility: FileAdmissibility,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct RepositoryRuleDefinitionProjection {
    pub(crate) defining_label: CanonicalLabel,
    pub(crate) exported_name: CompactString,
    pub(crate) attributes: Arc<[RepositoryRuleAttribute]>,
    pub(crate) local: bool,
    pub(crate) configure: bool,
    pub(crate) environment: Arc<SmallSet<CompactString>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) enum RepositoryRuleCallValue {
    None,
    Bool(bool),
    Int(i32),
    String(CompactString),
    Label(CanonicalLabel),
    Sequence(Arc<[RepositoryRuleCallValue]>),
    Map(Arc<[(RepositoryRuleCallKey, RepositoryRuleCallValue)]>),
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) enum RepositoryRuleCallKey {
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

    pub(crate) fn is_active(eval: &Evaluator<'_, '_, '_>) -> bool {
        eval.extra
            .and_then(|extra| extra.downcast_ref::<Self>())
            .is_some()
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
    local: bool,
    #[trace(unsafe_ignore)]
    configure: bool,
    #[trace(unsafe_ignore)]
    environment: Arc<SmallSet<CompactString>>,
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
    local: bool,
    configure: bool,
    environment: Arc<SmallSet<CompactString>>,
    exported_name: Option<CompactString>,
}

starlark::starlark_complex_values!(RepositoryRuleDefinition);

impl<'v> RepositoryRuleDefinition<'v> {
    pub(crate) fn new(
        implementation: Value<'v>,
        defining_label: CanonicalLabel,
        attributes: Arc<[RepositoryRuleAttribute]>,
        local: bool,
        configure: bool,
        environment: Arc<SmallSet<CompactString>>,
    ) -> Self {
        Self {
            implementation,
            defining_label,
            attributes,
            local,
            configure,
            environment,
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
                local: self.local,
                configure: self.configure,
                environment: self.environment.clone(),
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
            local: self.local,
            configure: self.configure,
            environment: self.environment,
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
                local: self.local,
                configure: self.configure,
                environment: self.environment.clone(),
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
    project_call_value_inner(value, &mut Vec::new())
}

fn project_call_value_inner<'v>(
    value: Value<'v>,
    active: &mut Vec<ValueIdentity<'v>>,
) -> anyhow::Result<RepositoryRuleCallValue> {
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
    if let Some(values) = ListRef::from_value(value) {
        return project_call_sequence(value, values.iter(), active);
    }
    if let Some(values) = TupleRef::from_value(value) {
        return values
            .iter()
            .map(|value| project_call_value_inner(value, active))
            .collect::<anyhow::Result<Vec<_>>>()
            .map(|values| RepositoryRuleCallValue::Sequence(values.into()));
    }
    if let Some(values) = DictRef::from_value(value) {
        return project_call_map(value, values.iter(), active);
    }
    anyhow::bail!(
        "unexpected Starlark value: {} (of type {})",
        value.to_repr(),
        value.get_type()
    )
}

fn project_call_sequence<'v>(
    value: Value<'v>,
    values: impl Iterator<Item = Value<'v>>,
    active: &mut Vec<ValueIdentity<'v>>,
) -> anyhow::Result<RepositoryRuleCallValue> {
    let identity = value.identity();
    if active.contains(&identity) {
        anyhow::bail!("unexpected cyclic Starlark container")
    }
    active.push(identity);
    let result = values
        .map(|value| project_call_value_inner(value, active))
        .collect::<anyhow::Result<Vec<_>>>()
        .map(|values| RepositoryRuleCallValue::Sequence(values.into()));
    active.pop();
    result
}

fn project_call_map<'v>(
    value: Value<'v>,
    values: impl Iterator<Item = (Value<'v>, Value<'v>)>,
    active: &mut Vec<ValueIdentity<'v>>,
) -> anyhow::Result<RepositoryRuleCallValue> {
    let identity = value.identity();
    if active.contains(&identity) {
        anyhow::bail!("unexpected cyclic Starlark container")
    }
    active.push(identity);
    let result = values
        .map(|(key, value)| {
            Ok((
                project_call_key(key)?,
                project_call_value_inner(value, active)?,
            ))
        })
        .collect::<anyhow::Result<Vec<_>>>()
        .map(|values| RepositoryRuleCallValue::Map(values.into()));
    active.pop();
    result
}

fn project_call_key(value: Value<'_>) -> anyhow::Result<RepositoryRuleCallKey> {
    if let Some(value) = value.unpack_str() {
        return Ok(RepositoryRuleCallKey::String(value.into()));
    }
    if let Some(label) = StarlarkLabel::from_value(value) {
        return Ok(RepositoryRuleCallKey::Label(label.canonical().clone()));
    }
    anyhow::bail!("repository-rule dictionary keys must be strings or Labels")
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
    use crate::package::FrozenModuleExtensionDefinition;
    use crate::package::build_file_loading_globals;
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

    fn evaluate_build(source: &str) -> Result<(), String> {
        let ast = AstModule::parse("//:BUILD", source.to_owned(), &Dialect::Standard)
            .map_err(|error| error.to_string())?;
        let module = Module::new();
        Evaluator::new(&module)
            .eval_module(ast, &build_file_loading_globals())
            .map(|_| ())
            .map_err(|error| error.to_string())
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

    fn projection(loaded: &FrozenModule, name: &str) -> RepositoryRuleDefinitionProjection {
        loaded
            .get(name)
            .unwrap()
            .downcast::<FrozenRepositoryRuleDefinition>()
            .unwrap()
            .projection()
            .unwrap()
    }

    fn extension_projection(source: &str) -> crate::package::ModuleExtensionDefinitionProjection {
        load(source)
            .unwrap()
            .get("ext")
            .unwrap()
            .downcast::<FrozenModuleExtensionDefinition>()
            .unwrap()
            .projection()
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
    fn native_existing_rules_are_empty_immutable_and_drive_repository_calls() {
        let loaded = load(&format!(
            "{BASE}\ndef run():\n  one=native.existing_rule('missing')\n  many=native.existing_rules()\n  if one != None or len(many) != 0 or 'missing' in many or many:\n    fail('existing-rule no-op contract')\n  _repo(name='created', evidence=(one, many))\n\ndef mutate():\n  many=native.existing_rules()\n  many['late']=1\n  _repo(name='unreachable')\n"
        ))
        .unwrap();

        let (result, records) = invoke(&loaded, "run", |_| Vec::new());
        assert_eq!(result.unwrap(), "None");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].name, "created");
        assert!(matches!(
            &records[0].kwargs[1].1,
            RepositoryRuleCallValue::Sequence(values)
                if matches!(values.as_ref(), [
                    RepositoryRuleCallValue::None,
                    RepositoryRuleCallValue::Map(entries),
                ] if entries.is_empty())
        ));

        let (error, records) = invoke(&loaded, "mutate", |_| Vec::new());
        let error = error.unwrap_err();
        assert!(error.to_ascii_lowercase().contains("immutable"), "{error}");
        assert!(records.is_empty());
    }

    #[test]
    fn native_existing_rule_methods_enforce_signature_and_context() {
        let loaded = load(
            "def missing(): native.existing_rule()\ndef typed(): native.existing_rule(1)\ndef extra(): native.existing_rule('x', 'y')\ndef plural_arg(): native.existing_rules('x')\n",
        )
        .unwrap();
        for function in ["missing", "typed", "extra", "plural_arg"] {
            let (error, records) = invoke(&loaded, function, |_| Vec::new());
            assert!(error.is_err(), "{function} unexpectedly succeeded");
            assert!(records.is_empty());
        }

        for error in [
            load("native.existing_rule('x')\n").unwrap_err(),
            load("native.existing_rules()\n").unwrap_err(),
            evaluate_build("native.existing_rule('x')\n").unwrap_err(),
            evaluate_build("native.existing_rules()\n").unwrap_err(),
        ] {
            assert!(error.contains("only during module extension evaluation"));
        }
    }

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
            "{BASE}\ndef positional():\n    _repo('x')\n\ndef missing():\n    _repo()\n\ndef typed():\n    _repo(name=1)\n\ndef bad_name():\n    _repo(name='+bad')\n\ndef bad_value():\n    _repo(name='valid', bad=set())\n"
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
        for source in ["def run():\n  _repo(name='valid', bad=123456789012345678901234567890)\n"] {
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
    fn declaration_metadata_survives_freeze_export_and_repeated_calls() {
        let defaults = load(
            "def impl(ctx): pass\nr=repository_rule(impl)\ndef run():\n  r(name='one')\n  r(name='two')\n",
        )
        .unwrap();
        let default_projection = projection(&defaults, "r");
        assert!(!default_projection.local);
        assert!(!default_projection.configure);
        assert!(default_projection.environment.is_empty());
        assert_eq!(
            projection(
                &load(
                    "def impl(ctx): pass\nr=repository_rule(impl, local=False, configure=False, environ=[])\n"
                )
                .unwrap(),
                "r"
            ),
            default_projection
        );

        let full = load(
            "def impl(ctx): pass\nr=repository_rule(impl, local=True, configure=True, environ=['B','A','B'], doc='  repository docs\\n  second line')\ndef run():\n  r(name='one')\n  r(name='two')\n",
        )
        .unwrap();
        let full_projection = projection(&full, "r");
        assert!(full_projection.local);
        assert!(full_projection.configure);
        assert_eq!(
            full_projection
                .environment
                .iter()
                .map(CompactString::as_str)
                .collect::<Vec<_>>(),
            ["B", "A"]
        );
        let (result, calls) = invoke(&full, "run", |_| Vec::new());
        assert_eq!(result.unwrap(), "None");
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].definition, full_projection);
        assert_eq!(calls[1].definition, full_projection);
        assert!(Arc::ptr_eq(
            &calls[0].definition.environment,
            &calls[1].definition.environment
        ));
        for doc in ["", ", doc=None"] {
            let variant = load(&format!(
                "def impl(ctx): pass\nr=repository_rule(impl, local=True, configure=True, environ=['B','A','B']{doc})\ndef run():\n  r(name='one')\n  r(name='two')\n"
            ))
            .unwrap();
            let (result, variant_calls) = invoke(&variant, "run", |_| Vec::new());
            assert_eq!(result.unwrap(), "None");
            assert_eq!(variant_calls, calls);
        }

        for source in [
            "def impl(ctx): pass\nr=repository_rule(impl, local=True, configure=True, environ=['B','A'])\n",
            "def impl(ctx): pass\nr=repository_rule(impl, local=True, configure=True, environ=['A','B'])\n",
            "def impl(ctx): pass\nr=repository_rule(impl, local=True, configure=True, environ=['B','A'], doc=None)\n",
            "def impl(ctx): pass\nr=repository_rule(impl, local=True, configure=True, environ=['B','A'], doc='different')\n",
        ] {
            assert_eq!(projection(&load(source).unwrap(), "r"), full_projection);
        }
        let extension_base = extension_projection(
            "def impl(ctx): pass\ntag=tag_class()\next=module_extension(implementation=impl, tag_classes={'tag':tag})\n",
        );
        for source in [
            "def impl(ctx): pass\ntag=tag_class(doc=None)\next=module_extension(implementation=impl, tag_classes={'tag':tag}, doc=None)\n",
            "def impl(ctx): pass\ntag=tag_class(doc='tag docs')\next=module_extension(implementation=impl, tag_classes={'tag':tag}, doc='extension docs')\n",
        ] {
            assert_eq!(extension_projection(source), extension_base);
        }
        for source in [
            "def impl(ctx): pass\nr=repository_rule(impl, local=False, configure=True, environ=['A','B'])\n",
            "def impl(ctx): pass\nr=repository_rule(impl, local=True, configure=False, environ=['A','B'])\n",
            "def impl(ctx): pass\nr=repository_rule(impl, local=True, configure=True, environ=['A','C'])\n",
        ] {
            assert_ne!(projection(&load(source).unwrap(), "r"), full_projection);
        }
    }

    #[test]
    fn file_admissibility_category_survives_freeze_export_and_repeated_calls() {
        let loaded = load(
            r#"
def impl(ctx): fail("repository implementation must stay inert")
r = repository_rule(impl, attrs = {
    "omitted": attr.label(),
    "none": attr.label(allow_files = None),
    "any": attr.label(allow_files = True),
    "false": attr.label(allow_files = False),
    "ordered": attr.label(allow_files = [".a", ".a", ""]),
    "tuple": attr.label(allow_files = (".b", ".a")),
    "empty": attr.label(allow_files = []),
    "build_file": attr.label(allow_single_file = True),
    "single_false": attr.label(allow_single_file = False),
    "single_empty": attr.label(allow_single_file = ()),
    "list": attr.label_list(allow_files = (".list",)),
    "string_keyed": attr.string_keyed_label_dict(allow_files = True),
    "label_keyed": attr.label_keyed_string_dict(allow_files = False),
    "list_dict": attr.label_list_dict(allow_files = []),
})
def run():
    r(name = "one", build_file = "//:BUILD")
    r(name = "two")
"#,
        )
        .unwrap();
        let full_projection = projection(&loaded, "r");
        let policy = |name: &str| {
            full_projection
                .attributes
                .iter()
                .find(|attribute| attribute.name == name)
                .unwrap()
                .file_admissibility
                .clone()
        };
        assert!(policy("omitted").is_no_files());
        assert!(policy("none").is_no_files());
        assert!(policy("any").is_any_file());
        assert!(policy("false").is_no_files());
        assert_eq!(
            policy("ordered")
                .suffixes()
                .unwrap()
                .iter()
                .map(CompactString::as_str)
                .collect::<Vec<_>>(),
            [".a", ".a", ""]
        );
        assert_eq!(
            policy("tuple")
                .suffixes()
                .unwrap()
                .iter()
                .map(CompactString::as_str)
                .collect::<Vec<_>>(),
            [".b", ".a"]
        );
        assert_eq!(policy("empty").suffixes(), Some([].as_slice()));
        assert!(policy("build_file").is_any_file());
        assert!(policy("build_file").single_artifact());
        assert!(policy("single_false").is_no_files());
        assert!(policy("single_false").single_artifact());
        assert_eq!(policy("single_empty").suffixes(), Some([].as_slice()));
        assert!(policy("single_empty").single_artifact());
        assert_eq!(policy("list").suffixes(), Some([".list".into()].as_slice()));
        assert!(policy("string_keyed").is_any_file());
        assert!(policy("label_keyed").is_no_files());
        assert_eq!(policy("list_dict").suffixes(), Some([].as_slice()));

        let (result, calls) = invoke(&loaded, "run", |_| Vec::new());
        assert_eq!(result.unwrap(), "None");
        assert_eq!(calls.len(), 2);
        assert!(Arc::ptr_eq(
            &calls[0].definition.attributes,
            &calls[1].definition.attributes
        ));
        assert!(Arc::ptr_eq(
            &full_projection.attributes,
            &calls[0].definition.attributes
        ));
        assert!(matches!(
            &calls[0].kwargs[1],
            (name, RepositoryRuleCallValue::String(value))
                if name == "build_file" && value == "//:BUILD"
        ));

        let reordered = projection(
            &load("def impl(ctx): pass\nr=repository_rule(impl, attrs={'ordered':attr.label(allow_files=['', '.a', '.a'])})\n").unwrap(),
            "r",
        );
        assert_ne!(full_projection.attributes, reordered.attributes);
    }

    #[test]
    fn definition_surface_accepts_metadata_and_rejects_deferred_families() {
        for source in [
            "def impl(ctx): pass\nr=repository_rule(impl)\n",
            "def impl(ctx): pass\nr=repository_rule(implementation=impl, attrs=None, local=False, configure=False, environ=[], doc=None)\n",
            "def impl(ctx): pass\nr=repository_rule(impl, doc='')\n",
            "def impl(ctx): pass\nr=repository_rule(impl, local=True, configure=True, environ=['B','A','B'])\n",
            "def impl(ctx): pass\nr=repository_rule(impl, attrs={'b':attr.bool(default=True), 'i':attr.int(default=1), 's':attr.string(default='s'), 'l':attr.label(default=':l'), 'o':attr.output(), 'sl':attr.string_list(default=['s']), 'll':attr.label_list(default=[':l']), 'ol':attr.output_list(), 'sd':attr.string_dict(default={'k':'v'}), 'sld':attr.string_list_dict(default={'k':['v']}), 'skld':attr.string_keyed_label_dict(default={'k':':l'}), 'lksd':attr.label_keyed_string_dict(default={':l':'v'}), 'lld':attr.label_list_dict(default={'k':[':l']})})\n",
        ] {
            load(source).unwrap();
        }
        for constructor in [
            "label",
            "label_list",
            "string_keyed_label_dict",
            "label_keyed_string_dict",
            "bool",
            "int",
            "label_list_dict",
            "output",
            "output_list",
            "string",
            "string_list",
            "string_dict",
            "string_list_dict",
        ] {
            load(&format!("x=attr.{constructor}(doc='docs')\n")).unwrap();
        }
        for source in [
            "def impl(ctx): pass\nx=rule(implementation=impl, doc='docs')\n",
            "def impl(target, ctx): return []\nx=aspect(implementation=impl, doc='docs')\n",
            "x=provider(doc='docs')\n",
            "def impl(name, visibility): pass\nx=macro(implementation=impl, doc='docs')\n",
        ] {
            load(source).unwrap();
        }
        for (source, expected) in [
            (
                "r=repository_rule(1)\n",
                "repository_rule implementation must be callable",
            ),
            (
                "def impl(ctx): pass\nr=repository_rule(impl, environ=['X', 1])\n",
                "list[str]",
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
                "def impl(ctx): pass\nr=repository_rule(impl, attrs={'x':attr.string(configurable=False)})\n",
                "unsupported repository_rule attribute schema",
            ),
            (
                "def impl(ctx): pass\nr=repository_rule(impl, attrs={'x':attr.label(allow_files=True, allow_single_file=[])})\n",
                "allow_files and allow_single_file",
            ),
            (
                "def impl(ctx): pass\nr=repository_rule(impl, attrs={'x':attr.label_list(allow_files=1)})\n",
                "allow_files",
            ),
        ] {
            let error = load(source).unwrap_err();
            assert!(error.contains(expected), "{error}");
        }
        for invalid in ["1", "True", "[]", "{}", "lambda: None"] {
            for call in [
                format!("repository_rule(impl, doc={invalid})"),
                format!("tag_class(doc={invalid})"),
                format!("module_extension(implementation=impl, doc={invalid})"),
            ] {
                let error = load(&format!("def impl(ctx): pass\nx={call}\n")).unwrap_err();
                assert!(error.contains("doc") && error.contains("str"), "{error}");
            }
        }
    }

    #[test]
    fn recursive_capture_normalizes_sequences_maps_and_rejects_cycles() {
        let loaded = load(
            "def impl(ctx): pass\nr=repository_rule(impl)\ndef run(label):\n  r(name='ok', xs=(None, True, 7, 's', label, ['nested'], {'z':['a'], label:{'a':'b'}}))\n",
        )
        .unwrap();
        let (result, records) = invoke(&loaded, "run", |module| {
            vec![module.heap().alloc_simple(StarlarkLabel::new(
                CanonicalLabel::parse("@@dep+//p:t").unwrap(),
            ))]
        });
        assert_eq!(result.unwrap(), "None");
        let RepositoryRuleCallValue::Sequence(values) = &records[0].kwargs[1].1 else {
            panic!("tuple must normalize to a sequence")
        };
        assert!(matches!(values[4], RepositoryRuleCallValue::Label(_)));
        assert!(matches!(values[5], RepositoryRuleCallValue::Sequence(_)));
        assert!(matches!(values[6], RepositoryRuleCallValue::Map(_)));

        let copied = load(
            "def impl(ctx): pass\nr=repository_rule(impl)\ndef run():\n  inner=['before']\n  nested={'inner':inner}\n  r(name='copied', value=nested)\n  inner.append('after')\n  nested['late']='added'\n",
        )
        .unwrap();
        let (result, records) = invoke(&copied, "run", |_| Vec::new());
        assert_eq!(result.unwrap(), "None");
        let RepositoryRuleCallValue::Map(entries) = &records[0].kwargs[1].1 else {
            panic!("dictionary must be copied into the map carrier")
        };
        assert_eq!(entries.len(), 1, "post-call dictionary mutation leaked");
        assert!(matches!(
            &entries[0],
            (
                RepositoryRuleCallKey::String(key),
                RepositoryRuleCallValue::Sequence(values)
            ) if key == "inner"
                && matches!(values.as_ref(), [RepositoryRuleCallValue::String(value)] if value == "before")
        ));

        let cyclic = load(
            "def impl(ctx): pass\nr=repository_rule(impl)\ndef run():\n  value=[]\n  value.append(value)\n  r(name='bad', value=value)\n",
        )
        .unwrap();
        let (error, records) = invoke(&cyclic, "run", |_| Vec::new());
        assert!(error.unwrap_err().contains("cyclic"));
        assert!(records.is_empty());

        for source in [
            "def impl(ctx): pass\nr=repository_rule(impl)\ndef run():\n  r(name='bad', value={1:'x'})\n",
            "def impl(ctx): pass\nr=repository_rule(impl)\ndef run():\n  r(name='bad', value=set())\n",
            "def impl(ctx): pass\nr=repository_rule(impl)\ndef run():\n  r(name='bad', value=lambda: None)\n",
        ] {
            let loaded = load(source).unwrap();
            let (error, records) = invoke(&loaded, "run", |_| Vec::new());
            assert!(error.is_err());
            assert!(records.is_empty());
        }
    }
}
