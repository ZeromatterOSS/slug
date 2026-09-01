/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file. You may select, at
 * your option, one of the above-listed licenses.
 */

//! Phase-scratch configured dependencies shared by ordinary late-bound rule
//! attributes and lifted subrule attributes.

use std::sync::Arc;

use compact_str::CompactString;
use slug_build_api_v2::ActionOutputKind;
use slug_build_api_v2::AnalysisArtifact;
use slug_build_api_v2::ProviderIdentity;
use slug_configuration_v2::HostPathFlavor;
use slug_identity_v2::CanonicalLabel;
use slug_loading_v2::AttributeKind;
use slug_loading_v2::ConfiguredDependencyDefault;
use slug_loading_v2::FileAdmissibility;
use slug_loading_v2::SubruleIdentity;
use slug_loading_v2::package::StarlarkRuleImplementation;

use crate::configured_target::ConfiguredAttributeDependency;
use crate::dice::AnalysisError;
use crate::exec_group::ConfiguredExecGroup;
use crate::key::ConfigurationKind;
use crate::key::ConfiguredNodeKey;
use crate::result::ConfiguredNodeKind;
use crate::result::ConfiguredNodeResult;

#[derive(Debug, Clone)]
pub(crate) struct ConfiguredDependencyValidation {
    file_admissibility: FileAdmissibility,
    executable: bool,
    required_providers: Arc<[Arc<[ProviderIdentity]>]>,
}

#[derive(Debug, Clone)]
pub(crate) struct DeclaredDependencyKey {
    pub(crate) attribute: CompactString,
    pub(crate) attribute_index: u32,
    pub(crate) node: ConfiguredNodeKey,
    pub(crate) dependency: ConfiguredAttributeDependency,
    pub(crate) hidden: bool,
    pub(crate) source_admitted: bool,
    pub(crate) path_flavor: Option<HostPathFlavor>,
    pub(crate) validation: Option<ConfiguredDependencyValidation>,
    pub(crate) configured_row: Option<u32>,
}

#[derive(Debug, Clone)]
pub(crate) struct ConfiguredDependencyRow {
    pub(crate) index: u32,
    pub(crate) attribute: CompactString,
    pub(crate) user_name: Option<CompactString>,
    pub(crate) owner: Option<Arc<SubruleIdentity>>,
    pub(crate) kind: AttributeKind,
    pub(crate) labels: Vec<CanonicalLabel>,
    pub(crate) hidden: bool,
    pub(crate) dependency: ConfiguredAttributeDependency,
    validation: ConfiguredDependencyValidation,
}

impl ConfiguredDependencyRow {
    pub(crate) fn attribute_name(&self) -> &str {
        &self.attribute
    }

    pub(crate) fn requires_path_flavor(&self) -> bool {
        self.validation.file_admissibility.suffixes().is_some()
    }

    pub(crate) fn allow_single_file(&self) -> bool {
        self.validation.file_admissibility.single_artifact()
    }

    pub(crate) fn executable(&self) -> bool {
        self.validation.executable
    }

    pub(crate) fn into_keys(
        &self,
        path_flavor: Option<HostPathFlavor>,
        make_node: impl Fn(CanonicalLabel, &ConfiguredAttributeDependency) -> ConfiguredNodeKey,
    ) -> Vec<DeclaredDependencyKey> {
        let validation = self.validation.clone();
        self.labels
            .iter()
            .cloned()
            .enumerate()
            .map(|(index, label)| DeclaredDependencyKey {
                attribute: self.attribute.clone(),
                attribute_index: u32::try_from(index).expect("dependency index fits u32"),
                node: make_node(label, &self.dependency),
                dependency: self.dependency.clone(),
                hidden: self.hidden,
                source_admitted: validation.admits_file(),
                path_flavor,
                validation: Some(validation.clone()),
                configured_row: Some(self.index),
            })
            .collect()
    }
}

impl ConfiguredDependencyValidation {
    fn admits_file(&self) -> bool {
        self.file_admissibility.admits_direct_file()
    }

