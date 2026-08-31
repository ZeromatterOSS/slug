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
use std::hash::Hash;
#[cfg(test)]
use std::path::PathBuf;
use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use dupe::Dupe;
use slug_build_api_v2::AnalysisDepset;
use slug_build_api_v2::AnalysisDepsetOccurrence;
use slug_build_api_v2::AnalysisValueType;
use slug_build_api_v2::DepsetBuild;
use slug_build_api_v2::DepsetBuildError;
use slug_build_api_v2::DepsetOrder;
use slug_build_api_v2::DepsetSuccessor;
use slug_build_api_v2::DepsetView;
use slug_build_api_v2::ProviderId;
use slug_build_api_v2::ProviderIdentity;
use slug_build_api_v2::build_depset;
use slug_build_api_v2::traverse_depset;
use slug_identity_v2::CanonicalLabel;
use starlark::any::ProvidesStaticType;
use starlark::environment::Methods;
use starlark::environment::MethodsBuilder;
use starlark::environment::MethodsStatic;
use starlark::eval::Arguments;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::values::AllocValue;
use starlark::values::Demand;
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
use starlark::values::ValueIdentity;
use starlark::values::ValueLike;
use starlark::values::dict::DictRef;
use starlark::values::list::ListRef;
use starlark::values::starlark_value;
use starlark::values::structs::StarlarkStructuralValue;
use starlark::values::structs::starlark_structural_is_immutable;
use starlark::values::structs::write_starlark_structural_hash;
use starlark::values::tuple::AllocTuple;
use starlark::values::tuple::TupleRef;
use starlark::values::typing::StarlarkCallable;
use starlark_map::StarlarkHasher;
use starlark_map::small_map::SmallMap;

use crate::bzl_module::BzlLoadManifest;
use crate::bzl_module::BzlModuleIdentity;
use crate::bzl_module::manifest_starlark_sources;
use crate::bzl_visibility::BzlLoadVisibility;
use crate::starlark_label::StarlarkLabel;

pub fn starlark_label(value: Value<'_>) -> Option<CanonicalLabel> {
    StarlarkLabel::from_value(value).map(|value| value.canonical().clone())
}

pub fn alloc_starlark_label<'v>(heap: Heap<'v>, label: CanonicalLabel) -> Value<'v> {
    heap.alloc_simple(StarlarkLabel::new(label))
}

pub fn alloc_frozen_starlark_label(
    heap: &starlark::values::FrozenHeap,
    label: CanonicalLabel,
) -> FrozenValue {
    heap.alloc(StarlarkLabel::new(label))
}

/// A built-in provider identity that is usable as a key but has no Starlark
/// constructor. Bazel prints all built-in provider keys with function syntax,
/// including providers whose Java implementation has no self-call method.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub(crate) struct BuiltinProviderKey {
    name: &'static str,
}

impl BuiltinProviderKey {
    pub(crate) const fn new(name: &'static str) -> Self {
        Self { name }
    }
}

impl fmt::Display for BuiltinProviderKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<function {}>", self.name)
    }
}

starlark::starlark_simple_value!(BuiltinProviderKey);

#[starlark_value(type = "Provider")]
impl<'v> StarlarkValue<'v> for BuiltinProviderKey {
    fn write_hash(&self, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        self.name.hash(hasher);
        Ok(())
    }

    fn equals(&self, other: Value<'v>) -> starlark::Result<bool> {
        Ok(Self::from_value(other).is_some_and(|other| self.name == other.name))
    }
}

/// Fixed `.bzl` declaration token; configured output-group values are deferred.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub(crate) struct OutputGroupInfo;

impl fmt::Display for OutputGroupInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<function OutputGroupInfo>")
    }
}

starlark::starlark_simple_value!(OutputGroupInfo);

#[starlark_value(type = "OutputGroupInfo")]
impl<'v> StarlarkValue<'v> for OutputGroupInfo {
    fn invoke(
        &self,
        _me: Value<'v>,
        _args: &Arguments<'v, '_>,
        _eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        Err(starlark::Error::new_other(anyhow::anyhow!(
            "OutputGroupInfo construction is unsupported during loading"
        )))
    }
}

/// Fixed `.bzl` declaration token; configured environment values are deferred.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub(crate) struct RunEnvironmentInfo;

impl fmt::Display for RunEnvironmentInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<function RunEnvironmentInfo>")
    }
}

starlark::starlark_simple_value!(RunEnvironmentInfo);

#[starlark_value(type = "RunEnvironmentInfo")]
impl<'v> StarlarkValue<'v> for RunEnvironmentInfo {
    fn invoke(
        &self,
        _me: Value<'v>,
        _args: &Arguments<'v, '_>,
        _eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        Err(starlark::Error::new_other(anyhow::anyhow!(
            "RunEnvironmentInfo construction is unsupported during loading"
        )))
    }
}

#[derive(Debug, ProvidesStaticType)]
pub(crate) struct BzlEvaluationContext {
    source_label: CompactString,
    source_identity: BzlModuleIdentity,
    source_identities_by_filename: Arc<[(CompactString, BzlModuleIdentity)]>,
    bzl_load_visibility: RefCell<Option<BzlLoadVisibility>>,
}

impl BzlEvaluationContext {
    #[cfg(test)]
    pub(crate) fn new(source_label: impl Into<CompactString>) -> Self {
        let source_label = source_label.into();
        let canonical = if source_label.starts_with("@@") {
            source_label.to_string()
        } else {
            format!("@@{source_label}")
        };
        let label = CanonicalLabel::parse(&canonical)
            .expect("Bzl evaluation context requires a valid source label");
        Self {
            source_identity: BzlModuleIdentity {
                label,
                workspace_path: PathBuf::new(),
                repository_mapping: Arc::from([]),
            },
            source_label,
            source_identities_by_filename: Arc::from([]),
            bzl_load_visibility: RefCell::new(None),
        }
    }

    #[cfg(test)]
    pub(crate) fn from_identity(source_identity: BzlModuleIdentity) -> Self {
        Self {
            source_label: source_identity.label.to_string().into(),
            source_identity,
            source_identities_by_filename: Arc::from([]),
            bzl_load_visibility: RefCell::new(None),
        }
    }

    pub(crate) fn from_manifest(manifest: &BzlLoadManifest) -> Self {
        let canonical_source = manifest.root.label.to_string();
        let source_label = if manifest.root.label.package().repo().is_root() {
            canonical_source
                .strip_prefix("@@")
                .expect("canonical root labels begin with @@")
                .into()
        } else {
            canonical_source.into()
        };
        Self {
            source_label,
            source_identity: manifest.root.clone(),
            source_identities_by_filename: manifest_starlark_sources(manifest),
            bzl_load_visibility: RefCell::new(None),
        }
    }

    pub(crate) fn from_evaluator<'a>(eval: &'a Evaluator<'_, '_, '_>) -> anyhow::Result<&'a Self> {
        eval.extra
            .and_then(|extra| {
                extra.downcast_ref::<Self>().or_else(|| {
                    extra
                        .downcast_ref::<crate::package::MacroEvaluationContext<'_>>()
                        .map(crate::package::MacroEvaluationContext::bzl)
                })
            })
            .ok_or_else(|| anyhow::anyhow!("operation may only be called in a .bzl module"))
    }

    pub(crate) fn source_label(&self) -> &str {
        &self.source_label
    }

    pub(crate) fn source_identity(&self) -> &BzlModuleIdentity {
        &self.source_identity
    }

