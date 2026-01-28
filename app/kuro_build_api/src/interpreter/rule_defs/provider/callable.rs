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
use std::fmt;
use std::fmt::Display;
use std::fmt::Formatter;
use std::hash::BuildHasher;
use std::hash::Hasher;
use std::sync::Arc;

use allocative::Allocative;
use kuro_core::cells::cell_path::CellPath;
use kuro_core::provider::id::ProviderId;
use kuro_error::BuckErrorContext;
use kuro_error::conversion::from_any_with_tag;
use kuro_interpreter::build_context::starlark_path_from_build_context;
use kuro_interpreter::types::provider::callable::ProviderCallableLike;
use dupe::Dupe;
use either::Either;
use indexmap::IndexMap;
use itertools::Itertools;
use starlark::any::ProvidesStaticType;
use starlark::docs::DocItem;
use starlark::docs::DocMember;
use starlark::docs::DocProperty;
use starlark::docs::DocString;
use starlark::docs::DocStringKind;
use starlark::environment::GlobalsBuilder;
use starlark::eval::Arguments;
use starlark::eval::Evaluator;
use starlark::eval::ParametersSpec;
use starlark::eval::ParametersSpecParam;
use starlark::eval::param_specs;
use starlark::typing::Ty;
use starlark::typing::TyCallable;
use starlark::typing::TyStarlarkValue;
use starlark::values::AllocValue;
use starlark::values::Demand;
use starlark::values::Freeze;
use starlark::values::FreezeError;
use starlark::values::FreezeResult;
use starlark::values::Freezer;
use starlark::values::FrozenRef;
use starlark::values::FrozenValue;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::StarlarkValue;
use starlark::values::Trace;
use starlark::values::Value;
use starlark::values::ValueLike;
use starlark::values::dict::AllocDict;
use starlark::values::dict::DictRef;
use starlark::values::list::AllocList;
use starlark::values::list::ListRef;
use starlark::values::list_or_tuple::UnpackListOrTuple;
use starlark::values::starlark_value;
use starlark::values::starlark_value_as_type::StarlarkValueAsType;
use starlark::values::typing::TypeCompiled;
use starlark::values::typing::TypeInstanceId;
use starlark::values::typing::TypeMatcher;
use starlark::values::typing::TypeMatcherFactory;
use starlark_map::StarlarkHasher;
use starlark_map::StarlarkHasherBuilder;
use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;

use crate::interpreter::rule_defs::provider::doc::ProviderMembersSource;
use crate::interpreter::rule_defs::provider::doc::provider_callable_documentation;
use crate::interpreter::rule_defs::provider::ty::abstract_provider::AbstractProvider;
use crate::interpreter::rule_defs::provider::ty::provider::ty_provider;
use crate::interpreter::rule_defs::provider::ty::provider_callable::ty_provider_callable;
use crate::interpreter::rule_defs::provider::user::UserProvider;
use crate::interpreter::rule_defs::provider::user::user_provider_creator;

#[derive(Debug, kuro_error::Error)]
#[kuro(tag = Input)]
enum ProviderCallableError {
    #[error(
        "The result of `provider()` must be assigned to a top-level variable before it can be called"
    )]
    NotBound,
    #[error(
        "Provider type must be assigned to a variable, e.g. `ProviderInfo = provider(fields = {0:?})`"
    )]
    ProviderNotAssigned(SmallSet<String>),
    #[error("non-unique field names: [{}]", .0.iter().map(|s| format!("`{s}`")).join(", "))]
    NonUniqueFields(Vec<String>),
    #[error("Field default value can be either frozen value or an empty list or dict")]
    InvalidDefaultValue,
    #[error("Default value `{0}` (type `{1}`) does not match field type `{2}`")]
    InvalidDefaultValueType(String, &'static str, Ty),
}

