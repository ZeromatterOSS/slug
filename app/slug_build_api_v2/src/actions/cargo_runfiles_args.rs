//! Structural projection of the pinned Cargo `_runfiles_map` closure.

use std::fmt;

use allocative::Allocative;
use compact_str::CompactString;

use crate::ActionOutputKind;
use crate::AnalysisArtifact;
use crate::ArtifactInputSource;
use crate::ArtifactInputs;
use crate::analysis_value::PublicationEqState;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CargoRunfilesArgsError(&'static str);

impl fmt::Display for CargoRunfilesArgsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Cargo runfiles Args {}", self.0)
    }
}
impl std::error::Error for CargoRunfilesArgsError {}

#[derive(Debug, Clone, Allocative)]
pub struct RetainedCargoRunfilesArgs {
    inputs: ArtifactInputs,
    fake_exe: AnalysisArtifact,
    workspace_name: CompactString,
}

impl RetainedCargoRunfilesArgs {
    pub fn new(
        inputs: ArtifactInputs,
        fake_exe: AnalysisArtifact,
        workspace_name: CompactString,
    ) -> Result<Self, CargoRunfilesArgsError> {
        if !regular(&fake_exe) {
            return Err(CargoRunfilesArgsError(
                "fake executable must be a regular File",
            ));
        }
        if inputs
            .sources()
            .iter()
            .any(|source| matches!(source, ArtifactInputSource::FilesToRun(_)))
        {
            return Err(CargoRunfilesArgsError("does not admit FilesToRun inputs"));
        }
        let mut all_regular = true;
        inputs
            .visit(|artifact| all_regular &= regular(artifact))
            .map_err(|_| CargoRunfilesArgsError("requires File inputs"))?;
        if !all_regular {
            return Err(CargoRunfilesArgsError("requires regular File inputs"));
        }
        Ok(Self {
            inputs,
            fake_exe,
            workspace_name,
        })
    }

    pub(super) fn render(&self) -> Vec<String> {
        let mut values = Vec::new();
        self.inputs
            .visit(|artifact| {
                if artifact == &self.fake_exe {
                    return;
                }
                let short = artifact.short_path();
                let location = match short.strip_prefix("../") {
                    Some(external) => external.to_owned(),
                    None => format!("{}/{}", self.workspace_name, short),
                };
                values.push(format!("{}={location}", artifact.path()));
            })
            .expect("validated Cargo runfiles inputs");
        values
    }

    pub(super) fn publication_eq_with(&self, other: &Self, state: &mut PublicationEqState) -> bool {
        self.fake_exe == other.fake_exe
            && self.workspace_name == other.workspace_name
            && self.inputs.publication_eq_with(&other.inputs, state)
    }
}

fn regular(artifact: &AnalysisArtifact) -> bool {
    !matches!(artifact, AnalysisArtifact::Derived { output, .. } if output.kind() != ActionOutputKind::File)
}

#[cfg(test)]
mod tests;
