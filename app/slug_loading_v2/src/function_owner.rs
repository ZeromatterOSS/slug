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
    bzlmod: bool,
    bazel_internal_strings: bool,
    /// Nonsemantic candidate bucket only; full equality/hash ignore this cache.
    candidate_index_hash: u64,
    shape: PublicationShape,
}
/// One immutable module's facts. Child links contain complete stable keys only;
/// equality and hashing never follow another evaluation owner.
#[derive(Debug, Allocative)]
struct LocalBzlInput {
    key: Arc<BzlStableKey>,
    source_digest: [u8; 32],
    fingerprint: [u8; 32],
    source: CodeMap,
    /// Full original client-data manifest storage remains budgeted even though
    /// its redundant expanded semantic representation is validated then dropped.
    original_metadata: PublicationShape,
    children: Arc<[Arc<BzlStableKey>]>,
    shape: PublicationShape,
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
}
impl PartialEq for BzlStableKey {
    fn eq(&self, other: &Self) -> bool {
        self.identity == other.identity
            && self.bzlmod == other.bzlmod
            && self.bazel_internal_strings == other.bazel_internal_strings
    }
}
impl Eq for BzlStableKey {}
impl Hash for BzlStableKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        hash_stable_key_fields(
            &self.identity,
            self.bzlmod,
            self.bazel_internal_strings,
            state,
        );
    }
}
/// Keep the persistent semantic hash byte stream exactly unchanged. The
/// candidate cache producer uses this same full-field writer once per key.
fn hash_stable_key_fields<H: Hasher>(
    identity: &BzlModuleIdentity,
    bzlmod: bool,
    bazel_internal_strings: bool,
    state: &mut H,
) {
    identity.hash(state);
    bzlmod.hash(state);
    bazel_internal_strings.hash(state);
}
fn write_candidate_index_hash<H: Hasher>(
    identity: &BzlModuleIdentity,
    bzlmod: bool,
    bazel_internal_strings: bool,
    shape: PublicationShape,
    work: &mut PublicationShape,
    state: &mut H,
) -> anyhow::Result<()> {
    // Refuse before any hash write or owning root-identity/Arc clone.
    add_shape(work, shape, 0)
        .map_err(|error| anyhow::anyhow!("stable candidate-index hash construction: {error}"))?;
    hash_stable_key_fields(identity, bzlmod, bazel_internal_strings, state);
    Ok(())
}
impl ImmutablePublicationInput for LocalBzlInput {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn retained_shape(&self) -> PublicationShape {
        self.shape
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
        self.rows == other.rows
    }
}
impl Eq for BzlInputValidity {}
impl StructuralFunctionOwnerPart for BzlStableKey {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn eq_part(&self, other: &dyn StructuralFunctionOwnerPart) -> bool {
        other.as_any().downcast_ref::<Self>() == Some(self)
    }
    fn hash_part(&self, mut state: &mut dyn Hasher) {
        self.hash(&mut state);
    }
    fn retained_bytes(&self) -> usize {
        self.shape.bytes
    }
    fn retained_shape(&self) -> PublicationShape {
        self.shape
    }
}
impl StructuralFunctionOwnerPart for BzlInputValidity {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn eq_part(&self, other: &dyn StructuralFunctionOwnerPart) -> bool {
        other.as_any().downcast_ref::<Self>() == Some(self)
    }
    fn hash_part(&self, mut state: &mut dyn Hasher) {
        self.rows.hash(&mut state);
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
            add_shape(work, row.key.shape, 0)
                .map_err(|error| anyhow::anyhow!("flat union candidate-key comparison: {error}"))?;
            add_shape(work, previous.key.shape, 0)
                .map_err(|error| anyhow::anyhow!("flat union retained-key comparison: {error}"))?;
            if previous.key == row.key {
                add_shape(work, previous.shape, 0).map_err(|error| {
                    anyhow::anyhow!("flat union duplicate retained-local facts: {error}")
                })?;
                add_shape(work, row.shape, 0).map_err(|error| {
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
    manifest: &BzlLoadManifest,
    source: CodeMap,
    bzlmod: bool,
    bazel_internal_strings: bool,
    children: impl IntoIterator<Item = anyhow::Result<FunctionEvaluationOwner>>,
) -> anyhow::Result<FunctionEvaluationOwner> {
    if source.source().len() > 64 * 1024 * 1024 {
        anyhow::bail!("function owner source byte limit");
    }
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
    let mut local_shape = fact(
        std::mem::size_of::<LocalBzlInput>() + 4 * std::mem::size_of::<usize>(),
        4,
        1,
    );
    local_shape.nodes = 1;
    add_shape(&mut local_shape, stable_shape, 1)?;
    let (original_metadata, metadata_work) = manifest::original_storage(manifest)
        .map_err(|error| anyhow::anyhow!("original manifest retention: {error}"))?;
    add_shape(&mut local_shape, original_metadata, 1)?;
    // Conservative fixed CodeMap heap metadata: filename handle, the larger
    // owned-source String plus enum tag/alignment, line Vec and reporting flag,
    // and Arc controls for CodeMap, filename and a possible shared source.
    add_shape(
        &mut local_shape,
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
    add_shape(&mut local_shape, fact(source.source().len(), 0, 0), 1)?;
    add_shape(&mut local_shape, fact(source.filename().len(), 0, 0), 1)?;
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
    add_shape(
        &mut local_shape,
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
    let mut work = local_shape;
    add_shape(&mut work, metadata_work, 0)?;
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
            .downcast_ref::<BzlStableKey>()
            .ok_or_else(|| {
                anyhow::anyhow!("Bzl child owner has an unknown stable key certificate")
            })?;
        add_shape(&mut work, stable.shape, 0)?;
        add_shape(&mut work, root.key.shape, 0)?;
        if stable != root.key.as_ref() {
            anyhow::bail!("Bzl child owner root certificate mismatch");
        }
        let reference = fact(std::mem::size_of::<Arc<BzlStableKey>>(), 1, 1);
        add_shape(&mut local_shape, reference, 0)?;
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
    let source_digest = manifest::validate(manifest, &child_rows, &mut work)
        .map_err(|error| anyhow::anyhow!("Bzl manifest validation: {error}"))?;
    let mut candidate_hasher = DefaultHasher::new();
    write_candidate_index_hash(
        &manifest.root,
        bzlmod,
        bazel_internal_strings,
        stable_shape,
        &mut work,
        &mut candidate_hasher,
    )?;
    let key = Arc::new(BzlStableKey {
        identity: manifest.root.clone(),
        bzlmod,
        bazel_internal_strings,
        candidate_index_hash: candidate_hasher.finish(),
        shape: stable_shape,
    });
    let root = Arc::new(LocalBzlInput {
        key: key.clone(),
        source_digest,
        fingerprint: manifest.fingerprint,
        source,
        original_metadata,
        children: child_keys.into(),
        shape: local_shape,
    });
    let mut rows = Vec::new();
    let mut index = BTreeMap::new();
    let mut occurrences = BTreeSet::new();
    let mut intrinsic = fact(
        std::mem::size_of::<BzlInputValidity>() + 4 * std::mem::size_of::<usize>(),
        2,
        1,
    );
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
    // Constructor scratch/hash/collision work is bounded above, independently
    // of retained payload. Do not replay that history in each publication.
    let mut complete = retained;
    add_shape(&mut complete, stable_shape, 0)
        .map_err(|error| anyhow::anyhow!("combined stable key and flat validity: {error}"))?;
    // Every slot and conversion peak was checked before the corresponding row
    // entered `rows`. Coercion clones the existing row Arc, with no new cookie
    // or payload allocation and no source/manifest copy.
    let mut shared_inputs = Vec::with_capacity(rows.len());
    for row in &rows {
        shared_inputs.push(SharedPublicationInput::new(row.clone()));
    }
    Ok(FunctionEvaluationOwner::new(
        FunctionOwnerDomain::BzlLoad,
        key,
        Arc::new(BzlInputValidity {
            rows: rows.into(),
            shared_inputs: shared_inputs.into(),
            shape: intrinsic,
        }),
    )?)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod candidate_index_tests;
