//! Native template variables snapshot their input and return fresh dict copies.
use slug_build_api_v2::TemplateVariableInfo;
use starlark::values::FrozenHeap;
use starlark::values::dict::AllocDict;

use super::*;

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub(crate) struct TemplateVariableInfoConstructor;
starlark::starlark_simple_value!(TemplateVariableInfoConstructor);

impl fmt::Display for TemplateVariableInfoConstructor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<function TemplateVariableInfo>")
    }
}

#[starlark_value(type = "Provider")]
impl<'v> StarlarkValue<'v> for TemplateVariableInfoConstructor {
    fn equals(&self, other: Value<'v>) -> starlark::Result<bool> {
        Ok(Self::from_value(other).is_some())
    }
    fn write_hash(&self, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        "TemplateVariableInfo constructor".hash(hasher);
        Ok(())
    }
    fn invoke(
        &self,
        _me: Value<'v>,
        args: &Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        construct(args, eval)
    }
}

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct TemplateVariableInfoValue(TemplateVariableInfo);
starlark::starlark_simple_value!(TemplateVariableInfoValue);

impl fmt::Display for TemplateVariableInfoValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("struct(variables = {")?;
        for (index, (key, value)) in self.0.variables().iter().enumerate() {
            if index != 0 {
                f.write_str(", ")?;
            }
            native_quoted_string(f, key)?;
            f.write_str(": ")?;
            native_quoted_string(f, value)?;
        }
        f.write_str("})")
    }
}

// Bazel 9.2 Printer.java239–258. The generic runtime string repr intentionally
// uses different Unicode escaping; do not change it for this native provider.
fn native_quoted_string(f: &mut fmt::Formatter<'_>, value: &str) -> fmt::Result {
    f.write_str("\"")?;
    for character in value.chars() {
        match character {
            '"' => f.write_str("\\\"")?,
            '\\' => f.write_str("\\\\")?,
            '\r' => f.write_str("\\r")?,
            '\n' => f.write_str("\\n")?,
            '\t' => f.write_str("\\t")?,
            c if c < ' ' => write!(f, "\\x{:02x}", c as u32)?,
            c => write!(f, "{c}")?,
        }
    }
    f.write_str("\"")
}

impl StarlarkStructuralValue for TemplateVariableInfoValue {
    fn is_structurally_immutable(&self) -> bool {
        true
    }
    fn write_structural_hash(&self, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        self.0.hash(hasher);
        Ok(())
    }
}

#[starlark_value(type = "TemplateVariableInfo")]
impl<'v> StarlarkValue<'v> for TemplateVariableInfoValue {
    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn StarlarkStructuralValue>(self);
    }
    fn equals(&self, other: Value<'v>) -> starlark::Result<bool> {
        Ok(Self::from_value(other).is_some_and(|other| self.0 == other.0))
    }
    fn write_hash(&self, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        self.write_structural_hash(hasher)
    }
    fn get_attr(&self, name: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        (name == "variables").then(|| {
            // Native Map return conversion makes a fresh MUTABLE Starlark
            // dictionary, even when the immutable provider is frozen.
            heap.alloc(AllocDict(
                self.0
                    .variables()
                    .iter()
                    .map(|(key, value)| (key.as_str(), value.as_str())),
            ))
        })
    }
    fn dir_attr(&self) -> Vec<String> {
        vec!["variables".to_owned()]
    }
    fn has_attr(&self, name: &str, _heap: Heap<'v>) -> bool {
        name == "variables"
    }
}

pub(super) fn construct<'v>(
    args: &Arguments<'v, '_>,
    eval: &mut Evaluator<'v, '_, '_>,
) -> starlark::Result<Value<'v>> {
    let positions = args.positions(eval.heap())?.collect::<Vec<_>>();
    let names = args.names_map()?;
    if positions.len() > 1 || names.keys().any(|name| name.as_str() != "vars") {
        return Err(starlark::Error::new_other(anyhow::anyhow!(
            "TemplateVariableInfo requires one vars dict argument"
        )));
    }
    if !positions.is_empty() && names.contains_key("vars") {
        return Err(starlark::Error::new_other(anyhow::anyhow!(
            "TemplateVariableInfo vars specified twice"
        )));
    }
    let value = positions
        .first()
        .copied()
        .or_else(|| names.get("vars").copied())
        .ok_or_else(|| {
            starlark::Error::new_other(anyhow::anyhow!(
                "TemplateVariableInfo missing required vars argument"
            ))
        })?;
    let dict = DictRef::from_value(value).ok_or_else(|| {
        starlark::Error::new_other(anyhow::anyhow!("TemplateVariableInfo vars must be a dict"))
    })?;
    let mut variables = SmallMap::with_capacity(dict.len());
    for (key, value) in dict.iter() {
        let key = key.unpack_str().ok_or_else(|| {
            starlark::Error::new_other(anyhow::anyhow!(
                "TemplateVariableInfo vars keys must be strings"
            ))
        })?;
        let value = value.unpack_str().ok_or_else(|| {
            starlark::Error::new_other(anyhow::anyhow!(
                "TemplateVariableInfo vars values must be strings"
            ))
        })?;
        variables.insert(CompactString::new(key), CompactString::new(value));
    }
    Ok(eval
        .heap()
        .alloc_simple(TemplateVariableInfoValue(TemplateVariableInfo::new(
            variables,
        ))))
}

pub fn template_variable_info(value: Value<'_>) -> Option<TemplateVariableInfo> {
    TemplateVariableInfoValue::from_value(value).map(|value| value.0.clone())
}

pub fn alloc_frozen_template_variable_info(
    heap: &FrozenHeap,
    info: TemplateVariableInfo,
) -> FrozenValue {
    heap.alloc(TemplateVariableInfoValue(info))
}

#[cfg(test)]
mod tests;
