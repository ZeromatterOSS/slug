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
    source_context: Option<crate::BzlModuleSourceContext>,
    function_owner: Option<slug_build_api_v2::published_value::FunctionEvaluationOwner>,
}

/// Closed description of the client data this original module will retain.
/// The same borrowed selection supplies the owner certificate and attachment.
#[derive(Clone, Copy)]
pub(crate) enum PreparedDefiningModuleMetadata<'a> {
    RawManifest {
        manifest: &'a BzlLoadManifest,
        // The legacy producer still constructs and holds this context while
        // building its owner. Its joint peak is work-only, never attached data.
        transient_context: &'a crate::BzlModuleSourceContext,
    },
    EffectiveContext(&'a crate::BzlModuleSourceContext),
}
impl<'a> PreparedDefiningModuleMetadata<'a> {
    pub(crate) fn raw_manifest(self) -> &'a BzlLoadManifest {
        match self {
            Self::RawManifest { manifest, .. } => manifest,
            Self::EffectiveContext(context) => context.raw_load_manifest(),
        }
    }
    pub(crate) fn transient_context(self) -> Option<&'a crate::BzlModuleSourceContext> {
        match self {
            Self::RawManifest {
                transient_context, ..
            } => Some(transient_context),
            Self::EffectiveContext(_) => None,
        }
    }
    pub(crate) fn header_bytes() -> usize {
        // Includes ALL inline Option layouts even when their value is None.
        // Existing conservative heap/control credit is retained independently.
        std::mem::size_of::<DefiningBzlModuleContext>() + 8 * std::mem::size_of::<usize>()
    }
    pub(crate) fn attach(
        self,
        module: &Module,
        owner: slug_build_api_v2::published_value::FunctionEvaluationOwner,
    ) -> anyhow::Result<()> {
        match self {
            Self::RawManifest { manifest, .. } => {
                attach_defining_module_owner(module, manifest, owner)
            }
            Self::EffectiveContext(context) => {
                attach_defining_module_environment(module, context, owner)
            }
        }
    }
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
        source_context: None,
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
        source_context: None,
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
    let data = DefiningBzlModuleContext::from_value(original_module_client_data)?;
    data.source_context
        .as_ref()
        .map(|context| context.raw_load_manifest())
        .or(data.source_manifest.as_deref())
}

/// Borrow distinct raw-load and effective-environment provenance from the
/// actual module owner; filenames never manufacture this source authority.
pub fn defining_module_source_context(data: Value<'_>) -> Option<&crate::BzlModuleSourceContext> {
    DefiningBzlModuleContext::from_value(data)?
        .source_context
        .as_ref()
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
        source_context: None,
        function_owner: Some(owner),
    }))
}

pub(crate) fn attach_defining_module_environment(
    module: &Module,
    context: &crate::BzlModuleSourceContext,
    owner: slug_build_api_v2::published_value::FunctionEvaluationOwner,
) -> anyhow::Result<()> {
    module.set_client_data_no_overwrite(module.heap().alloc_simple(DefiningBzlModuleContext {
        identity: context.raw_load_manifest().root.clone(),
        source_manifest: None,
        source_context: Some(context.clone()),
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
