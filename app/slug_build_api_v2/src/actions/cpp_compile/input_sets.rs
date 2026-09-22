//! Pure regular-file projections for the no-scanning C++ compile slice.
use starlark_map::small_set::SmallSet;

use super::CppCompileSpec;
use crate::ActionOutputKind;
use crate::AnalysisArtifact;
use crate::ArtifactInputs;

#[derive(Debug, Eq, PartialEq)]
pub struct CppCompileInputSets {
    mandatory: Vec<AnalysisArtifact>,
    additional: Vec<AnalysisArtifact>,
    spawn: Vec<AnalysisArtifact>,
    allowed_derived: Vec<AnalysisArtifact>,
}
impl CppCompileInputSets {
    pub fn mandatory(&self) -> &[AnalysisArtifact] {
        &self.mandatory
    }
    pub fn additional(&self) -> &[AnalysisArtifact] {
        &self.additional
    }
    pub fn spawn(&self) -> &[AnalysisArtifact] {
        &self.spawn
    }
    /// Native allowed-derived candidates include source leaves; resolution indexes
    /// only the derived leaves, preserving the first artifact at each exec path.
    pub fn allowed_derived(&self) -> &[AnalysisArtifact] {
        &self.allowed_derived
    }
}

pub(super) fn require_regular(artifact: &AnalysisArtifact) -> Result<(), String> {
    if let AnalysisArtifact::Derived { output, .. } = artifact {
        if output.kind() != ActionOutputKind::File {
            return Err(format!(
                "CppCompile dependency reconciliation requires regular Files: {}",
                output.path()
            ));
        }
    }
    Ok(())
}

fn leaves(inputs: &ArtifactInputs) -> Result<SmallSet<AnalysisArtifact>, String> {
    let mut values = SmallSet::new();
    inputs
        .visit(|artifact| {
            values.insert(artifact.clone());
        })
        .map_err(|e| e.to_string())?;
    for artifact in &values {
        require_regular(artifact)?;
    }
    Ok(values)
}

impl CppCompileSpec {
    pub fn input_sets(&self) -> Result<CppCompileInputSets, String> {
        let data = self.data();
        require_regular(&data.source)?;
        let mandatory_spawn = leaves(&data.mandatory_spawn_inputs)?;
        let cache = leaves(&data.cache_key_inputs)?;
        let declared = leaves(&data.declared_headers)?;
        let prunable = leaves(&data.additional_prunable_inputs)?;
        // Builtins already belong to mandatory spawn inputs. Validate their
        // supported kinds without inventing another inclusion/order policy.
        let _ = leaves(&data.builtin_inputs)?;
        let mandatory: SmallSet<_> = mandatory_spawn
            .iter()
            .chain(cache.iter())
            .cloned()
            .collect();
        let additional: SmallSet<_> = declared.iter().chain(prunable.iter()).cloned().collect();
        let spawn = mandatory_spawn
            .iter()
            .chain(additional.iter())
            .cloned()
            .collect::<SmallSet<_>>();
        let allowed_derived = mandatory
            .iter()
            .chain(prunable.iter())
            .chain(declared.iter())
            .chain(std::iter::once(&data.source))
            .cloned()
            .collect::<SmallSet<_>>();
        Ok(CppCompileInputSets {
            mandatory: mandatory.into_iter().collect(),
            additional: additional.into_iter().collect(),
            spawn: spawn.into_iter().collect(),
            allowed_derived: allowed_derived.into_iter().collect(),
        })
    }
}
