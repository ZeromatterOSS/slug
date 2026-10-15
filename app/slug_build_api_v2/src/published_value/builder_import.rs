//! One bounded immutable owner/arena retention graph. Identity keys own guards.
use super::*;
impl PublishedValueArenaBuilder {
    pub fn import(
        &mut self,
        arena: PublishedValueArena,
    ) -> Result<PublishedImportId, PublishedValueError> {
        let result = (|| {
            self.audit_arena_closure(&arena, 0)?;
            self.register_import(&arena)
        })();
        if result.is_err() {
            self.failed_budget = true;
        }
        result
    }
    pub(super) fn register_import(
        &mut self,
        arena: &PublishedValueArena,
    ) -> Result<PublishedImportId, PublishedValueError> {
        self.charge_visit()?;
        let identity = arena.identity();
        if let Some(id) = self.import_lookup.get(&identity) {
            return Ok(*id);
        }
        self.charge_bytes(
            std::mem::size_of::<PublishedValueArena>() * 2
                + std::mem::size_of::<(PublishedArenaIdentity, PublishedImportId)>() * 3,
        )?;
        self.charge_owner_edge()?;
        let id = PublishedImportId::new(self.imports.len())?;
        self.import_lookup.insert(identity, id);
        self.retained_root_depth = self
            .retained_root_depth
            .max(arena.shape().depth.saturating_add(1));
        self.imports.push(arena.clone());
        Ok(id)
    }
    pub(in crate::published_value) fn audit_arena_closure(
        &mut self,
        arena: &PublishedValueArena,
        depth: usize,
    ) -> Result<(), PublishedValueError> {
        self.charge_visit()?;
        self.charge_owner_edge()?;
        self.observe_closure_depth(depth.saturating_add(arena.shape().depth))?;
        let key = arena.identity();
        if self.closure_seen.contains(&key) {
            return Ok(());
        }
        if self.arena_active.contains(&key) {
            return Err(PublishedValueError::Invalid("owner arena cycle"));
        }
        // This private count was certified by the completed prior seal. It is
        // retained closure data, not another traversal of that immutable body.
        let shape = arena.0.local_shape;
        let next_import_visits = self
            .import_visits
            .checked_add(shape.visits)
            .filter(|visits| {
                visits
                    .checked_add(self.owner_payload.visits)
                    .and_then(|facts| facts.checked_add(self.certified_shared_visits))
                    .is_some_and(|cached| cached <= RETAINED_FACT_LIMIT)
            })
            .ok_or_else(|| {
                self.failed_budget = true;
                PublishedValueError::Limit("import visits")
            })?;
        self.charge_bytes(std::mem::size_of::<PublishedArenaIdentity>() * 3)?;
        self.arena_active.insert(key.clone());
        self.charge_storage(crate::DepsetStorageCharge {
            nodes: shape.nodes,
            edges: shape.edges,
            bytes: shape.bytes,
            visits: 0,
        })
        .map_err(|error| {
            error
                .with_byte_context(PublicationByteContext::ImportBody)
                .with_edge_context(PublicationEdgeContext::ImportBody)
        })?;
        self.owner_nodes -= shape.nodes;
        self.owner_edges -= shape.edges;
        self.imported_nodes += shape.nodes;
        self.imported_edges += shape.edges;
        self.import_bytes += shape.bytes;
        // Commit only after the complete body storage admission. A later
        // descendant failure poisons the builder; accepted charges stay paid.
        self.import_visits = next_import_visits;
        self.import_depth = self.import_depth.max(depth.saturating_add(shape.depth));
        for owner in arena.owners() {
            self.walk_owner(owner, depth.saturating_add(1))?;
        }
        for imported in arena.imports() {
            self.audit_arena_closure(imported, depth.saturating_add(1))?;
        }
        for source in arena.sources() {
            self.charge_visit()?;
            if !self.source_seen.contains(&source.storage_identity()) {
                self.charge_bytes(std::mem::size_of::<CallableSourceAuthority>() * 4)?;
                self.source_seen.insert(source.storage_identity());
                self.sources.push(source.clone());
            }
        }
        self.charge_bytes(std::mem::size_of::<PublishedArenaIdentity>() * 3)?;
        self.arena_active.remove(&key);
        self.closure_seen.insert(key);
        Ok(())
    }
}

#[cfg(test)]
#[path = "sealed_import_tests.rs"]
pub(super) mod tests;
