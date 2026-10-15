//! One initial input commit; explicit observations remain authoritative.
use std::mem::size_of;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use dice::DiceTransactionUpdater;
use slug_bzlmod_v2::RepositoryMaterializationEpochEntry as Entry;
use slug_bzlmod_v2::RepositoryMaterializationKind as Kind;
use slug_bzlmod_v2::RepositoryMaterializationResult as ResultValue;
use slug_bzlmod_v2::RepositoryMaterializationResultEpoch as Epoch;
use slug_bzlmod_v2::RepositoryMaterializationResultEpochKey;
use slug_bzlmod_v2::RepositoryMaterializationSuccess as Success;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathObservationEpoch;
use slug_workspace_v2::PathObservationEpochKey;
use slug_workspace_v2::PathObservationNamespace;
use slug_workspace_v2::WorkspaceDirectorySnapshotKey;
use slug_workspace_v2::WorkspaceRawSnapshotKey;
use slug_workspace_v2::WorkspaceSnapshotKey;
use slug_workspace_v2::path_observation_shards;

use super::PriorInputs;
use super::SavedInputs;
use super::preflight;
#[path = "initial_inputs/automatic.rs"]
mod automatic;
#[cfg(test)]
pub(super) use automatic::host_epoch;

pub(crate) enum FixtureObservations {
    Automatic,
    Explicit(PathObservationEpoch),
}
impl From<PathObservationEpoch> for FixtureObservations {
    fn from(epoch: PathObservationEpoch) -> Self {
        Self::Explicit(epoch)
    }
}
fn generated(entry: &Entry) -> bool {
    matches!(
        (&entry.request.kind, &entry.result),
        (
            Kind::GeneratedFileEffects(_),
            ResultValue::Success(Success::Immutable { .. })
        )
    )
}
pub(in super::super) fn admit_inventory(
    previous: Option<&Epoch>,
    entries: &[Entry],
    root: &NormalizedAbsolutePath,
    budget: &mut super::super::Budget,
) -> Result<(), String> {
    budget.charge(
        entries
            .len()
            .checked_mul(entries.len())
            .and_then(|n| n.checked_add(1))
            .ok_or("fixture active map work")?,
        0,
    )?;
    for entry in entries {
        budget.charge(
            1,
            entry
                .request
                .id
                .canonical_repo
                .as_str()
                .len()
                .checked_mul(entries.len().max(1))
                .and_then(|n| n.checked_mul(8))
                .ok_or("fixture active map key width")?,
        )?;
    }
    let old = match previous {
        Some(p) => p
            .borrowed_retained_generated(root, 65_536)
            .map_err(|e| format!("{e:?}"))?,
        None => &[],
    };
    let active = previous
        .map(|p| p.borrowed_entries(root, 65_536))
        .transpose()
        .map_err(|e| format!("{e:?}"))?;
    let count = old
        .len()
        .checked_add(active.as_ref().map_or(0, |i| i.len()))
        .and_then(|n| n.checked_add(entries.len()))
        .ok_or("fixture inventory count")?;
    // Map construction scans all active entries, but complete foreign entry Eq
    // happens only among the actual supported generated incarnation records.
    budget.charge(
        count,
        count
            .max(4)
            .checked_mul(16 * (size_of::<Arc<Entry>>() + size_of::<Entry>()))
            .ok_or("fixture inventory storage")?,
    )?;
    let generated_count = old
        .iter()
        .map(Arc::as_ref)
        .chain(active.into_iter().flatten())
        .chain(entries)
        .filter(|e| generated(e))
        .count();
    let comparisons = generated_count
        .checked_add(1)
        .and_then(|n| n.checked_mul(4))
        .ok_or("fixture inventory comparisons")?;
    let active = previous
        .map(|p| p.borrowed_entries(root, 65_536))
        .transpose()
        .map_err(|e| format!("{e:?}"))?;
    for entry in old
        .iter()
        .map(Arc::as_ref)
        .chain(active.into_iter().flatten())
        .chain(entries)
        .filter(|e| generated(e))
    {
        let before = (budget.records, budget.bytes);
        preflight::request(&entry.request, budget)?;
        if let ResultValue::Success(Success::Immutable {
            source_identity,
            generation_root,
            ..
        }) = &entry.result
        {
            budget.charge(
                4,
                (source_identity.len() + generation_root.as_os_str().len())
                    .checked_mul(16)
                    .ok_or("fixture immutable payload")?,
            )?;
        }
        let records = budget.records - before.0;
        let bytes = budget.bytes - before.1;
        // Complete multiplied work/payload-width bound is paid BEFORE the real
        // inventory constructor's foreign collision comparisons or Arc clones.
        budget.charge(
            records
                .checked_mul(comparisons)
                .ok_or("fixture inventory work")?,
            bytes
                .checked_mul(comparisons)
                .ok_or("fixture inventory comparison width")?,
        )?;
    }
    Ok(())
}
fn automatic_roots(
    materializations: &Epoch,
    entries: &[Entry],
    root: &NormalizedAbsolutePath,
    workspace: &Path,
    budget: &mut super::super::Budget,
) -> Result<Vec<PathBuf>, String> {
    let mut roots = Vec::new();
    for entry in materializations
        .borrowed_retained_generated(root, 65_536)
        .map_err(|e| format!("{e:?}"))?
    {
        let ResultValue::Success(Success::Immutable {
            generation_root, ..
        }) = &entry.result
        else {
            unreachable!("validated inventory")
        };
        budget.charge(1, 0)?;
        if !generation_root.starts_with(workspace) {
            continue;
        }
        for current in entries {
            budget.charge(1, 0)?;
            if let Kind::Local { logical_root } = &current.request.kind {
                if logical_root.as_path().starts_with(generation_root)
                    || generation_root.starts_with(logical_root.as_path())
                {
                    return Err("generated source overlaps explicit local authority".into());
                }
            }
        }
        budget.charge(
            1,
            generation_root
                .as_os_str()
                .len()
                .checked_mul(4)
                .and_then(|n| n.checked_add(16 * size_of::<PathBuf>()))
                .ok_or("fixture root guard")?,
        )?;
        roots.push(generation_root.clone());
    }
    Ok(roots)
}

