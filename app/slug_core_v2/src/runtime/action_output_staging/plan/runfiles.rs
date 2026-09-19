//! Coupled tree selection and exact producer backing expansion.
use slug_build_api_v2::RunfilesSupportActionSpec;

use super::*;

pub(super) struct CoupledTree {
    pub index: usize,
    manifest_index: usize,
    manifest: ActionOutput,
}

fn producer(plan: &PreparedActionPlan<'_>, artifact: &AnalysisArtifact) -> io::Result<usize> {
    let AnalysisArtifact::Derived { owner, output } = artifact else {
        return Err(io::Error::other("runfiles backing must be derived"));
    };
    if let Some(index) = plan.actions().iter().position(|step| {
        step.action().context().owner().artifact_owner() == *owner
            && step.action().outputs().contains(output)
    }) {
        return Ok(index);
    }
    // Equivalent shared FileWrites preserve the original exact artifact on an input.
    plan.actions()
        .iter()
        .flat_map(|step| step.inputs())
        .find(|input| input.artifact() == artifact)
        .and_then(|input| input.producer())
        .ok_or_else(|| io::Error::other("runfiles backing has no exact planned producer"))
}

pub(super) fn expand_groups(
    plan: &PreparedActionPlan<'_>,
    inputs: Option<&PreparedActionChainInputs>,
    groups: &mut SmallMap<usize, SmallSet<ActionOutput>>,
) -> io::Result<Vec<CoupledTree>> {
    let mut supports = Vec::new();
    for (index, _) in groups.iter() {
        if let Some(
            spec @ (RunfilesSupportActionSpec::SymlinkTree { .. }
            | RunfilesSupportActionSpec::RunfilesTree { .. }),
        ) = plan.actions()[*index].action().runfiles_support_spec()
        {
            let inputs = inputs.ok_or_else(|| {
                io::Error::other("runfiles publication requires prepared source inputs")
            })?;
            inputs
                .runfiles_action(*index)
                .map_err(|error| io::Error::other(error.to_string()))?;
            if !supports.contains(&spec.support()) {
                supports.push(spec.support());
            }
        }
    }
    let mut trees = Vec::new();
    for support in supports {
        let tree_index = producer(plan, &support.tree)?;
        let manifest = support
            .manifest
            .as_ref()
            .ok_or_else(|| io::Error::other("runfiles manifest is absent"))?;
        let manifest_index = producer(plan, manifest)?;
        let layout = support
            .layout()
            .map_err(|error| io::Error::other(error.to_string()))?;
        for artifact in layout.constituents().iter().chain([&support.tree]) {
            let AnalysisArtifact::Derived { output, .. } = artifact else {
                continue;
            };
            let index = producer(plan, artifact)?;
            groups
                .entry(index)
                .or_insert_with(SmallSet::new)
                .insert(output.clone());
        }
        let AnalysisArtifact::Derived { output, .. } = manifest else {
            unreachable!()
        };
        // The public manifest is one link in the tree's single physical replacement.
        groups.shift_remove(&manifest_index);
        trees.push(CoupledTree {
            index: tree_index,
            manifest_index,
            manifest: output.clone(),
        });
    }
    Ok(trees)
}

pub(super) fn stage_trees(
    owner: &ConfiguredOutputOwner,
    plan: &PreparedActionPlan<'_>,
    inputs: Option<&PreparedActionChainInputs>,
    trees: Vec<CoupledTree>,
    reserved: &SmallSet<&str>,
    stages: &mut Vec<PlannedActionOutputStaging>,
) -> io::Result<()> {
    let mut seen_backing = SmallSet::new();
    for tree in trees {
        let inputs = inputs.unwrap();
        let links = inputs
            .runfiles_links(tree.index)
            .map_err(|error| io::Error::other(error.to_string()))?;
        let mut backing = Vec::new();
        for index in links.materialized_sources {
            let source = inputs
                .sources()
                .nth(index)
                .ok_or_else(|| io::Error::other("missing runfiles source"))?;
            let permissions = inputs
                .source_permissions(source)
                .map_err(|error| io::Error::other(error.to_string()))?;
            let path = source_backing_path(inputs.workspace_path(), source.digest(), permissions)?;
            if seen_backing.insert(path) {
                backing.push(SourceBacking::new(
                    inputs.workspace_path(),
                    source.digest(),
                    permissions,
                    &mut inputs.open_source(index)?,
                )?);
            }
        }
        let staging = owner.stage_runfiles(
            plan.actions()[tree.index].action(),
            &links.workspace_name,
            &links.entries,
            reserved,
        )?;
        let covered = vec![PublishedPlannedActionOutputs {
            action_index: tree.manifest_index,
            outputs: PublishedActionOutputs {
                root: staging.root.clone(),
                outputs: Arc::from([tree.manifest]),
            },
        }];
        stages.push(PlannedActionOutputStaging {
            action_index: tree.index,
            staging,
            backing,
            covered,
        });
    }
    Ok(())
}
