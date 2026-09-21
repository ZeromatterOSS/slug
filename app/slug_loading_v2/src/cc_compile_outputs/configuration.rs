//! Phase-local facade: share the structural configuration, never publish a
//! second retained configuration identity or a coverage-only approximation.
use std::fmt;

use allocative::Allocative;
use slug_configuration_v2::SlugConfiguration;
use starlark::any::ProvidesStaticType;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::StarlarkValue;
use starlark::values::Value;
use starlark::values::starlark_value;

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
struct ConfigurationValue(SlugConfiguration);
starlark::starlark_simple_value!(ConfigurationValue);

impl fmt::Display for ConfigurationValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<configuration>")
    }
}

#[starlark_value(type = "configuration")]
impl<'v> StarlarkValue<'v> for ConfigurationValue {
    fn get_attr(&self, name: &str, _heap: Heap<'v>) -> Option<Value<'v>> {
        match name {
            "coverage_enabled" => self.0.coverage_enabled().ok().map(Value::new_bool),
            _ => None,
        }
    }
    fn dir_attr(&self) -> Vec<String> {
        vec!["coverage_enabled".to_owned()]
    }
}

pub fn alloc_configuration<'v>(
    heap: Heap<'v>,
    configuration: SlugConfiguration,
) -> anyhow::Result<Value<'v>> {
    configuration.coverage_enabled()?;
    Ok(heap.alloc_simple(ConfigurationValue(configuration)))
}

pub(super) fn unpack(value: Value<'_>) -> anyhow::Result<&SlugConfiguration> {
    ConfigurationValue::from_value(value)
        .map(|value| &value.0)
        .ok_or_else(|| anyhow::anyhow!("requires a genuine configuration"))
}
