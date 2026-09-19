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
    let mut pending = groups.keys().copied().collect::<Vec<_>>();
    let mut seen = SmallSet::new();
    let mut supports = Vec::new();
    let mut trees = Vec::new();
    while let Some(index) = pending.pop() {
        if !seen.insert(index) {
            continue;
        }
        let action = plan.actions()[index].action();
        if let Some(spec) = action.symlink_spec() {
            let input = crate::runtime::action_prerequisites::symlink::target(action, spec)
                .map_err(|error| io::Error::other(error.to_string()))?;
            if let AnalysisArtifact::Derived { output, .. } = input {
                let producer = producer(plan, input)?;
                groups
                    .entry(producer)
                    .or_insert_with(SmallSet::new)
                    .insert(output.clone());
                pending.push(producer);
            }
        }
        let Some(
            spec @ (RunfilesSupportActionSpec::SymlinkTree { .. }
            | RunfilesSupportActionSpec::RunfilesTree { .. }),
        ) = action.runfiles_support_spec()
        else {
            continue;
        };
        let inputs = inputs.ok_or_else(|| {
            io::Error::other("runfiles publication requires prepared source inputs")
        })?;
        inputs
            .prepare_runfiles_action(action)
            .map_err(|error| io::Error::other(error.to_string()))?;
        let support = spec.support();
        if supports.contains(&support) {
            continue;
        }
        supports.push(support);
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
            pending.push(index);
        }
        let AnalysisArtifact::Derived { output, .. } = manifest else {
            unreachable!()
        };
        trees.push(CoupledTree {
            index: tree_index,
            manifest_index,
            manifest: output.clone(),
        });
    }
    // Coupled manifests are links inside the tree's one physical replacement.
    // Remove them after reaching the fixed point, including aliases to MANIFEST.
    for tree in &trees {
        groups.shift_remove(&tree.manifest_index);
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
    seen_backing: &mut SmallSet<PathBuf>,
) -> io::Result<()> {
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
