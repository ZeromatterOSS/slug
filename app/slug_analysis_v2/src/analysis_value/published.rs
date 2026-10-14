//! Original frozen values owned by a sealed publication. Execution stays in the
//! destination's ordinary evaluator; this binding has no invocation interface.
pub(crate) mod project;

use std::any::Any;
use std::fmt;
use std::sync::Arc;

use slug_build_api_v2::published_value::PublishedOriginalBinding;
use slug_build_api_v2::published_value::PublishedValueArena;
use slug_build_api_v2::published_value::PublishedValueRef;
use starlark::values::FrozenHeap;
use starlark::values::FrozenValue;
use starlark::values::OwnedFrozenValue;
use starlark::values::ValueLike;
use starlark::values::tuple::TupleRef;

pub(crate) struct StarlarkPublishedOriginals {
    roots: OwnedFrozenValue,
    root_count: usize,
    imports: Arc<[PublishedValueArena]>,
}

impl fmt::Debug for StarlarkPublishedOriginals {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StarlarkPublishedOriginals")
            .field("root_count", &self.root_count)
            .field("imports", &self.imports.len())
            .finish_non_exhaustive()
    }
}

impl StarlarkPublishedOriginals {
    pub(crate) fn new(
        roots: OwnedFrozenValue,
        imports: Arc<[PublishedValueArena]>,
    ) -> Result<Self, String> {
        let tuple = TupleRef::from_value(roots.value())
            .ok_or("publication original roots must be a frozen tuple")?;
        if tuple.len() > slug_build_api_v2::callable_graph::CALLABLE_GRAPH_NODE_LIMIT {
            return Err("publication original root count limit".into());
        }
        if tuple.iter().any(|value| value.unpack_frozen().is_none()) {
            return Err("publication original tuple contains a mutable root".into());
        }
        let root_count = tuple.len();
        Ok(Self {
            roots,
            root_count,
            imports,
        })
    }

    pub(crate) fn owned_root(&self, index: usize) -> Result<OwnedFrozenValue, String> {
        if index >= self.root_count {
            return Err("publication original root slot is out of bounds".into());
        }
        Ok(self.roots.map(|tuple| {
            TupleRef::from_value(tuple.to_value())
                .expect("validated frozen tuple")
                .content()[index]
                .unpack_frozen()
                .expect("validated frozen root")
        }))
    }
}

impl PublishedOriginalBinding for StarlarkPublishedOriginals {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn root_count(&self) -> usize {
        self.root_count
    }
    fn imported_arenas(&self) -> &[PublishedValueArena] {
        &self.imports
    }
}

pub(crate) fn materialize_original(
    original: &PublishedValueRef,
    heap: &FrozenHeap,
) -> Result<FrozenValue, String> {
    let binding = original
        .arena()
        .originals()
        .as_any()
        .downcast_ref::<StarlarkPublishedOriginals>()
        .ok_or("publication has no trusted original Starlark binding")?;
    let slot = original
        .original_slot()
        .ok_or("publication node has no original materializable root")?;
    binding
        .owned_root(slot.index())?
        .owned_value(heap)
        .unpack_frozen()
        .ok_or_else(|| "original publication root is not frozen".into())
}

/// Borrow complete physical views before descriptor traversal allocates rows.
pub(crate) fn configured_target_view<'v>(
    value: starlark::values::Value<'v>,
) -> Option<(
    &'v slug_build_api_v2::ConfiguredTargetValue,
    &'v starlark_map::small_map::SmallMap<slug_build_api_v2::ProviderIdentity, FrozenValue>,
)> {
    let value = super::AnalysisConfiguredTargetValue::from_value(value)?;
    Some((&value.retained, &value.providers))
}

pub(crate) fn runfiles_value(
    value: starlark::values::Value<'_>,
) -> Option<(
    &slug_build_api_v2::RetainedRunfiles,
    &slug_build_api_v2::RunfilesOccurrence,
)> {
    let value = super::StarlarkRunfiles::from_value(value)?;
    Some((&value.retained, &value.occurrence))
}

pub(crate) fn provider_header(
    value: starlark::values::Value<'_>,
) -> Option<(
    slug_build_api_v2::ProviderIdentity,
    usize,
    Option<&slug_build_api_v2::FilesToRunProvider>,
)> {
    if super::EmptyInstrumentedFiles::from_value(value).is_some() {
        return Some((
            slug_build_api_v2::ProviderIdentity::builtin("InstrumentedFilesInfo"),
            2,
            None,
        ));
    }
    if let Some(value) = super::BuiltinProviderView::from_value(value) {
        return Some((value.identity.clone(), value.fields.len(), None));
    }
    if let Some(value) = super::StarlarkFilesToRunProvider::from_value(value) {
        return Some((
            slug_build_api_v2::ProviderIdentity::builtin("FilesToRunProvider"),
            3,
            Some(&value.retained),
        ));
    }
    None
}
pub(crate) fn visit_provider_fields<'v>(
    value: starlark::values::Value<'v>,
    visitor: &mut dyn FnMut(&str, starlark::values::Value<'v>) -> Result<(), String>,
) -> Result<bool, String> {
    if let Some(value) = super::EmptyInstrumentedFiles::from_value(value) {
        value.visit_fields(visitor)?;
        return Ok(true);
    }
    if let Some(value) = super::BuiltinProviderView::from_value(value) {
        for (name, value) in &value.fields {
            visitor(name, value.to_value())?;
        }
        return Ok(true);
    }
    if let Some(value) = super::StarlarkFilesToRunProvider::from_value(value) {
        for (name, value) in [
            ("executable", value.executable),
            ("runfiles_manifest", value.runfiles_manifest),
            ("repo_mapping_manifest", value.repo_mapping_manifest),
        ] {
            visitor(name, value.to_value())?;
        }
        return Ok(true);
    }
    Ok(false)
}

#[cfg(test)]
mod runfiles_tests;
