//! Native label_flag declarations. Selection and alias traversal belong to Analysis.

use super::*;

/// Source-normalized native metadata; no selected target or evaluator state.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct LabelFlagDeclaration {
    declaration: BuildSettingDeclaration,
    help: CompactString,
    help_explicit: bool,
}

impl LabelFlagDeclaration {
    pub fn declaration(&self) -> &BuildSettingDeclaration {
        &self.declaration
    }

    pub fn default(&self) -> &CanonicalLabel {
        let BuildSettingDefault::Label(label) = self.declaration.default() else {
            unreachable!("native label_flag has a typed label default")
        };
        label
    }

    pub fn help(&self) -> &str {
        &self.help
    }

    pub fn scope(&self) -> BuildSettingScope {
        self.declaration.scope()
    }

    pub(super) fn populate_native_attributes(&self, values: &mut [NativeAttributeValue]) {
        let class = NativeRuleClass::LabelFlag;
        for (name, provenance, value) in [
            (
                ":alias",
                AttributeProvenance::Implicit,
                CoercedAttributeValue::Label(self.default().clone()),
            ),
            (
                "scope",
                if self.scope() == BuildSettingScope::Default {
                    AttributeProvenance::Default
                } else {
                    AttributeProvenance::Explicit
                },
                CoercedAttributeValue::String("universal".into()),
            ),
            (
                "build_setting_default",
                AttributeProvenance::Explicit,
                CoercedAttributeValue::Label(self.default().clone()),
            ),
            (
                "help",
                if self.help_explicit {
                    AttributeProvenance::Explicit
                } else {
                    AttributeProvenance::Default
                },
                CoercedAttributeValue::String(self.help.clone()),
            ),
        ] {
            set_native_value(class, values, name, provenance, value);
        }
    }
}

pub(super) fn declare<'v>(
    name: &str,
    default: Value<'v>,
    help: Option<Value<'v>>,
    scope: Option<Value<'v>>,
    visibility: Option<UnpackVisibility>,
    kwargs: SmallMap<String, Value<'v>>,
    eval: &mut Evaluator<'v, '_, '_>,
) -> anyhow::Result<NoneType> {
    let recorder = PackageRecorder::from_evaluator(eval)?;
    let default = coerce_native_direct_label(recorder, "build_setting_default", default)?;
    // Repository-map provenance belongs to the tracked package input, not the
    // semantic value. Equal canonical labels must elide equal command defaults.
    let default = slug_identity_v2::ResolvedOptionLabel::from_canonical(&default)
        .canonical()
        .expect("visible canonical label");
    // AttributeProvider.java239–240 treats a known optional attribute's None
    // as omitted, including its explicitness. The mandatory default still fails.
    let help = help.filter(|value| !value.is_none());
    let scope = scope.filter(|value| !value.is_none());
    let help_explicit = help.is_some();
    let help = help
        .map(|value| nonconfigurable_string(recorder, "help", value))
        .transpose()?
        .unwrap_or_default();
    let scope = match scope {
        None => BuildSettingScope::Default,
        Some(value) => {
            let scope = nonconfigurable_string(recorder, "scope", value)?;
            // LabelBuildSettings uses AllowedValueSet, not the command's
            // case-insensitive scope converter. Other valid scopes are deferred.
            anyhow::ensure!(
                scope == "universal",
                "native label_flag scope `{scope}` is unsupported; expected universal"
            );
            BuildSettingScope::Universal
        }
    };
    let declaration = LabelFlagDeclaration {
        declaration: BuildSettingDeclaration {
            definition: BuildSettingDefinition::Label { flag: true },
            default: BuildSettingDefault::Label(default),
            scope,
        },
        help,
        help_explicit,
    };
    recorder.record_target(
        name.to_owned(),
        PackageTargetKind::LabelFlag(declaration),
        recorder.visibility_source(
            visibility.map(|value| value.items),
            VisibilitySource::PackageDefault,
        )?,
    )?;
    recorder.set_native_generator_from_evaluator(name, eval)?;
    recorder.set_native_overrides(name, kwargs)?;
    Ok(NoneType)
}

fn nonconfigurable_string(
    recorder: &PackageRecorder,
    name: &str,
    value: Value<'_>,
) -> anyhow::Result<CompactString> {
    match coerce_starlark_value(recorder, AttributeKind::String, name, false, value)? {
        CoercedAttributeValue::String(value) => Ok(value),
        _ => anyhow::bail!("attribute `{name}` must be a string"),
    }
}

#[cfg(test)]
mod tests;
