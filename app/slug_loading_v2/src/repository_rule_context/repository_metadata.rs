//! Repository return metadata, independent of the defining rule or caller.
//!
//! Bazel 9.2 StarlarkRepositoryContext.repoMetadata validates the dictionary
//! keys at construction but retains the dictionary itself. Snapshot its values
//! only when RepositoryFetchFunction classifies the implementation's return.

use std::fmt;
use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use slug_identity_v2::CanonicalLabel;
use starlark::any::ProvidesStaticType;
use starlark::values::Freeze;
use starlark::values::FrozenValue;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::StarlarkValue;
use starlark::values::Trace;
use starlark::values::Value;
use starlark::values::ValueIdentity;
use starlark::values::ValueLike;
use starlark::values::dict::AllocDict;
use starlark::values::dict::DictRef;
use starlark::values::list::ListRef;
use starlark::values::starlark_value;
use starlark::values::tuple::TupleRef;

use crate::starlark_label::StarlarkLabel;

const MAX_METADATA_NODES: usize = 4096;
const MAX_METADATA_DEPTH: usize = 32;
const MAX_METADATA_STRING_BYTES: usize = 1024 * 1024;

/// A retained result fact, not a source capability or repository-cache grant.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct RepositoryMetadata {
    reproducible: bool,
    attrs_for_reproducibility: Arc<[(CompactString, RepositoryMetadataValue)]>,
}

impl RepositoryMetadata {
    pub fn nonreproducible() -> Self {
        Self {
            reproducible: false,
            attrs_for_reproducibility: Arc::from([]),
        }
    }

    pub fn reproducible(&self) -> bool {
        self.reproducible
    }

    pub fn attrs_for_reproducibility(&self) -> &[(CompactString, RepositoryMetadataValue)] {
        &self.attrs_for_reproducibility
    }
}

/// The supported retained attribute-value family. Other values fail explicitly
/// rather than contributing their display text to semantic identity.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub enum RepositoryMetadataValue {
    None,
    Bool(bool),
    Int(i32),
    String(CompactString),
    Label(CanonicalLabel),
    List(Arc<[RepositoryMetadataValue]>),
    Tuple(Arc<[RepositoryMetadataValue]>),
    Dict(Arc<[(RepositoryMetadataKey, RepositoryMetadataValue)]>),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Allocative)]
pub enum RepositoryMetadataKey {
    String(CompactString),
    Label(CanonicalLabel),
}

#[derive(Debug, Trace, Freeze, ProvidesStaticType, NoSerialize, Allocative)]
struct StarlarkRepositoryMetadataGen<V> {
    reproducible: bool,
    attrs: V,
}

type StarlarkRepositoryMetadata<'v> = StarlarkRepositoryMetadataGen<Value<'v>>;
type FrozenStarlarkRepositoryMetadata = StarlarkRepositoryMetadataGen<FrozenValue>;
starlark::starlark_complex_values!(StarlarkRepositoryMetadata);

impl<V> fmt::Display for StarlarkRepositoryMetadataGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<repo_metadata>")
    }
}

#[starlark_value(type = "repo_metadata")]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for StarlarkRepositoryMetadataGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    type Canonical = FrozenStarlarkRepositoryMetadata;
}

pub(super) fn new_repository_metadata<'v>(
    reproducible: bool,
    attrs: Option<Value<'v>>,
    heap: Heap<'v>,
) -> anyhow::Result<Value<'v>> {
    let attrs = attrs.unwrap_or_else(|| heap.alloc(AllocDict(std::iter::empty::<(&str, Value)>())));
    let dict = string_key_dict(attrs, "attrs_for_reproducibility")?;
    if reproducible && !dict.is_empty() {
        anyhow::bail!("attrs_for_reproducibility can only be specified if reproducible is False");
    }
    Ok(heap.alloc(StarlarkRepositoryMetadata {
        reproducible,
        attrs,
    }))
}

