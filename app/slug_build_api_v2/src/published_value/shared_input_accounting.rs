use super::*;
use crate::published_value::owner::OwnerSharedAudit;

// Typed temporary frame fields/control allowance, paid by the caller BEFORE
// invoking the recursive frame (including completed memo hits).
const SHARED_FRAME_BYTES: usize = std::mem::size_of::<PublicationShape>() * 8
    + std::mem::size_of::<(&SharedPublicationInput, usize, bool)>() * 8;

enum SharedAdmission<'a> {
    Raw(&'a SharedPublicationInput),
    Owner(OwnerSharedAudit<'a>),
}
impl PublishedValueArenaBuilder {
    // Raw inputs never self-certify, even when their public trait returns a
    // cached shape. Only actual owner declarations can supply the private borrow.
    pub(in crate::published_value) fn audit_shared_input(
        &mut self,
        input: &SharedPublicationInput,
        depth: usize,
    ) -> Result<(), PublishedValueError> {
        self.audit_shared(SharedAdmission::Raw(input), depth)
    }
    pub(super) fn audit_owner_shared_input(
        &mut self,
        input: OwnerSharedAudit<'_>,
        depth: usize,
    ) -> Result<(), PublishedValueError> {
        self.audit_shared(SharedAdmission::Owner(input), depth)
    }
    fn audit_shared(
        &mut self,
        admission: SharedAdmission<'_>,
        depth: usize,
    ) -> Result<(), PublishedValueError> {
        let (input, certified) = match admission {
            SharedAdmission::Raw(input) => (input, false),
            SharedAdmission::Owner(witness) => (witness.input(), !self.strict_shared_audit),
        };
        let result = (|| {
            self.charge_visit()?;
            self.charge_bytes(SHARED_FRAME_BYTES)?;
            self.audit_shared_node(input, depth, certified)
        })();
        if result.is_err() {
            self.failed_budget = true;
        }
        result
    }
    fn audit_shared_node(
        &mut self,
        input: &SharedPublicationInput,
        depth: usize,
        certified: bool,
    ) -> Result<(), PublishedValueError> {
        self.charge_visit()?;
        self.charge_owner_edge()?;
        let full = input.retained_shape();
        self.observe_closure_depth(
            depth
                .checked_add(full.depth)
                .ok_or(PublishedValueError::Limit("closure depth"))?,
        )?;
        // The caller prepaid the new descriptor/iteration/active-state work
        // before borrowing this input or invoking this frame, even on a hit.
        let shape = input.intrinsic_shape();
        if shape.visits > NODE_LIMIT {
            return Err(PublishedValueError::Limit("storage visits"));
        }
        let key = input.storage_identity();
        if self.shared_seen.contains_key(&key) {
            // A paid raw body is never promoted/refunded by a later owner path.
            return Ok(());
        }
        if self.shared_active.contains(&key) {
            return Err(PublishedValueError::Invalid("shared input cycle"));
        }
        // Fixed descriptor credit also covers these capacity/length reads.
        // New active-table growth includes minimum bucket/control allocation
        // and old/new tables together; no allocation precedes this admission.
        let active_capacity = self.shared_active.capacity();
        if self.shared_active.len() == active_capacity {
            let growth = active_capacity
                .checked_add(1)
                .and_then(|n| n.checked_mul(8))
                .and_then(|n| n.checked_mul(std::mem::size_of::<usize>() + 1))
                .and_then(|n| n.checked_add(32 + std::mem::size_of::<fxhash::FxHashSet<usize>>()))
                .ok_or(PublishedValueError::Limit("shared active bytes"))?;
            self.charge_bytes(growth)?;
        }
        // Preserve the existing owning completed-entry forecast. The parent
        // guard remains borrowed until successful postorder insertion.
        self.charge_bytes(std::mem::size_of::<(usize, SharedPublicationInput)>() * 3)?;
        self.shared_active.insert(key);
        let result = (|| {
            let shared_visits = self
                .shared_payload
                .visits
                .checked_add(shape.visits)
                .filter(|facts| *facts <= NODE_LIMIT)
                .ok_or(PublishedValueError::Limit("shared visits"))?;
            let certified_totals = if certified {
                let facts = self
                    .certified_shared_visits
                    .checked_add(shape.visits)
                    .filter(|facts| {
                        self.import_visits
                            .checked_add(self.owner_payload.visits)
                            .and_then(|n| n.checked_add(*facts))
                            .is_some_and(|n| n <= RETAINED_FACT_LIMIT)
                    })
                    .ok_or(PublishedValueError::Limit("shared certificate visits"))?;
                let legacy = self
                    .visits
                    .checked_add(shape.visits)
                    .ok_or(PublishedValueError::Limit("complete visits"))?;
                Some((facts, legacy))
            } else {
                None
            };
            self.charge_storage(crate::DepsetStorageCharge {
                nodes: shape.nodes,
                edges: shape.edges,
                bytes: shape.bytes,
                visits: if certified { 0 } else { shape.visits },
            })
            .map_err(|error| match error {
                PublishedValueError::Limit("visits") => {
                    PublishedValueError::Limit("shared body visits")
                }
                other => other
                    .with_byte_context(PublicationByteContext::SharedBody)
                    .with_edge_context(PublicationEdgeContext::SharedBody),
            })?;
            self.shared_payload.nodes += shape.nodes;
            self.shared_payload.edges += shape.edges;
            self.shared_payload.bytes += shape.bytes;
            self.shared_payload.visits = shared_visits;
            if let Some((facts, legacy)) = certified_totals {
                // Full body storage passed. Pair V/C with the full S commit;
                // no live work or byte credit is refunded on later failure.
                self.certified_shared_visits = facts;
                self.visits = legacy;
            }
            // Commit each admitted positive body before descendants. A later
            // failure keeps all paid charges but cannot publish completion.
            let child_depth = depth
                .checked_add(1)
                .ok_or(PublishedValueError::Limit("closure depth"))?;
            let children = input.shared_inputs();
            let mut index = 0;
            while index < children.len() {
                // Pay the new iteration/descriptor work and the child frame
                // BEFORE the child-handle read or recursive invocation.
                self.charge_visit()?;
                self.charge_bytes(SHARED_FRAME_BYTES)?;
                let child = &children[index];
                self.audit_shared_node(child, child_depth, certified)?;
                index += 1;
            }
            self.shared_seen.insert(key, input.clone());
            Ok(())
        })();
        self.shared_active.remove(&key);
        result
    }
}

#[cfg(test)]
#[path = "shared_input_accounting_tests.rs"]
mod tests;
