//! Borrowed preflight precedes semantic-key storage and lifetime guard copies.
use std::collections::hash_map::DefaultHasher;
use std::hash::Hash;
use std::hash::Hasher;

use starlark_map::small_map::SmallMap;

use super::*;
use crate::bzl_module::semantic_evaluation::budget::peak_context::FirstLedger;
use crate::bzl_module::semantic_evaluation::budget::peak_context::PeakContext;
use crate::bzl_module::semantic_evaluation::budget::peak_context::PeakOwner;
use crate::bzl_module::semantic_evaluation::budget::peak_context::PeakPhase;

#[derive(Debug, Allocative)]
pub(crate) struct ActualChild {
    pub(crate) raw: Arc<str>,
    pub(crate) module: FrozenBzlModule,
}
#[derive(Debug, Allocative)]
pub(crate) struct PreparedSemanticBzlEvaluation {
    pub(crate) rows: Arc<[Arc<facts::LocalFacts>]>,
    pub(crate) children: Box<[ActualChild]>,
    pub(super) budget: budget::Budget,
    pub(super) environment: crate::bzl_environment::PreparedBzlEnvironment,
    pub(crate) source_proof: Option<Arc<crate::bzl_environment::EnvironmentSourceFacts>>,
    references: Arc<crate::bzl_environment::references::EnvironmentReferencePlan>,
}
impl PartialEq for PreparedSemanticBzlEvaluation {
    fn eq(&self, other: &Self) -> bool {
        self.references == other.references
            && self.rows.len() == other.rows.len()
            && self
                .rows
                .iter()
                .zip(other.rows.iter())
                .all(|(a, b)| facts::local_fields_eq(a, b))
    }
}
impl Eq for PreparedSemanticBzlEvaluation {}
impl Hash for PreparedSemanticBzlEvaluation {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.references.hash(h);
        self.rows.len().hash(h);
        for row in &*self.rows {
            facts::hash_local_fields(row, h);
        }
    }
}

impl PreparedSemanticBzlEvaluation {
    pub(crate) fn logical_budget(&self) -> budget::Budget {
        self.budget
    }
}

