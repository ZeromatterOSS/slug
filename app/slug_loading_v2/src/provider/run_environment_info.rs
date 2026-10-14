//! Immutable native run environment with caller-owned mutable field copies.
use slug_build_api_v2::RunEnvironmentInfo as NativeRunEnvironmentInfo;
use starlark::values::FrozenHeap;
use starlark::values::dict::AllocDict;
use starlark::values::list::AllocList;
use starlark::values::range::Range;

use super::*;

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub(crate) struct RunEnvironmentInfo;
starlark::starlark_simple_value!(RunEnvironmentInfo);

impl fmt::Display for RunEnvironmentInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<function RunEnvironmentInfo>")
    }
}

#[starlark_value(type = "Provider")]
impl<'v> StarlarkValue<'v> for RunEnvironmentInfo {
    fn equals(&self, other: Value<'v>) -> starlark::Result<bool> {
        Ok(Self::from_value(other).is_some())
    }

    fn write_hash(&self, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        "RunEnvironmentInfo constructor".hash(hasher);
        Ok(())
    }

    fn invoke(
        &self,
        _me: Value<'v>,
        args: &Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        construct(args, eval)
    }
}

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct StarlarkRunEnvironmentInfo(NativeRunEnvironmentInfo);
starlark::starlark_simple_value!(StarlarkRunEnvironmentInfo);

impl fmt::Display for StarlarkRunEnvironmentInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // NativeInfo.repr enumerates field names lexicographically.
        f.write_str("struct(environment = {")?;
        for (index, (key, value)) in self.0.environment().iter().enumerate() {
            if index != 0 {
                f.write_str(", ")?;
            }
            quoted(f, key)?;
            f.write_str(": ")?;
            quoted(f, value)?;
        }
        f.write_str("}, inherited_environment = [")?;
        for (index, value) in self.0.inherited_environment().iter().enumerate() {
            if index != 0 {
                f.write_str(", ")?;
            }
            quoted(f, value)?;
        }
        f.write_str("])")
    }
}

// Pinned Bazel Printer.java239–258: native repr preserves valid Unicode and
// quotes control characters independently of the Engine's string repr policy.
fn quoted(f: &mut fmt::Formatter<'_>, value: &str) -> fmt::Result {
    f.write_str("\"")?;
    for character in value.chars() {
        match character {
            '"' => f.write_str("\\\"")?,
            '\\' => f.write_str("\\\\")?,
            '\r' => f.write_str("\\r")?,
            '\n' => f.write_str("\\n")?,
            '\t' => f.write_str("\\t")?,
            c if c < ' ' => write!(f, "\\x{:02x}", c as u32)?,
            c => write!(f, "{c}")?,
        }
    }
    f.write_str("\"")
}

impl StarlarkStructuralValue for StarlarkRunEnvironmentInfo {
    fn is_structurally_immutable(&self) -> bool {
        true
    }

    fn write_structural_hash(&self, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        "RunEnvironmentInfo".hash(hasher);
        self.0.hash_fields(hasher);
        Ok(())
    }
}

#[starlark_value(type = "RunEnvironmentInfo")]
impl<'v> StarlarkValue<'v> for StarlarkRunEnvironmentInfo {
    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn StarlarkStructuralValue>(self);
    }

    fn equals(&self, other: Value<'v>) -> starlark::Result<bool> {
        Ok(Self::from_value(other).is_some_and(|other| self.0.field_eq(&other.0)))
    }

    fn write_hash(&self, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        self.write_structural_hash(hasher)
    }

    fn get_attr(&self, name: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match name {
            "environment" => Some(
                heap.alloc(AllocDict(
                    self.0
                        .environment()
                        .iter()
                        .map(|(key, value)| (key.as_str(), value.as_str())),
                )),
            ),
            "inherited_environment" => Some(
                heap.alloc(AllocList(
                    self.0
                        .inherited_environment()
                        .iter()
                        .map(CompactString::as_str),
                )),
            ),
            _ => None,
        }
    }

    fn dir_attr(&self) -> Vec<String> {
        vec!["environment".to_owned(), "inherited_environment".to_owned()]
    }

    fn has_attr(&self, name: &str, _heap: Heap<'v>) -> bool {
        matches!(name, "environment" | "inherited_environment")
    }
}

