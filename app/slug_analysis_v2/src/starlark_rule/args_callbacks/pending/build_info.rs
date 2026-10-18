//! Typed phase-only boxes. Header validation is separate from original publication.
use allocative::Allocative;
use slug_build_api_v2::BuildInfoTransformHeader;
use slug_build_api_v2::BuildInfoTransformSpec;
use slug_build_api_v2::RetainedBuildInfoTransform;
use slug_loading_v2::build_info_transform::BuildInfoCallbackRegistration;
use starlark::environment::Module;

use crate::starlark_rule::build_info_transform::DraftBuildInfo;
#[derive(Debug, Allocative)]
pub(in crate::starlark_rule) struct PendingBuildInfo {
    pub(super) header: BuildInfoTransformHeader,
    pub(super) callback: BuildInfoCallbackRegistration,
}
#[derive(Debug)]
pub(in crate::starlark_rule) struct PreparedBuildInfo {
    header: BuildInfoTransformHeader,
    callback: DraftBuildInfo,
}
impl PendingBuildInfo {
    pub(super) fn new(
        module: &Module,
        header: BuildInfoTransformHeader,
        callback: BuildInfoCallbackRegistration,
    ) -> anyhow::Result<Box<Self>> {
        super::resource_storage::build_info_pending(module)?;
        Ok(Box::new(Self { header, callback }))
    }
    pub(super) fn prepare(
        &self,
        publish: &mut impl FnMut(&BuildInfoCallbackRegistration) -> anyhow::Result<DraftBuildInfo>,
        admit: &mut impl FnMut(usize, usize) -> Result<(), String>,
    ) -> anyhow::Result<Box<PreparedBuildInfo>> {
        super::resource_storage::build_info_prepared(admit).map_err(anyhow::Error::msg)?;
        prepare_header_copy(&self.header, admit)?;
        // Reconstruct owned validated data only AFTER admission; Header has no Clone.
        let header = BuildInfoTransformHeader::new(
            self.header.kind(),
            self.header.status().clone(),
            self.header.template().clone(),
            self.header.output().clone(),
        )
        .map_err(anyhow::Error::msg)?;
        let callback = publish(&self.callback)?;
        Ok(Box::new(PreparedBuildInfo { header, callback }))
    }
}
/// New reconstruction work and Source-only owned string copies use the SAME ledger.
fn prepare_header_copy(
    header: &BuildInfoTransformHeader,
    admit: &mut impl FnMut(usize, usize) -> Result<(), String>,
) -> anyhow::Result<()> {
    use std::mem::size_of;
    admit(4, 8 * size_of::<usize>()).map_err(anyhow::Error::msg)?;
    let bytes = if let slug_build_api_v2::AnalysisArtifact::Source(label) = header.template() {
        label
            .package()
            .repo()
            .as_str()
            .len()
            .checked_add(label.package().package().as_str().len())
            .and_then(|n| n.checked_add(label.target().as_str().len()))
            .and_then(|n| n.checked_add(label.mapping_id().map_or(0, |id| id.as_str().len())))
            .ok_or_else(|| anyhow::anyhow!("build-info template clone length overflow"))?
    } else {
        // Derived owner/output and workspace-status roots clone Arc handles only.
        0
    };
    // Three actual validation scans (backslash, NUL, path segments), plus
    // per-segment control and dot checks, bounded by six units per byte +16.
    // This repeats Header::new's check without changing its policy.
    let path_bytes = header.output().root_relative_path().len();
    let validation = path_bytes
        .checked_mul(6)
        .and_then(|n| n.checked_add(16))
        .ok_or_else(|| anyhow::anyhow!("build-info header validation work overflow"))?;
    admit(validation, bytes).map_err(anyhow::Error::msg)
}
impl PreparedBuildInfo {
    pub(super) fn complete(
        self,
        publish: &mut impl FnMut(DraftBuildInfo) -> anyhow::Result<RetainedBuildInfoTransform>,
    ) -> anyhow::Result<BuildInfoTransformSpec> {
        Ok(BuildInfoTransformSpec::new(
            self.header,
            publish(self.callback)?,
        ))
    }
}
#[cfg(test)]
mod tests;
