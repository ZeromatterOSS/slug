//! Bazel's Starlark-created FeatureFlagInfo has a fresh native occurrence.
use slug_build_api_v2::FeatureFlagInfo;
use starlark::values::FrozenHeap;

use super::*;

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub(crate) struct FeatureFlagInfoConstructor;
starlark::starlark_simple_value!(FeatureFlagInfoConstructor);

impl fmt::Display for FeatureFlagInfoConstructor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<function FeatureFlagInfo>")
    }
}

#[starlark_value(type = "Provider")]
impl<'v> StarlarkValue<'v> for FeatureFlagInfoConstructor {
    fn equals(&self, other: Value<'v>) -> starlark::Result<bool> {
        Ok(Self::from_value(other).is_some())
    }

    fn write_hash(&self, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        "FeatureFlagInfo constructor".hash(hasher);
        Ok(())
    }

    fn invoke(
        &self,
        _me: Value<'v>,
        args: &Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let positional = args.positions(eval.heap())?.collect::<Vec<_>>();
        let named = args.names_map()?;
        if positional.len() > 1 || named.keys().any(|name| name.as_str() != "value") {
            return Err(starlark::Error::new_other(anyhow::anyhow!(
                "FeatureFlagInfo requires one value string argument"
            )));
        }
        if !positional.is_empty() && named.contains_key("value") {
            return Err(starlark::Error::new_other(anyhow::anyhow!(
                "FeatureFlagInfo value specified twice"
            )));
        }
        let value = positional
            .first()
            .copied()
            .or_else(|| named.get("value").copied())
            .ok_or_else(|| {
                starlark::Error::new_other(anyhow::anyhow!(
                    "FeatureFlagInfo missing required value argument"
                ))
            })?;
        let value = value.unpack_str().ok_or_else(|| {
            starlark::Error::new_other(anyhow::anyhow!("FeatureFlagInfo value must be a string"))
        })?;
        Ok(eval
            .heap()
            .alloc_simple(FeatureFlagInfoValue(FeatureFlagInfo::new(value))))
    }
}

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct FeatureFlagInfoValue(FeatureFlagInfo);
starlark::starlark_simple_value!(FeatureFlagInfoValue);

impl fmt::Display for FeatureFlagInfoValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "struct(error = None, value = {:?})", self.0.value())
    }
}

#[starlark_value(type = "FeatureFlagInfo")]
impl<'v> StarlarkValue<'v> for FeatureFlagInfoValue {
    fn equals(&self, other: Value<'v>) -> starlark::Result<bool> {
        Ok(Self::from_value(other).is_some_and(|other| self.0 == other.0))
    }

    fn write_hash(&self, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        self.0.hash(hasher);
        Ok(())
    }

    fn get_attr(&self, name: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match name {
            "value" => Some(heap.alloc_str(self.0.value()).to_value()),
            "error" => Some(Value::new_none()),
            _ => None,
        }
    }

    fn dir_attr(&self) -> Vec<String> {
        vec!["error".to_owned(), "value".to_owned()]
    }

    fn get_methods() -> Option<&'static Methods> {
        static METHODS: MethodsStatic = MethodsStatic::new();
        METHODS.methods(feature_flag_info_methods)
    }
}

#[starlark_module]
fn feature_flag_info_methods(builder: &mut MethodsBuilder) {
    fn is_valid_value(this: Value<'_>, value: &str) -> anyhow::Result<bool> {
        let _ = (this, value);
        Ok(true)
    }
}

pub fn feature_flag_info(value: Value<'_>) -> Option<FeatureFlagInfo> {
    FeatureFlagInfoValue::from_value(value).map(|value| value.0.dupe())
}

pub fn alloc_frozen_feature_flag_info(heap: &FrozenHeap, info: FeatureFlagInfo) -> FrozenValue {
    heap.alloc(FeatureFlagInfoValue(info))
}
