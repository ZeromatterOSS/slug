//! Native package-group publication; membership ownership remains in BuildAPI.
use std::fmt;
use std::hash::Hash;
use std::sync::Arc;

use allocative::Allocative;
use slug_build_api_v2::PackageGroupContents;
use slug_build_api_v2::PackageSpecificationInfo;
use slug_build_api_v2::ProviderCollection;
use slug_build_api_v2::ProviderValue;
use slug_identity_v2::CanonicalRepoName;
use slug_identity_v2::OptionLabelContext;
use starlark::any::ProvidesStaticType;
use starlark::collections::StarlarkHasher;
use starlark::environment::Methods;
use starlark::environment::MethodsBuilder;
use starlark::environment::MethodsStatic;
use starlark::starlark_module;
use starlark::values::Demand;
use starlark::values::FrozenHeap;
use starlark::values::FrozenValue;
use starlark::values::NoSerialize;
use starlark::values::StarlarkValue;
use starlark::values::Value;
use starlark::values::starlark_value;
use starlark::values::structs::StarlarkStructuralValue;

use crate::result::ConfiguredNodeResult;

pub(crate) fn providers(
    contents: &Arc<PackageGroupContents>,
    repository: &CanonicalRepoName,
    children: &[Arc<ConfiguredNodeResult>],
) -> Result<ProviderCollection, String> {
    let includes = children
        .iter()
        .map(|child| {
            child
                .providers()
                .package_specification_info()
                .cloned()
                .ok_or_else(|| {
                    format!(
                        "package-group include {} does not refer to a package group (missing PackageSpecificationInfo)",
                        child.key().label()
                    )
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let contents = if repository.is_root() {
        contents.clone()
    } else {
        Arc::new(contents.in_repository_context(repository)?)
    };
    ProviderCollection::from_values(
        vec![ProviderValue::PackageSpecificationInfo(
            PackageSpecificationInfo::new(contents, includes)?,
        )],
        false,
    )
    .map_err(|error| error.to_string())
}

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct PackageSpecificationValue(PackageSpecificationInfo);

impl fmt::Display for PackageSpecificationValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("struct()")
    }
}

starlark::starlark_simple_value!(PackageSpecificationValue);

impl StarlarkStructuralValue for PackageSpecificationValue {
    fn is_structurally_immutable(&self) -> bool {
        true
    }
    fn write_structural_hash(&self, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        // NativeInfo's only field is a callable, excluded from native equality.
        "PackageSpecificationInfo".hash(hasher);
        Ok(())
    }
}

#[starlark_value(type = "PackageSpecificationInfo")]
impl<'v> StarlarkValue<'v> for PackageSpecificationValue {
    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn StarlarkStructuralValue>(self);
    }
    fn equals(&self, other: Value<'v>) -> starlark::Result<bool> {
        Ok(Self::from_value(other).is_some())
    }
    fn write_hash(&self, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        self.write_structural_hash(hasher)
    }
    fn get_methods() -> Option<&'static Methods> {
        static METHODS: MethodsStatic = MethodsStatic::new();
        METHODS.methods(package_specification_methods)
    }
}

#[starlark_module]
fn package_specification_methods(builder: &mut MethodsBuilder) {
    fn contains(this: Value, #[starlark(require = pos)] target: Value) -> anyhow::Result<bool> {
        let this = PackageSpecificationValue::from_value(this)
            .ok_or_else(|| anyhow::anyhow!("invalid PackageSpecificationInfo receiver"))?;
        let label = if let Some(label) = slug_loading_v2::provider::starlark_label(target) {
            label
        } else if let Some(raw) = target.unpack_str() {
            // Unlike Label() this method has no caller-relative or apparent mapping.
            // Guard before the shared option parser, which otherwise rewrites
            // relative strings into main-repository absolute labels.
            if !raw.starts_with("//") && !raw.starts_with('@') {
                anyhow::bail!("contains requires an absolute canonical label, got {raw:?}")
            }
            OptionLabelContext::FirstRoundCanonical
                .parse(raw)
                .map_err(anyhow::Error::msg)?
                .canonical()
                .ok_or_else(|| anyhow::anyhow!("contains requires a visible canonical label"))?
        } else {
            anyhow::bail!(
                "contains expected a string or Label, got {}",
                target.get_type()
            );
        };
        Ok(this.0.contains_package(label.package()))
    }
}

pub(crate) fn alloc(heap: &FrozenHeap, info: PackageSpecificationInfo) -> FrozenValue {
    heap.alloc(PackageSpecificationValue(info))
}

/// Borrow the complete native model before any retained depset handle clone.
pub(crate) fn from_value_ref<'v>(value: Value<'v>) -> Option<&'v PackageSpecificationInfo> {
    Some(&PackageSpecificationValue::from_value(value)?.0)
}

pub(crate) fn from_value(value: Value<'_>) -> Option<PackageSpecificationInfo> {
    PackageSpecificationValue::from_value(value).map(|value| value.0.clone())
}

#[cfg(test)]
mod tests;