fn compose_observations(
    prior: Option<&PriorInputs>,
    current: &PathObservationEpoch,
    entries: &[Entry],
    explicit: &[slug_identity_v2::CanonicalRepoName],
    budget: &mut super::super::Budget,
) -> Result<PathObservationEpoch, String> {
    super::admit_transfer(current, entries, budget)?;
    let mut observations = Vec::new();
    budget.charge(
        current.observations().len(),
        current
            .observations()
            .len()
            .max(4)
            .checked_mul(
                16 * size_of::<(
                    slug_workspace_v2::PathObservationDemand,
                    Arc<slug_workspace_v2::PathObservationResult>,
                )>(),
            )
            .ok_or("fixture initial observations")?,
    )?;
    observations.extend(
        current
            .observations()
            .iter()
            .map(|(d, v)| (d.clone(), v.clone())),
    );
    if let Some(prior) = prior {
        for entry in entries {
            if explicit.contains(&entry.request.id.canonical_repo) {
                continue;
            }
            let ResultValue::Success(Success::Immutable {
                observation_instance,
                ..
            }) = &entry.result
            else {
                continue;
            };
            preflight::observations(&prior.observations, current, *observation_instance, budget)?;
            observations.extend(
                prior
                    .observations
                    .observations()
                    .iter()
                    .filter(|(d, _)| {
                        d.namespace()
                            == PathObservationNamespace::Materialization(*observation_instance)
                            && current.get(d).is_none()
                    })
                    .map(|(d, v)| (d.clone(), v.clone())),
            );
        }
    }
    PathObservationEpoch::from_shared(observations).map_err(|e| format!("{e:?}"))
}