    pub(crate) fn new(
        file_admissibility: FileAdmissibility,
        executable: bool,
        required_providers: Arc<[Arc<[ProviderIdentity]>]>,
    ) -> Self {
        Self {
            file_admissibility,
            executable,
            required_providers,
        }
    }
}

pub(crate) fn configured_dependency_rows(
    implementation: &StarlarkRuleImplementation,
    configuration: &slug_configuration_v2::SlugConfiguration,
) -> Result<Vec<ConfiguredDependencyRow>, AnalysisError> {
    implementation
        .configured_dependency_attributes()
        .enumerate()
        .map(|(index, attribute)| {
            if !matches!(
                attribute.kind(),
                AttributeKind::Label | AttributeKind::LabelList
            ) {
                return Err(AnalysisError::message(format!(
                    "configured dependency `{}` has unsupported attribute kind {:?}",
                    attribute.name(),
                    attribute.kind()
                )));
            }
            let labels = match attribute.default() {
                ConfiguredDependencyDefault::Literal(value) => {
                    let mut labels = Vec::new();
                    value.labels(&mut labels);
                    labels
                }
                ConfiguredDependencyDefault::ConfigurationField(identity) => configuration
                    .configuration_field_label(identity)
                    .map_err(|error| {
                        AnalysisError::message(format!(
                            "resolving configured dependency `{}`: {error}",
                            attribute.name()
                        ))
                    })?
                    .into_iter()
                    .collect(),
            };
            if attribute.kind() == AttributeKind::Label && labels.len() > 1 {
                return Err(AnalysisError::message(format!(
                    "configured label dependency `{}` resolved to multiple labels",
                    attribute.name()
                )));
            }
            Ok(ConfiguredDependencyRow {
                index: u32::try_from(index).expect("configured dependency row fits u32"),
                attribute: attribute.name().into(),
                user_name: attribute.user_name().map(CompactString::new),
                owner: attribute.owner().cloned(),
                kind: attribute.kind(),
                labels,
                hidden: attribute.is_hidden(),
                dependency: if attribute.exec_configuration() {
                    ConfiguredAttributeDependency::Exec(ConfiguredExecGroup::Default)
                } else {
                    ConfiguredAttributeDependency::Target
                },
                validation: ConfiguredDependencyValidation::new(
                    attribute.file_admissibility().clone(),
                    attribute.executable(),
                    Arc::from(attribute.required_providers()),
                ),
            })
        })
        .collect()
}

pub(crate) fn validate_configured_dependency(
    dependency: &DeclaredDependencyKey,
    result: &ConfiguredNodeResult,
) -> Result<(), AnalysisError> {
    let Some(validation) = &dependency.validation else {
        return Ok(());
    };
    if dependency.dependency.tool()
        && dependency
            .node
            .configured_target()
            .is_some_and(|target| target.configuration().kind() != ConfigurationKind::Exec)
    {
        return Err(AnalysisError::message(format!(
            "configured dependency `{}` did not retain its selected Exec configuration",
            dependency.attribute
        )));
    }
    let file = matches!(
        result.kind(),
        ConfiguredNodeKind::SourceFile | ConfiguredNodeKind::GeneratedFile
    );
    if !file
        && !validation.required_providers.is_empty()
        && !validation.required_providers.iter().any(|alternative| {
            alternative
                .iter()
                .all(|provider| result.providers().contains(provider))
        })
    {
        let required = validation
            .required_providers
            .iter()
            .map(|alternative| {
                alternative
                    .iter()
                    .map(|provider| match provider {
                        ProviderIdentity::Builtin(name) => name.to_string(),
                        ProviderIdentity::User(id) => id.to_string(),
                    })
                    .collect::<Vec<_>>()
                    .join(" and ")
            })
            .collect::<Vec<_>>()
            .join(" or ");
        return Err(AnalysisError::message(format!(
            "configured dependency `{}` target `{}` does not provide any admitted provider alternative: {}",
            dependency.attribute,
            dependency.node.label(),
            required,
        )));
    }
    validate_file_admissibility(dependency, result, validation, file)?;
    if validation.executable
        && !file
        && result
            .providers()
            .default_info()
            .and_then(|info| info.files_to_run.executable.as_ref())
            .is_none()
    {
        return Err(AnalysisError::message(format!(
            "configured dependency `{}` is not executable",
            dependency.attribute
        )));
    }
    Ok(())
}

