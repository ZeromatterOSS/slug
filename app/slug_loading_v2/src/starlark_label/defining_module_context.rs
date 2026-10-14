//! Immutable identity attached to the actual defining .bzl module. It is not a
//! Starlark global or mutable evaluation context; the module owns its lifetime.
use std::fmt;
use std::sync::Arc;

use allocative::Allocative;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::CanonicalRepoName;
use starlark::any::ProvidesStaticType;
use starlark::environment::Module;
use starlark::values::NoSerialize;
use starlark::values::StarlarkValue;
use starlark::values::Value;
use starlark::values::starlark_value;

use crate::BzlLoadManifest;
use crate::BzlModuleIdentity;

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
pub(super) struct DefiningBzlModuleContext {
    label: CanonicalLabel,
    repository_mapping: Arc<[(ApparentRepoName, CanonicalRepoName)]>,
    source_manifest: Option<Arc<BzlLoadManifest>>,
}

impl DefiningBzlModuleContext {
    pub(super) fn label(&self) -> &CanonicalLabel {
        &self.label
    }
    pub(super) fn repository_mapping(&self) -> &[(ApparentRepoName, CanonicalRepoName)] {
        &self.repository_mapping
    }
}

impl fmt::Display for DefiningBzlModuleContext {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("<defining .bzl module context>")
    }
}

starlark::starlark_simple_value!(DefiningBzlModuleContext);

#[starlark_value(type = "defining_bzl_module_context")]
impl<'v> StarlarkValue<'v> for DefiningBzlModuleContext {}

/// One small projection per initialized module. Existing immutable mapping
/// storage is shared; no source pathname or per-Label-call context is copied.
pub(crate) fn attach_defining_module_context(
    module: &Module,
    identity: &BzlModuleIdentity,
) -> anyhow::Result<()> {
    module.set_client_data_no_overwrite(module.heap().alloc_simple(DefiningBzlModuleContext {
        label: identity.label.clone(),
        repository_mapping: identity.repository_mapping.clone(),
        source_manifest: None,
    }))
}

/// Attach the existing observed load manifest once at module initialization.
/// Its source/mapping arrays remain shared; only the root pathname is cloned.
pub(crate) fn attach_defining_module_manifest(
    module: &Module,
    manifest: &BzlLoadManifest,
) -> anyhow::Result<()> {
    module.set_client_data_no_overwrite(module.heap().alloc_simple(DefiningBzlModuleContext {
        label: manifest.root.label.clone(),
        repository_mapping: manifest.root.repository_mapping.clone(),
        source_manifest: Some(Arc::new(manifest.clone())),
    }))
}

/// Borrow the tracked load manifest from an actual original module's client data.
///
/// Pass the value returned by Engine function inspection or defining-module
/// inspection. Identity-only Label contexts and other values return None; this
/// getter does not infer authority from a filename, function name, or evaluator.
pub fn defining_module_source_manifest(
    original_module_client_data: Value<'_>,
) -> Option<&BzlLoadManifest> {
    DefiningBzlModuleContext::from_value(original_module_client_data)?
        .source_manifest
        .as_deref()
}