/// `Hashed` from starlark contains the small hash,
/// we get it in `UserProvider::get_hashed`.
/// To lookup in `IndexMap` we can promote it to `u64`.
/// This is what this hasher does.
#[derive(Default, Debug, Clone, Copy, Dupe)]
pub(crate) struct StarlarkHasherSmallPromoteBuilder(StarlarkHasherBuilder);
pub(crate) struct StarlarkHasherSmallPromote(StarlarkHasher);

impl BuildHasher for StarlarkHasherSmallPromoteBuilder {
    type Hasher = StarlarkHasherSmallPromote;

    fn build_hasher(&self) -> Self::Hasher {
        StarlarkHasherSmallPromote(self.0.build_hasher())
    }
}

impl Hasher for StarlarkHasherSmallPromote {
    fn finish(&self) -> u64 {
        self.0.finish_small().promote()
    }

    fn write(&mut self, bytes: &[u8]) {
        self.0.write(bytes)
    }
}

fn create_callable_function_signature(
    function_name: &str,
    fields: &IndexMap<String, UserProviderField, StarlarkHasherSmallPromoteBuilder>,
    ret_ty: Ty,
) -> kuro_error::Result<(ParametersSpec<FrozenValue>, TyCallable)> {
    let (parameters_spec, param_spec) = param_specs(
        function_name,
        [],
        [],
        None,
        fields.iter().map(|(name, field)| {
            (
                name.as_str(),
                match field.default {
                    None => ParametersSpecParam::Required,
                    Some(default) => ParametersSpecParam::Defaulted(default),
                },
                field.ty.as_ty().dupe(),
            )
        }),
        None,
    )
    .internal_error("Must have created correct signature")?;

    Ok((parameters_spec, TyCallable::new(param_spec, ret_ty)))
}

// ============================================================================
// InitProviderConstructor - Wraps a provider with an init function
// ============================================================================

/// A provider constructor that uses an init function to transform arguments.
///
/// When a provider is defined with `init=fn`, this wrapper is returned as part
/// of the tuple. When invoked, it:
/// 1. Calls the init function with all provided arguments
/// 2. Expects init to return a dict mapping field names to values
/// 3. Uses that dict to construct the provider instance
///
/// This is used by rules_cc and other rulesets that define providers with
/// custom construction logic.
#[derive(Debug, ProvidesStaticType, Trace, NoSerialize, Allocative)]
pub struct InitProviderConstructor<'v> {
    /// The underlying provider callable
    provider: Value<'v>,
    /// The init function that transforms arguments
    init_fn: Value<'v>,
}

impl<'v> Display for InitProviderConstructor<'v> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "InitProviderConstructor({})", self.provider)
    }
}

impl<'v> AllocValue<'v> for InitProviderConstructor<'v> {
    fn alloc_value(self, heap: Heap<'v>) -> Value<'v> {
        heap.alloc_complex(self)
    }
}

impl Freeze for InitProviderConstructor<'_> {
    type Frozen = FrozenInitProviderConstructor;
    fn freeze(self, freezer: &Freezer) -> FreezeResult<Self::Frozen> {
        Ok(FrozenInitProviderConstructor {
            provider: freezer.freeze(self.provider)?,
            init_fn: freezer.freeze(self.init_fn)?,
        })
    }
}

#[starlark_value(type = "InitProviderConstructor")]
impl<'v> StarlarkValue<'v> for InitProviderConstructor<'v> {
    type Canonical = FrozenInitProviderConstructor;

    fn invoke(
        &self,
        _me: Value<'v>,
        args: &Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        invoke_init_provider_constructor(self.init_fn, self.provider, args, eval)
    }

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderCallableLike>(self);
    }
}

// InitProviderConstructor implements ProviderCallableLike by delegating to underlying provider
impl<'v> ProviderCallableLike for InitProviderConstructor<'v> {
    fn id(&self) -> kuro_error::Result<&Arc<ProviderId>> {
        self.provider
            .request_value::<&dyn ProviderCallableLike>()
            .ok_or_else(|| {
                kuro_error::kuro_error!(
                    kuro_error::ErrorTag::Input,
                    "InitProviderConstructor contains invalid provider"
                )
            })?
            .id()
    }
}

