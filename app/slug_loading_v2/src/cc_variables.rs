//! Native C++ variables: shallow live bindings and immutable publication seeds.
use std::cell::RefCell;
use std::fmt;
use std::hash::Hash;
use std::sync::Arc;
use std::sync::Mutex;

use allocative::Allocative;
use compact_str::CompactString;
use slug_build_api_v2::cc_variables::CcVariableCacheEntry;
use slug_build_api_v2::cc_variables::CcVariablesKind;
use slug_build_api_v2::cc_variables::CcVariablesOccurrence;
use starlark::any::ProvidesStaticType;
use starlark::collections::StarlarkHasher;
use starlark::values::Demand;
use starlark::values::Freeze;
use starlark::values::FreezeResult;
use starlark::values::Freezer;
use starlark::values::FrozenHeap;
use starlark::values::FrozenValue;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::StarlarkValue;
use starlark::values::Trace;
use starlark::values::Value;
use starlark::values::ValueLike;
use starlark::values::dict::DictRef;
use starlark::values::list::AllocImmutableList;
use starlark::values::list::ListRef;
use starlark::values::starlark_value;
use starlark::values::structs::StarlarkStructuralValue;
use starlark::values::tuple::TupleRef;
use starlark_map::small_map::SmallMap;

mod access;
pub(crate) mod dispatch;
pub(crate) mod link_args;
mod raw_hash;
#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative, Trace)]
struct Name(#[trace(unsafe_ignore)] CompactString);

#[derive(Debug, Clone, Allocative, Trace)]
pub(crate) enum Entry<V> {
    Value(V),
    Sequence(Box<[V]>),
    Missing,
    Error(#[trace(unsafe_ignore)] CompactString),
}
impl<V> Entry<V> {
    fn map<W>(self, mut f: impl FnMut(V) -> W) -> Entry<W> {
        match self {
            Self::Value(v) => Entry::Value(f(v)),
            Self::Sequence(v) => Entry::Sequence(v.into_vec().into_iter().map(f).collect()),
            Self::Missing => Entry::Missing,
            Self::Error(e) => Entry::Error(e),
        }
    }
    fn public(self) -> CcVariableCacheEntry<V> {
        match self {
            Self::Value(v) => CcVariableCacheEntry::Value(v),
            Self::Sequence(v) => CcVariableCacheEntry::Sequence(v),
            Self::Missing => CcVariableCacheEntry::Missing,
            Self::Error(e) => CcVariableCacheEntry::Error(e),
        }
    }
    fn from_public(value: CcVariableCacheEntry<V>) -> Self {
        match value {
            CcVariableCacheEntry::Value(v) => Self::Value(v),
            CcVariableCacheEntry::Sequence(v) => Self::Sequence(v),
            CcVariableCacheEntry::Missing => Self::Missing,
            CcVariableCacheEntry::Error(e) => Self::Error(e),
        }
    }
}

#[derive(Debug, Allocative)]
struct LiveCache<V>(Mutex<SmallMap<Name, Entry<V>>>);
// Tracing has exclusive access to the enclosing value. No lock survives tracing
// or conversion, and frozen values never contain this variant.
unsafe impl<'v, V: Trace<'v>> Trace<'v> for LiveCache<V> {
    fn trace(&mut self, tracer: &starlark::values::Tracer<'v>) {
        self.0
            .get_mut()
            .expect("C++ variable cache is not poisoned")
            .trace(tracer);
    }
}

#[derive(Debug, Allocative, Trace)]
enum Cache<V> {
    Live(LiveCache<V>),
    Frozen(Box<[(Name, Entry<V>)]>),
}
impl<V: Copy> Cache<V> {
    fn snapshot(&self) -> Vec<(Name, Entry<V>)> {
        match self {
            Self::Live(cache) => cache
                .0
                .lock()
                .expect("C++ variable cache is not poisoned")
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            Self::Frozen(cache) => cache.to_vec(),
        }
    }
}

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative, Trace)]
struct VariablesGen<V> {
    #[trace(unsafe_ignore)]
    kind: CcVariablesKind,
    #[trace(unsafe_ignore)]
    occurrence: CcVariablesOccurrence,
    parent: Option<V>,
    #[trace(unsafe_ignore)]
    names: Arc<[CompactString]>,
    values: Box<[V]>,
    cache: Cache<V>,
}
type Variables<'v> = VariablesGen<Value<'v>>;
type FrozenVariables = VariablesGen<FrozenValue>;
starlark::starlark_complex_values!(Variables);