fn invalid(message: &str) -> starlark::Error {
    starlark::Error::new_other(anyhow::anyhow!("{message}"))
}

fn construct<'v>(
    args: &Arguments<'v, '_>,
    eval: &mut Evaluator<'v, '_, '_>,
) -> starlark::Result<Value<'v>> {
    // RunEnvironmentInfoApi.java83–117; RunEnvironmentInfo.java97–104.
    let mut positions = args.positions(eval.heap())?;
    let positions = [positions.next(), positions.next(), positions.next()];
    let names = args.names_map()?;
    if positions[2].is_some() {
        return Err(invalid(
            "RunEnvironmentInfo accepts at most two positional arguments",
        ));
    }
    for name in names.keys() {
        if !matches!(name.as_str(), "environment" | "inherited_environment") {
            return Err(invalid(&format!(
                "RunEnvironmentInfo got unexpected argument `{name}`"
            )));
        }
    }
    let bind = |index: usize, name: &str| -> starlark::Result<Option<Value<'v>>> {
        let position = positions[index];
        let named = names.get(name).copied();
        if position.is_some() && named.is_some() {
            return Err(invalid(&format!(
                "RunEnvironmentInfo {name} specified twice"
            )));
        }
        Ok(position.or(named))
    };
    let environment = bind(0, "environment")?;
    let inherited = bind(1, "inherited_environment")?;
    let mut entries = SmallMap::new();
    if let Some(environment) = environment {
        let environment = DictRef::from_value(environment)
            .ok_or_else(|| invalid("RunEnvironmentInfo environment must be a dict"))?;
        entries = SmallMap::with_capacity(environment.len());
        for (key, value) in environment.iter() {
            let key = key
                .unpack_str()
                .ok_or_else(|| invalid("RunEnvironmentInfo environment keys must be strings"))?;
            let value = value
                .unpack_str()
                .ok_or_else(|| invalid("RunEnvironmentInfo environment values must be strings"))?;
            entries.insert(CompactString::new(key), CompactString::new(value));
        }
    }
    let inherited = match inherited {
        Some(value) => {
            let values = if let Some(list) = ListRef::from_value(value) {
                list.content()
            } else if let Some(tuple) = TupleRef::from_value(value) {
                tuple.content()
            } else if let Some(range) = Range::from_value(value) {
                if range.to_bool() {
                    return Err(invalid(
                        "RunEnvironmentInfo inherited_environment elements must be strings",
                    ));
                }
                &[]
            } else {
                return Err(invalid(
                    "RunEnvironmentInfo inherited_environment must be a list, tuple, or empty range",
                ));
            };
            values
                .iter()
                .map(|value| {
                    value.unpack_str().map(CompactString::new).ok_or_else(|| {
                        invalid("RunEnvironmentInfo inherited_environment elements must be strings")
                    })
                })
                .collect::<starlark::Result<Arc<[_]>>>()?
        }
        None => Arc::from([]),
    };
    Ok(eval
        .heap()
        .alloc_simple(StarlarkRunEnvironmentInfo(NativeRunEnvironmentInfo::new(
            entries, inherited,
        ))))
}

/// Borrow immutable raw fields without allocating getter copies.
pub fn run_environment_info_ref(value: Value<'_>) -> Option<&NativeRunEnvironmentInfo> {
    StarlarkRunEnvironmentInfo::from_value(value).map(|value| &value.0)
}

pub fn run_environment_info(value: Value<'_>) -> Option<NativeRunEnvironmentInfo> {
    run_environment_info_ref(value).cloned()
}

pub fn alloc_frozen_run_environment_info(
    heap: &FrozenHeap,
    info: NativeRunEnvironmentInfo,
) -> FrozenValue {
    heap.alloc(StarlarkRunEnvironmentInfo(info))
}

#[cfg(test)]
mod tests;
