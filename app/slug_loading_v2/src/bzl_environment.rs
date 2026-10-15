//! Explicit immutable builtin environment inputs paired with original exports.
use std::hash::Hash;
use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use slug_build_api_v2::published_value::FunctionEvaluationOwner;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::CanonicalRepoName;
use starlark::environment::FrozenModule;
use starlark::environment::Globals;
use starlark::environment::GlobalsBuilder;
use starlark::values::OwnedFrozenValue;

use crate::BzlLoadManifest;
use crate::BzlModuleIdentity;
use crate::BzlModuleSourceProvenance;
use crate::bzl_module::semantic_evaluation::budget::Budget;
use crate::bzl_module::semantic_evaluation::budget::peak_context::FirstLedger;
use crate::bzl_module::semantic_evaluation::budget::peak_context::PeakContext;
use crate::bzl_module::semantic_evaluation::budget::peak_context::PeakOwner;
use crate::bzl_module::semantic_evaluation::budget::peak_context::PeakPhase;
use crate::bzl_module::semantic_evaluation::facts::LocalFacts;
mod catalog;
mod catalog_parse;
mod load;
mod mapping;
pub(crate) mod publication;
pub(crate) mod references;
mod source_context;
mod source_facts;
pub(crate) use catalog::BuiltinCatalogSnapshot;
pub(crate) use catalog::BuiltinSourceCatalogKey;
pub(crate) use load::prepare_build_environment;
pub(crate) use load::prepare_source_environment;
pub(crate) use mapping::BuiltinRepositoryMapping;
pub use source_context::BzlEnvironmentSourceBinding;
pub use source_context::BzlModuleSourceContext;
pub(crate) use source_facts::EnvironmentSourceFacts;
pub(crate) use source_facts::PreparedSourcePublication;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Allocative)]
pub(crate) enum BzlEnvironmentMode {
    Build,
    Bzlmod,
    BzlmodBootstrap,
    Builtin,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
struct ExportSelection {
    global: CompactString,
    export: CompactString,
    module: BzlModuleIdentity,
}
#[derive(Debug, Allocative)]
pub(crate) struct BzlEnvironmentInputs {
    mode: BzlEnvironmentMode,
    native_revision: u32,
    catalog: Option<Arc<catalog::BuiltinSourceCatalog>>,
    selections: Arc<[ExportSelection]>,
    rows: Arc<[Arc<LocalFacts>]>,
    budget: Budget,
    local: Budget,
    proof: Option<Arc<EnvironmentSourceFacts>>,
    hash: u64,
    publication: publication::PublicationMetadata,
}
impl PartialEq for BzlEnvironmentInputs {
    fn eq(&self, other: &Self) -> bool {
        self.descriptor_eq(other) && self.payload() == other.payload()
    }
}
impl Eq for BzlEnvironmentInputs {}
impl std::hash::Hash for BzlEnvironmentInputs {
    fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
        self.hash.hash(h);
    }
}
impl BzlEnvironmentInputs {
    pub(crate) fn publication(&self) -> &publication::PublicationMetadata {
        &self.publication
    }
    pub(crate) fn budget(&self) -> Budget {
        self.budget
    }
    pub(crate) fn local_budget(&self) -> Budget {
        self.local
    }
    pub(crate) fn source_proof(&self) -> Option<&Arc<EnvironmentSourceFacts>> {
        self.proof.as_ref()
    }
    pub(crate) fn cached_hash(&self) -> u64 {
        self.hash
    }
    fn descriptor_eq(&self, other: &Self) -> bool {
        self.mode == other.mode
            && self.native_revision == other.native_revision
            && self.selections == other.selections
    }
    fn hash_descriptor<H: std::hash::Hasher>(&self, h: &mut H) {
        self.mode.hash(h);
        self.native_revision.hash(h);
        self.selections.hash(h);
    }
    pub(crate) fn payload(&self) -> Option<references::PayloadRef<'_>> {
        self.proof
            .as_ref()
            .map(|p| references::PayloadRef::Sources(p))
            .or_else(|| {
                self.catalog
                    .as_ref()
                    .map(|c| references::PayloadRef::Catalog(c))
            })
    }
    // Callers prepay the complete local field writer before retaining guards.
    fn certify_hash(&mut self) {
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        self.hash_descriptor(&mut hash);
        self.payload().hash(&mut hash);
        self.hash = std::hash::Hasher::finish(&hash);
    }
}
pub(crate) fn publication_shape(b: Budget) -> slug_build_api_v2::published_value::PublicationShape {
    slug_build_api_v2::published_value::PublicationShape {
        nodes: 0,
        edges: b.records,
        bytes: b.bytes,
        visits: b.records,
        depth: b.depth,
    }
}
#[derive(Debug, Clone, Allocative)]
struct OriginalExport {
    selection: ExportSelection,
    #[allocative(skip)]
    value: OwnedFrozenValue,
    module: crate::bzl_module::FrozenBzlModule,
}
#[derive(Debug, Clone, Allocative)]
pub(crate) struct PreparedBzlEnvironment {
    inputs: Arc<BzlEnvironmentInputs>,
    exports: Arc<[OriginalExport]>,
}
impl PreparedBzlEnvironment {
    pub(crate) fn mode(&self) -> BzlEnvironmentMode {
        self.inputs.mode
    }
    pub(crate) fn inputs(&self) -> &Arc<BzlEnvironmentInputs> {
        &self.inputs
    }
    pub(crate) fn globals(&self, source: &CanonicalLabel) -> Globals {
        let mut builder = GlobalsBuilder::new();
        crate::package::populate_bzl_environment_globals(&mut builder, source, self.mode());
        for export in &*self.exports {
            builder.set(export.selection.global.as_str(), export.value.clone());
        }
        builder.build()
    }
    pub(crate) fn uninjected(mode: BzlEnvironmentMode) -> Result<Self, String> {
        if matches!(mode, BzlEnvironmentMode::Build | BzlEnvironmentMode::Bzlmod) {
            return Err("ordinary BUILD/Bzlmod environment requires actual builtin exports".into());
        }
        Self::seed(mode, None)
    }
    fn seed(
        mode: BzlEnvironmentMode,
        catalog: Option<&Arc<catalog::BuiltinSourceCatalog>>,
    ) -> Result<Self, String> {
        let mut budget = Budget::default();
        budget.charge_with_context(
            budget_context(PeakPhase::SeedHash, FirstLedger::Logical),
            1,
            std::mem::size_of::<BzlEnvironmentInputs>()
                + std::mem::size_of::<Self>()
                + 6 * std::mem::size_of::<usize>(),
        )?;
        let mut local = budget;
        let mut hash_work = Budget::default();
        hash_work.charge_with_context(
            budget_context(PeakPhase::SeedHash, FirstLedger::Work),
            1,
            0,
        )?; // optional child count/read before preflight
        let count = usize::from(catalog.is_some());
        let publication = publication::PreparedPublication::prepare(
            publication_shape(local),
            count,
            catalog.into_iter().map(|c| publication_shape(c.budget)),
            |records, bytes| {
                hash_work.charge_with_context(
                    budget_context(PeakPhase::SeedHash, FirstLedger::Work),
                    records,
                    bytes,
                )
            },
        )?;
        let storage = publication.storage();
        local.charge_with_context(
            budget_context(PeakPhase::SeedHash, FirstLedger::Local),
            storage.visits,
            storage.bytes,
        )?;
        budget.charge_with_context(
            budget_context(PeakPhase::SeedHash, FirstLedger::Logical),
            storage.visits,
            storage.bytes,
        )?;
        if let Some(catalog) = catalog {
            budget.charge_with_context(
                budget_context(PeakPhase::SeedHash, FirstLedger::Logical),
                catalog.budget.records,
                catalog.budget.bytes,
            )?;
        }
        hash_work.charge_with_context(
            budget_context(PeakPhase::SeedHash, FirstLedger::Work),
            local.records,
            local.bytes,
        )?;
        crate::bzl_module::semantic_evaluation::budget::admit_peak(
            PeakContext::new(
                PeakOwner::EnvironmentSeed,
                PeakPhase::SeedHash,
                FirstLedger::Logical,
            ),
            budget,
            hash_work,
            0,
        )?;
        let mut inputs = BzlEnvironmentInputs {
            mode,
            native_revision: 1,
            catalog: catalog.cloned(),
            selections: Arc::from([]),
            rows: Arc::from([]),
            budget,
            local,
            proof: None,
            hash: 0,
            publication: publication.finish(
                catalog
                    .into_iter()
                    .map(|c| {
                        slug_build_api_v2::published_value::SharedPublicationInput::new(c.clone())
                    })
                    .collect(),
            )?,
        };
        inputs.certify_hash();
        Ok(Self {
            inputs: Arc::new(inputs),
            exports: Arc::from([]),
        })
    }
    pub(crate) fn modules(&self) -> impl Iterator<Item = &crate::bzl_module::FrozenBzlModule> {
        self.exports.iter().map(|e| &e.module)
    }
    pub(crate) fn declared_owners(&self) -> Result<Vec<FunctionEvaluationOwner>, String> {
        self.exports
            .iter()
            .map(|e| {
                e.module
                    .module
                    .client_data()
                    .and_then(|d| {
                        crate::starlark_label::defining_module_function_owner(d.to_value())
                    })
                    .cloned()
                    .ok_or_else(|| "builtin export has no actual function owner".into())
            })
            .collect()
    }
}
const fn budget_context(phase: PeakPhase, ledger: FirstLedger) -> PeakContext {
    PeakContext::new(PeakOwner::EnvironmentSeed, phase, ledger)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod apple_provider_tests;

#[cfg(test)]
mod reference_plan_tests;

#[cfg(test)]
pub(crate) mod environment_publication_tests;