impl Freeze for Variables<'_> {
    type Frozen = FrozenVariables;
    fn freeze(self, freezer: &Freezer) -> FreezeResult<Self::Frozen> {
        let cache = match self.cache {
            Cache::Live(cache) => cache
                .0
                .into_inner()
                .expect("C++ variable cache is not poisoned")
                .into_iter()
                .collect::<Vec<_>>(),
            Cache::Frozen(cache) => cache.into_vec(),
        };
        let cache = cache
            .into_iter()
            .map(|(name, value)| {
                let value = match value {
                    Entry::Value(v) => Entry::Value(v.freeze(freezer)?),
                    Entry::Sequence(v) => Entry::Sequence(
                        v.into_vec()
                            .into_iter()
                            .map(|v| v.freeze(freezer))
                            .collect::<FreezeResult<_>>()?,
                    ),
                    Entry::Missing => Entry::Missing,
                    Entry::Error(e) => Entry::Error(e),
                };
                Ok((name, value))
            })
            .collect::<FreezeResult<_>>()?;
        Ok(VariablesGen {
            kind: self.kind,
            occurrence: self.occurrence,
            parent: self.parent.freeze(freezer)?,
            names: self.names,
            values: self
                .values
                .into_vec()
                .into_iter()
                .map(|v| v.freeze(freezer))
                .collect::<FreezeResult<_>>()?,
            cache: Cache::Frozen(cache),
        })
    }
}

impl<V> fmt::Display for VariablesGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self.kind {
            CcVariablesKind::Map => "<unknown object com.google.devtools.build.lib.rules.cpp.CcToolchainVariables$MapVariables>",
            CcVariablesKind::Single => "<unknown object com.google.devtools.build.lib.rules.cpp.CcToolchainVariables$SingleVariables>",
        })
    }
}

#[starlark_value(type = "Variables")]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for VariablesGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    type Canonical = FrozenVariables;
    fn equals(&self, other: Value<'v>) -> starlark::Result<bool> {
        let Some(other) = cc_variables_value(other) else {
            return Ok(false);
        };
        if self.kind != other.kind
            || self.names.len() != other.locals.len()
            || parent_occurrence(self.parent.map(ValueLike::to_value))
                != parent_occurrence(other.parent)
        {
            return Ok(false);
        }
        for ((name, value), (other_name, other_value)) in
            self.names.iter().zip(self.values.iter()).zip(other.locals)
        {
            if *name != other_name || !value.to_value().equals(other_value)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
    fn write_hash(&self, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        self.write_structural_hash(hasher)
    }
    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn StarlarkStructuralValue>(self);
    }
}
impl<'v, V: ValueLike<'v>> StarlarkStructuralValue for VariablesGen<V> {
    fn is_structurally_immutable(&self) -> bool {
        self.kind == CcVariablesKind::Map
    }
    fn write_structural_hash(&self, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        if self.kind == CcVariablesKind::Single {
            return Err(starlark::Error::new_other(anyhow::anyhow!(
                "unhashable type: Variables"
            )));
        }
        parent_occurrence(self.parent.map(ValueLike::to_value)).hash(hasher);
        for (name, value) in self.names.iter().zip(self.values.iter()) {
            name.hash(hasher);
            raw_hash::hash(value.to_value(), hasher)?;
        }
        Ok(())
    }
}

fn parent_occurrence(value: Option<Value<'_>>) -> Option<CcVariablesOccurrence> {
    match Variables::from_value(value?)? {
        starlark::__macro_refs::Either::Left(v) => Some(v.occurrence.clone()),
        starlark::__macro_refs::Either::Right(v) => Some(v.occurrence.clone()),
    }
}

fn local_bindings(value: Value<'_>) -> Option<Vec<(CompactString, Value<'_>)>> {
    fn locals<'v, V: ValueLike<'v>>(v: &VariablesGen<V>) -> Vec<(CompactString, Value<'v>)> {
        v.names
            .iter()
            .cloned()
            .zip(v.values.iter().map(|v| v.to_value()))
            .collect()
    }
    match Variables::from_value(value)? {
        starlark::__macro_refs::Either::Left(v) => Some(locals(v)),
        starlark::__macro_refs::Either::Right(v) => Some(locals(v)),
    }
}

