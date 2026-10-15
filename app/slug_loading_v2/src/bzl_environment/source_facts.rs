//! Pure derived member of prepared Builtin input; never points back to that owner.
use std::hash::Hash;
use std::hash::Hasher;

use super::references::EnvironmentReferencePlan;
use super::*;
use crate::bzl_module::semantic_evaluation::budget::peak_context::FirstLedger;
use crate::bzl_module::semantic_evaluation::budget::peak_context::PeakContext;
use crate::bzl_module::semantic_evaluation::budget::peak_context::PeakOwner;
use crate::bzl_module::semantic_evaluation::budget::peak_context::PeakPhase;
use crate::bzl_module::semantic_evaluation::facts;
#[derive(Debug, Allocative)]
pub(crate) struct EnvironmentSourceFacts {
    rows: Arc<[Arc<LocalFacts>]>,
    plan: Arc<EnvironmentReferencePlan>,
    budget: Budget,
    local: Budget,
    physical: Budget,
    hash: u64,
    publication: super::publication::PublicationMetadata,
}
pub(crate) struct PreparedSourcePublication {
    native: super::publication::PreparedPublication,
    logical_storage: slug_build_api_v2::published_value::PublicationShape,
}
impl PreparedSourcePublication {
    pub(crate) fn storage(&self) -> slug_build_api_v2::published_value::PublicationShape {
        self.native.storage()
    }
    pub(crate) fn logical_storage(&self) -> slug_build_api_v2::published_value::PublicationShape {
        self.logical_storage
    }
    #[cfg(test)]
    fn full(&self) -> slug_build_api_v2::published_value::PublicationShape {
        self.native.full()
    }
}
impl EnvironmentSourceFacts {
    /// Called on borrowed parent inputs BEFORE its first row/guard copies.
    pub(crate) fn prepare_publication(
        plan: &EnvironmentReferencePlan,
        row_count: usize,
        mut rows: impl Iterator<Item = Budget>,
        work: &mut Budget,
    ) -> Result<PreparedSourcePublication, String> {
        work.charge_with_context(
            budget_context(PeakPhase::SourceProofMetadata, FirstLedger::Work),
            1,
            std::mem::size_of::<Budget>(),
        )?;
        let mut local = plan.physical_local_budget();
        local.charge_with_context(
            budget_context(PeakPhase::SourceProofMetadata, FirstLedger::Local),
            1,
            std::mem::size_of::<Self>() + 2 * std::mem::size_of::<usize>(),
        )?;
        local.array_with_context::<Arc<LocalFacts>>(
            budget_context(PeakPhase::SourceProofMetadata, FirstLedger::Local),
            row_count,
        )?;
        for _ in 0..row_count {
            work.charge_with_context(
                budget_context(PeakPhase::SourceProofMetadata, FirstLedger::Work),
                2,
                std::mem::size_of::<Budget>(),
            )?;
            let row = rows.next().ok_or("source publication row count mismatch")?;
            local.charge_with_context(
                budget_context(PeakPhase::SourceProofMetadata, FirstLedger::Local),
                row.records,
                row.bytes,
            )?;
        }
        work.charge_with_context(
            budget_context(PeakPhase::SourceProofMetadata, FirstLedger::Work),
            2,
            0,
        )?;
        if rows.next().is_some() {
            return Err("source publication row count mismatch".into());
        }
        let children = plan.physical_payloads();
        let native = super::publication::PreparedPublication::prepare(
            super::publication_shape(local),
            children.len(),
            children.iter().map(|p| p.publication_shape()),
            |records, bytes| {
                work.charge_with_context(
                    budget_context(PeakPhase::SourceProofMetadata, FirstLedger::Work),
                    records,
                    bytes,
                )
            },
        )?;
        // The already-validated builtin_catalog_only normal form has exactly
        // one logical catalog, irrespective of equal foreign physical guards.
        work.charge_with_context(
            budget_context(PeakPhase::SourceProofMetadata, FirstLedger::Work),
            1,
            std::mem::size_of::<slug_build_api_v2::published_value::PublicationShape>(),
        )?;
        let logical_storage = super::publication::declaration_storage(1)?;
        Ok(PreparedSourcePublication {
            native,
            logical_storage,
        })
    }
    pub(crate) fn prepare(
        rows: &Arc<[Arc<LocalFacts>]>,
        environment: &PreparedBzlEnvironment,
        plan: &Arc<EnvironmentReferencePlan>,
        admitted: Budget,
        work: Budget,
        publication: PreparedSourcePublication,
    ) -> Result<Arc<Self>, String> {
        if environment.mode() != BzlEnvironmentMode::Builtin {
            return Err("source proof requires actual Builtin inputs".into());
        }
        // The parent's borrowed reference plan already fully compared every
        // foreign catalog and validates the exact row/binding environment order.
        // Hashing plus this closed-tag inspection were prepaid before ANY parent
        // key/row guard clone. No second constructor budget is opened here.
        crate::bzl_module::semantic_evaluation::budget::admit_peak(
            PeakContext::new(
                PeakOwner::SourceProof,
                PeakPhase::SourceProofPrecopy,
                FirstLedger::AdmittedPhysical,
            ),
            admitted,
            work,
            0,
        )?;
        if !plan.builtin_catalog_only() {
            return Err("builtin source proof has inconsistent catalog/environment facts".into());
        }
        let mut budget = plan.budget();
        budget.charge_with_context(
            budget_context(PeakPhase::SourceProofPrecopy, FirstLedger::Logical),
            1,
            std::mem::size_of::<Self>() + 2 * std::mem::size_of::<usize>(),
        )?;
        budget.array_with_context::<Arc<LocalFacts>>(
            budget_context(PeakPhase::SourceProofPrecopy, FirstLedger::Logical),
            rows.len(),
        )?;
        for row in &**rows {
            budget.charge_with_context(
                budget_context(PeakPhase::SourceProofPrecopy, FirstLedger::Logical),
                row.budget.records,
                row.budget.bytes,
            )?;
        }
        let mut local = plan.physical_local_budget();
        local.charge_with_context(
            budget_context(PeakPhase::SourceProofPrecopy, FirstLedger::Local),
            1,
            std::mem::size_of::<Self>() + 2 * std::mem::size_of::<usize>(),
        )?;
        local.array_with_context::<Arc<LocalFacts>>(
            budget_context(PeakPhase::SourceProofPrecopy, FirstLedger::Local),
            rows.len(),
        )?;
        for row in &**rows {
            local.charge_with_context(
                budget_context(PeakPhase::SourceProofPrecopy, FirstLedger::Local),
                row.budget.records,
                row.budget.bytes,
            )?;
        }
        let mut physical = plan.physical_budget();
        physical.charge_with_context(
            budget_context(PeakPhase::SourceProofPrecopy, FirstLedger::Physical),
            1,
            std::mem::size_of::<Self>() + 2 * std::mem::size_of::<usize>(),
        )?;
        physical.array_with_context::<Arc<LocalFacts>>(
            budget_context(PeakPhase::SourceProofPrecopy, FirstLedger::Physical),
            rows.len(),
        )?;
        for row in &**rows {
            physical.charge_with_context(
                budget_context(PeakPhase::SourceProofPrecopy, FirstLedger::Physical),
                row.budget.records,
                row.budget.bytes,
            )?;
        }
        let storage = publication.storage();
        let logical_storage = publication.logical_storage();
        budget.charge_with_context(
            budget_context(PeakPhase::SourceProofPrecopy, FirstLedger::Logical),
            logical_storage.visits,
            logical_storage.bytes,
        )?;
        local.charge_with_context(
            budget_context(PeakPhase::SourceProofPrecopy, FirstLedger::Local),
            storage.visits,
            storage.bytes,
        )?;
        physical.charge_with_context(
            budget_context(PeakPhase::SourceProofPrecopy, FirstLedger::Physical),
            storage.visits,
            storage.bytes,
        )?;
        // Full borrowed hashing cost and payload/Arc storage admission precede
        // the new owning proof. Hash is an accelerator; equality uses all facts.
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        plan.hash(&mut hash);
        rows.len().hash(&mut hash);
        for row in &**rows {
            facts::hash_local_fields(row, &mut hash);
        }
        Ok(Arc::new(Self {
            rows: rows.clone(),
            plan: plan.clone(),
            budget,
            local,
            physical,
            hash: hash.finish(),
            publication: publication.native.finish(
                plan.physical_payloads()
                    .iter()
                    .map(|p| p.shared())
                    .collect(),
            )?,
        }))
    }
    pub(crate) fn physical_budget(&self) -> Budget {
        self.physical
    }
    pub(crate) fn nested_payloads(&self) -> &[super::references::Payload] {
        self.plan.physical_payloads()
    }
    pub(crate) fn budget(&self) -> Budget {
        self.budget
    }
}
impl PartialEq for EnvironmentSourceFacts {
    fn eq(&self, o: &Self) -> bool {
        self.plan == o.plan
            && self.rows.len() == o.rows.len()
            && self
                .rows
                .iter()
                .zip(o.rows.iter())
                .all(|(a, b)| facts::local_fields_eq(a, b))
    }
}
impl Eq for EnvironmentSourceFacts {}
impl Hash for EnvironmentSourceFacts {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.hash.hash(h)
    }
}
impl slug_build_api_v2::published_value::ImmutablePublicationInput for EnvironmentSourceFacts {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn retained_shape(&self) -> slug_build_api_v2::published_value::PublicationShape {
        self.publication.full()
    }
    fn intrinsic_shape(&self) -> slug_build_api_v2::published_value::PublicationShape {
        self.publication.intrinsic()
    }
    fn shared_inputs(&self) -> &[slug_build_api_v2::published_value::SharedPublicationInput] {
        self.publication.children()
    }
}

const fn budget_context(phase: PeakPhase, ledger: FirstLedger) -> PeakContext {
    PeakContext::new(PeakOwner::SourceProof, phase, ledger)
}

#[cfg(test)]
#[path = "source_facts/tests.rs"]
mod tests;
