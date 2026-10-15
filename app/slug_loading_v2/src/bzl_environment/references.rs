//! Lossless first-occurrence environment view; pointer identities are scratch only.
use std::hash::Hash;
use std::hash::Hasher;
use std::mem::size_of;

use starlark_map::small_map::SmallMap;

use super::*;
use crate::bzl_module::semantic_evaluation::budget::admit_peak;
use crate::bzl_module::semantic_evaluation::budget::limit;
use crate::bzl_module::semantic_evaluation::budget::peak_context::FirstLedger;
use crate::bzl_module::semantic_evaluation::budget::peak_context::PeakContext;
use crate::bzl_module::semantic_evaluation::budget::peak_context::PeakOwner;
use crate::bzl_module::semantic_evaluation::budget::peak_context::PeakPhase;

mod descriptor_comparison;

#[derive(Debug, Clone, Allocative)]
pub(crate) enum Payload {
    Catalog(Arc<catalog::BuiltinSourceCatalog>),
    Sources(Arc<super::source_facts::EnvironmentSourceFacts>),
}
#[derive(Clone, Copy)]
pub(crate) enum PayloadRef<'a> {
    Catalog(&'a Arc<catalog::BuiltinSourceCatalog>),
    Sources(&'a Arc<super::source_facts::EnvironmentSourceFacts>),
}
impl PayloadRef<'_> {
    pub(crate) fn pointer(self) -> (u8, usize) {
        match self {
            Self::Catalog(v) => (0, Arc::as_ptr(v) as usize),
            Self::Sources(v) => (1, Arc::as_ptr(v) as usize),
        }
    }
    pub(crate) fn budget(self) -> Budget {
        match self {
            Self::Catalog(v) => v.budget,
            Self::Sources(v) => v.budget(),
        }
    }
    pub(crate) fn physical_budget(self) -> Budget {
        match self {
            Self::Catalog(v) => v.budget,
            Self::Sources(v) => v.physical_budget(),
        }
    }
    /// Clones only after the caller's complete declaration admission.
    pub(crate) fn shared(self) -> slug_build_api_v2::published_value::SharedPublicationInput {
        use slug_build_api_v2::published_value::SharedPublicationInput;
        match self {
            Self::Catalog(v) => SharedPublicationInput::new(v.clone()),
            Self::Sources(v) => SharedPublicationInput::new(v.clone()),
        }
    }
    fn retain(self) -> Payload {
        match self {
            Self::Catalog(v) => Payload::Catalog(v.clone()),
            Self::Sources(v) => Payload::Sources(v.clone()),
        }
    }
}
impl PartialEq for PayloadRef<'_> {
    fn eq(&self, o: &Self) -> bool {
        match (self, o) {
            (Self::Catalog(a), Self::Catalog(b)) => a == b,
            (Self::Sources(a), Self::Sources(b)) => a == b,
            _ => false,
        }
    }
}
impl Eq for PayloadRef<'_> {}
impl Hash for PayloadRef<'_> {
    fn hash<H: Hasher>(&self, h: &mut H) {
        match self {
            Self::Catalog(v) => {
                0u8.hash(h);
                v.hash(h)
            }
            Self::Sources(v) => {
                1u8.hash(h);
                v.hash(h)
            }
        }
    }
}
impl Payload {
    fn borrowed(&self) -> PayloadRef<'_> {
        match self {
            Self::Catalog(v) => PayloadRef::Catalog(v),
            Self::Sources(v) => PayloadRef::Sources(v),
        }
    }
    pub(crate) fn budget(&self) -> Budget {
        self.borrowed().budget()
    }
    pub(crate) fn physical_budget(&self) -> Budget {
        self.borrowed().physical_budget()
    }
    /// Cached native closure, borrowed only after caller prepays the read.
    pub(crate) fn publication_shape(&self) -> slug_build_api_v2::published_value::PublicationShape {
        use slug_build_api_v2::published_value::ImmutablePublicationInput;
        match self {
            Self::Catalog(c) => c.retained_shape(),
            Self::Sources(p) => p.retained_shape(),
        }
    }
    pub(crate) fn shared_count(&self) -> usize {
        match self {
            Self::Catalog(_) => 1,
            Self::Sources(v) => 1usize.saturating_add(v.nested_payloads().len()),
        }
    }
    pub(crate) fn append_shared(
        &self,
        out: &mut Vec<slug_build_api_v2::published_value::SharedPublicationInput>,
    ) {
        out.push(self.shared());
        if let Self::Sources(v) = self {
            for nested in v.nested_payloads() {
                out.push(nested.shared());
            }
        }
    }
    pub(crate) fn shared(&self) -> slug_build_api_v2::published_value::SharedPublicationInput {
        use slug_build_api_v2::published_value::SharedPublicationInput;
        match self {
            Self::Catalog(v) => SharedPublicationInput::new(v.clone()),
            Self::Sources(v) => SharedPublicationInput::new(v.clone()),
        }
    }
}
impl PartialEq for Payload {
    fn eq(&self, o: &Self) -> bool {
        self.borrowed() == o.borrowed()
    }
}
impl Eq for Payload {}
impl Hash for Payload {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.borrowed().hash(h)
    }
}
#[derive(Debug, Allocative)]
struct Descriptor {
    environment: Arc<BzlEnvironmentInputs>,
    payload: Option<usize>,
}
impl PartialEq for Descriptor {
    fn eq(&self, o: &Self) -> bool {
        self.payload == o.payload && self.environment.descriptor_eq(&o.environment)
    }
}
impl Eq for Descriptor {}
impl Hash for Descriptor {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.payload.hash(h);
        self.environment.hash_descriptor(h);
    }
}
#[derive(Debug, Allocative)]
pub(crate) struct EnvironmentReferencePlan {
    payloads: Arc<[Payload]>,
    descriptors: Arc<[Descriptor]>,
    ordinals: Arc<[usize]>,
    // Foreign-equal storage is still physically owned by original row keys.
    physical_payloads: Arc<[Payload]>,
    logical: Budget,
    local: Budget,
    construction: Budget,
    physical: Budget,
    physical_local: Budget,
}
impl PartialEq for EnvironmentReferencePlan {
    fn eq(&self, o: &Self) -> bool {
        self.payloads == o.payloads
            && self.descriptors == o.descriptors
            && self.ordinals == o.ordinals
    }
}
impl Eq for EnvironmentReferencePlan {}
impl Hash for EnvironmentReferencePlan {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.payloads.hash(h);
        self.descriptors.hash(h);
        self.ordinals.hash(h);
    }
}
impl EnvironmentReferencePlan {
    pub(crate) fn budget(&self) -> Budget {
        self.logical
    }
    pub(crate) fn local_budget(&self) -> Budget {
        self.local
    }
    /// Positive original table/header prefix, never a subtraction from bodies.
    /// Caller prepays the fixed length read and recomputation before this call.
    pub(crate) fn owner_table_budget(&self) -> Result<(Budget, usize), String> {
        let count = self.ordinals.len();
        Ok((
            table_budget(
                count,
                PeakContext::new(
                    PeakOwner::FunctionOwner,
                    PeakPhase::ReferenceTables,
                    FirstLedger::Local,
                ),
            )?,
            count,
        ))
    }
    pub(crate) fn builtin_catalog_only(&self) -> bool {
        self.payloads.len() == 1
            && matches!(self.payloads[0], Payload::Catalog(_))
            && self.descriptors.iter().all(|d| {
                d.environment.mode == BzlEnvironmentMode::Builtin && d.environment.proof.is_none()
            })
    }
    pub(crate) fn physical_budget(&self) -> Budget {
        self.physical
    }
    pub(crate) fn physical_local_budget(&self) -> Budget {
        self.physical_local
    }
    pub(crate) fn construction_budget(&self) -> Budget {
        self.construction
    }
    pub(crate) fn physical_payloads(&self) -> &[Payload] {
        &self.physical_payloads
    }
    pub(crate) fn prepare<'a>(
        envs: impl Iterator<Item = &'a Arc<BzlEnvironmentInputs>> + Clone,
    ) -> Result<Self, String> {
        Self::prepare_indexed(
            envs,
            |hash| hash,
            Budget::default(),
            Budget::default(),
            PeakOwner::EnvironmentReferences,
        )
    }
    pub(crate) fn prepare_with_budget<'a>(
        envs: impl Iterator<Item = &'a Arc<BzlEnvironmentInputs>> + Clone,
        base: Budget,
        work: Budget,
        owner: PeakOwner,
    ) -> Result<Self, String> {
        Self::prepare_indexed(envs, |hash| hash, work, base, owner)
    }
    pub(crate) fn prepare_covered_owner(
        inputs: crate::function_owner::reference_coverage::OwnerReferenceInputs<'_>,
        work: Budget,
    ) -> Result<Self, String> {
        let base = inputs.base();
        Self::prepare_indexed_physical::<covered::Covered>(
            inputs.environments(),
            |hash| hash,
            work,
            base,
            PeakOwner::FunctionOwner,
        )
    }
    fn prepare_indexed<'a>(
        envs: impl Iterator<Item = &'a Arc<BzlEnvironmentInputs>> + Clone,
        bucket: impl Fn(u64) -> u64,
        work: Budget,
        base: Budget,
        owner: PeakOwner,
    ) -> Result<Self, String> {
        Self::prepare_indexed_physical::<covered::Full>(envs, bucket, work, base, owner)
    }
    fn prepare_indexed_physical<'a, A: covered::PhysicalAdmission>(
        envs: impl Iterator<Item = &'a Arc<BzlEnvironmentInputs>> + Clone,
        bucket: impl Fn(u64) -> u64,
        mut work: Budget,
        base: Budget,
        owner: PeakOwner,
    ) -> Result<Self, String> {
        let mut count = 0usize;
        for _ in envs.clone() {
            work.charge_with_context(
                PeakContext::new(owner, PeakPhase::ReferenceTables, FirstLedger::Work),
                1,
                0,
            )?;
            count = count.checked_add(1).ok_or_else(|| {
                crate::bzl_module::semantic_evaluation::budget::limit_with_context(
                    PeakContext::new(owner, PeakPhase::ReferenceTables, FirstLedger::Work),
                )
            })?;
        }
        // All arrays (old/new/final), maps, candidate buckets and borrowed guards
        // are prepaid before the first Vec/map can grow. No allocator identity
        // participates in persistent semantic ordering or equality.
        let mut scratch = Budget::default();
        scratch.charge_with_context(
            PeakContext::new(owner, PeakPhase::ReferenceTables, FirstLedger::Work),
            count,
            count.saturating_mul(4096),
        )?;
        scratch.array_with_context::<Payload>(
            PeakContext::new(owner, PeakPhase::ReferenceTables, FirstLedger::Work),
            count.saturating_mul(5),
        )?;
        scratch.array_with_context::<Descriptor>(
            PeakContext::new(owner, PeakPhase::ReferenceTables, FirstLedger::Work),
            count.saturating_mul(5),
        )?;
        scratch.array_with_context::<usize>(
            PeakContext::new(owner, PeakPhase::ReferenceTables, FirstLedger::Work),
            count.saturating_mul(10),
        )?;
        // This is additional scratch, never repurposed payload/table credit.
        scratch.charge_with_context(
            PeakContext::new(owner, PeakPhase::ReferenceTables, FirstLedger::Work),
            1,
            descriptor_memo_scratch_bytes(count).map_err(|_| {
                crate::bzl_module::semantic_evaluation::budget::limit_with_context(
                    PeakContext::new(owner, PeakPhase::ReferenceTables, FirstLedger::Work),
                )
            })?,
        )?;
        work.charge_with_context(
            PeakContext::new(owner, PeakPhase::ReferenceTables, FirstLedger::Work),
            scratch.records,
            scratch.bytes,
        )?;
        let mut local = table_budget(
            count,
            PeakContext::new(owner, PeakPhase::ReferenceTables, FirstLedger::Local),
        )?;
        admit_combined(
            owner,
            PeakPhase::ReferenceTables,
            FirstLedger::Logical,
            base,
            local,
            work,
        )?;
        let mut aggregate = A::prepare(&local, &mut work, &base, owner)?;
        let mut payloads = Vec::<Payload>::new();
        let mut descriptors = Vec::<Descriptor>::new();
        let mut ordinals = Vec::with_capacity(count);
        let mut physical_payloads = Vec::<Payload>::new();
        let mut payload_index = SmallMap::<u64, Vec<usize>>::new();
        let mut descriptor_index = SmallMap::<u64, Vec<usize>>::new();
        let mut physical = local;
        let mut physical_local = local;
        let mut logical = local;
        let mut seen_payloads = std::collections::BTreeSet::new();
        // All borrowed &Arc inputs remain alive for this constructor, including
        // foreign-equal noncanonical descriptors: allocation keys cannot ABA.
        // Nothing pointer-based survives in the completed semantic plan.
        let mut successful_descriptors = SmallMap::<usize, usize>::with_capacity(count);
        for env in envs {
            work.charge_with_context(
                PeakContext::new(owner, PeakPhase::ReferenceTables, FirstLedger::Work),
                1,
                0,
            )?; // every incoming reference
            work.charge_with_context(
                PeakContext::new(owner, PeakPhase::ReferenceTables, FirstLedger::Work),
                1,
                0,
            )?; // every actual descriptor memo lookup
            let descriptor_pointer = Arc::as_ptr(env) as usize;
            if let Some(&ordinal) = successful_descriptors.get(&descriptor_pointer) {
                aggregate.admit_state(
                    owner,
                    PeakPhase::ReferenceMemoHit,
                    &base,
                    &logical,
                    &physical,
                    &mut work,
                )?;
                ordinals.push(ordinal);
                continue;
            }
            let own = env.local_budget();
            // A memo miss is exactly a first actual allocation encounter. Equal
            // foreign descriptors still pay/retain their complete own storage.
            physical.charge_with_context(
                PeakContext::new(
                    owner,
                    PeakPhase::ReferenceDescriptorStorage,
                    FirstLedger::Physical,
                ),
                own.records,
                own.bytes,
            )?;
            physical_local.charge_with_context(
                PeakContext::new(
                    owner,
                    PeakPhase::ReferenceDescriptorStorage,
                    FirstLedger::Local,
                ),
                own.records,
                own.bytes,
            )?;
            aggregate.admit_physical(
                owner,
                PeakPhase::ReferenceDescriptorStorage,
                &base,
                &physical,
                &mut work,
            )?;
            let payload = env.payload();
            let payload_slot = if let Some(candidate) = payload {
                work.charge_with_context(
                    PeakContext::new(owner, PeakPhase::ReferencePayloadStorage, FirstLedger::Work),
                    1,
                    0,
                )?;
                if seen_payloads.insert(candidate.pointer()) {
                    let full = candidate.physical_budget();
                    physical.charge_with_context(
                        PeakContext::new(
                            owner,
                            PeakPhase::ReferencePayloadStorage,
                            FirstLedger::Physical,
                        ),
                        full.records,
                        full.bytes,
                    )?;
                    aggregate.add_payload(&full, &mut work, owner)?;
                    aggregate.admit_physical(
                        owner,
                        PeakPhase::ReferencePayloadStorage,
                        &base,
                        &physical,
                        &mut work,
                    )?;
                    physical_payloads.push(candidate.retain());
                }
                let mut h = std::collections::hash_map::DefaultHasher::new();
                candidate.hash(&mut h);
                let hash = bucket(h.finish());
                let mut found = None;
                if let Some(indices) = payload_index.get(&hash) {
                    for &i in indices {
                        work.charge_with_context(
                            PeakContext::new(
                                owner,
                                PeakPhase::ReferencePayloadCompare,
                                FirstLedger::Work,
                            ),
                            1,
                            0,
                        )?;
                        let prior = &payloads[i];
                        if prior.borrowed().pointer() != candidate.pointer() {
                            let a = prior.budget();
                            let b = candidate.budget();
                            work.charge_with_context(
                                PeakContext::new(
                                    owner,
                                    PeakPhase::ReferencePayloadCompare,
                                    FirstLedger::Work,
                                ),
                                a.records,
                                a.bytes,
                            )?;
                            work.charge_with_context(
                                PeakContext::new(
                                    owner,
                                    PeakPhase::ReferencePayloadCompare,
                                    FirstLedger::Work,
                                ),
                                b.records,
                                b.bytes,
                            )?;
                            aggregate.admit_state(
                                owner,
                                PeakPhase::ReferencePayloadCompare,
                                &base,
                                &logical,
                                &physical,
                                &mut work,
                            )?;
                        }
                        if prior.borrowed() == candidate {
                            found = Some(i);
                            break;
                        }
                    }
                }
                Some(match found {
                    Some(i) => i,
                    None => {
                        let full = candidate.budget();
                        logical.charge_with_context(
                            PeakContext::new(
                                owner,
                                PeakPhase::ReferencePayloadCanonical,
                                FirstLedger::Logical,
                            ),
                            full.records,
                            full.bytes,
                        )?;
                        aggregate.admit_state(
                            owner,
                            PeakPhase::ReferencePayloadCanonical,
                            &base,
                            &logical,
                            &physical,
                            &mut work,
                        )?;
                        let i = payloads.len();
                        payloads.push(candidate.retain());
                        payload_index.entry(hash).or_default().push(i);
                        i
                    }
                })
            } else {
                None
            };
            work.charge_with_context(
                PeakContext::new(
                    owner,
                    PeakPhase::ReferenceDescriptorCompare,
                    FirstLedger::Work,
                ),
                1,
                0,
            )?;
            let hash = bucket(env.cached_hash());
            let mut found = None;
            if let Some(indices) = descriptor_index.get(&hash) {
                for &i in indices {
                    let prior = &descriptors[i];
                    if descriptor_comparison::equal(
                        prior.payload,
                        &prior.environment,
                        payload_slot,
                        env,
                        |records, bytes| {
                            work.charge_with_context(
                                PeakContext::new(
                                    owner,
                                    PeakPhase::ReferenceDescriptorCompare,
                                    FirstLedger::Work,
                                ),
                                records,
                                bytes,
                            )?;
                            aggregate.admit_state(
                                owner,
                                PeakPhase::ReferenceDescriptorCompare,
                                &base,
                                &logical,
                                &physical,
                                &mut work,
                            )
                        },
                    )? {
                        found = Some(i);
                        break;
                    }
                }
            }
            let ordinal = match found {
                Some(i) => i,
                None => {
                    logical.charge_with_context(
                        PeakContext::new(
                            owner,
                            PeakPhase::ReferenceDescriptorCanonical,
                            FirstLedger::Logical,
                        ),
                        own.records,
                        own.bytes,
                    )?;
                    local.charge_with_context(
                        PeakContext::new(
                            owner,
                            PeakPhase::ReferenceDescriptorCanonical,
                            FirstLedger::Local,
                        ),
                        own.records,
                        own.bytes,
                    )?;
                    aggregate.admit_state(
                        owner,
                        PeakPhase::ReferenceDescriptorCanonical,
                        &base,
                        &logical,
                        &physical,
                        &mut work,
                    )?;
                    let i = descriptors.len();
                    descriptors.push(Descriptor {
                        environment: env.clone(),
                        payload: payload_slot,
                    });
                    descriptor_index.entry(hash).or_default().push(i);
                    i
                }
            };
            // Full payload and descriptor collision resolution succeeded. No
            // refused/partial result can enter the memo, even for equal foreign
            // allocations that resolved to an already existing canonical slot.
            work.charge_with_context(
                PeakContext::new(owner, PeakPhase::ReferenceMemoCommit, FirstLedger::Work),
                1,
                0,
            )?;
            aggregate.admit_state(
                owner,
                PeakPhase::ReferenceMemoCommit,
                &base,
                &logical,
                &physical,
                &mut work,
            )?;
            successful_descriptors.insert(descriptor_pointer, ordinal);
            ordinals.push(ordinal);
        }
        // Full foreign physical memory and constructor work remain independently
        // bounded even when equal facts normalize to the same semantic slot.
        aggregate.admit_physical(
            owner,
            PeakPhase::ReferencePhysicalFinal,
            &base,
            &physical,
            &mut work,
        )?;
        aggregate.admit_state(
            owner,
            PeakPhase::ReferenceFinal,
            &base,
            &logical,
            &physical,
            &mut work,
        )?;

        Ok(Self {
            payloads: payloads.into(),
            descriptors: descriptors.into(),
            ordinals: ordinals.into(),
            physical_payloads: physical_payloads.into(),
            logical,
            local,
            construction: work,
            physical,
            physical_local,
        })
    }
}

