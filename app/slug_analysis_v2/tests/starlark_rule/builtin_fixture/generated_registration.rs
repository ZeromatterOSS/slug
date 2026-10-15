//! Opt-in source-prefix preparation before registration declaration packages.
//! Never retries the configured/resolution consumer or repairs unrelated Needs.
use std::path::Path;

use dice::DiceTransaction;
use slug_bzlmod_v2::RepositoryMaterializationEpochEntry;
use slug_bzlmod_v2::RepositoryMaterializationKind;
use slug_bzlmod_v2::RepositoryMaterializationResultEpoch;
use slug_bzlmod_v2::RepositoryMaterializationResultEpochKey;
use slug_bzlmod_v2::SourcePreparationNeeds;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathObservationEpoch;
use slug_workspace_v2::PathObservationEpochKey;
use slug_workspace_v2::PathObservationNamespace;
use slug_workspace_v2::path_observation_shards;

#[path = "generated_registration/files.rs"]
mod files;
#[path = "generated_registration/request.rs"]
mod request;
#[path = "generated_registration/routes.rs"]
mod routes;
#[path = "generated_registration/stable_inputs.rs"]
mod stable_inputs;
pub(crate) use stable_inputs::FixtureObservations;
pub(crate) use stable_inputs::SavedInputs;
pub(crate) use stable_inputs::capture;

#[derive(Default)]
struct Budget {
    records: usize,
    bytes: usize,
}
impl Budget {
    fn charge(&mut self, records: usize, bytes: usize) -> Result<(), String> {
        let records = self.records.checked_add(records).filter(|n| *n <= 65_536);
        let bytes = self
            .bytes
            .checked_add(bytes)
            .filter(|n| *n <= 64 * 1024 * 1024);
        match (records, bytes) {
            (Some(records), Some(bytes)) => {
                self.records = records;
                self.bytes = bytes;
                Ok(())
            }
            _ => Err("generated registration fixture input budget".into()),
        }
    }
}

// Entire-batch classification precedes every allocation or write by the writer.
fn file_only(need: &SourcePreparationNeeds) -> bool {
    need.scratch_elf_metadata_requests().is_none()
        && need.root_module_bootstrap_request().is_none()
        && need.repository_environment().is_none()
        && need.path_observations().is_none()
        && need.selected_extension_downloads().is_empty()
        && need.repository_executes().is_empty()
        && need.repository_filesystem_operations().is_empty()
        && !need.repository_materializations().is_empty()
        && need.repository_materializations().values().all(|request| {
            matches!(&request.kind, RepositoryMaterializationKind::GeneratedFileEffects(plan)
                if plan.ordered_effects().all(|effect| matches!(effect,
                    slug_bzlmod_v2::GeneratedRepositoryOrderedEffectRef::File(_))))
        })
}

pub(in super::super) async fn prepare(
    mut transaction: DiceTransaction,
    workspace: &Path,
    mut entries: Vec<RepositoryMaterializationEpochEntry>,
) -> Result<DiceTransaction, String> {
    prepare_saved(transaction, workspace, entries, SavedInputs::empty()).await
}

pub(super) async fn prepare_saved(
    mut transaction: DiceTransaction,
    workspace: &Path,
    mut entries: Vec<RepositoryMaterializationEpochEntry>,
    saved: SavedInputs,
) -> Result<DiceTransaction, String> {
    let root = NormalizedAbsolutePath::new(workspace.to_path_buf()).map_err(|e| e.to_string())?;
    let SavedInputs {
        mut prior,
        mut budget,
        floor,
        explicit,
    } = saved;
    for _ in 0..16 {
        let Some(need) = routes::repository_need(&mut transaction, &root, &mut budget).await?
        else {
            return Ok(transaction);
        };
        if !file_only(&need) {
            return Ok(transaction);
        }
        transaction = materialize(
            transaction,
            workspace,
            &root,
            &need,
            &mut entries,
            &mut budget,
            &mut prior,
            floor,
            &explicit,
        )
        .await?;
    }
    Err("generated registration fixture preparation did not converge".into())
}

