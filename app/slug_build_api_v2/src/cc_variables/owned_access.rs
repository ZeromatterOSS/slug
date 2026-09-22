//! NOOP-path-mapping access to published Variables. All lookup state is scratch;
//! immutable cache seeds and nested values are never mutated or rematerialized.
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use compact_str::CompactString;
use dupe::Dupe;
use starlark_map::small_map::SmallMap;

use super::CcToolchainVariables;
use super::CcVariableCacheEntry;
use crate::AnalysisValue;
use crate::AnalysisValueKind;
use crate::cc_features::CcFeatureError;
use crate::cc_features::CcVariableAccess;

#[derive(Clone)]
pub(crate) enum OwnedCcVariableValue {
    Value(AnalysisValue),
    Sequence(Arc<[AnalysisValue]>),
    Missing,
    Error(CompactString),
}
type Entry = OwnedCcVariableValue;
type Cache = SmallMap<CompactString, Entry>;

#[derive(Clone)]
enum Source<'a> {
    Native(&'a CcToolchainVariables),
    Scoped(Rc<Scope<'a>>),
}
struct Scope<'a> {
    parent: Source<'a>,
    name: CompactString,
    value: Entry,
}

#[derive(Clone)]
pub(crate) struct OwnedCcVariableAccess<'a> {
    source: Source<'a>,
    seed: &'a [(CompactString, CcVariableCacheEntry<AnalysisValue>)],
    cache: Rc<RefCell<Cache>>,
}

pub(crate) struct OwnedCcVariableSequence {
    values: Arc<[AnalysisValue]>,
    next: usize,
}
impl Iterator for OwnedCcVariableSequence {
    type Item = Result<Entry, CcFeatureError>;
    fn next(&mut self) -> Option<Self::Item> {
        let value = self.values.get(self.next)?;
        self.next += 1;
        // Native Iterables.transform adapts only the visited element, before
        // entering the iteration body. Do not eagerly convert the whole list.
        Some(adapt(value))
    }
}

impl<'a> OwnedCcVariableAccess<'a> {
    pub(crate) fn original(variables: &'a CcToolchainVariables) -> Self {
        Self {
            source: Source::Native(variables),
            seed: &variables.model().cache,
            cache: Rc::new(RefCell::new(SmallMap::new())),
        }
    }

    pub(crate) fn fresh_overlay(variables: &'a CcToolchainVariables) -> Self {
        // CompileCommandLine's empty overwrite child delegates direct bindings
        // to its parent, but starts with its own empty dotted cache. There is no
        // observable Variables occurrence to allocate for this projection.
        Self {
            seed: &[],
            ..Self::original(variables)
        }
    }

    fn direct(&self, name: &str) -> Result<Option<Entry>, CcFeatureError> {
        let mut source = &self.source;
        loop {
            match source {
                Source::Scoped(scope) => {
                    if scope.name == name {
                        return Ok(present(scope.value.clone()));
                    }
                    source = &scope.parent;
                }
                Source::Native(variables) => {
                    let mut current = Some(*variables);
                    while let Some(variables) = current {
                        if let Some((_, value)) =
                            variables.model().locals.iter().find(|(key, _)| key == name)
                        {
                            // Local None shadows a parent's direct binding.
                            return adapt(value).map(present);
                        }
                        current = variables.model().parent.as_ref();
                    }
                    return Ok(None);
                }
            }
        }
    }

    fn cached(&self, name: &str) -> Result<Option<Entry>, CcFeatureError> {
        if let Some(value) = self.cache.borrow().get(name) {
            return Ok(Some(value.clone()));
        }
        let Some((_, seed)) = self.seed.iter().find(|(key, _)| key == name) else {
            return Ok(None);
        };
        let value = match seed {
            CcVariableCacheEntry::Value(value) => adapt(value)?,
            CcVariableCacheEntry::Sequence(values) => Entry::Sequence(values.to_vec().into()),
            CcVariableCacheEntry::Missing => Entry::Missing,
            CcVariableCacheEntry::Error(message) => Entry::Error(message.clone()),
        };
        self.cache.borrow_mut().insert(name.into(), value.clone());
        Ok(Some(value))
    }

    fn dotted(&self, name: &str, throw: bool) -> Result<Entry, CcFeatureError> {
        let mut prefix = name;
        let mut fields = Vec::new();
        let (mut value, struct_path) = loop {
            let Some((parent, field)) = prefix.rsplit_once('.') else {
                return Ok(Entry::Missing);
            };
            fields.push(field);
            prefix = parent;
            if let Some(value) = self.direct(prefix)? {
                break (value, prefix);
            }
        };
        for field in fields.into_iter().rev() {
            let raw = match &value {
                Entry::Value(raw) => structure(raw),
                _ => None,
            };
            let Some(raw) = raw else {
                return Ok(if throw {
                    Entry::Error(format!("Cannot expand variable '{struct_path}.{field}': variable '{struct_path}' is {}, expected structure", type_name(&value)).into())
                } else {
                    Entry::Missing
                });
            };
            // Unsupported conversions remain fatal, including in availability
            // guards. Only ordinary missing/type expansion failures are cached.
            value = raw
                .get(field)
                .map(adapt)
                .transpose()?
                .unwrap_or(Entry::Missing);
            if matches!(value, Entry::Missing) {
                return Ok(if throw {
                    Entry::Error(format!("Cannot expand variable '{struct_path}.{field}': structure {struct_path} doesn't have a field named '{field}'").into())
                } else {
                    Entry::Missing
                });
            }
        }
        Ok(value)
    }
}