/// Bazel 9.2 RepositoryFetchFunction:678-686 retains a string-key legacy dict
/// as suggested attrs, retains repo_metadata, and ignores other return types.
pub(super) fn classify_repository_return(value: Value<'_>) -> anyhow::Result<RepositoryMetadata> {
    let (reproducible, attrs) =
        if let Some(metadata) = StarlarkRepositoryMetadata::from_value(value) {
            match metadata {
                starlark::__macro_refs::Either::Left(metadata) => {
                    (metadata.reproducible, metadata.attrs)
                }
                starlark::__macro_refs::Either::Right(metadata) => {
                    (metadata.reproducible, metadata.attrs.to_value())
                }
            }
        } else if DictRef::from_value(value).is_some() {
            (false, value)
        } else {
            return Ok(RepositoryMetadata::nonreproducible());
        };
    let dict = string_key_dict(attrs, "repository rule return value")?;
    let mut projection = MetadataProjection::default();
    let mut attributes = Vec::with_capacity(dict.len());
    for (name, value) in dict.iter() {
        let name = name.unpack_str().expect("dictionary keys validated above");
        projection.node()?;
        projection.string(name.len())?;
        attributes.push((CompactString::new(name), projection.value(value, 0)?));
    }
    attributes.sort_unstable_by(|left, right| left.0.cmp(&right.0));
    Ok(RepositoryMetadata {
        reproducible,
        attrs_for_reproducibility: attributes.into(),
    })
}

fn string_key_dict<'v>(value: Value<'v>, argument: &str) -> anyhow::Result<DictRef<'v>> {
    let dict = DictRef::from_value(value)
        .ok_or_else(|| anyhow::anyhow!("{argument} must be a dictionary with string keys"))?;
    if dict.len() > MAX_METADATA_NODES {
        anyhow::bail!("repository metadata exceeds its node limit");
    }
    if dict.iter().any(|(key, _)| key.unpack_str().is_none()) {
        anyhow::bail!("{argument} must be a dictionary with string keys");
    }
    Ok(dict)
}

#[derive(Default)]
struct MetadataProjection<'v> {
    nodes: usize,
    string_bytes: usize,
    active: Vec<ValueIdentity<'v>>,
}

impl<'v> MetadataProjection<'v> {
    fn node(&mut self) -> anyhow::Result<()> {
        if self.nodes == MAX_METADATA_NODES {
            anyhow::bail!("repository metadata exceeds its node limit");
        }
        self.nodes += 1;
        Ok(())
    }

    fn string(&mut self, bytes: usize) -> anyhow::Result<()> {
        if bytes > MAX_METADATA_STRING_BYTES - self.string_bytes {
            anyhow::bail!("repository metadata exceeds its string-byte limit");
        }
        self.string_bytes += bytes;
        Ok(())
    }

    fn label(&mut self, label: &CanonicalLabel) -> anyhow::Result<()> {
        self.string(label.package().repo().as_str().len())?;
        self.string(label.package().package().as_str().len())?;
        self.string(label.target().as_str().len())
    }

    fn value(&mut self, value: Value<'v>, depth: usize) -> anyhow::Result<RepositoryMetadataValue> {
        if depth > MAX_METADATA_DEPTH {
            anyhow::bail!("repository metadata exceeds its depth limit");
        }
        self.node()?;
        if value.is_none() {
            return Ok(RepositoryMetadataValue::None);
        }
        if let Some(value) = value.unpack_bool() {
            return Ok(RepositoryMetadataValue::Bool(value));
        }
        if let Some(value) = value.unpack_i32() {
            return Ok(RepositoryMetadataValue::Int(value));
        }
        if let Some(value) = value.unpack_str() {
            self.string(value.len())?;
            return Ok(RepositoryMetadataValue::String(value.into()));
        }
        if let Some(label) = StarlarkLabel::from_value(value) {
            self.label(label.canonical())?;
            return Ok(RepositoryMetadataValue::Label(label.canonical().clone()));
        }
        let identity = value.identity();
        if self.active.contains(&identity) {
            anyhow::bail!("cyclic repository metadata is unsupported");
        }
        self.active.push(identity);
        let result = self.container(value, depth);
        self.active.pop();
        result
    }

