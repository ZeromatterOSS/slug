//! Pure, privately completed guard for one exact ordered mapping allocation.
use slug_build_api_v2::DepsetStorageCharge;
use slug_build_api_v2::published_value::PublishedValueArenaBuilder;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalRepoName;

use super::*;
pub(crate) type Raw = Arc<[(ApparentRepoName, CanonicalRepoName)]>;
#[derive(Debug, Allocative)]
pub(super) struct Body {
    raw: Raw,
    shape: PublicationShape,
}
impl ImmutablePublicationInput for Body {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn retained_shape(&self) -> PublicationShape {
        self.shape
    }
}
// Private positive legacy COPY volume, sealed only with the exact raw guard.
#[derive(Debug, Clone, Copy, Allocative)]
struct CopyVolume {
    records: usize,
    bytes: usize,
}
#[derive(Debug, Clone, Allocative)]
pub(crate) struct MappingPair {
    raw: Raw,
    input: SharedPublicationInput,
    copy: CopyVolume,
}
impl MappingPair {
    pub(super) fn pointer(&self) -> usize {
        Arc::as_ptr(&self.raw) as *const () as usize
    }
    pub(super) fn shape(&self) -> PublicationShape {
        self.input.retained_shape()
    }
    pub(super) fn input(&self) -> &SharedPublicationInput {
        &self.input
    }
}
/// No owning payload copy is made before the surrounding joint preflight.
pub(super) struct Prepared {
    native: PublishedValueArenaBuilder,
    remaining: usize,
    copy: CopyVolume,
}
impl Prepared {
    pub(super) fn new(count: usize, work: &mut Budget) -> Result<Self, String> {
        work.charge(
            8,
            std::mem::size_of::<Self>() + std::mem::size_of::<MappingPair>() + header_bytes(),
        )?;
        // Two scalar initializations; both temporary and retained Pair layouts
        // are also covered by the original size_of-based envelope above.
        work.charge(2, std::mem::size_of::<CopyVolume>())?;
        let backing = std::alloc::Layout::array::<(ApparentRepoName, CanonicalRepoName)>(count)
            .map_err(|_| "mapping backing overflow")?
            .size();
        let mut native = PublishedValueArenaBuilder::new();
        native
            .charge_storage(DepsetStorageCharge {
                nodes: 0,
                edges: 2,
                visits: 4,
                bytes: header_bytes()
                    .checked_add(backing)
                    .ok_or("mapping header overflow")?,
            })
            .map_err(|e| e.to_string())?;
        native.observe_depth(4).map_err(|e| e.to_string())?;
        Ok(Self {
            native,
            remaining: count,
            copy: CopyVolume {
                records: 0,
                bytes: 0,
            },
        })
    }
    pub(super) fn entry(
        &mut self,
        a: &ApparentRepoName,
        c: &CanonicalRepoName,
        work: &mut Budget,
    ) -> Result<(), String> {
        work.charge(4, 0)?;
        // Prepay the new two length reads and checked positive accumulation
        // before the old remaining/native operations; no cache is committed on
        // any refused or incomplete entry.
        work.charge(4, std::mem::size_of::<(CopyVolume, usize)>())?;
        let entry_bytes = std::mem::size_of::<(ApparentRepoName, CanonicalRepoName)>()
            .checked_add(a.as_str().len())
            .and_then(|n| n.checked_add(c.as_str().len()))
            .ok_or("mapping COPY volume overflow")?;
        let next = CopyVolume {
            records: self
                .copy
                .records
                .checked_add(1)
                .ok_or("mapping COPY records overflow")?,
            bytes: self
                .copy
                .bytes
                .checked_add(entry_bytes)
                .ok_or("mapping COPY bytes overflow")?,
        };
        self.remaining = self.remaining.checked_sub(1).ok_or("mapping entry count")?;
        crate::attrs::owner_metadata::charge::mapping_entry(a.as_str(), c.as_str(), &mut |v| {
            self.native.charge_storage(v).map_err(|e| e.to_string())
        })?;
        let bytes = a
            .retained_capacity_bytes()
            .checked_sub(a.as_str().len())
            .and_then(|n| {
                c.retained_capacity_bytes()
                    .checked_sub(c.as_str().len())
                    .and_then(|m| n.checked_add(m))
            })
            .ok_or("mapping capacity overflow")?;
        self.native
            .charge_storage(DepsetStorageCharge {
                nodes: 0,
                edges: 0,
                visits: 0,
                bytes,
            })
            .map_err(|e| e.to_string())?;
        self.copy = next;
        Ok(())
    }
    pub(super) fn shape(&self) -> Result<PublicationShape, String> {
        if self.remaining != 0 {
            return Err("incomplete mapping".into());
        }
        Ok(self.native.local_shape())
    }
    pub(super) fn finish(self, raw: &Raw) -> Result<MappingPair, String> {
        let shape = self.shape()?;
        if self.copy.records != raw.len() {
            return Err("incomplete mapping COPY volume".into());
        }
        let body = Arc::new(Body {
            raw: raw.clone(),
            shape,
        });
        Ok(MappingPair {
            raw: raw.clone(),
            input: SharedPublicationInput::new(body),
            copy: self.copy,
        })
    }
}
pub(super) const fn header_bytes() -> usize {
    std::mem::size_of::<Body>() + 4 * std::mem::size_of::<usize>()
}
#[path = "mapping/bank.rs"]
pub(super) mod bank;
