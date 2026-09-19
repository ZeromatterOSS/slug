//! Complete rule output metadata under the configured-result owner.

use compact_str::CompactString;
use slug_build_api_v2::AnalysisDepset;
use slug_build_api_v2::AnalysisValue;
use slug_build_api_v2::DepsetOrder;
use slug_build_api_v2::OutputGroupInfo;
use slug_build_api_v2::ProviderCollection;
use slug_build_api_v2::ProviderValue;
use slug_loading_v2::AttributeSchema;
use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;

use crate::starlark_rule::PreparedDependency;

const HIDDEN: &str = "_hidden_top_level_INTERNAL_";
const VALIDATION: &str = "_validation";

pub(crate) fn collect_validation_groups<'a>(
    schema: &[AttributeSchema],
    dependencies: &[PreparedDependency],
    late_bound_names: impl IntoIterator<Item = &'a str>,
) -> Result<Vec<AnalysisDepset>, String> {
    let late_bound_names: SmallSet<_> = late_bound_names.into_iter().collect();
    let policy: SmallMap<_, _> = schema
        .iter()
        .map(|attribute| {
            (
                attribute.declaration_name(),
                attribute.propagates_validations(
                    late_bound_names.contains(attribute.declaration_name()),
                ),
            )
        })
        .collect();
    let mut groups = Vec::new();
    for dependency in dependencies {
        if dependency.filtered {
            continue;
        }
        let eligible = policy.get(dependency.attribute.as_str()).ok_or_else(|| {
            format!(
                "configured dependency {} has no retained attribute schema",
                dependency.attribute
            )
        })?;
        if !eligible {
            continue;
        }
        if let Some(files) = dependency
            .providers
            .output_group_info()
            .and_then(|groups| groups.groups().get(VALIDATION))
            .filter(|files| !files.is_empty())
        {
            groups.push(files.clone());
        }
    }
    Ok(groups)
}

pub(crate) fn complete_rule_output_groups(
    providers: ProviderCollection,
    validations: Vec<AnalysisDepset>,
) -> Result<ProviderCollection, String> {
    let mut groups: SmallMap<CompactString, Vec<AnalysisDepset>> = providers
        .output_group_info()
        .into_iter()
        .flat_map(|info| info.groups().iter())
        .map(|(name, files)| (name.clone(), vec![files.clone()]))
        .collect();
    let default = providers
        .default_info()
        .expect("checked provider collection");
    let hidden = if let Some(support) = &default.files_to_run.support {
        AnalysisDepset::new(
            DepsetOrder::Default,
            vec![AnalysisValue::artifact(support.tree.clone())],
            vec![],
        )
    } else {
        // Bazel Runfiles.getAllArtifacts preserves the files graph and adds
        // symlink targets directly. Empty filenames have no artifact producer.
        let runfiles = &default.default_runfiles;
        let symlink_targets = runfiles
            .symlinks
            .to_list()
            .into_iter()
            .chain(runfiles.root_symlinks.to_list())
            .map(|link| AnalysisValue::artifact(link.artifact.clone()))
            .collect();
        AnalysisDepset::new(
            DepsetOrder::Default,
            symlink_targets,
            vec![runfiles.files.clone()],
        )
    }
    .map_err(|error| error.to_string())?;
    groups.entry(HIDDEN.into()).or_default().push(hidden);
    if !validations.is_empty() {
        groups
            .entry(VALIDATION.into())
            .or_default()
            .extend(validations);
    }
    // RuleConfiguredTargetBuilder uses stable builders even for explicit
    // groups receiving no automatic additions. This differs from construction
    // of a standalone/nested OutputGroupInfo value.
    let effective = groups
        .into_iter()
        .map(|(name, children)| {
            AnalysisDepset::new(DepsetOrder::Default, vec![], children)
                .map(|files| (name, files))
                .map_err(|error| error.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let effective = OutputGroupInfo::new(effective).map_err(|error| error.to_string())?;
    ProviderCollection::new(
        providers
            .iter()
            .filter(|(_, value)| !matches!(value, ProviderValue::OutputGroupInfo(_)))
            .map(|(_, value)| value.clone())
            .chain(std::iter::once(ProviderValue::OutputGroupInfo(effective)))
            .collect(),
    )
    .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests;
