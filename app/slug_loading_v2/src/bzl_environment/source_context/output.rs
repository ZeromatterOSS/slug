//! Full persistent output and simultaneous construction/transfer admission.
use std::mem::size_of;

use super::*;

pub(super) struct Output<'a> {
    pub(super) retained: Budget,
    pub(super) work: Budget,
    table: Option<crate::source_table::prepare::Plan<'a>>,
}
impl<'a> Output<'a> {
    pub(super) fn new(work: Budget) -> Self {
        Self {
            retained: Budget::default(),
            work,
            table: None,
        }
    }
    fn charge(&mut self, records: usize, bytes: usize) -> Result<(), String> {
        // No second allowance: persistent output joins the already-paid plan and
        // scratch before any owning clone. Retention is only a derived certificate.
        self.work.charge(records, bytes)?;
        self.retained_charge("effective source output retained", records, bytes)
    }
    pub(super) fn retained_charge(
        &mut self,
        phase: &'static str,
        records: usize,
        bytes: usize,
    ) -> Result<(), String> {
        self.retained.charge(records, bytes).map_err(|error| {
            format!(
                "{} (requested_records={records} requested_bytes={bytes})",
                self.retained.context(phase, error),
            )
        })
    }
    fn identity(
        &mut self,
        id: &BzlModuleIdentity,
        inherited: &crate::source_table::inherited::Inherited<'_>,
    ) -> Result<(), String> {
        plan::identity_prefix(id, |r, b| self.charge(r, b))?;
        let volume = inherited.copy_volume(&id.repository_mapping, &mut self.work)?;
        self.mapping_copy(id, volume)
    }
    /// Only the completed bank lookup supplies Some in production. Aggregate
    /// retention is attempted first and reports its real partial Budget state.
    pub(super) fn mapping_copy(
        &mut self,
        id: &BzlModuleIdentity,
        volume: Option<(usize, usize)>,
    ) -> Result<(), String> {
        match volume {
            Some((records, bytes)) => {
                let control_bytes = size_of::<(usize, usize, Budget)>();
                self.work.charge(3, control_bytes).map_err(|error| {
                    format!(
                        "{} (requested_records=3 requested_bytes={control_bytes})",
                        self.work
                            .context("effective source COPY control work", error),
                    )
                })?;
                self.retained_charge("effective source COPY retained", records, bytes)?;
                self.work.charge(0, bytes).map_err(|error| {
                    format!(
                        "{} (requested_records=0 requested_bytes={bytes})",
                        self.work.context("effective source COPY work", error),
                    )
                })
            }
            None => plan::mapping_entries(id, |r, b, _, _| self.charge(r, b)),
        }
    }
    fn array<T>(&mut self, count: usize, transfer: bool) -> Result<(), String> {
        self.charge(
            0,
            (2 * size_of::<usize>()).saturating_add(count.saturating_mul(size_of::<T>())),
        )?;
        if transfer {
            // Complete min4 Vec old/new plus final Arc coexistence. Final payload
            // above stays retained; only these additional buffers are temporary.
            self.work.charge(
                1,
                size_of::<Vec<T>>().saturating_add(
                    count
                        .max(4)
                        .saturating_mul(4)
                        .saturating_mul(size_of::<T>()),
                ),
            )?;
        }
        Ok(())
    }
    pub(super) fn forecast(
        &mut self,
        raw: &BzlLoadManifest,
        env: &PreparedBzlEnvironment,
        rows: &[&'a BzlModuleSourceProvenance],
        inherited: crate::source_table::inherited::Inherited<'a>,
    ) -> Result<(), String> {
        self.charge(
            1,
            size_of::<BzlModuleSourceContext>()
                + size_of::<BzlLoadManifest>()
                + 8 * size_of::<usize>(),
        )?;
        self.identity(&raw.root, &inherited)?;
        // The retained raw manifest keeps BOTH Arc-backed arrays, independently
        // of the unique effective output. No temporary input guard substitutes.
        self.array::<BzlModuleIdentity>(raw.direct_children.len(), false)?;
        for child in &*raw.direct_children {
            self.identity(child, &inherited)?;
        }
        self.array::<BzlModuleSourceProvenance>(raw.reachable.len(), false)?;
        for source in &*raw.reachable {
            self.identity(&source.identity, &inherited)?;
            self.charge(1, 32)?;
        }
        self.array::<BzlEnvironmentSourceBinding>(env.exports.len(), true)?;
        for export in &*env.exports {
            self.identity(&export.selection.module, &inherited)?;
            self.charge(
                1,
                export
                    .selection
                    .global
                    .capacity()
                    .saturating_add(export.selection.export.capacity()),
            )?;
        }
        self.array::<BzlModuleSourceProvenance>(rows.len(), true)?;
        self.array::<(CompactString, BzlModuleSourceProvenance)>(rows.len(), true)?;
        let mut table =
            crate::source_table::prepare::Plan::with_bank(rows.len(), inherited, &mut self.work)?;
        self.retained_charge(
            "effective source descriptor retained",
            4,
            crate::source_table::prepare::Plan::descriptor_bytes(),
        )?;
        for source in rows {
            plan::identity_prefix(&source.identity, |r, b| self.charge(r, b))?;
            let volume = table.copy_volume(&source.identity.repository_mapping, &mut self.work)?;
            self.mapping_copy(&source.identity, volume)?;
            self.charge(1, 32)?;
            plan::identity_prefix(&source.identity, |r, b| self.charge(r, b))?;
            table.prefix(source, &mut self.work)?;
            if table.needs_mapping_preparation(&mut self.work)? {
                plan::mapping_entries(&source.identity, |r, b, a, c| {
                    self.charge(r, b)?;
                    table.mapping(a, c, &mut self.work)
                })?;
            } else {
                plan::mapping_entries(&source.identity, |r, b, a, c| {
                    self.charge(r, b)?;
                    table.mapping_completed(a, c, &mut self.work)
                })?;
            }
            let path = source.identity.source.diagnostic_path();
            let name = path.to_str().ok_or("non-UTF8 effective source filename")?;
            self.charge(
                1,
                32usize
                    .saturating_add(size_of::<CompactString>())
                    .saturating_add(name.len())
                    .saturating_add(2 * size_of::<usize>()),
            )?;
            table.filename_from_str(name, &mut self.work)?;
        }
        table.prepay_finish(&mut self.work)?;
        // The context at depth0 owns the complete table at depth1. Admit the
        // cached scalar read, copy, checked addition and both depth checks on
        // the original producer before any output/capture guard is cloned.
        self.work.charge(
            6,
            size_of::<slug_build_api_v2::published_value::PublicationShape>()
                + 2 * size_of::<usize>(),
        )?;
        let depth = table
            .completed_full_shape()?
            .depth
            .checked_add(1)
            .ok_or("effective source table depth overflow")?;
        self.work.depth(depth)?;
        self.retained.depth(depth).map_err(|error| {
            format!(
                "{} (requested_depth={depth})",
                self.retained
                    .context("effective source depth retained", error),
            )
        })?;
        self.retained_charge("effective source capacity retained", 0, table.extra_bytes())?;
        let metadata = table.metadata_storage();
        self.retained_charge(
            "effective source metadata retained",
            metadata.visits,
            metadata.bytes,
        )?;
        self.table = Some(table);
        // Exact existing complete fingerprint stream is still traversed in full.
        // Admit that work now, BEFORE the first owned output clone.
        self.work.charge(1, 32)?;
        for export in &*env.exports {
            self.work.charge(
                1,
                export
                    .selection
                    .global
                    .len()
                    .saturating_add(export.selection.export.len()),
            )?;
            plan::identity(&export.selection.module, |r, b| self.work.charge(r, b))?;
        }
        for source in rows {
            plan::identity(&source.identity, |r, b| self.work.charge(r, b))?;
            self.work.charge(1, 32)?;
        }
        Ok(())
    }
    pub(super) fn finish_table(
        &mut self,
        raw: &crate::source_table::Rows,
    ) -> Result<crate::source_table::CertifiedSourceTable, String> {
        self.table
            .take()
            .ok_or("source table forecast missing")?
            .finish(raw)
    }
}
