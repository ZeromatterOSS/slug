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
use slug_bzlmod_v2::OverrideAttributeKey;
use slug_bzlmod_v2::OverrideAttributeValue;
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
use starlark::values::list::AllocList;
use starlark::values::none::NoneType;
use starlark::values::starlark_value;
use starlark_map::small_map::SmallMap;

use crate::attrs::AttributeKind;
use crate::attrs::CoercedAttributeValue;
use crate::module_extension_repository_rule::RepositoryRuleAttribute;
use crate::starlark_label::StarlarkLabel;

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct RepositoryRuleInvocationInput {
    name: CompactString,
    original_name: Option<CompactString>,
    attributes: Arc<SmallMap<CompactString, OverrideAttributeValue>>,
    declaration: Arc<[RepositoryRuleAttribute]>,
}

impl RepositoryRuleInvocationInput {
    pub(crate) fn new(
        name: CompactString,
        original_name: Option<CompactString>,
        attributes: Arc<SmallMap<CompactString, OverrideAttributeValue>>,
        declaration: Arc<[RepositoryRuleAttribute]>,
    ) -> Result<Self, CompactString> {
        for (index, attribute) in declaration.iter().enumerate() {
            if declaration[..index]
                .iter()
                .any(|previous| previous.name == attribute.name)
            {
                return Err(
                    format!("duplicate repository-rule attribute '{}'", attribute.name).into(),
                );
            }
        }
        for (name, value) in attributes.iter() {
            if matches!(value, OverrideAttributeValue::None) {
                continue;
            }
            let attribute = declaration
                .iter()
                .find(|attribute| attribute.name == *name)
                .ok_or_else(|| {
                    CompactString::from(format!("unknown repository-rule attribute '{name}'"))
                })?;
            if !matches_override(attribute.kind, value) {
                return Err(
                    format!("repository-rule attribute '{name}' has the wrong kind").into(),
                );
            }
        }
        for attribute in declaration.iter() {
            let supplied = attributes
                .get(&attribute.name)
                .filter(|value| !matches!(value, OverrideAttributeValue::None));
            if supplied.is_none() && attribute.mandatory {
                return Err(format!(
                    "mandatory repository-rule attribute '{}' isn't being specified",
                    attribute.name
                )
                .into());
            }
            if let Some(default) = attribute.default.as_ref() {
                if !matches_coerced(attribute.kind, default) {
                    return Err(format!(
                        "default for repository-rule attribute '{}' has the wrong kind",
                        attribute.name
                    )
                    .into());
                }
            }
        }
        Ok(Self {
            name,
            original_name,
            attributes,
            declaration,
        })
    }

    fn original_name(&self) -> &str {
        self.original_name
            .as_deref()
            .filter(|name| !name.is_empty())
            .unwrap_or(&self.name)
    }

    fn attribute(&self, name: &str) -> Option<RepositoryAttributeValueRef<'_>> {
        if name == "name" {
            return Some(RepositoryAttributeValueRef::String(&self.name));
        }
        let attribute = self
            .declaration
            .iter()
            .find(|attribute| attribute.name == name)?;
        self.attributes
            .get(name)
            .filter(|value| !matches!(value, OverrideAttributeValue::None))
            .map(RepositoryAttributeValueRef::from_override)
            .or_else(|| {
                attribute
                    .default
                    .as_ref()
                    .map(RepositoryAttributeValueRef::from_coerced)
            })
            .or(Some(RepositoryAttributeValueRef::implicit(attribute.kind)))
    }
}