fn validate_file_admissibility(
    dependency: &DeclaredDependencyKey,
    result: &ConfiguredNodeResult,
    validation: &ConfiguredDependencyValidation,
    direct_file: bool,
) -> Result<(), AnalysisError> {
    let policy = &validation.file_admissibility;
    if direct_file {
        let filename = result
            .key()
            .label()
            .target()
            .as_str()
            .rsplit('/')
            .next()
            .expect("target name is nonempty");
        if policy_matches_filename(dependency, policy, filename)? {
            return Ok(());
        }
        return Err(AnalysisError::message(format!(
            "configured dependency `{}` source file {} does not match its admitted file types",
            dependency.attribute,
            result.key().label()
        )));
    }
    if policy.is_no_files() || (policy.is_any_file() && !policy.single_artifact()) {
        return Ok(());
    }
    let artifacts = result
        .providers()
        .default_info()
        .map(|info| info.file_artifacts())
        .unwrap_or_default();
    if policy.single_artifact() && artifacts.len() != 1 {
        return Err(AnalysisError::message(format!(
            "configured dependency `{}` must provide exactly one file, got {}",
            dependency.attribute,
            artifacts.len()
        )));
    }
    if policy.is_any_file() {
        return Ok(());
    }
    if generated_artifacts_match(&artifacts, |filename| {
        policy_matches_filename(dependency, policy, filename)
    })? {
        return Ok(());
    }
    Err(AnalysisError::message(format!(
        "configured dependency `{}` does not produce any file matching its admitted file types",
        dependency.attribute
    )))
}

fn generated_artifacts_match(
    artifacts: &[AnalysisArtifact],
    mut matches_filename: impl FnMut(&str) -> Result<bool, AnalysisError>,
) -> Result<bool, AnalysisError> {
    for artifact in artifacts {
        if matches!(
            artifact,
            AnalysisArtifact::Derived { output, .. }
                if output.kind() == ActionOutputKind::Directory
        ) || matches_filename(artifact.path().rsplit('/').next().unwrap_or_default())?
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn policy_matches_filename(
    dependency: &DeclaredDependencyKey,
    policy: &FileAdmissibility,
    filename: &str,
) -> Result<bool, AnalysisError> {
    if policy.is_no_files() {
        return Ok(false);
    }
    if policy.is_any_file() {
        return Ok(true);
    }
    let path_flavor = dependency.path_flavor.ok_or_else(|| {
        AnalysisError::message(format!(
            "configured dependency `{}` suffix policy is missing structural Host path flavor",
            dependency.attribute
        ))
    })?;
    Ok(policy.matches_filename(path_flavor, filename))
}

#[cfg(test)]
mod tests {
    use slug_build_api_v2::ActionOutput;
    use slug_build_api_v2::AnalysisConfiguredTargetKey;

    use super::*;

    #[test]
    fn generated_directory_is_admitted_before_suffix_matching() {
        let directory = AnalysisArtifact::Derived {
            owner: AnalysisConfiguredTargetKey::new(
                CanonicalLabel::parse("@@//pkg:producer").unwrap(),
                Arc::<[u8]>::from([]),
            ),
            output: ActionOutput::new("pkg/tree", ActionOutputKind::Directory),
        };
        let mut suffix_checks = 0;
        assert!(
            generated_artifacts_match(&[directory], |_| {
                suffix_checks += 1;
                Ok(false)
            })
            .unwrap()
        );
        assert_eq!(suffix_checks, 0);
    }
}
