//! Phase-local facade: share the structural configuration, never publish a
//! second retained configuration identity or a coverage-only approximation.
use std::fmt;

use allocative::Allocative;
use slug_configuration_v2::SlugConfiguration;
use slug_configuration_v2::SlugConfigurationKind;
use starlark::any::ProvidesStaticType;
use starlark::environment::Methods;
use starlark::environment::MethodsBuilder;
use starlark::environment::MethodsStatic;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::StarlarkValue;
use starlark::values::Value;
use starlark::values::dict::AllocDict;
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
    fn get_methods() -> Option<&'static Methods> {
        static METHODS: MethodsStatic = MethodsStatic::new();
        METHODS.methods(configuration_methods)
    }
    fn get_attr(&self, name: &str, _heap: Heap<'v>) -> Option<Value<'v>> {
        match name {
            "coverage_enabled" => self.0.coverage_enabled().ok().map(Value::new_bool),
            _ => None,
        }
    }
    fn dir_attr(&self) -> Vec<String> {
        vec![
            "coverage_enabled".to_owned(),
            "default_shell_env".to_owned(),
        ]
    }
}

#[starlark_module]
fn configuration_methods(builder: &mut MethodsBuilder) {
    #[starlark(attribute)]
    fn default_shell_env<'v>(
        this: &ConfigurationValue,
        heap: Heap<'v>,
    ) -> anyhow::Result<Value<'v>> {
        let environment = this.0.configured_action_environment()?;
        // Bazel's TreeMap uses Java String order. Keep this borrowed sorting
        // scratch separate from the configuration's native canonical identity.
        let mut fixed = environment.fixed().iter().collect::<Vec<_>>();
        fixed.sort_unstable_by(|(left, _), (right, _)| {
            left.encode_utf16().cmp(right.encode_utf16())
        });
        // Java Map return conversion makes a fresh mutable dictionary. Only
        // these evaluator-owned strings survive the producer and sort scratch.
        Ok(heap.alloc(AllocDict(fixed)))
    }

    fn is_tool_configuration(this: &ConfigurationValue) -> anyhow::Result<bool> {
        Ok(this.0.kind() == SlugConfigurationKind::Exec)
    }

    fn stamp_binaries(
        this: &ConfigurationValue,
        eval: &mut Evaluator<'_, '_, '_>,
    ) -> anyhow::Result<bool> {
        crate::builtin_restriction::check_default_allowlist(
            eval,
            &crate::builtin_restriction::source_identities_for_evaluator(eval)?,
        )?;
        Ok(this.0.stamp_binaries()?)
    }

    fn is_sibling_repository_layout(
        this: &ConfigurationValue,
        eval: &mut Evaluator<'_, '_, '_>,
    ) -> anyhow::Result<bool> {
        let _ = this;
        crate::builtin_restriction::check_default_allowlist(
            eval,
            &crate::builtin_restriction::source_identities_for_evaluator(eval)?,
        )?;
        // Slug admits only the nonsibling layout. Bazel's switch belongs to
        // Starlark semantics, not CoreOptions; no untracked true mode is exposed.
        Ok(false)
    }
}

pub fn alloc_configuration<'v>(
    heap: Heap<'v>,
    configuration: SlugConfiguration,
) -> anyhow::Result<Value<'v>> {
    configuration.coverage_enabled()?;
    Ok(heap.alloc_simple(ConfigurationValue(configuration)))
}

pub(crate) fn unpack(value: Value<'_>) -> anyhow::Result<&SlugConfiguration> {
    ConfigurationValue::from_value(value)
        .map(|value| &value.0)
        .ok_or_else(|| anyhow::anyhow!("requires a genuine configuration"))
}

#[cfg(test)]
mod shell_environment_tests;
