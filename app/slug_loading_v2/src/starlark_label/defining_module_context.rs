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
    identity: BzlModuleIdentity,
    source_manifest: Option<Arc<BzlLoadManifest>>,
    function_owner: Option<slug_build_api_v2::published_value::FunctionEvaluationOwner>,
}

impl DefiningBzlModuleContext {
    pub(super) fn label(&self) -> &CanonicalLabel {
        &self.identity.label
    }
    pub(super) fn repository_mapping(&self) -> &[(ApparentRepoName, CanonicalRepoName)] {
        &self.identity.repository_mapping
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
        identity: identity.clone(),
        source_manifest: None,
        function_owner: None,
    }))
}

/// Attach the existing observed load manifest once at module initialization.
/// Its source/mapping arrays remain shared; only the root pathname is cloned.
pub(crate) fn attach_defining_module_manifest(
    module: &Module,
    manifest: &BzlLoadManifest,
) -> anyhow::Result<()> {
    module.set_client_data_no_overwrite(module.heap().alloc_simple(DefiningBzlModuleContext {
        identity: manifest.root.clone(),
        source_manifest: Some(Arc::new(manifest.clone())),
        function_owner: None,
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

/// Borrow actual lexical source identity; no filename lookup is performed.
pub fn defining_module_identity(data: Value<'_>) -> Option<&BzlModuleIdentity> {
    Some(&DefiningBzlModuleContext::from_value(data)?.identity)
}

pub(crate) fn attach_defining_module_owner(
    module: &Module,
    manifest: &BzlLoadManifest,
    owner: slug_build_api_v2::published_value::FunctionEvaluationOwner,
) -> anyhow::Result<()> {
    module.set_client_data_no_overwrite(module.heap().alloc_simple(DefiningBzlModuleContext {
        identity: manifest.root.clone(),
        source_manifest: Some(Arc::new(manifest.clone())),
        function_owner: Some(owner),
    }))
}
pub(crate) fn defining_module_function_owner(
    data: Value<'_>,
) -> Option<&slug_build_api_v2::published_value::FunctionEvaluationOwner> {
    DefiningBzlModuleContext::from_value(data)?
        .function_owner
        .as_ref()
}

/// Verify actual original-module provenance against the current admitted source
/// closure. Filenames are diagnostic metadata only and never select authority.
pub(crate) fn validate_defining_module_source<'v>(
    data: Value<'v>,
    admitted: &[(compact_str::CompactString, crate::BzlModuleSourceProvenance)],
) -> anyhow::Result<&'v BzlModuleIdentity> {
    let manifest = defining_module_source_manifest(data).ok_or_else(|| {
        anyhow::anyhow!("source-sensitive builtin requires actual defining .bzl manifest")
    })?;
    let root = manifest
        .reachable
        .iter()
        .find(|p| p.identity == manifest.root)
        .ok_or_else(|| anyhow::anyhow!("actual defining .bzl manifest has no root provenance"))?;
    let mut found = false;
    for (_, candidate) in admitted {
        if candidate.identity.label == root.identity.label {
            if candidate != root {
                anyhow::bail!(
                    "conflicting original module provenance for {}",
                    root.identity.label
                );
            }
            found = true;
        }
    }
    if !found {
        anyhow::bail!(
            "original module is absent from the current admitted source closure: {}",
            root.identity.label
        );
    }
    Ok(&manifest.root)
}