#[derive(Clone, Copy)]
enum RepositoryAttributeValueRef<'a> {
    None,
    Bool(bool),
    Int(i32),
    String(&'a str),
    Label(&'a slug_identity_v2::CanonicalLabel),
    Iterable(RepositoryAttributeIterableRef<'a>),
    Map(RepositoryAttributeMapRef<'a>),
}

#[derive(Clone, Copy)]
enum RepositoryAttributeIterableRef<'a> {
    Override(&'a [OverrideAttributeValue]),
    Integers(&'a [i32]),
    Strings(&'a [CompactString]),
    Labels(&'a [slug_identity_v2::CanonicalLabel]),
    Empty,
}

enum RepositoryAttributeIterableIter<'a> {
    Override(std::slice::Iter<'a, OverrideAttributeValue>),
    Integers(std::slice::Iter<'a, i32>),
    Strings(std::slice::Iter<'a, CompactString>),
    Labels(std::slice::Iter<'a, slug_identity_v2::CanonicalLabel>),
    Empty,
}

impl<'a> IntoIterator for RepositoryAttributeIterableRef<'a> {
    type Item = RepositoryAttributeValueRef<'a>;
    type IntoIter = RepositoryAttributeIterableIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            Self::Override(values) => RepositoryAttributeIterableIter::Override(values.iter()),
            Self::Integers(values) => RepositoryAttributeIterableIter::Integers(values.iter()),
            Self::Strings(values) => RepositoryAttributeIterableIter::Strings(values.iter()),
            Self::Labels(values) => RepositoryAttributeIterableIter::Labels(values.iter()),
            Self::Empty => RepositoryAttributeIterableIter::Empty,
        }
    }
}

impl<'a> Iterator for RepositoryAttributeIterableIter<'a> {
    type Item = RepositoryAttributeValueRef<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Override(values) => values
                .next()
                .map(RepositoryAttributeValueRef::from_override),
            Self::Integers(values) => values.next().copied().map(RepositoryAttributeValueRef::Int),
            Self::Strings(values) => values
                .next()
                .map(|value| RepositoryAttributeValueRef::String(value)),
            Self::Labels(values) => values.next().map(RepositoryAttributeValueRef::Label),
            Self::Empty => None,
        }
    }
}

#[derive(Clone, Copy)]
enum RepositoryAttributeMapRef<'a> {
    Override(&'a SmallMap<OverrideAttributeKey, OverrideAttributeValue>),
    StringString(&'a [(CompactString, CompactString)]),
    StringStrings(&'a [(CompactString, Arc<[CompactString]>)]),
    StringLabel(&'a [(CompactString, slug_identity_v2::CanonicalLabel)]),
    LabelString(&'a [(slug_identity_v2::CanonicalLabel, CompactString)]),
    StringLabels(&'a [(CompactString, Arc<[slug_identity_v2::CanonicalLabel]>)]),
    Empty,
}

enum RepositoryAttributeMapIter<'a> {
    Override(starlark_map::small_map::Iter<'a, OverrideAttributeKey, OverrideAttributeValue>),
    StringString(std::slice::Iter<'a, (CompactString, CompactString)>),
    StringStrings(std::slice::Iter<'a, (CompactString, Arc<[CompactString]>)>),
    StringLabel(std::slice::Iter<'a, (CompactString, slug_identity_v2::CanonicalLabel)>),
    LabelString(std::slice::Iter<'a, (slug_identity_v2::CanonicalLabel, CompactString)>),
    StringLabels(std::slice::Iter<'a, (CompactString, Arc<[slug_identity_v2::CanonicalLabel]>)>),
    Empty,
}

impl<'a> IntoIterator for RepositoryAttributeMapRef<'a> {
    type Item = (
        RepositoryAttributeValueRef<'a>,
        RepositoryAttributeValueRef<'a>,
    );
    type IntoIter = RepositoryAttributeMapIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            Self::Override(values) => RepositoryAttributeMapIter::Override(values.iter()),
            Self::StringString(values) => RepositoryAttributeMapIter::StringString(values.iter()),
            Self::StringStrings(values) => RepositoryAttributeMapIter::StringStrings(values.iter()),
            Self::StringLabel(values) => RepositoryAttributeMapIter::StringLabel(values.iter()),
            Self::LabelString(values) => RepositoryAttributeMapIter::LabelString(values.iter()),
            Self::StringLabels(values) => RepositoryAttributeMapIter::StringLabels(values.iter()),
            Self::Empty => RepositoryAttributeMapIter::Empty,
        }
    }
}

impl<'a> Iterator for RepositoryAttributeMapIter<'a> {
    type Item = (
        RepositoryAttributeValueRef<'a>,
        RepositoryAttributeValueRef<'a>,
    );

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Override(values) => values.next().map(|(key, value)| {
                let key = match key {
                    OverrideAttributeKey::String(value) => {
                        RepositoryAttributeValueRef::String(value)
                    }
                    OverrideAttributeKey::Label(value) => RepositoryAttributeValueRef::Label(value),
                };
                (key, RepositoryAttributeValueRef::from_override(value))
            }),
            Self::StringString(values) => values.next().map(|(key, value)| {
                (
                    RepositoryAttributeValueRef::String(key),
                    RepositoryAttributeValueRef::String(value),
                )
            }),
            Self::StringStrings(values) => values.next().map(|(key, value)| {
                (
                    RepositoryAttributeValueRef::String(key),
                    RepositoryAttributeValueRef::Iterable(RepositoryAttributeIterableRef::Strings(
                        value,
                    )),
                )
            }),
            Self::StringLabel(values) => values.next().map(|(key, value)| {
                (
                    RepositoryAttributeValueRef::String(key),
                    RepositoryAttributeValueRef::Label(value),
                )
            }),
            Self::LabelString(values) => values.next().map(|(key, value)| {
                (
                    RepositoryAttributeValueRef::Label(key),
                    RepositoryAttributeValueRef::String(value),
                )
            }),
            Self::StringLabels(values) => values.next().map(|(key, value)| {
                (
                    RepositoryAttributeValueRef::String(key),
                    RepositoryAttributeValueRef::Iterable(RepositoryAttributeIterableRef::Labels(
                        value,
                    )),
                )
            }),
            Self::Empty => None,
        }
    }
}

impl<'a> RepositoryAttributeValueRef<'a> {
    fn from_override(value: &'a OverrideAttributeValue) -> Self {
        match value {
            OverrideAttributeValue::None => Self::None,
            OverrideAttributeValue::Bool(value) => Self::Bool(*value),
            OverrideAttributeValue::Int(value) => Self::Int(*value),
            OverrideAttributeValue::String(value) => Self::String(value),
            OverrideAttributeValue::Label(value) => Self::Label(value),
            OverrideAttributeValue::Iterable(values) => {
                Self::Iterable(RepositoryAttributeIterableRef::Override(values))
            }
            OverrideAttributeValue::Map(values) => {
                Self::Map(RepositoryAttributeMapRef::Override(values))
            }
        }
    }

    fn from_coerced(value: &'a CoercedAttributeValue) -> Self {
        match value {
            CoercedAttributeValue::None => Self::None,
            CoercedAttributeValue::Boolean(value) => Self::Bool(*value),
            CoercedAttributeValue::Integer(value) => Self::Int(*value),
            CoercedAttributeValue::IntegerList(values) => {
                Self::Iterable(RepositoryAttributeIterableRef::Integers(values))
            }
            CoercedAttributeValue::String(value) => Self::String(value),
            CoercedAttributeValue::Label(value) | CoercedAttributeValue::Output(value) => {
                Self::Label(value)
            }
            CoercedAttributeValue::StringList(values) => {
                Self::Iterable(RepositoryAttributeIterableRef::Strings(values))
            }
            CoercedAttributeValue::LabelList(values)
            | CoercedAttributeValue::OutputList(values) => {
                Self::Iterable(RepositoryAttributeIterableRef::Labels(values))
            }
            CoercedAttributeValue::StringDict(values) => {
                Self::Map(RepositoryAttributeMapRef::StringString(values))
            }
            CoercedAttributeValue::StringListDict(values) => {
                Self::Map(RepositoryAttributeMapRef::StringStrings(values))
            }
            CoercedAttributeValue::StringKeyedLabelDict(values) => {
                Self::Map(RepositoryAttributeMapRef::StringLabel(values))
            }
            CoercedAttributeValue::LabelKeyedStringDict(values) => {
                Self::Map(RepositoryAttributeMapRef::LabelString(values))
            }
            CoercedAttributeValue::LabelListDict(values) => {
                Self::Map(RepositoryAttributeMapRef::StringLabels(values))
            }
            CoercedAttributeValue::Selector { .. } | CoercedAttributeValue::Concatenation(_, _) => {
                unreachable!("repository-rule preflight rejects configured attributes")
            }
        }
    }

    fn implicit(kind: AttributeKind) -> Self {
        match kind {
            AttributeKind::String => Self::String(""),
            AttributeKind::Boolean => Self::Bool(false),
            AttributeKind::Integer => Self::Int(0),
            AttributeKind::Label | AttributeKind::Output => Self::None,
            AttributeKind::IntegerList
            | AttributeKind::LabelList
            | AttributeKind::StringList
            | AttributeKind::OutputList => Self::Iterable(RepositoryAttributeIterableRef::Empty),
            AttributeKind::StringDict
            | AttributeKind::StringListDict
            | AttributeKind::StringKeyedLabelDict
            | AttributeKind::LabelKeyedStringDict
            | AttributeKind::LabelListDict => Self::Map(RepositoryAttributeMapRef::Empty),
        }
    }
}

#[rustfmt::skip]
fn matches_override(kind: AttributeKind, value: &OverrideAttributeValue) -> bool { match kind {
    AttributeKind::String => matches!(value, OverrideAttributeValue::String(_)), AttributeKind::Boolean => matches!(value, OverrideAttributeValue::Bool(_)), AttributeKind::Integer => matches!(value, OverrideAttributeValue::Int(_)), AttributeKind::Label | AttributeKind::Output => matches!(value, OverrideAttributeValue::Label(_)),
    AttributeKind::IntegerList => matches!(value, OverrideAttributeValue::Iterable(values) if values.iter().all(|value| matches_override(AttributeKind::Integer, value))), AttributeKind::StringList => matches!(value, OverrideAttributeValue::Iterable(values) if values.iter().all(|value| matches_override(AttributeKind::String, value))), AttributeKind::LabelList | AttributeKind::OutputList => matches!(value, OverrideAttributeValue::Iterable(values) if values.iter().all(|value| matches_override(AttributeKind::Label, value))),
    AttributeKind::StringDict => matches!(value, OverrideAttributeValue::Map(values) if values.iter().all(|(key, value)| matches!(key, OverrideAttributeKey::String(_)) && matches_override(AttributeKind::String, value))), AttributeKind::StringListDict => matches!(value, OverrideAttributeValue::Map(values) if values.iter().all(|(key, value)| matches!(key, OverrideAttributeKey::String(_)) && matches_override(AttributeKind::StringList, value))), AttributeKind::StringKeyedLabelDict => matches!(value, OverrideAttributeValue::Map(values) if values.iter().all(|(key, value)| matches!(key, OverrideAttributeKey::String(_)) && matches_override(AttributeKind::Label, value))), AttributeKind::LabelKeyedStringDict => matches!(value, OverrideAttributeValue::Map(values) if values.iter().all(|(key, value)| matches!(key, OverrideAttributeKey::Label(_)) && matches_override(AttributeKind::String, value))), AttributeKind::LabelListDict => matches!(value, OverrideAttributeValue::Map(values) if values.iter().all(|(key, value)| matches!(key, OverrideAttributeKey::String(_)) && matches_override(AttributeKind::LabelList, value))),
} }

#[rustfmt::skip]
fn matches_coerced(kind: AttributeKind, value: &CoercedAttributeValue) -> bool { matches!((kind, value),
    (AttributeKind::String, CoercedAttributeValue::String(_)) | (AttributeKind::Boolean, CoercedAttributeValue::Boolean(_)) | (AttributeKind::Integer, CoercedAttributeValue::Integer(_)) | (AttributeKind::IntegerList, CoercedAttributeValue::IntegerList(_)) | (AttributeKind::Label, CoercedAttributeValue::None | CoercedAttributeValue::Label(_)) | (AttributeKind::Output, CoercedAttributeValue::None | CoercedAttributeValue::Output(_)) | (AttributeKind::StringList, CoercedAttributeValue::StringList(_)) | (AttributeKind::LabelList, CoercedAttributeValue::LabelList(_)) | (AttributeKind::OutputList, CoercedAttributeValue::OutputList(_)) | (AttributeKind::StringDict, CoercedAttributeValue::StringDict(_)) | (AttributeKind::StringListDict, CoercedAttributeValue::StringListDict(_)) | (AttributeKind::StringKeyedLabelDict, CoercedAttributeValue::StringKeyedLabelDict(_)) | (AttributeKind::LabelKeyedStringDict, CoercedAttributeValue::LabelKeyedStringDict(_)) | (AttributeKind::LabelListDict, CoercedAttributeValue::LabelListDict(_))) }
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
struct RepositoryRuleContext { platform: RepositoryPlatform, snapshot: RepositoryEnvironmentSnapshot, input: RepositoryRuleInvocationInput }

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
struct RepositoryRuleAttributes {
    input: RepositoryRuleInvocationInput,
}

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
starlark::starlark_simple_value!(RepositoryRuleAttributes);

#[starlark_value(type = "repository_ctx")]
#[rustfmt::skip]
impl<'v> StarlarkValue<'v> for RepositoryRuleContext {
    fn get_attr(&self, name: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match name {
            "name" => Some(heap.alloc_str(&self.input.name).to_value()),
            "original_name" => Some(heap.alloc_str(self.input.original_name()).to_value()),
            "attr" => Some(heap.alloc_simple(RepositoryRuleAttributes { input: self.input.clone() })),
            "os" => Some(heap.alloc_simple(RepositoryOs { platform: self.platform.clone(), snapshot: self.snapshot.dupe() })),
            _ => None,
        }
    }

    fn dir_attr(&self) -> Vec<String> { vec!["name".to_owned(), "original_name".to_owned(), "attr".to_owned(), "os".to_owned()] }

    fn get_methods() -> Option<&'static Methods> {
        static METHODS: MethodsStatic = MethodsStatic::new();
        METHODS.methods(repository_rule_context_methods)
    }
}

#[rustfmt::skip]
impl fmt::Display for RepositoryRuleAttributes { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str("<repository_attrs>") } }

#[starlark_value(type = "repository_attrs")]
#[rustfmt::skip]
impl<'v> StarlarkValue<'v> for RepositoryRuleAttributes {
    fn get_attr(&self, name: &str, heap: Heap<'v>) -> Option<Value<'v>> { self.input.attribute(name).map(|value| allocate_repository_attribute_value(value, heap)) }
    fn dir_attr(&self) -> Vec<String> { std::iter::once("name".to_owned()).chain(self.input.declaration.iter().filter(|attribute| attribute.name != "name").map(|attribute| attribute.name.to_string())).collect() }
}

#[rustfmt::skip]
fn allocate_repository_attribute_value<'v>(value: RepositoryAttributeValueRef<'_>, heap: Heap<'v>) -> Value<'v> {
    match value {
        RepositoryAttributeValueRef::None => Value::new_none(),
        RepositoryAttributeValueRef::Bool(value) => Value::new_bool(value),
        RepositoryAttributeValueRef::Int(value) => heap.alloc(value),
        RepositoryAttributeValueRef::String(value) => heap.alloc_str(value).to_value(),
        RepositoryAttributeValueRef::Label(value) => heap.alloc_simple(StarlarkLabel::new(value.clone())),
        RepositoryAttributeValueRef::Iterable(values) => heap.alloc(AllocList(values.into_iter().map(|value| allocate_repository_attribute_value(value, heap)))),
        RepositoryAttributeValueRef::Map(values) => heap.alloc(AllocDict(values.into_iter().map(|(key, value)| (allocate_repository_attribute_value(key, heap), allocate_repository_attribute_value(value, heap))))),
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
    input: RepositoryRuleInvocationInput,
    platform: RepositoryPlatform,
    snapshot: RepositoryEnvironmentSnapshot,
    print_handler: Option<&dyn PrintHandler>,
) -> Result<RepositoryRuleInvocation, RepositoryRuleInvocationError> {
    let invocation_module = Module::new();
    let context = invocation_module.heap().alloc_simple(RepositoryRuleContext { platform, snapshot, input });
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
    use slug_identity_v2::CanonicalLabel;
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
        invoke_input(
            source,
            RepositoryRuleInvocationInput::new(
                "repo".into(),
                None,
                Arc::new(SmallMap::new()),
                Arc::from([]),
            )
            .unwrap(),
        )
    }

    fn invoke_input(
        source: &str,
        input: RepositoryRuleInvocationInput,
    ) -> Result<RepositoryRuleInvocation, RepositoryRuleInvocationError> {
        let (_owner, implementation) = implementation(source);
        invoke_repository_rule(
            implementation,
            input,
            RepositoryPlatform::new("linux", "x86_64"),
            RepositoryEnvironmentSnapshot::empty(),
            None,
        )
    }

    fn schema(
        name: &str,
        kind: AttributeKind,
        mandatory: bool,
        default: Option<CoercedAttributeValue>,
    ) -> RepositoryRuleAttribute {
        RepositoryRuleAttribute {
            name: name.into(),
            kind,
            mandatory,
            default,
        }
    }

    fn attributes(
        values: impl IntoIterator<Item = (CompactString, OverrideAttributeValue)>,
    ) -> Arc<SmallMap<CompactString, OverrideAttributeValue>> {
        let mut result = SmallMap::new();
        for (name, value) in values {
            result.insert(name, value);
        }
        Arc::new(result)
    }

    #[test]
    fn repository_context_projects_all_attribute_kinds_and_reflection() {
        let label = CanonicalLabel::parse("@@dep+//pkg:item").unwrap();
        let output = CanonicalLabel::parse("@@//defs:out").unwrap();
        let values = attributes([
            ("s".into(), OverrideAttributeValue::String("value".into())),
            ("b".into(), OverrideAttributeValue::Bool(true)),
            ("i".into(), OverrideAttributeValue::Int(7)),
            ("l".into(), OverrideAttributeValue::Label(label.clone())),
            (
                "ll".into(),
                OverrideAttributeValue::Iterable(Arc::from([OverrideAttributeValue::Label(
                    label.clone(),
                )])),
            ),
            ("o".into(), OverrideAttributeValue::Label(output.clone())),
            (
                "ol".into(),
                OverrideAttributeValue::Iterable(Arc::from([OverrideAttributeValue::Label(
                    output,
                )])),
            ),
            (
                "sd".into(),
                OverrideAttributeValue::Map(Arc::new(SmallMap::from_iter([(
                    OverrideAttributeKey::String("k".into()),
                    OverrideAttributeValue::String("v".into()),
                )]))),
            ),
            (
                "sld".into(),
                OverrideAttributeValue::Map(Arc::new(SmallMap::from_iter([(
                    OverrideAttributeKey::String("k".into()),
                    OverrideAttributeValue::Iterable(Arc::from([OverrideAttributeValue::String(
                        "v".into(),
                    )])),
                )]))),
            ),
            (
                "skld".into(),
                OverrideAttributeValue::Map(Arc::new(SmallMap::from_iter([(
                    OverrideAttributeKey::String("k".into()),
                    OverrideAttributeValue::Label(label.clone()),
                )]))),
            ),
            (
                "lksd".into(),
                OverrideAttributeValue::Map(Arc::new(SmallMap::from_iter([(
                    OverrideAttributeKey::Label(label.clone()),
                    OverrideAttributeValue::String("v".into()),
                )]))),
            ),
            (
                "lld".into(),
                OverrideAttributeValue::Map(Arc::new(SmallMap::from_iter([(
                    OverrideAttributeKey::String("k".into()),
                    OverrideAttributeValue::Iterable(Arc::from([OverrideAttributeValue::Label(
                        label,
                    )])),
                )]))),
            ),
            (
                "sl".into(),
                OverrideAttributeValue::Iterable(Arc::from([OverrideAttributeValue::String(
                    "v".into(),
                )])),
            ),
        ]);
        let declaration: Arc<[RepositoryRuleAttribute]> = [
            schema("s", AttributeKind::String, false, None),
            schema("b", AttributeKind::Boolean, false, None),
            schema("i", AttributeKind::Integer, false, None),
            schema("l", AttributeKind::Label, false, None),
            schema("ll", AttributeKind::LabelList, false, None),
            schema("o", AttributeKind::Output, false, None),
            schema("ol", AttributeKind::OutputList, false, None),
            schema("sd", AttributeKind::StringDict, false, None),
            schema("sld", AttributeKind::StringListDict, false, None),
            schema("skld", AttributeKind::StringKeyedLabelDict, false, None),
            schema("lksd", AttributeKind::LabelKeyedStringDict, false, None),
            schema("lld", AttributeKind::LabelListDict, false, None),
            schema("sl", AttributeKind::StringList, false, None),
            schema("implicit", AttributeKind::String, false, None),
            schema("ib", AttributeKind::Boolean, false, None),
            schema("ii", AttributeKind::Integer, false, None),
            schema("il", AttributeKind::Label, false, None),
            schema("ill", AttributeKind::LabelList, false, None),
            schema("io", AttributeKind::Output, false, None),
            schema("iol", AttributeKind::OutputList, false, None),
            schema("isd", AttributeKind::StringDict, false, None),
            schema("isld", AttributeKind::StringListDict, false, None),
            schema("iskld", AttributeKind::StringKeyedLabelDict, false, None),
            schema("ilksd", AttributeKind::LabelKeyedStringDict, false, None),
            schema("illd", AttributeKind::LabelListDict, false, None),
            schema(
                "default",
                AttributeKind::String,
                false,
                Some(CoercedAttributeValue::String("default".into())),
            ),
        ]
        .into();
        let input = RepositoryRuleInvocationInput::new(
            "canonical".into(),
            Some("".into()),
            values,
            declaration,
        )
        .unwrap();
        let invocation = invoke_input(r#"
def implementation(ctx):
    if not (ctx.name == "canonical" and ctx.original_name == "canonical" and ctx.attr.name == ctx.name): fail("names")
    if not (hasattr(ctx, "attr") and getattr(ctx, "name") == "canonical" and dir(ctx) == ["attr", "file", "getenv", "name", "original_name", "os"]): fail("context reflection")
    if not (ctx.attr.s == "value" and ctx.attr.b and ctx.attr.i == 7 and type(ctx.attr.l) == "Label"): fail("scalars")
    if not (type(ctx.attr.ll[0]) == "Label" and type(ctx.attr.o) == "Label" and type(ctx.attr.ol[0]) == "Label"): fail("labels")
    if not (ctx.attr.sd == {"k": "v"} and ctx.attr.sld == {"k": ["v"]} and type(ctx.attr.skld["k"]) == "Label"): fail("maps")
    if not (type(ctx.attr.lksd.keys()[0]) == "Label" and type(ctx.attr.lld["k"][0]) == "Label"): fail("keys")
    if not (ctx.attr.sl == ["v"] and ctx.attr.implicit == "" and ctx.attr.ib == False and ctx.attr.ii == 0 and ctx.attr.il == None and ctx.attr.ill == [] and ctx.attr.io == None and ctx.attr.iol == []): fail("implicit scalars")
    if not (ctx.attr.isd == {} and ctx.attr.isld == {} and ctx.attr.iskld == {} and ctx.attr.ilksd == {} and ctx.attr.illd == {} and ctx.attr.default == "default" and "name" in dir(ctx.attr)): fail("implicit maps")
    if not (hasattr(ctx.attr, "default") and getattr(ctx.attr, "default") == "default" and dir(ctx.attr) == ["b", "default", "i", "ib", "ii", "il", "ilksd", "ill", "illd", "implicit", "io", "iol", "isd", "iskld", "isld", "l", "lksd", "ll", "lld", "name", "o", "ol", "s", "sd", "skld", "sl", "sld"]): fail("attribute reflection")
    ctx.file("ok")
"#, input.clone()).unwrap();
        assert_eq!(invocation.plan.effects().len(), 1);
        assert!(matches!(
            invoke_input(
                "def implementation(ctx):\n    ctx.file('staged')\n    return ctx.attr.defaul\n",
                input,
            ),
            Err(RepositoryRuleInvocationError::Evaluation(message))
                if message.contains("default")
        ));
    }

    #[test]
    fn repository_context_projects_all_declared_default_kinds_and_order() {
        let label_z = CanonicalLabel::parse("@@dep+//pkg:z").unwrap();
        let label_a = CanonicalLabel::parse("@@dep+//pkg:a").unwrap();
        let output_z = CanonicalLabel::parse("@@//defs:z.out").unwrap();
        let output_a = CanonicalLabel::parse("@@//defs:a.out").unwrap();
        let declaration: Arc<[RepositoryRuleAttribute]> = [
            schema(
                "s",
                AttributeKind::String,
                false,
                Some(CoercedAttributeValue::String("value".into())),
            ),
            schema(
                "b",
                AttributeKind::Boolean,
                false,
                Some(CoercedAttributeValue::Boolean(true)),
            ),
            schema(
                "i",
                AttributeKind::Integer,
                false,
                Some(CoercedAttributeValue::Integer(7)),
            ),
            schema(
                "l",
                AttributeKind::Label,
                false,
                Some(CoercedAttributeValue::Label(label_z.clone())),
            ),
            schema(
                "o",
                AttributeKind::Output,
                false,
                Some(CoercedAttributeValue::Output(output_z.clone())),
            ),
            schema(
                "sl",
                AttributeKind::StringList,
                false,
                Some(CoercedAttributeValue::StringList(Arc::from([
                    "z".into(),
                    "a".into(),
                ]))),
            ),
            schema(
                "ll",
                AttributeKind::LabelList,
                false,
                Some(CoercedAttributeValue::LabelList(Arc::from([
                    label_z.clone(),
                    label_a.clone(),
                ]))),
            ),
            schema(
                "ol",
                AttributeKind::OutputList,
                false,
                Some(CoercedAttributeValue::OutputList(Arc::from([
                    output_z, output_a,
                ]))),
            ),
            schema(
                "sd",
                AttributeKind::StringDict,
                false,
                Some(CoercedAttributeValue::StringDict(Arc::from([
                    ("z".into(), "last".into()),
                    ("a".into(), "first".into()),
                ]))),
            ),
            schema(
                "sld",
                AttributeKind::StringListDict,
                false,
                Some(CoercedAttributeValue::StringListDict(Arc::from([
                    ("z".into(), Arc::from(["two".into(), "one".into()])),
                    ("a".into(), Arc::from(["first".into()])),
                ]))),
            ),
            schema(
                "skld",
                AttributeKind::StringKeyedLabelDict,
                false,
                Some(CoercedAttributeValue::StringKeyedLabelDict(Arc::from([
                    ("z".into(), label_z.clone()),
                    ("a".into(), label_a.clone()),
                ]))),
            ),
            schema(
                "lksd",
                AttributeKind::LabelKeyedStringDict,
                false,
                Some(CoercedAttributeValue::LabelKeyedStringDict(Arc::from([
                    (label_z.clone(), "last".into()),
                    (label_a.clone(), "first".into()),
                ]))),
            ),
            schema(
                "lld",
                AttributeKind::LabelListDict,
                false,
                Some(CoercedAttributeValue::LabelListDict(Arc::from([
                    ("z".into(), Arc::from([label_z.clone(), label_a.clone()])),
                    ("a".into(), Arc::from([label_a])),
                ]))),
            ),
        ]
        .into();
        let input = RepositoryRuleInvocationInput::new(
            "repo".into(),
            None,
            Arc::new(SmallMap::new()),
            declaration,
        )
        .unwrap();
        invoke_input(
            r#"
def implementation(ctx):
    if not (ctx.attr.s == "value" and ctx.attr.b and ctx.attr.i == 7): fail("default scalars")
    if not (type(ctx.attr.l) == "Label" and str(ctx.attr.l) == "@@dep+//pkg:z" and type(ctx.attr.o) == "Label" and str(ctx.attr.o) == "@@//defs:z.out"): fail("default labels")
    if not (ctx.attr.sl == ["z", "a"] and [str(v) for v in ctx.attr.ll] == ["@@dep+//pkg:z", "@@dep+//pkg:a"] and [str(v) for v in ctx.attr.ol] == ["@@//defs:z.out", "@@//defs:a.out"]): fail("default lists")
    if not (ctx.attr.sd.keys() == ["z", "a"] and ctx.attr.sd["z"] == "last"): fail("default string dict")
    if not (ctx.attr.sld.keys() == ["z", "a"] and ctx.attr.sld["z"] == ["two", "one"]): fail("default nested strings")
    if not (ctx.attr.skld.keys() == ["z", "a"] and str(ctx.attr.skld["z"]) == "@@dep+//pkg:z"): fail("default label values")
    if not ([str(v) for v in ctx.attr.lksd.keys()] == ["@@dep+//pkg:z", "@@dep+//pkg:a"] and ctx.attr.lksd.keys()[0] == ctx.attr.l): fail("default label keys")
    if not (ctx.attr.lld.keys() == ["z", "a"] and [str(v) for v in ctx.attr.lld["z"]] == ["@@dep+//pkg:z", "@@dep+//pkg:a"]): fail("default nested labels")
"#,
            input,
        )
        .unwrap();
    }

    #[test]
    fn repository_context_projects_integer_lists_for_explicit_default_and_implicit_values() {
        let values = SmallMap::from_iter([(
            CompactString::from("explicit"),
            OverrideAttributeValue::Iterable(Arc::from([
                OverrideAttributeValue::Int(1),
                OverrideAttributeValue::Int(-2),
            ])),
        )]);
        let declaration: Arc<[RepositoryRuleAttribute]> = [
            schema("explicit", AttributeKind::IntegerList, false, None),
            schema(
                "defaulted",
                AttributeKind::IntegerList,
                false,
                Some(CoercedAttributeValue::IntegerList(Arc::from([3, 4]))),
            ),
            schema("implicit", AttributeKind::IntegerList, false, None),
        ]
        .into();
        let input =
            RepositoryRuleInvocationInput::new("repo".into(), None, values.into(), declaration)
                .unwrap();
        invoke_input(
            "def implementation(ctx):\n    if ctx.attr.explicit != [1, -2] or ctx.attr.defaulted != [3, 4] or ctx.attr.implicit != []: fail('integer lists')\n    ctx.file('ok')\n",
            input,
        )
        .unwrap();
    }

    #[test]
    fn repository_attribute_preflight_elides_none_before_other_checks() {
        let empty = Arc::from([]);
        let none_unknown = attributes([("unknown".into(), OverrideAttributeValue::None)]);
        assert!(
            RepositoryRuleInvocationInput::new("repo".into(), None, none_unknown, empty).is_ok()
        );
        let declaration: Arc<[RepositoryRuleAttribute]> =
            [schema("needed", AttributeKind::String, true, None)].into();
        let none_needed = attributes([("needed".into(), OverrideAttributeValue::None)]);
        assert!(
            RepositoryRuleInvocationInput::new("repo".into(), None, none_needed, declaration)
                .unwrap_err()
                .contains("mandatory")
        );
        let wrong = attributes([("needed".into(), OverrideAttributeValue::Int(1))]);
        let declaration: Arc<[RepositoryRuleAttribute]> =
            [schema("needed", AttributeKind::String, false, None)].into();
        assert!(
            RepositoryRuleInvocationInput::new("repo".into(), None, wrong, declaration)
                .unwrap_err()
                .contains("wrong kind")
        );
        let optional_none = RepositoryRuleInvocationInput::new(
            "repo".into(),
            None,
            attributes([
                ("defaulted".into(), OverrideAttributeValue::None),
                ("implicit".into(), OverrideAttributeValue::None),
            ]),
            [
                schema(
                    "defaulted",
                    AttributeKind::String,
                    false,
                    Some(CoercedAttributeValue::String("fallback".into())),
                ),
                schema("implicit", AttributeKind::String, false, None),
            ]
            .into(),
        )
        .unwrap();
        invoke_input(
            "def implementation(ctx):\n    if ctx.attr.defaulted != 'fallback' or ctx.attr.implicit != '': fail('None did not fall through')\n",
            optional_none,
        )
        .unwrap();
        let duplicate: Arc<[RepositoryRuleAttribute]> = [
            schema("same", AttributeKind::String, false, None),
            schema("same", AttributeKind::String, false, None),
        ]
        .into();
        assert!(
            RepositoryRuleInvocationInput::new(
                "repo".into(),
                None,
                Arc::new(SmallMap::new()),
                duplicate,
            )
            .unwrap_err()
            .contains("duplicate")
        );
        let wrong_default: Arc<[RepositoryRuleAttribute]> = [schema(
            "value",
            AttributeKind::String,
            false,
            Some(CoercedAttributeValue::Integer(1)),
        )]
        .into();
        assert!(
            RepositoryRuleInvocationInput::new(
                "repo".into(),
                None,
                Arc::new(SmallMap::new()),
                wrong_default,
            )
            .unwrap_err()
            .contains("default")
        );
        let original = RepositoryRuleInvocationInput::new(
            "canonical".into(),
            Some("original".into()),
            Arc::new(SmallMap::new()),
            Arc::from([]),
        )
        .unwrap();
        assert_eq!(original.original_name(), "original");
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
            RepositoryRuleInvocationInput::new(
                "repo".into(),
                None,
                Arc::new(SmallMap::new()),
                Arc::from([]),
            )
            .unwrap(),
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
