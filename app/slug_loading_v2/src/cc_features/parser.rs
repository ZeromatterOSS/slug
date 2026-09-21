//! Bazel 9.2 CcToolchainFeaturesLib conversion, not feature expansion.
use compact_str::CompactString;
use slug_build_api_v2::cc_features::*;
use starlark::values::Value;
use starlark::values::list::ListRef;
use starlark::values::structs::StructRef;
use starlark::values::tuple::TupleRef;

use crate::provider::starlark_user_provider_fields;
use crate::subrule_invocation::AnalysisArtifactValue;

struct Info<'v> {
    fields: Vec<(CompactString, Value<'v>)>,
}
impl<'v> Info<'v> {
    fn new(value: Value<'v>) -> anyhow::Result<Self> {
        let fields = if let Some(info) = StructRef::from_value(value) {
            info.iter()
                .map(|(key, value)| (key.as_str().into(), value))
                .collect()
        } else if let Some((_, fields)) = starlark_user_provider_fields(value) {
            fields
        } else {
            anyhow::bail!("expected StarlarkInfo, got {}", value.get_type());
        };
        Ok(Self { fields })
    }
    fn tagged(value: Value<'v>, tag: &str) -> anyhow::Result<Self> {
        let info = Self::new(value)?;
        if info.field("type_name").and_then(Value::unpack_str) != Some(tag) {
            anyhow::bail!("Expected object of type '{tag}'");
        }
        Ok(info)
    }
    fn field(&self, name: &str) -> Option<Value<'v>> {
        self.fields
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| *value)
    }
    fn required(&self, name: &str) -> anyhow::Result<Value<'v>> {
        self.field(name)
            .ok_or_else(|| anyhow::anyhow!("Missing mandatory field '{name}'"))
    }
    fn string(&self, name: &str, required: bool) -> anyhow::Result<Option<CompactString>> {
        let value = if required {
            Some(self.required(name)?)
        } else {
            self.field(name)
        };
        value
            .filter(|v| !v.is_none())
            .map(|v| {
                v.unpack_str()
                    .map(Into::into)
                    .ok_or_else(|| anyhow::anyhow!("Field '{name}' is not of string type"))
            })
            .transpose()
    }
    fn nonempty(&self, name: &str) -> anyhow::Result<CompactString> {
        self.string(name, true)?
            .filter(|s| !s.is_empty())
            .ok_or_else(|| anyhow::anyhow!("'{name}' must be a nonempty string"))
    }
    fn boolean(&self, name: &str) -> anyhow::Result<bool> {
        self.required(name)?
            .unpack_bool()
            .ok_or_else(|| anyhow::anyhow!("Field '{name}' is not of bool type"))
    }
    fn list(&self, name: &str) -> anyhow::Result<Vec<Value<'v>>> {
        match self.field(name) {
            None => Ok(Vec::new()),
            Some(value) if value.is_none() => Ok(Vec::new()),
            Some(value) => sequence(value, name),
        }
    }
    fn strings(&self, name: &str) -> anyhow::Result<Vec<CompactString>> {
        self.list(name)?
            .into_iter()
            .map(|value| {
                value
                    .unpack_str()
                    .map(Into::into)
                    .ok_or_else(|| anyhow::anyhow!("'{name}' must contain strings"))
            })
            .collect()
    }
    fn set(&self, name: &str) -> anyhow::Result<CcSet<CompactString>> {
        Ok(self.strings(name)?.into_iter().collect())
    }
}

fn sequence<'v>(value: Value<'v>, name: &str) -> anyhow::Result<Vec<Value<'v>>> {
    if let Some(list) = ListRef::from_value(value) {
        return Ok(list.iter().collect());
    }
    if let Some(tuple) = TupleRef::from_value(value) {
        return Ok(tuple.iter().collect());
    }
    anyhow::bail!("'{name}' must be a sequence")
}

