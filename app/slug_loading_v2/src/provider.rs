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
use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use dupe::Dupe;
use slug_build_api_v2::ProviderId;
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
use starlark::values::Value;
use starlark::values::list::ListRef;
use starlark::values::starlark_value;
use starlark_map::small_map::SmallMap;

#[derive(Debug, ProvidesStaticType)]
pub(crate) struct BzlEvaluationContext {
    source_label: CompactString,
}

impl BzlEvaluationContext {
    pub(crate) fn new(source_label: impl Into<CompactString>) -> Self {
        Self {
            source_label: source_label.into(),
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
}

/// Loading-time provider constructor. `export_as` establishes its structural
/// identity before the containing module may freeze.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative, Trace)]
pub struct UserProviderCallable {
    source_label: CompactString,
    fields: Arc<[CompactString]>,
    #[allocative(skip)]
    id: OnceCell<ProviderId>,
}

impl UserProviderCallable {
    pub(crate) fn from_evaluator(
        fields: SmallMap<String, String>,
        eval: &Evaluator<'_, '_, '_>,
    ) -> anyhow::Result<Self> {
        let context = BzlEvaluationContext::from_evaluator(eval)
            .map_err(|_| anyhow::anyhow!("provider() may only be called in a .bzl module"))?;
        let mut names = fields
            .into_iter()
            .map(|(name, _documentation)| CompactString::new(name))
            .collect::<Vec<_>>();
        names.sort_unstable();
        names.dedup();
        Ok(Self {
            source_label: context.source_label.clone(),
            fields: names.into(),
            id: OnceCell::new(),
        })
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
            fields: self.fields,
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
        let id = self.id.get().ok_or_else(|| {
            starlark::Error::new_other(anyhow::anyhow!(
                "the result of provider() must be assigned before it can be called"
            ))
        })?;
        invoke_provider(id, &self.fields, args, eval)
    }
}

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct FrozenUserProviderCallable {
    id: ProviderId,
    fields: Arc<[CompactString]>,
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
        invoke_provider(&self.id, &self.fields, args, eval)
    }
}

fn invoke_provider<'v>(
    id: &ProviderId,
    fields: &[CompactString],
    args: &Arguments<'v, '_>,
    eval: &mut Evaluator<'v, '_, '_>,
) -> starlark::Result<Value<'v>> {
    args.no_positional_args(eval.heap())?;
    let names = args.names_map()?;
    for name in names.keys() {
        if !fields.iter().any(|field| field.as_str() == name.as_str()) {
            return Err(starlark::Error::new_other(anyhow::anyhow!(
                "provider {} received unknown field `{}`",
                id,
                name
            )));
        }
    }
    let mut values = SmallMap::with_capacity(fields.len());
    for field in fields {
        let value = names.get(field.as_str()).ok_or_else(|| {
            starlark::Error::new_other(anyhow::anyhow!(
                "provider {} is missing required field `{}`",
                id,
                field
            ))
        })?;
        let value = value.unpack_str().ok_or_else(|| {
            starlark::Error::new_other(anyhow::anyhow!(
                "provider {} field `{}` must be a string",
                id,
                field
            ))
        })?;
        values.insert(field.clone(), CompactString::new(value));
    }
    Ok(eval.heap().alloc_simple(StarlarkUserProvider {
        id: id.dupe(),
        fields: values,
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
    files: V,
}

pub type StarlarkDefaultInfo<'v> = StarlarkDefaultInfoGen<Value<'v>>;
type FrozenStarlarkDefaultInfo = StarlarkDefaultInfoGen<FrozenValue>;
starlark::starlark_complex_values!(StarlarkDefaultInfo);

impl<'v> StarlarkDefaultInfo<'v> {
    pub fn files(&self) -> Value<'v> {
        self.files
    }

    pub fn files_from_value(value: Value<'v>) -> Option<Value<'v>> {
        match Self::from_value(value)? {
            starlark::__macro_refs::Either::Left(value) => Some(value.files),
            starlark::__macro_refs::Either::Right(value) => Some(value.files.to_value()),
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
                let direct = args.positional1(eval.heap())?;
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
                let files = *names.get("files").ok_or_else(|| {
                    starlark::Error::new_other(anyhow::anyhow!(
                        "DefaultInfo requires named argument `files`"
                    ))
                })?;
                if names.len() != 1 {
                    return Err(starlark::Error::new_other(anyhow::anyhow!(
                        "DefaultInfo only supports `files` in this analysis packet"
                    )));
                }
                Ok(eval.heap().alloc(StarlarkDefaultInfo { files }))
            }
            _ => Err(starlark::Error::new_other(anyhow::anyhow!(
                "unsupported analysis builtin {}",
                self.name
            ))),
        }
    }
}
