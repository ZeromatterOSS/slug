//! Native HeaderInfo values own their complete artifact/dependency model.
use std::fmt;
use std::hash::Hash;

use allocative::Allocative;
use slug_build_api_v2::AnalysisArtifact;
use slug_build_api_v2::CcHeaderInfo;
use slug_build_api_v2::CcHeaderInfoInput;
use starlark::any::ProvidesStaticType;
use starlark::collections::StarlarkHasher;
use starlark::eval::Evaluator;
use starlark::values::Demand;
use starlark::values::FrozenHeap;
use starlark::values::FrozenValue;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::StarlarkValue;
use starlark::values::Value;
use starlark::values::list::AllocImmutableList;
use starlark::values::list::ListRef;
use starlark::values::starlark_value;
use starlark::values::structs::StarlarkStructuralValue;
use starlark::values::tuple::TupleRef;

use crate::subrule_invocation::AnalysisArtifactValue;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct HeaderInfoValue(CcHeaderInfo);
starlark::starlark_simple_value!(HeaderInfoValue);

impl fmt::Display for HeaderInfoValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("HeaderInfo")
    }
}

#[starlark_value(type = "HeaderInfo")]
impl<'v> StarlarkValue<'v> for HeaderInfoValue {
    fn equals(&self, other: Value<'v>) -> starlark::Result<bool> {
        Ok(cc_header_info(other).is_some_and(|other| self.0 == other))
    }
    fn write_hash(&self, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        self.0.hash(hasher);
        Ok(())
    }
    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn StarlarkStructuralValue>(self);
    }
    fn get_attr(&self, name: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        let model = self.0.model();
        let module = match name {
            "header_module" => Some(&model.header_module),
            "pic_header_module" => Some(&model.pic_header_module),
            "separate_module" => Some(&model.separate_module),
            "separate_pic_module" => Some(&model.separate_pic_module),
            _ => None,
        };
        if let Some(module) = module {
            return Some(module.as_ref().map_or_else(Value::new_none, |artifact| {
                heap.alloc(AnalysisArtifactValue::new(artifact.clone()))
            }));
        }
        let headers = match name {
            "modular_public_headers" => &model.modular_public_headers,
            "modular_private_headers" => &model.modular_private_headers,
            "textual_headers" => &model.textual_headers,
            "separate_module_headers" => &model.separate_module_headers,
            _ => return None,
        };
        // Native getters make immutable list copies. No evaluator projection is
        // stored in the retained model or carried across heaps.
        Some(
            heap.alloc(AllocImmutableList(headers.iter().map(|artifact| {
                heap.alloc(AnalysisArtifactValue::new(artifact.clone()))
            }))),
        )
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
impl StarlarkStructuralValue for HeaderInfoValue {
    fn is_structurally_immutable(&self) -> bool {
        true
    }
    fn write_structural_hash(&self, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        self.0.hash(hasher);
        Ok(())
    }
}

pub fn cc_header_info(value: Value<'_>) -> Option<CcHeaderInfo> {
    HeaderInfoValue::from_value(value).map(|value| value.0.clone())
}
pub fn alloc_frozen_cc_header_info(heap: &FrozenHeap, info: CcHeaderInfo) -> FrozenValue {
    heap.alloc(HeaderInfoValue(info))
}

fn sequence<'v>(value: Value<'v>, name: &str, heap: Heap<'v>) -> anyhow::Result<Vec<Value<'v>>> {
    if let Some(list) = ListRef::from_value(value) {
        Ok(list.iter().collect())
    } else if let Some(tuple) = TupleRef::from_value(value) {
        Ok(tuple.iter().collect())
    } else if value.get_type() == "range" {
        // Ranges contain integers, so a nonempty one fails the caller's typed
        // element check immediately. Never materialize a potentially huge range.
        Ok(value
            .iterate(heap)
            .map_err(starlark::Error::into_anyhow)?
            .take(1)
            .collect())
    } else {
        anyhow::bail!("{name} requires a sequence")
    }
}
pub(crate) fn headers<'v>(
    value: Value<'v>,
    name: &str,
    heap: Heap<'v>,
) -> anyhow::Result<Box<[AnalysisArtifact]>> {
    sequence(value, name, heap)?
        .into_iter()
        .map(|value| {
            AnalysisArtifactValue::from_starlark(value)
                .map(|value| value.artifact().clone())
                .ok_or_else(|| anyhow::anyhow!("{name} requires Files"))
        })
        .collect()
}
pub(crate) fn module(value: Value<'_>, name: &str) -> anyhow::Result<Option<AnalysisArtifact>> {
    if value.is_none() {
        return Ok(None);
    }
    AnalysisArtifactValue::from_starlark(value)
        .map(|value| Some(value.artifact().clone()))
        .ok_or_else(|| anyhow::anyhow!("{name} requires a derived File or None"))
}
pub(crate) fn dependencies<'v>(
    value: Value<'v>,
    name: &str,
    heap: Heap<'v>,
) -> anyhow::Result<Box<[CcHeaderInfo]>> {
    sequence(value, name, heap)?
        .into_iter()
        .map(|value| {
            cc_header_info(value)
                .ok_or_else(|| anyhow::anyhow!("{name} requires HeaderInfo values"))
        })
        .collect()
}
pub(crate) fn create<'v>(
    input: CcHeaderInfoInput,
    eval: &Evaluator<'v, '_, '_>,
) -> anyhow::Result<Value<'v>> {
    let windows_host = windows_host(&input, eval)?;
    let info = CcHeaderInfo::create(input, windows_host, &[]).map_err(anyhow::Error::msg)?;
    Ok(eval.heap().alloc(HeaderInfoValue(info)))
}
pub(crate) fn with_deps<'v>(
    header_info: Value<'v>,
    deps: Value<'v>,
    merged_deps: Value<'v>,
    eval: &Evaluator<'v, '_, '_>,
) -> anyhow::Result<Value<'v>> {
    let info = cc_header_info(header_info)
        .ok_or_else(|| anyhow::anyhow!("header_info requires HeaderInfo"))?;
    let deps = dependencies(deps, "deps", eval.heap())?;
    let merged = dependencies(merged_deps, "merged_deps", eval.heap())?;
    let windows_host = windows_host(info.model(), eval)?;
    let result = info
        .with_deps(deps, &merged, windows_host)
        .map_err(anyhow::Error::msg)?;
    Ok(eval.heap().alloc(HeaderInfoValue(result)))
}

fn windows_host(input: &CcHeaderInfoInput, eval: &Evaluator<'_, '_, '_>) -> anyhow::Result<bool> {
    if input.modular_public_headers.is_empty() && input.modular_private_headers.is_empty() {
        return Ok(false);
    }
    eval.extra
        .and_then(|extra| {
            extra.downcast_ref::<crate::subrule_invocation::AnalysisEvaluationContext>()
        })
        .ok_or_else(|| anyhow::anyhow!("HeaderInfo normalization requires observed analysis Host"))?
        .header_info_windows_host()
}