    fn container(
        &mut self,
        value: Value<'v>,
        depth: usize,
    ) -> anyhow::Result<RepositoryMetadataValue> {
        if let Some(list) = ListRef::from_value(value) {
            return list
                .iter()
                .map(|item| self.value(item, depth + 1))
                .collect::<anyhow::Result<Vec<_>>>()
                .map(|items| RepositoryMetadataValue::List(items.into()));
        }
        if let Some(tuple) = TupleRef::from_value(value) {
            return tuple
                .iter()
                .map(|item| self.value(item, depth + 1))
                .collect::<anyhow::Result<Vec<_>>>()
                .map(|items| RepositoryMetadataValue::Tuple(items.into()));
        }
        if let Some(dict) = DictRef::from_value(value) {
            let mut items = Vec::with_capacity(dict.len().min(MAX_METADATA_NODES));
            for (key, item) in dict.iter() {
                self.node()?;
                let key = if let Some(key) = key.unpack_str() {
                    self.string(key.len())?;
                    RepositoryMetadataKey::String(key.into())
                } else if let Some(label) = StarlarkLabel::from_value(key) {
                    self.label(label.canonical())?;
                    RepositoryMetadataKey::Label(label.canonical().clone())
                } else {
                    anyhow::bail!(
                        "repository metadata nested dictionary keys support strings or Labels"
                    );
                };
                items.push((key, self.value(item, depth + 1)?));
            }
            items.sort_unstable_by(|left, right| left.0.cmp(&right.0));
            return Ok(RepositoryMetadataValue::Dict(items.into()));
        }
        anyhow::bail!(
            "repository metadata value type '{}' is unsupported",
            value.get_type()
        )
    }
}

#[cfg(test)]
mod tests {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::Hash;
    use std::hash::Hasher;

    use starlark::environment::GlobalsBuilder;
    use starlark::environment::Module;
    use starlark::eval::Evaluator;
    use starlark::starlark_module;
    use starlark::syntax::AstModule;
    use starlark::syntax::Dialect;

    use super::*;

