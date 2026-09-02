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
    attribute_kind: AttributeKind,
    file_admissibility: FileAdmissibility,
    skip_analysis_time_filetype_check: bool,
    silent_ruleclass_filter: bool,
    allowed_rule_classes: Option<Arc<[CompactString]>>,
    executable: bool,
    required_providers: Arc<[Arc<[ProviderIdentity]>]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConfiguredDependencyDisposition {
    Visible,
    Filtered,
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
        attribute_kind: AttributeKind,
        file_admissibility: FileAdmissibility,
        skip_analysis_time_filetype_check: bool,
        silent_ruleclass_filter: bool,
        allowed_rule_classes: Option<Arc<[CompactString]>>,
        executable: bool,
        required_providers: Arc<[Arc<[ProviderIdentity]>]>,
    ) -> Self {
        Self {
            attribute_kind,
            file_admissibility,
            skip_analysis_time_filetype_check,
            silent_ruleclass_filter,
            allowed_rule_classes,
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
                    attribute.kind(),
                    attribute.file_admissibility().clone(),
                    attribute.skip_analysis_time_filetype_check(),
                    attribute.silent_ruleclass_filter(),
                    attribute.allowed_rule_classes().cloned(),
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
) -> Result<ConfiguredDependencyDisposition, AnalysisError> {
    let Some(validation) = &dependency.validation else {
        return Ok(ConfiguredDependencyDisposition::Visible);
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
    let prerequisite_rule_class = result.prerequisite_rule_class();
    let silently_filtered = validation.silent_ruleclass_filter
        && validation
            .allowed_rule_classes
            .as_ref()
            .is_some_and(|classes| !rule_class_matches(classes, prerequisite_rule_class));
    if silently_filtered {
        if matches!(
            validation.attribute_kind,
            AttributeKind::StringKeyedLabelDict | AttributeKind::LabelListDict
        ) {
            return Err(AnalysisError::message(format!(
                "configured dependency `{}` would enter Bazel 9.2's unsupported silent-filter projection for {:?}",
                dependency.attribute, validation.attribute_kind
            )));
        }
        return Ok(ConfiguredDependencyDisposition::Filtered);
    }

    let file = matches!(
        result.kind(),
        ConfiguredNodeKind::SourceFile | ConfiguredNodeKind::GeneratedFile
    );
    if prerequisite_rule_class.is_some() {
        let providers_match = !validation.required_providers.is_empty()
            && validation.required_providers.iter().any(|alternative| {
                alternative
                    .iter()
                    .all(|provider| result.providers().contains(provider))
            });
        let class_matches = match &validation.allowed_rule_classes {
            Some(classes) => rule_class_matches(classes, prerequisite_rule_class),
            None => validation.required_providers.is_empty(),
        };
        if !class_matches && !providers_match {
            return Err(rule_or_provider_error(dependency, validation));
        }
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
    Ok(ConfiguredDependencyDisposition::Visible)
}

fn rule_class_matches(classes: &[CompactString], rule_class: Option<&str>) -> bool {
    let rule_class = rule_class.unwrap_or("");
    classes
        .binary_search_by(|candidate| candidate.as_str().cmp(rule_class))
        .is_ok()
}

fn rule_or_provider_error(
    dependency: &DeclaredDependencyKey,
    validation: &ConfiguredDependencyValidation,
) -> AnalysisError {
    let required_providers = (!validation.required_providers.is_empty()).then(|| {
        validation
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
            .join(" or ")
    });
    if validation.allowed_rule_classes.is_none() {
        return AnalysisError::message(format!(
            "configured dependency `{}` target `{}` does not provide any admitted provider alternative: {}",
            dependency.attribute,
            dependency.node.label(),
            required_providers.expect("provider-only failure has requirements"),
        ));
    }
    let mut requirements = Vec::new();
    if let Some(classes) = &validation.allowed_rule_classes {
        requirements.push(if classes.is_empty() {
            "rule class expected nothing".to_owned()
        } else {
            format!(
                "rule class expected {}",
                classes
                    .iter()
                    .map(CompactString::as_str)
                    .collect::<Vec<_>>()
                    .join(" or ")
            )
        });
    }
    if let Some(required) = required_providers {
        requirements.push(format!("providers expected {required}"));
    }
    AnalysisError::message(format!(
        "configured dependency `{}` target `{}` is misplaced ({})",
        dependency.attribute,
        dependency.node.label(),
        requirements.join("; "),
    ))
}

fn validate_file_admissibility(
    dependency: &DeclaredDependencyKey,
    result: &ConfiguredNodeResult,
    validation: &ConfiguredDependencyValidation,
    direct_file: bool,
) -> Result<(), AnalysisError> {
    let policy = &validation.file_admissibility;
    if validation.skip_analysis_time_filetype_check
        && !matches!(result.kind(), ConfiguredNodeKind::SourceFile)
    {
        return Ok(());
    }
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
    use slug_build_api_v2::ProviderCollection;
    use slug_build_api_v2::RunfilesPackageDepset;

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

    #[test]
    fn package_group_bypasses_rule_class_and_provider_predicates() {
        let label = CanonicalLabel::parse("@@//pkg:visibility").unwrap();
        let node = ConfiguredNodeKey::null(label);
        let result = ConfiguredNodeResult::new_native(
            node.clone(),
            ConfiguredNodeKind::PackageGroup,
            ProviderCollection::from_values(Vec::new(), false).unwrap(),
            None,
            RunfilesPackageDepset::empty(),
        );
        let dependency = DeclaredDependencyKey {
            attribute: "dep".into(),
            attribute_index: 0,
            node,
            dependency: ConfiguredAttributeDependency::Target,
            hidden: false,
            source_admitted: false,
            path_flavor: None,
            validation: Some(ConfiguredDependencyValidation::new(
                AttributeKind::Label,
                FileAdmissibility::default(),
                false,
                false,
                Some(Arc::from([CompactString::new("missing_rule")])),
                false,
                Arc::from([Arc::from([ProviderIdentity::builtin("MissingInfo")])]),
            )),
            configured_row: None,
        };

        assert_eq!(
            validate_configured_dependency(&dependency, &result).unwrap(),
            ConfiguredDependencyDisposition::Visible
        );
    }
}