/// Frozen version of InitProviderConstructor
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct FrozenInitProviderConstructor {
    /// The underlying provider callable
    provider: FrozenValue,
    /// The init function that transforms arguments
    init_fn: FrozenValue,
}

impl Display for FrozenInitProviderConstructor {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "InitProviderConstructor({})", self.provider)
    }
}

starlark_simple_value!(FrozenInitProviderConstructor);

#[starlark_value(type = "InitProviderConstructor")]
impl<'v> StarlarkValue<'v> for FrozenInitProviderConstructor {
    type Canonical = Self;

    fn invoke(
        &self,
        _me: Value<'v>,
        args: &Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        invoke_init_provider_constructor(
            self.init_fn.to_value(),
            self.provider.to_value(),
            args,
            eval,
        )
    }

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderCallableLike>(self);
    }
}

// InitProviderConstructor implements ProviderCallableLike by delegating to underlying provider
impl ProviderCallableLike for FrozenInitProviderConstructor {
    fn id(&self) -> kuro_error::Result<&Arc<ProviderId>> {
        self.provider
            .to_value()
            .request_value::<&dyn ProviderCallableLike>()
            .ok_or_else(|| {
                kuro_error::kuro_error!(
                    kuro_error::ErrorTag::Input,
                    "InitProviderConstructor contains invalid provider"
                )
            })?
            .id()
    }
}

/// Shared implementation for invoking init provider constructor
fn invoke_init_provider_constructor<'v>(
    init_fn: Value<'v>,
    provider: Value<'v>,
    args: &Arguments<'v, '_>,
    eval: &mut Evaluator<'v, '_, '_>,
) -> starlark::Result<Value<'v>> {
    // Call the init function with all the provided arguments
    let init_result = init_fn.invoke(args, eval)?;

    // The init function should return a dict mapping field names to values
    let dict = DictRef::from_value(init_result).ok_or_else(|| {
        kuro_error::kuro_error!(
            kuro_error::ErrorTag::Input,
            "provider init function must return a dict, got {}",
            init_result.get_type()
        )
    })?;

    // Convert the dict to keyword arguments for the provider constructor
    let mut named_args: Vec<(&str, Value<'v>)> = Vec::new();
    for (k, v) in dict.iter() {
        let key_str = k.unpack_str().ok_or_else(|| {
            kuro_error::kuro_error!(
                kuro_error::ErrorTag::Input,
                "provider init function must return a dict with string keys, got key of type {}",
                k.get_type()
            )
        })?;
        named_args.push((key_str, v));
    }

    // Call the provider with the dict as keyword arguments
    eval.eval_function(provider, &[], &named_args)
}

#[derive(Debug, Allocative)]
pub(crate) struct UserProviderCallableData {
    pub(crate) provider_id: Arc<ProviderId>,
    /// Type id of provider callable instance.
    pub(crate) ty_provider_type_instance_id: TypeInstanceId,
    pub(crate) fields: IndexMap<String, UserProviderField, StarlarkHasherSmallPromoteBuilder>,
}

/// Initialized after the name is assigned to the provider.
#[derive(Debug, Trace, Allocative)]
struct UserProviderCallableNamed {
    /// The name of this provider, filled in by `export_as()`. This must be set before this
    /// object can be called and Providers created.
    id: Arc<ProviderId>,
    signature: ParametersSpec<FrozenValue>,
    /// This field is shared with provider instances.
    data: FrozenRef<'static, UserProviderCallableData>,
    /// Type of provider instance.
    ty_provider: Ty,
    /// Type of provider callable.
    ty_callable: Ty,
}

impl UserProviderCallableNamed {
    fn invoke<'v>(
        &self,
        args: &Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        self.signature.parser(args, eval, |parser, eval| {
            user_provider_creator(self.data, eval, parser).map_err(Into::into)
        })
    }
}