    pub(crate) fn set_bzl_load_visibility(
        &self,
        visibility: BzlLoadVisibility,
    ) -> anyhow::Result<()> {
        let mut declaration = self.bzl_load_visibility.borrow_mut();
        if declaration.is_some() {
            anyhow::bail!("load visibility may not be set more than once");
        }
        *declaration = Some(visibility);
        Ok(())
    }

    pub(crate) fn ensure_bzl_load_visibility_unset(&self) -> anyhow::Result<()> {
        if self.bzl_load_visibility.borrow().is_some() {
            anyhow::bail!("load visibility may not be set more than once");
        }
        Ok(())
    }

    pub(crate) fn bzl_load_visibility(&self) -> BzlLoadVisibility {
        self.bzl_load_visibility.borrow().dupe().unwrap_or_default()
    }

    pub(crate) fn source_identity_for_call<'a>(
        &'a self,
        eval: &Evaluator<'_, '_, '_>,
    ) -> anyhow::Result<&'a BzlModuleIdentity> {
        let caller = eval.native_caller_function_filename();
        let Some(filename) = eval
            .native_call_source_filename()
            .or_else(|| caller.clone())
        else {
            return Ok(&self.source_identity);
        };
        if caller.is_none() && self.source_identities_by_filename.is_empty() {
            return Ok(&self.source_identity);
        }
        let mut identities = self
            .source_identities_by_filename
            .iter()
            .filter_map(|(source, identity)| (source.as_str() == filename).then_some(identity));
        let identity = identities.next().ok_or_else(|| {
            anyhow::anyhow!(
                "Starlark caller source is not present in the recursive Bzl manifest: {filename}"
            )
        })?;
        if identities.next().is_some() {
            anyhow::bail!("ambiguous Starlark caller in the Bzl manifest: {filename}");
        }
        Ok(identity)
    }

    pub(crate) fn macro_runtime_context(
        source_identity: BzlModuleIdentity,
        source_identities_by_filename: Arc<[(CompactString, BzlModuleIdentity)]>,
    ) -> Self {
        let canonical_source = source_identity.label.to_string();
        let source_label = if source_identity.label.package().repo().is_root() {
            canonical_source
                .strip_prefix("@@")
                .expect("canonical root labels begin with @@")
                .into()
        } else {
            canonical_source.into()
        };
        Self {
            source_label,
            source_identity,
            source_identities_by_filename,
            bzl_load_visibility: RefCell::new(None),
        }
    }

    pub(crate) fn source_identities_by_filename(
        &self,
    ) -> Arc<[(CompactString, BzlModuleIdentity)]> {
        self.source_identities_by_filename.clone()
    }

    pub(crate) fn source_label_for_call(
        &self,
        eval: &Evaluator<'_, '_, '_>,
    ) -> anyhow::Result<CanonicalLabel> {
        Ok(self.source_identity_for_call(eval)?.label.clone())
    }
}

/// Loading-time provider constructor. `export_as` establishes its structural
/// identity before the containing module may freeze.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative, Trace)]
pub struct UserProviderCallable {
    source_label: CompactString,
    schema: UserProviderSchema,
    #[allocative(skip)]
    id: OnceCell<ProviderId>,
}

#[derive(Debug, Allocative, Trace)]
enum UserProviderSchema {
    Schemaless,
    List(Arc<[CompactString]>),
    Documented(Arc<[CompactString]>),
}

impl UserProviderSchema {
    fn fields(&self) -> Option<&Arc<[CompactString]>> {
        match self {
            Self::Schemaless => None,
            Self::List(fields) | Self::Documented(fields) => Some(fields),
        }
    }
}

pub(crate) fn user_provider_from_arguments<'v>(
    doc: Option<Value<'v>>,
    fields: Option<Value<'v>>,
    init: Option<Value<'v>>,
    eval: &mut Evaluator<'v, '_, '_>,
) -> anyhow::Result<Value<'v>> {
    if doc.is_some_and(|value| !value.is_none() && value.unpack_str().is_none()) {
        anyhow::bail!("provider doc must be a string or None");
    }
    if let Some(init) = init.filter(|value| !value.is_none()) {
        if doc.and_then(Value::unpack_str).is_none() {
            anyhow::bail!("initialized provider requires a string doc");
        }
        let callable: Option<StarlarkCallable<'v>> = StarlarkCallable::unpack_value_opt(init);
        if callable.is_none() {
            anyhow::bail!("provider init must be callable");
        }
        let fields = fields.ok_or_else(|| {
            anyhow::anyhow!("initialized provider fields must be a list or dictionary")
        })?;
        let fields = if let Some(fields) = ListRef::from_value(fields) {
            provider_list_fields(fields)?
        } else if let Some(fields) = DictRef::from_value(fields) {
            provider_documented_fields(&fields)?
        } else {
            anyhow::bail!("initialized provider fields must be a list or dictionary");
        };
        return InitializedUserProviderCallable::allocate_pair(fields, init, eval);
    }
    let schema = match fields.filter(|value| !value.is_none()) {
        None => UserProviderSchema::Schemaless,
        Some(fields) if ListRef::from_value(fields).is_some() => {
            UserProviderSchema::List(provider_list_fields(ListRef::from_value(fields).unwrap())?)
        }
        Some(fields) => {
            let fields = DictRef::from_value(fields).ok_or_else(|| {
                anyhow::anyhow!("provider fields must be a list, dictionary or None")
            })?;
            UserProviderSchema::Documented(provider_documented_fields(&fields)?)
        }
    };
    Ok(eval
        .heap()
        .alloc(UserProviderCallable::from_evaluator(schema, eval)?))
}

fn provider_list_fields(fields: &ListRef<'_>) -> anyhow::Result<Arc<[CompactString]>> {
    let mut names = fields
        .iter()
        .map(|value| {
            value
                .unpack_str()
                .map(CompactString::new)
                .ok_or_else(|| anyhow::anyhow!("provider fields must be strings"))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    names.sort_unstable();
    if names.windows(2).any(|pair| pair[0] == pair[1]) {
        anyhow::bail!("provider fields must not contain duplicates");
    }
    Ok(names.into())
}

fn provider_documented_fields(fields: &DictRef<'_>) -> anyhow::Result<Arc<[CompactString]>> {
    let mut names = Vec::with_capacity(fields.len());
    for (name, documentation) in fields.iter() {
        let name = name
            .unpack_str()
            .ok_or_else(|| anyhow::anyhow!("provider field names must be strings"))?;
        if documentation.unpack_str().is_none() {
            anyhow::bail!("provider field docs must be strings");
        }
        names.push(CompactString::new(name));
    }
    names.sort_unstable();
    Ok(names.into())
}

impl UserProviderCallable {
    fn from_evaluator(
        schema: UserProviderSchema,
        eval: &Evaluator<'_, '_, '_>,
    ) -> anyhow::Result<Self> {
        let context = BzlEvaluationContext::from_evaluator(eval)
            .map_err(|_| anyhow::anyhow!("provider() may only be called in a .bzl module"))?;
        Ok(Self {
            source_label: context.source_label.clone(),
            schema,
            id: OnceCell::new(),
        })
    }

    pub(crate) fn id(&self) -> Option<&ProviderId> {
        self.id.get()
    }
}

impl fmt::Display for UserProviderCallable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.id.get() {
            Some(id) => write!(f, "provider[{id}]"),
            None => f.write_str("provider[unbound]"),
        }
    }
}

impl Freeze for UserProviderCallable {
    type Frozen = FrozenUserProviderCallable;

