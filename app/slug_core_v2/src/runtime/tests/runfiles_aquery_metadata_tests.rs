#[tokio::test]
async fn runfiles_aquery_metadata_guards_identity_environment_and_priority() {
    use slug_analysis_v2::ConfiguredActionAspectProvenance;
    use slug_analysis_v2::ConfiguredActionOwnerContext;
    use slug_analysis_v2::ConfiguredExecGroup;
    use slug_analysis_v2::PlatformSemanticFact;
    use slug_build_api_v2::ActionKind;
    use slug_build_api_v2::RunfilesPackageDepset;
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let mut transaction = build_root_transaction(&dice, runfiles_epoch(30, &[])).await;
    let outcome = transaction.compute(&runfiles_key("//:tool")).await.unwrap();
    let evaluation = runfiles_evaluation(&outcome);
    let owner = evaluation.targets[0].analysis.as_ref().unwrap();
    let specs = owner.actions()[1..]
        .iter()
        .map(|action| action.runfiles_support_spec().unwrap().clone())
        .collect::<Vec<_>>();
    let context = owner.actions()[1].context().clone();
    let snapshot = owner.as_ref().clone();
    let configured = |spec: RunfilesSupportActionSpec| {
        with_specs(
            owner,
            vec![ActionSpec::runfiles_support(spec)],
            context.clone(),
        )
    };
    let rejected = |spec, expected| {
        let node = configured(spec);
        let saved = node.clone();
        assert_eq!(
            ConfiguredRunfilesSupportActionView::from_configured_action(&node.actions()[0])
                .unwrap_err(),
            expected
        );
        assert_eq!(node, saved);
    };
    let unresolved = with_specs(
        owner,
        vec![ActionSpec::runfiles_support(specs[1].clone())],
        Arc::new(
            ConfiguredActionOwnerContext::unresolved_default(context.owner().clone()).unwrap(),
        ),
    );
    assert_eq!(
        ConfiguredRunfilesSupportActionView::from_configured_action(&unresolved.actions()[0])
            .unwrap_err(),
        "configured runfiles-support action requires a selected execution platform"
    );
    let custom = with_specs(
        owner,
        vec![
            ActionSpec::runfiles_support(specs[1].clone()).with_exec_properties(
                std::collections::BTreeMap::from([("custom".to_owned(), "value".to_owned())]),
            ),
        ],
        context.clone(),
    );
    assert_eq!(
        ConfiguredRunfilesSupportActionView::from_configured_action(&custom.actions()[0])
            .unwrap_err(),
        "configured runfiles-support query requires default action group and properties"
    );
    let mut empty = specs[3].support().as_ref().clone();
    empty.runfiles = slug_build_api_v2::RetainedRunfiles::empty();
    let mut empty_spec = specs[3].clone();
    support_spec(&mut empty_spec, Arc::new(empty));
    rejected(
        empty_spec,
        "configured runfiles-support query requires nonempty runfiles",
    );
    let mut missing = specs[3].support().as_ref().clone();
    missing.manifest = None;
    let mut invalid = specs[3].clone();
    support_spec(&mut invalid, Arc::new(missing));
    rejected(
        invalid.clone(),
        "configured runfiles-support query requires a public manifest",
    );
    let mut missing = specs[3].support().as_ref().clone();
    missing.repo_mapping_manifest = None;
    let mut no_mapping = specs[3].clone();
    support_spec(&mut no_mapping, Arc::new(missing));
    rejected(
        no_mapping,
        "configured runfiles-support query requires a repository mapping manifest",
    );
    for index in 0..5 {
        let mut support = specs[3].support().as_ref().clone();
        let original = match &support.input_manifest {
            AnalysisArtifact::Derived { output, .. } => output,
            _ => unreachable!(),
        };
        let mut retained = context.owner().artifact_owner();
        let replacement = match index {
            0 => AnalysisArtifact::Source(CanonicalLabel::parse("@@//:manifest").unwrap()),
            1 => AnalysisArtifact::Derived {
                owner: retained,
                output: ActionOutput::new(
                    original.root().clone(),
                    original.root_relative_path(),
                    ActionOutputKind::Directory,
                ),
            },
            2 => AnalysisArtifact::Derived {
                owner: retained,
                output: ActionOutput::new_constant(
                    original.root().clone(),
                    original.root_relative_path(),
                ),
            },
            3 => {
                retained = slug_analysis_v2::ConfiguredTargetKey::new(
                    CanonicalLabel::parse("@@//:other").unwrap(),
                    context.owner().configuration().clone(),
                )
                .artifact_owner();
                AnalysisArtifact::Derived {
                    owner: retained,
                    output: original.clone(),
                }
            }
            _ => {
                let different = context
                    .owner()
                    .configuration()
                    .slug_configuration()
                    .unwrap()
                    .clone()
                    .with_host_platform_label(
                        &CanonicalLabel::parse("@@//:different-host").unwrap(),
                    );
                let root = slug_build_api_v2::DerivedArtifactRoot::bin(different).unwrap();
                assert_ne!(root, *original.root());
                AnalysisArtifact::Derived {
                    owner: retained,
                    output: ActionOutput::new(
                        root,
                        original.root_relative_path(),
                        ActionOutputKind::File,
                    ),
                }
            }
        };
        support.input_manifest = replacement;
        let mut spec = specs[3].clone();
        support_spec(&mut spec, Arc::new(support));
        rejected(
            spec,
            match index {
                0 => "configured runfiles-support roles must be configured Derived outputs",
                1 | 2 => "configured runfiles-support roles require ordinary expected output kinds",
                3 => "configured runfiles-support roles must have the exact action owner",
                _ => "configured runfiles-support roles require the action configuration root",
            },
        );
    }
    let mut wrong_output = specs[3].clone();
    if let RunfilesSupportActionSpec::RunfilesTree { output, .. } = &mut wrong_output {
        *output = ActionOutput::new(
            output.root().clone(),
            "other.runfiles",
            ActionOutputKind::RunfilesTree,
        );
    }
    rejected(
        wrong_output,
        "configured runfiles-support output differs from its exact support role",
    );
    let mut source = specs[1].clone();
    if let RunfilesSupportActionSpec::SourceSymlinkManifest { remotable, .. } = &mut source {
        *remotable = true;
    }
    rejected(
        source,
        "configured source manifest query requires default nonremotable policy",
    );
    for index in 0..3 {
        let mut mapping = specs[0].clone();
        if let RunfilesSupportActionSpec::RepoMappingManifest {
            packages,
            workspace_name,
            emit_compact_repo_mapping,
            ..
        } = &mut mapping
        {
            match index {
                0 => *packages = RunfilesPackageDepset::empty(),
                1 => *workspace_name = "wrong".into(),
                _ => *emit_compact_repo_mapping = false,
            }
        }
        rejected(
            mapping,
            "configured repository mapping query requires default retained metadata",
        );
    }
    for (value, expected) in [
        (
            AnalysisValue::string("not an artifact"),
            "configured runfiles-support files must be typed artifacts",
        ),
        (
            AnalysisValue::artifact(specs[3].support().tree.clone()),
            "configured runfiles-support query requires Source or Derived file/tree/symlink inputs",
        ),
        (
            AnalysisValue::artifact(AnalysisArtifact::WorkspaceStatus(
                slug_build_api_v2::WorkspaceStatusArtifacts::new(
                    NormalizedAbsolutePath::new("/workspace").unwrap(),
                )
                .stable()
                .clone(),
            )),
            "configured runfiles-support query requires Source or Derived file/tree/symlink inputs",
        ),
    ] {
        let mut support = specs[3].support().as_ref().clone();
        support.runfiles.files =
            AnalysisDepset::new(DepsetOrder::Default, vec![value], vec![]).unwrap();
        let mut spec = specs[3].clone();
        support_spec(&mut spec, Arc::new(support));
        rejected(spec, expected);
    }
    let env = slug_configuration_v2::RetainedActionEnvironment::default()
        .for_action(false, [("z", "last"), ("E", "a b"), ("a", "first")]);
    let mut tree = specs[2].clone();
    if let RunfilesSupportActionSpec::SymlinkTree { environment, .. } = &mut tree {
        *environment = env;
    }
    let platform_context = Arc::new(
        ConfiguredActionOwnerContext::new(
            context.owner().clone(),
            ConfiguredExecGroup::Default,
            context.execution_platform().unwrap().clone(),
            PlatformSemanticFact {
                exec_properties: Arc::from([
                    ("a".into(), "a b".into()),
                    ("z".into(), "last".into()),
                ]),
                missing_toolchain_error: None,
            },
            &Default::default(),
            &Default::default(),
            Vec::new(),
            None,
            ConfiguredActionAspectProvenance::Absent,
        )
        .unwrap(),
    );
    let enriched = with_specs(
        owner,
        vec![
            ActionSpec::runfiles_support(tree),
            ActionSpec::runfiles_support(specs[0].clone()),
            ActionSpec::runfiles_support(specs[1].clone()),
            ActionSpec::runfiles_support(specs[3].clone()),
        ],
        platform_context,
    );
    let text = render_support(&enriched);
    let blocks = runfiles_records(&text);
    assert!(
        blocks[0].ends_with(
            "  Environment: [E=a b, a=first, z=last]\n  ExecutionInfo: {a: 'a b', z: last}\n"
        ),
        "{text}"
    );
    for block in &blocks[1..] {
        assert!(!block.contains("  Environment:"));
        assert!(!block.contains("  ExecutionInfo:"));
    }
    let mut support = specs[3].support().as_ref().clone();
    let output = owner.actions()[0].outputs()[0].clone();
    let derived = AnalysisArtifact::Derived {
        owner: context.owner().artifact_owner(),
        output: output.clone(),
    };
    let source = AnalysisArtifact::Source(
        CanonicalLabel::parse(&format!("@@//:{}", output.path())).unwrap(),
    );
    assert_ne!(derived, source);
    assert_eq!(derived.path(), source.path());
    support.runfiles.files = AnalysisDepset::new(
        DepsetOrder::Default,
        vec![
            AnalysisValue::artifact(derived.clone()),
            AnalysisValue::artifact(source),
        ],
        vec![],
    )
    .unwrap();
    let mut spec = specs[3].clone();
    support_spec(&mut spec, Arc::new(support));
    let identity_text = render_support(&configured(spec));
    assert!(
        identity_text.contains(&format!("{}, {}", output.path(), output.path())),
        "{identity_text}"
    );
    let malformed = Arc::new(with_specs(
        owner,
        vec![
            ActionSpec::new(
                ActionKind::Write {
                    content: String::new(),
                    is_executable: true,
                },
                "FileWrite",
                vec![ActionOutput::new(
                    owner.actions()[0].outputs()[0].root().clone(),
                    "early'quoted",
                    ActionOutputKind::File,
                )],
            ),
            ActionSpec::runfiles_support(invalid),
        ],
        context,
    ));
    let early_view = ResolvedFileWriteSemanticView::from_configured_action(
        slug_analysis_v2::ConfiguredActionView::from_configured_action(&malformed.actions()[0])
            .unwrap(),
    );
    assert_eq!(
        crate::runtime::format_file_write_aquery_text(&early_view),
        Err("FileWrite aquery text requires a normalized relative output")
    );
    let mut targets = evaluation.targets.to_vec();
    targets[0].analysis = Some(malformed.clone());
    let altered = BuildCommandEvaluation {
        workspace: evaluation.workspace.dupe(),
        anchor: evaluation.anchor.clone(),
        targets: targets.into(),
        action_closure: super::ValidatedActionClosure::new(vec![malformed].into()).unwrap(),
    };
    for scope in [AqueryScope::Literal, AqueryScope::Deps] {
        assert_eq!(
            crate::runtime::format_aquery_text_output_for_scope(&altered, scope),
            Err("configured runfiles-support query requires a public manifest".into())
        );
    }
    assert_eq!(owner.as_ref(), &snapshot);
}

