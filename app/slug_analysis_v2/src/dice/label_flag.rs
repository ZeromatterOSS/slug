//! Native label settings: own option identity, selected target providers.
use slug_build_api_v2::ProviderIdentity;
use slug_build_api_v2::RunfilesPackageMetadata;
use slug_configuration_v2::StarlarkOptionValue;
use slug_loading_v2::PackageTargetKind;
use slug_loading_v2::package::BuildSettingDefinition;
use slug_loading_v2::package::ConfigSettingTarget;

use super::*;
use crate::build_setting::BuildSettingLookupStep;
use crate::build_setting::build_setting_lookup_step;
use crate::build_setting::effective_value;

pub(super) async fn declaration(
    ctx: &mut DiceComputations<'_>,
    mode: ConfiguredAnalysisMode,
    workspace: &NormalizedAbsolutePath,
    requested: &CanonicalLabel,
) -> AnalysisSemanticOutcome<(CanonicalLabel, BuildSettingDeclaration)> {
    let mut label = requested.clone();
    let mut visited = SmallSet::new();
    loop {
        if !visited.insert(label.clone()) {
            return analysis_semantic_complete(Err(AnalysisError::message(format!(
                "build setting alias cycle at {label}"
            ))));
        }
        let inventory = semantic_value!(
            compute_configured_package_input(
                ctx,
                mode,
                workspace.dupe(),
                label.package().clone(),
                "loading build-setting alias declaration through DICE",
            )
            .await
        );
        let package = match inventory.loaded() {
            Ok(package) => package,
            Err(error) => return analysis_semantic_complete(Err(package_inventory_error(error))),
        };
        match build_setting_lookup_step(package, &label) {
            Ok(BuildSettingLookupStep::Setting(declaration)) => {
                return analysis_semantic_complete(Ok((label, declaration)));
            }
            Ok(BuildSettingLookupStep::Alias(next)) => label = next,
            Err(error) => return analysis_semantic_complete(Err(AnalysisError::message(error))),
        }
    }
}

/// A native label_flag keeps its label-typed BuildSettingProvider while
/// forwarding the selected target's FeatureFlagInfo. Classify the requested
/// flag (through aliases), not the forwarded actual target.
async fn feature_label_typed(
    ctx: &mut DiceComputations<'_>,
    mode: ConfiguredAnalysisMode,
    workspace: &NormalizedAbsolutePath,
    requested: &CanonicalLabel,
) -> AnalysisSemanticOutcome<bool> {
    let mut label = requested.clone();
    let mut visited = SmallSet::new();
    loop {
        if !visited.insert(label.clone()) {
            return analysis_semantic_complete(Err(AnalysisError::message(format!(
                "build setting alias cycle at {label}"
            ))));
        }
        let inventory = semantic_value!(
            compute_configured_package_input(
                ctx,
                mode,
                workspace.dupe(),
                label.package().clone(),
                "loading feature-flag declaration through DICE",
            )
            .await
        );
        let package = match inventory.loaded() {
            Ok(package) => package,
            Err(error) => return analysis_semantic_complete(Err(package_inventory_error(error))),
        };
        let Some(target) = package
            .targets
            .iter()
            .find(|target| target.name == label.target().as_str())
        else {
            return analysis_semantic_complete(Err(AnalysisError::message(format!(
                "feature-flag target {label} is missing from its package"
            ))));
        };
        if !matches!(target.kind, PackageTargetKind::Alias { .. }) {
            return match target.kind.build_setting_declaration() {
                Ok(Some(_)) => match build_setting_lookup_step(package, &label) {
                    Ok(BuildSettingLookupStep::Setting(declaration)) => {
                        analysis_semantic_complete(Ok(matches!(
                            declaration.definition(),
                            BuildSettingDefinition::Label { .. }
                        )))
                    }
                    Ok(BuildSettingLookupStep::Alias(_)) => unreachable!("nonalias is a setting"),
                    Err(error) => analysis_semantic_complete(Err(AnalysisError::message(error))),
                },
                Ok(None) => analysis_semantic_complete(Ok(false)),
                Err(error) => analysis_semantic_complete(Err(AnalysisError::message(format!(
                    "invalid build-setting declaration for {label}: {error}"
                )))),
            };
        }
        match build_setting_lookup_step(package, &label) {
            Ok(BuildSettingLookupStep::Alias(next)) => label = next,
            Ok(BuildSettingLookupStep::Setting(_)) => unreachable!("alias is not a setting"),
            Err(error) => return analysis_semantic_complete(Err(AnalysisError::message(error))),
        }
    }
}