    fn freeze(self, _freezer: &Freezer) -> FreezeResult<Self::Frozen> {
        let Some(id) = self.id.into_inner() else {
            return Err(FreezeError::new(
                "the result of provider() must be assigned to a top-level variable".to_owned(),
            ));
        };
        Ok(FrozenUserProviderCallable {
            id,
            schema: self.schema,
        })
    }
}

impl<'v> AllocValue<'v> for UserProviderCallable {
    fn alloc_value(self, heap: Heap<'v>) -> Value<'v> {
        heap.alloc_complex(self)
    }
}

#[starlark_value(type = "provider_callable")]
impl<'v> StarlarkValue<'v> for UserProviderCallable {
    type Canonical = FrozenUserProviderCallable;

    fn export_as(
        &self,
        variable_name: &str,
        _eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<()> {
        if self.id.get().is_none() {
            let id = ProviderId::new(self.source_label.clone(), variable_name)
                .map_err(|error| starlark::Error::new_other(anyhow::anyhow!(error.to_string())))?;
            let _ = self.id.set(id);
        }
        Ok(())
    }

    fn invoke(
        &self,
        _me: Value<'v>,
        args: &Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let id = self.id.get().ok_or_else(unbound_provider_error)?;
        invoke_provider(id, &self.schema, args, eval)
    }
}

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct FrozenUserProviderCallable {
    id: ProviderId,
    schema: UserProviderSchema,
}

starlark::starlark_simple_value!(FrozenUserProviderCallable);

impl FrozenUserProviderCallable {
    pub fn id(&self) -> &ProviderId {
        &self.id
    }
}

impl fmt::Display for FrozenUserProviderCallable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "provider[{}]", self.id)
    }
}

#[starlark_value(type = "provider_callable")]
impl<'v> StarlarkValue<'v> for FrozenUserProviderCallable {
    type Canonical = Self;

    fn invoke(
        &self,
        _me: Value<'v>,
        args: &Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        invoke_provider(&self.id, &self.schema, args, eval)
    }
}

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative, Trace)]
pub(crate) struct InitializedUserProviderCallable<'v> {
    source_label: CompactString,
    fields: Arc<[CompactString]>,
    init: Value<'v>,
    #[allocative(skip)]
    id: OnceCell<ProviderId>,
}
impl<'v> InitializedUserProviderCallable<'v> {
    pub(crate) fn allocate_pair(
        fields: Arc<[CompactString]>,
        init: Value<'v>,
        eval: &Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        let context = BzlEvaluationContext::from_evaluator(eval)
            .map_err(|_| anyhow::anyhow!("provider() may only be called in a .bzl module"))?;
        let provider = eval.heap().alloc_complex(Self {
            source_label: context.source_label.clone(),
            fields,
            init,
            id: OnceCell::new(),
        });
        let raw = eval
            .heap()
            .alloc_complex(InitializedProviderRawGen { provider });
        Ok(eval.heap().alloc((provider, raw)))
    }
}
impl fmt::Display for InitializedUserProviderCallable<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.id.get() {
            Some(id) => write!(f, "provider[{id}]"),
            None => f.write_str("provider[unbound]"),
        }
    }
}
impl Freeze for InitializedUserProviderCallable<'_> {
    type Frozen = FrozenInitializedUserProviderCallable;
    fn freeze(self, freezer: &Freezer) -> FreezeResult<Self::Frozen> {
        let Some(id) = self.id.into_inner() else {
            return Err(FreezeError::new(
                "the result of provider() must be assigned to a top-level variable".to_owned(),
            ));
        };
        Ok(FrozenInitializedUserProviderCallable {
            id,
            fields: self.fields,
            init: self.init.freeze(freezer)?,
        })
    }
}
#[starlark_value(type = "provider_callable")]
impl<'v> StarlarkValue<'v> for InitializedUserProviderCallable<'v> {
    type Canonical = FrozenInitializedUserProviderCallable;
    fn export_as(
        &self,
        variable_name: &str,
        _eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<()> {
        if self.id.get().is_none() {
            let id = ProviderId::new(self.source_label.clone(), variable_name)
                .map_err(|error| starlark::Error::new_other(anyhow::anyhow!(error.to_string())))?;
            let _ = self.id.set(id);
        }
        Ok(())
    }
    fn invoke(
        &self,
        _me: Value<'v>,
        args: &Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let id = self.id.get().ok_or_else(unbound_provider_error)?;
        invoke_initialized_provider(id, &self.fields, self.init, args, eval)
    }
}
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub(crate) struct FrozenInitializedUserProviderCallable {
    id: ProviderId,
    fields: Arc<[CompactString]>,
    init: FrozenValue,
}
starlark::starlark_simple_value!(FrozenInitializedUserProviderCallable);
impl fmt::Display for FrozenInitializedUserProviderCallable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "provider[{}]", self.id)
    }
}
#[starlark_value(type = "provider_callable")]
impl<'v> StarlarkValue<'v> for FrozenInitializedUserProviderCallable {
    type Canonical = Self;
    fn invoke(
        &self,
        _me: Value<'v>,
        args: &Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        invoke_initialized_provider(&self.id, &self.fields, self.init.to_value(), args, eval)
    }
}
fn unbound_provider_error() -> starlark::Error {
    let message = "the result of provider() must be assigned before it can be called";
    starlark::Error::new_other(anyhow::anyhow!(message))
}
#[derive(Debug, Trace, Freeze, ProvidesStaticType, NoSerialize, Allocative)]
struct InitializedProviderRawGen<V> {
    provider: V,
}
type InitializedProviderRaw<'v> = InitializedProviderRawGen<Value<'v>>;
type FrozenInitializedProviderRaw = InitializedProviderRawGen<FrozenValue>;
starlark::starlark_complex_values!(InitializedProviderRaw);
impl<V> fmt::Display for InitializedProviderRawGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<raw constructor>")
    }
}
#[starlark_value(type = "function")]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for InitializedProviderRawGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    type Canonical = FrozenInitializedProviderRaw;
    fn invoke(
        &self,
        _me: Value<'v>,
        args: &Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let provider = self.provider.to_value();
        if let Some(provider) = provider.downcast_ref::<InitializedUserProviderCallable>() {
            let id = provider.id.get().ok_or_else(unbound_provider_error)?;
            invoke_initialized_raw(id, &provider.fields, args, eval)
        } else {
            let provider = provider
                .downcast_ref::<FrozenInitializedUserProviderCallable>()
                .expect("raw constructor retains its provider callable");
            invoke_initialized_raw(&provider.id, &provider.fields, args, eval)
        }
    }
}
#[derive(Debug, Trace, Freeze, Allocative)]
enum LoadingProviderFieldsGen<V> {
    Schemaful {
        #[trace(unsafe_ignore)]
        #[freeze(identity)]
        schema: Arc<[CompactString]>,
        values: SmallMap<u32, V>,
    },
    Schemaless(SchemalessLoadingFieldsGen<V>),
}
#[derive(Debug, Trace, Freeze, Allocative)]
struct SchemalessLoadingFieldsGen<V> {
    #[trace(unsafe_ignore)]
    #[freeze(identity)]
    names: Arc<[CompactString]>,
    values: Vec<V>,
}
#[derive(Debug, Trace, Freeze, ProvidesStaticType, NoSerialize, Allocative)]
pub(crate) struct LoadingStarlarkUserProviderGen<V> {
    #[trace(unsafe_ignore)]
    #[freeze(identity)]
    id: ProviderId,
    fields: LoadingProviderFieldsGen<V>,
}
pub(crate) type LoadingStarlarkUserProvider<'v> = LoadingStarlarkUserProviderGen<Value<'v>>;
type FrozenLoadingStarlarkUserProvider = LoadingStarlarkUserProviderGen<FrozenValue>;
starlark::starlark_complex_values!(LoadingStarlarkUserProvider);

