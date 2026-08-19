use super::*;

fn finalize_epoch_for_test(
    runtime: &WorkspaceRuntime,
    token: crate::runtime::repository_io::RepositorySessionToken,
    certificate: &SourceCertificate,
    epoch: &PathObservationEpoch,
) -> NativeFinalization {
    runtime.runtime.block_on(async {
        let terminal = runtime.dice.updater().existing_state().await;
        let mut selected = runtime.dice.updater();
        selected
            .changed_to(vec![(PathObservationEpochKey, epoch.clone())])
            .unwrap();
        runtime
            .request_revision
            .finalize_native(&terminal, certificate, selected, epoch, |demands| {
                runtime
                    .repository_materializer
                    .observe_native(token, demands)
                    .map_err(|error| RequestRevisionError::Observation(format!("{error:?}")))
            })
            .await
            .unwrap()
    })
}

    #[test]
    fn multi_target_exported_sources_do_not_enter_revision_bridge() {
        let workspace = tempfile::tempdir().unwrap();
        fs::write(workspace.path().join("MODULE.bazel"), "").unwrap();
        fs::write(
            workspace.path().join("BUILD.bazel"),
            "exports_files([\"one.txt\", \"two.txt\"])\n",
        )
        .unwrap();
        fs::write(workspace.path().join("one.txt"), b"one").unwrap();
        fs::write(workspace.path().join("two.txt"), b"two").unwrap();

        let runtime = test_runtime(workspace.path()).unwrap();
        let workspace_path = NormalizedAbsolutePath::new(workspace.path().to_path_buf()).unwrap();
        let root = BuildCommandRootKey::new(
            workspace_path.clone(),
            &[
                TargetPattern::parse("//:one.txt").unwrap(),
                TargetPattern::parse("//:two.txt").unwrap(),
            ],
            build_test_configuration("target"),
        )
        .unwrap();
        assert!(!root.initializes_request_revision());

        let driven = runtime
            .drive_command(NativeDemandRequestInputBundle::normalized_initial(), root)
            .unwrap();
        let evaluation = driven
            .accepted
            .terminal_for_test()
            .as_ref()
            .as_ref()
            .unwrap();
        assert_eq!(evaluation.targets.len(), 2);
        assert!(evaluation.targets.iter().all(|target| {
            target.completion == BuildTargetCompletion::ObservedExportedSource
                && target.source_certificate.is_none()
        }));

        let revision = runtime.runtime.block_on(async {
            let mut transaction = runtime.dice.updater().existing_state().await;
            transaction
                .compute(&RequestRevisionKey::new(workspace_path))
                .await
        });
        assert!(revision.is_err());
    }

    #[test]
    fn root_exported_source_revision_bridge_retries_changed_terminal_and_preserves_epoch() {
        let workspace = tempfile::tempdir().unwrap();
        fs::write(workspace.path().join("MODULE.bazel"), "").unwrap();
        fs::write(
            workspace.path().join("BUILD.bazel"),
            "print(\"BUILD_EVENT\")\nexports_files([\"source.txt\"])\n",
        )
        .unwrap();
        let source = workspace.path().join("source.txt");
        fs::write(&source, b"V1").unwrap();

        let runtime = Arc::new(test_runtime(workspace.path()).unwrap());
        let workspace_path = NormalizedAbsolutePath::new(workspace.path().to_path_buf()).unwrap();
        let root = BuildCommandRootKey::new(
            workspace_path,
            &[TargetPattern::parse("//:source.txt").unwrap()],
            build_test_configuration("target"),
        )
        .unwrap();
        assert!(root.initializes_request_revision());

        let drive = || {
            runtime
                .drive_command(
                    NativeDemandRequestInputBundle::normalized_initial(),
                    root.clone(),
                )
                .unwrap()
        };
        let drive_after_gate = |mutate: &dyn Fn()| {
            runtime.request_revision.arm_native_finalize_gate();
            let thread_runtime = runtime.clone();
            let thread_root = root.clone();
            let handle = std::thread::spawn(move || {
                thread_runtime
                    .drive_command(
                        NativeDemandRequestInputBundle::normalized_initial(),
                        thread_root,
                    )
                    .unwrap()
            });
            runtime.request_revision.wait_native_finalize_gate();
            mutate();
            runtime.request_revision.release_native_finalize_gate();
            handle.join().unwrap()
        };
        let accepted_epoch = || {
            runtime
                .native_demand_sessions
                .state
                .lock()
                .unwrap()
                .accepted
                .path_observations
                .clone()
        };
        let source_demand = PathObservationDemand::new(
            PathObservationNamespace::Host,
            NormalizedAbsolutePath::new(source.clone()).unwrap(),
            PathObservationOperation::FileBytes,
        );
        let assert_source = |epoch: &PathObservationEpoch, expected: Option<&[u8]>| {
            let observation = epoch.get(&source_demand).expect("source stays selected");
            match (observation.as_ref(), expected) {
                (
                    PathObservationResult::FileBytes(PathOperationResult::Present(actual)),
                    Some(expected),
                ) => assert_eq!(actual.as_ref(), expected),
                (PathObservationResult::FileBytes(PathOperationResult::Missing), None) => {}
                (actual, expected) => {
                    panic!("unexpected source observation {actual:?} for {expected:?}")
                }
            }
        };

        let v1 = drive();
        assert!(
            v1.accepted
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .is_observed_exported_source()
        );
        assert_eq!(accepted_output_text(&v1.accepted), ["BUILD_EVENT"]);
        let v1_epoch = accepted_epoch();
        assert_source(&v1_epoch, Some(b"V1"));
        assert!(v1_epoch.observations().len() > 1);

        let v2 = drive_after_gate(&|| fs::write(&source, b"V2").unwrap());
        assert_eq!(v2.attempts, 2);
        assert!(v2.accepted.terminal_for_test().as_ref().is_ok());
        assert!(accepted_output_text(&v2.accepted).is_empty());
        let v2_epoch = accepted_epoch();
        assert_source(&v2_epoch, Some(b"V2"));
        assert_eq!(v2_epoch.observations().len(), v1_epoch.observations().len());

        let warm = drive();
        assert_eq!(warm.attempts, 1);
        assert!(warm.accepted.terminal_for_test().as_ref().is_ok());

        let missing = drive_after_gate(&|| fs::remove_file(&source).unwrap());
        assert_eq!(missing.attempts, 2);
        let missing_error = missing
            .accepted
            .terminal_for_test()
            .as_ref()
            .as_ref()
            .unwrap_err();
        assert!(missing_error.source_certificate().is_some());
        assert_source(&accepted_epoch(), None);

        let restored = drive_after_gate(&|| fs::write(&source, b"V1").unwrap());
        assert_eq!(restored.attempts, 2);
        assert!(restored.accepted.terminal_for_test().as_ref().is_ok());
        assert_source(&accepted_epoch(), Some(b"V1"));
    }

    #[cfg(unix)]
    #[test]
    fn epoch_certificate_reobserves_retained_materialization_and_symlink_lifecycle() {
        use compact_str::CompactString;
        use sha2::Digest;
        use slug_bzlmod_v2::OverrideAttributeValue;
        use slug_bzlmod_v2::RepoRuleId;
        use slug_bzlmod_v2::RepoSpec;
        use slug_bzlmod_v2::RepositoryMaterializationKind;
        use sha2::Sha256;

        let stable_parent = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/debug/incremental/slug-epoch-source-certificate");
        fs::create_dir_all(&stable_parent).unwrap();
        let workspace = tempfile::tempdir_in(stable_parent).unwrap();
        let archive = workspace.path().join("empty.tar");
        fs::write(&archive, vec![0; 1_024]).unwrap();
        let digest = format!("{:x}", Sha256::digest(fs::read(&archive).unwrap()));
        let attributes = [
            (
                CompactString::new("urls"),
                OverrideAttributeValue::Iterable(Arc::new([
                    OverrideAttributeValue::String(
                        url::Url::from_file_path(&archive)
                            .unwrap()
                            .to_string()
                            .into(),
                    ),
                ])),
            ),
            (
                CompactString::new("sha256"),
                OverrideAttributeValue::String(digest.into()),
            ),
            (
                CompactString::new("type"),
                OverrideAttributeValue::String("tar".into()),
            ),
        ];
        let workspace_path = NormalizedAbsolutePath::new(workspace.path().to_path_buf()).unwrap();
        let request = Arc::new(RepositoryMaterializationRequest {
            id: RepositoryMaterializationRequestId {
                workspace: workspace_path.clone(),
                canonical_repo: slug_identity_v2::CanonicalRepoName::new("cert").unwrap(),
            },
            repo_spec: RepoSpec {
                rule_id: RepoRuleId {
                    bzl_file: CanonicalLabel::parse(
                        "@@bazel_tools//tools/build_defs/repo:http.bzl",
                    )
                    .unwrap(),
                    rule_name: "http_archive".into(),
                },
                attributes: Arc::new(SmallMap::from_iter(attributes)),
            },
            kind: RepositoryMaterializationKind::Immutable,
        });
        let runtime = test_runtime(workspace.path()).unwrap();
        let token = runtime.repository_materializer.begin().unwrap();
        runtime
            .repository_materializer
            .preflight_native(token, std::iter::empty())
            .unwrap();
        runtime
            .repository_materializer
            .materialize_native(token, request, RepositoryMaterializationGeneration(1))
            .unwrap();

        let target_a = workspace.path().join("a");
        let target_b = workspace.path().join("b");
        let logical = workspace.path().join("logical");
        let unrelated = workspace.path().join("unrelated");
        fs::write(&target_a, b"same").unwrap();
        fs::write(&target_b, b"same").unwrap();
        fs::write(&unrelated, b"unrelated").unwrap();
        std::os::unix::fs::symlink(&target_a, &logical).unwrap();
        let readlink = PathObservationDemand::new(
            PathObservationNamespace::Host,
            NormalizedAbsolutePath::new(logical.clone()).unwrap(),
            PathObservationOperation::ReadLink,
        );
        let materialized = PathObservationDemand::new(
            PathObservationNamespace::Materialization(
                slug_workspace_v2::PathObservationInstanceId::new(1),
            ),
            NormalizedAbsolutePath::new(target_a).unwrap(),
            PathObservationOperation::FileBytes,
        );
        let unrelated = PathObservationDemand::new(
            PathObservationNamespace::Host,
            NormalizedAbsolutePath::new(unrelated).unwrap(),
            PathObservationOperation::FileBytes,
        );
        let initial = runtime
            .repository_materializer
            .observe_native(
                token,
                [readlink.dupe(), materialized.dupe(), unrelated.dupe()],
            )
            .unwrap();
        let certificate = SourceCertificate::from_epoch(
            PathObservationEpoch::from_shared(
                [readlink.dupe(), materialized.dupe()]
                    .into_iter()
                    .map(|demand| (demand.dupe(), initial.get(&demand).unwrap().dupe())),
            )
            .unwrap(),
        )
        .unwrap();
        runtime.runtime.block_on(async {
            let mut updater = runtime.dice.updater();
            updater
                .changed_to(vec![(PathObservationEpochKey, initial.clone())])
                .unwrap();
            drop(
                runtime
                    .request_revision
                    .commit_native_attempt(updater)
                    .await
                    .unwrap(),
            );
        });
        let accepted_before_failure = accepted_native_snapshot(&runtime);
        runtime.runtime.block_on(async {
            let terminal = runtime.dice.updater().existing_state().await;
            let mut selected = runtime.dice.updater();
            selected
                .changed_to(vec![(PathObservationEpochKey, initial.clone())])
                .unwrap();
            assert!(matches!(
                runtime
                    .request_revision
                    .finalize_native(
                        &terminal,
                        &certificate,
                        selected,
                        &initial,
                        |_| Err(RequestRevisionError::Observation("forced".to_owned())),
                    )
                    .await,
                Err(RequestRevisionError::Observation(_))
            ));
            let mut current = runtime.dice.updater().existing_state().await;
            let retained = current.compute(&PathObservationEpochKey).await.unwrap();
            assert!(initial
                .observations()
                .iter()
                .all(|(demand, result)| Arc::ptr_eq(result, retained.get(demand).unwrap())));
        });
        let accepted_after_failure = accepted_native_snapshot(&runtime);
        assert_eq!(
            accepted_after_failure.path_observations,
            accepted_before_failure.path_observations
        );
        assert_eq!(
            accepted_after_failure.repository_results,
            accepted_before_failure.repository_results
        );
        assert_eq!(accepted_after_failure.events, accepted_before_failure.events);

        fs::remove_file(&logical).unwrap();
        std::os::unix::fs::symlink(&target_b, &logical).unwrap();
        let NativeFinalization::RetrySourceChanged { merged_epoch: changed } =
            finalize_epoch_for_test(&runtime, token, &certificate, &initial)
        else {
            panic!("symlink retarget did not advance the certificate epoch");
        };
        assert!(!Arc::ptr_eq(
            initial.get(&readlink).unwrap(),
            changed.get(&readlink).unwrap()
        ));
        assert!(Arc::ptr_eq(
            initial.get(&materialized).unwrap(),
            changed.get(&materialized).unwrap()
        ));
        assert!(Arc::ptr_eq(
            initial.get(&unrelated).unwrap(),
            changed.get(&unrelated).unwrap()
        ));

        let changed_certificate = SourceCertificate::from_epoch(
            PathObservationEpoch::from_shared(
                [readlink.dupe(), materialized.dupe()]
                    .into_iter()
                    .map(|demand| (demand.dupe(), changed.get(&demand).unwrap().dupe())),
            )
            .unwrap(),
        )
        .unwrap();
        fs::write(workspace.path().join("a"), b"changed").unwrap();
        fs::remove_file(&logical).unwrap();
        let NativeFinalization::RetrySourceChanged {
            merged_epoch: missing,
        } = finalize_epoch_for_test(&runtime, token, &changed_certificate, &changed)
        else {
            panic!("missing symlink did not advance the certificate epoch");
        };
        assert!(matches!(
            missing.get(&readlink).unwrap().as_ref(),
            PathObservationResult::ReadLink(PathOperationResult::Missing)
        ));
        assert!(!Arc::ptr_eq(
            changed.get(&materialized).unwrap(),
            missing.get(&materialized).unwrap()
        ));

        fs::create_dir(&logical).unwrap();
        let missing_certificate = SourceCertificate::from_epoch(
            PathObservationEpoch::from_shared(
                [readlink.dupe(), materialized.dupe()]
                    .into_iter()
                    .map(|demand| (demand.dupe(), missing.get(&demand).unwrap().dupe())),
            )
            .unwrap(),
        )
        .unwrap();
        let NativeFinalization::RetrySourceChanged {
            merged_epoch: directory,
        } = finalize_epoch_for_test(&runtime, token, &missing_certificate, &missing)
        else {
            panic!("directory replacement did not advance the certificate epoch");
        };
        assert!(matches!(
            directory.get(&readlink).unwrap().as_ref(),
            PathObservationResult::ReadLink(PathOperationResult::Error(_))
        ));

        fs::remove_dir(&logical).unwrap();
        fs::write(workspace.path().join("a"), b"same").unwrap();
        std::os::unix::fs::symlink(workspace.path().join("a"), &logical).unwrap();
        let directory_certificate = SourceCertificate::from_epoch(
            PathObservationEpoch::from_shared(
                [readlink.dupe(), materialized.dupe()]
                    .into_iter()
                    .map(|demand| (demand.dupe(), directory.get(&demand).unwrap().dupe())),
            )
            .unwrap(),
        )
        .unwrap();
        let NativeFinalization::RetrySourceChanged {
            merged_epoch: restored,
        } = finalize_epoch_for_test(&runtime, token, &directory_certificate, &directory)
        else {
            panic!("restored symlink did not advance the certificate epoch");
        };
        assert_eq!(restored.get(&readlink), initial.get(&readlink));
        assert_eq!(
            restored.get(&materialized).unwrap(),
            initial.get(&materialized).unwrap()
        );
        assert!(!Arc::ptr_eq(
            restored.get(&materialized).unwrap(),
            initial.get(&materialized).unwrap()
        ));
        runtime.repository_materializer.discard(token).unwrap();
    }

    #[test]
    fn build_command_root_identity_is_canonical_ordered_and_preflighted() {
        let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
        let configuration = build_test_configuration("target");
        let shorthand = BuildCommandRootKey::new(
            workspace.clone(),
            &[TargetPattern::parse("//pkg").unwrap()],
            configuration.clone(),
        )
        .unwrap();
        let explicit = BuildCommandRootKey::new(
            workspace.clone(),
            &[TargetPattern::parse("//pkg:pkg").unwrap()],
            configuration.clone(),
        )
        .unwrap();
        assert_eq!(shorthand, explicit);
        assert_eq!(shorthand.targets.as_ref(), [Arc::<str>::from("//pkg:pkg")]);

        let duplicate = BuildCommandRootKey::new(
            workspace.clone(),
            &[
                TargetPattern::parse("//pkg:pkg").unwrap(),
                TargetPattern::parse("//pkg:pkg").unwrap(),
            ],
            configuration.clone(),
        )
        .unwrap();
        let reversed = BuildCommandRootKey::new(
            workspace.clone(),
            &[
                TargetPattern::parse("//other:t").unwrap(),
                TargetPattern::parse("//pkg:pkg").unwrap(),
            ],
            configuration.clone(),
        )
        .unwrap();
        assert_ne!(duplicate, explicit);
        assert_ne!(reversed, explicit);
        assert_ne!(
            explicit,
            BuildCommandRootKey::new(
                workspace.clone(),
                &[TargetPattern::parse("//pkg:pkg").unwrap()],
                build_test_configuration_with_root_setting("other"),
            )
            .unwrap()
        );
        assert_ne!(
            explicit,
            BuildCommandRootKey::new(
                NormalizedAbsolutePath::new("/other").unwrap(),
                &[TargetPattern::parse("//pkg:pkg").unwrap()],
                configuration.clone(),
            )
            .unwrap()
        );
        assert!(
            BuildCommandRootKey::new(
                workspace.clone(),
                &[TargetPattern::parse("@repo//pkg:t").unwrap()],
                configuration.clone(),
            )
            .is_ok()
        );
        for targets in [
            vec![TargetPattern::parse("@repo//pkg:all").unwrap()],
            vec![TargetPattern::parse("@repo//pkg/...").unwrap()],
            vec![
                TargetPattern::parse("//pkg:t").unwrap(),
                TargetPattern::parse("@repo//pkg:t").unwrap(),
            ],
            vec![
                TargetPattern::parse("@repo//pkg:one").unwrap(),
                TargetPattern::parse("@repo//pkg:two").unwrap(),
            ],
        ] {
            assert!(matches!(
                BuildCommandRootKey::new(workspace.clone(), &targets, configuration.clone()),
                Err(BuildCommandRequestError::ExternalRepository { .. })
            ));
        }
        assert!(matches!(
            BuildCommandRootKey::new(
                workspace,
                &[TargetPattern::parse("//pkg/...").unwrap()],
                configuration,
            ),
            Err(BuildCommandRequestError::RecursivePattern { pattern })
                if pattern.as_ref() == "//pkg/..."
        ));
    }

    #[tokio::test]
    async fn build_command_root_anchors_empty_and_preserves_ordered_package_results() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(LegacyBuildTracker::default());
        let user_data = UserComputationData {
            activation_tracker: Some(tracker.clone() as Arc<dyn ActivationTracker>),
            ..Default::default()
        };
        let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
        let configuration = build_test_configuration("target");
        let empty_key =
            BuildCommandRootKey::new(workspace.clone(), &[], configuration.clone()).unwrap();
        let mut empty =
            build_root_transaction_with_data(&dice, BuildRootEpoch::base(1).build(), user_data)
                .await;
        let empty_outcome = compute_build_root(&mut empty, &empty_key, &tracker).await;
        let slug_bzlmod_v2::SourcePreparationOutcome::Complete(empty_value) = &empty_outcome else {
            panic!("complete root anchor returned Need");
        };
        assert!(empty_value.as_ref().as_ref().unwrap().targets.is_empty());
        assert!(BuildCommandRootKey::validity(&empty_outcome));
        assert!(BuildCommandRootKey::equality(
            &empty_outcome,
            &empty_outcome
        ));

        let targets = [
            TargetPattern::parse("//second:all").unwrap(),
            TargetPattern::parse("//first:t").unwrap(),
            TargetPattern::parse("//first:t").unwrap(),
        ];
        let key = BuildCommandRootKey::new(workspace, &targets, configuration).unwrap();
        let mut epoch = BuildRootEpoch::base(2);
        epoch.package("first", "filegroup(name = \"t\")\n", 2);
        epoch.package("second", "filegroup(name = \"other\")\n", 2);
        let user_data = UserComputationData {
            activation_tracker: Some(tracker.clone() as Arc<dyn ActivationTracker>),
            ..Default::default()
        };
        let mut transaction =
            build_root_transaction_with_data(&dice, epoch.build(), user_data).await;
        let value = compute_build_root(&mut transaction, &key, &tracker).await;
        let slug_bzlmod_v2::SourcePreparationOutcome::Complete(value) = value else {
            panic!("complete package bundle returned Need");
        };
        let targets = &value.as_ref().as_ref().unwrap().targets;
        assert_eq!(
            targets
                .iter()
                .map(|target| target.pattern.as_ref())
                .collect::<Vec<_>>(),
            ["//second:all", "//first:t", "//first:t"]
        );
        assert!(targets.iter().all(|target| target.analysis.is_none()));
    }

    #[tokio::test]
    async fn build_action_closure_traverses_alias_and_generated_nodes_but_excludes_null_nodes() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let key = BuildCommandRootKey::new(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            &[TargetPattern::parse("//:root").unwrap()],
            build_test_configuration("target"),
        )
        .unwrap();
        let mut transaction =
            build_root_transaction(&dice, delegating_action_closure_epoch(1)).await;
        let outcome = transaction.compute(&key).await.unwrap();
        let evaluation = complete_build_evaluation(&outcome);

        assert_eq!(evaluation.analyzed_target_count(), 1);
        assert_eq!(evaluation.declared_action_count(), 1);
        assert_eq!(
            evaluation
                .analyses()
                .map(|analysis| analysis.key().label().to_string())
                .collect::<Vec<_>>(),
            [
                "@@//:root",
                "@@//:alias_outer",
                "@@//:producer.out",
                "@@//:alias_inner",
                "@@//:producer",
            ]
        );
        assert!(
            evaluation
                .analyses()
                .all(|analysis| analysis.configured_target_key().is_some())
        );
        let root = evaluation.analyses().next().unwrap();
        assert_eq!(root.edges().len(), 4);
        assert_eq!(
            root.edges()
                .iter()
                .filter(|edge| edge.target().configured_target().is_none())
                .count(),
            2,
            "source and declaring-visibility null nodes stay outside the build action closure"
        );
        let producer = evaluation.analyses().last().unwrap();
        assert_eq!(producer.actions().len(), 1);
        assert_eq!(producer.actions()[0].outputs()[0].path(), "producer.out");
    }

    #[tokio::test]
    async fn build_action_closure_is_roots_first_breadth_first_and_deduplicates_diamonds() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let key = BuildCommandRootKey::new(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            &[
                TargetPattern::parse("//top:top").unwrap(),
                TargetPattern::parse("//left:left").unwrap(),
                TargetPattern::parse("//top:top").unwrap(),
            ],
            build_test_configuration("target"),
        )
        .unwrap();
        let tracker = Arc::new(ActionClosureTracker::default());
        let mut user_data = UserComputationData {
            activation_tracker: Some(tracker.dupe()),
            ..Default::default()
        };
        user_data.data.set(CaptureEvaluationEvents);
        let mut transaction = build_root_transaction_with_data(
            &dice,
            action_closure_epoch(1, "shared-a", true, true),
            user_data,
        )
        .await;
        let outcome = transaction.compute(&key).await.unwrap();
        let evaluation = complete_build_evaluation(&outcome);
        assert_eq!(evaluation.analyzed_target_count(), 3);
        assert_eq!(evaluation.declared_action_count(), 4);
        assert_eq!(
            evaluation
                .analyses()
                .map(|analysis| analysis.key().label().to_string())
                .collect::<Vec<_>>(),
            [
                "@@//top:top",
                "@@//left:left",
                "@@//right:right",
                "@@//shared:shared",
            ]
        );
        assert_eq!(
            evaluation
                .analyses()
                .map(|analysis| analysis.actions()[0].outputs()[0].path())
                .collect::<Vec<_>>(),
            [
                "top/top.txt",
                "left/left.txt",
                "right/right.txt",
                "shared/shared.txt",
            ]
        );
        assert!(Arc::ptr_eq(
            evaluation.targets[0].analysis.as_ref().unwrap(),
            &evaluation.action_closure[0],
        ));
        assert!(Arc::ptr_eq(
            evaluation.targets[1].analysis.as_ref().unwrap(),
            &evaluation.action_closure[1],
        ));
        assert!(Arc::ptr_eq(
            evaluation.targets[0].analysis.as_ref().unwrap(),
            evaluation.targets[2].analysis.as_ref().unwrap(),
        ));
        let activations = tracker.take();
        let mut evaluated = activations
            .iter()
            .filter(|(_, kind, _)| *kind == dice::ActivationKind::Evaluated)
            .map(|(label, _, batch)| {
                assert_eq!(
                    batch.as_ref().map(|batch| batch.events().len()),
                    Some(1),
                    "target-local event batch for {label}"
                );
                label.as_str()
            })
            .collect::<Vec<_>>();
        evaluated.sort();
        assert_eq!(
            evaluated,
            [
                "@@//left:left",
                "@@//right:right",
                "@@//shared:shared",
                "@@//top:top",
            ]
        );

        let mut warm_data = UserComputationData {
            activation_tracker: Some(tracker.dupe()),
            ..Default::default()
        };
        warm_data.data.set(CaptureEvaluationEvents);
        let mut warm_transaction = build_root_transaction_with_data(
            &dice,
            action_closure_epoch(1, "shared-a", true, true),
            warm_data,
        )
        .await;
        let warm = warm_transaction.compute(&key).await.unwrap();
        assert!(BuildCommandRootKey::equality(&outcome, &warm));
        assert!(tracker.take().is_empty());
    }

    #[tokio::test]
    async fn resolved_run_view_reuses_exact_executable_filewrite_relation() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let key = BuildCommandRootKey::new(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            &[TargetPattern::parse("//:write").unwrap()],
            build_test_configuration("target"),
        )
        .unwrap();
        let mut rejected_tx =
            build_root_transaction(&dice, resolved_write_epoch(40, "setting_a", &[])).await;
        let rejected = rejected_tx.compute(&key).await.unwrap();
        assert_eq!(
            complete_build_evaluation(&rejected)
                .resolved_run_semantic_view()
                .unwrap_err(),
            "run requires an executable non-test rule"
        );

        let replacements = [
            (
                "ctx.actions.write(out, \"content\\n\")",
                "ctx.actions.write(out, \"content\\n\", is_executable = True)",
            ),
            (
                "return [DefaultInfo(files = depset([out]))]",
                "return [DefaultInfo(executable = out)]",
            ),
            (
                "write = rule(implementation = _write, toolchains",
                "write = rule(implementation = _write, executable = True, toolchains",
            ),
        ];
        let mut accepted_tx =
            build_root_transaction(&dice, resolved_write_epoch(41, "setting_a", &replacements))
                .await;
        let accepted = accepted_tx.compute(&key).await.unwrap();
        let view = complete_build_evaluation(&accepted)
            .resolved_run_semantic_view()
            .unwrap();
        assert_eq!(view.executable(), "write.txt");
        assert_eq!(view.file_write().action().output().path(), "write.txt");
        assert!(std::ptr::eq(
            view.owner().providers().default_info().unwrap(),
            view.default_info(),
        ));

        let fail_closed_cases = [
            (
                42,
                vec![
                    (
                        "def _write(ctx):",
                        "Extra = provider(fields = {\"value\": \"value\"})\ndef _write(ctx):",
                    ),
                    (
                        "ctx.actions.write(out, \"content\\n\")",
                        "ctx.actions.write(out, \"content\\n\", is_executable = True)",
                    ),
                    (
                        "return [DefaultInfo(files = depset([out]))]",
                        "return [DefaultInfo(executable = out), Extra(value = \"extra\")]",
                    ),
                    (
                        "write = rule(implementation = _write, toolchains",
                        "write = rule(implementation = _write, executable = True, toolchains",
                    ),
                ],
                "run target requires only built-in DefaultInfo",
            ),
            (
                43,
                vec![
                    (
                        "ctx.actions.write(out, \"content\\n\")",
                        "ctx.actions.write(out, \"content\\n\", is_executable = True)",
                    ),
                    (
                        "return [DefaultInfo(files = depset([out]))]",
                        "return [DefaultInfo(executable = out, files = depset([]))]",
                    ),
                    (
                        "write = rule(implementation = _write, toolchains",
                        "write = rule(implementation = _write, executable = True, toolchains",
                    ),
                ],
                "run files and runfiles must contain only the executable",
            ),
            (
                44,
                vec![
                    (
                        "return [DefaultInfo(files = depset([out]))]",
                        "return [DefaultInfo(executable = out)]",
                    ),
                    (
                        "write = rule(implementation = _write, toolchains",
                        "write = rule(implementation = _write, executable = True, toolchains",
                    ),
                ],
                "run executable is not the sole executable FileWrite output",
            ),
        ];
        for (variant, replacements, expected) in fail_closed_cases {
            let mut transaction = build_root_transaction(
                &dice,
                resolved_write_epoch(variant, "setting_a", &replacements),
            )
            .await;
            let outcome = transaction.compute(&key).await.unwrap();
            assert_eq!(
                complete_build_evaluation(&outcome)
                    .resolved_run_semantic_view()
                    .unwrap_err(),
                expected
            );
        }
    }

    #[tokio::test]
    async fn resolved_file_write_view_borrows_exact_platform_fact_and_rejects_bad_closures() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
        let targets = [TargetPattern::parse("//:write").unwrap()];
        let key = BuildCommandRootKey::new(workspace, &targets, build_test_configuration("target"))
            .unwrap();
        let mut transaction =
            build_root_transaction(&dice, resolved_write_epoch(3, "setting_a", &[])).await;
        let outcome = transaction.compute(&key).await.unwrap();
        let base = complete_build_evaluation(&outcome);
        let platform = base
            .action_closure
            .iter()
            .find(|node| node.kind() == &ConfiguredNodeKind::Platform)
            .unwrap()
            .dupe();
        let evaluation = |closure: Vec<Arc<ConfiguredNodeResult>>| BuildCommandEvaluation {
            anchor: base.anchor.clone(),
            targets: base.targets.clone(),
            action_closure: closure.into(),
        };
        let views = base.resolved_file_write_semantic_views().unwrap();
        let constraint = views[0].platform_constraints()[0];
        let baseline_projection = resolved_setting_label(base);
        assert_eq!(baseline_projection, "@@//:setting_a");
        let platform_key = platform.configured_target_key().unwrap().clone();
        let wrong = Arc::new(ConfiguredNodeResult::new_rule(
            platform_key,
            platform.providers().clone(),
            None,
        ));
        let unordered = Arc::new(platform.as_ref().clone().with_edges(vec![
            slug_analysis_v2::ConfiguredEdge::new(
                platform.edges()[0].target().clone(),
                ConfiguredEdgeKind::PlatformConstraint { index: 1 },
            ),
        ]));
        let bad_value = Arc::new(ConfiguredNodeResult::new_rule(
            constraint
                .constraint_value()
                .configured_target_key()
                .unwrap()
                .clone(),
            constraint.constraint_value().providers().clone(),
            None,
        ));
        let missing_setting =
            Arc::new(constraint.constraint_value().clone().with_edges(Vec::new()));
        let mismatched = Arc::new(platform.as_ref().clone().with_edges(vec![
            slug_analysis_v2::ConfiguredEdge::new(
                ConfiguredTargetKey::new(
                    constraint.constraint_value().key().label().clone(),
                    ConfigurationKey::exec("legacy-mismatch").unwrap(),
                )
                .into(),
                ConfiguredEdgeKind::PlatformConstraint { index: 0 },
            ),
        ]));
        let duplicate_setting = Arc::new(platform.as_ref().clone().with_edges(vec![
            platform.edges()[0].clone(),
            slug_analysis_v2::ConfiguredEdge::new(
                platform.edges()[0].target().clone(),
                ConfiguredEdgeKind::PlatformConstraint { index: 1 },
            ),
        ]));
        let replace = |replacement: Arc<ConfiguredNodeResult>| {
            let key = replacement.configured_target_key().unwrap();
            base.action_closure
                .iter()
                .map(|node| {
                    if node.configured_target_key() == Some(key) {
                        replacement.dupe()
                    } else {
                        node.dupe()
                    }
                })
                .collect()
        };
        for (closure, message) in [
            (
                base.action_closure
                    .iter()
                    .filter(|node| node.kind() != &ConfiguredNodeKind::Platform)
                    .cloned()
                    .collect(),
                "absent",
            ),
            (
                base.action_closure
                    .iter()
                    .cloned()
                    .chain([platform.dupe()])
                    .collect(),
                "duplicated",
            ),
            (replace(wrong), "wrong kind"),
            (replace(unordered), "unordered"),
            (replace(bad_value), "wrong kind"),
            (replace(missing_setting), "exactly one setting edge"),
            (replace(mismatched), "mismatched configuration"),
            (replace(duplicate_setting), "duplicate constraint setting"),
        ] {
            let error = evaluation(closure)
                .resolved_file_write_semantic_views()
                .unwrap_err();
            assert!(error.contains(message));
        }
        let mut edited_tx =
            build_root_transaction(&dice, resolved_write_epoch(4, "setting_b", &[])).await;
        let edited = edited_tx.compute(&key).await.unwrap();
        let edited_projection = resolved_setting_label(complete_build_evaluation(&edited));
        assert_eq!(edited_projection, "@@//:setting_b");
        assert_ne!(edited_projection, baseline_projection);
        let mut restored_tx =
            build_root_transaction(&dice, resolved_write_epoch(5, "setting_a", &[])).await;
        let restored = restored_tx.compute(&key).await.unwrap();
        let restored_projection = resolved_setting_label(complete_build_evaluation(&restored));
        assert_eq!(restored_projection, "@@//:setting_a");
        assert_eq!(restored_projection, baseline_projection);
    }

    #[tokio::test]
    async fn filewrite_aquery_text_matches_frozen_baseline() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let key = BuildCommandRootKey::new(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            &[TargetPattern::parse("//:write").unwrap()],
            build_test_configuration("target"),
        )
        .unwrap();
        let mut transaction =
            build_root_transaction(&dice, resolved_write_epoch(19, "setting_a", &[])).await;
        let outcome = transaction.compute(&key).await.unwrap();
        let evaluation = complete_build_evaluation(&outcome);
        let text = crate::runtime::format_file_write_aquery_text_output(evaluation).unwrap();
        assert_eq!(
            text,
            concat!(
                "action 'Writing file write.txt'\n",
                "  Mnemonic: FileWrite\n",
                "  Target: //:write\n",
                "  Configuration: slugcfg-v1:abc6de66486cc9eff604c3e0795796631112a6d92cf3336370de8e8f6acf953a\n",
                "  Execution platform: //:platform\n",
                "  SlugActionToken: slugact-display-v1:9107d642a3e8b06ebfbe865544a76344a8cdf2078f75ba39e01e6dca5125f361\n",
                "  Inputs: []\n",
                "  Outputs: [bazel-out/slugcfg-v1-abc6de66486cc9eff604c3e0795796631112a6d92cf3336370de8e8f6acf953a/bin/write.txt]\n",
                "  IsExecutable: false\n\n",
            )
        );

        let missing = BuildCommandEvaluation {
            anchor: evaluation.anchor.clone(),
            targets: Vec::new().into(),
            action_closure: evaluation.action_closure.clone(),
        };
        assert_eq!(
            crate::runtime::format_file_write_aquery_text_output(&missing),
            Err("FileWrite aquery requires exactly one requested analysis")
        );
        let duplicated = BuildCommandEvaluation {
            anchor: evaluation.anchor.clone(),
            targets: vec![evaluation.targets[0].clone(), evaluation.targets[0].clone()].into(),
            action_closure: evaluation.action_closure.clone(),
        };
        assert_eq!(
            crate::runtime::format_file_write_aquery_text_output(&duplicated),
            Err("FileWrite aquery requires exactly one requested analysis")
        );
    }

    #[tokio::test]
    async fn filewrite_aquery_text_keeps_root_order_and_excludes_dependency_actions() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
        let key = |target| {
            BuildCommandRootKey::new(
                workspace.clone(),
                &[TargetPattern::parse(target).unwrap()],
                build_test_configuration("target"),
            )
            .unwrap()
        };
        let mut transaction =
            build_root_transaction(&dice, resolved_write_epoch(20, "setting_a", &[])).await;
        let outcome = transaction.compute(&key("//:ordered")).await.unwrap();
        let evaluation = complete_build_evaluation(&outcome);
        assert_eq!(evaluation.declared_action_count(), 3);
        let text = crate::runtime::format_file_write_aquery_text_output(evaluation).unwrap();
        assert_eq!(text.matches("action '").count(), 2);
        assert!(text.find("z-root.txt").unwrap() < text.find("a-root.txt").unwrap());
        assert!(!text.contains("write.txt"));
        assert_eq!(text.matches("\n\n").count(), 2);
        assert!(text.ends_with("  IsExecutable: false\n\n"));

        let deps_text = crate::runtime::format_file_write_aquery_text_output_for_scope(
            evaluation,
            slug_query_v2::AqueryScope::Deps,
        )
        .unwrap();
        assert_eq!(deps_text.matches("action '").count(), 3);
        assert!(deps_text.contains("write.txt"));
        assert_eq!(deps_text.matches("\n\n").count(), 3);

        let mut empty_transaction =
            build_root_transaction(&dice, resolved_write_epoch(21, "setting_a", &[])).await;
        let empty = empty_transaction.compute(&key("//:empty")).await.unwrap();
        assert_eq!(
            crate::runtime::format_file_write_aquery_text_output(complete_build_evaluation(&empty)),
            Err("FileWrite aquery text requires at least one resolved action")
        );
    }

    #[tokio::test]
    async fn filewrite_semantic_identity_discriminates_admitted_structure_and_restores() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
        let configuration = build_test_configuration("target");
        let key = |target: &str, configuration: ConfigurationKey| {
            BuildCommandRootKey::new(
                workspace.clone(),
                &[TargetPattern::parse(target).unwrap()],
                configuration,
            )
            .unwrap()
        };
        let baseline_key = key("//:write", configuration.clone());
        let baseline = resolved_identity(
            &dice,
            &baseline_key,
            resolved_write_epoch(20, "setting_a", &[]),
        )
        .await;
        assert!(baseline.as_bytes().starts_with(b"slugact\0\0\x01"));
        let baseline_token = baseline.aquery_display_token();
        assert!(baseline_token.starts_with("slugact-display-v1:"));
        assert_eq!(baseline_token.len(), 83);
        let owner = resolved_identity(
            &dice,
            &key("//:other", configuration.clone()),
            resolved_write_epoch(21, "setting_a", &[]),
        )
        .await;
        let configured = resolved_identity(
            &dice,
            &key(
                "//:write",
                build_test_configuration_with_root_setting("changed"),
            ),
            resolved_write_epoch(22, "setting_a", &[]),
        )
        .await;
        let output = resolved_identity(
            &dice,
            &baseline_key,
            resolved_write_epoch(23, "setting_a", &[("write.txt", "changed.txt")]),
        )
        .await;
        let content = resolved_identity(
            &dice,
            &baseline_key,
            resolved_write_epoch(24, "setting_a", &[(r#""content\n""#, r#""changed\n""#)]),
        )
        .await;
        let property = resolved_identity(
            &dice,
            &baseline_key,
            resolved_write_epoch(
                25,
                "setting_a",
                &[(
                    r#"{"z": "last", "a": "first"}"#,
                    r#"{"z": "changed", "a": "first"}"#,
                )],
            ),
        )
        .await;
        for changed in [owner, configured, output, content, property] {
            assert_ne!(changed.aquery_display_token(), baseline_token);
            assert_ne!(changed, baseline);
        }
        let reordered = resolved_identity(
            &dice,
            &baseline_key,
            resolved_write_epoch(
                26,
                "setting_a",
                &[(
                    r#"{"z": "last", "a": "first"}"#,
                    r#"{"a": "first", "z": "last"}"#,
                )],
            ),
        )
        .await;
        assert_eq!(reordered, baseline);
        assert_eq!(reordered.aquery_display_token(), baseline_token);
        let platform = resolved_identity(
            &dice,
            &baseline_key,
            resolved_write_epoch(27, "setting_a", &[("//:platform", "//:platform_alt")]),
        )
        .await;
        assert_ne!(platform, baseline);
        assert_ne!(platform.aquery_display_token(), baseline_token);
        let platform_restored = resolved_identity(
            &dice,
            &baseline_key,
            resolved_write_epoch(28, "setting_a", &[]),
        )
        .await;
        assert_eq!(platform_restored, baseline);
        assert_eq!(platform_restored.aquery_display_token(), baseline_token);
        let constraint = resolved_identity(
            &dice,
            &baseline_key,
            resolved_write_epoch(29, "setting_b", &[]),
        )
        .await;
        assert_ne!(constraint, baseline);
        assert_ne!(constraint.aquery_display_token(), baseline_token);
        let restored = resolved_identity(
            &dice,
            &baseline_key,
            resolved_write_epoch(30, "setting_a", &[]),
        )
        .await;
        assert_eq!(restored, baseline);
        assert_eq!(restored.aquery_display_token(), baseline_token);
    }

    #[tokio::test]
    async fn build_action_closure_retains_accepted_parent_second_first_actions() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut epoch = BuildRootEpoch::base(5);
        epoch.package("rules", "", 5);
        epoch.file("/workspace/rules/defs.bzl", ACTION_CLOSURE_DEFS, 5);
        epoch.package(
            "leaf",
            "load(\"//rules:defs.bzl\", \"node\")\nnode(name = \"first\", marker = \"first\")\nnode(name = \"second\", marker = \"second\")\n",
            5,
        );
        epoch.package(
            "parent",
            "load(\"//rules:defs.bzl\", \"node\")\nnode(name = \"parent\", deps = [\"//leaf:second\", \"//leaf:first\"], marker = \"parent\")\n",
            5,
        );
        let key = BuildCommandRootKey::new(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            &[TargetPattern::parse("//parent:parent").unwrap()],
            build_test_configuration("target"),
        )
        .unwrap();
        let mut transaction = build_root_transaction(&dice, epoch.build()).await;
        let outcome = transaction.compute(&key).await.unwrap();
        let evaluation = complete_build_evaluation(&outcome);
        assert_eq!(evaluation.analyzed_target_count(), 1);
        assert_eq!(evaluation.declared_action_count(), 3);
        assert_eq!(
            evaluation
                .analyses()
                .map(|analysis| analysis.key().label().to_string())
                .collect::<Vec<_>>(),
            ["@@//parent:parent", "@@//leaf:second", "@@//leaf:first"]
        );
    }

    #[tokio::test]
    async fn build_action_frontier_need_precedes_an_earlier_sibling_analysis_error() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let configuration = build_test_configuration("target");
        let error_target = ConfiguredTargetKey::new(
            CanonicalLabel::parse("@@//error:missing").unwrap(),
            configuration.clone(),
        );
        let mut epoch = BuildRootEpoch::base(6);
        epoch.package("error", "", 6);
        let mut transaction = build_root_transaction(&dice, epoch.build()).await;
        let error = transaction
            .compute(
                &ConfiguredNodeAnalysisKey::new(
                    NormalizedAbsolutePath::new("/workspace").unwrap(),
                    error_target.clone(),
                )
                .unwrap(),
            )
            .await
            .unwrap();
        assert!(matches!(
            error,
            slug_bzlmod_v2::SourcePreparationOutcome::Complete(ref value)
                if value.as_ref().is_err()
        ));

        let need_target = ConfiguredTargetKey::new(
            CanonicalLabel::parse("@@//need:child").unwrap(),
            configuration,
        );
        let reduced = collect_build_action_frontier(vec![
            (error_target, error),
            (
                need_target,
                slug_bzlmod_v2::SourcePreparationOutcome::Need(build_test_need("/workspace/need")),
            ),
        ])
        .unwrap();
        let slug_bzlmod_v2::SourcePreparationOutcome::Need(needs) = reduced else {
            panic!("same-frontier analysis error won over sibling Need")
        };
        assert_eq!(
            needs.path_observations().unwrap().demands()[0]
                .path()
                .as_path(),
            Path::new("/workspace/need")
        );
    }

    #[tokio::test]
    async fn build_action_closure_tracks_child_actions_prunes_orphans_and_restores_equality() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let key = BuildCommandRootKey::new(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            &[TargetPattern::parse("//top:top").unwrap()],
            build_test_configuration("target"),
        )
        .unwrap();
        let mut first_transaction =
            build_root_transaction(&dice, action_closure_epoch(10, "shared-a", true, true)).await;
        let first = first_transaction.compute(&key).await.unwrap();
        let first_evaluation = complete_build_evaluation(&first);
        let first_parent = first_evaluation.action_closure[0].dupe();
        let first_shared = first_evaluation.action_closure[3].dupe();

        let mut warm_transaction =
            build_root_transaction(&dice, action_closure_epoch(10, "shared-a", true, true)).await;
        let warm = warm_transaction.compute(&key).await.unwrap();
        assert!(BuildCommandRootKey::equality(&first, &warm));

        let mut edited_transaction =
            build_root_transaction(&dice, action_closure_epoch(11, "shared-b", true, true)).await;
        let edited = edited_transaction.compute(&key).await.unwrap();
        let edited_evaluation = complete_build_evaluation(&edited);
        assert!(!BuildCommandRootKey::equality(&first, &edited));
        assert_eq!(
            first_parent.as_ref(),
            edited_evaluation.action_closure[0].as_ref()
        );
        assert_ne!(
            first_shared.as_ref(),
            edited_evaluation.action_closure[3].as_ref()
        );

        let mut orphaned_transaction =
            build_root_transaction(&dice, action_closure_epoch(12, "shared-b", false, true)).await;
        let orphaned = orphaned_transaction.compute(&key).await.unwrap();
        assert_eq!(
            complete_build_evaluation(&orphaned).declared_action_count(),
            3
        );
        let mut pruned_transaction =
            build_root_transaction(&dice, action_closure_epoch(12, "shared-c", false, true)).await;
        let pruned = pruned_transaction.compute(&key).await.unwrap();
        assert!(BuildCommandRootKey::equality(&orphaned, &pruned));

        let mut deleted_transaction =
            build_root_transaction(&dice, action_closure_epoch(13, "shared-b", true, false)).await;
        let deleted = deleted_transaction.compute(&key).await.unwrap();
        assert!(matches!(
            deleted,
            slug_bzlmod_v2::SourcePreparationOutcome::Complete(ref value)
                if value.as_ref().is_err()
        ));

        let mut restored_transaction =
            build_root_transaction(&dice, action_closure_epoch(14, "shared-a", true, true)).await;
        let restored = restored_transaction.compute(&key).await.unwrap();
        assert!(BuildCommandRootKey::equality(&first, &restored));
    }

    #[tokio::test]
    async fn build_command_root_selects_each_terminal_producer_once_for_duplicate_targets() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
        let effects = CommandEffectOwner::new();
        let attempt = effects.begin_attempt().unwrap();
        let demands = WorkspaceDemandOwner::new(&dice, workspace.clone());
        let mut user_data = UserComputationData::default();
        demands
            .install(&dice, &mut user_data, Some(attempt.clone()))
            .unwrap();

        let definitions = r#"
