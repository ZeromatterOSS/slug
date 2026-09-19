//! Output groups retain typed depsets through construction, freeze and analysis.

use super::*;
use crate::subrule_invocation::AnalysisArtifactValue;

#[derive(Debug, Trace, Freeze, ProvidesStaticType, NoSerialize, Allocative)]
pub struct StarlarkOutputGroupInfoGen<V> {
    #[trace(unsafe_ignore)]
    #[freeze(identity)]
    names: Arc<[CompactString]>,
    values: Vec<V>,
}

pub type StarlarkOutputGroupInfo<'v> = StarlarkOutputGroupInfoGen<Value<'v>>;
type FrozenStarlarkOutputGroupInfo = StarlarkOutputGroupInfoGen<FrozenValue>;
starlark::starlark_complex_values!(StarlarkOutputGroupInfo);

fn sorted_fields<V>(
    fields: impl IntoIterator<Item = (CompactString, V)>,
) -> (Arc<[CompactString]>, Vec<V>) {
    let mut fields = fields.into_iter().collect::<Vec<_>>();
    // Java String.compareTo orders UTF-16 code units, including supplementary names.
    fields.sort_by(|(left, _), (right, _)| left.encode_utf16().cmp(right.encode_utf16()));
    let (names, values): (Vec<_>, Vec<_>) = fields.into_iter().unzip();
    (names.into(), values)
}

/// Empty depsets are untyped; nonempty depsets must carry Files. Their elements
/// were validated by the existing depset constructor, so no graph walk is needed.
fn empty_artifact_depset(value: Value<'_>, name: &str) -> starlark::Result<bool> {
    let element_type = match StarlarkDepset::from_value(value) {
        Some(starlark::__macro_refs::Either::Left(value)) => &value.element_type,
        Some(starlark::__macro_refs::Either::Right(value)) => &value.element_type,
        None => {
            return Err(starlark::Error::new_other(anyhow::anyhow!(
                "output group '{name}' must be a sequence or depset of Files"
            )));
        }
    };
    match element_type.as_deref() {
        None => Ok(true),
        Some("File") => Ok(false),
        Some(kind) => Err(starlark::Error::new_other(anyhow::anyhow!(
            "output group '{name}' must contain Files, got {kind}"
        ))),
    }
}

pub(super) fn construct<'v>(
    args: &Arguments<'v, '_>,
    eval: &mut Evaluator<'v, '_, '_>,
) -> starlark::Result<Value<'v>> {
    args.no_positional_args(eval.heap())?;
    let names = args.names_map()?;
    let (names, mut values) = sorted_fields(
        names
            .iter()
            .map(|(name, value)| (CompactString::new(name.as_str()), *value)),
    );
    let mut all_empty = true;
    for (name, value) in names.iter().zip(&mut values) {
        let sequence = ListRef::from_value(*value)
            .map(|list| list.iter().collect::<Vec<_>>())
            .or_else(|| TupleRef::from_value(*value).map(|tuple| tuple.iter().collect()));
        if let Some(sequence) = sequence {
            if sequence
                .iter()
                .any(|value| AnalysisArtifactValue::from_starlark(*value).is_none())
            {
                return Err(starlark::Error::new_other(anyhow::anyhow!(
                    "output group '{name}' sequence must contain Files"
                )));
            }
            // Delegate normalization, deduplication and occurrence ownership to
            // the same native depset constructor exposed to Starlark.
            let depset = eval.heap().alloc(AnalysisBuiltinCallable::new("depset"));
            *value = eval.eval_function(depset, &[*value], &[])?;
        }
        all_empty &= empty_artifact_depset(*value, name)?;
    }
    // Bazel's EmptyFiles drops supplied empty orders only when every group is
    // empty. A mixed provider must preserve its empty and nonempty depsets alike.
    if all_empty && !values.is_empty() {
        values.fill(empty_evaluator_depset(DepsetOrder::Default, eval));
    }
    Ok(eval
        .heap()
        .alloc_complex(StarlarkOutputGroupInfo { names, values }))
}

