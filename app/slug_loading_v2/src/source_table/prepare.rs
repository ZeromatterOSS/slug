//! Positive full table certificate composed during an already-paid identity pass.
use slug_build_api_v2::DepsetStorageCharge;
use slug_build_api_v2::published_value::PublishedValueArenaBuilder;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalRepoName;

use super::*;
use crate::attrs::owner_metadata::charge;

pub(crate) struct Plan<'a> {
    native: PublishedValueArenaBuilder,
    intrinsic: PublishedValueArenaBuilder,
    bank: super::mapping::bank::Bank<'a>,
    publication: Option<crate::bzl_environment::publication::PreparedPublication>,
    count: usize,
    rows: usize,
    names: usize,
    finish_prepaid: bool,
    extra_bytes: usize,
    metadata_storage: PublicationShape,
}
impl<'a> Plan<'a> {
    pub(crate) fn new(count: usize, work: &mut Budget) -> Result<Self, String> {
        let bank = super::mapping::bank::Bank::new(count, count, work)?;
        Self::with_bank(count, bank, work)
    }
    pub(crate) fn with_bank(
        count: usize,
        bank: super::mapping::bank::Bank<'a>,
        work: &mut Budget,
    ) -> Result<Self, String> {
        // Read/layout/control and complete new descriptor/cache/guard envelope.
        work.charge(
            4,
            std::mem::size_of::<Self>() + std::mem::size_of::<CertifiedSourceTable>(),
        )?;
        let backing =
            std::alloc::Layout::array::<(CompactString, BzlModuleSourceProvenance)>(count)
                .map_err(|_| "source table array overflow")?
                .size();
        let header = Self::descriptor_bytes();
        work.charge(4, header)?;
        let mut native = PublishedValueArenaBuilder::new();
        native
            .charge_storage(DepsetStorageCharge {
                nodes: 0,
                edges: 2,
                visits: 4,
                bytes: header
                    .checked_add(backing)
                    .ok_or("source table backing overflow")?,
            })
            .map_err(|e| e.to_string())?;
        native.observe_depth(4).map_err(|e| e.to_string())?;
        work.charge(4, 0)?;
        let mut intrinsic = PublishedValueArenaBuilder::new();
        intrinsic
            .charge_storage(DepsetStorageCharge {
                nodes: 0,
                edges: 2,
                visits: 4,
                bytes: header
                    .checked_add(backing)
                    .ok_or("source table backing overflow")?,
            })
            .map_err(|e| e.to_string())?;
        intrinsic.observe_depth(5).map_err(|e| e.to_string())?;
        Ok(Self {
            native,
            intrinsic,
            bank,
            publication: None,
            count,
            rows: 0,
            names: 0,
            finish_prepaid: false,
            extra_bytes: 0,
            metadata_storage: PublicationShape {
                nodes: 0,
                edges: 0,
                bytes: 0,
                visits: 0,
                depth: 0,
            },
        })
    }
    fn emit(&mut self, c: DepsetStorageCharge, work: &mut Budget) -> Result<(), String> {
        work.charge(c.visits, 0)?;
        self.native.charge_storage(c).map_err(|e| e.to_string())?;
        work.charge(c.visits, 0)?;
        self.intrinsic.charge_storage(c).map_err(|e| e.to_string())
    }
    fn emit_mapping(&mut self, c: DepsetStorageCharge, work: &mut Budget) -> Result<(), String> {
        work.charge(c.visits, 0)?;
        self.native.charge_storage(c).map_err(|e| e.to_string())
    }
    pub(crate) const fn descriptor_bytes() -> usize {
        std::mem::size_of::<SourceIdentityTable>() + 4 * std::mem::size_of::<usize>()
    }
    pub(crate) fn prefix(
        &mut self,
        source: &'a BzlModuleSourceProvenance,
        work: &mut Budget,
    ) -> Result<(), String> {
        self.bank
            .select(&source.identity.repository_mapping, work)?;
        work.charge(2, 0)?;
        self.rows = self
            .rows
            .checked_add(1)
            .filter(|n| *n <= self.count)
            .ok_or("source table row bound")?;
        charge::normal(
            std::mem::size_of::<(CompactString, BzlModuleSourceProvenance)>(),
            &mut |c| self.emit(c, work),
        )?;
        charge::source_prefix(&source.identity, &mut |c| self.emit(c, work))?;
        work.charge(8, 0)?;
        let id = &source.identity;
        let label = &id.label;
        let extras = [
            label
                .package()
                .repo()
                .retained_capacity_bytes()
                .checked_sub(label.package().repo().as_str().len()),
            label
                .package()
                .package()
                .retained_capacity_bytes()
                .checked_sub(label.package().package().as_str().len()),
            label
                .target()
                .retained_capacity_bytes()
                .checked_sub(label.target().as_str().len()),
            label.mapping_id().map_or(Some(0), |m| {
                m.retained_capacity_bytes().checked_sub(m.as_str().len())
            }),
            match &id.source {
                slug_build_api_v2::ModuleSourceOrigin::Workspace(p) => {
                    p.capacity().checked_sub(p.as_os_str().len())
                }
                _ => Some(0),
            },
        ]
        .into_iter()
        .try_fold(0usize, |a, b| a.checked_add(b?))
        .ok_or("source table identity capacity")?;
        self.extra(extras, work)
    }
    pub(crate) fn filename(
        &mut self,
        name: &CompactString,
        work: &mut Budget,
    ) -> Result<(), String> {
        work.charge(2, 0)?;
        self.filename_prepaid(name, name.capacity(), work)
    }
    pub(crate) fn filename_from_str(
        &mut self,
        name: &str,
        work: &mut Budget,
    ) -> Result<(), String> {
        work.charge(2, 0)?;
        // compact_str 0.8.2 fresh heap storage has a 32-byte minimum on 64-bit;
        // an additional word conservatively covers out-of-line capacity metadata.
        self.filename_prepaid(
            name,
            name.len()
                .max(32)
                .checked_add(std::mem::size_of::<usize>())
                .ok_or("source table filename bound")?,
            work,
        )
    }
    fn filename_prepaid(
        &mut self,
        name: &str,
        capacity: usize,
        work: &mut Budget,
    ) -> Result<(), String> {
        self.names = self
            .names
            .checked_add(1)
            .filter(|n| *n <= self.count)
            .ok_or("source table name bound")?;
        charge::normal(name.len(), &mut |c| self.emit(c, work))?;
        self.extra(
            capacity
                .checked_sub(name.len())
                .ok_or("source table filename capacity")?,
            work,
        )
    }
    pub(crate) fn mapping(
        &mut self,
        apparent: &ApparentRepoName,
        canonical: &CanonicalRepoName,
        work: &mut Budget,
    ) -> Result<(), String> {
        self.bank.entry(apparent, canonical, work)?;
        self.mapping_completed(apparent, canonical, work)
    }
    /// One paid classification per row, before choosing either mapping loop.
    pub(crate) fn needs_mapping_preparation(&self, work: &mut Budget) -> Result<bool, String> {
        self.bank.needs_preparation(work)
    }
    /// Complete legacy mapping forecast; completed rows perform no bank lookup.
    pub(crate) fn mapping_completed(
        &mut self,
        apparent: &ApparentRepoName,
        canonical: &CanonicalRepoName,
        work: &mut Budget,
    ) -> Result<(), String> {
        work.charge(3, 0)?;
        charge::mapping_entry(apparent.as_str(), canonical.as_str(), &mut |c| {
            self.emit_mapping(c, work)
        })?;
        let extra = apparent
            .retained_capacity_bytes()
            .checked_sub(apparent.as_str().len())
            .and_then(|a| {
                canonical
                    .retained_capacity_bytes()
                    .checked_sub(canonical.as_str().len())
                    .and_then(|b| a.checked_add(b))
            })
            .ok_or("source table mapping capacity")?;
        self.extra_mapping(extra, work)
    }
    fn extra_mapping(&mut self, bytes: usize, work: &mut Budget) -> Result<(), String> {
        work.charge(0, bytes)?;
        self.native
            .charge_storage(DepsetStorageCharge {
                nodes: 0,
                edges: 0,
                visits: 0,
                bytes,
            })
            .map_err(|e| e.to_string())?;
        self.extra_bytes = self
            .extra_bytes
            .checked_add(bytes)
            .ok_or("source table capacity overflow")?;
        Ok(())
    }
    fn extra(&mut self, bytes: usize, work: &mut Budget) -> Result<(), String> {
        work.charge(0, bytes)?;
        self.native
            .charge_storage(DepsetStorageCharge {
                nodes: 0,
                edges: 0,
                visits: 0,
                bytes,
            })
            .map_err(|e| e.to_string())?;
        work.charge(1, 0)?;
        self.intrinsic
            .charge_storage(DepsetStorageCharge {
                nodes: 0,
                edges: 0,
                visits: 0,
                bytes,
            })
            .map_err(|e| e.to_string())?;
        self.extra_bytes = self
            .extra_bytes
            .checked_add(bytes)
            .ok_or("source table capacity overflow")?;
        Ok(())
    }
    pub(crate) fn extra_bytes(&self) -> usize {
        self.extra_bytes
    }
    pub(crate) fn copy_volume(
        &self,
        raw: &super::mapping::Raw,
        work: &mut Budget,
    ) -> Result<Option<(usize, usize)>, String> {
        self.bank.copy_volume(raw, work)
    }
    pub(crate) fn shape(&self) -> PublicationShape {
        self.native.local_shape()
    }
    pub(crate) fn completed_full_shape(&self) -> Result<PublicationShape, String> {
        self.publication
            .as_ref()
            .map(|p| p.full())
            .ok_or_else(|| "source table publication missing".into())
    }
    pub(crate) fn metadata_storage(&self) -> PublicationShape {
        self.metadata_storage
    }
    pub(crate) fn prepay_finish(&mut self, work: &mut Budget) -> Result<(), String> {
        work.charge(8, 0)?;
        self.bank
            .prepay_finish(self.intrinsic.local_shape(), work)?;
        self.metadata_storage = self.bank.metadata_storage()?;
        let storage = self.bank.storage()?;
        self.intrinsic
            .charge_storage(DepsetStorageCharge {
                nodes: storage.nodes,
                edges: storage.edges,
                visits: storage.visits,
                bytes: storage.bytes,
            })
            .map_err(|e| e.to_string())?;
        let publication = crate::bzl_environment::publication::PreparedPublication::prepare(
            self.intrinsic.local_shape(),
            self.count,
            self.bank.shapes(),
            |r, b| work.charge(r, b),
        )?;
        self.publication = Some(publication);
        self.finish_prepaid = true;
        Ok(())
    }
    pub(crate) fn finish(self, raw: &Rows) -> Result<CertifiedSourceTable, String> {
        if !self.finish_prepaid
            || self.rows != self.count
            || self.names != self.count
            || raw.len() != self.count
        {
            return Err("source table completion rows".into());
        }
        let (mappings, children) = self.bank.finish()?;
        let publication = self
            .publication
            .ok_or("source table publication missing")?
            .finish(children)?;
        let table = Arc::new(SourceIdentityTable {
            raw: raw.clone(),
            publication,
            mappings,
        });
        Ok(CertifiedSourceTable {
            table: table.clone(),
            declarations: [SharedPublicationInput::new(table)],
        })
    }
}