#[derive(Debug, Trace, Allocative, ProvidesStaticType, NoSerialize, Clone, Dupe)]
pub(crate) struct UserProviderField {
    /// Field type.
    pub(crate) ty: TypeCompiled<FrozenValue>,
    /// Default value. If `None`, the field is required.
    pub(crate) default: Option<FrozenValue>,
}

impl<'v> AllocValue<'v> for UserProviderField {
    fn alloc_value(self, heap: Heap<'v>) -> Value<'v> {
        heap.alloc_simple(self)
    }
}

impl Display for UserProviderField {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "ProviderField({}, ", self.ty)?;
        if let Some(default) = &self.default {
            write!(f, "default = {default}")?;
        } else {
            write!(f, "required")?;
        }
        write!(f, ")")
    }
}

impl UserProviderField {
    pub(crate) fn default() -> UserProviderField {
        UserProviderField {
            ty: TypeCompiled::any(),
            default: Some(FrozenValue::new_none()),
        }
    }
}

#[starlark_value(type = "ProviderField")]
impl<'v> StarlarkValue<'v> for UserProviderField {}

/// The result of calling `provider()`. This is a callable that accepts the fields
/// provided in the `provider()` call, and generates a Starlark `UserProvider` object.
///
/// This object must be assigned to a variable at the top level of the module before it may be invoked
///
/// Field values default to `None`
#[derive(Debug, ProvidesStaticType, Trace, NoSerialize, Allocative)]
pub struct UserProviderCallable {
    /// The path where this `ProviderCallable` is created and assigned
    path: CellPath,
    /// The docstring for this provider
    docs: Option<DocString>,
    /// The names of the fields used in `callable`
    fields: IndexMap<String, UserProviderField, StarlarkHasherSmallPromoteBuilder>,
    /// Field is initialized after the provider is assigned to a variable.
    callable: OnceCell<UserProviderCallableNamed>,
}

fn user_provider_callable_display(
    id: Option<&Arc<ProviderId>>,
    fields: &IndexMap<String, UserProviderField, StarlarkHasherSmallPromoteBuilder>,
    f: &mut Formatter,
) -> fmt::Result {
    write!(f, "provider")?;
    if let Some(id) = id {
        write!(f, "[{}]", id.name)?;
    }
    write!(f, "(fields={{")?;
    for (i, (name, ty)) in fields.iter().enumerate() {
        if i != 0 {
            write!(f, ", ")?;
        }
        write!(f, "\"{}\": provider_field({}", name, ty.ty)?;
        if let Some(default) = ty.default {
            write!(f, ", default={default}")?;
        }
        write!(f, ")")?;
    }
    write!(f, "}})")?;
    Ok(())
}

impl Display for UserProviderCallable {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        user_provider_callable_display(self.callable.get().map(|x| &x.id), &self.fields, f)
    }
}

impl UserProviderCallable {
    fn new(
        path: CellPath,
        docs: Option<DocString>,
        fields: IndexMap<String, UserProviderField, StarlarkHasherSmallPromoteBuilder>,
    ) -> Self {
        Self {
            callable: OnceCell::new(),
            path,
            docs,
            fields,
        }
    }
}

impl ProviderCallableLike for UserProviderCallable {
    fn id(&self) -> kuro_error::Result<&Arc<ProviderId>> {
        self.callable
            .get()
            .map(|x| &x.id)
            .ok_or(ProviderCallableError::NotBound.into())
    }
}