fn loading_provider_fields<'v, V: ValueLike<'v>>(
    provider: &LoadingStarlarkUserProviderGen<V>,
) -> Vec<(CompactString, Value<'v>)> {
    let mut fields = match &provider.fields {
        LoadingProviderFieldsGen::Schemaful { schema, values } => values
            .iter()
            .map(|(index, value)| (schema[*index as usize].clone(), value.to_value()))
            .collect::<Vec<_>>(),
        LoadingProviderFieldsGen::Schemaless(values) => values
            .names
            .iter()
            .cloned()
            .zip(values.values.iter().map(|value| value.to_value()))
            .collect(),
    };
    fields.sort_by(|left, right| left.0.cmp(&right.0));
    fields
}

#[cfg(test)]
pub(crate) fn loading_provider_id(value: Value<'_>) -> Option<ProviderId> {
    match LoadingStarlarkUserProvider::from_value(value)? {
        starlark::__macro_refs::Either::Left(provider) => Some(provider.id.dupe()),
        starlark::__macro_refs::Either::Right(provider) => Some(provider.id.dupe()),
    }
}

pub fn starlark_user_provider_fields<'v>(
    value: Value<'v>,
) -> Option<(ProviderId, Vec<(CompactString, Value<'v>)>)> {
    match LoadingStarlarkUserProvider::from_value(value)? {
        starlark::__macro_refs::Either::Left(provider) => {
            Some((provider.id.dupe(), loading_provider_fields(provider)))
        }
        starlark::__macro_refs::Either::Right(provider) => {
            Some((provider.id.dupe(), loading_provider_fields(provider)))
        }
    }
}

pub fn alloc_starlark_user_provider(
    heap: &starlark::values::FrozenHeap,
    id: ProviderId,
    fields: impl IntoIterator<Item = (CompactString, FrozenValue)>,
) -> FrozenValue {
    let mut fields = fields.into_iter().collect::<SmallMap<_, _>>();
    fields.sort_keys();
    let (names, values): (Vec<_>, Vec<_>) = fields.into_iter().unzip();
    heap.alloc(LoadingStarlarkUserProviderGen {
        id,
        fields: LoadingProviderFieldsGen::Schemaless(SchemalessLoadingFieldsGen {
            names: names.into(),
            values,
        }),
    })
}
impl<V> fmt::Display for LoadingStarlarkUserProviderGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}(...)", self.id.exported_name())
    }
}
#[starlark_value(type = "struct")]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for LoadingStarlarkUserProviderGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    type Canonical = FrozenLoadingStarlarkUserProvider;

    fn equals(&self, other: Value<'v>) -> starlark::Result<bool> {
        let Some((other_id, other_fields)) = starlark_user_provider_fields(other) else {
            return Ok(false);
        };
        if self.id != other_id {
            return Ok(false);
        }
        let fields = loading_provider_fields(self);
        if fields.len() != other_fields.len() {
            return Ok(false);
        }
        for ((name, value), (other_name, other_value)) in fields.iter().zip(&other_fields) {
            if name != other_name || !value.equals(*other_value)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn write_hash(&self, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        if !self.is_structurally_immutable() {
            return Err(starlark::Error::new_other(anyhow::anyhow!(
                "unhashable type: provider"
            )));
        }
        self.write_structural_hash(hasher)
    }

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn StarlarkStructuralValue>(self);
    }

    fn get_attr(&self, attribute: &str, _heap: Heap<'v>) -> Option<Value<'v>> {
        match &self.fields {
            LoadingProviderFieldsGen::Schemaful { schema, values } => {
                let index = schema
                    .iter()
                    .position(|field| field.as_str() == attribute)?
                    as u32;
                values.get(&index).map(|value| value.to_value())
            }
            LoadingProviderFieldsGen::Schemaless(values) => {
                let index = values
                    .names
                    .iter()
                    .position(|name| name.as_str() == attribute)?;
                Some(values.values[index].to_value())
            }
        }
    }

    fn dir_attr(&self) -> Vec<String> {
        loading_provider_fields(self)
            .into_iter()
            .map(|(name, _)| name.to_string())
            .collect()
    }
}

impl<'v, V: ValueLike<'v>> StarlarkStructuralValue for LoadingStarlarkUserProviderGen<V> {
    fn is_structurally_immutable(&self) -> bool {
        loading_provider_fields(self)
            .iter()
            .all(|(_, value)| starlark_structural_is_immutable(*value))
    }

    fn write_structural_hash(&self, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        self.id.hash(hasher);
        for (name, value) in loading_provider_fields(self) {
            name.hash(hasher);
            write_starlark_structural_hash(value, hasher)?;
        }
        Ok(())
    }
}
fn invoke_initialized_provider<'v>(
    id: &ProviderId,
    fields: &Arc<[CompactString]>,
    init: Value<'v>,
    args: &Arguments<'v, '_>,
    eval: &mut Evaluator<'v, '_, '_>,
) -> starlark::Result<Value<'v>> {
    let positions = args.positions(eval.heap())?.collect::<Vec<_>>();
    let kwargs = eval.heap().alloc(args.names_map()?);
    let initialized = init.invoke_pos_kwargs(&positions, Some(kwargs), eval)?;
    let values = DictRef::from_value(initialized).ok_or_else(|| {
        starlark::Error::new_other(anyhow::anyhow!(
            "provider init must return a dictionary, got {}",
            initialized.get_type()
        ))
    })?;
    allocate_schemaful_loading_provider(id, fields, values.iter(), eval)
}
fn invoke_initialized_raw<'v>(
    id: &ProviderId,
    fields: &Arc<[CompactString]>,
    args: &Arguments<'v, '_>,
    eval: &mut Evaluator<'v, '_, '_>,
) -> starlark::Result<Value<'v>> {
    args.no_positional_args(eval.heap())?;
    let values = args.names_map()?;
    allocate_schemaful_loading_provider(
        id,
        fields,
        values.iter().map(|(name, value)| (name.to_value(), *value)),
        eval,
    )
}
fn allocate_schemaful_loading_provider<'v>(
    id: &ProviderId,
    fields: &Arc<[CompactString]>,
    values: impl IntoIterator<Item = (Value<'v>, Value<'v>)>,
    eval: &Evaluator<'v, '_, '_>,
) -> starlark::Result<Value<'v>> {
    let mut retained = SmallMap::new();
    for (name, value) in values {
        let name = name.unpack_str().ok_or_else(|| {
            starlark::Error::new_other(anyhow::anyhow!("provider field keys must be strings"))
        })?;
        let Some(index) = fields.iter().position(|field| field.as_str() == name) else {
            return Err(starlark::Error::new_other(anyhow::anyhow!(
                "provider {} received unknown field `{name}`",
                id
            )));
        };
        retained.insert(index as u32, value);
    }
    Ok(eval.heap().alloc_complex(LoadingStarlarkUserProviderGen {
        id: id.dupe(),
        fields: LoadingProviderFieldsGen::Schemaful {
            schema: fields.dupe(),
            values: retained,
        },
    }))
}
fn invoke_provider<'v>(
    id: &ProviderId,
    schema: &UserProviderSchema,
    args: &Arguments<'v, '_>,
    eval: &mut Evaluator<'v, '_, '_>,
) -> starlark::Result<Value<'v>> {
    args.no_positional_args(eval.heap())?;
    let names = args.names_map()?;
    if let Some(fields) = schema.fields() {
        for name in names.keys() {
            if !fields.iter().any(|field| field.as_str() == name.as_str()) {
                return Err(starlark::Error::new_other(anyhow::anyhow!(
                    "provider {} received unknown field `{}`",
                    id,
                    name
                )));
            }
        }
        return allocate_schemaful_loading_provider(
            id,
            fields,
            names.iter().map(|(name, value)| (name.to_value(), *value)),
            eval,
        );
    }
    let field_names = names
        .keys()
        .map(|name| CompactString::new(name.as_str()))
        .collect::<Vec<_>>();
    let values = names.values().copied().collect();
    Ok(eval.heap().alloc_complex(LoadingStarlarkUserProviderGen {
        id: id.dupe(),
        fields: LoadingProviderFieldsGen::Schemaless(SchemalessLoadingFieldsGen {
            names: field_names.into(),
            values,
        }),
    }))
}