impl<'v> StarlarkOutputGroupInfo<'v> {
    pub fn fields_from_value(value: Value<'v>) -> Option<Vec<(CompactString, Value<'v>)>> {
        fn fields<'v, V: ValueLike<'v>>(
            value: &StarlarkOutputGroupInfoGen<V>,
        ) -> Vec<(CompactString, Value<'v>)> {
            value
                .names
                .iter()
                .cloned()
                .zip(value.values.iter().map(|value| value.to_value()))
                .collect()
        }
        match Self::from_value(value)? {
            starlark::__macro_refs::Either::Left(value) => Some(fields(value)),
            starlark::__macro_refs::Either::Right(value) => Some(fields(value)),
        }
    }

    /// Materialize groups already checked by the retained OutputGroupInfo owner.
    pub fn alloc(
        heap: &starlark::values::FrozenHeap,
        fields: impl IntoIterator<Item = (CompactString, FrozenValue)>,
    ) -> FrozenValue {
        let (names, mut values) = sorted_fields(fields);
        let all_empty = names.iter().zip(&values).all(|(name, value)| {
            empty_artifact_depset(value.to_value(), name)
                .expect("retained output groups contain artifact depsets")
        });
        if all_empty && !values.is_empty() {
            values.fill(alloc_starlark_depset(
                heap,
                AnalysisDepset::empty(DepsetOrder::Default),
                Vec::new(),
            ));
        }
        heap.alloc(StarlarkOutputGroupInfoGen { names, values })
    }
}

impl<V> fmt::Display for StarlarkOutputGroupInfoGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("OutputGroupInfo(...)")
    }
}

#[starlark_value(type = "OutputGroupInfo")]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for StarlarkOutputGroupInfoGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    type Canonical = FrozenStarlarkOutputGroupInfo;

    fn equals(&self, other: Value<'v>) -> starlark::Result<bool> {
        let Some(other) = StarlarkOutputGroupInfo::fields_from_value(other) else {
            return Ok(false);
        };
        if self.names.len() != other.len() {
            return Ok(false);
        }
        for ((name, value), (other_name, other_value)) in
            self.names.iter().zip(&self.values).zip(other)
        {
            if name != &other_name || !value.to_value().equals(other_value)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn write_hash(&self, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        "OutputGroupInfo instance".hash(hasher);
        for (name, value) in self.names.iter().zip(&self.values) {
            name.hash(hasher);
            value.to_value().get_hashed()?.hash().hash(hasher);
        }
        Ok(())
    }

    fn get_attr(&self, name: &str, _heap: Heap<'v>) -> Option<Value<'v>> {
        let index = self.names.iter().position(|candidate| candidate == name)?;
        Some(self.values[index].to_value())
    }

    fn dir_attr(&self) -> Vec<String> {
        self.names.iter().map(ToString::to_string).collect()
    }

    fn at(&self, key: Value<'v>, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        let name = key.unpack_str().ok_or_else(|| {
            starlark::Error::new_other(anyhow::anyhow!("output group names must be strings"))
        })?;
        self.get_attr(name, heap).ok_or_else(|| {
            starlark::Error::new_other(anyhow::anyhow!("output group '{name}' not present"))
        })
    }

    fn is_in(&self, key: Value<'v>) -> starlark::Result<bool> {
        Ok(key
            .unpack_str()
            .is_some_and(|name| self.names.iter().any(|candidate| candidate == name)))
    }

    fn iterate_collect(&self, heap: Heap<'v>) -> starlark::Result<Vec<Value<'v>>> {
        Ok(self
            .names
            .iter()
            .map(|name| heap.alloc_str(name).to_value())
            .collect())
    }

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn StarlarkStructuralValue>(self);
    }
}

impl<'v, V: ValueLike<'v>> StarlarkStructuralValue for StarlarkOutputGroupInfoGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    fn is_structurally_immutable(&self) -> bool {
        true
    }

    fn write_structural_hash(&self, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        self.write_hash(hasher)
    }
}

#[cfg(test)]
mod tests;
