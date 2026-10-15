//! Private builtin source route; completed semantic modules own actual exports.
use dice::DiceComputations;

use super::*;
use crate::bzl_module::BzlModuleContext;
use crate::bzl_module::FrozenBzlModule;
use crate::bzl_module::semantic_evaluation;
use crate::bzl_module::semantic_evaluation::budget::peak_context::FirstLedger;
use crate::bzl_module::semantic_evaluation::budget::peak_context::PeakContext;
use crate::bzl_module::semantic_evaluation::budget::peak_context::PeakOwner;
use crate::bzl_module::semantic_evaluation::budget::peak_context::PeakPhase;

pub(super) fn resolve(
    catalog: &catalog::BuiltinSourceCatalog,
    raw: &str,
    work: &mut Budget,
) -> Result<usize, String> {
    let path = raw
        .strip_prefix(':')
        .ok_or("builtin loads must stay in the private root package")?;
    if path.split('/').any(|p| matches!(p, "" | "." | "..")) || path.contains(['\\', '\0', ':']) {
        return Err("invalid builtin load path".into());
    }
    for (index, entry) in catalog.entries.iter().enumerate() {
        work.charge_with_context(
            budget_context(PeakPhase::BuiltinGraph, FirstLedger::Work),
            1,
            0,
        )?;
        if entry.path == path {
            return Ok(index);
        }
    }
    Err("builtin load is outside its source catalog".into())
}
fn order(
    catalog: &catalog::BuiltinSourceCatalog,
    root: usize,
    work: &mut Budget,
) -> Result<Vec<usize>, String> {
    let count = catalog.entries.len();
    work.array_with_context::<usize>(
        budget_context(PeakPhase::BuiltinGraph, FirstLedger::Work),
        count.saturating_mul(4),
    )?;
    work.array_with_context::<u8>(
        budget_context(PeakPhase::BuiltinGraph, FirstLedger::Work),
        count,
    )?;
    let mut colors = vec![0u8; count];
    let mut frames = Vec::with_capacity(count);
    let mut out = Vec::with_capacity(count);
    frames.push((root, 0usize));
    colors[root] = 1;
    while let Some((node, next)) = frames.last_mut() {
        work.charge_with_context(
            budget_context(PeakPhase::BuiltinGraph, FirstLedger::Work),
            1,
            0,
        )?;
        if *next == catalog.entries[*node].loads.len() {
            colors[*node] = 2;
            out.push(*node);
            frames.pop();
            continue;
        }
        let child = resolve(catalog, &catalog.entries[*node].loads[*next], work)?;
        *next += 1;
        match colors[child] {
            1 => return Err("cycle in private builtin source loads".into()),
            2 => {}
            _ => {
                work.depth_with_context(
                    budget_context(PeakPhase::BuiltinGraph, FirstLedger::Work),
                    frames.len(),
                )?;
                colors[child] = 1;
                frames.push((child, 0));
            }
        }
    }
    Ok(out)
}
pub(crate) async fn prepare_build_environment(
    ctx: &mut DiceComputations<'_>,
    key: &BuiltinSourceCatalogKey,
    mapping: &BuiltinRepositoryMapping,
) -> Result<PreparedBzlEnvironment, String> {
    prepare_source_environment(ctx, key, mapping, BzlEnvironmentMode::Build).await
}
/// Ordinary source modes share actual builtin producers, while retaining their
/// distinct native globals and complete semantic environment identity.
pub(crate) async fn prepare_source_environment(
    ctx: &mut DiceComputations<'_>,
    key: &BuiltinSourceCatalogKey,
    mapping: &BuiltinRepositoryMapping,
    mode: BzlEnvironmentMode,
) -> Result<PreparedBzlEnvironment, String> {
    if !matches!(mode, BzlEnvironmentMode::Build | BzlEnvironmentMode::Bzlmod) {
        return Err("source export preparation requires an ordinary mode".into());
    }
    let value = ctx.compute(key).await.map_err(|e| e.to_string())?;
    let catalog = value.as_ref().as_ref().map_err(|e| e.to_string())?;
    let seed = PreparedBzlEnvironment::seed(BzlEnvironmentMode::Builtin, Some(catalog))?;
    let mut work = seed.inputs.budget;
    let root = resolve(&catalog, ":common/objc/apple_common.bzl", &mut work)?;
    let order = order(&catalog, root, &mut work)?;
    work.array_with_context::<Option<FrozenBzlModule>>(
        budget_context(PeakPhase::BuiltinGraph, FirstLedger::Work),
        catalog.entries.len().saturating_mul(2),
    )?;
    let mut completed = vec![None; catalog.entries.len()];
    for index in order {
        let entry = &catalog.entries[index];
        work.charge_with_context(
            budget_context(PeakPhase::BuiltinGraph, FirstLedger::Work),
            1,
            std::mem::size_of::<BzlModuleIdentity>()
                + entry.path.len().saturating_mul(3)
                + catalog.tag.len(),
        )?;
        work.array_with_context::<(&str, &FrozenBzlModule)>(
            budget_context(PeakPhase::BuiltinGraph, FirstLedger::Work),
            entry.loads.len(),
        )?;
        work.array_with_context::<(ApparentRepoName, CanonicalRepoName)>(
            budget_context(PeakPhase::BuiltinGraph, FirstLedger::Work),
            mapping.entries.len(),
        )?;
        for (apparent, canonical) in &*mapping.entries {
            work.charge_with_context(
                budget_context(PeakPhase::BuiltinGraph, FirstLedger::Work),
                1,
                apparent
                    .as_str()
                    .len()
                    .saturating_add(canonical.as_str().len()),
            )?;
        }
        let identity = BzlModuleIdentity {
            label: CanonicalLabel::parse(&format!("@@_builtins//:{}", entry.path))
                .map_err(|e| e.to_string())?,
            source: slug_build_api_v2::ModuleSourceOrigin::builtin(&catalog.tag, &entry.path)?,
            repository_mapping: mapping.entries.clone(),
        };
        let mut children = Vec::with_capacity(entry.loads.len());
        for raw in &*entry.loads {
            let child = resolve(&catalog, raw, &mut work)?;
            children.push((
                raw.as_str(),
                completed[child]
                    .as_ref()
                    .ok_or("builtin dependency is incomplete")?,
            ));
        }
        let key = semantic_evaluation::prepare(
            &identity,
            &entry.path,
            &entry.source,
            BzlModuleContext::Bzlmod,
            &seed,
            children.iter().copied(),
        )?;
        let result = ctx.compute(&key).await.map_err(|e| e.to_string())?;
        let borrowed = result
            .result()
            .map_err(|e| format!("builtin module evaluation: {e:?}"))?;
        borrowed.forecast_clone(
            &mut work,
            budget_context(PeakPhase::BuiltinGraph, FirstLedger::Work),
        )?;
        let module = borrowed.clone();
        completed[index] = Some(module);
    }
    let module = completed[root]
        .take()
        .ok_or("missing builtin export module")?;
    select(
        &seed,
        &module,
        "apple_common",
        "apple_common_bazel",
        mode,
        work,
    )
}
/// Only this private extraction path pairs selected facts with actual exports.
/// Production supplies the fixed reviewed export; tests exercise another real
/// export through the identical complete admission/lookup path.
pub(super) fn select(
    seed: &PreparedBzlEnvironment,
    module: &FrozenBzlModule,
    global: &str,
    export: &str,
    mode: BzlEnvironmentMode,
    mut work: Budget,
) -> Result<PreparedBzlEnvironment, String> {
    if !matches!(mode, BzlEnvironmentMode::Build | BzlEnvironmentMode::Bzlmod) {
        return Err("source export selection requires an ordinary mode".into());
    }
    let catalog = seed
        .inputs
        .catalog
        .as_ref()
        .ok_or("export seed lacks catalog")?;
    let inputs = module
        .semantic_input_facts()
        .ok_or("builtin export lacks semantic inputs")?;
    let proof = inputs
        .source_proof
        .as_ref()
        .ok_or("successful builtin module lacks paired source proof")?;
    let mut budget = seed.inputs.local_budget();
    budget.array_with_context::<Arc<LocalFacts>>(
        export_context(PeakPhase::ExportFacade, FirstLedger::Logical),
        inputs.rows.len().saturating_mul(2),
    )?;
    budget.array_with_context::<ExportSelection>(
        export_context(PeakPhase::ExportFacade, FirstLedger::Logical),
        2,
    )?;
    budget.array_with_context::<OriginalExport>(
        export_context(PeakPhase::ExportFacade, FirstLedger::Logical),
        2,
    )?;
    // Both retained copies, including inline or allocated text capacity, are
    // bounded before constructing strings or cloning any actual module guard.
    budget.charge_with_context(
        export_context(PeakPhase::ExportFacade, FirstLedger::Logical),
        1,
        global
            .len()
            .saturating_add(export.len())
            .saturating_mul(2)
            .saturating_add(4 * std::mem::size_of::<CompactString>()),
    )?;
    budget.identity_with_context(
        export_context(PeakPhase::ExportFacade, FirstLedger::Logical),
        &module.manifest.root,
    )?;
    budget.identity_with_context(
        export_context(PeakPhase::ExportFacade, FirstLedger::Logical),
        &module.manifest.root,
    )?;
    module.forecast_clone(
        &mut budget,
        export_context(PeakPhase::ExportFacade, FirstLedger::Logical),
    )?;
    semantic_evaluation::budget::admit_peak(
        PeakContext::new(
            PeakOwner::ExportSelection,
            PeakPhase::ExportFacade,
            FirstLedger::Logical,
        ),
        budget,
        work,
        0,
    )?;
    let mut local = budget;
    // Both independent actual guards are retained, including seed catalog B
    // when the successful module/proof was produced with equal foreign A.
    work.charge_with_context(
        export_context(PeakPhase::ExportGuards, FirstLedger::Work),
        1,
        0,
    )?;
    let publication = super::publication::PreparedPublication::prepare(
        super::publication_shape(local),
        2,
        std::iter::once_with(|| super::publication_shape(catalog.budget)).chain(
            std::iter::once_with(|| {
                use slug_build_api_v2::published_value::ImmutablePublicationInput;
                proof.retained_shape()
            }),
        ),
        |records, bytes| {
            work.charge_with_context(
                export_context(PeakPhase::ExportGuards, FirstLedger::Work),
                records,
                bytes,
            )
        },
    )?;
    let storage = publication.storage();
    local.charge_with_context(
        export_context(PeakPhase::ExportGuards, FirstLedger::Local),
        storage.visits,
        storage.bytes,
    )?;
    budget.charge_with_context(
        export_context(PeakPhase::ExportGuards, FirstLedger::Logical),
        storage.visits,
        storage.bytes,
    )?;
    let full = proof.budget();
    budget.charge_with_context(
        export_context(PeakPhase::ExportGuards, FirstLedger::Logical),
        full.records,
        full.bytes,
    )?;
    let mut physical = local;
    let full_physical = proof.physical_budget();
    physical.charge_with_context(
        export_context(PeakPhase::ExportGuards, FirstLedger::Physical),
        full_physical.records,
        full_physical.bytes,
    )?;
    // New self-contained closure admission is additive even if the proof
    // conservatively already covers this exact catalog allocation.
    work.charge_with_context(
        export_context(PeakPhase::ExportGuards, FirstLedger::Work),
        1,
        std::mem::size_of::<Budget>(),
    )?;
    physical.charge_with_context(
        export_context(PeakPhase::ExportGuards, FirstLedger::Physical),
        catalog.budget.records,
        catalog.budget.bytes,
    )?;
    work.charge_with_context(
        export_context(PeakPhase::ExportGuards, FirstLedger::Work),
        local.records,
        local.bytes,
    )?;
    // Equal descriptors keep deterministic certificate/event credits, while
    // actual foreign proof storage remains live before every export guard copy.
    semantic_evaluation::budget::admit_peak(
        PeakContext::new(
            PeakOwner::ExportSelection,
            PeakPhase::ExportGuards,
            FirstLedger::Physical,
        ),
        physical,
        work,
        0,
    )?;
    let selection = ExportSelection {
        global: global.into(),
        export: export.into(),
        module: module.manifest.root.clone(),
    };
    let value = module
        .module
        .get(selection.export.as_str())
        .map_err(|e| e.to_string())?;
    let rows = inputs.rows.clone();
    let mut certificate = BzlEnvironmentInputs {
        mode,
        native_revision: 1,
        catalog: Some(catalog.clone()),
        selections: Arc::from([selection.clone()]),
        rows,
        budget,
        local,
        proof: Some(proof.clone()),
        hash: 0,
        publication: publication.finish(vec![
            slug_build_api_v2::published_value::SharedPublicationInput::new(catalog.clone()),
            slug_build_api_v2::published_value::SharedPublicationInput::new(proof.clone()),
        ])?,
    };
    certificate.certify_hash();
    Ok(PreparedBzlEnvironment {
        inputs: Arc::new(certificate),
        exports: Arc::from([OriginalExport {
            selection,
            value,
            module: module.clone(),
        }]),
    })
}

const fn export_context(phase: PeakPhase, ledger: FirstLedger) -> PeakContext {
    PeakContext::new(PeakOwner::ExportSelection, phase, ledger)
}
const fn budget_context(phase: PeakPhase, ledger: FirstLedger) -> PeakContext {
    PeakContext::new(PeakOwner::BuiltinPreparation, phase, ledger)
}
