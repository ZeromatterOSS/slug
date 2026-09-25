//! Native srcs-only filegroup: selected graph inputs, no action implementation.
use slug_build_api_v2::AnalysisDepset;
use slug_build_api_v2::Depset;
use slug_build_api_v2::DepsetOrder;
use slug_build_api_v2::ProviderIdentity;
use slug_build_api_v2::ProviderOccurrence;
use slug_build_api_v2::RetainedRunfiles;
use slug_build_api_v2::RunfilesConflictPolicy;
use slug_loading_v2::AttributeProvenance;
use slug_loading_v2::RuleVisibility;

use super::*;

pub(super) async fn analyze(
    ctx: &mut DiceComputations<'_>,
    mode: ConfiguredAnalysisMode,
    workspace: &NormalizedAbsolutePath,
    package: &LoadedPackage,
    target: &ConfiguredTargetKey,
) -> RootAnalysisDriverValue {
    match prepare(ctx, mode, workspace, package, target).await {
        LoadingPreparationOutcome::Need(need) => LoadingPreparationOutcome::Need(need),
        LoadingPreparationOutcome::Complete(Err(error)) => {
            LoadingPreparationOutcome::Complete(Err(error))
        }
        LoadingPreparationOutcome::Complete(Ok(result)) => root_analysis_driver_complete(result),
    }
}

#[derive(Clone, Copy)]
enum InputRole {
    Source(usize),
    Condition,
    Visibility,
}

