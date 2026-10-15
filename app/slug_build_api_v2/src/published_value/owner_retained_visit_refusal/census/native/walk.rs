//! Two envelope passes; payload children, arrays and names remain opaque.
use super::*;

pub(super) fn first<'a>(
    b: &'a PublishedValueArenaBuilder,
    cursor: &mut Cursor,
    out: &mut PublicationNativeRetentionCensus,
) -> Option<feature::Candidate<'a>> {
    let mut candidate = feature::Candidate::default();
    for identity in &b.closure_seen {
        cursor.reads.before_read()?;
        let arena = &identity.0.0;
        if arena.rows.len() > cursor.rows {
            return None;
        }
        out.rows = out.rows.checked_add(arena.rows.len())?;
        out.edges = out.edges.checked_add(arena.edges.len())?;
        out.slots = out.slots.checked_add(arena.original_slots.len())?;
        out.maximum_rows = out.maximum_rows.max(arena.rows.len());
        out.maximum_edges = out.maximum_edges.max(arena.edges.len());
        out.maximum_slots = out.maximum_slots.max(arena.original_slots.len());
        out.maximum_local_visits = out.maximum_local_visits.max(arena.local_shape.visits);
        let mut local = PublicationNativeStatistics::default();
        for row in arena.rows.iter() {
            cursor.row()?;
            let index = classifier::row(row) as usize;
            out.row_kinds[index] = out.row_kinds[index].checked_add(1)?;
            match row {
                PublishedNode::Native(atom) => {
                    cursor.certificate()?;
                    let (family, model) = classifier::atom(atom.value())?;
                    let visits = atom.retained_visits();
                    out.record(PublicationNativeChannel::Atom, family, visits, &mut local)?;
                    if let Some(model) = model {
                        candidate.consider(model, visits);
                    }
                }
                PublishedNode::Provider(provider) => {
                    if let Some(files) = &provider.files_to_run {
                        cursor.certificate()?;
                        out.record(
                            PublicationNativeChannel::HiddenFiles,
                            PublicationNativeRootFamily::FilesProvider,
                            files.retained_visits(),
                            &mut local,
                        )?;
                    }
                }
                PublishedNode::ConfiguredTarget { providers, .. } => {
                    if providers.len() > cursor.providers {
                        return None;
                    }
                    for entry in &**providers {
                        cursor.provider()?;
                        if let PublishedTargetProvider::Native { provider, .. } = entry {
                            cursor.certificate()?;
                            let family = classifier::provider(provider.value())?;
                            out.record(
                                PublicationNativeChannel::TargetProvider,
                                family,
                                provider.retained_visits(),
                                &mut local,
                            )?;
                        }
                    }
                }
                _ => {}
            }
        }
        if local.visits > arena.local_shape.visits {
            return None;
        }
        out.maximum_arena_certificates = out.maximum_arena_certificates.max(local.count);
        out.maximum_arena_native_visits = out.maximum_arena_native_visits.max(local.visits);
    }
    Some(candidate)
}
pub(super) fn second(
    b: &PublishedValueArenaBuilder,
    cursor: &mut Cursor,
    candidate: &feature::Candidate<'_>,
    out: &mut PublicationNativeRetentionCensus,
) -> Option<()> {
    for identity in &b.closure_seen {
        cursor.reads.before_read()?;
        let arena = &identity.0.0;
        if arena.rows.len() > cursor.rows {
            return None;
        }
        let mut matched = false;
        for row in arena.rows.iter() {
            cursor.row()?;
            if let PublishedNode::Native(atom) = row {
                cursor.certificate()?;
                if let (_, Some(model)) = classifier::atom(atom.value())? {
                    if candidate.matches(model) {
                        matched = true;
                        out.feature_occurrences = out.feature_occurrences.checked_add(1)?;
                        out.feature_wrapper_visits = out
                            .feature_wrapper_visits
                            .checked_add(atom.retained_visits())?;
                    }
                }
            }
        }
        if matched {
            out.feature_arenas = out.feature_arenas.checked_add(1)?;
        }
    }
    Some(())
}
