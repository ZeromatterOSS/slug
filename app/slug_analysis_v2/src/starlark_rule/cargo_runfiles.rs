//! Lower authenticated evaluator captures without retaining a callable or heap.

use super::*;

pub(super) fn lower<'v>(
    source: EvaluatorVectorSourceGen<Value<'v>>,
    fake_exe: Value<'v>,
    workspace_name: CompactString,
    lowerer: &mut AnalysisValueLowerer<'v>,
) -> anyhow::Result<RetainedVectorSource> {
    let fake_exe = AnalysisArtifactValue::from_starlark(fake_exe)
        .ok_or_else(|| anyhow::anyhow!("Cargo runfiles fake executable must be a File"))?
        .artifact()
        .clone();
    let inputs = match source {
        EvaluatorVectorSourceGen::Sequence(values) => values
            .into_iter()
            .map(|value| {
                let artifact = AnalysisArtifactValue::from_starlark(value)
                    .ok_or_else(|| anyhow::anyhow!("Cargo runfiles Args requires Files"))?;
                Ok(ArtifactInputSource::Direct(artifact.artifact().clone()))
            })
            .collect::<anyhow::Result<Vec<_>>>()?,
        EvaluatorVectorSourceGen::Depset(value) => {
            let lowered = lowerer
                .lower(value, "Cargo runfiles Args inputs")
                .map_err(anyhow::Error::msg)?;
            let AnalysisValueKind::Depset(depset) = lowered.kind() else {
                anyhow::bail!("Cargo runfiles Args requires a File depset");
            };
            vec![ArtifactInputSource::Depset(RetainedArtifactInputs::new(
                depset.clone(),
            )?)]
        }
    };
    Ok(RetainedVectorSource::CargoRunfiles(
        slug_build_api_v2::RetainedCargoRunfilesArgs::new(
            ArtifactInputs::new(inputs),
            fake_exe,
            workspace_name,
        )?,
    ))
}
