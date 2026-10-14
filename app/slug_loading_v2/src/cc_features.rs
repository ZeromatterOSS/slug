//! Native C++ feature values. This does not activate the public rules_cc
//! configure_features wrapper, context projections, or expansion APIs.
use std::fmt;

use allocative::Allocative;
use slug_build_api_v2::cc_features::CcFeatureConfiguration;
use slug_build_api_v2::cc_features::CcToolchainFeatures;
use starlark::any::ProvidesStaticType;
use starlark::collections::StarlarkHasher;
use starlark::environment::Methods;
use starlark::environment::MethodsBuilder;
use starlark::environment::MethodsStatic;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::values::Demand;
use starlark::values::FrozenHeap;
use starlark::values::FrozenValue;
use starlark::values::NoSerialize;
use starlark::values::StarlarkValue;
use starlark::values::Value;
use starlark::values::list::AllocImmutableList;
use starlark::values::list::ListRef;
use starlark::values::starlark_value;
use starlark::values::structs::StarlarkStructuralValue;

use crate::builtin_restriction::check_default_allowlist;
use crate::builtin_restriction::source_identities_for_evaluator;

mod parser;
#[cfg(test)]
mod tests;

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct ToolchainFeaturesValue(CcToolchainFeatures);
starlark::starlark_simple_value!(ToolchainFeaturesValue);

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct FeatureConfigurationValue(CcFeatureConfiguration);
starlark::starlark_simple_value!(FeatureConfigurationValue);

impl fmt::Display for ToolchainFeaturesValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<unknown object com.google.devtools.build.lib.rules.cpp.CcToolchainFeatures>")
    }
}
impl fmt::Display for FeatureConfigurationValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<unknown object com.google.devtools.build.lib.rules.cpp.FeatureConfigurationForStarlark>")
    }
}

fn unhashable(name: &str) -> starlark::Result<()> {
    Err(starlark::Error::new_other(anyhow::anyhow!(
        "unhashable type: {name}"
    )))
}

#[starlark_value(type = "CcToolchainFeatures")]
impl<'v> StarlarkValue<'v> for ToolchainFeaturesValue {
    fn equals(&self, other: Value<'v>) -> starlark::Result<bool> {
        Ok(Self::from_value(other).is_some_and(|other| self.0 == other.0))
    }
    fn write_hash(&self, _: &mut StarlarkHasher) -> starlark::Result<()> {
        unhashable("CcToolchainFeatures")
    }
    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn StarlarkStructuralValue>(self);
    }
    fn get_methods() -> Option<&'static Methods> {
        static METHODS: MethodsStatic = MethodsStatic::new();
        METHODS.methods(toolchain_features_methods)
    }
}

#[starlark_value(type = "FeatureConfiguration")]
impl<'v> StarlarkValue<'v> for FeatureConfigurationValue {
    fn collect_str(&self, collector: &mut String) {
        collector.push_str("<FeatureConfiguration(ENABLED:");
        for (index, name) in self.0.model().enabled_features.iter().enumerate() {
            if index != 0 {
                collector.push_str(", ");
            }
            collector.push_str(name);
        }
        collector.push_str(";REQUESTED:");
        for (index, name) in self.0.model().requested.iter().enumerate() {
            if index != 0 {
                collector.push_str(", ");
            }
            collector.push_str(name);
        }
        collector.push_str(")>");
    }
    fn equals(&self, other: Value<'v>) -> starlark::Result<bool> {
        Ok(Self::from_value(other).is_some_and(|other| self.0 == other.0))
    }
    fn write_hash(&self, _: &mut StarlarkHasher) -> starlark::Result<()> {
        unhashable("FeatureConfiguration")
    }
    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn StarlarkStructuralValue>(self);
    }
    fn get_methods() -> Option<&'static Methods> {
        static METHODS: MethodsStatic = MethodsStatic::new();
        METHODS.methods(feature_configuration_methods)
    }
}

