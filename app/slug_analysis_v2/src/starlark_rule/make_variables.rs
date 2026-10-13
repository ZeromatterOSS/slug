//! Bazel's default make-variable suppliers, collected into one immutable dict.
use slug_configuration_v2::SlugConfiguration;
use slug_loading_v2::package::StarlarkRuleImplementation;
use starlark::values::FrozenHeap;

use super::*;

pub(super) fn prepare(
    heap: &FrozenHeap,
    configuration: Option<&SlugConfiguration>,
    platform_cpu: &str,
    root: &DerivedArtifactRoot,
    implementation: &StarlarkRuleImplementation,
    dependencies: &[PreparedDependency],
    groups: &ConfiguredExecGroupCollection,
) -> Result<FrozenValue, CompactString> {
    let configuration = configuration
        .ok_or_else(|| CompactString::from("ctx.var requires structural configuration"))?;
    let inputs = configuration
        .make_variable_inputs()
        .map_err(|error| CompactString::from(error.to_string()))?;

    // RuleContext900–939: prerequisites are visited in these fixed attribute
    // slots, preserving each slot's configured target encounter order.
    let mut providers = SmallMap::<&str, &str>::new();
    for name in [
        "toolchains",
        ":cc_toolchain",
        "$toolchains",
        "$cc_toolchain",
    ] {
        for schema in implementation.schema() {
            let declaration = schema.declaration_name();
            let late_bound = implementation
                .late_bound_rule_attributes()
                .any(|(attribute, _)| attribute == declaration);
            let matches = if late_bound {
                name.strip_prefix(':') == declaration.strip_prefix('_') && name.starts_with(':')
            } else {
                schema.query_name() == name
            };
            if !matches {
                continue;
            }
            for dependency in dependencies
                .iter()
                .filter(|dependency| !dependency.filtered && dependency.attribute == declaration)
            {
                if let Some(info) = dependency.providers.template_variable_info() {
                    for (key, value) in info.variables() {
                        providers.insert(key.as_str(), value.as_str());
                    }
                }
            }
        }
    }

    // Resolved-context order is not yet proven equivalent. Admit only results
    // independent of that order; refusing conflicts also applies when defines
    // would mask them. No caller, repository, or expected-effect admission.
    let mut resolved = SmallMap::<&str, &str>::new();
    for group in groups.rows() {
        let Some(context) = group.action_context().toolchain() else {
            continue;
        };
        for row in context.rows() {
            let Some(info) = row
                .selected()
                .and_then(|selection| selection.template_variable_info())
            else {
                continue;
            };
            for (key, value) in info.variables() {
                let key = key.as_str();
                let value = value.as_str();
                if resolved.get(key).is_some_and(|previous| *previous != value) {
                    return Err(format!(
                        "ctx.var conflicting resolved-toolchain variable `{key}` is unsupported until supplier encounter order is modeled"
                    ).into());
                }
                resolved.insert(key, value);
            }
        }
    }
    for (key, value) in resolved {
        providers.insert(key, value);
    }
    // MakeVariableSupplier79–84 sorts the combined provider supplier globally,
    // after last-wins collection, rather than sorting each provider separately.
    let mut provider_entries = providers.into_iter().collect::<Vec<_>>();
    provider_entries.sort_unstable_by(|(a, _), (b, _)| a.encode_utf16().cmp(b.encode_utf16()));

    // ConfigurationMakeVariableContext75–82 merges reverse-priority suppliers
    // without moving an existing key. Package vardef has no admitted producer:
    // Loading rejects that unsupported package input before analysis.
    let mut variables = SmallMap::new();
    variables.insert("TARGET_CPU", platform_cpu);
    variables.insert("COMPILATION_MODE", inputs.compilation_mode());
    variables.insert("BINDIR", root.path());
    variables.insert("GENDIR", root.path());
    for (key, value) in provider_entries {
        variables.insert(key, value);
    }
    for (key, value) in inputs.defines() {
        variables.insert(key, value);
    }
    Ok(heap.alloc(AllocDict(variables)))
}
