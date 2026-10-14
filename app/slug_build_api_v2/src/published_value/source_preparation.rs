//! Same-builder payment for one actual, constructor-validated source allocation.
use std::mem::align_of;
use std::mem::size_of;

use super::*;

#[derive(Debug)]
struct PaidAuthority {
    guard: CallableSourceAuthority,
    bytes: usize,
}
#[derive(Debug, Default)]
pub(super) struct SourcePreparations {
    paid: fxhash::FxHashMap<usize, PaidAuthority>,
    credit: usize,
    header_admitted: bool,
}

/// A trusted native producer's borrowed complete source-output admission.
///
/// R plus transfer scratch stays fully paid; complete final Q must fit R.
/// This proves payment in this builder and
/// binds it to the exact successfully validated authority allocation, not the
/// completeness of an arbitrary Rust caller's pre-copy forecast. Abandonment
/// poisons the builder, including after an ignored error. No credit transfers.
#[must_use]
pub struct PublicationSourcePreparation<'a> {
    builder: &'a mut PublishedValueArenaBuilder,
    retained_output: usize,
    next_credit: usize,
    finished: bool,
}
impl PublishedValueArenaBuilder {
    /// Admit complete final output and the reviewed consumed Vec backings.
    /// `retained_output` must cover full constructor Q; `output_transfer` may
    /// classify only temporary backing, never moved elements or source payload.
    pub fn begin_source_authority(
        &mut self,
        retained_output: usize,
        output_transfer: usize,
        mapping_scratch: usize,
    ) -> Result<PublicationSourcePreparation<'_>, PublishedValueError> {
        let result = (|| {
            let count = self
                .source_preparations
                .paid
                .len()
                .checked_add(1)
                .filter(|n| *n <= NODE_LIMIT)
                .ok_or(PublishedValueError::Limit("source preparation entries"))?;
            let next_credit = map_peak(count)?;
            let growth = next_credit
                .checked_sub(self.source_preparations.credit)
                .ok_or(PublishedValueError::Invalid("source preparation credit"))?;
            let header = if self.source_preparations.header_admitted {
                0
            } else {
                size_of::<SourcePreparations>()
            };
            let scratch = output_transfer
                .checked_add(mapping_scratch)
                .and_then(|n| n.checked_add(header))
                .and_then(|n| n.checked_add(size_of::<PublicationSourcePreparation<'_>>()))
                .and_then(|n| {
                    n.checked_add(size_of::<
                        crate::callable_graph::PreparedCallableSourceAuthority,
                    >())
                })
                .and_then(|n| n.checked_add(growth))
                .and_then(|n| n.checked_add(size_of::<PaidAuthority>()))
                .ok_or(PublishedValueError::Limit("source preparation bytes"))?;
            // Preserve output+mapping admission before pending native/host work.
            // Only transferred Vec backing and dropped proof/token storage
            // are temporary; full final Q must remain in retained_output.
            self.producer_scratch()
                .charge_parts(retained_output, scratch)?;
            self.source_preparations.header_admitted = true;
            // Prepay the actual finish probe and insertion before any output or
            // mapping guard copies. No admission remains at the commit point.
            self.charge_visit()?;
            self.charge_visit()?;
            Ok(next_credit)
        })();
        match result {
            Ok(next_credit) => Ok(PublicationSourcePreparation {
                builder: self,
                retained_output,
                next_credit,
                finished: false,
            }),
            Err(error) => {
                self.failed_budget = true;
                Err(error)
            }
        }
    }
    pub(super) fn prepaid_source_bytes(
        &mut self,
        source: &CallableSourceAuthority,
    ) -> Result<Option<usize>, PublishedValueError> {
        // Both the lookup and conditional guard check are paid before borrowing
        // the entry. A miss conservatively retains this fixed encounter charge.
        self.charge_visit()?;
        self.charge_visit()?;
        match self
            .source_preparations
            .paid
            .get(&source.storage_identity())
        {
            Some(entry) if entry.guard.shares_storage_with(source) => Ok(Some(entry.bytes)),
            Some(_) => {
                self.failed_budget = true;
                Err(PublishedValueError::Invalid("source payment guard"))
            }
            None => Ok(None),
        }
    }
}
impl PublicationSourcePreparation<'_> {
    pub fn charge_visit(&mut self) -> Result<(), PublishedValueError> {
        self.builder.charge_visit()
    }
    pub fn finish(
        mut self,
        data: crate::CallableSourceAuthorityData,
    ) -> Result<CallableSourceAuthority, PublishedValueError> {
        let result = self.finish_inner(data);
        self.finished = result.is_ok();
        result
    }
    fn finish_inner(
        &mut self,
        data: crate::CallableSourceAuthorityData,
    ) -> Result<CallableSourceAuthority, PublishedValueError> {
        if self.builder.failed_budget {
            return Err(PublishedValueError::Limit("poisoned source preparation"));
        }
        // Own the validated Data and Q together until the full retained-R check passes.
        let data = CallableSourceAuthority::prepare(data).map_err(|error| match error {
            crate::CallableGraphError::Limit(kind) => PublishedValueError::Limit(kind),
            _ => PublishedValueError::Invalid("source authority data"),
        })?;
        let bytes = data.byte_cost();
        if bytes > self.retained_output {
            return Err(PublishedValueError::Invalid(
                "source authority underpayment",
            ));
        }
        let authority = data.finish();
        let key = authority.storage_identity();
        if self.builder.source_preparations.paid.contains_key(&key) {
            return Err(PublishedValueError::Invalid(
                "source preparation allocation",
            ));
        }
        self.builder.source_preparations.paid.insert(
            key,
            PaidAuthority {
                guard: authority.clone(),
                bytes,
            },
        );
        self.builder.source_preparations.credit = self.next_credit;
        Ok(authority)
    }
    pub fn abort(self) {
        // Drop poisons without refunding any accepted construction forecast.
    }
}
impl Drop for PublicationSourcePreparation<'_> {
    fn drop(&mut self) {
        if !self.finished {
            self.builder.failed_budget = true;
        }
    }
}
fn map_peak(count: usize) -> Result<usize, PublishedValueError> {
    // Existing guarded proof-map policy: min4, bucket/control/alignment and
    // old/new rehash coexistence, using actual private entry sizes.
    count
        .max(4)
        .checked_mul(8)
        .and_then(|n| {
            size_of::<(usize, PaidAuthority)>()
                .checked_add(16)
                .and_then(|cell| cell.checked_add(align_of::<(usize, PaidAuthority)>()))
                .and_then(|cell| n.checked_mul(cell))
        })
        .and_then(|n| n.checked_add(size_of::<fxhash::FxHashMap<usize, PaidAuthority>>()))
        .ok_or(PublishedValueError::Limit("source preparation bytes"))
}

#[cfg(test)]
#[path = "source_preparation/tests.rs"]
mod tests;
