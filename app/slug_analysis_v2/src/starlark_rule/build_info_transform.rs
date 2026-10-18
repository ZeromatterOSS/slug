//! Native immutable build-info registration; callback execution is deferred.
use slug_build_api_v2::BuildInfoTransformHeader;
use slug_build_api_v2::BuildInfoTransformKind;
use slug_loading_v2::build_info_transform::BuildInfoTransformRequest;

use super::*;

impl SynchronousAnalysisActionSink {
    pub(super) fn register_build_info_transform(
        &self,
        request: BuildInfoTransformRequest<'_>,
    ) -> anyhow::Result<AnalysisArtifactValue> {
        super::args_callbacks::pending::resource_storage::build_info_header(request.module)?;
        let relative = if self.package_path.is_empty() {
            request.output_file_name.to_owned()
        } else {
            format!("{}/{}", self.package_path, request.output_file_name)
        };
        let relative = slug_build_api_v2::repository_relative_output_path(
            self.owner.label().package().repo(),
            &relative,
        );
        let volatile = request.kind == BuildInfoTransformKind::Version;
        let output = if volatile {
            ActionOutput::new_constant(self.root.clone(), relative.clone())
        } else {
            ActionOutput::new(self.root.clone(), relative.clone(), ActionOutputKind::File)
        };
        let status = if volatile {
            self.workspace_status.volatile()
        } else {
            self.workspace_status.stable()
        };
        let header = BuildInfoTransformHeader::new(
            request.kind,
            status.dupe(),
            request.template,
            output.dupe(),
        )
        .map_err(anyhow::Error::msg)?;
        // All fallible model/path validation precedes mutation. Registration and
        // declaration commit synchronously under existing locks; no DICE awaits.
        let mut declarations = self
            .declared_outputs
            .lock()
            .map_err(|_| anyhow::anyhow!("ctx.actions declaration lock is poisoned"))?;
        if let Some(previous) = declarations.get(&relative) {
            anyhow::ensure!(
                previous == &output,
                "output '{relative}' has already been declared with a different type or metadata"
            );
        }
        let pending = super::args_callbacks::pending::PendingRegisteredAction::build_info(
            request.module,
            header,
            request.callback,
        )?;
        self.register(|actions| actions.register_pending(pending))?;
        declarations.insert(relative, output.dupe());
        Ok(AnalysisArtifactValue::new(AnalysisArtifact::Derived {
            owner: self.owner.clone(),
            output,
        }))
    }
}

/// A bare None is no explicit providers. List elements and other bare values
/// still pass through the same genuine-provider decoder, so a list containing
/// None cannot acquire Info status.
pub(super) fn returned_providers(value: Value<'_>) -> Vec<Value<'_>> {
    if value.is_none() {
        Vec::new()
    } else {
        ListRef::from_value(value).map_or_else(|| vec![value], |list| list.iter().collect())
    }
}

mod publication;
mod runtime;
pub(super) use publication::DraftBuildInfo;
pub(super) use publication::complete;
pub(super) use publication::prepare;
#[cfg(test)]
pub(super) use runtime::StarlarkBuildInfoRuntime as RuntimeAllocationType;

#[cfg(test)]
mod tests;