/// A shallow evaluator snapshot. Values remain rooted in their original heap.
pub struct CcVariablesView<'v> {
    pub kind: CcVariablesKind,
    pub occurrence: CcVariablesOccurrence,
    pub parent: Option<Value<'v>>,
    pub locals: Vec<(CompactString, Value<'v>)>,
    pub cache: Vec<(CompactString, CcVariableCacheEntry<Value<'v>>)>,
}
/// Test the actual moving or frozen variables facade without projecting caches.
pub fn is_cc_variables_value(value: Value<'_>) -> bool {
    Variables::from_value(value).is_some()
}

/// Borrowed facade header; does not snapshot or lock the semantic seed.
pub struct CcVariablesHeader<'v> {
    pub kind: CcVariablesKind,
    pub occurrence: &'v CcVariablesOccurrence,
    pub parent: Option<Value<'v>>,
    pub local_count: usize,
}
pub fn cc_variables_header<'v>(value: Value<'v>) -> Option<CcVariablesHeader<'v>> {
    fn header<'v, V: ValueLike<'v>>(v: &'v VariablesGen<V>) -> CcVariablesHeader<'v> {
        CcVariablesHeader {
            kind: v.kind,
            occurrence: &v.occurrence,
            parent: v.parent.map(ValueLike::to_value),
            local_count: v.names.len(),
        }
    }
    Some(match Variables::from_value(value)? {
        starlark::__macro_refs::Either::Left(v) => header(v),
        starlark::__macro_refs::Either::Right(v) => header(v),
    })
}
pub fn visit_cc_variables_locals<'v, E>(
    value: Value<'v>,
    visit: &mut dyn FnMut(&str, Value<'v>) -> Result<(), E>,
) -> Option<Result<(), E>> {
    fn locals<'v, V: ValueLike<'v>, E>(
        v: &VariablesGen<V>,
        visit: &mut dyn FnMut(&str, Value<'v>) -> Result<(), E>,
    ) -> Result<(), E> {
        for (name, value) in v.names.iter().zip(v.values.iter()) {
            visit(name, value.to_value())?;
        }
        Ok(())
    }
    Some(match Variables::from_value(value)? {
        starlark::__macro_refs::Either::Left(v) => locals(v, visit),
        starlark::__macro_refs::Either::Right(v) => locals(v, visit),
    })
}
/// Charge all copied seed storage before copying. The callback must only perform
/// checked scalar budget accounting: no recursion, evaluator or DICE work. All
/// returned Value handles are inspected after the live-cache lock is released.
pub fn snapshot_cc_variables_seed_bounded<'v, E>(
    value: Value<'v>,
    charge: &mut dyn FnMut(usize, usize) -> Result<(), E>,
) -> Result<Option<Vec<(CompactString, CcVariableCacheEntry<Value<'v>>)>>, E> {
    fn snapshot<'v, V: ValueLike<'v>, E>(
        v: &VariablesGen<V>,
        charge: &mut dyn FnMut(usize, usize) -> Result<(), E>,
    ) -> Result<Vec<(CompactString, CcVariableCacheEntry<Value<'v>>)>, E> {
        fn bounded<'a, 'v, V: ValueLike<'v> + 'a, E>(
            count: usize,
            entries: impl Clone + Iterator<Item = (&'a Name, &'a Entry<V>)>,
            charge: &mut dyn FnMut(usize, usize) -> Result<(), E>,
        ) -> Result<Vec<(CompactString, CcVariableCacheEntry<Value<'v>>)>, E> {
            charge(
                count.saturating_mul(std::mem::size_of::<(
                    CompactString,
                    CcVariableCacheEntry<Value<'v>>,
                )>()),
                count,
            )?;
            for (name, entry) in entries.clone() {
                charge(name.0.len(), 0)?;
                match entry {
                    Entry::Value(_) => charge(0, 1)?,
                    Entry::Sequence(values) => charge(
                        values
                            .len()
                            .saturating_mul(std::mem::size_of::<Value<'v>>()),
                        values.len(),
                    )?,
                    Entry::Error(message) => charge(message.len(), 0)?,
                    Entry::Missing => {}
                }
            }
            Ok(entries
                .map(|(name, entry)| {
                    let entry = match entry {
                        Entry::Value(v) => CcVariableCacheEntry::Value(v.to_value()),
                        Entry::Sequence(v) => {
                            CcVariableCacheEntry::Sequence(v.iter().map(|v| v.to_value()).collect())
                        }
                        Entry::Missing => CcVariableCacheEntry::Missing,
                        Entry::Error(message) => CcVariableCacheEntry::Error(message.clone()),
                    };
                    (name.0.clone(), entry)
                })
                .collect())
        }
        match &v.cache {
            Cache::Live(cache) => {
                let guard = cache.0.lock().expect("C++ variable cache is not poisoned");
                bounded(guard.len(), guard.iter(), charge)
            }
            Cache::Frozen(cache) => bounded(
                cache.len(),
                cache.iter().map(|(name, entry)| (name, entry)),
                charge,
            ),
        }
    }
    match Variables::from_value(value) {
        Some(starlark::__macro_refs::Either::Left(v)) => snapshot(v, charge).map(Some),
        Some(starlark::__macro_refs::Either::Right(v)) => snapshot(v, charge).map(Some),
        None => Ok(None),
    }
}

pub fn cc_variables_value(value: Value<'_>) -> Option<CcVariablesView<'_>> {
    fn view<'v, V: ValueLike<'v>>(v: &VariablesGen<V>) -> CcVariablesView<'v> {
        CcVariablesView {
            kind: v.kind,
            occurrence: v.occurrence.clone(),
            parent: v.parent.map(ValueLike::to_value),
            locals: v
                .names
                .iter()
                .cloned()
                .zip(v.values.iter().map(|v| v.to_value()))
                .collect(),
            cache: v
                .cache
                .snapshot()
                .into_iter()
                .map(|(name, value)| (name.0, value.map(ValueLike::to_value).public()))
                .collect(),
        }
    }
    match Variables::from_value(value)? {
        starlark::__macro_refs::Either::Left(v) => Some(view(v)),
        starlark::__macro_refs::Either::Right(v) => Some(view(v)),
    }
}

pub fn alloc_frozen_cc_variables(
    heap: &FrozenHeap,
    kind: CcVariablesKind,
    occurrence: CcVariablesOccurrence,
    parent: Option<FrozenValue>,
    locals: Vec<(CompactString, FrozenValue)>,
    cache: Vec<(CompactString, CcVariableCacheEntry<FrozenValue>)>,
) -> FrozenValue {
    let (names, values): (Vec<_>, Vec<_>) = locals.into_iter().unzip();
    heap.alloc(VariablesGen {
        kind,
        occurrence,
        parent,
        names: names.into(),
        values: values.into_boxed_slice(),
        cache: Cache::Frozen(
            cache
                .into_iter()
                .map(|(name, value)| (Name(name), Entry::from_public(value)))
                .collect(),
        ),
    })
}

fn alloc<'v>(
    heap: Heap<'v>,
    kind: CcVariablesKind,
    parent: Option<Value<'v>>,
    mut locals: Vec<(CompactString, Value<'v>)>,
) -> Value<'v> {
    locals.sort_by(|(a, _), (b, _)| a.encode_utf16().cmp(b.encode_utf16()));
    let (names, values): (Vec<_>, Vec<_>) = locals.into_iter().unzip();
    heap.alloc(VariablesGen {
        kind,
        occurrence: CcVariablesOccurrence::new(),
        parent,
        names: names.into(),
        values: values.into_boxed_slice(),
        cache: Cache::Live(LiveCache(Mutex::new(SmallMap::new()))),
    })
}
pub(crate) fn construct<'v>(vars: Value<'v>, heap: Heap<'v>) -> anyhow::Result<Value<'v>> {
    let dict = DictRef::from_value(vars)
        .ok_or_else(|| anyhow::anyhow!("vars must be a dict, got {}", vars.get_type()))?;
    let locals = dict
        .iter()
        .map(|(name, value)| {
            name.unpack_str()
                .map(|name| (name.into(), value))
                .ok_or_else(|| anyhow::anyhow!("vars key must be string, got {}", name.get_type()))
        })
        .collect::<anyhow::Result<_>>()?;
    Ok(alloc(heap, CcVariablesKind::Map, None, locals))
}
pub(crate) fn combine<'v>(
    parent: Value<'v>,
    variables: &[Value<'v>],
    heap: Heap<'v>,
) -> anyhow::Result<Value<'v>> {
    anyhow::ensure!(
        Variables::from_value(parent).is_some(),
        "parent must be CcToolchainVariables"
    );
    let mut locals = SmallMap::new();
    for variable in variables {
        let variable = local_bindings(*variable)
            .ok_or_else(|| anyhow::anyhow!("variables must contain CcToolchainVariables"))?;
        let overlaps = variable
            .iter()
            .filter(|(name, _)| locals.contains_key(name))
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>();
        anyhow::ensure!(
            overlaps.is_empty(),
            "Cannot overwrite existing variables: [{}]",
            overlaps.join(", ")
        );
        for (name, value) in variable {
            locals.insert(name, value);
        }
    }
    let kind = if locals.len() == 1 {
        CcVariablesKind::Single
    } else {
        CcVariablesKind::Map
    };
    Ok(alloc(
        heap,
        kind,
        Some(parent),
        locals.into_iter().collect(),
    ))
}
pub(crate) fn string_sequence<'v>(value: Value<'v>, heap: Heap<'v>) -> anyhow::Result<Value<'v>> {
    let values = if let Some(v) = ListRef::from_value(value) {
        v.iter().collect::<Vec<_>>()
    } else if let Some(v) = TupleRef::from_value(value) {
        v.iter().collect()
    } else if value.get_type() == "range" {
        value
            .iterate(heap)
            .map_err(starlark::Error::into_anyhow)?
            .collect()
    } else {
        anyhow::bail!("string_sequence must be a sequence")
    };
    anyhow::ensure!(
        values.iter().all(|v| v.unpack_str().is_some()),
        "string_sequence must contain strings"
    );
    Ok(heap.alloc(AllocImmutableList(values)))
}
