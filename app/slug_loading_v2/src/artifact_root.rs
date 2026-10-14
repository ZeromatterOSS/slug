//! Immutable, publishable view of an owned logical artifact root.
use std::fmt;
use std::hash::Hash;

use allocative::Allocative;
use slug_build_api_v2::AnalysisArtifactRoot;
use starlark::any::ProvidesStaticType;
use starlark::collections::StarlarkHasher;
use starlark::values::Demand;
use starlark::values::FrozenHeap;
use starlark::values::FrozenValue;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::StarlarkValue;
use starlark::values::Value;
use starlark::values::starlark_value;
use starlark::values::structs::StarlarkStructuralValue;

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct ArtifactRootValue(AnalysisArtifactRoot);

impl fmt::Display for ArtifactRootValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match &self.0 {
            AnalysisArtifactRoot::Source(_) => "<source root>",
            AnalysisArtifactRoot::Derived(_) | AnalysisArtifactRoot::WorkspaceStatus(_) => {
                "<derived root>"
            }
        })
    }
}

starlark::starlark_simple_value!(ArtifactRootValue);

impl StarlarkStructuralValue for ArtifactRootValue {
    fn is_structurally_immutable(&self) -> bool {
        true
    }
    fn write_structural_hash(&self, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        self.0.hash(hasher);
        Ok(())
    }
}

#[starlark_value(type = "root")]
impl<'v> StarlarkValue<'v> for ArtifactRootValue {
    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn StarlarkStructuralValue>(self);
    }
    fn write_hash(&self, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        self.0.hash(hasher);
        Ok(())
    }
    fn equals(&self, other: Value<'v>) -> starlark::Result<bool> {
        Ok(Self::from_value(other).is_some_and(|other| self.0 == other.0))
    }
    fn get_attr(&self, name: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        (name == "path").then(|| heap.alloc_str(self.0.path()).to_value())
    }
    fn dir_attr(&self) -> Vec<String> {
        vec!["path".to_owned()]
    }
}

/// Borrow the actual root metadata before projecting an owned root.
pub fn artifact_root_value_ref(value: Value<'_>) -> Option<&AnalysisArtifactRoot> {
    ArtifactRootValue::from_value(value).map(|value| &value.0)
}

pub fn artifact_root_value(value: Value<'_>) -> Option<AnalysisArtifactRoot> {
    ArtifactRootValue::from_value(value).map(|value| value.0.clone())
}

pub fn alloc_artifact_root<'v>(heap: Heap<'v>, root: AnalysisArtifactRoot) -> Value<'v> {
    heap.alloc_simple(ArtifactRootValue(root))
}

pub fn alloc_frozen_artifact_root(heap: &FrozenHeap, root: AnalysisArtifactRoot) -> FrozenValue {
    heap.alloc(ArtifactRootValue(root))
}

#[cfg(test)]
pub(crate) fn test_root() -> slug_build_api_v2::DerivedArtifactRoot {
    use slug_configuration_v2::native::host::AutoCpuToken;
    use slug_configuration_v2::native::host::HostConversionInputs;
    use slug_configuration_v2::native::host::HostPathFlavor;
    let host = HostConversionInputs::new(
        Some(AutoCpuToken::K8),
        Some(HostPathFlavor::Unix),
        None,
        std::sync::Arc::from([]),
        std::sync::Arc::from([]),
    )
    .unwrap();
    slug_build_api_v2::DerivedArtifactRoot::bin(
        slug_configuration_v2::SlugConfiguration::default_target(&host).unwrap(),
    )
    .unwrap()
}

#[cfg(test)]
mod tests {
    use starlark::environment::Globals;
    use starlark::environment::Module;
    use starlark::eval::Evaluator;
    use starlark::syntax::AstModule;
    use starlark::syntax::Dialect;

    use super::*;

    #[test]
    fn roots_are_immutable_hashable_noncallable_values_with_source_identity() {
        let main = slug_identity_v2::CanonicalLabel::parse("@@//:source").unwrap();
        let external = slug_identity_v2::CanonicalLabel::parse("@@repo+//:source").unwrap();
        let heap = FrozenHeap::new();
        let source = alloc_frozen_artifact_root(
            &heap,
            AnalysisArtifactRoot::Source(main.package().repo().clone()),
        );
        let other_source = alloc_frozen_artifact_root(
            &heap,
            AnalysisArtifactRoot::Source(external.package().repo().clone()),
        );
        let derived = alloc_frozen_artifact_root(&heap, AnalysisArtifactRoot::Derived(test_root()));
        assert_eq!(source.to_value().to_repr(), "<source root>");
        assert_eq!(derived.to_value().to_repr(), "<derived root>");
        assert!(!source.to_value().equals(other_source.to_value()).unwrap());
        for statement in [
            "RESULT = root.path()",
            "root.path = 'changed'\nRESULT = None",
        ] {
            let module = Module::new();
            module.set("root", derived.to_value());
            let mut eval = Evaluator::new(&module);
            assert!(
                eval.eval_module(
                    AstModule::parse("negative.bzl", statement.to_owned(), &Dialect::Bazel)
                        .unwrap(),
                    &Globals::standard()
                )
                .is_err()
            );
        }
        let module = Module::new();
        module.set("source", source.to_value());
        module.set("other", other_source.to_value());
        module.set("root", derived.to_value());
        let mut eval = Evaluator::new(&module);
        eval.eval_module(AstModule::parse("roots.bzl", "values = {source: 1, other: 2, root: 3}\nRESULT = [type(root),source.path,other.path,len(values),values[root],dir(root)]".to_owned(), &Dialect::Bazel).unwrap(), &Globals::standard()).unwrap();
        assert_eq!(
            module.get("RESULT").unwrap().to_repr(),
            "[\"root\", \"\", \"\", 3, 3, [\"path\"]]"
        );
    }
}