impl<'v> AllocValue<'v> for UserProviderCallable {
    fn alloc_value(self, heap: Heap<'v>) -> Value<'v> {
        heap.alloc_complex(self)
    }
}

impl Freeze for UserProviderCallable {
    type Frozen = FrozenUserProviderCallable;
    fn freeze(self, _freezer: &Freezer) -> FreezeResult<Self::Frozen> {
        let callable = self.callable.into_inner();
        let callable = match callable {
            Some(x) => x,
            None => {
                // Unfortunately we have no name or location for the provider at this point,
                // so reproduce the fields so that the provider can be identified.
                return Err(FreezeError::new(
                    ProviderCallableError::ProviderNotAssigned(
                        self.fields.into_iter().map(|(name, _)| name).collect(),
                    )
                    .to_string(),
                ));
            }
        };

        Ok(FrozenUserProviderCallable::new(
            self.docs,
            self.fields,
            callable,
        ))
    }
}

#[derive(Debug, Clone, Allocative)]
struct UserProviderMatcher {
    type_instance_id: TypeInstanceId,
}

impl TypeMatcher for UserProviderMatcher {
    fn matches(&self, value: Value) -> bool {
        match UserProvider::from_value(value) {
            Some(x) => {
                // TODO(nga): this is a bit suboptimal:
                //   instead we could compare just a pointer to the callable.
                x.callable.ty_provider_type_instance_id == self.type_instance_id
            }
            None => false,
        }
    }
}

#[starlark_value(type = "ProviderCallable")]
impl<'v> StarlarkValue<'v> for UserProviderCallable {
    type Canonical = FrozenUserProviderCallable;

    fn export_as(
        &self,
        variable_name: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<()> {
        // First export wins
        self.callable.get_or_try_init(|| {
            let provider_id = Arc::new(ProviderId {
                path: Some(self.path.clone()),
                name: variable_name.to_owned(),
            });
            let ty_provider_type_instance_id = TypeInstanceId::r#gen();
            let ty_provider = ty_provider(
                &provider_id.name,
                ty_provider_type_instance_id,
                TyStarlarkValue::new::<UserProvider>(),
                Some(TypeMatcherFactory::new(UserProviderMatcher {
                    type_instance_id: ty_provider_type_instance_id,
                })),
                self.fields
                    .iter()
                    .map(|(name, field)| (name.to_owned(), field.ty.as_ty().dupe()))
                    .collect(),
            )?;
            let (signature, creator_func) = create_callable_function_signature(
                &provider_id.name,
                &self.fields,
                ty_provider.clone(),
            )?;
            let ty_callable = ty_provider_callable::<UserProviderCallable>(creator_func)?;
            kuro_error::Ok(UserProviderCallableNamed {
                id: provider_id.dupe(),
                signature,
                data: eval.frozen_heap().alloc_any(UserProviderCallableData {
                    provider_id,
                    fields: self.fields.clone(),
                    ty_provider_type_instance_id,
                }),
                ty_provider,
                ty_callable,
            })
        })?;
        Ok(())
    }

    fn invoke(
        &self,
        _me: Value<'v>,
        args: &Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        match self.callable.get() {
            Some(callable) => callable.invoke(args, eval),
            None => Err(kuro_error::Error::from(ProviderCallableError::NotBound).into()),
        }
    }

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderCallableLike>(self);
    }

    fn eval_type(&self) -> Option<Ty> {
        self.callable.get().map(|named| named.ty_provider.dupe())
    }

    fn documentation(&self) -> DocItem {
        let return_types = vec![Ty::any(); self.fields.len()];
        let Some(callable) = self.callable.get() else {
            // This shouldn't really happen, we mostly don't even ask for documentation on
            // non-frozen things
            return DocItem::Member(DocMember::Property(DocProperty {
                docs: None,
                typ: Ty::any(),
            }));
        };
        let field_names: Vec<_> = self.fields.keys().map(|x| x.as_str()).collect();
        provider_callable_documentation(
            None,
            ProviderMembersSource::FromFields {
                fields: &field_names,
                // TODO(nga): types.
                field_docs: &vec![None; self.fields.len()],
                field_types: &return_types,
            },
            callable.ty_callable.dupe(),
            &self.docs,
        )
    }

    fn typechecker_ty(&self) -> Option<Ty> {
        self.callable.get().map(|named| named.ty_callable.dupe())
    }
}

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct FrozenUserProviderCallable {
    /// The docstring for this provider
    docs: Option<DocString>,
    /// The names of the fields used in `callable`
    fields: IndexMap<String, UserProviderField, StarlarkHasherSmallPromoteBuilder>,
    /// The actual callable that creates instances of `UserProvider`
    callable: UserProviderCallableNamed,
}
starlark_simple_value!(FrozenUserProviderCallable);

impl Display for FrozenUserProviderCallable {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        user_provider_callable_display(Some(&self.callable.id), &self.fields, f)
    }
}