// Bazel native wrappers remain mutable for Starlark hash-barrier purposes,
// even though the Rust payload is owned and immutable, including after freeze.
impl StarlarkStructuralValue for ToolchainFeaturesValue {
    fn is_structurally_immutable(&self) -> bool {
        false
    }
    fn write_structural_hash(&self, _: &mut StarlarkHasher) -> starlark::Result<()> {
        unhashable("CcToolchainFeatures")
    }
}
impl StarlarkStructuralValue for FeatureConfigurationValue {
    fn is_structurally_immutable(&self) -> bool {
        false
    }
    fn write_structural_hash(&self, _: &mut StarlarkHasher) -> starlark::Result<()> {
        unhashable("FeatureConfiguration")
    }
}

fn check_private(eval: &Evaluator<'_, '_, '_>) -> anyhow::Result<()> {
    check_default_allowlist(eval, &source_identities_for_evaluator(eval)?)
}

#[starlark_module]
fn toolchain_features_methods(builder: &mut MethodsBuilder) {
    fn default_features_and_action_configs<'v>(
        this: &ToolchainFeaturesValue,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        check_private(eval)?;
        Ok(eval.heap().alloc(AllocImmutableList(
            this.0
                .default_features_and_action_configs()
                .iter()
                .map(|s| s.as_str()),
        )))
    }

    fn configure_features<'v>(
        this: &ToolchainFeaturesValue,
        requested_features: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<FeatureConfigurationValue> {
        // Java's method parameter is StarlarkList, not the broader Sequence.
        let requested = ListRef::from_value(requested_features)
            .ok_or_else(|| anyhow::anyhow!("requested_features must be a list of strings"))?;
        check_private(eval)?;
        let requested = requested
            .iter()
            .map(|value| {
                value
                    .unpack_str()
                    .map(Into::into)
                    .ok_or_else(|| anyhow::anyhow!("requested_features must be a list of strings"))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        Ok(FeatureConfigurationValue(
            this.0.configure_features(requested)?,
        ))
    }
}

#[starlark_module]
fn feature_configuration_methods(builder: &mut MethodsBuilder) {
    fn is_enabled(
        this: &FeatureConfigurationValue,
        #[starlark(require = pos)] feature: &str,
        eval: &mut Evaluator<'_, '_, '_>,
    ) -> anyhow::Result<bool> {
        check_private(eval)?;
        Ok(this.0.is_enabled(feature))
    }
    fn is_requested(
        this: &FeatureConfigurationValue,
        #[starlark(require = pos)] feature: &str,
        eval: &mut Evaluator<'_, '_, '_>,
    ) -> anyhow::Result<bool> {
        check_private(eval)?;
        Ok(this.0.is_requested(feature))
    }
}

pub(crate) fn construct<'v>(
    info: Value<'v>,
    directory: &str,
    eval: &mut Evaluator<'v, '_, '_>,
) -> anyhow::Result<Value<'v>> {
    Ok(eval
        .heap()
        .alloc_simple(ToolchainFeaturesValue(parser::parse(info, directory)?)))
}

pub fn cc_toolchain_features_value(value: Value<'_>) -> Option<CcToolchainFeatures> {
    Some(ToolchainFeaturesValue::from_value(value)?.0.clone())
}
pub fn cc_feature_configuration_value(value: Value<'_>) -> Option<CcFeatureConfiguration> {
    Some(FeatureConfigurationValue::from_value(value)?.0.clone())
}
pub fn alloc_frozen_cc_toolchain_features(
    heap: &FrozenHeap,
    value: CcToolchainFeatures,
) -> FrozenValue {
    heap.alloc(ToolchainFeaturesValue(value))
}
pub fn alloc_frozen_cc_feature_configuration(
    heap: &FrozenHeap,
    value: CcFeatureConfiguration,
) -> FrozenValue {
    heap.alloc(FeatureConfigurationValue(value))
}

pub fn cc_toolchain_features_ref(value: Value<'_>) -> Option<&CcToolchainFeatures> {
    Some(&ToolchainFeaturesValue::from_value(value)?.0)
}
pub fn cc_feature_configuration_ref(value: Value<'_>) -> Option<&CcFeatureConfiguration> {
    Some(&FeatureConfigurationValue::from_value(value)?.0)
}
