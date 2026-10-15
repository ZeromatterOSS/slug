//! Trusted bridge from native structural evaluation owners to actual Def tokens.
use std::any::Any;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::hash_map::DefaultHasher;
use std::hash::Hash;
use std::hash::Hasher;
use std::sync::Arc;

use allocative::Allocative;
use slug_build_api_v2::published_value::FunctionEvaluationOwner;
use slug_build_api_v2::published_value::FunctionOwnerDomain;
use slug_build_api_v2::published_value::FunctionSymbolName;
use slug_build_api_v2::published_value::ImmutablePublicationInput;
use slug_build_api_v2::published_value::PublicationShape;
use slug_build_api_v2::published_value::PublishedFunctionSymbol;
use slug_build_api_v2::published_value::SharedPublicationInput;
use slug_build_api_v2::published_value::StructuralFunctionOwnerPart;
use starlark::codemap::CodeMap;
use starlark::eval::FunctionSymbolOwner;
use starlark::eval::FunctionSymbolSession;
use starlark::eval::FunctionSymbolStateRef;
use starlark::eval::inspect_function_symbol;
use starlark::values::Value;

use crate::BzlLoadManifest;
use crate::BzlModuleIdentity;
use crate::bzl_module::semantic_evaluation::budget::peak_context::FirstLedger;
use crate::bzl_module::semantic_evaluation::budget::peak_context::PeakContext;
use crate::bzl_module::semantic_evaluation::budget::peak_context::PeakOwner;
use crate::bzl_module::semantic_evaluation::budget::peak_context::PeakPhase;

#[derive(Debug, Allocative)]
struct NativeFunctionOwner(FunctionEvaluationOwner);
impl FunctionSymbolOwner for NativeFunctionOwner {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn equals(&self, other: &dyn FunctionSymbolOwner) -> bool {
        other
            .as_any()
            .downcast_ref::<Self>()
            .is_some_and(|other| self.0 == other.0)
    }
    fn hash_owner(&self, mut state: &mut dyn Hasher) {
        self.0.hash(&mut state);
    }
}
pub fn function_symbol_session(owner: FunctionEvaluationOwner) -> FunctionSymbolSession {
    FunctionSymbolSession::new(Arc::new(NativeFunctionOwner(owner)))
}
/// Borrow the trusted host owner for quota forecasting before retaining a symbol.
pub fn function_symbol_owner_ref(
    value: Value<'_>,
) -> anyhow::Result<Option<&FunctionEvaluationOwner>> {
    let Some(symbol) = inspect_function_symbol(value) else {
        return Ok(None);
    };
    let owner = symbol
        .owner()
        .as_any()
        .downcast_ref::<NativeFunctionOwner>()
        .ok_or_else(|| {
            anyhow::anyhow!("function publication requires an admitted structural evaluation owner")
        })?;
    Ok(Some(&owner.0))
}
pub fn snapshot_function_symbol(
    value: Value<'_>,
) -> anyhow::Result<Option<PublishedFunctionSymbol>> {
    let Some(owner) = function_symbol_owner_ref(value)? else {
        return Ok(None);
    };
    let symbol = inspect_function_symbol(value).expect("owner inspector checked an actual Def");
    let name = match symbol.state() {
        FunctionSymbolStateRef::Local { ordinal } => FunctionSymbolName::Local(ordinal),
        FunctionSymbolStateRef::Global { name } => FunctionSymbolName::Exported(Arc::from(name)),
    };
    Ok(Some(PublishedFunctionSymbol::new(owner.clone(), name)))
}

