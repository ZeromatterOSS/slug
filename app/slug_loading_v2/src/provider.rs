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
#[cfg(test)]
use std::path::PathBuf;
use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use dupe::Dupe;
use slug_build_api_v2::ProviderId;
use slug_identity_v2::CanonicalLabel;
use starlark::any::ProvidesStaticType;
use starlark::eval::Arguments;
use starlark::eval::Evaluator;
use starlark::values::AllocValue;
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
use starlark::values::starlark_value;
use starlark::values::typing::StarlarkCallable;
use starlark_map::small_map::SmallMap;

use crate::bzl_module::BzlLoadManifest;
use crate::bzl_module::BzlModuleIdentity;
use crate::bzl_module::manifest_starlark_sources;
use crate::bzl_visibility::BzlLoadVisibility;

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
            .and_then(|extra| extra.downcast_ref::<Self>())
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

    fn supports_configured_strings(&self) -> bool {
        matches!(self, Self::Documented(_))
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
#[cfg(test)]
pub(crate) fn loading_provider_id(value: Value<'_>) -> Option<ProviderId> {
    match LoadingStarlarkUserProvider::from_value(value)? {
        starlark::__macro_refs::Either::Left(provider) => Some(provider.id.dupe()),
        starlark::__macro_refs::Either::Right(provider) => Some(provider.id.dupe()),
    }
}
impl<V> fmt::Display for LoadingStarlarkUserProviderGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}(...)", self.id.exported_name())
    }
}
#[starlark_value(type = "provider")]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for LoadingStarlarkUserProviderGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    type Canonical = FrozenLoadingStarlarkUserProvider;
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
        if schema.supports_configured_strings() {
            let configured: Option<SmallMap<CompactString, CompactString>> = fields
                .iter()
                .map(|field| {
                    Some((
                        field.clone(),
                        CompactString::new(names.get(field.as_str())?.unpack_str()?),
                    ))
                })
                .collect();
            if let Some(fields) = configured {
                return Ok(eval.heap().alloc_simple(StarlarkUserProvider {
                    id: id.dupe(),
                    fields,
                }));
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

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct StarlarkUserProvider {
    id: ProviderId,
    fields: SmallMap<CompactString, CompactString>,
}

starlark::starlark_simple_value!(StarlarkUserProvider);

impl StarlarkUserProvider {
    pub fn new(id: ProviderId, fields: SmallMap<CompactString, CompactString>) -> Self {
        Self { id, fields }
    }

    pub fn id(&self) -> &ProviderId {
        &self.id
    }

    pub fn fields(&self) -> &SmallMap<CompactString, CompactString> {
        &self.fields
    }
}

impl fmt::Display for StarlarkUserProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}(...)", self.id.exported_name())
    }
}

#[starlark_value(type = "provider")]
impl<'v> StarlarkValue<'v> for StarlarkUserProvider {
    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        self.fields
            .get(attribute)
            .map(|value| heap.alloc_str(value).to_value())
    }
}

#[derive(Debug, Trace, Freeze, ProvidesStaticType, NoSerialize, Allocative)]
pub struct StarlarkDepsetGen<V> {
    direct: Vec<V>,
}

pub type StarlarkDepset<'v> = StarlarkDepsetGen<Value<'v>>;
type FrozenStarlarkDepset = StarlarkDepsetGen<FrozenValue>;
starlark::starlark_complex_values!(StarlarkDepset);

impl<'v> StarlarkDepset<'v> {
    pub fn direct(&self) -> &[Value<'v>] {
        &self.direct
    }

    pub fn direct_from_value(value: Value<'v>) -> Option<Vec<Value<'v>>> {
        match Self::from_value(value)? {
            starlark::__macro_refs::Either::Left(value) => {
                Some(value.direct.iter().copied().collect())
            }
            starlark::__macro_refs::Either::Right(value) => {
                Some(value.direct.iter().map(|value| value.to_value()).collect())
            }
        }
    }
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

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct StarlarkToolchainInfo {
    marker: CompactString,
}

starlark::starlark_simple_value!(StarlarkToolchainInfo);

impl StarlarkToolchainInfo {
    pub fn marker(&self) -> &str {
        &self.marker
    }
}

impl fmt::Display for StarlarkToolchainInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ToolchainInfo(...)")
    }
}

#[starlark_value(type = "ToolchainInfo")]
impl<'v> StarlarkValue<'v> for StarlarkToolchainInfo {
    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        (attribute == "marker").then(|| heap.alloc_str(&self.marker).to_value())
    }
}

impl AnalysisBuiltinCallable {
    pub(crate) const fn new(name: &'static str) -> Self {
        Self { name }
    }
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
                let mut positions = args.positions(eval.heap())?;
                let direct = match (positions.next(), positions.next()) {
                    (None, None) => {
                        args.no_named_args()?;
                        return Ok(eval.heap().alloc(StarlarkDepset { direct: Vec::new() }));
                    }
                    (Some(direct), None) => direct,
                    _ => args.positional1(eval.heap())?,
                };
                let list = ListRef::from_value(direct).ok_or_else(|| {
                    starlark::Error::new_other(anyhow::anyhow!(
                        "depset direct elements must be a list"
                    ))
                })?;
                Ok(eval.heap().alloc(StarlarkDepset {
                    direct: list.iter().collect(),
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
                if eval
                    .extra
                    .and_then(|extra| extra.downcast_ref::<ToolchainInfoAnalysisContext>())
                    .is_none()
                {
                    return Err(starlark::Error::new_other(anyhow::anyhow!(
                        "unsupported analysis builtin ToolchainInfo"
                    )));
                }
                args.no_positional_args(eval.heap())?;
                let names = args.names_map()?;
                if names.len() != 1 {
                    return Err(starlark::Error::new_other(anyhow::anyhow!(
                        "ToolchainInfo requires exactly one named string `marker`"
                    )));
                }
                let marker = names.get("marker").ok_or_else(|| {
                    starlark::Error::new_other(anyhow::anyhow!(
                        "ToolchainInfo requires named argument `marker`"
                    ))
                })?;
                let marker = marker.unpack_str().ok_or_else(|| {
                    starlark::Error::new_other(anyhow::anyhow!(
                        "ToolchainInfo marker must be a string"
                    ))
                })?;
                Ok(eval.heap().alloc_simple(StarlarkToolchainInfo {
                    marker: marker.into(),
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
}