async fn prepare(
    ctx: &mut DiceComputations<'_>,
    mode: ConfiguredAnalysisMode,
    workspace: &NormalizedAbsolutePath,
    package: &LoadedPackage,
    target: &ConfiguredTargetKey,
) -> AnalysisSemanticOutcome<ConfiguredNodeResult> {
    let configuration = target
        .configuration()
        .slug_configuration()
        .expect("configured native rule");
    let policy = (|| {
        if configuration
            .coverage_enabled()
            .map_err(|e| e.to_string())?
        {
            return Err("native filegroup coverage is unsupported".to_owned());
        }
        Ok((
            configuration
                .filegroup_runfiles_for_data()
                .map_err(|e| e.to_string())?,
            configuration
                .check_visibility()
                .map_err(|e| e.to_string())?,
        ))
    })();
    let (data_policy, check_visibility) = match policy {
        Ok(policy) => policy,
        Err(error) => return analysis_semantic_complete(Err(AnalysisError::message(error))),
    };
    let name = target.label().target().as_str();
    let declaration = package
        .targets
        .iter()
        .find(|t| t.name == name)
        .expect("filegroup declaration");
    let attrs = package
        .native_attributes(name)
        .expect("native filegroup schema");
    let selectors = attrs
        .iter()
        .flat_map(|(_, value)| value.value.selector_key_labels())
        .collect::<SmallSet<_>>()
        .into_iter()
        .collect();
    let conditions = semantic_value!(
        prepare_configured_attribute_conditions(
            ctx,
            mode,
            workspace,
            target.configuration(),
            selectors
        )
        .await
    );
    let selected = (|| {
        for (schema, attribute) in attrs.iter() {
            let name = schema.query_name();
            if matches!(
                name,
                "srcs"
                    | "visibility"
                    | "$config_dependencies"
                    | "name"
                    | "generator_name"
                    | "generator_function"
                    | "generator_location"
            ) {
                continue;
            }
            let value = resolve_configured_attribute(&attribute.value, &conditions.values)
                .map_err(|e| e.to_string())?;
            // Bazel's native filegroup forwards sources without consuming the
            // package's default notice license under default license options.
            if name == "licenses"
                && attribute.provenance == AttributeProvenance::Default
                && matches!(
                    &value,
                    CoercedAttributeValue::StringList(values)
                        if values.len() == 1 && values[0].as_str() == "notice"
                )
            {
                continue;
            }
            if !neutral(&value) {
                return Err(format!("native filegroup nondefault {name} is unsupported"));
            }
        }
        let srcs = resolve_configured_attribute(
            &attrs.get("srcs").expect("native srcs").1.value,
            &conditions.values,
        )
        .map_err(|e| e.to_string())?;
        match srcs {
            CoercedAttributeValue::LabelList(labels) => Ok(labels),
            _ => Err("native filegroup srcs must resolve to labels".to_owned()),
        }
    })();
    let labels = match selected {
        Ok(labels) => labels,
        Err(e) => return analysis_semantic_complete(Err(AnalysisError::message(e))),
    };
    let visibility = package.effective_visibility(declaration);
    let visibility_labels = visibility
        .as_ref()
        .map_or(&[][..], |v| v.dependency_labels());
    for label in visibility_labels {
        if let Err(error) = require_root_delegating_reference(label, "declaring visibility") {
            return analysis_semantic_complete(Err(error));
        }
    }
    let inputs = labels
        .iter()
        .enumerate()
        .map(|(i, l)| (InputRole::Source(i), l.clone()))
        .chain(
            conditions
                .packages
                .iter()
                .map(|k| (InputRole::Condition, k.label().clone())),
        )
        .chain(
            visibility_labels
                .iter()
                .map(|l| (InputRole::Visibility, l.clone())),
        )
        .collect::<Vec<_>>();
    let cycle_guard = ctx.cycle_guard::<ConfiguredAnalysisCycleGuard>();
    let future = ctx.compute_join(inputs, |ctx, (role, label)| {
        Box::pin(async move {
            let result = if matches!(role, InputRole::Source(_)) {
                source_child(ctx, mode, workspace, &label, target.configuration()).await
            } else {
                compute_configured_child(
                    ctx,
                    mode,
                    workspace.dupe(),
                    label.clone(),
                    target.configuration().clone(),
                )
                .await
            };
            (role, label, result)
        })
    });
    // Acquire the guard before polling the join, with no mutex held across DICE.
    let outcomes = match cycle_guard {
        Ok(Some(guard)) => match guard.guard_this(future).await {
            Ok(outcomes) => outcomes,
            Err(e) => {
                return analysis_semantic_complete(Err(AnalysisError::message(e.to_string())));
            }
        },
        Ok(None) => future.await,
        Err(e) => return analysis_semantic_complete(Err(AnalysisError::message(e.to_string()))),
    };
    let mut needs: Option<LoadingPreparationNeeds> = None;
    let mut outer = None;
    let mut error = None;
    let mut sources = Vec::new();
    let mut extra_rows = Vec::new();
    let mut edges = Vec::new();
    for (role, label, outcome) in outcomes {
        match outcome {
            LoadingPreparationOutcome::Need(need) => {
                needs = Some(needs.map_or(need.clone(), |old| {
                    old.try_union(&need).expect("filegroup Needs agree")
                }))
            }
            LoadingPreparationOutcome::Complete(Err(e)) => {
                outer.get_or_insert(e);
            }
            LoadingPreparationOutcome::Complete(Ok(result)) => {
                match result.as_ref() {
                    Err(e) => {
                        error.get_or_insert_with(|| e.clone());
                    }
                    Ok(child) => match role {
                        InputRole::Source(index) => {
                            edges.push(crate::ConfiguredEdge::new(child.key().clone(), crate::ConfiguredEdgeKind::Attribute {
                            attribute: "srcs".into(), index: index as u32, hidden: false,
                            dependency: crate::configured_target::ConfiguredAttributeDependency::Target,
                        }));
                            sources.push((label, child.dupe()));
                        }
                        InputRole::Condition => {
                            extra_rows.push(RunfilesPackageClosureRow::from_result(child))
                        }
                        InputRole::Visibility => {
                            edges.push(crate::ConfiguredEdge::new(
                                child.key().clone(),
                                crate::ConfiguredEdgeKind::DeclaringVisibility,
                            ));
                            extra_rows.push(RunfilesPackageClosureRow::from_result(child));
                        }
                    },
                }
            }
        }
    }
    if let Some(e) = outer {
        return LoadingPreparationOutcome::Complete(Err(e));
    }
    if let Some(needs) = needs {
        return LoadingPreparationOutcome::Need(needs);
    }
    if let Some(e) = error {
        return analysis_semantic_complete(Err(e));
    }
    for (label, child) in &sources {
        let actual = semantic_value!(
            compute_actual_child(ctx, mode, workspace.dupe(), child.dupe())
                .await
                .map(|result| result.map(|value| value.as_ref().clone()))
        );
        if *actual.kind() == ConfiguredNodeKind::PackageGroup {
            return analysis_semantic_complete(Err(AnalysisError::message(
                if child.key() == actual.key() {
                    format!("package group {label} is misplaced in filegroup srcs")
                } else {
                    format!("native filegroup alias to package group is unsupported: {label}")
                },
            )));
        }
        let actual_inventory = semantic_value!(
            compute_configured_package_input(
                ctx,
                mode,
                workspace.dupe(),
                actual.key().label().package().clone(),
                "filegroup prerequisite testonly"
            )
            .await
        );
        let actual_package = match actual_inventory.loaded() {
            Ok(package) => package,
            Err(e) => return analysis_semantic_complete(Err(package_inventory_error(e))),
        };
        if let Err(e) = admit_testonly(actual_package, actual.key().label()) {
            return analysis_semantic_complete(Err(AnalysisError::message(e)));
        }
        if check_visibility {
            let inventory = semantic_value!(
                compute_configured_package_input(
                    ctx,
                    mode,
                    workspace.dupe(),
                    label.package().clone(),
                    "filegroup source visibility"
                )
                .await
            );
            let dependency_package = match inventory.loaded() {
                Ok(package) => package,
                Err(e) => return analysis_semantic_complete(Err(package_inventory_error(e))),
            };
            if let Err(e) = admit_visibility(package, target.label(), dependency_package, label) {
                return analysis_semantic_complete(Err(AnalysisError::message(e)));
            }
        }
    }
    let children = sources
        .into_iter()
        .map(|(_, child)| child)
        .collect::<Vec<_>>();
    let providers = match providers(&children, data_policy) {
        Ok(p) => p,
        Err(e) => return analysis_semantic_complete(Err(AnalysisError::message(e))),
    };
    let closure = match collect_runfiles_packages(
        package,
        &edges,
        children.iter().map(Arc::as_ref),
        extra_rows,
    ) {
        Ok(closure) => closure,
        Err(error) => return analysis_semantic_complete(Err(error)),
    };
    analysis_semantic_complete(Ok(ConfiguredNodeResult::new_rule(
        target.clone(),
        providers,
        declaration.rule_capability().cloned(),
        closure,
    )
    .with_edges(edges)))
}