#[derive(Debug, Clone, Trace, Freeze, Allocative)]
pub enum StarlarkDepsetSuccessorGen<V> {
    Direct(V),
    Transitive(V),
}

#[derive(Debug, Trace, Freeze, ProvidesStaticType, NoSerialize, Allocative)]
pub struct StarlarkDepsetGen<V> {
    #[trace(unsafe_ignore)]
    #[freeze(identity)]
    order: DepsetOrder,
    #[trace(unsafe_ignore)]
    #[freeze(identity)]
    element_type: Option<CompactString>,
    #[trace(unsafe_ignore)]
    #[freeze(identity)]
    occurrence: AnalysisDepsetOccurrence,
    #[trace(unsafe_ignore)]
    #[freeze(identity)]
    retained: Option<AnalysisDepset>,
    #[trace(unsafe_ignore)]
    #[freeze(identity)]
    depth: usize,
    successors: Vec<StarlarkDepsetSuccessorGen<V>>,
}

pub type StarlarkDepset<'v> = StarlarkDepsetGen<Value<'v>>;
type FrozenStarlarkDepset = StarlarkDepsetGen<FrozenValue>;
starlark::starlark_complex_values!(StarlarkDepset);

impl<'v> StarlarkDepset<'v> {
    pub fn direct_from_value(value: Value<'v>) -> Option<Vec<Value<'v>>> {
        Self::parts_from_value(value).map(|(_, _, _, _, _, successors)| {
            successors
                .into_iter()
                .filter_map(|successor| match successor {
                    StarlarkDepsetSuccessorGen::Direct(value) => Some(value),
                    StarlarkDepsetSuccessorGen::Transitive(_) => None,
                })
                .collect()
        })
    }

    pub fn parts_from_value(
        value: Value<'v>,
    ) -> Option<(
        DepsetOrder,
        Option<CompactString>,
        AnalysisDepsetOccurrence,
        Option<AnalysisDepset>,
        usize,
        Vec<StarlarkDepsetSuccessorGen<Value<'v>>>,
    )> {
        fn parts<'v, V: ValueLike<'v>>(
            value: &StarlarkDepsetGen<V>,
        ) -> (
            DepsetOrder,
            Option<CompactString>,
            AnalysisDepsetOccurrence,
            Option<AnalysisDepset>,
            usize,
            Vec<StarlarkDepsetSuccessorGen<Value<'v>>>,
        ) {
            (
                value.order,
                value.element_type.clone(),
                value.occurrence.dupe(),
                value.retained.clone(),
                value.depth,
                value
                    .successors
                    .iter()
                    .map(|successor| match successor {
                        StarlarkDepsetSuccessorGen::Direct(value) => {
                            StarlarkDepsetSuccessorGen::Direct(value.to_value())
                        }
                        StarlarkDepsetSuccessorGen::Transitive(value) => {
                            StarlarkDepsetSuccessorGen::Transitive(value.to_value())
                        }
                    })
                    .collect(),
            )
        }
        match Self::from_value(value)? {
            starlark::__macro_refs::Either::Left(value) => Some(parts(value)),
            starlark::__macro_refs::Either::Right(value) => Some(parts(value)),
        }
    }

    fn depth_from_value(value: Value<'v>) -> Option<usize> {
        match Self::from_value(value)? {
            starlark::__macro_refs::Either::Left(value) => Some(value.depth),
            starlark::__macro_refs::Either::Right(value) => Some(value.depth),
        }
    }

    fn order_from_value(value: Value<'v>) -> Option<DepsetOrder> {
        match Self::from_value(value)? {
            starlark::__macro_refs::Either::Left(value) => Some(value.order),
            starlark::__macro_refs::Either::Right(value) => Some(value.order),
        }
    }

    fn singleton_from_value(value: Value<'v>) -> Option<Value<'v>> {
        fn singleton<'v, V: ValueLike<'v>>(value: &StarlarkDepsetGen<V>) -> Option<Value<'v>> {
            match value.successors.as_slice() {
                [StarlarkDepsetSuccessorGen::Direct(value)] => Some(value.to_value()),
                _ => None,
            }
        }
        match Self::from_value(value)? {
            starlark::__macro_refs::Either::Left(value) => singleton(value),
            starlark::__macro_refs::Either::Right(value) => singleton(value),
        }
    }

    fn visit_successors_reverse<E>(
        value: Value<'v>,
        visitor: &mut impl FnMut(DepsetSuccessor<Value<'v>, EvaluatorDepset<'v>>) -> Result<(), E>,
    ) -> Option<Result<(), E>> {
        fn visit<'v, V: ValueLike<'v>, E>(
            value: &StarlarkDepsetGen<V>,
            visitor: &mut impl FnMut(DepsetSuccessor<Value<'v>, EvaluatorDepset<'v>>) -> Result<(), E>,
        ) -> Result<(), E> {
            for successor in value.successors.iter().rev() {
                visitor(match successor {
                    StarlarkDepsetSuccessorGen::Direct(value) => {
                        DepsetSuccessor::Direct(value.to_value())
                    }
                    StarlarkDepsetSuccessorGen::Transitive(value) => {
                        DepsetSuccessor::Transitive(EvaluatorDepset(value.to_value()))
                    }
                })?;
            }
            Ok(())
        }
        match Self::from_value(value)? {
            starlark::__macro_refs::Either::Left(value) => Some(visit(value, visitor)),
            starlark::__macro_refs::Either::Right(value) => Some(visit(value, visitor)),
        }
    }
    fn flatten(value: Value<'v>) -> starlark::Result<Vec<Value<'v>>> {
        traverse_depset(
            &EvaluatorDepset(value),
            |value| {
                value
                    .get_hashed()
                    .map(|value| u64::from(value.hash().get()))
            },
            |left, right| left.equals(*right),
        )
    }
}

#[derive(Clone, Copy)]
struct EvaluatorDepset<'v>(Value<'v>);

impl<'v> DepsetView for EvaluatorDepset<'v> {
    type Item = Value<'v>;
    type NodeKey = ValueIdentity<'v>;
    fn order(&self) -> DepsetOrder {
        StarlarkDepset::order_from_value(self.0).expect("evaluator depset view wraps a depset")
    }
    fn depth(&self) -> usize {
        StarlarkDepset::depth_from_value(self.0).expect("evaluator depset view wraps a depset")
    }
    fn node_key(&self) -> Self::NodeKey {
        self.0.identity()
    }
    fn singleton_item(&self) -> Option<Self::Item> {
        StarlarkDepset::singleton_from_value(self.0)
    }