pub(super) struct PreparedPredicates {
    pub(super) matches: bool,
    pub(super) normalized: Arc<[(CanonicalLabel, CompactString)]>,
}

/// Called by truth AND selector preparation. The latter needs direct tracked
/// dependencies even when a change leaves the cached truth value unchanged.
pub(super) async fn predicates(
    ctx: &mut DiceComputations<'_>,
    mode: ConfiguredAnalysisMode,
    workspace: &NormalizedAbsolutePath,
    configuration: &ConfigurationKey,
    package: &LoadedPackage,
    condition: &ConfigSettingTarget,
) -> AnalysisSemanticOutcome<PreparedPredicates> {
    let mut normalized = SmallMap::new();
    let mut matches = true;
    let mut all_need: Option<LoadingPreparationNeeds> = None;
    let mut first_outer = None;
    let mut first_error = None;
    for (requested, text) in condition.flag_values().value().iter() {
        let outcome = predicate(
            ctx,
            mode,
            workspace,
            configuration,
            package,
            requested,
            text,
        )
        .await;
        match outcome {
            LoadingPreparationOutcome::Need(need) => {
                all_need = Some(all_need.map_or(need.clone(), |current| {
                    current
                        .try_union(&need)
                        .expect("condition predicate Needs agree")
                }));
            }
            LoadingPreparationOutcome::Complete(Err(error)) => {
                first_outer.get_or_insert(error);
            }
            LoadingPreparationOutcome::Complete(Ok(Err(error))) => {
                first_error.get_or_insert(error);
            }
            LoadingPreparationOutcome::Complete(Ok(Ok((actual, expected, yes)))) => {
                matches &= yes;
                if normalized.insert(actual.clone(), expected).is_some() {
                    first_error.get_or_insert_with(|| {
                        AnalysisError::message(format!(
                            "flag '{actual}' referenced multiple times in config_setting"
                        ))
                    });
                }
            }
        }
    }
    if let Some(error) = first_outer {
        return LoadingPreparationOutcome::Complete(Err(error));
    }
    if let Some(need) = all_need {
        return LoadingPreparationOutcome::Need(need);
    }
    if let Some(error) = first_error {
        return analysis_semantic_complete(Err(error));
    }
    analysis_semantic_complete(Ok(PreparedPredicates {
        matches,
        normalized: normalized.into_iter().collect(),
    }))
}