fn neutral(value: &CoercedAttributeValue) -> bool {
    match value {
        CoercedAttributeValue::None => true,
        CoercedAttributeValue::Boolean(value) => !value,
        CoercedAttributeValue::String(value) => value.is_empty(),
        CoercedAttributeValue::StringList(value) => value.is_empty(),
        CoercedAttributeValue::LabelList(value) => value.is_empty(),
        _ => false,
    }
}

fn admit_testonly(package: &LoadedPackage, label: &CanonicalLabel) -> Result<(), String> {
    let mut name = label.target().as_str();
    let mut declaration = package.targets.iter().find(|target| target.name == name);
    if let Some(slug_loading_v2::PackageTarget {
        kind: PackageTargetKind::GeneratedFile {
            generating_rule, ..
        },
        ..
    }) = declaration
    {
        name = generating_rule;
        declaration = package.targets.iter().find(|target| target.name == name);
    }
    let value = match declaration.map(|target| &target.kind) {
        Some(PackageTargetKind::StarlarkRule(rule)) => rule
            .values()
            .iter()
            .find(|attribute| attribute.declaration_name == "testonly")
            .map(|attribute| attribute.value.as_ref()),
        Some(_) => package
            .native_attributes(name)
            .and_then(|attrs| attrs.get("testonly"))
            .map(|(_, attr)| &attr.value),
        None => None,
    };
    if value.is_some_and(|value| !matches!(value, CoercedAttributeValue::Boolean(false))) {
        return Err(format!(
            "native filegroup testonly prerequisite is unsupported: {label}"
        ));
    }
    Ok(())
}

