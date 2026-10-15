use super::rows::NativeCertificate;
use super::*;
use crate::AnalysisValue;
use crate::FilesToRunProvider;

#[derive(Debug, Default)]
pub struct PublishedValueArenaBuilder {
    rows: Vec<Option<PublishedNode>>,
    edges: Vec<PublishedEdge>,
    imports: Vec<PublishedValueArena>,
    import_lookup: fxhash::FxHashMap<PublishedArenaIdentity, PublishedImportId>,
    slots: Vec<Option<PublishedRootSlot>>,
    bytes: usize,
    // Classified subset of local construction bytes; never a live-budget refund.
    producer_scratch_bytes: usize,
    failed_budget: bool,
    // Legacy total includes producer work and full certified owner/shared facts.
    visits: usize,
    // Subset of shared_payload.visits whose replay was certified by Owner::new.
    certified_shared_visits: usize,
    strict_shared_audit: bool,
    pub(super) owner_audit: bool,
    inline_edges: usize,
    owner_edges: usize,
    owner_nodes: usize,
    maximum_depth: usize,
    closure_depth: usize,
    retained_root_depth: usize,
    owner_payload: PublicationShape,
    shared_payload: PublicationShape,
    shared_seen: fxhash::FxHashMap<usize, SharedPublicationInput>,
    // Addresses remain guarded by the exact borrowed root/parent until the
    // postorder owning memo commits. Never a semantic identity or certificate.
    shared_active: fxhash::FxHashSet<usize>,
    owner_roots: Vec<FunctionEvaluationOwner>,
    owner_root_seen: fxhash::FxHashSet<usize>,
    owner_active: fxhash::FxHashSet<usize>,
    arena_active: fxhash::FxHashSet<PublishedArenaIdentity>,
    import_depth: usize,
    owner_seen: fxhash::FxHashMap<usize, FunctionEvaluationOwner>,
    source_seen: fxhash::FxHashSet<usize>,
    sources: Vec<CallableSourceAuthority>,
    closure_seen: fxhash::FxHashSet<PublishedArenaIdentity>,
    imported_nodes: usize,
    imported_edges: usize,
    import_bytes: usize,
    import_visits: usize,
    feature_models: feature_proof::FeatureModelAudits,
    source_preparations: source_preparation::SourcePreparations,
}
impl PublishedValueArenaBuilder {
    pub fn owners(&self) -> &[FunctionEvaluationOwner] {
        &self.owner_roots
    }
    pub fn charged_local_bytes(&self) -> usize {
        self.bytes - self.import_bytes
    }
    pub fn charged_bytes(&self) -> usize {
        self.bytes
    }
    /// Producer work, excluding successfully certified owner/shared body facts.
    pub fn charged_visits(&self) -> usize {
        self.checked_producer_work()
            .expect("certified body visits are included in the legacy total")
    }
    fn checked_producer_work(&self) -> Option<usize> {
        if self.certified_shared_visits > self.shared_payload.visits {
            return None;
        }
        self.visits
            .checked_sub(self.owner_payload.visits)
            .and_then(|work| work.checked_sub(self.certified_shared_visits))
    }
    fn checked_cached_visits(&self) -> Option<usize> {
        self.import_visits
            .checked_add(self.owner_payload.visits)
            .and_then(|facts| facts.checked_add(self.certified_shared_visits))
    }
    pub fn new() -> Self {
        Self::default()
    }
    pub(in crate::published_value) fn new_strict_shared_audit() -> Self {
        Self {
            strict_shared_audit: true,
            ..Self::default()
        }
    }
    pub fn node(&self, id: PublishedNodeId) -> Result<&PublishedNode, PublishedValueError> {
        self.rows
            .get(id.index())
            .and_then(Option::as_ref)
            .ok_or(PublishedValueError::Invalid("incomplete node"))
    }
    pub fn value_type(
        &self,
        id: PublishedNodeId,
    ) -> Result<AnalysisValueType, PublishedValueError> {
        self.node(id).map(projection::value_type)
    }
    pub fn provider_value_atom_borrowed(
        &mut self,
        value: &crate::ProviderValue,
    ) -> Result<PurePublishedProviderValue, PublishedValueError> {
        let certificate = self.strict_pure_certificate(|b| native::provider_value(b, value, 0))?;
        Ok(PurePublishedProviderValue(value.clone(), certificate))
    }
    pub fn native_atom_borrowed(
        &mut self,
        value: &AnalysisValue,
    ) -> Result<PurePublishedAtom, PublishedValueError> {
        let certificate = self.native_root_certificate(value)?;
        Ok(PurePublishedAtom(value.clone(), certificate))
    }
    pub fn imports(&self) -> &[PublishedValueArena] {
        &self.imports
    }
    pub fn charge_owner_edge(&mut self) -> Result<(), PublishedValueError> {
        self.charge_owner_edges(1)
    }
    pub fn charge_owner_edges(&mut self, count: usize) -> Result<(), PublishedValueError> {
        let next = self.owner_edges.checked_add(count).filter(|n| {
            n.saturating_add(self.edges.len())
                .saturating_add(self.inline_edges)
                .saturating_add(self.imported_edges)
                <= EDGE_LIMIT
        });
        match next {
            Some(next) if !self.failed_budget => {
                self.owner_edges = next;
                Ok(())
            }
            _ => {
                let prior_poisoned = self.failed_budget;
                self.failed_budget = true;
                Err(self.edge_refusal(count, prior_poisoned))
            }
        }
    }
    pub fn charge_visit(&mut self) -> Result<(), PublishedValueError> {
        let next = self
            .checked_producer_work()
            .and_then(|work| work.checked_add(1))
            .filter(|work| *work <= NODE_LIMIT)
            .and_then(|_| self.visits.checked_add(1));
        match next {
            Some(n) if !self.failed_budget => {
                self.visits = n;
                Ok(())
            }
            _ => {
                self.failed_budget = true;
                Err(PublishedValueError::Limit("visits"))
            }
        }
    }
    pub fn charge_bytes(&mut self, n: usize) -> Result<(), PublishedValueError> {
        match self.bytes.checked_add(n).filter(|n| *n <= BYTE_LIMIT) {
            Some(n) if !self.failed_budget => {
                self.bytes = n;
                Ok(())
            }
            _ => {
                let prior_poisoned = self.failed_budget;
                self.failed_budget = true;
                Err(self.byte_refusal(n, prior_poisoned))
            }
        }
    }
    pub fn observe_depth(&mut self, n: usize) -> Result<(), PublishedValueError> {
        if n > DEPTH_LIMIT || self.failed_budget {
            self.failed_budget = true;
            Err(PublishedValueError::Limit("depth"))
        } else {
            self.maximum_depth = self.maximum_depth.max(n);
            Ok(())
        }
    }
    pub fn reserve(&mut self) -> Result<PublishedNodeId, PublishedValueError> {
        self.reserve_impl(true)
    }
    fn reserve_impl(&mut self, pay_visit: bool) -> Result<PublishedNodeId, PublishedValueError> {
        if self
            .rows
            .len()
            .saturating_add(self.imported_nodes)
            .saturating_add(self.owner_nodes)
            >= NODE_LIMIT
        {
            self.failed_budget = true;
            return Err(PublishedValueError::Limit("nodes"));
        }
        let id = PublishedNodeId::new(self.rows.len())?;
        if pay_visit {
            self.charge_visit()?;
        }
        self.charge_bytes(
            std::mem::size_of::<PublishedNode>()
                + std::mem::size_of::<Option<PublishedRootSlot>>()
                + crate::analysis_value::published_projection_forecast(),
        )?;
        self.rows.push(None);
        self.slots.push(None);
        Ok(id)
    }
    fn check_edge(&self, edge: PublishedEdge) -> Result<(), PublishedValueError> {
        match edge {
            PublishedEdge::Local(id) if id.index() < self.rows.len() => Ok(()),
            PublishedEdge::Imported { arena, node } => self
                .imports
                .get(arena.index())
                .ok_or(PublishedValueError::Invalid("import"))?
                .node(node)
                .map(|_| ()),
            _ => Err(PublishedValueError::Invalid("local edge")),
        }
    }
    pub fn append_edges(
        &mut self,
        edges: &[PublishedEdge],
    ) -> Result<PublishedEdgeRange, PublishedValueError> {
        let end = self
            .edges
            .len()
            .checked_add(edges.len())
            .ok_or(PublishedValueError::Limit("edges"))?;
        if end
            .saturating_add(self.inline_edges)
            .saturating_add(self.imported_edges)
            .saturating_add(self.owner_edges)
            > EDGE_LIMIT
        {
            self.failed_budget = true;
            return Err(PublishedValueError::Limit("edges"));
        }
        for edge in edges {
            self.check_edge(*edge)?;
        }
        self.charge_bytes(
            edges
                .len()
                .checked_mul(std::mem::size_of::<PublishedEdge>())
                .ok_or(PublishedValueError::Limit("bytes"))?,
        )?;
        let range = PublishedEdgeRange {
            start: self.edges.len() as u32,
            len: edges.len() as u32,
        };
        self.edges.extend_from_slice(edges);
        Ok(range)
    }
    pub fn set_node(
        &mut self,
        id: PublishedNodeId,
        row: PublishedNode,
    ) -> Result<(), PublishedValueError> {
        if self.rows.get(id.index()).is_none_or(Option::is_some) {
            return Err(PublishedValueError::Invalid("initialized node"));
        }
        let (bytes, edges) =
            match projection::inspect_row(&row, &self.edges, |edge| self.check_edge(edge)) {
                Ok(cost) => cost,
                Err(e) => {
                    if e.is_limit() {
                        self.failed_budget = true;
                    }
                    return Err(e);
                }
            };
        if self
            .inline_edges
            .saturating_add(edges)
            .saturating_add(self.edges.len())
            .saturating_add(self.imported_edges)
            .saturating_add(self.owner_edges)
            > EDGE_LIMIT
        {
            self.failed_budget = true;
            return Err(PublishedValueError::Limit("edges"));
        }
        self.charge_bytes(bytes)?;
        let code = match &row {
            PublishedNode::Function(f) => Some(&f.code),
            PublishedNode::Code(c) => Some(c),
            _ => None,
        };
        if let Some(code) = code {
            if !self.source_seen.contains(&code.source.storage_identity()) {
                let prepared = self.prepared_source_audit(&code.source)?.is_some();
                if !prepared {
                    self.charge_bytes(code.source.byte_cost())?;
                }
                self.charge_bytes(
                    std::mem::size_of::<CallableSourceAuthority>().saturating_mul(4),
                )?;
                // Only an exact same-builder completed native audit replaces
                // this logical-fact replay. Seal still retains the FULL facts.
                if !prepared {
                    for _ in 0..code.source.fact_count() {
                        self.charge_visit()?;
                    }
                }
                self.source_seen.insert(code.source.storage_identity());
                self.sources.push(code.source.clone());
            }
        }
        if let PublishedNode::Function(f) = &row {
            self.audit_owner_shape(f.symbol.owner())?;
        }
        self.inline_edges += edges;
        self.rows[id.index()] = Some(row);
        Ok(())
    }
    pub fn audit_owner_native(&mut self, value: &AnalysisValue) -> Result<(), PublishedValueError> {
        let prior = self.owner_audit;
        self.owner_audit = true;
        let result = native::audit(self, value, 0);
        self.owner_audit = prior;
        result
    }
    /// Audit a complete borrowed target before copying its identity or providers.
    /// The enclosing depth and ordinary native-value header are charged as for
    /// an already-wrapped target; errors poison the unpublished builder.
    pub fn audit_owner_configured_target_at_depth(
        &mut self,
        value: &crate::ConfiguredTargetValue,
        depth: usize,
    ) -> Result<(), PublishedValueError> {
        let prior = self.owner_audit;
        self.owner_audit = true;
        let result = native::audit_configured_target(self, value, depth);
        self.owner_audit = prior;
        if result.is_err() {
            self.failed_budget = true;
        }
        result
    }
    /// Audit the complete borrowed membership graph before cloning its handle.
    pub fn audit_package_specification_at_depth(
        &mut self,
        value: &crate::PackageSpecificationInfo,
        depth: usize,
    ) -> Result<(), PublishedValueError> {
        let result = native::audit_package_specification(self, value, depth);
        if result.is_err() {
            self.failed_budget = true;
        }
        result
    }
    /// Audit an actual immutable shared-input graph before retaining its guard.
    /// This raw entry pays full body work and never supplies an owner certificate.
    /// Depth, memo hits, resource refusal and poisoning use the existing walker.
    pub fn audit_shared_input_at_depth(
        &mut self,
        input: &SharedPublicationInput,
        depth: usize,
    ) -> Result<(), PublishedValueError> {
        self.audit_shared_input(input, depth)
    }
    /// Admit the complete immutable version before the VM bridge clones it.
    pub fn audit_dotted_version_at_depth(
        &mut self,
        value: &crate::DottedVersion,
        depth: usize,
    ) -> Result<(), PublishedValueError> {
        let result = native::audit_dotted_version(self, value, depth);
        if result.is_err() {
            self.failed_budget = true;
        }
        result
    }
    pub fn audit_owner_files_to_run(
        &mut self,
        value: &FilesToRunProvider,
    ) -> Result<(), PublishedValueError> {
        let prior = self.owner_audit;
        self.owner_audit = true;
        let result = native::files_to_run(self, value, 0);
        self.owner_audit = prior;
        result
    }
    pub fn audit_owner_provider_occurrence(
        &mut self,
        value: &crate::ProviderOccurrence,
    ) -> Result<(), PublishedValueError> {
        let prior = self.owner_audit;
        self.owner_audit = true;
        let result = native::audit_provider_occurrence(self, value, 0);
        self.owner_audit = prior;
        result
    }
    pub fn audit_owner_provider_value(
        &mut self,
        value: &crate::ProviderValue,
    ) -> Result<(), PublishedValueError> {
        let prior = self.owner_audit;
        self.owner_audit = true;
        let result = native::provider_value(self, value, 0);
        self.owner_audit = prior;
        result
    }
    pub fn audit_native(&mut self, value: &AnalysisValue) -> Result<(), PublishedValueError> {
        native::audit(self, value, 0)
    }
    pub fn audit_feature_model(
        &mut self,
        value: &crate::cc_features::CcToolchainFeatureModel,
    ) -> Result<(), PublishedValueError> {
        native::features(self, value, 0)
    }
    pub fn audit_provider_value(
        &mut self,
        value: &crate::ProviderValue,
    ) -> Result<(), PublishedValueError> {
        native::provider_value(self, value, 0)
    }
    pub fn provider_value_atom(
        &mut self,
        value: crate::ProviderValue,
    ) -> Result<PurePublishedProviderValue, PublishedValueError> {
        let certificate = self.strict_pure_certificate(|b| native::provider_value(b, &value, 0))?;
        Ok(PurePublishedProviderValue(value, certificate))
    }
    pub fn native_atom(
        &mut self,
        value: AnalysisValue,
    ) -> Result<PurePublishedAtom, PublishedValueError> {
        let certificate = self.native_root_certificate(&value)?;
        Ok(PurePublishedAtom(value, certificate))
    }
    /// Strictly audit a borrowed retained typed DAG before allocating its native
    /// wrapper or cloning its store guard. This fallback does not hide exposed
    /// original leaves: callers use typed graph rows whenever successors exist.
    pub fn depset_atom_borrowed(
        &mut self,
        value: &crate::AnalysisDepset,
    ) -> Result<PurePublishedAtom, PublishedValueError> {
        let certificate = self.strict_pure_certificate(|b| {
            // Covers the new AnalysisValue allocation and Arc headers before
            // constructing it; shared retained DAG storage is audited below.
            b.charge_bytes(crate::analysis_value::published_projection_forecast())?;
            native::audit_depset_borrowed(b, value)
        })?;
        Ok(PurePublishedAtom(
            AnalysisValue::depset(value.clone()),
            certificate,
        ))
    }
    pub fn files_to_run_atom_borrowed(
        &mut self,
        value: &FilesToRunProvider,
    ) -> Result<PurePublishedFilesToRun, PublishedValueError> {
        let certificate = self.strict_pure_certificate(|b| native::files_to_run(b, value, 0))?;
        Ok(PurePublishedFilesToRun(value.clone(), certificate))
    }
    pub fn files_to_run_atom(
        &mut self,
        value: FilesToRunProvider,
    ) -> Result<PurePublishedFilesToRun, PublishedValueError> {
        let certificate = self.strict_pure_certificate(|b| native::files_to_run(b, &value, 0))?;
        Ok(PurePublishedFilesToRun(value, certificate))
    }
    fn strict_pure_certificate(
        &mut self,
        audit: impl FnOnce(&mut Self) -> Result<(), PublishedValueError>,
    ) -> Result<NativeCertificate, PublishedValueError> {
        let prior = self.owner_audit;
        self.owner_audit = false;
        let prior_depth = std::mem::replace(&mut self.maximum_depth, 0);
        let before = self.visits;
        let result = (|| {
            // Both private certificate fields are retained wherever the wrapper is
            // embedded; admit it before the owned move or borrowed clone.
            self.charge_bytes(std::mem::size_of::<NativeCertificate>())?;
            audit(self)
        })();
        let ordinary_depth = self.maximum_depth;
        self.maximum_depth = prior_depth.max(ordinary_depth);
        self.owner_audit = prior;
        match result {
            Ok(()) => Ok(NativeCertificate {
                visits: self.visits - before,
                ordinary_depth,
            }),
            Err(error) => {
                self.failed_budget = true;
                Err(error)
            }
        }
    }