async fn materialize(
    mut transaction: DiceTransaction,
    workspace: &Path,
    root: &NormalizedAbsolutePath,
    need: &SourcePreparationNeeds,
    entries: &mut Vec<RepositoryMaterializationEpochEntry>,
    budget: &mut Budget,
    prior: &mut Option<stable_inputs::PriorInputs>,
    floor: u64,
    explicit: &[slug_identity_v2::CanonicalRepoName],
) -> Result<DiceTransaction, String> {
    if !file_only(need) {
        return Err("unsupported generated registration fixture Need".into());
    }
    // Existing entries, not a reconstruction of their capabilities, survive.
    for request in need.repository_materializations().values() {
        if request.id.workspace != *root
            || entries.iter().any(|entry| {
                entry.request.id.canonical_repo == request.id.canonical_repo
                    && (explicit.contains(&entry.request.id.canonical_repo) || prior.is_none())
            })
        {
            return Err(
                "generated registration fixture conflicts with existing materialization".into(),
            );
        }
    }
    budget.charge(
        1,
        std::mem::size_of::<RepositoryMaterializationResultEpoch>(),
    )?;
    let previous_epoch = stable_inputs::injected(
        transaction
            .compute(&RepositoryMaterializationResultEpochKey {
                workspace: root.clone(),
            })
            .await,
    )?;
    let retained_floor = previous_epoch
        .as_ref()
        .map(|epoch| stable_inputs::retained_floor(epoch, root, budget))
        .transpose()?
        .unwrap_or(0);
    stable_inputs::admit_inventory(previous_epoch.as_ref(), entries, root, budget)?;
    // Reserve the complete future incarnation comparison/copy peak before any
    // writer IO. Existing writer root and SHA256 identity widths are bounded
    // by workspace+128 and128 respectively (same bound as files::preflight).
    let old_count = previous_epoch
        .as_ref()
        .map(|p| p.borrowed_retained_generated(root, 65_536).map(|v| v.len()))
        .transpose()
        .map_err(|e| format!("{e:?}"))?
        .unwrap_or(0);
    let previous_generated = previous_epoch
        .as_ref()
        .map(|p| {
            p.borrowed_entries(root, 65_536).map(|i| {
                i.filter(|e| {
                    matches!(
                        (&e.request.kind, &e.result),
                        (
                            RepositoryMaterializationKind::GeneratedFileEffects(_),
                            slug_bzlmod_v2::RepositoryMaterializationResult::Success(
                                slug_bzlmod_v2::RepositoryMaterializationSuccess::Immutable { .. }
                            )
                        )
                    )
                })
                .count()
            })
        })
        .transpose()
        .map_err(|e| format!("{e:?}"))?
        .unwrap_or(0);
    let current_generated = entries
        .iter()
        .filter(|e| {
            matches!(
                (&e.request.kind, &e.result),
                (
                    RepositoryMaterializationKind::GeneratedFileEffects(_),
                    slug_bzlmod_v2::RepositoryMaterializationResult::Success(
                        slug_bzlmod_v2::RepositoryMaterializationSuccess::Immutable { .. }
                    )
                )
            )
        })
        .count();
    let count = old_count
        .checked_add(previous_generated)
        .and_then(|n| n.checked_add(current_generated))
        .and_then(|n| n.checked_add(need.repository_materializations().len()))
        .ok_or("fixture pending inventory count")?;
    let multiplier = count
        .checked_add(1)
        .and_then(|n| n.checked_mul(4))
        .ok_or("fixture pending inventory comparisons")?;
    for request in need.repository_materializations().values() {
        let before = (budget.records, budget.bytes);
        stable_inputs::admit_request(request, budget)?;
        let width = workspace
            .as_os_str()
            .len()
            .checked_add(256)
            .and_then(|n| n.checked_mul(16))
            .and_then(|n| {
                n.checked_add(16 * std::mem::size_of::<RepositoryMaterializationEpochEntry>())
            })
            .ok_or("fixture pending output width")?;
        budget.charge(4, width)?;
        let records = budget.records - before.0;
        let bytes = budget.bytes - before.1;
        budget.charge(
            records
                .checked_mul(multiplier)
                .ok_or("fixture pending work")?,
            bytes
                .checked_mul(multiplier)
                .ok_or("fixture pending bytes")?,
        )?;
    }
    files::preflight(workspace, need, budget)?;
    let original = transaction
        .compute(&PathObservationEpochKey)
        .await
        .map_err(|e| e.to_string())?;
    let mut next_instance = original
        .observations()
        .iter()
        .filter_map(|(demand, _)| match demand.namespace() {
            PathObservationNamespace::Materialization(id) => Some(id.value()),
            _ => None,
        })
        .chain(entries.iter().filter_map(|entry| match &entry.result {
            slug_bzlmod_v2::RepositoryMaterializationResult::Success(
                slug_bzlmod_v2::RepositoryMaterializationSuccess::Immutable {
                    observation_instance,
                    ..
                },
            ) => Some(observation_instance.value()),
            _ => None,
        }))
        .max()
        .unwrap_or(0)
        .max(floor)
        .max(retained_floor)
        .checked_add(1)
        .ok_or("fixture observation instance overflow")?;
    next_instance
        .checked_add(
            (need.repository_materializations().len() as u64)
                .checked_mul(64)
                .ok_or("fixture instance count overflow")?,
        )
        .ok_or("fixture observation instance overflow")?;
    // Prepay the old/new transfer peak before cloning any current entries or
    // observation guards. This is the same fixture budget, not another pool.
    let transfer_count = original
        .observations()
        .len()
        .checked_add(entries.len())
        .ok_or("fixture transfer overflow")?;
    let transfer_bytes = transfer_count
        .checked_mul(
            std::mem::size_of::<RepositoryMaterializationEpochEntry>()
                + std::mem::size_of::<(
                    slug_workspace_v2::PathObservationDemand,
                    std::sync::Arc<slug_workspace_v2::PathObservationResult>,
                )>(),
        )
        .and_then(|n| n.checked_mul(4))
        .ok_or("fixture transfer storage overflow")?;
    budget.charge(transfer_count, transfer_bytes)?;
    stable_inputs::admit_transfer(&original, entries, budget)?;
    let mut observations = original
        .observations()
        .iter()
        .map(|(demand, value)| (demand.clone(), value.clone()))
        .collect::<Vec<_>>();
    for request in need.repository_materializations().values() {
        let reused = match prior.as_mut() {
            Some(prior) => prior.reuse(request, &original, budget).await?,
            None => None,
        };
        let (entry, observed) = match reused {
            Some(reused) => reused,
            None => files::write(workspace, request, &mut next_instance)?,
        };
        // Only declaratively carried stale entries may be replaced after the
        // CURRENT real owner returned this changed request as an actual Need.
        if let Some(index) = entries
            .iter()
            .position(|old| old.request.id.canonical_repo == entry.request.id.canonical_repo)
        {
            entries[index] = entry;
        } else {
            entries.push(entry);
        }
        observations.extend(observed);
    }
    let epoch = PathObservationEpoch::from_shared(observations).map_err(|e| format!("{e:?}"))?;
    let materializations = RepositoryMaterializationResultEpoch::new(root.clone(), entries.clone())
        .and_then(|epoch| epoch.retaining_generated_outputs(previous_epoch.as_ref(), 65_536))
        .map_err(|e| format!("{e:?}"))?;
    let mut updater = transaction.into_updater();
    updater
        .changed_to(path_observation_shards(&epoch))
        .map_err(|e| e.to_string())?;
    updater
        .changed_to([(PathObservationEpochKey, epoch)])
        .map_err(|e| e.to_string())?;
    updater
        .changed_to([(
            RepositoryMaterializationResultEpochKey {
                workspace: root.clone(),
            },
            materializations,
        )])
        .map_err(|e| e.to_string())?;
    Ok(updater.commit().await)
}

#[cfg(test)]
#[path = "generated_registration/tests.rs"]
mod tests;
