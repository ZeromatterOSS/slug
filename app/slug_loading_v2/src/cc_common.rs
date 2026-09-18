/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::fmt;
use std::hash::Hash;

use allocative::Allocative;
use compact_str::CompactString;
use slug_build_api_v2::CcHeaderInfoOccurrence;
use starlark::any::ProvidesStaticType;
use starlark::collections::StarlarkHasher;
use starlark::environment::GlobalsBuilder;
use starlark::environment::Methods;
use starlark::environment::MethodsBuilder;
use starlark::environment::MethodsStatic;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::values::Demand;
use starlark::values::Freeze;
use starlark::values::FrozenHeap;
use starlark::values::FrozenValue;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::StarlarkValue;
use starlark::values::Trace;
use starlark::values::Value;
use starlark::values::ValueLike;
use starlark::values::dict::AllocImmutableDict;
use starlark::values::dict::DictRef;
use starlark::values::list::AllocImmutableList;
use starlark::values::list::AllocList;
use starlark::values::list::ListRef;
use starlark::values::none::NoneType;
use starlark::values::starlark_value;
use starlark::values::structs::StarlarkStructuralValue;
use starlark::values::tuple::TupleRef;

use crate::builtin_restriction::CustomAllowlistEntry;
use crate::builtin_restriction::check_custom_allowlist;
use crate::builtin_restriction::source_identities_for_evaluator;
use crate::provider::BzlEvaluationContext;
use crate::subrule_invocation::AnalysisActions;

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct CcCommonModule;
starlark::starlark_simple_value!(CcCommonModule);

impl fmt::Display for CcCommonModule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("cc_common")
    }
}

#[starlark_value(type = "cc_common")]
impl<'v> StarlarkValue<'v> for CcCommonModule {
    fn get_attr(&self, attribute: &str, _heap: Heap<'v>) -> Option<Value<'v>> {
        (attribute == "do_not_use_tools_cpp_compiler_present").then_some(Value::new_none())
    }

    fn get_methods() -> Option<&'static Methods> {
        static METHODS: MethodsStatic = MethodsStatic::new();
        METHODS.methods(cc_common_methods)
    }
}

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct CcInternalModule;
starlark::starlark_simple_value!(CcInternalModule);

impl fmt::Display for CcInternalModule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("cc_internal")
    }
}

#[starlark_value(type = "cc_internal")]
impl<'v> StarlarkValue<'v> for CcInternalModule {
    fn get_methods() -> Option<&'static Methods> {
        static METHODS: MethodsStatic = MethodsStatic::new();
        METHODS.methods(cc_internal_methods)
    }
}

#[derive(Debug, Trace, Freeze, ProvidesStaticType, NoSerialize, Allocative)]
struct EmptyCcHeaderInfoGen<V> {
    #[trace(unsafe_ignore)]
    #[freeze(identity)]
    occurrence: CcHeaderInfoOccurrence,
    empty_headers: V,
}

type EmptyCcHeaderInfo<'v> = EmptyCcHeaderInfoGen<Value<'v>>;
type FrozenEmptyCcHeaderInfo = EmptyCcHeaderInfoGen<FrozenValue>;
starlark::starlark_complex_values!(EmptyCcHeaderInfo);

impl<V> fmt::Display for EmptyCcHeaderInfoGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("HeaderInfo")
    }
}

#[starlark_value(type = "HeaderInfo")]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for EmptyCcHeaderInfoGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    type Canonical = FrozenEmptyCcHeaderInfo;

    fn equals(&self, other: Value<'v>) -> starlark::Result<bool> {
        Ok(empty_cc_header_info_occurrence(other).is_some_and(|other| self.occurrence == other))
    }
    fn write_hash(&self, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        self.occurrence.hash(hasher);
        Ok(())
    }
    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn StarlarkStructuralValue>(self);
    }

    fn get_attr(&self, attribute: &str, _heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "header_module" | "pic_header_module" | "separate_module" | "separate_pic_module" => {
                Some(Value::new_none())
            }
            "modular_public_headers"
            | "modular_private_headers"
            | "textual_headers"
            | "separate_module_headers" => Some(self.empty_headers.to_value()),
            _ => None,
        }
    }

    fn dir_attr(&self) -> Vec<String> {
        [
            "header_module",
            "pic_header_module",
            "modular_public_headers",
            "modular_private_headers",
            "textual_headers",
            "separate_module_headers",
            "separate_module",
            "separate_pic_module",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect()
    }
}

impl<V> StarlarkStructuralValue for EmptyCcHeaderInfoGen<V> {
    fn is_structurally_immutable(&self) -> bool {
        true
    }
    fn write_structural_hash(&self, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        self.occurrence.hash(hasher);
        Ok(())
    }
}

pub fn empty_cc_header_info_occurrence(value: Value<'_>) -> Option<CcHeaderInfoOccurrence> {
    match EmptyCcHeaderInfo::from_value(value)? {
        starlark::__macro_refs::Either::Left(value) => Some(value.occurrence.clone()),
        starlark::__macro_refs::Either::Right(value) => Some(value.occurrence.clone()),
    }
}

pub fn alloc_frozen_empty_cc_header_info(
    heap: &FrozenHeap,
    occurrence: CcHeaderInfoOccurrence,
) -> FrozenValue {
    heap.alloc(FrozenEmptyCcHeaderInfo {
        occurrence,
        empty_headers: heap.alloc(AllocList::EMPTY),
    })
}