print("BZL")
def _impl(ctx):
    print("ANALYSIS")
    return [DefaultInfo(files = depset([]))]
probe = rule(implementation = _impl)
"#;
        let mut epoch = BuildRootEpoch::base(3);
        epoch.file("/workspace/MODULE.bazel", "print(\"MODULE\")\n", 3);
        epoch.package("rules", "", 3);
        epoch.file("/workspace/rules/defs.bzl", definitions, 3);
        epoch.package(
            "app",
            "print(\"BUILD\")\nload(\"//rules:defs.bzl\", \"probe\")\nprobe(name = \"t\")\n",
            3,
        );
        let targets = [
            TargetPattern::parse("//app:t").unwrap(),
            TargetPattern::parse("//app:t").unwrap(),
        ];
        let key = BuildCommandRootKey::new(workspace, &targets, build_test_configuration("target"))
            .unwrap();
        let mut transaction =
            build_root_transaction_with_data(&dice, epoch.build(), user_data).await;
        let value = transaction.compute(&key).await.unwrap();
        assert!(matches!(
            value,
            slug_bzlmod_v2::SourcePreparationOutcome::Complete(ref value)
                if value.as_ref().as_ref().unwrap().targets.len() == 2
        ));
        let sealed = attempt.seal_terminal().unwrap();
        assert_eq!(sealed.root_count(), 1);
        let sidecars = sealed.select(&transaction).await.unwrap();
        let texts = sidecars
            .events()
            .batches()
            .iter()
            .flat_map(EventBatch::events)
            .map(|event| match event {
                EvaluationEvent::StarlarkPrint { text, .. } => text.as_str(),
                EvaluationEvent::Diagnostic { .. } => "<diagnostic>",
            })
            .collect::<Vec<_>>();
        assert_eq!(texts, ["MODULE", "BZL", "BUILD", "ANALYSIS"]);
    }

    #[tokio::test]
    async fn build_command_root_terminal_closure_retains_reused_and_clears_retry_only_batches() {
        for (retain_prints, expected) in [
            (true, vec!["MODULE", "BZL", "BUILD", "ANALYSIS"]),
            (false, Vec::new()),
        ] {
            let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
            let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
            let effects = CommandEffectOwner::new();
            let demands = WorkspaceDemandOwner::new(&dice, workspace.clone());
            let key = BuildCommandRootKey::new(
                workspace,
                &[
                    TargetPattern::parse("//app:t").unwrap(),
                    TargetPattern::parse("//later:all").unwrap(),
                ],
                build_test_configuration("target"),
            )
            .unwrap();
            let epoch = |variant: i64, prints: bool, later: bool| {
                let mut epoch = BuildRootEpoch::base(variant);
                epoch.file(
                    "/workspace/MODULE.bazel",
                    if prints { "print(\"MODULE\")\n" } else { "" },
                    variant,
                );
                epoch.package("rules", "", variant);
                let definition_name = if prints { "old.bzl" } else { "new.bzl" };
                epoch.file(
                    &format!("/workspace/rules/{definition_name}"),
                    if prints {
                        "print(\"BZL\")\ndef _impl(ctx):\n    print(\"ANALYSIS\")\n    return [DefaultInfo(files = depset([]))]\nprobe = rule(implementation = _impl)\n"
                    } else {
                        "def _impl(ctx):\n    return [DefaultInfo(files = depset([]))]\nprobe = rule(implementation = _impl)\n"
                    },
                    variant,
                );
                epoch.package(
                    "app",
                    if prints {
                        "print(\"BUILD\")\nload(\"//rules:old.bzl\", \"probe\")\nprobe(name = \"t\")\n"
                    } else {
                        "load(\"//rules:new.bzl\", \"probe\")\nprobe(name = \"t\")\n"
                    },
                    variant,
                );
                if later {
                    epoch.package("later", "filegroup(name = \"t\")\n", variant);
                }
                epoch.build()
            };

            let retry = effects.begin_attempt().unwrap();
            let mut retry_data = UserComputationData::default();
            demands
                .install(&dice, &mut retry_data, Some(retry.clone()))
                .unwrap();
            let mut retry_transaction =
                build_root_transaction_with_data(&dice, epoch(60, true, false), retry_data).await;
            assert!(matches!(
                retry_transaction.compute(&key).await.unwrap(),
                slug_bzlmod_v2::SourcePreparationOutcome::Need(_)
            ));
            retry.seal_retry().unwrap();

            let terminal = effects.begin_attempt().unwrap();
            let mut terminal_data = UserComputationData::default();
            demands
                .install(&dice, &mut terminal_data, Some(terminal.clone()))
                .unwrap();
            let terminal_variant = if retain_prints { 60 } else { 61 };
            let mut terminal_transaction = build_root_transaction_with_data(
                &dice,
                epoch(terminal_variant, retain_prints, true),
                terminal_data,
            )
            .await;
            assert!(matches!(
                terminal_transaction.compute(&key).await.unwrap(),
                slug_bzlmod_v2::SourcePreparationOutcome::Complete(ref value)
                    if value.as_ref().is_ok()
            ));
            let selected = terminal
                .seal_terminal()
                .unwrap()
                .select(&terminal_transaction)
                .await
                .unwrap();
            if expected.is_empty() {
                assert!(
                    selected.events().batches().is_empty(),
                    "empty terminal producers retained event batches"
                );
            }
            let texts = selected
                .events()
                .batches()
                .iter()
                .flat_map(EventBatch::events)
                .map(|event| match event {
                    EvaluationEvent::StarlarkPrint { text, .. } => text.as_str(),
                    EvaluationEvent::Diagnostic { .. } => "<diagnostic>",
                })
                .collect::<Vec<_>>();
            assert_eq!(texts, expected);
        }
    }

    #[tokio::test]
    async fn build_command_root_unions_target_needs_and_replays_typed_analysis_lifecycle() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
        let union_key = BuildCommandRootKey::new(
            workspace.clone(),
            &[
                TargetPattern::parse("//left:t").unwrap(),
                TargetPattern::parse("//right:t").unwrap(),
            ],
            build_test_configuration("target"),
        )
        .unwrap();
        let mut need_transaction =
            build_root_transaction(&dice, BuildRootEpoch::base(10).build()).await;
        let need = need_transaction.compute(&union_key).await.unwrap();
        let slug_bzlmod_v2::SourcePreparationOutcome::Need(needs) = &need else {
            panic!("independent missing packages did not return Need");
        };
        let paths = needs
            .path_observations()
            .unwrap()
            .demands()
            .iter()
            .map(|demand| demand.path().as_path())
            .collect::<Vec<_>>();
        assert!(paths.contains(&Path::new("/workspace/left")), "{paths:?}");
        assert!(paths.contains(&Path::new("/workspace/right")), "{paths:?}");
        assert!(!BuildCommandRootKey::validity(&need));
        assert!(!BuildCommandRootKey::equality(&need, &need));

        let key = BuildCommandRootKey::new(
            workspace,
            &[TargetPattern::parse("//app:t").unwrap()],
            build_test_configuration("target"),
        )
        .unwrap();
        let epoch = |variant: i64, marker: &str, deleted: bool| {
            let mut epoch = BuildRootEpoch::base(variant);
            epoch.package("rules", "", variant);
            epoch.file(
                "/workspace/rules/defs.bzl",
                &format!(
                    "def _impl(ctx):\n    print(\"{marker}\")\n    return [DefaultInfo(files = depset([]))]\nprobe = rule(implementation = _impl)\n"
                ),
                variant,
            );
            if deleted {
                epoch.deleted_package("app", variant);
            } else {
                epoch.package(
                    "app",
                    "load(\"//rules:defs.bzl\", \"probe\")\nprobe(name = \"t\")\n",
                    variant,
                );
            }
            epoch.build()
        };
        let mut first_transaction = build_root_transaction(&dice, epoch(11, "V1", false)).await;
        let first = first_transaction.compute(&key).await.unwrap();
        let slug_bzlmod_v2::SourcePreparationOutcome::Complete(first_value) = &first else {
            panic!("first typed analysis returned Need");
        };
        assert!(
            first_value.as_ref().as_ref().unwrap().targets[0]
                .analysis
                .is_some()
        );

        let mut edited_transaction = build_root_transaction(&dice, epoch(12, "V2", false)).await;
        let edited = edited_transaction.compute(&key).await.unwrap();
        assert!(!BuildCommandRootKey::equality(&first, &edited));

        let mut deleted_transaction = build_root_transaction(&dice, epoch(13, "V2", true)).await;
        let deleted = deleted_transaction.compute(&key).await.unwrap();
        assert!(matches!(
            deleted,
            slug_bzlmod_v2::SourcePreparationOutcome::Complete(ref value)
                if matches!(
                    value.as_ref(),
                    Err(BuildCommandError {
                        kind: BuildCommandErrorKind::Package(_),
                    })
                )
        ));

        let mut restored_transaction = build_root_transaction(&dice, epoch(14, "V1", false)).await;
        let restored = restored_transaction.compute(&key).await.unwrap();
        assert!(BuildCommandRootKey::equality(&first, &restored));
    }

    #[tokio::test]
    async fn build_command_root_anchor_need_and_error_suppress_target_branches() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let key = BuildCommandRootKey::new(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            &[TargetPattern::parse("//app:t").unwrap()],
            build_test_configuration("target"),
        )
        .unwrap();
        let mut missing_epoch = BuildRootEpoch::default();
        missing_epoch.node("/", PathNodeKind::Directory, 19);
        missing_epoch.node("/workspace", PathNodeKind::Directory, 19);
        let mut missing_anchor = build_root_transaction(&dice, missing_epoch.build()).await;
        let missing = missing_anchor.compute(&key).await.unwrap();
        let slug_bzlmod_v2::SourcePreparationOutcome::Need(needs) = missing else {
            panic!("missing anchor did not return Need");
        };
        let paths = needs
            .path_observations()
            .unwrap()
            .demands()
            .iter()
            .map(|demand| demand.path().as_path())
            .collect::<Vec<_>>();
        assert!(
            paths.contains(&Path::new("/workspace/MODULE.bazel")),
            "{paths:?}"
        );
        assert!(!paths.contains(&Path::new("/workspace/app")), "{paths:?}");

        let mut invalid_epoch = BuildRootEpoch::base(20);
        invalid_epoch.file("/workspace/MODULE.bazel", "this is invalid (", 20);
        let mut invalid_anchor = build_root_transaction(&dice, invalid_epoch.build()).await;
        let invalid = invalid_anchor.compute(&key).await.unwrap();
        assert!(matches!(
            invalid,
            slug_bzlmod_v2::SourcePreparationOutcome::Complete(ref value)
                if matches!(
                    value.as_ref(),
                    Err(BuildCommandError {
                        kind: BuildCommandErrorKind::RootAnchor(_),
                    })
                )
        ));
    }

    #[tokio::test]
    async fn build_command_root_real_branches_use_no_legacy_keys_and_structure_missing_target() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(LegacyBuildTracker::default());
        let user_data = UserComputationData {
            activation_tracker: Some(tracker.clone() as Arc<dyn ActivationTracker>),
            ..Default::default()
        };
        let mut epoch = BuildRootEpoch::base(30);
        epoch.package("native", "filegroup(name = \"t\")\n", 30);
        epoch.package("rules", "", 30);
        epoch.file(
            "/workspace/rules/defs.bzl",
            "def _impl(ctx):\n    return [DefaultInfo(files = depset([]))]\nprobe = rule(implementation = _impl)\n",
            30,
        );
        epoch.package(
            "custom",
            "load(\"//rules:defs.bzl\", \"probe\")\nprobe(name = \"t\")\n",
            30,
        );
        let mut transaction =
            build_root_transaction_with_data(&dice, epoch.build(), user_data).await;
        let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
        let configuration = build_test_configuration("target");
        for pattern in ["//native:all", "//native:t", "//custom:t"] {
            let key = BuildCommandRootKey::new(
                workspace.clone(),
                &[TargetPattern::parse(pattern).unwrap()],
                configuration.clone(),
            )
            .unwrap();
            let value = compute_build_root(&mut transaction, &key, &tracker).await;
            assert!(matches!(
                value,
                slug_bzlmod_v2::SourcePreparationOutcome::Complete(ref value)
                    if value.as_ref().is_ok()
            ));
        }
        let missing_key = BuildCommandRootKey::new(
            workspace,
            &[TargetPattern::parse("//native:missing").unwrap()],
            configuration,
        )
        .unwrap();
        let missing = compute_build_root(&mut transaction, &missing_key, &tracker).await;
        assert!(matches!(
            missing,
            slug_bzlmod_v2::SourcePreparationOutcome::Complete(ref value)
                if matches!(
                    value.as_ref(),
                    Err(BuildCommandError {
                        kind: BuildCommandErrorKind::TargetNotFound {
                            pattern,
                            package,
                            target,
                            build_file,
                        },
                    }) if pattern.as_ref() == "//native:missing"
                        && package.as_str() == "native"
                        && target.as_str() == "missing"
                        && build_file == Path::new("/workspace/native/BUILD.bazel")
                )
        ));
        assert_eq!(tracker.forbidden.load(Ordering::Relaxed), 0);
    }

    #[tokio::test]
    async fn build_command_root_replays_root_module_and_build_create_edit_delete_restore() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
        let configuration = build_test_configuration("target");
        let empty =
            BuildCommandRootKey::new(workspace.clone(), &[], configuration.clone()).unwrap();
        let module_epoch = |variant: i64, source: Option<&str>| {
            let mut epoch = BuildRootEpoch::base(variant);
            match source {
                Some(source) => epoch.file("/workspace/MODULE.bazel", source, variant),
                None => epoch.missing("/workspace/MODULE.bazel"),
            }
            epoch.build()
        };
        let mut missing_module = build_root_transaction(&dice, module_epoch(40, None)).await;
        let module_missing = missing_module.compute(&empty).await.unwrap();
        assert!(matches!(
            module_missing,
            slug_bzlmod_v2::SourcePreparationOutcome::Need(_)
        ));
        assert!(!BuildCommandRootKey::validity(&module_missing));
        let mut created_module =
            build_root_transaction(&dice, module_epoch(41, Some("module(name = \"one\")\n"))).await;
        let module_v1 = created_module.compute(&empty).await.unwrap();
        let mut edited_module =
            build_root_transaction(&dice, module_epoch(42, Some("module(name = \"two\")\n"))).await;
        let module_v2 = edited_module.compute(&empty).await.unwrap();
        assert!(!BuildCommandRootKey::equality(&module_v1, &module_v2));
        let mut deleted_module = build_root_transaction(&dice, module_epoch(43, None)).await;
        let module_deleted = deleted_module.compute(&empty).await.unwrap();
        assert!(matches!(
            module_deleted,
            slug_bzlmod_v2::SourcePreparationOutcome::Need(_)
        ));
        let mut restored_module =
            build_root_transaction(&dice, module_epoch(44, Some("module(name = \"one\")\n"))).await;
        let module_restored = restored_module.compute(&empty).await.unwrap();
        assert!(BuildCommandRootKey::equality(&module_v1, &module_restored));

        let package = BuildCommandRootKey::new(
            workspace,
            &[TargetPattern::parse("//app:all").unwrap()],
            configuration,
        )
        .unwrap();
        let build_epoch = |variant: i64, source: Option<&str>| {
            let mut epoch = BuildRootEpoch::base(variant);
            match source {
                Some(source) => epoch.package("app", source, variant),
                None => epoch.deleted_package("app", variant),
            }
            epoch.build()
        };
        let mut missing_build = build_root_transaction(&dice, build_epoch(45, None)).await;
        assert!(matches!(
            missing_build.compute(&package).await.unwrap(),
            slug_bzlmod_v2::SourcePreparationOutcome::Complete(ref value)
                if matches!(
                    value.as_ref(),
                    Err(BuildCommandError {
                        kind: BuildCommandErrorKind::Package(_),
                    })
                )
        ));
        let mut created_build =
            build_root_transaction(&dice, build_epoch(46, Some("filegroup(name = \"v1\")\n")))
                .await;
        let build_v1 = created_build.compute(&package).await.unwrap();
        let mut edited_build =
            build_root_transaction(&dice, build_epoch(47, Some("filegroup(name = \"v2\")\n")))
                .await;
        let build_v2 = edited_build.compute(&package).await.unwrap();
        assert!(!BuildCommandRootKey::equality(&build_v1, &build_v2));
        let mut deleted_build = build_root_transaction(&dice, build_epoch(48, None)).await;
        assert!(matches!(
            deleted_build.compute(&package).await.unwrap(),
            slug_bzlmod_v2::SourcePreparationOutcome::Complete(ref value)
                if matches!(
                    value.as_ref(),
                    Err(BuildCommandError {
                        kind: BuildCommandErrorKind::Package(_),
                    })
                )
        ));
        let mut restored_build =
            build_root_transaction(&dice, build_epoch(49, Some("filegroup(name = \"v1\")\n")))
                .await;
        let build_restored = restored_build.compute(&package).await.unwrap();
        assert!(BuildCommandRootKey::equality(&build_v1, &build_restored));
    }

    type ObservedBuildOutcome = slug_bzlmod_v2::SourcePreparationOutcome<
        Result<ObservedBuildCommandRoot, ObservedPathFrontierError>,
    >;

    fn singleton_package_all_key(pattern: &str) -> BuildCommandRootKey {
        BuildCommandRootKey::new(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            &[TargetPattern::parse(pattern).unwrap()],
            build_test_configuration("target"),
        )
        .unwrap()
    }

    fn complete_observed_build(outcome: &ObservedBuildOutcome) -> &ObservedBuildCommandRoot {
        let slug_bzlmod_v2::SourcePreparationOutcome::Complete(Ok(observed)) = outcome else {
            panic!("observed build did not complete successfully: {outcome:?}");
        };
        observed
    }

    #[derive(Debug, Clone)]
    struct ObservedBuildActivation {
        key: String,
        kind: dice::ActivationKind,
        batch: Option<EventBatch>,
    }

    #[derive(Default)]
    struct ObservedBuildTracker(Mutex<Vec<ObservedBuildActivation>>);

    impl ObservedBuildTracker {
        fn take(&self) -> Vec<ObservedBuildActivation> {
            std::mem::take(&mut *self.0.lock().unwrap())
        }
    }

    impl ActivationTracker for ObservedBuildTracker {
        fn key_activated(
            &self,
            _key: &DynKey,
            _deps: &mut dyn Iterator<Item = &DynKey>,
            _activation: ActivationData,
        ) {
        }

        fn tracks_rich_activations(&self) -> bool {
            true
        }

        fn key_activated_rich(&self, key: &DynKey, activation: RichActivation<'_>) {
            if key.downcast_ref::<BuildCommandRootKey>().is_none()
                && key
                    .downcast_ref::<BuildCommandRootObservationKey>()
                    .is_none()
                && key.downcast_ref::<RootModuleLoadingAnchorKey>().is_none()
                && key
                    .downcast_ref::<RootModuleLoadingAnchorObservationKey>()
                    .is_none()
                && key.downcast_ref::<RootPackageLoadKey>().is_none()
                && key
                    .downcast_ref::<RootPackageLoadObservationKey>()
                    .is_none()
            {
                return;
            }
            self.0.lock().unwrap().push(ObservedBuildActivation {
                key: key.to_string(),
                kind: activation.kind(),
                batch: activation
                    .evaluation_data()
                    .and_then(|data| data.downcast_ref::<EventBatch>())
                    .map(Dupe::dupe),
            });
        }
    }

    fn assert_observed_epoch_uses_input_arcs(
        observed: &PathObservationEpoch,
        input: &PathObservationEpoch,
    ) {
        assert!(!observed.observations().is_empty());
        for (demand, result) in observed.observations() {
            assert!(
                Arc::ptr_eq(result, input.get(demand).unwrap()),
                "observation did not retain the injected Arc for {demand:?}"
            );
        }
    }

    #[derive(Clone)]
    struct PointerDistinctObservedBuildRoot(BuildCommandRootObservationKey);

    #[async_trait]
    impl NativeCommandRoot for PointerDistinctObservedBuildRoot {
        type Terminal = ObservedBuildCommandRoot;

        fn observations<'a>(
            &self,
            terminal: &'a Self::Terminal,
        ) -> Option<&'a PathObservationEpoch> {
            Some(terminal.observations())
        }

        async fn compute(
            &self,
            transaction: &mut dice::DiceTransaction,
        ) -> Result<
            slug_bzlmod_v2::SourcePreparationOutcome<Self::Terminal>,
            NativeDemandSessionError,
        > {
            let outcome = NativeCommandRoot::compute(&self.0, transaction).await?;
            Ok(match outcome {
                slug_bzlmod_v2::SourcePreparationOutcome::Need(need) => {
                    slug_bzlmod_v2::SourcePreparationOutcome::Need(need)
                }
                slug_bzlmod_v2::SourcePreparationOutcome::Complete(mut terminal) => {
                    terminal.observations = PathObservationEpoch::from_shared(
                        terminal
                            .observations
                            .observations()
                            .iter()
                            .map(|(demand, result)| {
                                (demand.dupe(), Arc::new(result.as_ref().clone()))
                            }),
                    )
                    .unwrap();
                    slug_bzlmod_v2::SourcePreparationOutcome::Complete(terminal)
                }
            })
        }
    }

    #[test]
    fn observed_terminal_validation_requires_complete_values_and_exact_arcs() {
        let workspace = tempfile::tempdir().unwrap();
        let runtime = test_runtime(workspace.path()).unwrap();
        let mut snapshot = accepted_native_snapshot(&runtime);
        let first = BuildRootEpoch::demand("/first", PathObservationOperation::Lstat);
        let second = BuildRootEpoch::demand("/second", PathObservationOperation::Lstat);
        let missing = PathObservationResult::Lstat(PathOperationResult::Missing);
        let observed = PathObservationEpoch::new([(first.dupe(), missing.clone())]).unwrap();
        snapshot.path_observations = PathObservationEpoch::from_shared([(
            first.dupe(),
            observed.get(&first).unwrap().dupe(),
        )])
        .unwrap();
        snapshot.selected = SelectedWorkspaceDemands::for_test([], [first.dupe()]);
        validate_observed_terminal(&observed, &snapshot).unwrap();
        let assert_mismatch = |observed: &PathObservationEpoch,
                               snapshot: &AcceptedNativeDemandSnapshot,
                               expected: ObservedTerminalMismatch| {
            let Err(NativeDemandSessionError::ObservedTerminal(actual)) =
                validate_observed_terminal(observed, snapshot)
            else {
                panic!("observed validation did not return a typed mismatch");
            };
            assert_eq!(actual, expected);
        };

        snapshot.path_observations =
            PathObservationEpoch::new([(first.dupe(), missing.clone())]).unwrap();
        assert_mismatch(&observed, &snapshot, ObservedTerminalMismatch::ResultArc);
        snapshot.path_observations = PathObservationEpoch::new([(
            first.dupe(),
            PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
                PathNodeKind::RegularFile,
                1,
                1,
                1,
                1,
                0o644,
            ))),
        )])
        .unwrap();
        let fresh_changed = snapshot.path_observations.dupe();
        assert_mismatch(&observed, &snapshot, ObservedTerminalMismatch::Value);
        snapshot.path_observations = PathObservationEpoch::new([(second.dupe(), missing)]).unwrap();
        snapshot.selected = SelectedWorkspaceDemands::for_test([], [second]);
        assert_mismatch(&observed, &snapshot, ObservedTerminalMismatch::Demand);
        snapshot.path_observations = PathObservationEpoch::empty();
        snapshot.selected = SelectedWorkspaceDemands::empty();
        assert_mismatch(&observed, &snapshot, ObservedTerminalMismatch::Length);

        let normalized =
            NormalizedAbsolutePath::new(workspace.path().canonicalize().unwrap()).unwrap();
        let request = local_native_request(&normalized, "dep+", ".");
        snapshot.selected = SelectedWorkspaceDemands::for_test([request.dupe()], []);
        assert_mismatch(
            &PathObservationEpoch::empty(),
            &snapshot,
            ObservedTerminalMismatch::RepositoryRequests,
        );
        snapshot.selected =
            SelectedWorkspaceDemands::for_test_with_validation([], request, first.dupe());
        assert_mismatch(
            &PathObservationEpoch::empty(),
            &snapshot,
            ObservedTerminalMismatch::RepositoryValidations,
        );
        let fresh_equal = PathObservationEpoch::new([(
            first.dupe(),
            observed.get(&first).unwrap().as_ref().clone(),
        )])
        .unwrap();
        let reconciled = preserve_equal_observation_arcs(&observed, &fresh_equal);
        assert!(Arc::ptr_eq(
            reconciled.get(&first).unwrap(),
            observed.get(&first).unwrap()
        ));
        let reconciled = preserve_equal_observation_arcs(&observed, &fresh_changed);
        assert!(Arc::ptr_eq(
            reconciled.get(&first).unwrap(),
            fresh_changed.get(&first).unwrap()
        ));
        let absent =
            preserve_equal_observation_arcs(&PathObservationEpoch::empty(), &fresh_changed);
        assert!(Arc::ptr_eq(
            absent.get(&first).unwrap(),
            fresh_changed.get(&first).unwrap()
        ));
    }

    #[test]
    fn public_singleton_observation_replays_lifecycle_and_isolates_legacy() {
        let workspace = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR")).unwrap();
        fs::write(
            workspace.path().join("MODULE.bazel"),
            "print('MODULE_EVENT')\nmodule(name = 'publication')\n",
        )
        .unwrap();
        fs::create_dir(workspace.path().join("pkg")).unwrap();
        let build_file = workspace.path().join("pkg/BUILD.bazel");
        let write_build = |name: &str| {
            fs::write(
                &build_file,
                format!("print('PACKAGE_EVENT')\nfilegroup(name = '{name}')\n"),
            )
            .unwrap();
        };
        write_build("a");
        let audit = Arc::new(ExternalQueryActivationAudit::default());
        let runtime = test_runtime(workspace.path())
            .unwrap()
            .with_activation_audit(audit.dupe());
        let run = |target: &str| {
            runtime.build_command_with_bzlmod_inputs(
                &[TargetPattern::parse(target).unwrap()],
                BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                LockfileMode::Update,
                &[],
                None,
            )
        };

        let cold = run("//pkg:all").unwrap();
        let a = cold.terminal_for_test().dupe();
        assert!(a.as_ref().is_ok());
        assert_eq!(
            accepted_output_text(&cold),
            ["MODULE_EVENT", "PACKAGE_EVENT"]
        );
        assert!(
            accepted_native_snapshot(&runtime)
                .selected
                .repository_requests()
                .is_empty()
        );
        let after_cold = audit.build_root_counts();
        assert!(after_cold.0 > 0);
        assert_eq!(after_cold.1, 0);

        let cold_epoch = accepted_native_snapshot(&runtime).path_observations;
        let warm = run("//pkg:all").unwrap();
        assert_eq!(accepted_output_text(&warm), Vec::<&str>::new());
        let warm_epoch = accepted_native_snapshot(&runtime).path_observations;
        assert!(
            cold_epoch
                .observations()
                .iter()
                .all(|(demand, result)| { Arc::ptr_eq(result, warm_epoch.get(demand).unwrap()) })
        );
        write_build("b");
        let b = run("//pkg:all").unwrap();
        assert_ne!(a.as_ref(), b.terminal_for_test().as_ref());
        fs::remove_file(&build_file).unwrap();
        assert!(
            run("//pkg:all")
                .unwrap()
                .terminal_for_test()
                .as_ref()
                .is_err()
        );
        write_build("a");
        let restored = run("//pkg:all").unwrap();
        assert_eq!(a.as_ref(), restored.terminal_for_test().as_ref());

        let before_legacy = audit.build_root_counts();
        let legacy = run("//pkg:a").unwrap();
        assert!(legacy.terminal_for_test().as_ref().is_ok());
        let after_legacy = audit.build_root_counts();
        assert_eq!(after_legacy.0, before_legacy.0);
        assert!(after_legacy.1 > before_legacy.1);
    }

    #[test]
    fn pointer_distinct_observed_epoch_aborts_before_publication_and_recovers() {
        let stable_parent = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/debug/incremental/slug-pointer-distinct-observed-build");
        fs::create_dir_all(&stable_parent).unwrap();
        let workspace = tempfile::tempdir_in(stable_parent).unwrap();
        fs::write(
            workspace.path().join("MODULE.bazel"),
            "module(name = 'abort')\n",
        )
        .unwrap();
        fs::create_dir(workspace.path().join("pkg")).unwrap();
        fs::write(
            workspace.path().join("pkg/BUILD.bazel"),
            "filegroup(name = 'a')\n",
        )
        .unwrap();
        let runtime = test_runtime(workspace.path()).unwrap();
        let target = TargetPattern::parse("//pkg:all").unwrap();
        let accepted = runtime
            .build_command_with_bzlmod_inputs(
                std::slice::from_ref(&target),
                BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                LockfileMode::Update,
                &[],
                None,
            )
            .unwrap();
        assert!(accepted.terminal_for_test().as_ref().is_ok());
        let prior = accepted_native_snapshot(&runtime);
        runtime.native_demand_sessions.take_trace();

        let host = runtime.process_host.default_configuration_inputs().unwrap();
        let configuration = SlugConfiguration::default_target(&host).unwrap();
        let key = BuildCommandRootKey::new(
            NormalizedAbsolutePath::new(runtime.workspace.clone()).unwrap(),
            std::slice::from_ref(&target),
            ConfigurationKey::from_slug(configuration),
        )
        .unwrap();
        let error = runtime
            .drive_command(
                NativeDemandRequestInputBundle::normalized_initial(),
                PointerDistinctObservedBuildRoot(BuildCommandRootObservationKey::new(key).unwrap()),
            )
            .unwrap_err();
        assert!(matches!(
            error,
            NativeDemandSessionError::ObservedTerminal(ObservedTerminalMismatch::ResultArc)
        ));
        assert_current_native_snapshot(&runtime, &prior);
        let trace = runtime.native_demand_sessions.take_trace();
        assert!(trace.contains(&NativeDemandTestTrace::AttemptTransactionDroppedBeforeAbort));
        assert!(!trace.contains(&NativeDemandTestTrace::SelectedInjectionCommitted));
        assert!(!trace.contains(&NativeDemandTestTrace::AcceptedSnapshotReplaced));
        assert!(
            runtime
                .build_command_with_bzlmod_inputs(
                    &[target],
                    BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                    BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                    LockfileMode::Update,
                    &[],
                    None,
                )
                .unwrap()
                .terminal_for_test()
                .as_ref()
                .is_ok()
        );
    }

    #[test]
    fn observed_build_identity_and_epoch_union_are_restricted_and_left_stable() {
        let accepted = singleton_package_all_key("//pkg:all");
        let observed = BuildCommandRootObservationKey::new(accepted.clone()).unwrap();
        assert_eq!(observed.to_string(), format!("observed-{accepted}"));

        let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
        let configuration = build_test_configuration("target");
        for targets in [
            vec![],
            vec![TargetPattern::parse("//pkg:t").unwrap()],
            vec![
                TargetPattern::parse("//pkg:all").unwrap(),
                TargetPattern::parse("//other:all").unwrap(),
            ],
            vec![TargetPattern::parse("@repo//pkg:t").unwrap()],
        ] {
            let legacy =
                BuildCommandRootKey::new(workspace.clone(), &targets, configuration.clone())
                    .unwrap();
            assert!(BuildCommandRootObservationKey::new(legacy).is_none());
        }

        let demand = BuildRootEpoch::demand("/same", PathObservationOperation::Lstat);
        let result = PathObservationResult::Lstat(PathOperationResult::Missing);
        let left = PathObservationEpoch::new([(demand.dupe(), result.clone())]).unwrap();
        let duplicate = PathObservationEpoch::new([(demand.dupe(), result)]).unwrap();
        let first = left.get(&demand).unwrap().dupe();
        let merged = union_build_observations(&left, &duplicate).unwrap();
        assert!(Arc::ptr_eq(merged.get(&demand).unwrap(), &first));
        let conflicting = PathObservationEpoch::new([(
            demand,
            PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
                PathNodeKind::RegularFile,
                1,
                1,
                1,
                1,
                0o644,
            ))),
        )])
        .unwrap();
        assert!(union_build_observations(&merged, &conflicting).is_err());
    }

    #[tokio::test]
    async fn observed_build_matches_legacy_arcs_events_and_isolates_key_families() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(ObservedBuildTracker::default());
        let mut user_data = UserComputationData {
            activation_tracker: Some(tracker.dupe()),
            ..Default::default()
        };
        user_data.data.set(CaptureEvaluationEvents);
        let mut builder = BuildRootEpoch::base(1);
        builder.package("pkg", "print('PACKAGE')\nfilegroup(name = 'target')\n", 1);
        let epoch = builder.build();
        let legacy_key = singleton_package_all_key("//pkg:all");
        let observed_key = BuildCommandRootObservationKey::new(legacy_key.clone()).unwrap();
        let mut observed_transaction =
            build_root_transaction_with_data(&dice, epoch.dupe(), user_data).await;
        let observed_outcome = observed_transaction.compute(&observed_key).await.unwrap();
        let observed = complete_observed_build(&observed_outcome);
        let evaluation = observed.result().as_ref().as_ref().unwrap();
        assert_eq!(evaluation.loaded_package_count(), 1);
        assert_eq!(evaluation.analyzed_target_count(), 0);
        assert_eq!(evaluation.declared_action_count(), 0);
        assert_eq!(evaluation.targets[0].pattern.as_ref(), "//pkg:all");
        assert!(evaluation.action_closure.is_empty());
        assert_observed_epoch_uses_input_arcs(observed.observations(), &epoch);
        assert!(BuildCommandRootObservationKey::validity(&observed_outcome));
        assert!(BuildCommandRootObservationKey::equality(
            &observed_outcome,
            &observed_outcome
        ));

        let activations = tracker.take();
        assert!(activations.iter().any(|entry| {
            entry.key.starts_with("observed-build-command-root:")
                && entry.kind == dice::ActivationKind::Evaluated
                && entry.batch.is_none()
        }));
        assert!(activations.iter().any(|entry| {
            entry.key.starts_with("observed-host-package-load:")
                && entry.batch.as_ref().is_some_and(|batch| {
                    batch.events().iter().any(|event| {
                        matches!(
                            event,
                            EvaluationEvent::StarlarkPrint { text, .. } if text == "PACKAGE"
                        )
                    })
                })
        }));
        assert!(
            activations
                .iter()
                .all(|entry| !entry.key.starts_with("build-command-root:")
                    && !entry.key.starts_with("root-module-loading-anchor:")
                    && !entry.key.starts_with("host-package-load:"))
        );

        let legacy_data = UserComputationData {
            activation_tracker: Some(tracker.dupe()),
            ..Default::default()
        };
        let mut legacy = build_root_transaction_with_data(&dice, epoch, legacy_data).await;
        let legacy_outcome = legacy.compute(&legacy_key).await.unwrap();
        let slug_bzlmod_v2::SourcePreparationOutcome::Complete(legacy_result) = legacy_outcome
        else {
            panic!("legacy singleton package-all returned Need");
        };
        assert_eq!(observed.result().as_ref(), legacy_result.as_ref());
        let activations = tracker.take();
        assert!(
            activations
                .iter()
                .any(|entry| entry.key.starts_with("build-command-root:"))
        );
        assert!(activations.iter().all(|entry| {
            !entry.key.starts_with("observed-build-command-root:")
                && !entry
                    .key
                    .starts_with("observed-root-module-loading-anchor:")
                && !entry.key.starts_with("observed-host-package-load:")
        }));
    }

    #[tokio::test]
    async fn observed_build_terminal_polarity_keeps_only_complete_semantic_prefixes() {
        let key =
            BuildCommandRootObservationKey::new(singleton_package_all_key("//pkg:all")).unwrap();

        let need_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut need = build_root_transaction(&need_dice, BuildRootEpoch::base(2).build()).await;
        let need = need.compute(&key).await.unwrap();
        assert!(matches!(
            need,
            slug_bzlmod_v2::SourcePreparationOutcome::Need(_)
        ));
        assert!(!BuildCommandRootObservationKey::validity(&need));
        assert!(!BuildCommandRootObservationKey::equality(&need, &need));

        let anchor_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut anchor_epoch = BuildRootEpoch::base(3);
        anchor_epoch.file("/workspace/MODULE.bazel", "this is invalid (", 3);
        anchor_epoch.package("pkg", "filegroup(name = 'target')\n", 3);
        let anchor_epoch = anchor_epoch.build();
        let mut anchor = build_root_transaction(&anchor_dice, anchor_epoch.dupe()).await;
        let anchor = anchor.compute(&key).await.unwrap();
        let anchor = complete_observed_build(&anchor);
        assert!(matches!(
            anchor.result().as_ref(),
            Err(BuildCommandError {
                kind: BuildCommandErrorKind::RootAnchor(_),
            })
        ));
        assert_observed_epoch_uses_input_arcs(anchor.observations(), &anchor_epoch);
        assert!(
            anchor
                .observations()
                .observations()
                .iter()
                .all(|(demand, _)| !demand.path().as_path().starts_with("/workspace/pkg"))
        );

        let package_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut package_epoch = BuildRootEpoch::base(4);
        package_epoch.deleted_package("pkg", 4);
        let package_epoch = package_epoch.build();
        let mut package = build_root_transaction(&package_dice, package_epoch.dupe()).await;
        let package = package.compute(&key).await.unwrap();
        let package = complete_observed_build(&package);
        assert!(matches!(
            package.result().as_ref(),
            Err(BuildCommandError {
                kind: BuildCommandErrorKind::Package(_),
            })
        ));
        assert_observed_epoch_uses_input_arcs(package.observations(), &package_epoch);
        assert!(
            package
                .observations()
                .observations()
                .iter()
                .any(|(demand, _)| demand.path().as_path().starts_with("/workspace/pkg"))
        );

        let outer_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let outer_error = PathObservationEpoch::from_shared([(
            BuildRootEpoch::demand("/mismatch", PathObservationOperation::Lstat),
            Arc::new(PathObservationResult::FileBytes(
                PathOperationResult::Missing,
            )),
        )])
        .unwrap_err()
        .into();
        let mut outer_data = UserComputationData::default();
        outer_data
            .data
            .set(ForceBuildCommandRootObservationOuter(outer_error));
        let mut outer_epoch = BuildRootEpoch::base(5);
        outer_epoch.package("pkg", "filegroup(name = 'target')\n", 5);
        let mut outer =
            build_root_transaction_with_data(&outer_dice, outer_epoch.build(), outer_data).await;
        let outer = NativeCommandRoot::compute(&key, &mut outer)
            .await
            .unwrap_err();
        assert!(matches!(outer, NativeDemandSessionError::Computation(_)));
    }

    #[tokio::test]
    async fn observed_build_replays_lifecycle_and_cancellation_without_parent_publication() {
        let key =
            BuildCommandRootObservationKey::new(singleton_package_all_key("//pkg:all")).unwrap();
        let epoch = |variant: i64, target: Option<&str>| {
            let mut epoch = BuildRootEpoch::base(variant);
            match target {
                Some(target) => {
                    epoch.package("pkg", &format!("filegroup(name = '{target}')\n"), variant)
                }
                None => epoch.deleted_package("pkg", variant),
            }
            epoch.build()
        };
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut missing = build_root_transaction(&dice, epoch(10, None)).await;
        let missing = missing.compute(&key).await.unwrap();
        assert!(complete_observed_build(&missing).result().is_err());
        let mut created = build_root_transaction(&dice, epoch(11, Some("v1"))).await;
        let created = created.compute(&key).await.unwrap();
        let mut warm = build_root_transaction(&dice, epoch(11, Some("v1"))).await;
        let warm = warm.compute(&key).await.unwrap();
        assert!(BuildCommandRootObservationKey::equality(&created, &warm));
        assert!(Arc::ptr_eq(
            complete_observed_build(&created).result(),
            complete_observed_build(&warm).result()
        ));
        let mut edited = build_root_transaction(&dice, epoch(12, Some("v2"))).await;
        let edited = edited.compute(&key).await.unwrap();
        assert!(!BuildCommandRootObservationKey::equality(&created, &edited));
        let mut deleted = build_root_transaction(&dice, epoch(13, None)).await;
        assert!(
            complete_observed_build(&deleted.compute(&key).await.unwrap())
                .result()
                .is_err()
        );
        let mut restored = build_root_transaction(&dice, epoch(11, Some("v1"))).await;
        let restored = restored.compute(&key).await.unwrap();
        assert!(BuildCommandRootObservationKey::equality(
            &created, &restored
        ));

        let cancelled_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(ObservedBuildTracker::default());
        let mut data = UserComputationData {
            activation_tracker: Some(tracker.dupe()),
            ..Default::default()
        };
        data.data.set(CaptureEvaluationEvents);
        let complete_epoch = epoch(20, Some("target"));
        let mut cancelled =
            build_root_transaction_with_data(&cancelled_dice, complete_epoch.dupe(), data).await;
        let mut future = Box::pin(cancelled.compute(&key));
        std::future::poll_fn(|context| {
            assert!(Future::poll(future.as_mut(), context).is_pending());
            Poll::Ready(())
        })
        .await;
        drop(future);
        assert!(
            tracker
                .take()
                .iter()
                .all(|entry| { !entry.key.starts_with("observed-build-command-root:") })
        );
        drop(cancelled);

        let mut recovery = build_root_transaction(&cancelled_dice, complete_epoch).await;
        let recovered = recovery.compute(&key).await.unwrap();
        assert!(complete_observed_build(&recovered).result().is_ok());
    }

    #[test]
    fn build_branch_collection_has_total_infrastructure_need_and_error_precedence() {
        let first = build_test_error("//first:missing");
        let second = build_test_error("//second:missing");
        let complete_error = |error| {
            BuildBranchResult::Outcome(slug_bzlmod_v2::SourcePreparationOutcome::Complete(Err(
                error,
            )))
        };

        let slug_bzlmod_v2::SourcePreparationOutcome::Complete(Err(selected)) =
            collect_build_branches(vec![
                complete_error(first.clone()),
                complete_error(second.clone()),
            ])
            .unwrap()
        else {
            panic!("Complete errors did not remain terminal");
        };
        assert_eq!(selected, first);
        let slug_bzlmod_v2::SourcePreparationOutcome::Complete(Err(selected)) =
            collect_build_branches(vec![
                complete_error(second.clone()),
                complete_error(first.clone()),
            ])
            .unwrap()
        else {
            panic!("reversed Complete errors did not remain terminal");
        };
        assert_eq!(selected, second);

        let need_a = build_test_need("/workspace/a");
        let need_b = build_test_need("/workspace/b");
        let slug_bzlmod_v2::SourcePreparationOutcome::Need(combined) =
            collect_build_branches(vec![
                complete_error(first),
                BuildBranchResult::Outcome(slug_bzlmod_v2::SourcePreparationOutcome::Need(need_a)),
                BuildBranchResult::Outcome(slug_bzlmod_v2::SourcePreparationOutcome::Need(need_b)),
            ])
            .unwrap()
        else {
            panic!("reached Needs did not dominate a Complete error");
        };
        let paths = combined.path_observations().unwrap().demands();
        assert_eq!(paths.len(), 2);

        let infrastructure: Arc<str> = Arc::from("cancelled");
        assert_eq!(
            collect_build_branches(vec![
                BuildBranchResult::Outcome(slug_bzlmod_v2::SourcePreparationOutcome::Need(
                    build_test_need("/workspace/need"),
                )),
                BuildBranchResult::Infrastructure(infrastructure.clone()),
            ])
            .unwrap_err(),
            infrastructure
        );

        let conflicting_a = slug_bzlmod_v2::SourcePreparationNeeds::root_module_bootstrap(
            slug_bzlmod_v2::RootModuleBootstrapRequest {
                workspace: NormalizedAbsolutePath::new("/workspace").unwrap(),
            },
        );
        let conflicting_b = slug_bzlmod_v2::SourcePreparationNeeds::root_module_bootstrap(
            slug_bzlmod_v2::RootModuleBootstrapRequest {
                workspace: NormalizedAbsolutePath::new("/other").unwrap(),
            },
        );
        assert!(
            collect_build_branches(vec![
                BuildBranchResult::Outcome(slug_bzlmod_v2::SourcePreparationOutcome::Need(
                    conflicting_a,
                )),
                BuildBranchResult::Outcome(slug_bzlmod_v2::SourcePreparationOutcome::Need(
                    conflicting_b,
                )),
            ])
            .is_err()
        );
    }

    fn neutral_key(pattern: &str) -> Option<SingletonRootSingleBuildCommandKey> {
        SingletonRootSingleBuildCommandKey::new(
            BuildCommandRootKey::new(
                NormalizedAbsolutePath::new("/workspace").unwrap(),
                &[TargetPattern::parse(pattern).unwrap()],
                build_test_configuration("neutral"),
            )
            .unwrap(),
        )
    }

    fn complete_neutral(
        outcome: &SingletonRootSingleDriverOutcome,
    ) -> &SingletonRootSingleBuildCommandTerminal {
        let slug_bzlmod_v2::SourcePreparationOutcome::Complete(Ok(terminal)) = outcome else {
            panic!("neutral build root did not complete: {outcome:?}");
        };
        terminal
    }

    #[test]
    fn neutral_identity_and_source_carrier_are_complete_only_and_exact() {
        let key = neutral_key("//pkg:source.txt").unwrap();
        assert!(key.to_string().starts_with("neutral-build-command-root:"));
        assert!(neutral_key("//pkg:all").is_none());
        assert!(neutral_key("@repo//pkg:source.txt").is_none());
        let multi = BuildCommandRootKey::new(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            &[
                TargetPattern::parse("//pkg:one").unwrap(),
                TargetPattern::parse("//pkg:two").unwrap(),
            ],
            build_test_configuration("neutral"),
        )
        .unwrap();
        assert!(SingletonRootSingleBuildCommandKey::new(multi).is_none());

        let demand = BuildRootEpoch::demand(
            "/workspace/pkg/source.txt",
            PathObservationOperation::FileBytes,
        );
        let result = Arc::new(PathObservationResult::FileBytes(
            PathOperationResult::Present(Arc::from(&b"source"[..])),
        ));
        let inputs =
            PathObservationEpoch::from_shared([(demand.dupe(), result.dupe())]).unwrap();
        let semantic = BuildCommandError::new(BuildCommandErrorKind::RootSource {
            observation: result.as_ref().clone(),
            source_certificate: Some(Box::new(SourceCertificate::new(
                demand.dupe(),
                result.dupe(),
            ))),
        });
        let completed = singleton_root_single_complete(Err(semantic), &inputs);
        let terminal = complete_neutral(&completed);
        let observations = terminal.observations.as_ref().unwrap();
        assert!(Arc::ptr_eq(observations.get(&demand).unwrap(), &result));
        assert!(Arc::ptr_eq(
            terminal.source_certificate().unwrap().observation(),
            &result
        ));
        assert!(SingletonRootSingleBuildCommandKey::validity(&completed));
        assert!(SingletonRootSingleBuildCommandKey::equality(
            &completed, &completed
        ));

        let semantic_only = singleton_root_single_complete(
            Err(BuildCommandError::new(BuildCommandErrorKind::Analysis(
                AnalysisError::message("semantic"),
            ))),
            &inputs,
        );
        assert!(complete_neutral(&semantic_only).observations.is_none());

        let need: SingletonRootSingleDriverOutcome =
            slug_bzlmod_v2::SourcePreparationOutcome::Need(build_test_need("/workspace/pkg"));
        assert!(!SingletonRootSingleBuildCommandKey::validity(&need));
        assert!(!SingletonRootSingleBuildCommandKey::equality(&need, &need));

        let conflicting = Arc::new(PathObservationResult::FileBytes(
            PathOperationResult::Missing,
        ));
        let conflict = singleton_root_single_complete(
            Err(BuildCommandError::new(BuildCommandErrorKind::RootSource {
                observation: conflicting.as_ref().clone(),
                source_certificate: Some(Box::new(SourceCertificate::new(
                    demand,
                    conflicting,
                ))),
            })),
            &inputs,
        );
        assert!(matches!(
            conflict,
            slug_bzlmod_v2::SourcePreparationOutcome::Complete(Err(_))
        ));
        assert!(SingletonRootSingleBuildCommandKey::validity(&conflict));
    }

    #[derive(Debug)]
    struct NeutralActivation {
        key: String,
        batch: Option<EventBatch>,
    }

    #[derive(Default)]
    struct NeutralFamilyTracker {
        activations: Mutex<Vec<NeutralActivation>>,
        forbidden: Mutex<Vec<String>>,
        neutral_roots: AtomicUsize,
        observed_analyses: AtomicUsize,
    }

    impl ActivationTracker for NeutralFamilyTracker {
        fn key_activated(
            &self,
            _key: &DynKey,
            _deps: &mut dyn Iterator<Item = &DynKey>,
            _activation: ActivationData,
        ) {
        }

        fn tracks_rich_activations(&self) -> bool {
            true
        }

        fn key_activated_rich(&self, key: &DynKey, activation: RichActivation<'_>) {
            let text = key.to_string();
            if key.downcast_ref::<BuildCommandRootKey>().is_some()
                || key.downcast_ref::<RootModuleLoadingAnchorKey>().is_some()
                || key.downcast_ref::<RootPackageLoadKey>().is_some()
                || key.downcast_ref::<ConfiguredNodeAnalysisKey>().is_some()
                || text.starts_with("resolved-path:")
            {
                self.forbidden.lock().unwrap().push(text.clone());
            }
            if key
                .downcast_ref::<SingletonRootSingleBuildCommandKey>()
                .is_some()
            {
                self.neutral_roots.fetch_add(1, Ordering::Relaxed);
            }
            if key
                .downcast_ref::<ConfiguredNodeAnalysisObservationKey>()
                .is_some()
            {
                self.observed_analyses.fetch_add(1, Ordering::Relaxed);
            }
            self.activations.lock().unwrap().push(NeutralActivation {
                key: text,
                batch: activation
                    .evaluation_data()
                    .and_then(|data| data.downcast_ref::<EventBatch>())
                    .map(Dupe::dupe),
            });
        }
    }

    #[tokio::test]
    async fn neutral_rule_and_filegroup_use_one_observed_family_and_child_events() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(NeutralFamilyTracker::default());
        let mut data = UserComputationData {
            activation_tracker: Some(tracker.dupe()),
            ..Default::default()
        };
        data.data.set(CaptureEvaluationEvents);
        let mut epoch = BuildRootEpoch::base(70);
        epoch.file(
            "/workspace/MODULE.bazel",
            "print('MODULE')\nmodule(name = 'neutral')\n",
            70,
        );
        epoch.package("rules", "", 70);
        epoch.file(
            "/workspace/rules/defs.bzl",
            "print('BZL')\ndef _impl(ctx):\n    print('ANALYSIS')\n    return [DefaultInfo(files = depset([]))]\nprobe = rule(implementation = _impl)\n",
            70,
        );
        epoch.package(
            "pkg",
            "print('BUILD')\nload('//rules:defs.bzl', 'probe')\nprobe(name = 'rule')\nfilegroup(name = 'files')\n",
            70,
        );
        let epoch = epoch.build();
        let mut transaction =
            build_root_transaction_with_data(&dice, epoch.dupe(), data).await;
        let rule_key = neutral_key("//pkg:rule").unwrap();
        let rule = transaction.compute(&rule_key).await.unwrap();
        let rule = complete_neutral(&rule);
        let evaluation = rule.result.as_ref().as_ref().unwrap();
        assert!(evaluation.targets[0].analysis.is_some());
        assert!(rule.observations.is_none());

        let filegroup_key = neutral_key("//pkg:files").unwrap();
        let filegroup = transaction.compute(&filegroup_key).await.unwrap();
        let filegroup = complete_neutral(&filegroup);
        let evaluation = filegroup.result.as_ref().as_ref().unwrap();
        assert!(evaluation.targets[0].analysis.is_none());
        assert!(filegroup.observations.is_none());

        assert!(tracker.forbidden.lock().unwrap().is_empty());
        assert_eq!(tracker.neutral_roots.load(Ordering::Relaxed), 2);
        assert!(tracker.observed_analyses.load(Ordering::Relaxed) > 0);
        let activations = std::mem::take(&mut *tracker.activations.lock().unwrap());
        assert!(
            activations
                .iter()
                .filter(|entry| entry.key.starts_with("neutral-build-command-root:"))
                .all(|entry| entry.batch.is_none())
        );
        let texts = activations
            .iter()
            .filter_map(|entry| entry.batch.as_ref())
            .flat_map(EventBatch::events)
            .filter_map(|event| match event {
                EvaluationEvent::StarlarkPrint { text, .. } => Some(text.as_str()),
                EvaluationEvent::Diagnostic { .. } => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(texts, ["MODULE", "BZL", "BUILD", "ANALYSIS"]);

        let cancelled = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let cancelled_tracker = Arc::new(NeutralFamilyTracker::default());
        let mut cancelled_data = UserComputationData {
            activation_tracker: Some(cancelled_tracker.dupe()),
            ..Default::default()
        };
        cancelled_data.data.set(CaptureEvaluationEvents);
        let mut cancelled_transaction = build_root_transaction_with_data(
            &cancelled,
            epoch.dupe(),
            cancelled_data,
        )
        .await;
        let mut future = Box::pin(cancelled_transaction.compute(&rule_key));
        std::future::poll_fn(|context| {
            assert!(Future::poll(future.as_mut(), context).is_pending());
            Poll::Ready(())
        })
        .await;
        drop(future);
        assert!(
            cancelled_tracker
                .activations
                .lock()
                .unwrap()
                .iter()
                .all(|entry| !entry.key.starts_with("neutral-build-command-root:"))
        );
        drop(cancelled_transaction);
        let mut recovery = build_root_transaction(&cancelled, epoch).await;
        assert!(
            complete_neutral(&recovery.compute(&rule_key).await.unwrap())
                .result
                .as_ref()
                .is_ok()
        );
    }

    #[derive(Clone)]
    struct PointerDistinctNeutralRoot(SingletonRootSingleBuildCommandKey);

    #[async_trait]
    impl NativeCommandRoot for PointerDistinctNeutralRoot {
        type Terminal = SingletonRootSingleBuildCommandTerminal;

        fn initializes_request_revision(&self) -> bool {
            true
        }

        fn source_certificate<'a>(
            &self,
            terminal: &'a Self::Terminal,
        ) -> Option<&'a SourceCertificate> {
            terminal.source_certificate()
        }

        fn observations<'a>(
            &self,
            terminal: &'a Self::Terminal,
        ) -> Option<&'a PathObservationEpoch> {
            terminal.observations.as_ref()
        }

        async fn compute(
            &self,
            transaction: &mut dice::DiceTransaction,
        ) -> Result<
            slug_bzlmod_v2::SourcePreparationOutcome<Self::Terminal>,
            NativeDemandSessionError,
        > {
            let outcome = NativeCommandRoot::compute(&self.0, transaction).await?;
            Ok(outcome.map(|mut terminal| {
                terminal.observations = terminal.observations.map(|observations| {
                    PathObservationEpoch::from_shared(
                        observations.observations().iter().map(|(demand, result)| {
                            (demand.dupe(), Arc::new(result.as_ref().clone()))
                        }),
                    )
                    .unwrap()
                });
                terminal
            }))
        }
    }

    #[test]
    fn public_neutral_source_preserves_exact_arcs_and_mismatch_aborts() {
        let stable_parent = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/debug/incremental/slug-public-neutral-source");
        fs::create_dir_all(&stable_parent).unwrap();
        let workspace = tempfile::tempdir_in(stable_parent).unwrap();
        fs::write(
            workspace.path().join("MODULE.bazel"),
            "print('MODULE')\nmodule(name = 'neutral_source')\n",
        )
        .unwrap();
        fs::create_dir(workspace.path().join("pkg")).unwrap();
        fs::write(
            workspace.path().join("pkg/BUILD.bazel"),
            "print('BUILD')\nexports_files(['source.txt'])\n",
        )
        .unwrap();
        fs::write(workspace.path().join("pkg/source.txt"), b"source").unwrap();
        let audit = Arc::new(ExternalQueryActivationAudit::default());
        let runtime = test_runtime(workspace.path())
            .unwrap()
            .with_activation_audit(audit.dupe());
        let target = TargetPattern::parse("//pkg:source.txt").unwrap();
        let run = || {
            runtime.build_command_with_bzlmod_inputs(
                std::slice::from_ref(&target),
                BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                LockfileMode::Update,
                &[],
                None,
            )
        };
        let cold = run().unwrap();
        assert_eq!(accepted_output_text(&cold), ["MODULE", "BUILD"]);
        assert!(cold.terminal_for_test().as_ref().is_ok());
        let (observed, neutral, legacy) = audit.exact_build_root_counts();
        assert_eq!(observed, 0);
        assert!(neutral > 0);
        assert_eq!(legacy, 0);
        assert!(audit.take_configured_roots().is_empty());
        let accepted = accepted_native_snapshot(&runtime);
        let certificate = cold
            .terminal_for_test()
            .as_ref()
            .as_ref()
            .unwrap()
            .source_certificate()
            .unwrap();
        assert!(Arc::ptr_eq(
            certificate.observation(),
            accepted.path_observations.get(certificate.demand()).unwrap()
        ));

        let warm = run().unwrap();
        assert!(accepted_output_text(&warm).is_empty());
        let warm_epoch = accepted_native_snapshot(&runtime).path_observations;
        assert!(accepted.path_observations.observations().iter().all(
            |(demand, result)| Arc::ptr_eq(result, warm_epoch.get(demand).unwrap())
        ));
        let prior = accepted_native_snapshot(&runtime);
        runtime.native_demand_sessions.take_trace();
        let host = runtime.process_host.default_configuration_inputs().unwrap();
        let configuration = SlugConfiguration::default_target(&host).unwrap();
        let key = BuildCommandRootKey::new(
            NormalizedAbsolutePath::new(runtime.workspace.clone()).unwrap(),
            std::slice::from_ref(&target),
            ConfigurationKey::from_slug(configuration),
        )
        .unwrap();
        let error = runtime
            .drive_command(
                NativeDemandRequestInputBundle::normalized_initial(),
                PointerDistinctNeutralRoot(
                    SingletonRootSingleBuildCommandKey::new(key).unwrap(),
                ),
            )
            .unwrap_err();
        assert!(matches!(
            error,
            NativeDemandSessionError::ObservedTerminal(ObservedTerminalMismatch::ResultArc)
        ));
        assert_current_native_snapshot(&runtime, &prior);
        let trace = runtime.native_demand_sessions.take_trace();
        assert!(!trace.contains(&NativeDemandTestTrace::SelectedInjectionCommitted));
        assert!(!trace.contains(&NativeDemandTestTrace::AcceptedSnapshotReplaced));
        assert!(run().unwrap().terminal_for_test().as_ref().is_ok());
    }