    pub fn set_original_slot(
        &mut self,
        id: PublishedNodeId,
        slot: PublishedRootSlot,
    ) -> Result<(), PublishedValueError> {
        let target = self
            .slots
            .get_mut(id.index())
            .ok_or(PublishedValueError::Invalid("slot node"))?;
        if target.is_some() {
            return Err(PublishedValueError::Invalid("duplicate original slot"));
        }
        *target = Some(slot);
        Ok(())
    }
    pub fn seal(
        mut self,
        originals: Arc<dyn PublishedOriginalBinding>,
    ) -> Result<PublishedValueArena, PublishedValueError> {
        // The immutable outer allocation is independent of forecast row and
        // reference arrays. Charge before converting or publishing any storage.
        self.charge_bytes(
            std::mem::size_of::<PublishedValueArenaData>() + 2 * std::mem::size_of::<usize>(),
        )?;
        if self.complete_shape()?.depth > DEPTH_LIMIT {
            return Err(PublishedValueError::Limit("sealed closure depth"));
        }
        let mut work = self.checked_producer_work().ok_or_else(|| {
            self.failed_budget = true;
            PublishedValueError::Limit("producer visits")
        })?;
        let cached = self.checked_cached_visits().ok_or_else(|| {
            self.failed_budget = true;
            PublishedValueError::Limit("cached visits")
        })?;
        if self.failed_budget
            || self.bytes > BYTE_LIMIT
            || work > NODE_LIMIT
            || cached > RETAINED_FACT_LIMIT
            || self.shared_payload.visits > NODE_LIMIT
            || self
                .rows
                .len()
                .saturating_add(self.imported_nodes)
                .saturating_add(self.owner_nodes)
                > NODE_LIMIT
            || self
                .edges
                .len()
                .saturating_add(self.inline_edges)
                .saturating_add(self.imported_edges)
                .saturating_add(self.owner_edges)
                > EDGE_LIMIT
        {
            return Err(PublishedValueError::Limit("poisoned publication budget"));
        }
        if originals.root_count() > NODE_LIMIT
            || originals.imported_arenas().len() != self.imports.len()
            || originals
                .imported_arenas()
                .iter()
                .zip(&self.imports)
                .any(|(a, b)| !Arc::ptr_eq(&a.0, &b.0))
        {
            return Err(PublishedValueError::Invalid("original imports"));
        }
        // Precharge every simultaneously retained validator array before any
        // allocation, in the same ledger as canonical sources/final seal arrays.
        let scratch = projection::depth_scratch_bytes(&self.rows, &self.edges)?;
        self.producer_scratch().charge_bytes(scratch)?;
        // Inspect the borrowed, fully assembled draft before any final row/edge
        // conversion. Loan only producer work; cached owner facts remain paid
        // on the separate retained dimension. This consumes the builder on Err.
        let (local_facts, sources) = self.certify_local(originals.as_ref(), &mut work)?;
        // Restore the legacy total before any shape/certificate is observed.
        self.visits = work
            .checked_add(self.owner_payload.visits)
            .and_then(|visits| visits.checked_add(self.certified_shared_visits))
            .ok_or_else(|| {
                self.failed_budget = true;
                PublishedValueError::Limit("complete visits")
            })?;
        let mut intrinsic_shape = self.intrinsic_shape();
        let mut complete_shape = self.complete_shape()?;
        // Canonical local facts replace producer history. Prior sealed bodies
        // and opaque owner/shared payloads still contribute their FULL counts.
        let checked_total = retained_fact_refusal::checked_total(
            self.import_visits,
            self.owner_payload.visits,
            self.shared_payload.visits,
            local_facts.total,
        );
        complete_shape.visits = checked_total
            .filter(|visits| *visits <= RETAINED_FACT_LIMIT)
            .ok_or_else(|| {
                self.failed_budget = true;
                self.retained_fact_refusal(local_facts, work, checked_total)
            })?;
        intrinsic_shape.visits = local_facts.total;
        let rows = self
            .rows
            .into_iter()
            .map(|row| row.ok_or(PublishedValueError::Invalid("incomplete row")))
            .collect::<Result<Box<[_]>, _>>()?;
        for (row, slot) in rows.iter().zip(&self.slots) {
            if slot.is_some_and(|s| s.index() >= originals.root_count()) {
                return Err(PublishedValueError::Invalid("original slot"));
            }
            if !matches!(row, PublishedNode::Code(_) | PublishedNode::Cell { .. }) && slot.is_none()
            {
                return Err(PublishedValueError::Invalid("missing original slot"));
            }
        }
        let (intrinsic_depth, complete_depth) =
            projection::validate_complete(&rows, &self.edges, &self.imports)?;
        intrinsic_shape.depth = intrinsic_shape.depth.max(intrinsic_depth);
        complete_shape.depth = complete_shape.depth.max(complete_depth);
        // Host maps dropped at producer finish, and validators above returned
        // only scalar heights. Keep their full charges in live B, excluding only
        // the proven local scratch subset from these immutable certificates.
        let retained_bytes = intrinsic_shape
            .bytes
            .checked_sub(self.producer_scratch_bytes)
            .zip(
                complete_shape
                    .bytes
                    .checked_sub(self.producer_scratch_bytes),
            )
            .ok_or_else(|| {
                self.failed_budget = true;
                PublishedValueError::Limit("producer scratch certificate")
            })?;
        intrinsic_shape.bytes = retained_bytes.0;
        complete_shape.bytes = retained_bytes.1;
        // Producer-only proof guards/backing end before the outward allocation.
        drop(self.source_preparations);
        Ok(PublishedValueArena(Arc::new(PublishedValueArenaData {
            shape: complete_shape,
            local_shape: intrinsic_shape,
            owners: self.owner_roots.into(),
            rows,
            edges: self.edges.into_boxed_slice(),
            imports: self.imports.into(),
            originals,
            original_slots: self.slots.into_boxed_slice(),
            sources: sources.into(),
        })))
    }
}

