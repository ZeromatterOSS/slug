//! Native immutable build-info registration; callback execution is deferred.
use slug_build_api_v2::BuildInfoTransformKind;
use slug_build_api_v2::BuildInfoTransformSpec;
use slug_loading_v2::build_info_transform::BuildInfoTransformRequest;

use super::*;

impl SynchronousAnalysisActionSink {
    pub(super) fn register_build_info_transform(
        &self,
        request: BuildInfoTransformRequest<'_>,
    ) -> anyhow::Result<AnalysisArtifactValue> {
        let relative = if self.package_path.is_empty() {
            request.output_file_name.to_owned()
        } else {
            format!("{}/{}", self.package_path, request.output_file_name)
        };
        let relative = slug_build_api_v2::repository_relative_output_path(
            self.owner.label().package().repo(),
            &relative,
        );
        let volatile = request.callback.kind() == BuildInfoTransformKind::Version;
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
        let spec = BuildInfoTransformSpec::new(
            status.dupe(),
            request.template,
            output.dupe(),
            request.callback,
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
        self.register(|actions| actions.register_build_info_transform(spec))?;
        declarations.insert(relative, output.dupe());
        Ok(AnalysisArtifactValue::new(AnalysisArtifact::Derived {
            owner: self.owner.clone(),
            output,
        }))
    }
}

/// List elements and bare candidates pass through the *same* genuine-provider
/// decoder. Non-provider structs/Files/None therefore cannot acquire Info status.
pub(super) fn returned_providers(value: Value<'_>) -> Vec<Value<'_>> {
    ListRef::from_value(value).map_or_else(|| vec![value], |list| list.iter().collect())
}

#[cfg(test)]
mod tests;