fn table_budget(count: usize, context: PeakContext) -> Result<Budget, String> {
    let mut local = Budget::default();
    local.charge_with_context(
        context,
        1,
        size_of::<EnvironmentReferencePlan>() + 10 * size_of::<usize>(),
    )?;
    local.array_with_context::<usize>(context, count)?;
    local.array_with_context::<Payload>(context, count.saturating_mul(2))?;
    local.array_with_context::<Descriptor>(context, count)?;
    local.charge_with_context(context, count, 0)?; // all incoming references
    Ok(local)
}
impl BzlEnvironmentInputs {
    /// Accounting-only borrowed guard, including when payload() prefers proof.
    pub(crate) fn own_catalog(&self) -> Option<PayloadRef<'_>> {
        self.catalog.as_ref().map(PayloadRef::Catalog)
    }
}

/// Pinned SmallMap layout: Vec2<(key,value),hash> plus optional boxed Index.
/// Count bounds every actual descriptor, and preallocation avoids rehash growth;
/// both old/new allocations are nevertheless included conservatively.
pub(crate) fn descriptor_memo_scratch_bytes(count: usize) -> Result<usize, String> {
    use std::alloc::Layout;
    let capacity = count.checked_mul(2).ok_or_else(limit)?.max(4);
    let entries = Layout::array::<(usize, usize)>(capacity)
        .and_then(|pairs| pairs.extend(Layout::array::<starlark_map::StarlarkHashValue>(capacity)?))
        .map_err(|_| limit())?
        .0
        .size();
    // hashbrown0.14: 7/8 load, rounded power-of-two buckets plus control group.
    // The private Index/HashTable's pointer/size metadata is bounded by1024.
    let index = capacity
        .checked_mul(4)
        .and_then(|buckets| buckets.checked_mul(size_of::<usize>() + 1))
        .and_then(|bytes| bytes.checked_add(1024))
        .and_then(|bytes| bytes.checked_add(std::mem::align_of::<usize>().max(16) - 1))
        .and_then(|bytes| bytes.checked_add(16))
        .ok_or_else(limit)?;
    entries
        .checked_mul(2)
        .and_then(|bytes| bytes.checked_add(index.checked_mul(2)?))
        .and_then(|bytes| bytes.checked_add(size_of::<SmallMap<usize, usize>>()))
        .ok_or_else(limit)
}