#[path = "builder_owner.rs"]
mod owner_accounting;

#[path = "builder_import.rs"]
mod import_accounting;

#[path = "shared_input_accounting.rs"]
mod shared_input_accounting;

#[path = "retained_facts.rs"]
mod retained_facts;

#[cfg(test)]
#[path = "depset_factory_tests.rs"]
mod depset_factory_tests;

#[cfg(test)]
#[path = "native_depth_tests.rs"]
mod native_depth_tests;

#[path = "feature_proof.rs"]
mod feature_proof;

#[path = "record.rs"]
mod record;
pub use record::PublicationRecord;

#[path = "scratch.rs"]
mod scratch;
pub use scratch::PublicationScratch;

#[path = "byte_refusal.rs"]
mod byte_refusal;
pub use byte_refusal::PublicationByteContext;
pub use byte_refusal::PublicationByteRefusal;

#[path = "retained_work_refusal.rs"]
mod retained_work_refusal;
pub use retained_work_refusal::PublicationRetainedCertificateWorkRefusal;

#[path = "retained_fact_refusal.rs"]
mod retained_fact_refusal;
pub use retained_fact_refusal::PublicationRetainedFactRefusal;

#[path = "owner_retained_visit_refusal.rs"]
mod owner_retained_visit_refusal;
pub use owner_retained_visit_refusal::PublicationClosureCensus;
pub use owner_retained_visit_refusal::PublicationNativeChannel;
pub use owner_retained_visit_refusal::PublicationNativeRetentionCensus;
pub use owner_retained_visit_refusal::PublicationNativeRootFamily;
pub use owner_retained_visit_refusal::PublicationNativeStatistics;
pub use owner_retained_visit_refusal::PublicationOwnerDomainCensus;
pub use owner_retained_visit_refusal::PublicationOwnerRetainedVisitRefusal;
pub use owner_retained_visit_refusal::PublicationRowKind;

#[path = "edge_refusal.rs"]
mod edge_refusal;
pub use edge_refusal::PublicationEdgeContext;
pub use edge_refusal::PublicationEdgeRefusal;

#[path = "source_preparation.rs"]
mod source_preparation;
pub use source_preparation::PublicationSourcePreparation;

#[path = "builder/checked_draft.rs"]
pub(in crate::published_value) mod checked_draft;

#[path = "builder/local_certification.rs"]
mod local_certification;