#[starlark_module]
fn cc_internal_methods(builder: &mut MethodsBuilder) {
    fn absolute_symlink<'v>(
        #[starlark(this)] _cc_internal: Value<'v>,
        #[starlark(require = named)] ctx: Value<'v>,
        #[starlark(require = named)] output: Value<'v>,
        #[starlark(require = named)] target_path: &str,
        #[starlark(require = named)] progress_message: Option<&str>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<NoneType> {
        let actions = ctx
            .get_attr("actions", eval.heap())
            .map_err(|error| anyhow::anyhow!(error.to_string()))?
            .and_then(AnalysisActions::from_value)
            .ok_or_else(|| anyhow::anyhow!("absolute_symlink requires a rule or subrule ctx"))?;
        actions.register_absolute_symlink(output, target_path, progress_message)?;
        Ok(NoneType)
    }

    fn check_private_api<'v>(
        #[starlark(this)] _cc_internal: Value<'v>,
        #[starlark(require = named)] allowlist: Value<'v>,
        #[starlark(require = named, default = 1)] depth: i32,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<NoneType> {
        let depth = usize::try_from(depth)
            .map_err(|_| anyhow::anyhow!("check_private_api depth must be nonnegative"))?;
        let entries = custom_allowlist(allowlist)?;
        let identities = source_identities_for_evaluator(eval)?;
        check_custom_allowlist(eval, &identities, &entries, depth)?;
        Ok(NoneType)
    }

    fn freeze<'v>(
        #[starlark(this)] _cc_internal: Value<'v>,
        value: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        if let Some(dict) = DictRef::from_value(value) {
            return Ok(eval.heap().alloc(AllocImmutableDict(dict.iter())));
        }
        if let Some(list) = ListRef::from_value(value) {
            return Ok(eval.heap().alloc(AllocImmutableList(list.iter())));
        }
        if let Some(tuple) = TupleRef::from_value(value) {
            return Ok(eval.heap().alloc(AllocImmutableList(tuple.iter())));
        }
        // Pinned CcStarlarkInternal.freeze leaves non-iterable values untouched.
        if value.unpack_str().is_some() || value.unpack_bool().is_some() {
            anyhow::bail!("cc_internal.freeze requires a Starlark value, not string or bool");
        }
        if matches!(value.get_type(), "range" | "set") {
            anyhow::bail!(
                "cc_internal.freeze {} conversion is not supported",
                value.get_type()
            );
        }
        Ok(value)
    }

    fn create_header_info<'v>(
        #[starlark(this)] _cc_internal: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        let empty_headers = eval.frozen_heap().alloc(AllocList::EMPTY).to_value();
        Ok(eval.heap().alloc_complex(EmptyCcHeaderInfoGen {
            occurrence: CcHeaderInfoOccurrence::new(),
            empty_headers,
        }))
    }
}

fn custom_allowlist(value: Value<'_>) -> anyhow::Result<Vec<CustomAllowlistEntry>> {
    let values = if let Some(values) = ListRef::from_value(value) {
        values.iter().collect::<Vec<_>>()
    } else if let Some(values) = TupleRef::from_value(value) {
        values.iter().collect::<Vec<_>>()
    } else {
        anyhow::bail!("check_private_api allowlist must be a sequence of tuples")
    };
    values
        .into_iter()
        .map(|value| {
            let pair = TupleRef::from_value(value).ok_or_else(|| {
                anyhow::anyhow!("check_private_api allowlist entries must be two-string tuples")
            })?;
            let [apparent_repo, package_prefix] = pair.iter().collect::<Vec<_>>()[..] else {
                anyhow::bail!("check_private_api allowlist entries must be two-string tuples")
            };
            let apparent_repo = apparent_repo.unpack_str().ok_or_else(|| {
                anyhow::anyhow!("check_private_api allowlist entries must be two-string tuples")
            })?;
            let package_prefix = package_prefix.unpack_str().ok_or_else(|| {
                anyhow::anyhow!("check_private_api allowlist entries must be two-string tuples")
            })?;
            Ok(CustomAllowlistEntry {
                apparent_repo: CompactString::new(apparent_repo),
                package_prefix: CompactString::new(package_prefix),
            })
        })
        .collect()
}

#[starlark_module]
fn cc_common_methods(builder: &mut MethodsBuilder) {
    #[allow(non_snake_case)]
    fn internal_DO_NOT_USE<'v>(
        #[starlark(this)] _cc_common: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<CcInternalModule> {
        let context = BzlEvaluationContext::from_evaluator(eval)?;
        let source = context.source_identity_for_call(eval)?;
        if !source
            .label
            .package()
            .repo()
            .as_str()
            .starts_with("rules_cc+")
        {
            let canonical = source.label.to_string();
            let canonical = if source.label.package().repo().is_root() {
                canonical.strip_prefix("@@").unwrap_or(&canonical)
            } else {
                &canonical
            };
            anyhow::bail!("file '{canonical}' cannot use private API");
        }
        Ok(CcInternalModule)
    }
}

pub(crate) fn cc_common_globals(builder: &mut GlobalsBuilder) {
    builder.set("cc_common", CcCommonModule);
}
