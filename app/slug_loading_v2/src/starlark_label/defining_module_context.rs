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
use starlark::values::starlark_value;

use crate::BzlModuleIdentity;

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
pub(super) struct DefiningBzlModuleContext {
    label: CanonicalLabel,
    repository_mapping: Arc<[(ApparentRepoName, CanonicalRepoName)]>,
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
    }))
}