pub(super) fn parse(value: Value<'_>, directory: &str) -> anyhow::Result<CcToolchainFeatures> {
    // The top-level Java wrapper does not check provider key or type_name.
    let info = Info::new(value)?;
    let features = sequence(info.required("_features_DO_NOT_USE")?, "_features")?
        .into_iter()
        .map(feature)
        .collect::<anyhow::Result<Vec<_>>>()?;
    let os = info.nonempty("_exec_os_DO_NOT_USE")?;
    let os = CcExecOs::parse(&os)?;
    let actions = sequence(
        info.required("_action_configs_DO_NOT_USE")?,
        "_action_configs",
    )?
    .into_iter()
    .map(|value| action(value, os))
    .collect::<anyhow::Result<Vec<_>>>()?;
    let patterns = sequence(
        info.required("_artifact_name_patterns_DO_NOT_USE")?,
        "_artifact_name_patterns",
    )?
    .into_iter()
    .map(pattern)
    .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(CcToolchainFeatures::new(
        features, actions, patterns, directory, os,
    )?)
}

fn valid_name(name: &str, kind: &str) -> anyhow::Result<()> {
    if !name
        .bytes()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || b"_+-.".contains(&c))
    {
        anyhow::bail!(
            "A {kind}'s name must consist solely of lowercase ASCII letters, digits, '.', '_', '+', and '-', got '{name}'"
        );
    }
    Ok(())
}
fn feature(value: Value<'_>) -> anyhow::Result<CcFeature> {
    let info = Info::tagged(value, "feature")?;
    let name = info.string("name", true)?;
    let enabled = info.boolean("enabled")?;
    let name = name
        .filter(|name| !name.is_empty() || enabled)
        .ok_or_else(|| {
            anyhow::anyhow!("A feature must either have a nonempty 'name' field or be enabled.")
        })?;
    valid_name(&name, "feature")?;
    let flag_sets = info
        .list("flag_sets")?
        .into_iter()
        .map(|v| flag_set(v, None))
        .collect::<anyhow::Result<Vec<_>>>()?;
    let env_sets = info
        .list("env_sets")?
        .into_iter()
        .map(env_set)
        .collect::<anyhow::Result<Vec<_>>>()?;
    let requires = info
        .list("requires")?
        .into_iter()
        .map(|value| Info::tagged(value, "feature_set")?.set("features"))
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(CcFeature {
        name,
        enabled,
        flag_sets: flag_sets.into(),
        env_sets: env_sets.into(),
        requires: requires.into(),
        implies: info.strings("implies")?.into(),
        provides: info.strings("provides")?.into(),
    })
}

