//! Successful closed-model proofs: physical guarded identity, never native Eq.
use std::mem::align_of;
use std::mem::size_of;

use super::*;
use crate::AnalysisValueKind;
use crate::cc_features::CcToolchainFeatureModel;

#[derive(Debug, Clone, Copy)]
struct FeatureModelProof {
    canonical: PublicationShape,
}
#[derive(Debug)]
struct OwnedProof {
    guard: Arc<CcToolchainFeatureModel>,
    proof: FeatureModelProof,
}
#[derive(Debug, Default)]
pub(super) struct FeatureModelAudits {
    committed: fxhash::FxHashMap<usize, OwnedProof>,
    credit: usize,
    header_admitted: bool,
}
struct ModelPlan<'a> {
    model: &'a Arc<CcToolchainFeatureModel>,
    proof: FeatureModelProof,
    pending: bool,
}

impl PublishedValueArenaBuilder {
    pub(super) fn native_root_certificate(
        &mut self,
        value: &AnalysisValue,
    ) -> Result<NativeCertificate, PublishedValueError> {
        match value.kind() {
            AnalysisValueKind::CcToolchainFeatures(_)
            | AnalysisValueKind::CcFeatureConfiguration(_) => {
                self.strict_feature_root_certificate(value)
            }
            _ => self.strict_pure_certificate(|b| native::audit(b, value, 0)),
        }
    }
    fn strict_feature_root_certificate(
        &mut self,
        value: &AnalysisValue,
    ) -> Result<NativeCertificate, PublishedValueError> {
        let prior = self.owner_audit;
        self.owner_audit = false;
        let prior_depth = std::mem::replace(&mut self.maximum_depth, 0);
        let result = (|| {
            self.charge_bytes(size_of::<NativeCertificate>())?;
            let before_header = self.visits;
            // kind() delegates through Published wrappers. The unchanged header
            // and strict origin rejection MUST precede every cache lookup.
            if native::audit_header(self, value, 0)? {
                return Err(PublishedValueError::Invalid("strict feature import"));
            }
            let header_visits = self.visits - before_header;
            let (model, configuration) = match value.kind() {
                AnalysisValueKind::CcToolchainFeatures(f) => (f.model(), None),
                AnalysisValueKind::CcFeatureConfiguration(f) => {
                    (&f.model().model, Some(&**f.model()))
                }
                _ => return Err(PublishedValueError::Invalid("feature root kind")),
            };
            native::feature_outer(self, configuration.is_some())?;
            let plan = self.feature_model_plan(model, 1)?;
            let before_suffix = self.visits;
            if let Some(configuration) = configuration {
                native::feature_configuration_suffix(self, configuration)?;
            }
            let visits = header_visits
                .checked_add(plan.proof.canonical.visits)
                .and_then(|n| n.checked_add(self.visits - before_suffix))
                .filter(|n| *n <= NODE_LIMIT)
                .ok_or(PublishedValueError::Limit("feature canonical visits"))?;
            // No ownership mutation or guard clone until the complete root and
            // configuration suffix have passed, including commit work/growth.
            self.commit_feature_model(plan)?;
            Ok(NativeCertificate {
                visits,
                ordinary_depth: self.maximum_depth,
            })
        })();
        let observed = self.maximum_depth;
        self.maximum_depth = prior_depth.max(observed);
        self.owner_audit = prior;
        if result.is_err() {
            self.failed_budget = true;
        }
        result
    }

    fn feature_model_plan<'a>(
        &mut self,
        model: &'a Arc<CcToolchainFeatureModel>,
        depth: usize,
    ) -> Result<ModelPlan<'a>, PublishedValueError> {
        if !self.feature_models.header_admitted {
            self.producer_scratch()
                .charge_bytes(size_of::<FeatureModelAudits>())?;
            self.feature_models.header_admitted = true;
        }
        // The borrowed plan exists even on a hit, including its flag/padding.
        self.producer_scratch()
            .charge_bytes(size_of::<ModelPlan<'a>>())?;
        self.charge_visit()?;
        let key = Arc::as_ptr(model) as usize;
        let existing = self
            .feature_models
            .committed
            .get(&key)
            .map(|entry| (Arc::ptr_eq(&entry.guard, model), entry.proof));
        if let Some((same, proof)) = existing {
            if !same {
                return Err(PublishedValueError::Invalid("feature allocation guard"));
            }
            self.charge_visit()?;
            let enclosing = depth
                .checked_add(proof.canonical.depth)
                .ok_or(PublishedValueError::Limit("feature depth"))?;
            self.observe_depth(enclosing)?;
            self.charge_storage(crate::DepsetStorageCharge {
                nodes: proof.canonical.nodes,
                edges: proof.canonical.edges,
                bytes: proof.canonical.bytes,
                visits: 0,
            })?;
            return Ok(ModelPlan {
                model,
                proof,
                pending: false,
            });
        }
        let before = self.local_shape();
        let prior = std::mem::replace(&mut self.maximum_depth, 0);
        // Actual enclosing depth is enforced throughout the original full walk.
        let result = native::features(self, model, depth);
        let observed = self.maximum_depth;
        self.maximum_depth = prior.max(observed);
        result?;
        let after = self.local_shape();
        let proof = FeatureModelProof {
            canonical: PublicationShape {
                nodes: after.nodes - before.nodes,
                edges: after.edges - before.edges,
                bytes: after.bytes - before.bytes,
                visits: after.visits - before.visits,
                depth: observed - depth,
            },
        };
        Ok(ModelPlan {
            model,
            proof,
            pending: true,
        })
    }

    fn commit_feature_model(&mut self, plan: ModelPlan<'_>) -> Result<(), PublishedValueError> {
        if !plan.pending {
            return Ok(());
        }
        let count = self
            .feature_models
            .committed
            .len()
            .checked_add(1)
            .filter(|n| *n <= NODE_LIMIT)
            .ok_or(PublishedValueError::Limit("feature memo entries"))?;
        let required = map_peak(count)?;
        let growth = required.saturating_sub(self.feature_models.credit);
        self.producer_scratch().charge_bytes(growth)?;
        // Covers move temporary and guard-copy operation before Arc::clone.
        self.producer_scratch()
            .charge_bytes(size_of::<OwnedProof>())?;
        self.charge_visit()?;
        let previous = self.feature_models.committed.insert(
            Arc::as_ptr(plan.model) as usize,
            OwnedProof {
                guard: Arc::clone(plan.model),
                proof: plan.proof,
            },
        );
        debug_assert!(previous.is_none());
        self.feature_models.credit = required;
        Ok(())
    }
}

fn map_peak(count: usize) -> Result<usize, PublishedValueError> {
    // Pinned hash table minimum/control/alignment plus old/new rehash peak,
    // matching the established source-mapping guarded-proof forecast policy.
    let cell = size_of::<(usize, OwnedProof)>()
        .checked_add(16)
        .and_then(|n| n.checked_add(align_of::<(usize, OwnedProof)>()));
    count
        .max(4)
        .checked_mul(8)
        .and_then(|n| cell.and_then(|cell| n.checked_mul(cell)))
        .and_then(|n| n.checked_add(size_of::<fxhash::FxHashMap<usize, OwnedProof>>()))
        .ok_or(PublishedValueError::Limit("feature memo bytes"))
}

#[cfg(test)]
#[path = "feature_proof/basic_tests.rs"]
pub(super) mod basic_tests;
#[cfg(test)]
#[path = "feature_proof/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "feature_proof/storage_tests.rs"]
mod storage_tests;
