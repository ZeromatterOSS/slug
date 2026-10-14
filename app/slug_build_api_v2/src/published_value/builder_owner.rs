//! Cached local owner counts share the arena budget; scratch keys retain owners.
use super::*;
impl PublishedValueArenaBuilder {
    pub fn charge_storage(
        &mut self,
        c: crate::DepsetStorageCharge,
    ) -> Result<(), PublishedValueError> {
        self.charge_bytes(c.bytes)?;
        self.charge_owner_edges(c.edges)?;
        if c.visits > NODE_LIMIT {
            self.failed_budget = true;
            return Err(PublishedValueError::Limit("storage visits"));
        }
        for _ in 0..c.visits {
            self.charge_visit()?;
        }
        self.owner_nodes = self
            .owner_nodes
            .checked_add(c.nodes)
            .ok_or(PublishedValueError::Limit("storage nodes"))?;
        if self
            .rows
            .len()
            .saturating_add(self.owner_nodes)
            .saturating_add(self.imported_nodes)
            > NODE_LIMIT
        {
            self.failed_budget = true;
            return Err(PublishedValueError::Limit("storage nodes"));
        }
        Ok(())
    }
    pub fn local_shape(&self) -> PublicationShape {
        PublicationShape {
            nodes: self.rows.len().saturating_add(self.owner_nodes),
            edges: self
                .edges
                .len()
                .saturating_add(self.inline_edges)
                .saturating_add(self.owner_edges),
            bytes: self.charged_local_bytes(),
            visits: self.visits,
            depth: self.maximum_depth.max(self.closure_depth),
        }
    }
    /// Body accounting excluding declared owners, shared leaves and imported arenas.
    pub fn intrinsic_shape(&self) -> PublicationShape {
        let local = self.local_shape();
        PublicationShape {
            nodes: local.nodes - self.owner_payload.nodes - self.shared_payload.nodes,
            edges: local.edges - self.owner_payload.edges - self.shared_payload.edges,
            bytes: local.bytes - self.owner_payload.bytes - self.shared_payload.bytes,
            visits: local.visits - self.owner_payload.visits - self.shared_payload.visits,
            depth: self.maximum_depth,
        }
    }
    pub(in crate::published_value) fn complete_shape(
        &mut self,
    ) -> Result<PublicationShape, PublishedValueError> {
        let visits = self.visits.checked_add(self.import_visits).ok_or_else(|| {
            self.failed_budget = true;
            PublishedValueError::Limit("complete visits")
        })?;
        Ok(PublicationShape {
            nodes: self.rows.len() + self.owner_nodes + self.imported_nodes,
            edges: self.edges.len() + self.inline_edges + self.owner_edges + self.imported_edges,
            bytes: self.bytes,
            visits,
            depth: self
                .maximum_depth
                .max(self.closure_depth)
                .max(self.import_depth)
                .max(self.retained_root_depth),
        })
    }
    /// Enforce closure depth without folding it into the intrinsic body shape.
    pub fn observe_closure_depth(&mut self, depth: usize) -> Result<(), PublishedValueError> {
        if self.failed_budget || depth > DEPTH_LIMIT {
            self.failed_budget = true;
            return Err(PublishedValueError::Limit("closure depth"));
        }
        self.closure_depth = self.closure_depth.max(depth);
        Ok(())
    }
    pub fn audit_owner_shape(
        &mut self,
        owner: &FunctionEvaluationOwner,
    ) -> Result<(), PublishedValueError> {
        self.audit_owner_shape_at_depth(owner, 0)
    }
    /// Recheck complete depth on every occurrence, including a memo hit.
    pub fn audit_owner_shape_at_depth(
        &mut self,
        owner: &FunctionEvaluationOwner,
        enclosing_depth: usize,
    ) -> Result<(), PublishedValueError> {
        let result = (|| {
            self.walk_owner(owner, enclosing_depth)?;
            self.charge_visit()?;
            if !self.owner_root_seen.contains(&owner.storage_identity()) {
                self.charge_bytes(
                    std::mem::size_of::<FunctionEvaluationOwner>() * 2
                        + std::mem::size_of::<usize>() * 3,
                )?;
                self.charge_owner_edge()?;
                self.owner_root_seen.insert(owner.storage_identity());
                self.retained_root_depth = self
                    .retained_root_depth
                    .max(owner.retained_shape().depth.saturating_add(1));
                self.owner_roots.push(owner.clone());
            }
            Ok(())
        })();
        if result.is_err() {
            self.failed_budget = true;
        }
        result
    }
    pub(super) fn walk_owner(
        &mut self,
        owner: &FunctionEvaluationOwner,
        depth: usize,
    ) -> Result<(), PublishedValueError> {
        self.charge_visit()?;
        self.charge_owner_edge()?;
        self.observe_closure_depth(depth.saturating_add(owner.retained_shape().depth))?;
        let key = owner.storage_identity();
        if self.owner_seen.contains_key(&key) {
            return Ok(());
        }
        if self.owner_active.contains(&key) {
            return Err(PublishedValueError::Invalid("owner arena cycle"));
        }
        // Only the private immutable certificate from a completed Owner::new
        // can use retained admission. Keep actual lookups/children/source paid.
        let shape = owner.intrinsic_shape();
        let next = self
            .owner_payload
            .visits
            .checked_add(shape.visits)
            .zip(self.visits.checked_add(shape.visits))
            .filter(|(owner_visits, _)| {
                self.import_visits
                    .checked_add(*owner_visits)
                    .is_some_and(|cached| cached <= RETAINED_FACT_LIMIT)
            })
            .ok_or_else(|| {
                self.failed_budget = true;
                PublishedValueError::Limit("owner certificate visits")
            })?;
        // The local caller retains the Arc while the bounded recursive frame is
        // active. Completed entries retain independent owning guards.
        self.charge_bytes(std::mem::size_of::<usize>() * 3)?;
        self.owner_active.insert(key);
        self.charge_storage(crate::DepsetStorageCharge {
            nodes: shape.nodes,
            edges: shape.edges,
            bytes: shape.bytes,
            visits: 0,
        })
        .map_err(|error| error.with_byte_context(PublicationByteContext::OwnerBody))?;
        self.owner_payload.nodes += shape.nodes;
        self.owner_payload.edges += shape.edges;
        self.owner_payload.bytes += shape.bytes;
        // Full body storage passed: pair the legacy and retained fact commits.
        // Later descendant errors retain these charges only in a failed builder.
        self.owner_payload.visits = next.0;
        self.visits = next.1;
        for input in owner.shared_inputs() {
            self.audit_shared_input(input, depth.saturating_add(1))?;
        }
        for dependency in owner.retained_owners() {
            self.walk_owner(dependency, depth.saturating_add(1))?;
        }
        for arena in owner.imported_arenas() {
            self.audit_arena_closure(arena, depth.saturating_add(1))?;
            self.register_import(arena)?;
        }
        self.charge_bytes(std::mem::size_of::<(usize, FunctionEvaluationOwner)>() * 3)?;
        self.owner_active.remove(&key);
        self.owner_seen.insert(key, owner.clone());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Debug, allocative::Allocative)]
    struct Part;
    impl StructuralFunctionOwnerPart for Part {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
        fn eq_part(&self, other: &dyn StructuralFunctionOwnerPart) -> bool {
            other.as_any().is::<Self>()
        }
        fn hash_part(&self, state: &mut dyn std::hash::Hasher) {
            state.write_u8(0)
        }
        fn retained_bytes(&self) -> usize {
            1
        }
    }
    #[test]
    fn refused_owner_quota_does_not_insert_owning_memo_entry() {
        let owner = FunctionEvaluationOwner::new(
            FunctionOwnerDomain::ConfiguredRule,
            Arc::new(Part),
            Arc::new(Part),
        )
        .unwrap();
        let mut b = PublishedValueArenaBuilder::new();
        b.charge_bytes(BYTE_LIMIT).unwrap();
        assert!(b.audit_owner_shape(&owner).is_err());
        assert!(b.owner_seen.is_empty());
    }
}

#[cfg(test)]
#[path = "owner_certificate_tests.rs"]
mod certificate_tests;