#[derive(Debug, Allocative)]
struct BzlStableKey {
    identity: BzlModuleIdentity,
    environment: Arc<crate::bzl_environment::BzlEnvironmentInputs>,
    bazel_internal_strings: bool,
    /// Nonsemantic candidate bucket only; full equality/hash ignore this cache.
    candidate_index_hash: u64,
    shape: PublicationShape,
    comparison: PublicationShape,
    publication: crate::bzl_environment::publication::PublicationMetadata,
}
/// One immutable module's facts. Child links contain complete stable keys only;
/// equality and hashing never follow another evaluation owner.
#[derive(Debug, Allocative)]
struct LocalBzlInput {
    key: Arc<BzlStableKey>,
    source_digest: [u8; 32],
    fingerprint: [u8; 32],
    source: CodeMap,
    /// Complete ACTUAL attached client-data metadata remains budgeted, while
    /// its redundant expanded semantic representation is validated then dropped.
    original_metadata: PublicationShape,
    children: Arc<[Arc<BzlStableKey>]>,
    shape: PublicationShape,
    comparison: PublicationShape,
    publication: crate::bzl_environment::publication::PublicationMetadata,
}
#[derive(Debug, Allocative)]
struct BzlInputValidity {
    /// Root first, then first-seen direct-child closures in load order. Sharing
    /// these local rows avoids copying transitive source and manifest storage.
    rows: Arc<[Arc<LocalBzlInput>]>,
    /// Accounting-only handles over the exact same immutable row allocations.
    shared_inputs: Arc<[SharedPublicationInput]>,
    /// Intrinsic headers and ordered row references; row payload is declared.
    shape: PublicationShape,
    environment_owners: Arc<[FunctionEvaluationOwner]>,
    references: crate::bzl_environment::references::EnvironmentReferencePlan,
}
impl PartialEq for BzlStableKey {
    fn eq(&self, other: &Self) -> bool {
        self.identity == other.identity
            && self.environment == other.environment
            && self.bazel_internal_strings == other.bazel_internal_strings
    }
}
impl Eq for BzlStableKey {}
impl Hash for BzlStableKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        hash_stable_key_fields(
            &self.identity,
            &self.environment,
            self.bazel_internal_strings,
            state,
        );
    }
}
/// Use the same complete semantic field writer for both hashes. The
/// candidate cache producer uses this same full-field writer once per key.
fn hash_stable_key_fields<H: Hasher>(
    identity: &BzlModuleIdentity,
    environment: &crate::bzl_environment::BzlEnvironmentInputs,
    bazel_internal_strings: bool,
    state: &mut H,
) {
    identity.hash(state);
    environment.hash(state);
    bazel_internal_strings.hash(state);
}
fn write_candidate_index_hash<H: Hasher>(
    identity: &BzlModuleIdentity,
    environment: &crate::bzl_environment::BzlEnvironmentInputs,
    bazel_internal_strings: bool,
    shape: PublicationShape,
    work: &mut PublicationShape,
    state: &mut H,
) -> anyhow::Result<()> {
    // Refuse before any hash write or owning root-identity/Arc clone.
    add_shape(work, shape, 0)
        .map_err(|error| anyhow::anyhow!("stable candidate-index hash construction: {error}"))?;
    hash_stable_key_fields(identity, environment, bazel_internal_strings, state);
    Ok(())
}
impl ImmutablePublicationInput for LocalBzlInput {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn retained_shape(&self) -> PublicationShape {
        self.publication.full()
    }
    fn intrinsic_shape(&self) -> PublicationShape {
        self.publication.intrinsic()
    }
    fn shared_inputs(&self) -> &[SharedPublicationInput] {
        self.publication.children()
    }
}
impl ImmutablePublicationInput for BzlStableKey {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn retained_shape(&self) -> PublicationShape {
        self.publication.full()
    }
    fn intrinsic_shape(&self) -> PublicationShape {
        self.publication.intrinsic()
    }
    fn shared_inputs(&self) -> &[SharedPublicationInput] {
        self.publication.children()
    }
}
impl PartialEq for LocalBzlInput {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
            && self.source_digest == other.source_digest
            && self.fingerprint == other.fingerprint
            && self.source.source() == other.source.source()
            && self.children == other.children
    }
}
impl Eq for LocalBzlInput {}
impl Hash for LocalBzlInput {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.key.hash(state);
        self.source_digest.hash(state);
        self.fingerprint.hash(state);
        self.source.source().hash(state);
        self.children.hash(state);
    }
}
impl PartialEq for BzlInputValidity {
    fn eq(&self, other: &Self) -> bool {
        self.references == other.references
            && self.rows.len() == other.rows.len()
            && self
                .rows
                .iter()
                .zip(other.rows.iter())
                .all(|(a, b)| environment_view::local_eq(a, b))
    }
}
impl Eq for BzlInputValidity {}
mod environment_publication;
mod key_publication;
pub(crate) mod reference_coverage;
mod stable_owner;
use stable_owner::BzlStableOwner;
impl StructuralFunctionOwnerPart for BzlInputValidity {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn eq_part(&self, other: &dyn StructuralFunctionOwnerPart) -> bool {
        other.as_any().downcast_ref::<Self>() == Some(self)
    }
    fn hash_part(&self, mut state: &mut dyn Hasher) {
        self.references.hash(&mut state);
        self.rows.len().hash(&mut state);
        for row in &*self.rows {
            environment_view::hash_local(row, &mut state);
        }
    }
    fn retained_bytes(&self) -> usize {
        self.shape.bytes
    }
    fn retained_shape(&self) -> PublicationShape {
        self.shape
    }
    fn shared_inputs(&self) -> &[SharedPublicationInput] {
        &self.shared_inputs
    }
    fn retained_owners(&self) -> &[FunctionEvaluationOwner] {
        &self.environment_owners
    }
}
fn zero_shape() -> PublicationShape {
    PublicationShape {
        nodes: 0,
        edges: 0,
        bytes: 0,
        visits: 0,
        depth: 0,
    }
}
fn add_shape(
    total: &mut PublicationShape,
    part: PublicationShape,
    nesting: usize,
) -> anyhow::Result<()> {
    total.nodes = total.nodes.saturating_add(part.nodes);
    total.edges = total.edges.saturating_add(part.edges);
    total.bytes = total.bytes.saturating_add(part.bytes);
    total.visits = total.visits.saturating_add(part.visits);
    total.depth = total.depth.max(part.depth.saturating_add(nesting));
    check_shape(*total)
}
fn check_shape(shape: PublicationShape) -> anyhow::Result<()> {
    if shape.bytes > 64 * 1024 * 1024
        || shape.visits > 65536
        || shape.nodes > 65536
        || shape.edges > 524288
        || shape.depth > 256
    {
        anyhow::bail!(
            "function owner shape limit: nodes={} visits={} edges={} bytes={} depth={} (limits 65536/65536/524288/67108864/256)",
            shape.nodes,
            shape.visits,
            shape.edges,
            shape.bytes,
            shape.depth,
        );
    }
    Ok(())
}
fn fact(bytes: usize, edges: usize, depth: usize) -> PublicationShape {
    PublicationShape {
        bytes,
        edges,
        visits: 1,
        depth,
        nodes: 0,
    }
}
fn identity_shape(identity: &BzlModuleIdentity) -> anyhow::Result<PublicationShape> {
    identity_record::shape(identity)
}
mod identity_record;

