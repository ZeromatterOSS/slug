//! Complete byte proofs over actual mapping allocations, owned by one publisher.
use slug_build_api_v2::published_value::PublicationSourcePreparation;

use super::*;

type Mapping = Arc<
    [(
        slug_identity_v2::ApparentRepoName,
        slug_identity_v2::CanonicalRepoName,
    )],
>;
struct OwnedProof {
    guard: Mapping,
    bytes: usize,
}
struct BorrowedProof<'a> {
    guard: &'a Mapping,
    bytes: usize,
}

#[derive(Default)]
pub(crate) struct MappingAudits {
    committed: UnorderedMap<usize, OwnedProof>,
    credit: usize,
    failed: bool,
    header_admitted: bool,
}
impl MappingAudits {
    pub(crate) fn admit_header(
        &mut self,
        builder: &mut PublishedValueArenaBuilder,
    ) -> Result<(), String> {
        self.check()?;
        if !self.header_admitted {
            builder
                .producer_scratch()
                .charge_bytes(size_of::<Self>())
                .map_err(|e| e.to_string())?;
            self.header_admitted = true;
        }
        Ok(())
    }

    pub(crate) fn check(&self) -> Result<(), String> {
        if self.failed {
            Err("Args source session previously failed".into())
        } else {
            Ok(())
        }
    }
    pub(crate) fn fail(&mut self) {
        self.failed = true;
    }
}

pub(super) struct MappingPlan<'a> {
    pending: UnorderedMap<usize, BorrowedProof<'a>>,
    credit: usize,
    pub(super) output_bytes: usize,
}
impl<'a> MappingPlan<'a> {
    pub(super) fn new(builder: &mut PublishedValueArenaBuilder) -> Result<Self, String> {
        // Complete fixed owner, including scalars and padding, even when all
        // mappings hit and the pending table never allocates. Table peak's
        // inline map header is conservatively counted a second time on growth.
        builder
            .producer_scratch()
            .charge_bytes(size_of::<Self>())
            .map_err(|e| e.to_string())?;
        Ok(Self {
            pending: UnorderedMap::new(),
            credit: 0,
            output_bytes: 0,
        })
    }
    pub(super) fn mapping_bytes(
        &mut self,
        mapping: &'a Mapping,
        audits: &MappingAudits,
        builder: &mut PublishedValueArenaBuilder,
        remaining: &mut usize,
    ) -> Result<usize, String> {
        audits.check()?;
        let key = Arc::as_ptr(mapping) as *const () as usize;
        step(builder, remaining)?;
        if let Some(proof) = audits.committed.get(&key) {
            same(&proof.guard, mapping)?;
            return Ok(proof.bytes);
        }
        step(builder, remaining)?;
        if let Some(proof) = self.pending.get(&key) {
            same(proof.guard, mapping)?;
            return Ok(proof.bytes);
        }
        let mut bytes = 2 * size_of::<usize>();
        for (apparent, canonical) in &**mapping {
            step(builder, remaining)?;
            add(
                &mut bytes,
                size_of::<(
                    slug_identity_v2::ApparentRepoName,
                    slug_identity_v2::CanonicalRepoName,
                )>(),
            )?;
            add(&mut bytes, apparent.as_str().len())?;
            add(&mut bytes, canonical.as_str().len())?;
        }
        step(builder, remaining)?;
        let count = count(self.pending.len(), 1)?;
        let required = peak::<BorrowedProof<'a>>(count)?;
        builder
            .producer_scratch()
            .charge_bytes(required.saturating_sub(self.credit))
            .map_err(|e| e.to_string())?;
        self.pending.insert(
            key,
            BorrowedProof {
                guard: mapping,
                bytes,
            },
        );
        self.credit = required;
        Ok(bytes)
    }
    /// All output, map growth, move temporaries and commit work are admitted
    /// before cloning the first owning guard. No resource admission in commit.
    pub(super) fn admit<'b>(
        &self,
        audits: &MappingAudits,
        builder: &'b mut PublishedValueArenaBuilder,
        remaining: &mut usize,
        output_transfer: usize,
    ) -> Result<(usize, PublicationSourcePreparation<'b>), String> {
        audits.check()?;
        let total = count(audits.committed.len(), self.pending.len())?;
        let required = peak::<OwnedProof>(total)?;
        let mut bytes = self.output_bytes;
        add(&mut bytes, required.saturating_sub(audits.credit))?;
        add(
            &mut bytes,
            self.pending
                .len()
                .checked_mul(size_of::<OwnedProof>())
                .ok_or("Args mapping commit byte overflow")?,
        )?;
        // Preserve complete P before any owning clone. Only the checked old
        // Vec backing credit joins scratch; full native Q remains retained.
        let scratch = bytes
            .checked_sub(self.output_bytes)
            .ok_or("Args mapping scratch byte underflow")?;
        let retained_output = self
            .output_bytes
            .checked_sub(output_transfer)
            .ok_or_else(|| {
                // Match the enclosing conversion's permanent poison on admission
                // failure, including direct private use before a token exists.
                let _ = builder.charge_bytes(usize::MAX);
                "Args source transfer byte underflow".to_owned()
            })?;
        let mut preparation = builder
            .begin_source_authority(retained_output, output_transfer, scratch)
            .map_err(|e| e.to_string())?;
        for _ in 0..self.pending.len() {
            if let Err(error) = preparation_step(&mut preparation, remaining) {
                preparation.abort();
                return Err(error);
            }
        }
        Ok((required, preparation))
    }
    pub(super) fn commit(self, audits: &mut MappingAudits, admitted_credit: usize) {
        for (key, proof) in self.pending.entries_unordered() {
            let previous = audits.committed.insert(
                *key,
                OwnedProof {
                    guard: Arc::clone(proof.guard),
                    bytes: proof.bytes,
                },
            );
            debug_assert!(previous.is_none());
        }
        audits.credit = admitted_credit;
    }
}

// Preserve source::step's native-first/host-second order while the preparation
// owns the builder borrow. No callback or general budget access escapes it.
fn preparation_step(
    preparation: &mut PublicationSourcePreparation<'_>,
    remaining: &mut usize,
) -> Result<(), String> {
    preparation.charge_visit().map_err(|e| e.to_string())?;
    *remaining = remaining
        .checked_sub(1)
        .ok_or("Args source inspection visit limit")?;
    Ok(())
}

fn same(a: &Mapping, b: &Mapping) -> Result<(), String> {
    if Arc::ptr_eq(a, b) {
        Ok(())
    } else {
        Err("Args mapping allocation guard mismatch".into())
    }
}
fn count(old: usize, added: usize) -> Result<usize, String> {
    old.checked_add(added)
        .filter(|n| *n <= slug_build_api_v2::callable_graph::CALLABLE_GRAPH_NODE_LIMIT)
        .ok_or_else(|| "Args mapping audit entry limit".into())
}
fn peak<V>(count: usize) -> Result<usize, String> {
    if count == 0 {
        return Ok(0);
    }
    // Same pinned-table minimum/control/alignment and old/new rehash peak
    // forecast as graph/storage.rs, using this map's actual key/value layout.
    let cell = size_of::<(usize, V)>()
        .checked_add(16)
        .and_then(|n| n.checked_add(std::mem::align_of::<(usize, V)>()));
    count
        .max(4)
        .checked_mul(8)
        .and_then(|n| cell.and_then(|cell| n.checked_mul(cell)))
        .and_then(|n| n.checked_add(size_of::<UnorderedMap<usize, V>>()))
        .ok_or_else(|| "Args mapping audit storage overflow".into())
}

#[cfg(test)]
mod tests;