impl FrozenUserProviderCallable {
    fn new(
        docs: Option<DocString>,
        fields: IndexMap<String, UserProviderField, StarlarkHasherSmallPromoteBuilder>,
        callable: UserProviderCallableNamed,
    ) -> Self {
        Self {
            docs,
            fields,
            callable,
        }
    }
}

impl ProviderCallableLike for FrozenUserProviderCallable {
    fn id(&self) -> kuro_error::Result<&Arc<ProviderId>> {
        Ok(&self.callable.id)
    }
}

#[starlark_value(type = "ProviderCallable")]
impl<'v> StarlarkValue<'v> for FrozenUserProviderCallable {
    type Canonical = Self;

    fn invoke(
        &self,
        _me: Value<'v>,
        args: &Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        self.callable.invoke(args, eval)
    }

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderCallableLike>(self);
    }

    fn documentation(&self) -> DocItem {
        let return_types = vec![Ty::any(); self.fields.len()];
        let field_names: Vec<_> = self.fields.keys().map(|x| x.as_str()).collect();
        provider_callable_documentation(
            None,
            ProviderMembersSource::FromFields {
                fields: &field_names,
                field_docs: &vec![None; self.fields.len()],
                field_types: &return_types,
            },
            self.callable.ty_callable.dupe(),
            &self.docs,
        )
    }

    fn typechecker_ty(&self) -> Option<Ty> {
        Some(self.callable.ty_callable.dupe())
    }

    fn eval_type(&self) -> Option<Ty> {
        Some(self.callable.ty_provider.dupe())
    }
}

fn provider_field_parse_type<'v>(
    ty: Value<'v>,
    eval: &mut Evaluator<'v, '_, '_>,
) -> kuro_error::Result<TypeCompiled<FrozenValue>> {
    TypeCompiled::new(ty, eval.heap())
        .map(|ty| ty.to_frozen(eval.frozen_heap()))
        .map_err(|e| from_any_with_tag(e, kuro_error::ErrorTag::Interpreter))
}

