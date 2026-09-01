/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::sync::Arc;

use slug_identity_v2::ApparentLabel;
use slug_identity_v2::CanonicalLabel;
use starlark_map::small_set::SmallSet;

use crate::attrs::TransitionSetting;
use crate::bzl_module::BzlModuleIdentity;

const NATIVE_OPTION_PREFIX: &str = "//command_line_option:";

#[derive(Debug, Clone, Copy)]
pub(crate) enum TransitionSettingsKind {
    Inputs,
    Outputs,
}

impl TransitionSettingsKind {
    fn singular(self) -> &'static str {
        match self {
            Self::Inputs => "input",
            Self::Outputs => "output",
        }
    }

    fn plural(self) -> &'static str {
        match self {
            Self::Inputs => "INPUTS",
            Self::Outputs => "OUTPUTS",
        }
    }
}

/// Perform Bazel's first transition-setting phase. Callers validate inputs
/// before outputs, then canonicalize outputs before inputs.
pub(crate) fn validate_transition_settings(
    settings: &[String],
    kind: TransitionSettingsKind,
    source: &BzlModuleIdentity,
) -> anyhow::Result<Vec<TransitionSetting>> {
    let mut declared = SmallSet::with_capacity(settings.len());
    let mut resolved = Vec::with_capacity(settings.len());
    for setting in settings {
        let canonical = resolve_transition_setting(setting, kind, source)?;
        if !declared.insert(setting.as_str()) {
            anyhow::bail!("duplicate transition {} '{}'", kind.singular(), setting);
        }
        resolved.push(TransitionSetting::new(canonical, setting.as_str()));
    }
    Ok(resolved)
}

/// Perform Bazel's second transition-setting phase and freeze canonical order.
pub(crate) fn canonicalize_transition_settings(
    mut settings: Vec<TransitionSetting>,
    kind: TransitionSettingsKind,
) -> anyhow::Result<Arc<[TransitionSetting]>> {
    for current in 0..settings.len() {
        if let Some(previous) = settings[..current]
            .iter()
            .find(|previous| previous.canonical() == settings[current].canonical())
        {
            anyhow::bail!(
                "Transition declares duplicate build setting '{}' in {} (specified as '{}' and '{}')",
                bazel_label_display(settings[current].canonical()),
                kind.plural(),
                settings[current].declared(),
                previous.declared(),
            );
        }
    }
    settings.sort_by(|left, right| left.canonical().bazel_natural_cmp(right.canonical()));
    Ok(settings.into())
}

fn resolve_transition_setting(
    raw: &str,
    kind: TransitionSettingsKind,
    source: &BzlModuleIdentity,
) -> anyhow::Result<CanonicalLabel> {
    if let Some(option) = raw.strip_prefix(NATIVE_OPTION_PREFIX) {
        if !valid_regular_transition_option(option) {
            anyhow::bail!(
                "Invalid transition {} '{}'. Cannot transition on --experimental_* or --incompatible_* options",
                kind.singular(),
                raw,
            );
        }
        return CanonicalLabel::parse(&format!("@@{raw}"))
            .map_err(|error| malformed_setting(raw, kind, error));
    }

    let label = if raw.starts_with("@@") {
        CanonicalLabel::parse(raw)
    } else if raw.starts_with('@') {
        resolve_apparent_setting(raw, source)
    } else if raw.starts_with("//") {
        let provisional = CanonicalLabel::parse(&format!("@@{raw}"));
        provisional.and_then(|label| {
            if source.label.package().repo().is_root() {
                Ok(label)
            } else {
                label.rebind_provisional_root_repository(source.label.package().repo())
            }
        })
    } else {
        Err("absolute label must begin with '@' or '//'".to_owned())
    }
    .map_err(|error| malformed_setting(raw, kind, error))?;

    if label
        .package()
        .package()
        .as_str()
        .split('/')
        .any(|segment| segment == "...")
    {
        return Err(malformed_setting(
            raw,
            kind,
            "package name cannot contain '...'",
        ));
    }
    Ok(label)
}

fn resolve_apparent_setting(
    raw: &str,
    source: &BzlModuleIdentity,
) -> Result<CanonicalLabel, String> {
    let apparent = ApparentLabel::parse(raw)?;
    let repository = if apparent.repo().is_root() {
        slug_identity_v2::CanonicalRepoName::root()
    } else {
        source
            .repository_mapping
            .iter()
            .find_map(|(name, repository)| (name == apparent.repo()).then(|| repository.clone()))
            .ok_or_else(|| {
                format!(
                    "no repo visible as @{} from {}",
                    apparent.repo().as_str(),
                    source.label
                )
            })?
    };
    let spelling = if repository.is_root() {
        format!("@@//{}:{}", apparent.package(), apparent.target())
    } else {
        format!(
            "@@{}//{}:{}",
            repository.as_str(),
            apparent.package(),
            apparent.target()
        )
    };
    CanonicalLabel::parse(&spelling)
}

fn malformed_setting(
    raw: &str,
    kind: TransitionSettingsKind,
    error: impl std::fmt::Display,
) -> anyhow::Error {
    anyhow::anyhow!(
        "invalid transition {} '{}'. If this is intended as a native option, it must begin with //command_line_option: {}",
        kind.singular(),
        raw,
        error,
    )
}

fn valid_regular_transition_option(option: &str) -> bool {
    if option.starts_with("experimental_") {
        return false;
    }
    if matches!(
        option,
        "incompatible_enable_cc_toolchain_resolution"
            | "incompatible_enable_apple_toolchain_resolution"
    ) {
        return true;
    }
    !option.starts_with("incompatible_")
}

fn bazel_label_display(label: &CanonicalLabel) -> String {
    let is_root = label.package().repo().is_root();
    let display = label.to_string();
    if is_root {
        display
            .strip_prefix("@@")
            .expect("canonical root label begins with @@")
            .to_owned()
    } else {
        display
    }
}