impl SavedInputs {
    pub(crate) fn inject_initial(
        &mut self,
        updater: &mut DiceTransactionUpdater,
        workspace: &Path,
        mut entries: Vec<Entry>,
        source: FixtureObservations,
    ) -> Result<Vec<Entry>, String> {
        let root =
            NormalizedAbsolutePath::new(workspace.to_path_buf()).map_err(|e| e.to_string())?;
        self.budget.charge(
            entries.len(),
            entries
                .len()
                .max(4)
                .checked_mul(16 * size_of::<slug_identity_v2::CanonicalRepoName>())
                .ok_or("fixture explicit table")?,
        )?;
        for entry in &entries {
            self.budget.charge(
                1,
                entry
                    .request
                    .id
                    .canonical_repo
                    .as_str()
                    .len()
                    .checked_mul(4)
                    .ok_or("fixture explicit text")?,
            )?;
        }
        self.explicit = entries
            .iter()
            .map(|e| e.request.id.canonical_repo.clone())
            .collect();
        if let Some(prior) = &self.prior {
            admit_inventory(
                Some(&prior.materializations),
                &entries,
                &root,
                &mut self.budget,
            )?;
            let active = prior
                .materializations
                .borrowed_entries(&root, 65_536)
                .map_err(|e| format!("{e:?}"))?;
            for entry in active {
                self.budget.charge(1, 0)?;
                if generated(entry)
                    && !entries
                        .iter()
                        .any(|e| e.request.id.canonical_repo == entry.request.id.canonical_repo)
                {
                    self.budget.charge(entries.len(), size_of::<Entry>() * 16)?;
                    preflight::request(&entry.request, &mut self.budget)?;
                    if let ResultValue::Success(Success::Immutable {
                        generation_root,
                        source_identity,
                        ..
                    }) = &entry.result
                    {
                        self.budget.charge(
                            1,
                            (generation_root.as_os_str().len() + source_identity.len())
                                .checked_mul(8)
                                .ok_or("fixture carried result")?,
                        )?;
                    }
                    entries.push(entry.clone());
                }
            }
        }
        // Build and validate the complete physical owner ONCE, after full
        // prepayment and before any source walk. Current and retired genuine
        // declarations now use the same exact constructor/checks.
        admit_inventory(
            self.prior.as_ref().map(|p| &p.materializations),
            &entries,
            &root,
            &mut self.budget,
        )?;
        let materializations = Epoch::new(root.clone(), entries.clone())
            .and_then(|e| {
                e.retaining_generated_outputs(
                    self.prior.as_ref().map(|p| &p.materializations),
                    65_536,
                )
            })
            .map_err(|e| format!("{e:?}"))?;
        let roots = match &source {
            FixtureObservations::Automatic => automatic_roots(
                &materializations,
                &entries,
                &root,
                workspace,
                &mut self.budget,
            )?,
            FixtureObservations::Explicit(_) => Vec::new(),
        };
        let (text, raw, directories) = automatic::collect(workspace, &roots, &mut self.budget)?;
        let current = match source {
            FixtureObservations::Automatic => {
                automatic::host_epoch(workspace, &text, &directories, &mut self.budget)?
            }
            FixtureObservations::Explicit(epoch) => epoch,
        };
        let observations = compose_observations(
            self.prior.as_ref(),
            &current,
            &entries,
            &self.explicit,
            &mut self.budget,
        )?;
        updater
            .changed_to([(
                WorkspaceSnapshotKey {
                    workspace: workspace.to_path_buf(),
                },
                text,
            )])
            .map_err(|e| e.to_string())?;
        updater
            .changed_to([(
                WorkspaceRawSnapshotKey {
                    workspace: workspace.to_path_buf(),
                },
                raw,
            )])
            .map_err(|e| e.to_string())?;
        updater
            .changed_to([(
                WorkspaceDirectorySnapshotKey {
                    workspace: workspace.to_path_buf(),
                },
                directories,
            )])
            .map_err(|e| e.to_string())?;
        updater
            .changed_to(path_observation_shards(&observations))
            .map_err(|e| e.to_string())?;
        updater
            .changed_to([(PathObservationEpochKey, observations)])
            .map_err(|e| e.to_string())?;
        updater
            .changed_to([(
                RepositoryMaterializationResultEpochKey { workspace: root },
                materializations,
            )])
            .map_err(|e| e.to_string())?;
        Ok(entries)
    }
}