async fn predicate(
    ctx: &mut DiceComputations<'_>,
    mode: ConfiguredAnalysisMode,
    workspace: &NormalizedAbsolutePath,
    configuration: &ConfigurationKey,
    package: &LoadedPackage,
    requested: &CanonicalLabel,
    text: &str,
) -> AnalysisSemanticOutcome<(CanonicalLabel, CompactString, bool)> {
    let child = match compute_configured_child(
        ctx,
        mode,
        workspace.dupe(),
        requested.clone(),
        configuration.clone(),
    )
    .await
    {
        LoadingPreparationOutcome::Need(need) => return LoadingPreparationOutcome::Need(need),
        LoadingPreparationOutcome::Complete(Err(error)) => {
            return LoadingPreparationOutcome::Complete(Err(error));
        }
        LoadingPreparationOutcome::Complete(Ok(result)) => match result.as_ref() {
            Ok(result) => result.dupe(),
            Err(error) => return analysis_semantic_complete(Err(error.clone())),
        },
    };
    let actual = child.actual_target().label().clone();
    let feature = child
        .providers()
        .get(&ProviderIdentity::builtin("FeatureFlagInfo"));
    if let Some(feature) = feature {
        let ProviderValue::FeatureFlagInfo(feature) = feature else {
            return analysis_semantic_complete(Err(AnalysisError::message(format!(
                "target {actual} has a malformed FeatureFlagInfo provider"
            ))));
        };
        // Bazel canonicalizes before provider comparison using the requested
        // label_flag's BuildSettingProvider, not the forwarded actual target.
        let label_typed =
            semantic_value!(feature_label_typed(ctx, mode, workspace, requested).await);
        let expected = canonicalize_feature_expected(text, package.runfiles_package(), label_typed);
        let matches = feature.value() == expected.as_str();
        return analysis_semantic_complete(Ok((actual, expected, matches)));
    }

    let (setting, declaration) =
        semantic_value!(declaration(ctx, mode, workspace, requested).await);
    let expected = if matches!(
        declaration.definition(),
        BuildSettingDefinition::Label { .. }
    ) {
        let metadata = package.runfiles_package();
        match CanonicalLabel::parse_with_package_context(text, metadata.package(), |apparent| {
            if apparent.is_empty() {
                return Ok(slug_identity_v2::CanonicalRepoName::root());
            }
            metadata
                .mapping()
                .entries()
                .iter()
                .find(|(name, _)| name.as_str() == apparent)
                .map(|(_, repo)| repo.clone())
                .ok_or_else(|| format!("no repository visible as @{apparent} in config_setting"))
        }) {
            Ok(label) => CompactString::new(label.to_string()),
            Err(error) => return analysis_semantic_complete(Err(AnalysisError::message(error))),
        }
    } else {
        CompactString::new(text)
    };
    match matches_expected_text(
        &setting,
        &declaration,
        configuration.starlark_option(&setting),
        &expected,
    ) {
        Ok(matches) => analysis_semantic_complete(Ok((actual, expected, matches))),
        Err(error) => analysis_semantic_complete(Err(AnalysisError::message(error))),
    }
}

fn canonicalize_feature_expected(
    text: &str,
    metadata: &RunfilesPackageMetadata,
    label_typed: bool,
) -> CompactString {
    if !label_typed {
        return CompactString::new(text);
    }
    CanonicalLabel::parse_with_package_context(text, metadata.package(), |apparent| {
        if apparent.is_empty() {
            return Ok(slug_identity_v2::CanonicalRepoName::root());
        }
        metadata
            .mapping()
            .entries()
            .iter()
            .find(|(name, _)| name.as_str() == apparent)
            .map(|(_, repo)| repo.clone())
            .ok_or_else(|| format!("no repository visible as @{apparent} in config_setting"))
    })
    .map(|label| CompactString::new(label.to_string()))
    // Bazel leaves invalid label text intact for the subsequent type
    // conversion. For a feature provider, the raw text is compared verbatim.
    .unwrap_or_else(|_| CompactString::new(text))
}

#[cfg(test)]
mod feature_expected_tests {
    use slug_build_api_v2::RunfilesRepositoryMapping;
    use slug_identity_v2::ApparentRepoName;
    use slug_identity_v2::CanonicalRepoName;

    use super::*;

