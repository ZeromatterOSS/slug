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
use slug_build_api_v2::ProviderIdentity;
use slug_identity_v2::CanonicalLabel;
use slug_loading_v2::AllowSingleFile;
use slug_loading_v2::AttributeKind;
use slug_loading_v2::ConfiguredDependencyDefault;
use slug_loading_v2::SubruleIdentity;
use slug_loading_v2::package::StarlarkRuleImplementation;

use crate::dice::AnalysisError;
use crate::key::ConfigurationKind;
use crate::key::ConfiguredNodeKey;
use crate::result::ConfiguredNodeKind;
use crate::result::ConfiguredNodeResult;

#[derive(Debug, Clone)]
pub(crate) struct ConfiguredDependencyValidation {
    allow_files: bool,
    allow_single_file: Option<AllowSingleFile>,
    executable: bool,
    required_providers: Arc<[Arc<[ProviderIdentity]>]>,
}

#[derive(Debug, Clone)]
pub(crate) struct DeclaredDependencyKey {
    pub(crate) attribute: CompactString,
    pub(crate) attribute_index: u32,
    pub(crate) node: ConfiguredNodeKey,
    pub(crate) transition_output: Option<CanonicalLabel>,
    pub(crate) hidden: bool,
    pub(crate) exec_configuration: bool,
    pub(crate) source_admitted: bool,
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
    pub(crate) exec_configuration: bool,
    validation: ConfiguredDependencyValidation,
}

impl ConfiguredDependencyRow {
    pub(crate) fn allow_single_file(&self) -> bool {
        matches!(
            self.validation.allow_single_file,
            Some(AllowSingleFile::True | AllowSingleFile::Extensions(_))
        )
    }

    pub(crate) fn executable(&self) -> bool {
        self.validation.executable
    }

    pub(crate) fn into_keys(
        &self,
        make_node: impl Fn(CanonicalLabel, bool) -> ConfiguredNodeKey,
    ) -> Vec<DeclaredDependencyKey> {
        let validation = self.validation.clone();
        self.labels
            .iter()
            .cloned()
            .enumerate()
            .map(|(index, label)| DeclaredDependencyKey {
                attribute: self.attribute.clone(),
                attribute_index: u32::try_from(index).expect("dependency index fits u32"),
                node: make_node(label, self.exec_configuration),
                transition_output: None,
                hidden: self.hidden,
                exec_configuration: self.exec_configuration,
                source_admitted: validation.admits_file(),
                validation: Some(validation.clone()),
                configured_row: Some(self.index),
            })
            .collect()
    }
}

impl ConfiguredDependencyValidation {
    fn admits_file(&self) -> bool {
        self.allow_files
            || matches!(
                self.allow_single_file,
                Some(AllowSingleFile::True | AllowSingleFile::Extensions(_))
            )
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
                exec_configuration: attribute.exec_configuration(),
                validation: ConfiguredDependencyValidation {
                    allow_files: attribute.allow_files(),
                    allow_single_file: attribute.allow_single_file().cloned(),
                    executable: attribute.executable(),
                    required_providers: Arc::from(attribute.required_providers()),
                },
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
    if dependency.exec_configuration
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
    if !validation.required_providers.is_empty()
        && !validation.required_providers.iter().any(|alternative| {
            alternative.iter().all(|provider| {
                (file && provider.is_builtin("DefaultInfo"))
                    || result.providers().contains(provider)
            })
        })
    {
        return Err(AnalysisError::message(format!(
            "configured dependency `{}` does not provide any admitted provider alternative",
            dependency.attribute
        )));
    }
    let single_file = match &validation.allow_single_file {
        Some(AllowSingleFile::True) | Some(AllowSingleFile::Extensions(_)) => true,
        Some(AllowSingleFile::False) | None => false,
    };
    if file && !validation.allow_files && !single_file {
        return Err(AnalysisError::message(format!(
            "configured dependency `{}` does not admit file target {}",
            dependency.attribute,
            result.key().label()
        )));
    }
    if single_file {
        validate_single_file(dependency, result, validation)?;
    }
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

fn validate_single_file(
    dependency: &DeclaredDependencyKey,
    result: &ConfiguredNodeResult,
    validation: &ConfiguredDependencyValidation,
) -> Result<(), AnalysisError> {
    let files = if matches!(
        result.kind(),
        ConfiguredNodeKind::SourceFile | ConfiguredNodeKind::GeneratedFile
    ) {
        vec![result.key().label().target().as_str().to_owned()]
    } else {
        result
            .providers()
            .default_info()
            .map(|info| {
                info.file_artifacts()
                    .into_iter()
                    .map(|artifact| artifact.path().into_owned())
                    .collect()
            })
            .unwrap_or_default()
    };
    let [file] = files.as_slice() else {
        return Err(AnalysisError::message(format!(
            "configured dependency `{}` must provide exactly one file, got {}",
            dependency.attribute,
            files.len()
        )));
    };
    if let Some(AllowSingleFile::Extensions(extensions)) = &validation.allow_single_file
        && !extensions
            .iter()
            .any(|extension| file.ends_with(extension.as_str()))
    {
        return Err(AnalysisError::message(format!(
            "configured dependency `{}` file `{file}` does not match an admitted extension",
            dependency.attribute
        )));
    }
    Ok(())
}
