use super::*;
impl PublishedValueArenaBuilder {
    // Only declared owner facts are audited: no standalone fact can escape a
    // sealed arena without the actual owner retaining its leaf allocation.
    pub(in crate::published_value) fn audit_shared_input(
        &mut self,
        input: &SharedPublicationInput,
        depth: usize,
    ) -> Result<(), PublishedValueError> {
        self.charge_visit()?;
        self.charge_owner_edge()?;
        let shape = input.retained_shape();
        self.observe_closure_depth(depth.saturating_add(shape.depth))?;
        let key = input.storage_identity();
        if self.shared_seen.contains_key(&key) {
            return Ok(());
        }
        self.charge_storage(crate::DepsetStorageCharge {
            nodes: shape.nodes,
            edges: shape.edges,
            bytes: shape.bytes,
            visits: shape.visits,
        })
        .map_err(|error| match error {
            PublishedValueError::Limit("visits") => {
                PublishedValueError::Limit("shared body visits")
            }
            other => other.with_byte_context(PublicationByteContext::SharedBody),
        })?;
        self.shared_payload.nodes += shape.nodes;
        self.shared_payload.edges += shape.edges;
        self.shared_payload.bytes += shape.bytes;
        self.shared_payload.visits += shape.visits;
        self.charge_bytes(std::mem::size_of::<(usize, SharedPublicationInput)>() * 3)?;
        self.shared_seen.insert(key, input.clone());
        Ok(())
    }
}

#[cfg(test)]
#[path = "shared_input_accounting_tests.rs"]
mod tests;