    fn for_each_successor_reverse<E>(
        &self,
        mut visitor: impl FnMut(DepsetSuccessor<Self::Item, Self>) -> Result<(), E>,
    ) -> Result<(), E> {
        StarlarkDepset::visit_successors_reverse(self.0, &mut visitor)
            .expect("evaluator depset view wraps a depset")
    }
}

pub fn alloc_starlark_depset(
    heap: &starlark::values::FrozenHeap,
    retained: AnalysisDepset,
    successors: Vec<StarlarkDepsetSuccessorGen<FrozenValue>>,
) -> FrozenValue {
    alloc_starlark_depset_parts(
        heap,
        retained.order(),
        (retained.element_type() != AnalysisValueType::Empty)
            .then(|| CompactString::new(retained.element_type().as_str())),
        retained.occurrence(),
        Some(retained.clone()),
        retained.depth(),
        successors,
    )
}

pub fn alloc_starlark_depset_parts(
    heap: &starlark::values::FrozenHeap,
    order: DepsetOrder,
    element_type: Option<CompactString>,
    occurrence: AnalysisDepsetOccurrence,
    retained: Option<AnalysisDepset>,
    depth: usize,
    successors: Vec<StarlarkDepsetSuccessorGen<FrozenValue>>,
) -> FrozenValue {
    heap.alloc(StarlarkDepsetGen {
        order,
        element_type,
        occurrence,
        retained,
        depth,
        successors,
    })
}

impl<V> fmt::Display for StarlarkDepsetGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("depset(...)")
    }
}

#[starlark_value(type = "depset")]
impl<'v, V: starlark::values::ValueLike<'v>> StarlarkValue<'v> for StarlarkDepsetGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    type Canonical = FrozenStarlarkDepset;

    fn equals(&self, other: Value<'v>) -> starlark::Result<bool> {
        Ok(StarlarkDepset::parts_from_value(other)
            .is_some_and(|(_, _, occurrence, _, _, _)| self.occurrence == occurrence))
    }

    fn write_hash(&self, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        self.occurrence.hash(hasher);
        Ok(())
    }

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn StarlarkStructuralValue>(self);
    }

    fn get_methods() -> Option<&'static Methods> {
        static METHODS: MethodsStatic = MethodsStatic::new();
        METHODS.methods(starlark_depset_methods)
    }
}

impl<'v, V: ValueLike<'v>> StarlarkStructuralValue for StarlarkDepsetGen<V> {
    fn is_structurally_immutable(&self) -> bool {
        true
    }

    fn write_structural_hash(&self, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        self.occurrence.hash(hasher);
        Ok(())
    }
}

#[starlark_module]
fn starlark_depset_methods(builder: &mut MethodsBuilder) {
    fn to_list<'v>(
        this: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        Ok(eval.heap().alloc(StarlarkDepset::flatten(this)?))
    }
}

#[derive(Debug, Trace, Freeze, ProvidesStaticType, NoSerialize, Allocative)]
pub struct StarlarkDefaultInfoGen<V> {
    files: Option<V>,
    executable: Option<V>,
}

pub type StarlarkDefaultInfo<'v> = StarlarkDefaultInfoGen<Value<'v>>;
type FrozenStarlarkDefaultInfo = StarlarkDefaultInfoGen<FrozenValue>;
starlark::starlark_complex_values!(StarlarkDefaultInfo);

impl<'v> StarlarkDefaultInfo<'v> {
    pub fn fields_from_value(value: Value<'v>) -> Option<(Option<Value<'v>>, Option<Value<'v>>)> {
        match Self::from_value(value)? {
            starlark::__macro_refs::Either::Left(value) => Some((value.files, value.executable)),
            starlark::__macro_refs::Either::Right(value) => Some((
                value.files.map(|value| value.to_value()),
                value.executable.map(|value| value.to_value()),
            )),
        }
    }
}

impl<V> fmt::Display for StarlarkDefaultInfoGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DefaultInfo(...)")
    }
}

#[starlark_value(type = "DefaultInfo")]
impl<'v, V: starlark::values::ValueLike<'v>> StarlarkValue<'v> for StarlarkDefaultInfoGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    type Canonical = FrozenStarlarkDefaultInfo;
}

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub(crate) struct AnalysisBuiltinCallable {
    name: &'static str,
}

/// Analysis-only capability supplied by the existing root analysis evaluator.
/// Loading evaluators intentionally do not install it, so the same frozen
/// callable remains unavailable while a `.bzl` file is evaluated.
#[derive(Debug, ProvidesStaticType)]
pub struct ToolchainInfoAnalysisContext;

#[derive(Debug, Trace, Freeze, ProvidesStaticType, NoSerialize, Allocative)]
pub struct StarlarkToolchainInfoGen<V> {
    #[trace(unsafe_ignore)]
    #[freeze(identity)]
    names: Arc<[CompactString]>,
    values: Vec<V>,
}

pub type StarlarkToolchainInfo<'v> = StarlarkToolchainInfoGen<Value<'v>>;
type FrozenStarlarkToolchainInfo = StarlarkToolchainInfoGen<FrozenValue>;
starlark::starlark_complex_values!(StarlarkToolchainInfo);

impl<'v> StarlarkToolchainInfo<'v> {
    pub fn fields_from_value(value: Value<'v>) -> Option<Vec<(CompactString, Value<'v>)>> {
        fn collect<'v, V: ValueLike<'v>>(
            value: &StarlarkToolchainInfoGen<V>,
        ) -> Vec<(CompactString, Value<'v>)> {
            value
                .names
                .iter()
                .cloned()
                .zip(value.values.iter().map(|value| value.to_value()))
                .collect()
        }
        match Self::from_value(value)? {
            starlark::__macro_refs::Either::Left(value) => Some(collect(value)),
            starlark::__macro_refs::Either::Right(value) => Some(collect(value)),
        }
    }

    pub fn alloc_value(
        heap: Heap<'v>,
        fields: impl IntoIterator<Item = (CompactString, Value<'v>)>,
    ) -> Value<'v> {
        let mut fields = fields.into_iter().collect::<SmallMap<_, _>>();
        fields.sort_keys();
        let (names, values): (Vec<_>, Vec<_>) = fields.into_iter().unzip();
        heap.alloc_complex(StarlarkToolchainInfo {
            names: names.into(),
            values,
        })
    }

    pub fn alloc<'a>(
        heap: &'a starlark::values::FrozenHeap,
        fields: impl IntoIterator<Item = (CompactString, FrozenValue)>,
    ) -> FrozenValue {
        let mut fields = fields.into_iter().collect::<SmallMap<_, _>>();
        fields.sort_keys();
        let (names, values): (Vec<_>, Vec<_>) = fields.into_iter().unzip();
        heap.alloc(StarlarkToolchainInfoGen {
            names: names.into(),
            values,
        })
    }
}

impl<V> fmt::Display for StarlarkToolchainInfoGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ToolchainInfo(...)")
    }
}

