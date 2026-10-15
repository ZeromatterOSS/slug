//! The immutable native DottedVersion constructor and value.
use std::cmp::Ordering;
use std::fmt;
use std::mem::size_of;

use allocative::Allocative;
use slug_build_api_v2::DottedVersion;
use slug_build_api_v2::DottedVersionPrepareError;
use slug_build_api_v2::callable_graph::CALLABLE_GRAPH_BYTE_LIMIT;
use slug_build_api_v2::callable_graph::CALLABLE_GRAPH_NODE_LIMIT;
use starlark::any::ProvidesStaticType;
use starlark::collections::StarlarkHasher;
use starlark::environment::GlobalsBuilder;
use starlark::environment::Methods;
use starlark::environment::MethodsBuilder;
use starlark::environment::MethodsStatic;
use starlark::starlark_module;
use starlark::values::FrozenHeap;
use starlark::values::FrozenValue;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::StarlarkValue;
use starlark::values::Value;
use starlark::values::starlark_value;

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct DottedVersionValue(DottedVersion);
starlark::starlark_simple_value!(DottedVersionValue);

impl fmt::Display for DottedVersionValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0.spelling())
    }
}

#[starlark_value(type = "DottedVersion")]
impl<'v> StarlarkValue<'v> for DottedVersionValue {
    fn equals(&self, other: Value<'v>) -> starlark::Result<bool> {
        Ok(Self::from_value(other)
            .is_some_and(|other| self.0.normalized_cmp(&other.0) == Ordering::Equal))
    }

    fn write_hash(&self, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        self.0.hash_normalized(hasher);
        Ok(())
    }

    fn compare(&self, other: Value<'v>) -> starlark::Result<Ordering> {
        let other = Self::from_value(other).ok_or_else(|| {
            starlark::Error::new_other(anyhow::anyhow!(
                "DottedVersion can only be compared with DottedVersion"
            ))
        })?;
        Ok(self.0.normalized_cmp(&other.0))
    }

    fn get_attr(&self, name: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        (name == "to_string").then(|| heap.alloc(self.0.spelling()))
    }

    fn dir_attr(&self) -> Vec<String> {
        vec!["to_string".to_owned()]
    }

    fn get_methods() -> Option<&'static Methods> {
        static METHODS: MethodsStatic = MethodsStatic::new();
        METHODS.methods(dotted_version_methods)
    }
}

#[starlark_module]
fn dotted_version_methods(builder: &mut MethodsBuilder) {
    fn compare_to(
        this: &DottedVersionValue,
        #[starlark(require = pos)] other: &DottedVersionValue,
    ) -> anyhow::Result<i32> {
        Ok(match this.0.normalized_cmp(&other.0) {
            Ordering::Less => -1,
            Ordering::Equal => 0,
            Ordering::Greater => 1,
        })
    }
}

/// Complete borrowed payload for native publication preflight; no model clone.
pub fn dotted_version_ref(value: Value<'_>) -> Option<&DottedVersion> {
    Some(&DottedVersionValue::from_value(value)?.0)
}

pub fn alloc_frozen_dotted_version(heap: &FrozenHeap, value: DottedVersion) -> FrozenValue {
    heap.alloc(DottedVersionValue(value))
}

#[derive(Default)]
struct ConstructionBudget {
    records: usize,
    bytes: usize,
}

impl ConstructionBudget {
    fn records(&mut self, n: usize) -> anyhow::Result<()> {
        self.records = self
            .records
            .checked_add(n)
            .filter(|v| *v <= CALLABLE_GRAPH_NODE_LIMIT)
            .ok_or_else(|| anyhow::anyhow!("dotted version construction node budget exceeded"))?;
        Ok(())
    }

    fn bytes(&mut self, n: usize) -> anyhow::Result<()> {
        self.bytes = self
            .bytes
            .checked_add(n)
            .filter(|v| *v <= CALLABLE_GRAPH_BYTE_LIMIT)
            .ok_or_else(|| anyhow::anyhow!("dotted version construction byte budget exceeded"))?;
        Ok(())
    }
}

fn construct(input: &str, budget: &mut ConstructionBudget) -> anyhow::Result<DottedVersionValue> {
    // Bound the complete borrowed string BEFORE the allocation-free scanner.
    budget.bytes(input.len())?;
    let plan = DottedVersion::prepare(input, || budget.records(1)).map_err(|e| match e {
        DottedVersionPrepareError::Invalid(e) => anyhow::anyhow!(e.to_string()),
        DottedVersionPrepareError::Admission(e) => e,
        DottedVersionPrepareError::StorageOverflow => {
            anyhow::anyhow!("dotted version construction storage forecast overflow")
        }
    })?;
    budget.records(plan.construction_records())?;
    // Model peak includes its owning handle. Conservatively retain another
    // wrapper extent plus four words for the simple VM heap header/alignment.
    let peak = plan
        .storage()
        .construction_peak_bytes()
        .checked_add(size_of::<DottedVersionValue>())
        .and_then(|n| n.checked_add(4 * size_of::<usize>()))
        .ok_or_else(|| anyhow::anyhow!("dotted version construction storage forecast overflow"))?;
    budget.bytes(peak)?;
    Ok(DottedVersionValue(plan.into_owned()))
}

/// Populate the actual native constructor in its caller-owned namespace.
#[starlark_module]
pub(crate) fn dotted_version_globals(builder: &mut GlobalsBuilder) {
    fn dotted_version(
        #[starlark(require = pos)] version: &str,
    ) -> anyhow::Result<DottedVersionValue> {
        construct(version, &mut ConstructionBudget::default())
    }
}

#[cfg(test)]
mod tests;