pub(crate) fn prepare<'a>(
    identity: &BzlModuleIdentity,
    source_name: &str,
    source: &Arc<[u8]>,
    context: BzlModuleContext,
    environment: &crate::bzl_environment::PreparedBzlEnvironment,
    children: impl Iterator<Item = (&'a str, &'a FrozenBzlModule)> + Clone,
) -> Result<SemanticBzlEvaluationKey, String> {
    std::str::from_utf8(source).map_err(|_| "semantic .bzl source is not valid UTF-8")?;
    // Validate borrowed producer certificates before projecting full child
    // stable keys for the deterministic repeated-binding work forecast.
    let mut validation = budget::Budget::default();
    for (_, child) in children.clone() {
        validation.charge_with_context(
            budget_context(PeakPhase::InputValidation, FirstLedger::Validation),
            1,
            0,
        )?;
        if child
            .semantic_inputs
            .as_ref()
            .is_none_or(|inputs| inputs.rows.is_empty())
        {
            return Err("semantic .bzl child lacks actual semantic evaluation ownership".into());
        }
    }
    let local = facts::row_budget(
        identity,
        source_name,
        source,
        environment,
        children.clone().map(|(raw, child)| {
            let inputs = child.semantic_inputs.as_ref().expect("checked child");
            (raw, inputs.rows[0].key.as_ref())
        }),
    )
    .map_err(|e| format!("semantic local row preflight: {e}"))?;
    let comparison = facts::comparison_budget(
        identity,
        source_name,
        source,
        environment,
        children.clone().map(|(raw, child)| {
            (
                raw,
                child.semantic_inputs.as_ref().expect("checked child").rows[0]
                    .key
                    .as_ref(),
            )
        }),
    )?;
    let mut logical = local;
    logical.charge_with_context(
        budget_context(PeakPhase::InputHeader, FirstLedger::Logical),
        1,
        std::mem::size_of::<PreparedSemanticBzlEvaluation>() + 4 * std::mem::size_of::<usize>(),
    )?;
    let mut scratch = validation;
    // No owned identity/source/child guards exist until all borrowed checks pass.
    let mut borrowed = Vec::<&Arc<facts::LocalFacts>>::new();
    let mut index = SmallMap::<u64, Vec<usize>>::new();
    let mut direct_count = 0usize;
    let mut depth = 0usize;
    for (_, child) in children.clone() {
        direct_count = direct_count.checked_add(1).ok_or_else(|| {
            budget::limit_with_context(budget_context(PeakPhase::InputDepth, FirstLedger::Logical))
        })?;
        let inputs = child
            .semantic_inputs
            .as_ref()
            .ok_or("semantic .bzl child lacks actual semantic evaluation ownership")?;
        depth = depth.max(inputs.budget.depth.saturating_add(1));
        logical.depth_with_context(
            budget_context(PeakPhase::InputDepth, FirstLedger::Logical),
            depth,
        )?;
        for row in &*inputs.rows {
            scratch.charge_with_context(
                budget_context(PeakPhase::RowCandidateHash, FirstLedger::Work),
                1,
                0,
            )?;
            scratch.identity_with_context(
                budget_context(PeakPhase::RowCandidateHash, FirstLedger::Work),
                &row.key.identity,
            )?;
            scratch.charge_with_context(
                budget_context(PeakPhase::RowCandidateHash, FirstLedger::Work),
                1,
                row.key.source_name.len(),
            )?;
            let env = budget::Budget {
                records: 1,
                bytes: std::mem::size_of::<u64>(),
                depth: 0,
            };
            scratch
                .charge_with_context(
                    budget_context(PeakPhase::RowCandidateHash, FirstLedger::Work),
                    env.records,
                    env.bytes,
                )
                .map_err(|e| scratch.context("semantic flat candidate environment hash", e))?;
            budget::admit_peak(
                PeakContext::new(
                    PeakOwner::SemanticInputs,
                    PeakPhase::RowCandidateHash,
                    FirstLedger::Logical,
                ),
                logical,
                scratch,
                0,
            )?;
            let mut h = DefaultHasher::new();
            row.key.hash(&mut h);
            let hash = h.finish();
            if row.key.identity == *identity && row.key.context == context {
                return Err("semantic .bzl input closure contains its own module".into());
            }
            let mut duplicate = false;
            if let Some(candidates) = index.get(&hash) {
                for candidate in candidates {
                    let prior = borrowed[*candidate];
                    if Arc::ptr_eq(prior, row) {
                        duplicate = true;
                        break;
                    }
                    scratch
                        .charge_with_context(
                            budget_context(PeakPhase::RowCollision, FirstLedger::Work),
                            prior.comparison.records,
                            prior.comparison.bytes,
                        )
                        .map_err(|e| {
                            scratch.context("semantic flat candidate full collision comparison", e)
                        })?;
                    scratch.charge_with_context(
                        budget_context(PeakPhase::RowCollision, FirstLedger::Work),
                        row.comparison.records,
                        row.comparison.bytes,
                    )?;
                    budget::admit_peak(
                        PeakContext::new(
                            PeakOwner::SemanticInputs,
                            PeakPhase::RowCollision,
                            FirstLedger::Logical,
                        ),
                        logical,
                        scratch,
                        0,
                    )?;
                    if prior.key == row.key {
                        if **prior != **row {
                            return Err(
                                "semantic .bzl input has conflicting complete local facts".into()
                            );
                        }
                        duplicate = true;
                        break;
                    }
                }
            }
            if duplicate {
                continue;
            }
            logical
                .charge_with_context(
                    budget_context(PeakPhase::UniqueRow, FirstLedger::Logical),
                    row.budget.records,
                    row.budget.bytes,
                )
                .map_err(|e| logical.context("semantic flat unique retained row", e))?;
            budget::admit_peak(
                PeakContext::new(
                    PeakOwner::SemanticInputs,
                    PeakPhase::UniqueRow,
                    FirstLedger::Logical,
                ),
                logical,
                scratch,
                0,
            )?;
            // Both candidate list and outer row list are forecast before growth.
            let candidates = index.get(&hash).map_or(0, Vec::len);
            budget::admit_index_growth::<&Arc<facts::LocalFacts>>(
                PeakContext::new(
                    PeakOwner::SemanticInputs,
                    PeakPhase::RowIndexGrowth,
                    FirstLedger::Logical,
                ),
                logical,
                &mut scratch,
                borrowed.len(),
                candidates,
            )?;
            index.entry(hash).or_default().push(borrowed.len());
            borrowed.push(row);
        }
    }
    logical.array_with_context::<Arc<facts::LocalFacts>>(
        budget_context(PeakPhase::RowArray, FirstLedger::Logical),
        borrowed.len().saturating_add(1),
    )?;
    budget::admit_peak(
        PeakContext::new(
            PeakOwner::SemanticInputs,
            PeakPhase::RowArray,
            FirstLedger::Logical,
        ),
        logical,
        scratch,
        0,
    )?;
    logical.array_with_context::<ActualChild>(
        budget_context(PeakPhase::ChildArray, FirstLedger::Logical),
        direct_count,
    )?;
    budget::admit_peak(
        PeakContext::new(
            PeakOwner::SemanticInputs,
            PeakPhase::ChildArray,
            FirstLedger::Logical,
        ),
        logical,
        scratch,
        0,
    )?;
    // Facade clones have deterministic payloads; include them in both key and
    // event admission rather than depending on the caller's actual Arc sharing.
    for (_, child) in children.clone() {
        logical.charge_with_context(
            budget_context(PeakPhase::FacadePath, FirstLedger::Logical),
            1,
            child.path.as_os_str().len(),
        )?;
        budget::admit_peak(
            PeakContext::new(
                PeakOwner::SemanticInputs,
                PeakPhase::FacadePath,
                FirstLedger::Logical,
            ),
            logical,
            scratch,
            0,
        )?;
        logical.array_with_context::<String>(
            budget_context(PeakPhase::FacadeLoadsArray, FirstLedger::Logical),
            child.loads.len(),
        )?;
        budget::admit_peak(
            PeakContext::new(
                PeakOwner::SemanticInputs,
                PeakPhase::FacadeLoadsArray,
                FirstLedger::Logical,
            ),
            logical,
            scratch,
            0,
        )?;
        for load in &child.loads {
            logical.charge_with_context(
                budget_context(PeakPhase::FacadeLoad, FirstLedger::Logical),
                1,
                load.len(),
            )?;
            budget::admit_peak(
                PeakContext::new(
                    PeakOwner::SemanticInputs,
                    PeakPhase::FacadeLoad,
                    FirstLedger::Logical,
                ),
                logical,
                scratch,
                0,
            )?;
        }
        logical.identity_with_context(
            budget_context(PeakPhase::FacadeIdentity, FirstLedger::Logical),
            &child.manifest.root,
        )?;
        budget::admit_peak(
            PeakContext::new(
                PeakOwner::SemanticInputs,
                PeakPhase::FacadeIdentity,
                FirstLedger::Logical,
            ),
            logical,
            scratch,
            0,
        )?;
    }
    if environment.mode() == crate::bzl_environment::BzlEnvironmentMode::Builtin {
        logical.charge_with_context(
            budget_context(PeakPhase::SourceProofPrecopy, FirstLedger::Logical),
            1,
            std::mem::size_of::<crate::bzl_environment::EnvironmentSourceFacts>()
                + 2 * std::mem::size_of::<usize>(),
        )?;
    }
    let base = logical;
    let references =
        crate::bzl_environment::references::EnvironmentReferencePlan::prepare_with_budget(
            std::iter::once(environment.inputs())
                .chain(children.clone().map(|(_, child)| {
                    &child.semantic_inputs.as_ref().expect("checked child").rows[0]
                        .key
                        .environment
                }))
                .chain(borrowed.iter().flat_map(|row| {
                    std::iter::once(&row.key.environment)
                        .chain(row.bindings.iter().map(|b| &b.child.environment))
                })),
            logical,
            scratch,
            PeakOwner::SemanticInputs,
        )?;
    scratch = references.construction_budget();

    let env_budget = references.budget();
    logical.charge_with_context(
        budget_context(PeakPhase::ReferenceFinal, FirstLedger::Logical),
        env_budget.records,
        env_budget.bytes,
    )?;
    let mut physical = base;
    let plan_physical = references.physical_budget();
    physical.charge_with_context(
        budget_context(PeakPhase::InputPhysical, FirstLedger::Physical),
        plan_physical.records,
        plan_physical.bytes,
    )?;
    if environment.mode() == crate::bzl_environment::BzlEnvironmentMode::Builtin {
        // Whole logical facts bound proof hashing/tag validation; prepay it on
        // the SAME constructor work ledger before root/child guard retention.
        scratch.charge_with_context(
            budget_context(PeakPhase::SourceProofHash, FirstLedger::Work),
            logical.records,
            logical.bytes,
        )?;
        if !references.builtin_catalog_only() {
            return Err("builtin input source proof has inconsistent environment facts".into());
        }
    }
    // Native closure metadata is prepared against the SAME borrowed plan and
    // rows before any parent key/source/child guard is copied. Its expanded
    // full DAG shape is not replayed into the semantic/physical budgets.
    let proof_publication =
        if environment.mode() == crate::bzl_environment::BzlEnvironmentMode::Builtin {
            scratch.charge_with_context(
                budget_context(PeakPhase::SourceProofMetadata, FirstLedger::Work),
                1,
                std::mem::size_of::<Option<crate::bzl_environment::PreparedSourcePublication>>(),
            )?;
            let row_count = borrowed.len().checked_add(1).ok_or_else(|| {
                budget::limit_with_context(budget_context(
                    PeakPhase::SourceProofMetadata,
                    FirstLedger::Work,
                ))
            })?;
            let prepared = crate::bzl_environment::EnvironmentSourceFacts::prepare_publication(
                &references,
                row_count,
                std::iter::once(local).chain(borrowed.iter().map(|row| row.budget)),
                &mut scratch,
            )?;
            let storage = prepared.storage();
            let logical_storage = prepared.logical_storage();
            logical.charge_with_context(
                budget_context(PeakPhase::SourceProofMetadata, FirstLedger::Logical),
                logical_storage.visits,
                logical_storage.bytes,
            )?;
            physical.charge_with_context(
                budget_context(PeakPhase::SourceProofMetadata, FirstLedger::Physical),
                storage.visits,
                storage.bytes,
            )?;
            Some(prepared)
        } else {
            None
        };
    budget::admit_peak(
        PeakContext::new(
            PeakOwner::SemanticInputs,
            PeakPhase::InputPhysical,
            FirstLedger::Physical,
        ),
        physical,
        scratch,
        0,
    )?;
    // Retained arrays plus their construction/Vec-to-Arc transfer coexist.
    physical.array_with_context::<facts::Binding>(
        budget_context(PeakPhase::InputTransfer, FirstLedger::Physical),
        direct_count,
    )?;
    physical.array_with_context::<Arc<facts::LocalFacts>>(
        budget_context(PeakPhase::InputTransfer, FirstLedger::Physical),
        borrowed.len().saturating_add(1),
    )?;
    physical.array_with_context::<ActualChild>(
        budget_context(PeakPhase::InputTransfer, FirstLedger::Physical),
        direct_count,
    )?;
    physical.charge_with_context(
        budget_context(PeakPhase::InputTransfer, FirstLedger::Physical),
        0,
        borrowed
            .capacity()
            .saturating_mul(std::mem::size_of::<&Arc<facts::LocalFacts>>()),
    )?;
    // Preserve the full foreign-allocation differential until every output
    // array, transfer and derived proof is admitted. No guard exists before it.
    budget::admit_peak(
        PeakContext::new(
            PeakOwner::SemanticInputs,
            PeakPhase::InputTransfer,
            FirstLedger::Physical,
        ),
        physical,
        scratch,
        0,
    )?;
    let references = Arc::new(references);
    let key = Arc::new(facts::StableKey {
        identity: identity.clone(),
        source_name: source_name.into(),
        context,
        environment: environment.inputs().clone(),
    });
    let bindings = children
        .clone()
        .map(|(raw, child)| facts::Binding {
            raw: Arc::from(raw),
            child: child.semantic_inputs.as_ref().expect("checked child").rows[0]
                .key
                .clone(),
        })
        .collect::<Vec<_>>();
    let root = Arc::new(facts::LocalFacts {
        key,
        source: source.clone(),
        bindings: bindings.into(),
        budget: local,
        comparison,
    });
    let actual = children
        .enumerate()
        .map(|(index, (_raw, child))| ActualChild {
            raw: root.bindings[index].raw.clone(),
            module: child.clone(),
        })
        .collect::<Box<[_]>>();
    let mut rows = Vec::with_capacity(borrowed.len() + 1);
    rows.push(root);
    rows.extend(borrowed.into_iter().cloned());
    let rows: Arc<[Arc<facts::LocalFacts>]> = rows.into();
    let source_proof = if environment.mode() == crate::bzl_environment::BzlEnvironmentMode::Builtin
    {
        Some(crate::bzl_environment::EnvironmentSourceFacts::prepare(
            &rows,
            environment,
            &references,
            physical,
            scratch,
            proof_publication.expect("Builtin borrowed publication preparation"),
        )?)
    } else {
        None
    };
    Ok(SemanticBzlEvaluationKey(Arc::new(
        PreparedSemanticBzlEvaluation {
            rows,
            children: actual,
            budget: logical,
            environment: environment.clone(),
            source_proof,
            references,
        },
    )))
}

const fn budget_context(phase: PeakPhase, ledger: FirstLedger) -> PeakContext {
    PeakContext::new(PeakOwner::SemanticInputs, phase, ledger)
}