    #[test]
    fn label_setting_feature_expectations_use_condition_context_and_leave_invalid_text_raw() {
        let metadata = RunfilesPackageMetadata::new(
            CanonicalLabel::parse("@@//conditions:selector")
                .unwrap()
                .package()
                .clone(),
            Arc::new(RunfilesRepositoryMapping::new(
                Arc::from([(
                    ApparentRepoName::new("visible").unwrap(),
                    CanonicalRepoName::new("dep+").unwrap(),
                )]),
                None,
            )),
        );
        assert_eq!(
            canonicalize_feature_expected(":local", &metadata, true),
            "@@//conditions:local"
        );
        assert_eq!(
            canonicalize_feature_expected("@visible//:leaf", &metadata, true),
            "@@dep+//:leaf"
        );
        assert_eq!(
            canonicalize_feature_expected("//bad/...:x", &metadata, true),
            "//bad/...:x"
        );
        assert_eq!(
            canonicalize_feature_expected("@missing//:leaf", &metadata, true),
            "@missing//:leaf"
        );
        assert_eq!(
            canonicalize_feature_expected(":local", &metadata, false),
            ":local"
        );
    }
}

pub(super) async fn analyze(
    ctx: &mut DiceComputations<'_>,
    mode: ConfiguredAnalysisMode,
    workspace: &NormalizedAbsolutePath,
    package: &LoadedPackage,
    target: &ConfiguredTargetKey,
) -> RootAnalysisDriverValue {
    let checked = (|| {
        let BuildSettingLookupStep::Setting(declaration) =
            build_setting_lookup_step(package, target.label())?
        else {
            unreachable!("label_flag dispatch");
        };
        let attrs = package
            .native_attributes(target.label().target().as_str())
            .expect("label_flag schema");
        for (schema, attribute) in attrs.iter() {
            let name = schema.query_name();
            if matches!(
                name,
                "name"
                    | "visibility"
                    | "build_setting_default"
                    | "help"
                    | "scope"
                    | ":alias"
                    | "generator_name"
                    | "generator_function"
                    | "generator_location"
            ) {
                continue;
            }
            let neutral = match &attribute.value {
                CoercedAttributeValue::None => true,
                CoercedAttributeValue::Boolean(v) => !v,
                CoercedAttributeValue::String(v) => v.is_empty(),
                CoercedAttributeValue::StringList(v) => v.is_empty(),
                CoercedAttributeValue::LabelList(v) => v.is_empty(),
                _ => false,
            };
            if !neutral {
                return Err(format!(
                    "native label_flag nondefault {name} is unsupported"
                ));
            }
        }
        match effective_value(
            target.label(),
            &declaration,
            target.configuration().starlark_option(target.label()),
        )? {
            StarlarkOptionValue::Label(label) => Ok(label),
            _ => unreachable!("typed label setting"),
        }
    })();
    let selected = match checked {
        Ok(label) => label,
        Err(error) => return root_analysis_driver_complete(Err(AnalysisError::message(error))),
    };
    let declaration = package
        .targets
        .iter()
        .find(|t| t.name == target.label().target().as_str())
        .expect("label_flag declaration");
    let visibility = package.effective_visibility(declaration);
    let visibility_labels = visibility
        .as_ref()
        .map_or(&[][..], |visibility| visibility.dependency_labels());
    for label in visibility_labels {
        if let Err(error) = require_root_delegating_reference(label, "declaring visibility") {
            return root_analysis_driver_complete(Err(error));
        }
    }
    let inputs = std::iter::once((true, selected))
        .chain(
            visibility_labels
                .iter()
                .cloned()
                .map(|label| (false, label)),
        )
        .collect::<Vec<_>>();
    let cycle_guard = ctx.cycle_guard::<ConfiguredAnalysisCycleGuard>();
    let future = ctx.compute_join(inputs, |ctx, (selected, label)| {
        Box::pin(async move {
            let outcome = if selected {
                selected_child(ctx, mode, workspace, &label, target.configuration()).await
            } else {
                compute_configured_child(
                    ctx,
                    mode,
                    workspace.dupe(),
                    label,
                    target.configuration().clone(),
                )
                .await
            };
            (selected, outcome)
        })
    });
    let outcomes = match cycle_guard {
        Ok(Some(guard)) => match guard.guard_this(future).await {
            Ok(value) => value,
            Err(error) => {
                return root_analysis_driver_complete(Err(AnalysisError::message(
                    error.to_string(),
                )));
            }
        },
        Ok(None) => future.await,
        Err(error) => {
            return root_analysis_driver_complete(Err(AnalysisError::message(error.to_string())));
        }
    };
    let mut child = None;
    let mut visibility_children = Vec::new();
    let mut needs: Option<LoadingPreparationNeeds> = None;
    let mut outer = None;
    let mut error = None;
    for (selected, outcome) in outcomes {
        match outcome {
            LoadingPreparationOutcome::Need(need) => {
                needs = Some(needs.map_or(need.clone(), |current| {
                    current
                        .try_union(&need)
                        .expect("label_flag prerequisite Needs agree")
                }));
            }
            LoadingPreparationOutcome::Complete(Err(e)) => {
                outer.get_or_insert(e);
            }
            LoadingPreparationOutcome::Complete(Ok(result)) => match result.as_ref() {
                Err(e) => {
                    error.get_or_insert_with(|| e.clone());
                }
                Ok(result) if selected => child = Some(result.dupe()),
                Ok(result) if *result.kind() == ConfiguredNodeKind::PackageGroup => {
                    visibility_children.push(result.dupe())
                }
                Ok(result) => {
                    error.get_or_insert_with(|| {
                        AnalysisError::message(format!(
                            "declaring visibility {} does not refer to a package group",
                            result.key().label()
                        ))
                    });
                }
            },
        }
    }
    if let Some(error) = outer {
        return LoadingPreparationOutcome::Complete(Err(error));
    }
    if let Some(need) = needs {
        return LoadingPreparationOutcome::Need(need);
    }
    if let Some(error) = error {
        return root_analysis_driver_complete(Err(error));
    }
    let child = child.expect("selected label_flag prerequisite completed");
    let mut edges = vec![crate::configured_target::ConfiguredEdge::new(
        child.key().clone(),
        crate::configured_target::ConfiguredEdgeKind::AliasActual {
            rule_class: child.prerequisite_rule_class().map(CompactString::new),
        },
    )];
    edges.extend(visibility_children.iter().map(|child| {
        crate::configured_target::ConfiguredEdge::new(
            child.key().clone(),
            crate::configured_target::ConfiguredEdgeKind::DeclaringVisibility,
        )
    }));
    let visibility_rows = visibility_children
        .iter()
        .map(|child| RunfilesPackageClosureRow::from_result(child))
        .collect::<Vec<_>>();
    let capability = declaration.rule_capability().cloned();
    root_analysis_driver_complete(
        native_configured_result(
            package,
            target.clone().into(),
            ConfiguredNodeKind::Alias,
            child.providers().clone(),
            capability,
            edges,
            std::slice::from_ref(&child),
            &visibility_rows,
        )
        .map(|result| result.with_actual_target(child.actual_target().clone())),
    )
}

async fn selected_child(
    ctx: &mut DiceComputations<'_>,
    mode: ConfiguredAnalysisMode,
    workspace: &NormalizedAbsolutePath,
    label: &CanonicalLabel,
    configuration: &ConfigurationKey,
) -> RootAnalysisDriverValue {
    let key = match prepare_configured_node_analysis_driver_with_source_admission(
        ctx,
        mode,
        workspace.dupe(),
        label.clone(),
        configuration.clone(),
        true,
    )
    .await
    {
        LoadingPreparationOutcome::Need(need) => return LoadingPreparationOutcome::Need(need),
        LoadingPreparationOutcome::Complete(Err(error)) => {
            return LoadingPreparationOutcome::Complete(Err(error));
        }
        LoadingPreparationOutcome::Complete(Ok(Err(error))) => {
            return root_analysis_driver_complete(Err(error));
        }
        LoadingPreparationOutcome::Complete(Ok(Ok(key))) => key,
    };
    compute_configured_node_child(ctx, mode, workspace.dupe(), key.node).await
}