fn admit_state(
    owner: PeakOwner,
    phase: PeakPhase,
    base: Budget,
    logical: Budget,
    physical: Budget,
    work: Budget,
) -> Result<(), String> {
    // Event credits depend only on complete semantic facts. Foreign-equal
    // allocations remain in the physical peak through every later comparison.
    admit_combined(
        owner,
        phase,
        FirstLedger::Logical,
        base,
        logical,
        Budget::default(),
    )?;
    admit_combined(owner, phase, FirstLedger::Physical, base, physical, work)
}
fn admit_combined(
    owner: PeakOwner,
    phase: PeakPhase,
    first: FirstLedger,
    mut base: Budget,
    plan: Budget,
    work: Budget,
) -> Result<(), String> {
    base.charge_with_context(
        PeakContext::new(owner, phase, first),
        plan.records,
        plan.bytes,
    )?;
    admit_peak(PeakContext::new(owner, phase, first), base, work, 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn owner_table_certificate_is_exact_original_positive_prefix() {
        for count in [0usize, 1, 4, 17, 128] {
            let mut original = Budget::default();
            original
                .charge(
                    1,
                    size_of::<EnvironmentReferencePlan>() + 10 * size_of::<usize>(),
                )
                .unwrap();
            original.array::<usize>(count).unwrap();
            original.array::<Payload>(count.saturating_mul(2)).unwrap();
            original.array::<Descriptor>(count).unwrap();
            original.charge(count, 0).unwrap();
            let table = table_budget(
                count,
                PeakContext::new(
                    PeakOwner::EnvironmentReferences,
                    PeakPhase::ReferenceTables,
                    FirstLedger::Local,
                ),
            )
            .unwrap();
            assert_eq!(
                (table.records, table.bytes, table.depth),
                (original.records, original.bytes, original.depth)
            );
        }
        assert!(
            table_budget(
                usize::MAX,
                PeakContext::new(
                    PeakOwner::EnvironmentReferences,
                    PeakPhase::ReferenceTables,
                    FirstLedger::Local
                )
            )
            .is_err()
        );
    }
    #[test]
    fn reference_peak_context_keeps_caller_subphase_and_physical_ledger() {
        use crate::bzl_module::semantic_evaluation::budget::BYTE_LIMIT;
        let base = Budget {
            records: 17,
            bytes: BYTE_LIMIT - 4096,
            depth: 2,
        };
        let canonical = Budget::default();
        let physical = Budget::default();
        let work = Budget {
            records: 11,
            bytes: 4096,
            depth: 0,
        };
        for owner in [PeakOwner::SemanticInputs, PeakOwner::FunctionOwner] {
            admit_state(
                owner,
                PeakPhase::ReferenceDescriptorCompare,
                base,
                canonical,
                physical,
                work,
            )
            .unwrap();
            let refused = Budget {
                bytes: work.bytes + 1,
                ..work
            };
            let error = admit_state(
                owner,
                PeakPhase::ReferenceDescriptorCompare,
                base,
                canonical,
                physical,
                refused,
            )
            .unwrap_err();
            assert!(error.contains(&format!(
                "owner={} phase=reference-descriptor-compare first_ledger=physical-forecast",
                owner.name()
            )));
            assert!(error.contains("logical_records=17"));
            assert!(error.contains("scratch_records=11"));
            assert!(error.contains("scratch_bytes=4097 extra_bytes=0"));
        }
        assert_eq!(
            (base.records, base.bytes, base.depth),
            (17, BYTE_LIMIT - 4096, 2)
        );
        assert_eq!((work.records, work.bytes), (11, 4096));
    }

    fn env(source: &str) -> PreparedBzlEnvironment {
        use sha2::Digest;
        use sha2::Sha256;
        let digest = Sha256::digest(source.as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let catalog = super::super::catalog::prepare(
            "index-fixture",
            &[("defs.bzl", source.as_bytes(), &digest)],
        )
        .unwrap();
        PreparedBzlEnvironment::seed(BzlEnvironmentMode::Builtin, Some(&catalog)).unwrap()
    }
    #[test]
    fn environment_index_collisions_preserve_complete_foreign_facts_and_normalized_hash() {
        let a = env("value = 1\n");
        let equal = env("value = 1\n");
        let changed = env("value = 2\n");
        let values = [&a.inputs, &equal.inputs, &changed.inputs];
        let ordinary = EnvironmentReferencePlan::prepare(values.into_iter()).unwrap();
        let collided = EnvironmentReferencePlan::prepare_indexed(
            values.into_iter(),
            |_| 0,
            Budget::default(),
            Budget::default(),
            PeakOwner::EnvironmentReferences,
        )
        .unwrap();
        assert_eq!(ordinary, collided);
        assert_eq!(collided.payloads.len(), 2);
        assert_eq!(collided.descriptors.len(), 2);
        assert_eq!(collided.ordinals.as_ref(), &[0, 0, 1]);
        assert!(collided.construction.records > ordinary.construction.records);
        let mut x = std::collections::hash_map::DefaultHasher::new();
        ordinary.hash(&mut x);
        let mut y = std::collections::hash_map::DefaultHasher::new();
        collided.hash(&mut y);
        assert_eq!(x.finish(), y.finish());
        assert_ne!(
            EnvironmentReferencePlan::prepare([&a.inputs].into_iter()).unwrap(),
            EnvironmentReferencePlan::prepare([&changed.inputs].into_iter()).unwrap()
        );
    }
    #[test]
    fn environment_reference_refusal_precedes_index_hash_and_guard_retention() {
        let a = env("value = 1\n");
        let before = Arc::strong_count(&a.inputs);
        let catalog = a.inputs.catalog.as_ref().unwrap();
        let catalog_before = Arc::strong_count(catalog);
        let hashes = std::cell::Cell::new(0);
        let full = Budget {
            records: crate::bzl_module::semantic_evaluation::budget::RECORD_LIMIT,
            ..Budget::default()
        };
        assert!(
            EnvironmentReferencePlan::prepare_indexed(
                [&a.inputs].into_iter(),
                |h| {
                    hashes.set(hashes.get() + 1);
                    h
                },
                full,
                Budget::default(),
                PeakOwner::EnvironmentReferences
            )
            .is_err()
        );
        assert_eq!(hashes.get(), 0);
        let joint = Budget {
            bytes: crate::bzl_module::semantic_evaluation::budget::BYTE_LIMIT,
            ..Budget::default()
        };
        assert!(
            EnvironmentReferencePlan::prepare_with_budget(
                [&a.inputs].into_iter(),
                joint,
                Budget::default(),
                PeakOwner::EnvironmentReferences
            )
            .is_err()
        );

        assert_eq!(Arc::strong_count(&a.inputs), before);
        assert_eq!(Arc::strong_count(catalog), catalog_before);
        // Count-derived table/array growth refuses before creating any guard;
        // an infinite borrowed alias stream cannot bypass bounded pre-count.
        assert!(EnvironmentReferencePlan::prepare(std::iter::repeat(&a.inputs)).is_err());
        assert_eq!(Arc::strong_count(&a.inputs), before);
        assert_eq!(Arc::strong_count(catalog), catalog_before);
    }
    #[test]
    fn actual_descriptor_memo_preserves_foreign_collisions_and_ordered_full_facts() {
        let a = env("value = 1\n");
        let foreign = env("value = 1\n");
        let changed = env("value = 2\n");
        let values = [
            &a.inputs,
            &foreign.inputs,
            &foreign.inputs,
            &a.inputs,
            &changed.inputs,
        ];
        let hashes = std::cell::Cell::new(0);
        let collided = EnvironmentReferencePlan::prepare_indexed(
            values.into_iter(),
            |_| {
                hashes.set(hashes.get() + 1);
                0
            },
            Budget::default(),
            Budget::default(),
            PeakOwner::EnvironmentReferences,
        )
        .unwrap();
        // Each of the three distinct allocations has one payload and one
        // descriptor lookup. The already verified FOREIGN allocation then hits.
        assert_eq!(hashes.get(), 6);
        assert_eq!(collided.ordinals.as_ref(), &[0, 0, 0, 0, 1]);
        assert_eq!(collided.payloads.len(), 2);
        assert_eq!(collided.descriptors.len(), 2);
        let normal = EnvironmentReferencePlan::prepare(values.into_iter()).unwrap();
        assert_eq!(collided, normal);
        assert_eq!(collided.physical.records, normal.physical.records);
        assert_eq!(collided.physical.bytes, normal.physical.bytes);
        let fresh = env("value = 1\n");
        let more_foreign = EnvironmentReferencePlan::prepare(
            [
                &a.inputs,
                &foreign.inputs,
                &fresh.inputs,
                &a.inputs,
                &changed.inputs,
            ]
            .into_iter(),
        )
        .unwrap();
        assert_eq!(normal, more_foreign);
        assert_eq!(normal.budget().records, more_foreign.budget().records);
        assert_eq!(normal.budget().bytes, more_foreign.budget().bytes);
        assert!(more_foreign.physical.bytes > normal.physical.bytes);
        assert!(more_foreign.construction.records > normal.construction.records);
        let mut x = std::collections::hash_map::DefaultHasher::new();
        normal.hash(&mut x);
        let mut y = std::collections::hash_map::DefaultHasher::new();
        more_foreign.hash(&mut y);
        assert_eq!(x.finish(), y.finish());
    }
    #[test]
    fn actual_descriptor_memo_keeps_paid_alias_lookups_and_refuses_before_growth() {
        use crate::bzl_module::semantic_evaluation::budget::BYTE_LIMIT;
        use crate::bzl_module::semantic_evaluation::budget::RECORD_LIMIT;
        let a = env("value = 1\n");
        let inputs = [&a.inputs, &a.inputs, &a.inputs];
        let needed = EnvironmentReferencePlan::prepare(inputs.into_iter())
            .unwrap()
            .construction
            .records;
        let before = Arc::strong_count(&a.inputs);
        let work = Budget {
            records: RECORD_LIMIT - needed,
            ..Budget::default()
        };
        let exact = EnvironmentReferencePlan::prepare_with_budget(
            inputs.into_iter(),
            Budget::default(),
            work,
            PeakOwner::EnvironmentReferences,
        )
        .unwrap();
        assert_eq!(exact.construction.records, RECORD_LIMIT);
        assert_eq!(exact.ordinals.as_ref(), &[0, 0, 0]);
        drop(exact);
        let hashes = std::cell::Cell::new(0);
        let refused = EnvironmentReferencePlan::prepare_indexed(
            inputs.into_iter(),
            |h| {
                hashes.set(hashes.get() + 1);
                h
            },
            Budget {
                records: work.records + 1,
                ..work
            },
            Budget::default(),
            PeakOwner::EnvironmentReferences,
        );
        assert!(
            refused.is_err(),
            "the last warmed lookup still consumes work"
        );
        assert_eq!(hashes.get(), 2);
        assert_eq!(Arc::strong_count(&a.inputs), before);
        hashes.set(0);
        let base = Budget {
            bytes: BYTE_LIMIT - descriptor_memo_scratch_bytes(1).unwrap() + 1,
            ..Budget::default()
        };
        assert!(
            EnvironmentReferencePlan::prepare_indexed(
                [&a.inputs].into_iter(),
                |h| {
                    hashes.set(hashes.get() + 1);
                    h
                },
                Budget::default(),
                base,
                PeakOwner::EnvironmentReferences
            )
            .is_err()
        );
        assert_eq!(
            hashes.get(),
            0,
            "memo peak admission precedes hashing/guards/growth"
        );
        assert_eq!(Arc::strong_count(&a.inputs), before);
    }
    #[test]
    fn descriptor_memo_forecast_covers_actual_hashed_entries_and_index_peaks() {
        for count in [0, 1, 4, 17, 33, 65, 257] {
            let bound = descriptor_memo_scratch_bytes(count).unwrap();
            let mut map = SmallMap::<usize, usize>::with_capacity(count);
            for i in 0..count {
                map.insert(i, i);
            }
            let allocation = map.retained_storage_bytes_upper_bound().unwrap();
            assert!(bound >= size_of::<SmallMap<usize, usize>>() + 2 * allocation);
            let mut grown = SmallMap::<usize, usize>::new();
            for i in 0..count {
                grown.insert(i, i);
                assert!(
                    bound
                        >= size_of::<SmallMap<usize, usize>>()
                            + 2 * grown.retained_storage_bytes_upper_bound().unwrap()
                );
            }
        }
        assert!(descriptor_memo_scratch_bytes(usize::MAX).is_err());
    }
}

#[cfg(test)]
#[path = "references/context_tests.rs"]
mod context_tests;

#[path = "references/covered.rs"]
mod covered;