fn with_features(info: &Info<'_>) -> anyhow::Result<CcSet<CcWithFeatureSet>> {
    info.list("with_features")?
        .into_iter()
        .map(|value| {
            let info = Info::tagged(value, "with_feature_set")?;
            Ok(CcWithFeatureSet {
                features: info.set("features")?,
                not_features: info.set("not_features")?,
            })
        })
        .collect()
}
fn flag_set(value: Value<'_>, action: Option<&str>) -> anyhow::Result<CcFlagSet> {
    let info = Info::tagged(value, "flag_set")?;
    let mut actions = info.set("actions")?;
    if let Some(action) = action {
        if !actions.is_empty() {
            anyhow::bail!("flag_set in action_config '{action}' must not specify actions");
        }
        actions = [CompactString::new(action)].into_iter().collect();
    }
    let flag_groups = info
        .list("flag_groups")?
        .into_iter()
        .map(|v| flag_group(v, &mut Vec::new()))
        .collect::<anyhow::Result<Vec<_>>>()?;
    let with_features = with_features(&info)?;
    if action.is_none() && actions.is_empty() {
        anyhow::bail!(
            "A flag_set that belongs to a feature must have nonempty 'actions' parameter."
        );
    }
    Ok(CcFlagSet {
        actions,
        with_features,
        flag_groups: flag_groups.into(),
    })
}
fn flag_group<'v>(value: Value<'v>, ancestors: &mut Vec<Value<'v>>) -> anyhow::Result<CcFlagGroup> {
    if ancestors.iter().any(|ancestor| ancestor.ptr_eq(value)) {
        anyhow::bail!("cyclic flag_group metadata is unsupported");
    }
    ancestors.push(value);
    let result = flag_group_inner(value, ancestors);
    ancestors.pop();
    result
}
fn flag_group_inner<'v>(
    value: Value<'v>,
    ancestors: &mut Vec<Value<'v>>,
) -> anyhow::Result<CcFlagGroup> {
    let info = Info::tagged(value, "flag_group")?;
    let flags = info.strings("flags")?;
    let mut expandables = flags
        .iter()
        .map(|flag| Ok(CcExpandable::Flag(parse_cc_string(flag)?.into())))
        .collect::<anyhow::Result<Vec<_>>>()?;
    let groups = info.list("flag_groups")?;
    for group in &groups {
        expandables.push(CcExpandable::Group(flag_group(*group, ancestors)?));
    }
    if !flags.is_empty() && !groups.is_empty() {
        anyhow::bail!("flag_group must contain either a list of flags or a list of flag_groups.");
    }
    if expandables.is_empty() {
        anyhow::bail!("Both 'flags' and 'flag_groups' are empty.");
    }
    let iterate_over = info.string("iterate_over", true)?;
    let expand_if_available = info.string("expand_if_available", true)?;
    let expand_if_not_available = info.string("expand_if_not_available", true)?;
    let expand_if_true = info.string("expand_if_true", true)?;
    let expand_if_false = info.string("expand_if_false", true)?;
    let equal = info.required("expand_if_equal")?;
    let expand_if_equal = if equal.is_none() {
        None
    } else {
        let equal = Info::tagged(equal, "variable_with_value")?;
        Some(CcVariableWithValue {
            name: equal.nonempty("name")?,
            value: equal.nonempty("value")?,
        })
    };
    Ok(CcFlagGroup {
        expandables: expandables.into(),
        iterate_over,
        expand_if_available,
        expand_if_not_available,
        expand_if_true,
        expand_if_false,
        expand_if_equal,
    })
}
fn env_set(value: Value<'_>) -> anyhow::Result<CcEnvSet> {
    let info = Info::tagged(value, "env_set")?;
    let actions = info.set("actions")?;
    if actions.is_empty() {
        anyhow::bail!("actions parameter of env_set must be a nonempty list.");
    }
    let env_entries = info
        .list("env_entries")?
        .into_iter()
        .map(|value| {
            let info = Info::tagged(value, "env_entry")?;
            let key = info.nonempty("key")?;
            let value = info.nonempty("value")?;
            let expand_if_available = info.string("expand_if_available", false)?;
            Ok(CcEnvEntry {
                key,
                value: parse_cc_string(&value)?.into(),
                expand_if_available,
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(CcEnvSet {
        actions,
        env_entries: env_entries.into(),
        with_features: with_features(&info)?,
    })
}
fn tool(value: Value<'_>, os: CcExecOs) -> anyhow::Result<CcTool> {
    let info = Info::tagged(value, "tool")?;
    let path = info.string("path", false)?;
    let artifact = info
        .field("tool")
        .filter(|v| !v.is_none())
        .map(|value| {
            AnalysisArtifactValue::from_starlark(value)
                .map(|value| value.artifact().clone())
                .ok_or_else(|| anyhow::anyhow!("Field 'tool' is not of File type"))
        })
        .transpose()?;
    Ok(CcTool::new(
        path.as_deref(),
        artifact,
        with_features(&info)?,
        info.set("execution_requirements")?,
        os,
    )?)
}
fn action(value: Value<'_>, os: CcExecOs) -> anyhow::Result<CcActionConfig> {
    let info = Info::tagged(value, "action_config")?;
    let action_name = info.nonempty("action_name")?;
    valid_name(&action_name, "action_config")?;
    let enabled = info.boolean("enabled")?;
    let tools = info
        .list("tools")?
        .into_iter()
        .map(|v| tool(v, os))
        .collect::<anyhow::Result<Vec<_>>>()?;
    let flag_sets = info
        .list("flag_sets")?
        .into_iter()
        .map(|v| flag_set(v, Some(&action_name)))
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(CcActionConfig {
        action_name,
        enabled,
        tools: tools.into(),
        flag_sets: flag_sets.into(),
        implies: info.strings("implies")?.into(),
    })
}
fn pattern(value: Value<'_>) -> anyhow::Result<CcArtifactNamePattern> {
    let info = Info::tagged(value, "artifact_name_pattern")?;
    let category = info.nonempty("category_name")?;
    let extension = info.string("extension", true)?.unwrap_or_default();
    let prefix = info.string("prefix", true)?.unwrap_or_default();
    Ok(CcArtifactNamePattern::new(&category, &prefix, &extension)?)
}
