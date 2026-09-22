//! Finite authenticated build-info registration, never callback execution.
use std::sync::Arc;

use allocative::Allocative;
use dupe::Dupe;
use slug_identity_v2::CanonicalLabel;

use crate::ActionOutput;
use crate::ActionOutputKind;
use crate::ActionOutputMetadata;
use crate::AnalysisArtifact;
use crate::WorkspaceStatusArtifact;
use crate::WorkspaceStatusArtifactKind;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Allocative)]
pub enum BuildInfoTransformKind {
    Info,
    Version,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative)]
pub struct BuildInfoCallbackRecipe {
    kind: BuildInfoTransformKind,
    source: CanonicalLabel,
    digest: [u8; 32],
}

impl BuildInfoCallbackRecipe {
    pub const SOURCE_LABEL: &'static str =
        "@@bazel_tools//tools/build_defs/build_info:bazel_cc_build_info.bzl";
    pub const SOURCE_DIGEST: [u8; 32] = [
        0x72, 0xcc, 0x98, 0xdd, 0xc3, 0x9e, 0x4c, 0x7c, 0x66, 0xd2, 0x35, 0x41, 0xc7, 0xb1, 0x8e,
        0x10, 0xb7, 0x0f, 0xde, 0x60, 0x08, 0xe2, 0x88, 0x31, 0x44, 0x55, 0x4b, 0x74, 0x87, 0xe9,
        0x75, 0x14,
    ];
    pub fn new(
        kind: BuildInfoTransformKind,
        source: CanonicalLabel,
        digest: [u8; 32],
    ) -> Result<Self, String> {
        if source != CanonicalLabel::parse(Self::SOURCE_LABEL).expect("pinned canonical source")
            || digest != Self::SOURCE_DIGEST
        {
            return Err("build-info callback requires the pinned C++ source".to_owned());
        }
        Ok(Self {
            kind,
            source,
            digest,
        })
    }
    pub fn kind(&self) -> BuildInfoTransformKind {
        self.kind
    }
    pub fn source(&self) -> &CanonicalLabel {
        &self.source
    }
    pub fn digest(&self) -> &[u8; 32] {
        &self.digest
    }
}

#[derive(Debug, Eq, PartialEq, Allocative)]
struct BuildInfoTransformData {
    inputs: [AnalysisArtifact; 2],
    output: ActionOutput,
    callback: BuildInfoCallbackRecipe,
}

#[derive(Debug, Clone, Dupe, Eq, PartialEq, Allocative)]
pub struct BuildInfoTransformSpec(Arc<BuildInfoTransformData>);

impl BuildInfoTransformSpec {
    pub fn new(
        status: WorkspaceStatusArtifact,
        template: AnalysisArtifact,
        output: ActionOutput,
        callback: BuildInfoCallbackRecipe,
    ) -> Result<Self, String> {
        let volatile = callback.kind() == BuildInfoTransformKind::Version;
        let expected_kind = if volatile {
            WorkspaceStatusArtifactKind::Volatile
        } else {
            WorkspaceStatusArtifactKind::Stable
        };
        let expected_metadata = if volatile {
            ActionOutputMetadata::Constant
        } else {
            ActionOutputMetadata::Ordinary
        };
        if status.kind() != expected_kind
            || output.kind() != ActionOutputKind::File
            || output.metadata() != expected_metadata
        {
            return Err(
                "build-info transform status/output metadata must match its callback role"
                    .to_owned(),
            );
        }
        super::registry::validate_output(&output).map_err(|error| error.to_string())?;
        Ok(Self(Arc::new(BuildInfoTransformData {
            inputs: [AnalysisArtifact::WorkspaceStatus(status), template],
            output,
            callback,
        })))
    }
    pub fn status(&self) -> &WorkspaceStatusArtifact {
        let AnalysisArtifact::WorkspaceStatus(status) = &self.0.inputs[0] else {
            unreachable!("typed status input")
        };
        status
    }
    pub fn template(&self) -> &AnalysisArtifact {
        &self.0.inputs[1]
    }
    pub fn inputs(&self) -> &[AnalysisArtifact] {
        if self.0.inputs[0] == self.0.inputs[1] {
            &self.0.inputs[..1]
        } else {
            &self.0.inputs
        }
    }
    pub fn output(&self) -> &ActionOutput {
        &self.0.output
    }
    pub fn callback(&self) -> &BuildInfoCallbackRecipe {
        &self.0.callback
    }
    pub fn is_volatile(&self) -> bool {
        self.callback().kind() == BuildInfoTransformKind::Version
    }
    pub fn execute_unconditionally(&self) -> bool {
        self.is_volatile()
    }
    pub fn progress_message(&self) -> &'static str {
        if self.is_volatile() {
            "Translating volatile BuildInfo file"
        } else {
            "Translating stable BuildInfo file"
        }
    }
}

#[cfg(test)]
mod tests;