#[starlark_value(type = "ToolchainInfo")]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for StarlarkToolchainInfoGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    type Canonical = FrozenStarlarkToolchainInfo;

    fn equals(&self, other: Value<'v>) -> starlark::Result<bool> {
        let Some(other) = StarlarkToolchainInfo::fields_from_value(other) else {
            return Ok(false);
        };
        if self.names.len() != other.len() {
            return Ok(false);
        }
        for ((name, value), (other_name, other_value)) in self
            .names
            .iter()
            .zip(&self.values)
            .map(|(name, value)| (name, value.to_value()))
            .zip(&other)
        {
            if name != other_name || !value.equals(*other_value)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn write_hash(&self, _hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        Err(starlark::Error::new_other(anyhow::anyhow!(
            "unhashable type: ToolchainInfo"
        )))
    }

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn StarlarkStructuralValue>(self);
    }

    fn get_attr(&self, attribute: &str, _heap: Heap<'v>) -> Option<Value<'v>> {
        let index = self.names.iter().position(|name| name == attribute)?;
        Some(self.values[index].to_value())
    }

    fn dir_attr(&self) -> Vec<String> {
        self.names.iter().map(ToString::to_string).collect()
    }
}

impl<'v, V: ValueLike<'v>> StarlarkStructuralValue for StarlarkToolchainInfoGen<V> {
    fn is_structurally_immutable(&self) -> bool {
        false
    }

    fn write_structural_hash(&self, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        "ToolchainInfo".hash(hasher);
        for (name, value) in self.names.iter().zip(&self.values) {
            name.hash(hasher);
            write_starlark_structural_hash(value.to_value(), hasher)?;
        }
        Ok(())
    }
}

impl AnalysisBuiltinCallable {
    pub(crate) const fn new(name: &'static str) -> Self {
        Self { name }
    }
}

fn merge_evaluator_depset_type(
    element_type: &RefCell<Option<CompactString>>,
    candidate: Option<CompactString>,
) -> starlark::Result<()> {
    let Some(candidate) = candidate else {
        return Ok(());
    };
    let mut element_type = element_type.borrow_mut();
    if let Some(existing) = element_type.as_ref() {
        if existing != &candidate {
            return Err(starlark::Error::new_other(anyhow::anyhow!(
                "depset elements have incompatible types `{existing}` and `{candidate}`"
            )));
        }
    } else {
        *element_type = Some(candidate);
    }
    Ok(())
}

fn empty_evaluator_depset<'v>(order: DepsetOrder, eval: &mut Evaluator<'v, '_, '_>) -> Value<'v> {
    let index = match order {
        DepsetOrder::Default => 0,
        DepsetOrder::Postorder => 1,
        DepsetOrder::Preorder => 2,
        DepsetOrder::Topological => 3,
    };
    if let Some(cache) = eval.module().extra_value().and_then(TupleRef::from_value) {
        return cache.content()[index];
    }
    let values = [
        DepsetOrder::Default,
        DepsetOrder::Postorder,
        DepsetOrder::Preorder,
        DepsetOrder::Topological,
    ]
    .map(|order| {
        let retained = AnalysisDepset::empty(order);
        eval.heap().alloc_complex(StarlarkDepset {
            order,
            element_type: None,
            occurrence: retained.occurrence(),
            retained: Some(retained),
            depth: 0,
            successors: Vec::new(),
        })
    });
    let value = values[index];
    eval.module()
        .set_extra_value(eval.heap().alloc(AllocTuple(values)));
    value
}

pub fn starlark_provider_identity(value: Value<'_>) -> Option<ProviderIdentity> {
    if let Some(key) = BuiltinProviderKey::from_value(value) {
        return Some(ProviderIdentity::builtin(key.name));
    }
    if let Some(callable) = FrozenUserProviderCallable::from_value(value) {
        return Some(ProviderIdentity::user(callable.id().dupe()));
    }
    if let Some(callable) = FrozenInitializedUserProviderCallable::from_value(value) {
        return Some(ProviderIdentity::user(callable.id.dupe()));
    }
    if let Some(callable) = AnalysisBuiltinCallable::from_value(value) {
        return matches!(callable.name, "DefaultInfo" | "ToolchainInfo")
            .then(|| ProviderIdentity::builtin(callable.name));
    }
    if OutputGroupInfo::from_value(value).is_some() {
        return Some(ProviderIdentity::builtin("OutputGroupInfo"));
    }
    if RunEnvironmentInfo::from_value(value).is_some() {
        return Some(ProviderIdentity::builtin("RunEnvironmentInfo"));
    }
    if let Some(name) = crate::testing_bootstrap::testing_provider_identity(value) {
        return Some(ProviderIdentity::builtin(name));
    }
    None
}

/// Allocates the existing loading-owned callable token for an admitted
/// analysis provider without installing another evaluator global.
pub fn alloc_starlark_provider_callable(
    heap: &starlark::values::FrozenHeap,
    name: &'static str,
) -> Option<FrozenValue> {
    Some(match name {
        "PackageSpecificationInfo" => heap.alloc(BuiltinProviderKey::new(name)),
        "DefaultInfo" | "ToolchainInfo" => heap.alloc(AnalysisBuiltinCallable::new(name)),
        "OutputGroupInfo" => heap.alloc(OutputGroupInfo),
        "RunEnvironmentInfo" => heap.alloc(RunEnvironmentInfo),
        "ExecutionInfo"
        | "InstrumentedFilesInfo"
        | "AnalysisFailureInfo"
        | "AnalysisTestResultInfo" => {
            return crate::testing_bootstrap::alloc_testing_provider_token(heap, name);
        }
        _ => return None,
    })
}

impl fmt::Display for AnalysisBuiltinCallable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name)
    }
}

starlark::starlark_simple_value!(AnalysisBuiltinCallable);