    #[starlark_module]
    fn metadata_globals(builder: &mut GlobalsBuilder) {
        fn metadata<'v>(
            #[starlark(require = named, default = false)] reproducible: bool,
            #[starlark(require = named)] attrs_for_reproducibility: Option<Value<'v>>,
            eval: &mut Evaluator<'v, '_, '_>,
        ) -> anyhow::Result<Value<'v>> {
            new_repository_metadata(reproducible, attrs_for_reproducibility, eval.heap())
        }
    }

    fn evaluate(source: &str) -> anyhow::Result<RepositoryMetadata> {
        let module = Module::new();
        let globals = GlobalsBuilder::standard().with(metadata_globals).build();
        let ast = AstModule::parse(
            "independent_metadata.bzl",
            source.to_owned(),
            &Dialect::Bazel,
        )
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        Evaluator::new(&module)
            .eval_module(ast, &globals)
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        classify_repository_return(module.get("result").expect("fixture result"))
    }

    #[test]
    fn metadata_constructor_validates_default_and_opaque_return() {
        let default = evaluate("result = metadata()\n").unwrap();
        assert_eq!(default, RepositoryMetadata::nonreproducible());
        let reproducible = evaluate("result = metadata(reproducible=True)\n").unwrap();
        assert!(reproducible.reproducible());
        assert!(reproducible.attrs_for_reproducibility().is_empty());
        for source in [
            "result = metadata(reproducible=True, attrs_for_reproducibility={'x': 1})\n",
            "result = metadata(attrs_for_reproducibility={1: 'x'})\n",
            "result = metadata(attrs_for_reproducibility=None)\n",
            "result = metadata(reproducible='True')\n",
            "result = metadata(True)\n",
        ] {
            assert!(evaluate(source).is_err(), "{source}");
        }
        assert!(evaluate("m = metadata()\nresult = m.reproducible\n").is_err());
    }

    #[test]
    fn repository_return_classification_retains_legacy_attrs_and_ignores_other_types() {
        for source in [
            "result = None\n",
            "result = 42\n",
            "result = 'other'\n",
            "result = [1, 2]\n",
        ] {
            assert_eq!(
                evaluate(source).unwrap(),
                RepositoryMetadata::nonreproducible()
            );
        }
        let legacy = evaluate("result = {'version': 'changed', 'flags': ['-v']}\n").unwrap();
        let opaque = evaluate("result = metadata(attrs_for_reproducibility={'flags': ['-v'], 'version': 'changed'})\n").unwrap();
        assert_eq!(legacy, opaque);
        assert!(!legacy.reproducible());
        assert_eq!(legacy.attrs_for_reproducibility().len(), 2);
        assert!(evaluate("result = {1: 'invalid'}\n").is_err());
    }

    #[test]
    fn metadata_snapshots_dictionary_at_return_and_survives_freezing() {
        let mutated = evaluate("attrs = {'version': 'before'}\nresult = metadata(attrs_for_reproducibility=attrs)\nattrs['version'] = 'after'\n").unwrap();
        assert_eq!(
            mutated,
            evaluate("result = {'version': 'after'}\n").unwrap()
        );
        let module = Module::new();
        let value = new_repository_metadata(true, None, module.heap()).unwrap();
        module.set("result", value);
        let frozen = module.freeze().unwrap();
        let result = frozen.get("result").unwrap();
        assert!(
            classify_repository_return(result.value())
                .unwrap()
                .reproducible()
        );
    }

    #[test]
    fn metadata_structural_equality_and_hash_preserve_changes_and_restore() {
        let a = evaluate("result = {'nested': {'b': (True, None), 'a': ['x', 1]}}\n").unwrap();
        let restored = evaluate("result = metadata(attrs_for_reproducibility={'nested': {'a': ['x', 1], 'b': (True, None)}})\n").unwrap();
        let b = evaluate("result = {'nested': {'b': (True, None), 'a': ['y', 1]}}\n").unwrap();
        let tuple = evaluate("result = {'nested': {'b': (True, None), 'a': ('x', 1)}}\n").unwrap();
        assert_eq!(a, restored);
        assert_ne!(a, b);
        assert_ne!(a, tuple);
        let hash = |metadata: &RepositoryMetadata| {
            let mut hasher = DefaultHasher::new();
            metadata.hash(&mut hasher);
            hasher.finish()
        };
        assert_eq!(hash(&a), hash(&restored));
        assert_ne!(hash(&a), hash(&b));
    }

    #[test]
    fn metadata_projection_refuses_cycles_limits_and_unmodeled_values() {
        for source in [
            "items = []\nitems.append(items)\nresult = {'items': items}\n",
            "result = {'value': [0] * 4097}\n",
            "result = {'value': 1.25}\n",
            "result = {'value': {1: 'unsupported key'}}\n",
        ] {
            assert!(evaluate(source).is_err(), "{source}");
        }
        let huge = format!(
            "result = {{'value': '{}'}}\n",
            "x".repeat(MAX_METADATA_STRING_BYTES)
        );
        assert!(evaluate(&huge).is_err());
        let nested = format!(
            "result = {{'value': {}0{}}}\n",
            "[".repeat(MAX_METADATA_DEPTH + 2),
            "]".repeat(MAX_METADATA_DEPTH + 2)
        );
        assert!(evaluate(&nested).is_err());
    }
}