fn structure(value: &AnalysisValue) -> Option<&SmallMap<CompactString, AnalysisValue>> {
    match value.kind() {
        AnalysisValueKind::Struct(fields) => Some(fields),
        AnalysisValueKind::Provider(provider)
            if provider.identity().user_id().is_some()
                || provider.identity().is_builtin("ToolchainInfo") =>
        {
            Some(provider.fields())
        }
        _ => None,
    }
}

fn adapt(value: &AnalysisValue) -> Result<Entry, CcFeatureError> {
    Ok(match value.kind() {
        AnalysisValueKind::None => Entry::Missing,
        AnalysisValueKind::String(_)
        | AnalysisValueKind::Boolean(_)
        | AnalysisValueKind::Artifact(_) => Entry::Value(value.dupe()),
        AnalysisValueKind::List(values) | AnalysisValueKind::Tuple(values) => {
            Entry::Sequence(values.into())
        }
        AnalysisValueKind::Dictionary(values) => {
            Entry::Sequence(values.iter().map(|(key, _)| key.dupe()).collect())
        }
        AnalysisValueKind::Depset(values) => Entry::Sequence(values.to_list().into()),
        _ if structure(value).is_some() => Entry::Value(value.dupe()),
        _ => {
            return Err(CcFeatureError(format!(
                "unsupported C++ variable value of type {}",
                value.value_type().as_str()
            )));
        }
    })
}
fn present(value: Entry) -> Option<Entry> {
    (!matches!(value, Entry::Missing)).then_some(value)
}
fn type_name(value: &Entry) -> &str {
    match value {
        Entry::Value(value) => match value.kind() {
            AnalysisValueKind::Boolean(_) => "boolean",
            AnalysisValueKind::String(_) => "string",
            AnalysisValueKind::Artifact(_) => "artifact",
            _ => value.value_type().as_str(),
        },
        Entry::Sequence(_) => "sequence",
        Entry::Missing => "NoneType",
        Entry::Error(_) => "invalid",
    }
}

impl CcVariableAccess for OwnedCcVariableAccess<'_> {
    type Value = Entry;
    type Sequence = OwnedCcVariableSequence;
    fn lookup(&mut self, name: &str, throw: bool) -> Result<Option<Entry>, CcFeatureError> {
        if let Some(value) = self.direct(name)? {
            return Ok(Some(value));
        }
        let entry = if !name.contains('.') {
            Entry::Missing
        } else if let Some(value) = self.cached(name)? {
            value
        } else {
            let entry = self.dotted(name, throw)?;
            self.cache.borrow_mut().insert(name.into(), entry.clone());
            entry
        };
        match entry {
            Entry::Missing if throw => Err(CcFeatureError(format!(
                "Cannot find variable named '{name}'."
            ))),
            Entry::Error(message) if throw => Err(CcFeatureError(message.to_string())),
            Entry::Missing | Entry::Error(_) => Ok(None),
            value => Ok(Some(value)),
        }
    }
    fn string(&mut self, value: &Entry, name: &str) -> Result<CompactString, CcFeatureError> {
        if let Entry::Value(value) = value {
            match value.kind() {
                AnalysisValueKind::String(value) => return Ok(value.into()),
                AnalysisValueKind::Boolean(value) => {
                    return Ok(if value { "1" } else { "0" }.into());
                }
                AnalysisValueKind::Artifact(value) => return Ok(value.path().as_ref().into()),
                _ => {}
            }
        }
        Err(CcFeatureError(format!(
            "Cannot expand variable '{name}': expected string, found {}",
            type_name(value)
        )))
    }
    fn sequence(&mut self, value: &Entry, name: &str) -> Result<Self::Sequence, CcFeatureError> {
        match value {
            Entry::Sequence(values) => Ok(OwnedCcVariableSequence {
                values: values.dupe(),
                next: 0,
            }),
            _ => Err(CcFeatureError(format!(
                "Cannot expand variable '{name}': expected sequence, found {}",
                type_name(value)
            ))),
        }
    }
    fn truthy(&mut self, value: &Entry) -> Result<bool, CcFeatureError> {
        Ok(match value {
            Entry::Value(value) => match value.kind() {
                AnalysisValueKind::Boolean(value) => value,
                AnalysisValueKind::String(value) => !value.is_empty(),
                _ => true, // Only File or native Structure can be adapted here.
            },
            Entry::Sequence(values) => !values.is_empty(),
            Entry::Missing => false,
            Entry::Error(message) => return Err(CcFeatureError(message.to_string())),
        })
    }
    fn with_binding(&self, name: &str, value: Entry) -> Self {
        Self {
            source: Source::Scoped(Rc::new(Scope {
                parent: self.source.clone(),
                name: name.into(),
                value,
            })),
            seed: &[],
            cache: Rc::new(RefCell::new(SmallMap::new())),
        }
    }
}

#[cfg(test)]
mod tests;
