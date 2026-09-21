//! Composition of the injected public cc_common facade and native query engine.
//! Bazel's public wrapper supplies its own builtins frame; callers of this
//! composed public surface are not subjected to the Java-internal restriction.
use slug_build_api_v2::cc_features::CcFeatureConfiguration;
use starlark::eval::Evaluator;
use starlark::values::dict::AllocImmutableDict;

use super::*;
use crate::cc_features::cc_feature_configuration_value;

fn configuration(value: Value<'_>) -> anyhow::Result<CcFeatureConfiguration> {
    cc_feature_configuration_value(value).ok_or_else(|| {
        anyhow::anyhow!(
            "feature_configuration must be FeatureConfiguration, got {}",
            value.get_type()
        )
    })
}
pub(crate) fn tool<'v>(
    config: Value<'v>,
    action: &str,
    heap: Heap<'v>,
) -> anyhow::Result<Value<'v>> {
    Ok(heap
        .alloc_str(configuration(config)?.get_tool_for_action(action)?.as_str())
        .to_value())
}
pub(crate) fn requirements<'v>(
    config: Value<'v>,
    action: &str,
    heap: Heap<'v>,
) -> anyhow::Result<Value<'v>> {
    Ok(heap.alloc(AllocImmutableList(
        configuration(config)?
            .get_execution_requirements(action)?
            .iter()
            .map(|v| v.as_str()),
    )))
}
pub(crate) fn action_enabled(config: Value<'_>, action: &str) -> anyhow::Result<bool> {
    Ok(configuration(config)?.action_is_enabled(action))
}
pub(crate) fn command_line<'v>(
    config: Value<'v>,
    action: &str,
    variables: Value<'v>,
    eval: &mut Evaluator<'v, '_, '_>,
) -> anyhow::Result<Value<'v>> {
    let values = configuration(config)?
        .get_command_line(action, &mut access::Access::new(variables, eval.heap())?)?;
    Ok(eval
        .heap()
        .alloc(AllocImmutableList(values.iter().map(|v| v.as_str()))))
}
pub(crate) fn environment<'v>(
    config: Value<'v>,
    action: &str,
    variables: Value<'v>,
    eval: &mut Evaluator<'v, '_, '_>,
) -> anyhow::Result<Value<'v>> {
    let values = configuration(config)?
        .get_environment_variables(action, &mut access::Access::new(variables, eval.heap())?)?;
    Ok(eval.heap().alloc(AllocImmutableDict(
        values.iter().map(|(k, v)| (k.as_str(), v.as_str())),
    )))
}
pub(crate) fn empty(heap: Heap<'_>) -> Value<'_> {
    // CcToolchainVariables.EmptyVariablesHolder.EMPTY is one native object.
    // This immutable identity constant is not a cache of semantic inputs.
    static OCCURRENCE: std::sync::OnceLock<CcVariablesOccurrence> = std::sync::OnceLock::new();
    heap.alloc(VariablesGen {
        kind: CcVariablesKind::Map,
        occurrence: OCCURRENCE.get_or_init(CcVariablesOccurrence::new).clone(),
        parent: None,
        names: Arc::from([]),
        values: Box::new([]),
        cache: Cache::Live(LiveCache(Mutex::new(SmallMap::new()))),
    })
}