#[starlark_module]
pub fn register_provider(builder: &mut GlobalsBuilder) {
    /// Create a field definition object which can be passed to `provider` type constructor.
    fn provider_field<'v>(
        #[starlark(require=pos)] ty: Value<'v>,
        #[starlark(require=named)] default: Option<Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<UserProviderField> {
        let ty = provider_field_parse_type(ty, eval)?;
        let default = match default {
            None => None,
            Some(x) => {
                if let Some(x) = x.unpack_frozen() {
                    Some(x)
                } else if ListRef::from_value(x).is_some_and(|x| x.is_empty()) {
                    Some(eval.frozen_heap().alloc(AllocList::EMPTY))
                } else if DictRef::from_value(x).is_some_and(|x| x.is_empty()) {
                    Some(eval.frozen_heap().alloc(AllocDict::EMPTY))
                } else {
                    // Dealing only with frozen values is much easier.
                    return Err(kuro_error::Error::from(
                        ProviderCallableError::InvalidDefaultValue,
                    )
                    .into());
                }
            }
        };
        if let Some(default) = default {
            if !ty.matches(default.to_value()) {
                return Err(kuro_error::Error::from(
                    ProviderCallableError::InvalidDefaultValueType(
                        default.to_string(),
                        default.to_value().get_type(),
                        ty.as_ty().dupe(),
                    ),
                )
                .into());
            }
        }
        Ok(UserProviderField { ty, default })
    }

    /// Create a `"provider"` type that can be returned from `rule` implementations.
    /// Used to pass information from a rule to the things that depend on it.
    /// Typically named with an `Info` suffix.
    ///
    /// ```python
    /// GroovyLibraryInfo(fields = [
    ///     "objects",  # a list of artifacts
    ///     "options",  # a string containing compiler options
    /// ])
    /// ```
    ///
    /// Given a dependency you can obtain the provider with `my_dep[GroovyLibraryInfo]`
    /// which returns either `None` or a value of type `GroovyLibraryInfo`.
    ///
    /// For providers that accumulate upwards a transitive set is often a good choice.
    fn provider<'v>(
        // Allow doc as positional or named argument for Bazel compatibility
        // Bazel supports both: provider("doc", fields={...}) and provider(doc="doc", fields={...})
        #[starlark(default = "")] doc: &str,
        // Fields is optional in Bazel - when not specified, provider accepts any fields
        #[starlark(require=named)] fields: Option<Either<
            UnpackListOrTuple<String>,
            SmallMap<String, Value<'v>>,
        >>,
        // Bazel-compatible: init function for custom construction logic
        // When specified, provider() returns (ProviderType, constructor_fn)
        #[starlark(require=named)] init: Option<Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let docstring = DocString::from_docstring(DocStringKind::Starlark, doc);
        let path = starlark_path_from_build_context(eval)?.path();

        // If fields not specified, default to empty (provider accepts any field names)
        let fields: IndexMap<String, UserProviderField, StarlarkHasherSmallPromoteBuilder> = match fields {
            None => IndexMap::with_hasher(StarlarkHasherSmallPromoteBuilder::default()),
            Some(Either::Left(fields)) => {
                let new_fields: IndexMap<
                    String,
                    UserProviderField,
                    StarlarkHasherSmallPromoteBuilder,
                > = fields
                    .items
                    .iter()
                    .map(|name| (name.clone(), UserProviderField::default()))
                    .collect();
                if new_fields.len() != fields.items.len() {
                    return Err(
                        kuro_error::Error::from(ProviderCallableError::NonUniqueFields(
                            fields.items,
                        ))
                        .into(),
                    );
                }
                new_fields
            }
            Some(Either::Right(fields)) => {
                let mut new_fields = IndexMap::with_capacity_and_hasher(
                    fields.len(),
                    StarlarkHasherSmallPromoteBuilder::default(),
                );
                for (name, field) in fields {
                    if let Some(field) = field.downcast_ref::<UserProviderField>() {
                        new_fields.insert(name, field.dupe());
                    } else if field.unpack_str().is_some() {
                        // Bazel compatibility: field value can be a doc string
                        // In this case, the field has no type constraint
                        new_fields.insert(name, UserProviderField::default());
                    } else {
                        let ty = provider_field_parse_type(field, eval)
                            .with_buck_error_context(|| format!("Field `{name}` type `{field}` is not created with `provider_field`, and cannot be evaluated as a type"))?;
                        new_fields.insert(name, UserProviderField { ty, default: None });
                    }
                }
                new_fields
            }
        };
        let provider = UserProviderCallable::new(path.into_owned(), docstring, fields);

        // If init is provided, return (init_wrapped_provider, raw_provider) tuple
        // The first element wraps the provider with the init function
        // The second element is the raw provider constructor
        if let Some(init_fn) = init {
            let provider_val = eval.heap().alloc(provider);
            // Create init-wrapped constructor that calls init then provider
            let init_constructor = InitProviderConstructor {
                provider: provider_val,
                init_fn,
            };
            let init_constructor_val = eval.heap().alloc(init_constructor);
            // Return tuple of (init_wrapped, raw_provider)
            // Most code uses the first element which goes through init
            Ok(eval.heap().alloc((init_constructor_val, provider_val)))
        } else {
            Ok(eval.heap().alloc(provider))
        }
    }

    /// Provider type, can be used in type expressions.
    ///
    /// # Examples
    ///
    /// ```python
    /// def foo() -> list[Provider]:
    ///     return [DefaultInfo()]
    /// ```
    const Provider: StarlarkValueAsType<AbstractProvider> = StarlarkValueAsType::new_no_docs();
}