fn admit_visibility(
    consumer: &LoadedPackage,
    owner: &CanonicalLabel,
    package: &LoadedPackage,
    label: &CanonicalLabel,
) -> Result<(), String> {
    let declaration = package
        .targets
        .iter()
        .find(|t| t.name == label.target().as_str());
    let visibility = declaration
        .and_then(|t| package.effective_visibility(t))
        .unwrap_or(RuleVisibility::Private);
    if visibility.is_public() {
        return Ok(());
    }
    let macro_origin = package.macro_origin(label.target().as_str()).is_some()
        || declaration.is_some_and(|t| match &t.kind {
            PackageTargetKind::GeneratedFile {
                generating_rule, ..
            } => package.macro_origin(generating_rule).is_some(),
            _ => false,
        });
    if visibility.is_private()
        && !macro_origin
        && consumer.macro_origin(owner.target().as_str()).is_none()
        && owner.package() == label.package()
    {
        return Ok(());
    }
    Err(format!(
        "native filegroup srcs visibility is unsupported or private: {owner} -> {label}"
    ))
}

async fn source_child(
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

fn providers(
    children: &[Arc<ConfiguredNodeResult>],
    data_policy: bool,
) -> Result<ProviderCollection, String> {
    let defaults = children
        .iter()
        .filter_map(|child| child.providers().default_info())
        .collect::<Vec<_>>();
    let files = AnalysisDepset::new(
        DepsetOrder::Default,
        vec![],
        defaults.iter().map(|info| info.files().clone()).collect(),
    )
    .map_err(|e| e.to_string())?;
    let flattened = files.to_list();
    let executable = if let [value] = flattened.as_slice() {
        let slug_build_api_v2::AnalysisValueKind::Artifact(artifact) = value.kind() else {
            return Err("filegroup files must be Artifacts".to_owned());
        };
        Some(artifact.clone())
    } else {
        None
    };
    let default_runfiles = runfiles(None, defaults.iter().map(|info| &info.default_runfiles))?;
    let data_runfiles = runfiles(
        Some(files.clone()),
        defaults
            .iter()
            .filter(|_| data_policy)
            .map(|info| &info.data_runfiles),
    )?;
    let default = DefaultInfo::from_effective(files, default_runfiles, data_runfiles, executable)
        .map_err(|e| e.to_string())?;
    let empty = AnalysisValue::depset(AnalysisDepset::empty(DepsetOrder::Default));
    let providers = ProviderCollection::new(vec![
        ProviderValue::DefaultInfo(default),
        ProviderValue::Occurrence(ProviderOccurrence::new(
            ProviderIdentity::builtin("InstrumentedFilesInfo"),
            [
                ("instrumented_files", empty.clone()),
                ("metadata_files", empty),
            ],
        )),
    ])
    .map_err(|e| e.to_string())?;
    let validations = children
        .iter()
        .filter_map(|child| child.providers().output_group_info())
        .filter_map(|info| info.groups().get("_validation"))
        .filter(|files| !files.is_empty())
        .cloned()
        .collect();
    crate::output_groups::complete_rule_output_groups(providers, validations)
}

fn runfiles<'a>(
    files: Option<AnalysisDepset>,
    values: impl IntoIterator<Item = &'a RetainedRunfiles>,
) -> Result<RetainedRunfiles, String> {
    let values = values.into_iter().collect::<Vec<_>>();
    let conflict_policy = if values
        .iter()
        .any(|v| v.conflict_policy == RunfilesConflictPolicy::Error)
    {
        RunfilesConflictPolicy::Error
    } else {
        RunfilesConflictPolicy::Warn
    };
    let values = values
        .into_iter()
        .filter(|v| !v.is_empty())
        .collect::<Vec<_>>();
    // Admitted Bzlmod workspace name, matching the existing ctx.runfiles owner.
    let prefix = RetainedRunfiles::empty().repository_prefix;
    if values.iter().any(|v| v.repository_prefix != prefix) {
        return Err("filegroup runfiles repository prefixes differ".to_owned());
    }
    Ok(RetainedRunfiles {
        files: AnalysisDepset::new(
            DepsetOrder::Postorder,
            vec![],
            files
                .into_iter()
                .chain(values.iter().map(|v| v.files.clone()))
                .collect(),
        )
        .map_err(|e| e.to_string())?,
        symlinks: Depset::new(
            DepsetOrder::Default,
            vec![],
            values.iter().map(|v| v.symlinks.clone()).collect(),
        )
        .map_err(|e| e.to_string())?,
        root_symlinks: Depset::new(
            DepsetOrder::Default,
            vec![],
            values.iter().map(|v| v.root_symlinks.clone()).collect(),
        )
        .map_err(|e| e.to_string())?,
        empty_filenames: Depset::new(
            DepsetOrder::Default,
            vec![],
            values.iter().map(|v| v.empty_filenames.clone()).collect(),
        )
        .map_err(|e| e.to_string())?,
        conflict_policy,
        repository_prefix: prefix,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(name: &str) -> AnalysisValue {
        AnalysisValue::artifact(AnalysisArtifact::Source(
            CanonicalLabel::parse(&format!("@@//pkg:{name}")).unwrap(),
        ))
    }

    #[test]
    fn native_runfiles_preserve_compile_root_mixed_orders_and_auxiliary_dags() {
        let topo =
            AnalysisDepset::new(DepsetOrder::Topological, vec![file("a"), file("b")], vec![])
                .unwrap();
        let stable = AnalysisDepset::new(DepsetOrder::Default, vec![], vec![topo]).unwrap();
        assert_eq!(stable.to_list(), [file("b"), file("a")]);
        let mut original = RetainedRunfiles::empty();
        original.files = stable.clone();
        let link = file("link");
        let slug_build_api_v2::AnalysisValueKind::Artifact(artifact) = link.kind() else {
            unreachable!()
        };
        original.symlinks = Depset::from_direct(
            DepsetOrder::Default,
            vec![slug_build_api_v2::RunfilesSymlink::new(
                "s",
                artifact.clone(),
            )],
        )
        .unwrap();
        original.root_symlinks = Depset::from_direct(
            DepsetOrder::Default,
            vec![slug_build_api_v2::RunfilesSymlink::new(
                "r",
                artifact.clone(),
            )],
        )
        .unwrap();
        original.empty_filenames =
            Depset::from_direct(DepsetOrder::Default, vec!["empty".to_owned()]).unwrap();
        let snapshot = original.clone();
        let merged = runfiles(None, [&original, &original]).unwrap();
        assert_eq!(merged.files.order(), DepsetOrder::Postorder);
        assert_eq!(merged.files.to_list(), [file("b"), file("a")]);
        assert_eq!(merged.symlinks.to_list(), original.symlinks.to_list());
        assert_eq!(
            merged.root_symlinks.to_list(),
            original.root_symlinks.to_list()
        );
        assert_eq!(merged.empty_filenames.to_list(), ["empty"]);
        assert_eq!(original, snapshot);
        // The singleton case still gets the native compile-order builder root.
        assert_eq!(
            runfiles(None, [&original]).unwrap().files.order(),
            DepsetOrder::Postorder
        );
        let added = runfiles(
            Some(AnalysisDepset::new(DepsetOrder::Default, vec![file("data")], vec![]).unwrap()),
            [&original],
        )
        .unwrap();
        assert_eq!(added.files.to_list(), [file("data"), file("b"), file("a")]);
        assert_eq!(
            runfiles(Some(stable), []).unwrap().files.to_list(),
            [file("b"), file("a")]
        );
    }

    #[test]
    fn native_runfiles_preserve_empty_conflict_policy_and_reject_foreign_prefix() {
        let mut empty = RetainedRunfiles::empty();
        empty.conflict_policy = RunfilesConflictPolicy::Error;
        empty.repository_prefix = "ignored-empty-prefix".into();
        let result = runfiles(None, [&empty]).unwrap();
        assert!(result.is_empty());
        assert_eq!(result.conflict_policy, RunfilesConflictPolicy::Error);
        assert_eq!(result.repository_prefix, "_main");
        let mut foreign = RetainedRunfiles::empty();
        foreign.files = AnalysisDepset::new(DepsetOrder::Default, vec![file("x")], vec![]).unwrap();
        foreign.repository_prefix = "foreign".into();
        assert!(
            runfiles(None, [&foreign])
                .unwrap_err()
                .contains("prefixes differ")
        );
        let mut incompatible = RetainedRunfiles::empty();
        incompatible.files =
            AnalysisDepset::new(DepsetOrder::Preorder, vec![file("x")], vec![]).unwrap();
        assert!(
            runfiles(None, [&incompatible]).is_err(),
            "a native compile-order runfiles builder cannot accept preorder children"
        );
    }
}
