//! NOOP-path-mapping adapter. Only evaluator-live nodes mutate their own cache.
use std::rc::Rc;

use slug_build_api_v2::cc_features::CcFeatureError;
use slug_build_api_v2::cc_features::CcVariableAccess;
use starlark::values::structs::StructRef;

use super::*;
use crate::provider::StarlarkDepset;
use crate::provider::StarlarkToolchainInfo;
use crate::provider::starlark_user_provider_fields;
use crate::subrule_invocation::AnalysisArtifactValue;

type LookupCache<'v> = SmallMap<Name, Entry<Value<'v>>>;
#[derive(Clone)]
enum Source<'v> {
    Native(Value<'v>),
    Scoped(Rc<Scope<'v>>),
}
struct Scope<'v> {
    parent: Source<'v>,
    name: CompactString,
    value: Entry<Value<'v>>,
    cache: RefCell<LookupCache<'v>>,
}
#[derive(Clone)]
pub(crate) struct Access<'v> {
    source: Source<'v>,
    heap: Heap<'v>,
    // Frozen seeds cannot change. Within one query, parent and scoped views share
    // scratch caches so native lookup ordering is maintained without DICE mutation.
    frozen: Rc<RefCell<SmallMap<usize, LookupCache<'v>>>>,
}

pub(crate) struct Sequence<'v> {
    access: Access<'v>,
    values: std::vec::IntoIter<Value<'v>>,
}
impl<'v> Iterator for Sequence<'v> {
    type Item = Result<Entry<Value<'v>>, CcFeatureError>;
    fn next(&mut self) -> Option<Self::Item> {
        self.values.next().map(|value| self.access.adapt(value))
    }
}
impl<'v> Access<'v> {
    pub(crate) fn new(value: Value<'v>, heap: Heap<'v>) -> anyhow::Result<Self> {
        anyhow::ensure!(
            Variables::from_value(value).is_some(),
            "variables must be CcToolchainVariables, got {}",
            value.get_type()
        );
        Ok(Self {
            source: Source::Native(value),
            heap,
            frozen: Rc::new(RefCell::new(SmallMap::new())),
        })
    }
    fn adapt(&self, value: Value<'v>) -> Result<Entry<Value<'v>>, CcFeatureError> {
        if value.is_none() {
            return Ok(Entry::Missing);
        }
        if value.unpack_str().is_some()
            || value.unpack_bool().is_some()
            || AnalysisArtifactValue::from_starlark(value).is_some()
            || structure(value)
        {
            return Ok(Entry::Value(value));
        }
        if StarlarkDepset::parts_from_value(value).is_some() {
            return StarlarkDepset::flatten(value)
                .map(|v| Entry::Sequence(v.into_boxed_slice()))
                .map_err(error);
        }
        if matches!(
            value.get_type(),
            "list" | "tuple" | "dict" | "range" | "set"
        ) {
            return value
                .iterate(self.heap)
                .map(|values| Entry::Sequence(values.collect()))
                .map_err(error);
        }
        Err(CcFeatureError(format!(
            "unsupported C++ variable value of type {}",
            value.get_type()
        )))
    }
    fn direct(
        &self,
        source: &Source<'v>,
        name: &str,
    ) -> Result<Option<Entry<Value<'v>>>, CcFeatureError> {
        match source {
            Source::Scoped(scope) => {
                if scope.name == name {
                    return Ok(present(scope.value.clone()));
                }
                self.direct(&scope.parent, name)
            }
            Source::Native(value) => {
                // Do not snapshot/cache all bindings merely to query one direct key.
                fn find<'v, V: ValueLike<'v>>(
                    v: &VariablesGen<V>,
                    name: &str,
                ) -> (Option<Value<'v>>, Option<Value<'v>>) {
                    (
                        v.names
                            .iter()
                            .position(|n| n == name)
                            .map(|i| v.values[i].to_value()),
                        v.parent.map(ValueLike::to_value),
                    )
                }
                let (local, parent) =
                    match Variables::from_value(*value).expect("validated variable source") {
                        starlark::__macro_refs::Either::Left(v) => find(v, name),
                        starlark::__macro_refs::Either::Right(v) => find(v, name),
                    };
                // A local None does not fall back to a direct parent binding.
                if let Some(local) = local {
                    return self.adapt(local).map(present);
                }
                match parent {
                    Some(parent) => self.direct(&Source::Native(parent), name),
                    None => Ok(None),
                }
            }
        }
    }
    fn cache_get(&self, name: &str) -> Option<Entry<Value<'v>>> {
        let key = Name(name.into());
        match &self.source {
            Source::Scoped(scope) => scope.cache.borrow().get(&key).cloned(),
            Source::Native(value) => match Variables::from_value(*value).unwrap() {
                starlark::__macro_refs::Either::Left(v) => match &v.cache {
                    Cache::Live(cache) => cache
                        .0
                        .lock()
                        .expect("C++ variable cache is not poisoned")
                        .get(&key)
                        .cloned(),
                    Cache::Frozen(_) => unreachable!("live nodes have live caches"),
                },
                starlark::__macro_refs::Either::Right(v) => {
                    let mut scratch = self.frozen.borrow_mut();
                    if !scratch.contains_key(&v.occurrence.pointer()) {
                        scratch.insert(
                            v.occurrence.pointer(),
                            v.cache
                                .snapshot()
                                .into_iter()
                                .map(|(k, v)| (k, v.map(ValueLike::to_value)))
                                .collect(),
                        );
                    }
                    scratch
                        .get(&v.occurrence.pointer())
                        .unwrap()
                        .get(&key)
                        .cloned()
                }
            },
        }
    }
    fn cache_put(&self, name: &str, entry: Entry<Value<'v>>) {
        let key = Name(name.into());
        match &self.source {
            Source::Scoped(scope) => {
                scope.cache.borrow_mut().insert(key, entry);
            }
            Source::Native(value) => match Variables::from_value(*value).unwrap() {
                starlark::__macro_refs::Either::Left(v) => match &v.cache {
                    Cache::Live(cache) => {
                        cache
                            .0
                            .lock()
                            .expect("C++ variable cache is not poisoned")
                            .insert(key, entry);
                    }
                    Cache::Frozen(_) => unreachable!("live nodes have live caches"),
                },
                starlark::__macro_refs::Either::Right(v) => {
                    self.frozen
                        .borrow_mut()
                        .get_mut(&v.occurrence.pointer())
                        .expect("cache_get initialized scratch")
                        .insert(key, entry);
                }
            },
        }
    }
    fn dotted(&self, name: &str, throw: bool) -> Result<Entry<Value<'v>>, CcFeatureError> {
        let mut prefix = name;
        let mut fields = Vec::new();
        let (mut value, struct_path) = loop {
            let Some((parent, field)) = prefix.rsplit_once('.') else {
                return Ok(Entry::Missing);
            };
            fields.push(field);
            prefix = parent;
            if let Some(value) = self.direct(&self.source, prefix)? {
                break (value, prefix);
            }
        };
        for field in fields.into_iter().rev() {
            let raw = match &value {
                Entry::Value(v) if structure(*v) => Some(*v),
                _ => None,
            };
            let Some(raw) = raw else {
                return if throw {
                    Ok(Entry::Error(format!(
                        "Cannot expand variable '{struct_path}.{field}': variable '{struct_path}' is {}, expected structure",
                        type_name(&value)
                    ).into()))
                } else {
                    Ok(Entry::Missing)
                };
            };
            let field_value = raw.get_attr(field, self.heap).map_err(error)?;
            value = match field_value {
                Some(v) => self.adapt(v)?,
                None => Entry::Missing,
            };
            if matches!(value, Entry::Missing) {
                return if throw {
                    Ok(Entry::Error(format!(
                        "Cannot expand variable '{struct_path}.{field}': structure {struct_path} doesn't have a field named '{field}'"
                    ).into()))
                } else {
                    Ok(Entry::Missing)
                };
            }
        }
        Ok(value)
    }
}
fn error(error: impl fmt::Display) -> CcFeatureError {
    CcFeatureError(error.to_string())
}
fn present<V>(entry: Entry<V>) -> Option<Entry<V>> {
    if matches!(entry, Entry::Missing) {
        None
    } else {
        Some(entry)
    }
}
fn structure(value: Value<'_>) -> bool {
    StructRef::from_value(value).is_some()
        || starlark_user_provider_fields(value).is_some()
        || StarlarkToolchainInfo::fields_from_value(value).is_some()
}
fn type_name(entry: &Entry<Value<'_>>) -> &'static str {
    match entry {
        Entry::Sequence(_) => "sequence",
        Entry::Value(v) if v.unpack_bool().is_some() => "boolean",
        Entry::Value(v) if v.unpack_str().is_some() => "string",
        Entry::Value(v) if AnalysisArtifactValue::from_starlark(*v).is_some() => "artifact",
        Entry::Value(v) => v.get_type(),
        Entry::Missing => "NoneType",
        Entry::Error(_) => "invalid",
    }
}
impl<'v> CcVariableAccess for Access<'v> {
    type Value = Entry<Value<'v>>;
    type Sequence = Sequence<'v>;
    fn lookup(&mut self, name: &str, throw: bool) -> Result<Option<Self::Value>, CcFeatureError> {
        if let Some(value) = self.direct(&self.source, name)? {
            return Ok(Some(value));
        }
        let entry = if !name.contains('.') {
            Entry::Missing
        } else if let Some(value) = self.cache_get(name) {
            value
        } else {
            // Only ordinary expansion failures are cached/suppressed by an
            // availability query. Unsupported raw conversions are fatal even
            // inside a structure and must not masquerade as absent variables.
            let entry = self.dotted(name, throw)?;
            self.cache_put(name, entry.clone());
            entry
        };
        match entry {
            Entry::Missing if throw => Err(CcFeatureError(format!(
                "Cannot find variable named '{name}'."
            ))),
            Entry::Error(e) if throw => Err(CcFeatureError(e.to_string())),
            Entry::Missing | Entry::Error(_) => Ok(None),
            value => Ok(Some(value)),
        }
    }
    fn string(&mut self, value: &Self::Value, name: &str) -> Result<CompactString, CcFeatureError> {
        if let Entry::Value(value) = value {
            if let Some(s) = value.unpack_str() {
                return Ok(s.into());
            }
            if let Some(b) = value.unpack_bool() {
                return Ok(if b { "1" } else { "0" }.into());
            }
            if let Some(file) = AnalysisArtifactValue::from_starlark(*value) {
                return Ok(file.artifact().path().as_ref().into());
            }
        }
        Err(CcFeatureError(format!(
            "Cannot expand variable '{name}': expected string, found {}",
            type_name(value)
        )))
    }
    fn sequence(
        &mut self,
        value: &Self::Value,
        name: &str,
    ) -> Result<Self::Sequence, CcFeatureError> {
        match value {
            Entry::Sequence(values) => Ok(Sequence {
                access: self.clone(),
                values: values.to_vec().into_iter(),
            }),
            _ => Err(CcFeatureError(format!(
                "Cannot expand variable '{name}': expected sequence, found {}",
                type_name(value)
            ))),
        }
    }
    fn truthy(&mut self, value: &Self::Value) -> Result<bool, CcFeatureError> {
        Ok(match value {
            Entry::Value(v) => v.to_bool(),
            Entry::Sequence(v) => !v.is_empty(),
            Entry::Missing => false,
            Entry::Error(e) => return Err(CcFeatureError(e.to_string())),
        })
    }
    fn with_binding(&self, name: &str, value: Self::Value) -> Self {
        Self {
            source: Source::Scoped(Rc::new(Scope {
                parent: self.source.clone(),
                name: name.into(),
                value,
                cache: RefCell::new(SmallMap::new()),
            })),
            heap: self.heap,
            frozen: self.frozen.clone(),
        }
    }
}