#[tokio::test]
async fn runfiles_aquery_progress_uses_unsorted_primary_and_special_tree_header() {
    use slug_analysis_v2::ConfiguredActionAspectProvenance;
    use slug_analysis_v2::ConfiguredActionOwnerContext;
    use slug_analysis_v2::ConfiguredExecGroup;
    use slug_analysis_v2::ConfiguredNodeResult;
    use slug_analysis_v2::ConfiguredTargetKey;
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let mut transaction = build_root_transaction(&dice, runfiles_epoch(40, &[])).await;
    let outcome = transaction.compute(&runfiles_key("//:tool")).await.unwrap();
    let original = runfiles_evaluation(&outcome).targets[0]
        .analysis
        .as_ref()
        .unwrap();
    let old_context = original.actions()[1].context();
    let key = ConfiguredTargetKey::new(
        CanonicalLabel::parse("@@//:%{output}").unwrap(),
        old_context.owner().configuration().clone(),
    );
    let artifact_owner = key.artifact_owner();
    let root = original.actions()[0].outputs()[0].root().clone();
    let artifact = |path: &str, kind| AnalysisArtifact::Derived {
        owner: artifact_owner.clone(),
        output: ActionOutput::new(root.clone(), path, kind),
    };
    let mut support = original.actions()[1]
        .runfiles_support_spec()
        .unwrap()
        .support()
        .as_ref()
        .clone();
    support.tree = artifact("virtual.runfiles", ActionOutputKind::RunfilesTree);
    support.input_manifest = artifact("manifest-%{label}-%{input}", ActionOutputKind::File);
    support.manifest = Some(artifact("tree-%{input}/MANIFEST", ActionOutputKind::File));
    support.repo_mapping_manifest = Some(artifact("mapping", ActionOutputKind::File));
    let first = artifact("z-primary", ActionOutputKind::Symlink);
    let last = artifact("a-later", ActionOutputKind::Symlink);
    let directory = artifact("directory", ActionOutputKind::Directory);
    support.runfiles.files = AnalysisDepset::new(
        DepsetOrder::Default,
        vec![
            AnalysisValue::artifact(first.clone()),
            AnalysisValue::artifact(last.clone()),
            AnalysisValue::artifact(directory.clone()),
        ],
        vec![],
    )
    .unwrap();
    let support = Arc::new(support);
    let mut specs = original.actions()[1..]
        .iter()
        .map(|action| action.runfiles_support_spec().unwrap().clone())
        .collect::<Vec<_>>();
    for spec in &mut specs {
        support_spec(spec, support.clone());
        let selected = match spec {
            RunfilesSupportActionSpec::RepoMappingManifest { output, .. } => {
                (output, support.repo_mapping_manifest.as_ref().unwrap())
            }
            RunfilesSupportActionSpec::SourceSymlinkManifest { output, .. } => {
                (output, &support.input_manifest)
            }
            RunfilesSupportActionSpec::SymlinkTree { output, .. } => {
                (output, support.manifest.as_ref().unwrap())
            }
            RunfilesSupportActionSpec::RunfilesTree { output, .. } => (output, &support.tree),
        };
        let AnalysisArtifact::Derived { output, .. } = selected.1 else {
            unreachable!()
        };
        *selected.0 = output.clone();
    }
    let context = Arc::new(
        ConfiguredActionOwnerContext::new(
            key.clone(),
            ConfiguredExecGroup::Default,
            old_context.execution_platform().unwrap().clone(),
            old_context.raw_platform_fact().unwrap().clone(),
            &Default::default(),
            &Default::default(),
            Vec::new(),
            None,
            ConfiguredActionAspectProvenance::Absent,
        )
        .unwrap(),
    );
    let node = ConfiguredNodeResult::new_rule(
        key,
        original.providers().clone(),
        None,
        original.runfiles_packages().clone(),
    )
    .with_action_specs(
        specs
            .into_iter()
            .map(ActionSpec::runfiles_support)
            .collect(),
        vec![context],
    )
    .unwrap();
    // Synthetic retained payload: no unresolved-symlink producer execution is admitted.
    let text = render_support(&node);
    let blocks = runfiles_records(&text);
    assert!(blocks[0].starts_with("action 'Writing repo mapping manifest for //:mapping'\n"));
    assert!(
        blocks[1]
            .starts_with("action 'Creating source manifest for //:manifest-%{label}-z-primary'\n")
    );
    assert!(blocks[1].contains(&format!("  Inputs: [{}, {}]\n", last.path(), first.path())));
    assert!(!blocks[1].contains(directory.path().as_ref()));
    assert!(blocks[2].starts_with(&format!(
        "action 'Creating runfiles tree {}/tree-manifest-%{{label}}-%{{input}}'\n",
        root.path()
    )));
    assert!(blocks[2].contains(&format!("  Inputs: [{}]\n", support.input_manifest.path())));
    assert!(blocks[3].starts_with("runfiles for //:%{output}\n"));
    assert!(blocks[3].contains(directory.path().as_ref()));
    assert!(!text.contains("(TreeArtifact)"));
}