#[starlark_value(type = "analysis_builtin")]
impl<'v> StarlarkValue<'v> for AnalysisBuiltinCallable {
    fn invoke(
        &self,
        _me: Value<'v>,
        args: &Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        match self.name {
            "depset" => {
                let positions = args.positions(eval.heap())?.collect::<Vec<_>>();
                if positions.len() > 2 {
                    return Err(starlark::Error::new_other(anyhow::anyhow!(
                        "depset accepts at most two positional arguments"
                    )));
                }
                let names = args.names_map()?;
                if names
                    .keys()
                    .any(|name| !matches!(name.as_str(), "direct" | "order" | "transitive"))
                {
                    return Err(starlark::Error::new_other(anyhow::anyhow!(
                        "depset only supports `direct`, `order`, and `transitive`"
                    )));
                }
                if !positions.is_empty() && names.contains_key("direct") {
                    return Err(starlark::Error::new_other(anyhow::anyhow!(
                        "depset direct specified twice"
                    )));
                }
                if positions.len() == 2 && names.contains_key("order") {
                    return Err(starlark::Error::new_other(anyhow::anyhow!(
                        "depset order specified twice"
                    )));
                }
                let list_values = |name: &str, value: Option<Value<'v>>| {
                    let Some(value) = value.filter(|value| !value.is_none()) else {
                        return Ok(Vec::new());
                    };
                    if let Some(list) = ListRef::from_value(value) {
                        return Ok(list.iter().collect());
                    }
                    if let Some(tuple) = TupleRef::from_value(value) {
                        return Ok(tuple.iter().collect());
                    }
                    Err(starlark::Error::new_other(anyhow::anyhow!(
                        "depset {name} elements must be a sequence"
                    )))
                };
                let direct = list_values(
                    "direct",
                    positions
                        .first()
                        .copied()
                        .or_else(|| names.get("direct").copied()),
                )?;
                let transitive = list_values("transitive", names.get("transitive").copied())?;
                if transitive
                    .iter()
                    .any(|value| StarlarkDepset::from_value(*value).is_none())
                {
                    return Err(starlark::Error::new_other(anyhow::anyhow!(
                        "depset transitive elements must be depsets"
                    )));
                }
                let order = positions
                    .get(1)
                    .copied()
                    .or_else(|| names.get("order").copied())
                    .filter(|value| !value.is_none())
                    .map(|value| {
                        value
                            .unpack_str()
                            .ok_or_else(|| {
                                starlark::Error::new_other(anyhow::anyhow!(
                                    "depset order must be a string"
                                ))
                            })?
                            .parse::<DepsetOrder>()
                            .map_err(|error| starlark::Error::new_other(anyhow::anyhow!(error)))
                    })
                    .transpose()?
                    .unwrap_or(DepsetOrder::Default);

                let element_type = RefCell::new(None);
                let built = build_depset(
                    order,
                    direct,
                    transitive.into_iter().map(EvaluatorDepset).collect(),
                    |value| value.get_hashed().map(|value| u64::from(value.hash().get())),
                    |left, right| left.equals(*right),
                    |value| {
                        if ListRef::from_value(*value).is_some()
                            || DictRef::from_value(*value).is_some()
                            || !starlark_structural_is_immutable(*value)
                        {
                            return Err(starlark::Error::new_other(anyhow::anyhow!(
                                "depset elements must be immutable and may not be lists or dictionaries"
                            )));
                        }
                        merge_evaluator_depset_type(
                            &element_type,
                            Some(CompactString::new(value.get_type())),
                        )
                    },
                    |child| {
                        let (_, child_type, ..) = StarlarkDepset::parts_from_value(child.0)
                            .expect("transitive values were checked as depsets");
                        merge_evaluator_depset_type(&element_type, child_type)
                    },
                )
                .map_err(|error| match error {
                    DepsetBuildError::Element(error) => error,
                    DepsetBuildError::Depset(error) => {
                        starlark::Error::new_other(anyhow::anyhow!(error))
                    }
                })?;
                let (depth, successors) = match built {
                    DepsetBuild::Empty => return Ok(empty_evaluator_depset(order, eval)),
                    DepsetBuild::Reuse(value) => return Ok(value.0),
                    DepsetBuild::Dereference(child) => (
                        child.depth(),
                        vec![StarlarkDepsetSuccessorGen::Transitive(child.0)],
                    ),
                    DepsetBuild::Node(successors, depth) => {
                        let successors = successors
                            .into_iter()
                            .map(|successor| match successor {
                                DepsetSuccessor::Direct(value) => {
                                    StarlarkDepsetSuccessorGen::Direct(value)
                                }
                                DepsetSuccessor::Transitive(value) => {
                                    StarlarkDepsetSuccessorGen::Transitive(value.0)
                                }
                            })
                            .collect();
                        (depth, successors)
                    }
                };
                Ok(eval.heap().alloc_complex(StarlarkDepset {
                    order,
                    element_type: element_type.into_inner(),
                    occurrence: AnalysisDepsetOccurrence::new(),
                    retained: None,
                    depth,
                    successors,
                }))
            }
            "DefaultInfo" => {
                args.no_positional_args(eval.heap())?;
                let names = args.names_map()?;
                if names.len() > 2
                    || names
                        .keys()
                        .any(|name| name.as_str() != "files" && name.as_str() != "executable")
                {
                    return Err(starlark::Error::new_other(anyhow::anyhow!(
                        "DefaultInfo only supports optional named arguments `files` and `executable` in this analysis packet"
                    )));
                }
                let optional = |name| names.get(name).copied().filter(|value| !value.is_none());
                Ok(eval.heap().alloc(StarlarkDefaultInfo {
                    files: optional("files"),
                    executable: optional("executable"),
                }))
            }
            "ToolchainInfo" => {
                if eval.extra.is_none_or(|extra| {
                    extra
                        .downcast_ref::<ToolchainInfoAnalysisContext>()
                        .is_none()
                        && extra
                            .downcast_ref::<crate::subrule_invocation::AnalysisEvaluationContext>()
                            .is_none()
                }) {
                    return Err(starlark::Error::new_other(anyhow::anyhow!(
                        "unsupported analysis builtin ToolchainInfo"
                    )));
                }
                args.no_positional_args(eval.heap())?;
                let names = args.names_map()?;
                let mut fields = names
                    .iter()
                    .map(|(name, value)| (CompactString::new(name.as_str()), *value))
                    .collect::<SmallMap<_, _>>();
                fields.sort_keys();
                let (names, values): (Vec<_>, Vec<_>) = fields.into_iter().unzip();
                Ok(eval.heap().alloc_complex(StarlarkToolchainInfo {
                    names: names.into(),
                    values,
                }))
            }
            _ => Err(starlark::Error::new_other(anyhow::anyhow!(
                "unsupported analysis builtin {}",
                self.name
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use starlark::environment::Module;
    use starlark::eval::Evaluator;
    use starlark::syntax::AstModule;
    use starlark::syntax::Dialect;

    use super::StarlarkDefaultInfo;
    use crate::package::loading_globals;

    fn evaluate(source: &str) -> Result<(bool, bool), String> {
        let ast = AstModule::parse("provider_test.bzl", source.to_owned(), &Dialect::Standard)
            .map_err(|error| error.to_string())?;
        let module = Module::new();
        Evaluator::new(&module)
            .eval_module(ast, &loading_globals())
            .map_err(|error| error.to_string())?;
        let value = module
            .get("result")
            .ok_or_else(|| "result was not defined".to_owned())?;
        let (files, executable) = StarlarkDefaultInfo::fields_from_value(value)
            .ok_or_else(|| "not DefaultInfo".to_owned())?;
        Ok((files.is_some(), executable.is_some()))
    }

    fn evaluate_module(source: &str) -> Result<starlark::environment::FrozenModule, String> {
        let ast = AstModule::parse("depset_test.bzl", source.to_owned(), &Dialect::Standard)
            .map_err(|error| error.to_string())?;
        let module = Module::new();
        Evaluator::new(&module)
            .eval_module(ast, &loading_globals())
            .map_err(|error| error.to_string())?;
        module.freeze().map_err(|error| format!("{error:?}"))
    }

    #[test]
    fn default_info_omitted_and_none_arguments_are_equivalent() {
        assert_eq!(evaluate("result = DefaultInfo()").unwrap(), (false, false));
        assert_eq!(
            evaluate("result = DefaultInfo(files = None, executable = None)").unwrap(),
            (false, false)
        );
    }

    #[test]
    fn default_info_rejects_non_admitted_arguments() {
        let error = evaluate("result = DefaultInfo(runfiles = None)").unwrap_err();
        assert!(
            error.contains("only supports optional named arguments"),
            "{error}"
        );
    }

    #[test]
    fn depset_sequences_positional_order_and_empty_interning_share_one_constructor() {
        let module = evaluate_module("A=depset()\nB=depset(direct=())\nP=depset((), 'postorder')\nQ=depset(direct=[], order='postorder')\nCHILD=depset((1,), 'postorder')\nVALUES=depset((2,), 'postorder', transitive=(CHILD,)).to_list()\n").unwrap();
        let a = module.get("A").unwrap();
        let b = module.get("B").unwrap();
        let p = module.get("P").unwrap();
        let q = module.get("Q").unwrap();
        assert!(a.value().ptr_eq(b.value()));
        assert!(p.value().ptr_eq(q.value()));
        assert!(!a.value().ptr_eq(p.value()));
        assert_eq!(module.get("VALUES").unwrap().value().to_string(), "[1, 2]");

        for source in [
            "X = depset([], 'postorder', order = 'postorder')",
            "X = depset([], 'postorder', [])",
        ] {
            assert!(evaluate_module(source).is_err(), "{source}");
        }
    }
}