mod environment_view;
mod manifest;
/// A hash is only an index into candidates. Every match still compares the
/// complete typed key, followed by the complete local facts. Collision work is
/// explicitly charged before comparison, so even adversarial buckets are bounded.
fn insert_local_row(
    row: &Arc<LocalBzlInput>,
    rows: &mut Vec<Arc<LocalBzlInput>>,
    index: &mut BTreeMap<u64, Vec<usize>>,
    occurrences: &mut BTreeSet<usize>,
    retained: &mut PublicationShape,
    intrinsic: &mut PublicationShape,
    work: &mut PublicationShape,
) -> anyhow::Result<()> {
    add_shape(
        work,
        PublicationShape {
            nodes: 1,
            visits: 1,
            ..zero_shape()
        },
        0,
    )?;
    // An already-seen immutable row is the exact same facts, so no fields are
    // compared or hashed again. Addresses are phase-only work memo keys; the
    // persistent closure still deduplicates distinct rows by complete facts.
    let occurrence = Arc::as_ptr(row) as usize;
    if occurrences.contains(&occurrence) {
        return Ok(());
    }
    add_shape(work, fact(std::mem::size_of::<u64>(), 0, 0), 0)
        .map_err(|error| anyhow::anyhow!("flat union candidate-index hash read: {error}"))?;
    let hash = row.key.candidate_index_hash;
    if let Some(candidates) = index.get(&hash) {
        for &position in candidates {
            let previous = &rows[position];
            add_shape(work, row.key.comparison, 0)
                .map_err(|error| anyhow::anyhow!("flat union candidate-key comparison: {error}"))?;
            add_shape(work, previous.key.comparison, 0)
                .map_err(|error| anyhow::anyhow!("flat union retained-key comparison: {error}"))?;
            if previous.key == row.key {
                add_shape(work, previous.comparison, 0).map_err(|error| {
                    anyhow::anyhow!("flat union duplicate retained-local facts: {error}")
                })?;
                add_shape(work, row.comparison, 0).map_err(|error| {
                    anyhow::anyhow!("flat union duplicate candidate-local facts: {error}")
                })?;
                if previous != row {
                    anyhow::bail!("Bzl owner closure has conflicting facts for one stable key");
                }
                manifest::scratch_insertion(work, None)?;
                occurrences.insert(occurrence);
                return Ok(());
            }
        }
    }
    // Bound the retained row and both temporary index slots before growing
    // either collection. Index bytes are construction work, not retained state.
    add_shape(retained, row.shape, 1)
        .map_err(|error| anyhow::anyhow!("flat union unique retained-local facts: {error}"))?;
    // Both semantic and accounting arrays retain one handle per unique row.
    let handles = fact(
        std::mem::size_of::<Arc<LocalBzlInput>>() + std::mem::size_of::<SharedPublicationInput>(),
        2,
        1,
    );
    add_shape(retained, handles, 0)?;
    add_shape(intrinsic, handles, 0)?;
    // A growable typed Vec may hold four slots for its first row, then up to
    // twice its length. Allow five slots per row including its final Arc slice;
    // the exactly sized certificate Vec and its Arc also fit this forecast.
    // Check peak payload + both arrays BEFORE growing either array.
    let extra_handles = fact(
        handles
            .bytes
            .saturating_mul(4)
            .saturating_mul(rows.len().saturating_add(1)),
        0,
        0,
    );
    let mut peak = *retained;
    add_shape(&mut peak, extra_handles, 0)?;
    add_shape(work, fact(handles.bytes.saturating_mul(5), 2, 1), 0)?;
    let (len, capacity) = index
        .get(&hash)
        .map_or((0, 0), |values| (values.len(), values.capacity()));
    // Preflight BOTH independent tree insertions before mutating either one.
    manifest::scratch_insertion(work, Some((len, capacity, std::mem::size_of::<usize>())))?;
    manifest::scratch_insertion(work, None)?;
    let position = rows.len();
    index.entry(hash).or_default().push(position);
    occurrences.insert(occurrence);
    rows.push(row.clone());
    Ok(())
}
/// Construct before module evaluation; all child owners come from actual loaded
/// module client metadata. Source bytes participate directly in equality, with
/// source digests retained solely as tracked integrity metadata.
pub(crate) fn bzl_evaluation_owner(
    metadata: crate::starlark_label::PreparedDefiningModuleMetadata<'_>,
    source: CodeMap,
    environment: &crate::bzl_environment::PreparedBzlEnvironment,
    bazel_internal_strings: bool,
    children: impl IntoIterator<Item = anyhow::Result<FunctionEvaluationOwner>>,
) -> anyhow::Result<FunctionEvaluationOwner> {
    let manifest = metadata.raw_manifest();
    if source.source().len() > 64 * 1024 * 1024 {
        anyhow::bail!("function owner source byte limit");
    }
    let mut work = zero_shape();
    let mut stable_shape = identity_shape(&manifest.root)
        .map_err(|error| anyhow::anyhow!("root stable key: {error}"))?;
    add_shape(
        &mut stable_shape,
        fact(
            std::mem::size_of::<BzlStableKey>()
                .saturating_sub(std::mem::size_of::<BzlModuleIdentity>())
                .saturating_add(2 * std::mem::size_of::<usize>()),
            0,
            0,
        ),
        0,
    )?;
    // The semantic environment handle is distinct from its new accounting
    // guard; its bytes are already inline in sizeof(BzlStableKey).
    add_shape(&mut stable_shape, fact(0, 1, 1), 0)?;
    let stable_intrinsic = stable_shape;
    let environment_budget = environment.inputs().local_budget();
    add_shape(
        &mut stable_shape,
        PublicationShape {
            nodes: 0,
            edges: environment_budget.records,
            bytes: environment_budget.bytes,
            visits: environment_budget.records,
            depth: environment_budget.depth,
        },
        1,
    )?;
    let key_publication = key_publication::prepare(
        stable_intrinsic,
        1,
        std::iter::once_with(|| environment.inputs().retained_shape()),
        &mut work,
    )?;
    add_shape(&mut stable_shape, key_publication.storage(), 0)?;
    let mut stable_comparison = stable_shape;
    if let Some(payload) = environment.inputs().payload() {
        add_shape(
            &mut stable_comparison,
            crate::bzl_environment::publication_shape(payload.budget()),
            1,
        )?;
    }
    let mut local_shape = fact(
        std::mem::size_of::<LocalBzlInput>() + 4 * std::mem::size_of::<usize>(),
        4,
        1,
    );
    local_shape.nodes = 1;
    let mut local_intrinsic = local_shape;
    add_shape(&mut local_shape, stable_shape, 1)?;
    let (original_metadata, metadata_work) = manifest::original_storage(metadata)
        .map_err(|error| anyhow::anyhow!("original manifest retention: {error}"))?;
    key_publication::add_local(&mut local_shape, &mut local_intrinsic, original_metadata, 1)?;
    // Conservative fixed CodeMap heap metadata: filename handle, the larger
    // owned-source String plus enum tag/alignment, line Vec and reporting flag,
    // and Arc controls for CodeMap, filename and a possible shared source.
    key_publication::add_local(
        &mut local_shape,
        &mut local_intrinsic,
        fact(
            std::mem::size_of::<(
                Arc<str>,
                String,
                Vec<starlark::codemap::Pos>,
                bool,
                usize,
                usize,
            )>() + 6 * std::mem::size_of::<usize>(),
            0,
            1,
        ),
        0,
    )?;
    key_publication::add_local(
        &mut local_shape,
        &mut local_intrinsic,
        fact(source.source().len(), 0, 0),
        1,
    )?;
    key_publication::add_local(
        &mut local_shape,
        &mut local_intrinsic,
        fact(source.filename().len(), 0, 0),
        1,
    )?;
    let lines = source
        .source()
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count()
        .saturating_add(1);
    // CodeMap's line table is scalar indexing storage, not semantic graph
    // edges. Its positions are never traversed by owner Eq/hash. Forecast up
    // to twice the live line positions for Vec capacity, while source bytes
    // remain charged in full for actual source comparison and hashing.
    key_publication::add_local(
        &mut local_shape,
        &mut local_intrinsic,
        fact(
            lines
                .saturating_mul(2)
                .saturating_mul(std::mem::size_of::<starlark::codemap::Pos>()),
            0,
            2,
        ),
        0,
    )?;
    // Construction work is independent of child construction history. A shared
    // diamond contributes its flat rows twice here, never its expanded paths.
    add_shape(&mut work, local_shape, 0)?;
    add_shape(&mut work, metadata_work, 0)?;
    manifest::transient_peak(metadata, &mut work)?;
    let mut child_keys = Vec::new();
    let mut child_rows = Vec::new();
    for child in children {
        let child = child?;
        if child.domain() != FunctionOwnerDomain::BzlLoad {
            anyhow::bail!("Bzl owner child has an unsupported evaluation domain");
        }
        let validity = child
            .input_validity()
            .as_any()
            .downcast_ref::<BzlInputValidity>()
            .ok_or_else(|| {
                anyhow::anyhow!("Bzl child owner has an unknown validity certificate")
            })?;
        let root = validity
            .rows
            .first()
            .ok_or_else(|| anyhow::anyhow!("empty Bzl owner closure"))?;
        let stable = child
            .stable_key()
            .as_any()
            .downcast_ref::<BzlStableOwner>()
            .ok_or_else(|| {
                anyhow::anyhow!("Bzl child owner has an unknown stable key certificate")
            })?;
        add_shape(&mut work, stable.root().key.comparison, 0)?;
        add_shape(&mut work, root.key.comparison, 0)?;
        if stable.root().key.as_ref() != root.key.as_ref() {
            anyhow::bail!("Bzl child owner root certificate mismatch");
        }
        // An equal foreign key cannot replace the actual row whose complete
        // environment backing is declared by this same child's validity.
        add_shape(&mut work, fact(0, 0, 0), 0)?;
        if !Arc::ptr_eq(stable.root(), root) {
            anyhow::bail!("Bzl child owner root certificate mismatch");
        }
        let reference = fact(std::mem::size_of::<Arc<BzlStableKey>>(), 1, 1);
        key_publication::add_local(&mut local_shape, &mut local_intrinsic, reference, 0)?;
        add_shape(&mut local_shape, root.key.shape, 1)?;
        add_shape(&mut work, fact(reference.bytes.saturating_mul(5), 1, 1), 0)?;
        // Forecast all direct-child row handles before retaining this slice.
        add_shape(
            &mut work,
            fact(
                std::mem::size_of::<Arc<[Arc<LocalBzlInput>]>>().saturating_mul(5),
                1,
                1,
            ),
            0,
        )?;
        child_keys.push(root.key.clone());
        child_rows.push(validity.rows.clone());
    }
    add_shape(&mut work, fact(0, 0, 0), 0)?;
    let key_count = child_keys
        .len()
        .checked_add(1)
        .ok_or_else(|| anyhow::anyhow!("key declaration count overflow"))?;
    let row_publication = key_publication::prepare(
        local_intrinsic,
        key_count,
        std::iter::once_with(|| key_publication.full())
            .chain(child_keys.iter().map(|key| key.retained_shape())),
        &mut work,
    )?;
    add_shape(&mut local_shape, row_publication.storage(), 0)?;
    let mut comparison = local_shape;
    if let Some(payload) = environment.inputs().payload() {
        add_shape(
            &mut comparison,
            crate::bzl_environment::publication_shape(payload.budget()),
            1,
        )?;
    }
    for child in &child_keys {
        if let Some(payload) = child.environment.payload() {
            add_shape(
                &mut comparison,
                crate::bzl_environment::publication_shape(payload.budget()),
                1,
            )?;
        }
    }
    let source_digest = manifest::validate(manifest, &child_rows, &mut work)
        .map_err(|error| anyhow::anyhow!("Bzl manifest validation: {error}"))?;
    let mut candidate_hasher = DefaultHasher::new();
    write_candidate_index_hash(
        &manifest.root,
        environment.inputs(),
        bazel_internal_strings,
        stable_shape,
        &mut work,
        &mut candidate_hasher,
    )?;
    key_publication::precopy(local_shape, work)?;
    let key = Arc::new(BzlStableKey {
        identity: manifest.root.clone(),
        environment: environment.inputs().clone(),
        bazel_internal_strings,
        candidate_index_hash: candidate_hasher.finish(),
        shape: stable_shape,
        comparison: stable_comparison,
        publication: key_publication
            .finish(vec![SharedPublicationInput::new(
                environment.inputs().clone(),
            )])
            .map_err(anyhow::Error::msg)?,
    });
    let mut declared_keys = Vec::with_capacity(key_count);
    declared_keys.push(SharedPublicationInput::new(key.clone()));
    declared_keys.extend(
        child_keys
            .iter()
            .map(|key| SharedPublicationInput::new(key.clone())),
    );
    let root = Arc::new(LocalBzlInput {
        key: key.clone(),
        source_digest,
        fingerprint: manifest.fingerprint,
        source,
        original_metadata,
        children: child_keys.into(),
        shape: local_shape,
        comparison,
        publication: row_publication
            .finish(declared_keys)
            .map_err(anyhow::Error::msg)?,
    });
    let mut rows = Vec::new();
    let mut index = BTreeMap::new();
    let mut occurrences = BTreeSet::new();
    let mut intrinsic = fact(
        std::mem::size_of::<BzlInputValidity>() + 4 * std::mem::size_of::<usize>(),
        2,
        1,
    );
    let environment_owner_count = environment.modules().count();
    add_shape(
        &mut intrinsic,
        fact(
            (environment_owner_count
                .saturating_mul(std::mem::size_of::<FunctionEvaluationOwner>()))
            .saturating_add(2 * std::mem::size_of::<usize>()),
            environment_owner_count,
            1,
        ),
        0,
    )?;
    add_shape(
        &mut work,
        fact(
            environment_owner_count
                .saturating_mul(5 * std::mem::size_of::<FunctionEvaluationOwner>()),
            environment_owner_count,
            1,
        ),
        0,
    )?;
    let environment_owners = environment.declared_owners().map_err(anyhow::Error::msg)?;
    let mut retained = intrinsic;
    insert_local_row(
        &root,
        &mut rows,
        &mut index,
        &mut occurrences,
        &mut retained,
        &mut intrinsic,
        &mut work,
    )?;
    for child in &child_rows {
        for row in child.iter() {
            insert_local_row(
                row,
                &mut rows,
                &mut index,
                &mut occurrences,
                &mut retained,
                &mut intrinsic,
                &mut work,
            )?;
        }
    }
    reference_coverage::OwnerReferenceInputs::prepay(&mut work)?;
    let references =
        crate::bzl_environment::references::EnvironmentReferencePlan::prepare_covered_owner(
            reference_coverage::OwnerReferenceInputs {
                rows: &rows,
                base: crate::bzl_module::semantic_evaluation::budget::Budget {
                    records: retained.visits,
                    bytes: retained.bytes,
                    depth: retained.depth,
                },
            },
            crate::bzl_module::semantic_evaluation::budget::Budget {
                records: work.visits,
                bytes: work.bytes,
                depth: work.depth,
            },
        )
        .map_err(anyhow::Error::msg)?;
    let plan_work = references.construction_budget();
    work.visits = plan_work.records;
    work.bytes = plan_work.bytes;
    work.depth = work.depth.max(plan_work.depth);
    let shared_count = references
        .physical_payloads()
        .iter()
        .try_fold(rows.len(), |n, p| {
            n.checked_add(p.shared_count())
                .ok_or_else(|| anyhow::anyhow!("environment shared reference overflow"))
        })?;
    let (table_shape, environment_count) =
        environment_publication::table_shape(&references, &mut work)?;
    add_shape(&mut intrinsic, table_shape, 0)?;
    // These exact row/key shapes already cover every descriptor's local body.
    // Add the positive new plan table; whole payloads/catalogs remain below.
    add_shape(&mut retained, table_shape, 0)?;
    // New declaration array and all coercion/transfer handles are admitted
    // before cloning a source-proof/catalog guard into the native certificate.
    let extra = shared_count.saturating_sub(rows.len());
    let extra_shape = fact(
        extra.saturating_mul(std::mem::size_of::<SharedPublicationInput>()),
        extra,
        extra,
    );
    add_shape(&mut intrinsic, extra_shape, 0)?;
    add_shape(&mut retained, extra_shape, 0)?;
    add_shape(
        &mut work,
        fact(
            shared_count.saturating_mul(5 * std::mem::size_of::<SharedPublicationInput>()),
            shared_count,
            shared_count,
        ),
        0,
    )?;
    for payload in references.physical_payloads() {
        add_shape(
            &mut retained,
            crate::bzl_environment::publication_shape(payload.physical_budget()),
            1,
        )?;
    }
    // Preserve the old full declaration/body preflight and construction work.
    // The returned legacy array length is not the new stored representation.
    environment_publication::admit(
        &rows,
        environment_count,
        shared_count,
        &mut retained,
        &mut work,
    )?;
    // Pay the new stored-length read before any final transfer admission.
    add_shape(&mut work, fact(std::mem::size_of::<usize>(), 0, 0), 0)?;
    let row_count = rows.len();
    // The plan's foreign allocation differential stays live while row, owner
    // and shared-input arrays transfer. Work/scratch is the SAME caller ledger.
    crate::bzl_module::semantic_evaluation::budget::admit_peak(
        PeakContext::new(
            PeakOwner::FunctionOwner,
            PeakPhase::NativeTransfer,
            FirstLedger::NativeRetained,
        ),
        crate::bzl_module::semantic_evaluation::budget::Budget {
            records: retained.visits,
            bytes: retained.bytes,
            depth: retained.depth,
        },
        crate::bzl_module::semantic_evaluation::budget::Budget {
            records: work.visits,
            bytes: work.bytes,
            depth: work.depth,
        },
        0,
    )
    .map_err(anyhow::Error::msg)?;
    // Constructor scratch/hash/collision work is bounded above, independently
    // of retained payload. Do not replay that history in each publication.
    // The first successful insertion stored this exact root, and subsequent
    // union insertions never replace it. The same rows build both references
    // and validity below; the stable declaration must share that actual row.
    let stable = BzlStableOwner::new(&rows[0], retained, &mut work)?;
    // Every slot and conversion peak was checked before the corresponding row
    // entered `rows`. Coercion clones the existing row Arc, with no new cookie
    // or payload allocation and no source/manifest copy.
    // Every row already owns its complete key/environment/proof/catalog DAG.
    let mut shared_inputs = Vec::with_capacity(row_count);
    for row in &rows {
        shared_inputs.push(SharedPublicationInput::new(row.clone()));
    }
    assert_eq!(shared_inputs.len(), row_count);
    Ok(FunctionEvaluationOwner::new(
        FunctionOwnerDomain::BzlLoad,
        stable,
        Arc::new(BzlInputValidity {
            rows: rows.into(),
            shared_inputs: shared_inputs.into(),
            shape: intrinsic,
            environment_owners: environment_owners.into(),
            references,
        }),
    )?)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod candidate_index_tests;
