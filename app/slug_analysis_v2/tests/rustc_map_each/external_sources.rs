use super::*;

#[tokio::test]
async fn external_source_roots_route_render_and_restore() {
    let workspace = workspace();
    for repo in ["rules_rust", "rules_cc"] {
        fs::write(workspace.join(repo).join("lib.rs"), "pub fn probe() {}\n").unwrap();
    }
    fs::write(
        workspace.join("rules_cc/BUILD.bazel"),
        "exports_files(['lib.rs'])\n",
    )
    .unwrap();
    let build_path = workspace.join("rules_rust/BUILD.bazel");
    for observed in [false, true] {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut first = None;
        let mut second = None;
        for (index, repo) in ["rules_rust+", "rules_cc+", "rules_rust+"]
            .into_iter()
            .enumerate()
        {
            let exec = format!("external/{repo}/lib.rs");
            let short = format!("../{repo}/lib.rs");
            let directory = format!("external/{repo}");
            let expected = [&exec, &short, &directory, &directory];
            fs::write(&build_path, format!(
                "load(':proof.bzl', 'subject')\nexports_files(['lib.rs'])\nsubject(name='subject', src='@@{repo}//:lib.rs', expected_source_paths={expected:?})\n"
            )).unwrap();
            let result = request_with_epoch(&dice, &workspace, root_epoch(&workspace), observed)
                .await
                .unwrap();
            let spawn = result.actions()[0].spawn_spec().unwrap();
            assert!(
                spawn
                    .render_argv()
                    .windows(3)
                    .any(|args| args == ["--", "rustc", exec.as_str()])
            );
            let RetainedCommandLineSegment::ArgsSnapshot(args) =
                &spawn.command_line().segments()[2]
            else {
                panic!()
            };
            assert_eq!(args.recipe().render()[0], exec);
            assert!(
                args.recipe()
                    .render_write_content()
                    .starts_with(&format!("{exec}\n"))
            );
            let expanded = spawn.expand_forced_param_files().unwrap();
            assert!(
                expanded
                    .param_files()
                    .iter()
                    .any(|file| file.bytes().starts_with(format!("{exec}\n").as_bytes()))
            );
            let mut inputs = Vec::new();
            spawn
                .inputs()
                .visit(|artifact| inputs.push(artifact.clone()))
                .unwrap();
            assert_eq!(
                inputs,
                [slug_build_api_v2::AnalysisArtifact::Source(
                    CanonicalLabel::parse(&format!("@@{repo}//:lib.rs")).unwrap()
                )]
            );
            match index {
                0 => first = Some(result),
                1 => {
                    assert!(first.as_ref().unwrap() != &result);
                    second = Some(result);
                }
                2 => {
                    assert!(first.as_ref().unwrap() == &result);
                    assert!(second.as_ref().unwrap() != &result);
                }
                _ => unreachable!(),
            }
        }
        let source = workspace.join("rules_rust/lib.rs");
        fs::remove_file(&source).unwrap();
        // Main lib.rs is still present: resolving below workspace would wrongly succeed.
        assert!(workspace.join("lib.rs").is_file());
        let missing = request_with_epoch(
            &dice,
            &workspace,
            root_epoch_with_missing(&workspace, [source.clone()]),
            observed,
        )
        .await
        .unwrap_err();
        assert!(missing.contains("lib.rs"), "{missing}");
        assert!(
            missing.contains("not found") || missing.contains("does not exist"),
            "{missing}"
        );
        fs::create_dir(&source).unwrap();
        // The existing file-snapshot helper omits empty directories. Supply
        // the observed kind explicitly so this checks rejection, not Need.
        let epoch = root_epoch(&workspace);
        let directory = PathObservationDemand::new(
            PathObservationNamespace::Host,
            NormalizedAbsolutePath::new(source.clone()).unwrap(),
            PathObservationOperation::Lstat,
        );
        let epoch = PathObservationEpoch::from_shared(
            epoch
                .observations()
                .iter()
                .map(|(demand, result)| (demand.dupe(), result.dupe()))
                .chain(std::iter::once((
                    directory,
                    Arc::new(PathObservationResult::Lstat(PathOperationResult::Present(
                        PathLstat::new(PathNodeKind::Directory, 1, 1, 1, 1, 0o755),
                    ))),
                ))),
        )
        .unwrap();
        let wrong_kind = request_with_epoch(&dice, &workspace, epoch, observed)
            .await
            .unwrap_err();
        assert!(
            wrong_kind.contains("unsupported filesystem kind Directory"),
            "{wrong_kind}"
        );
        fs::remove_dir(&source).unwrap();
        fs::write(&source, "pub fn probe() {}\n").unwrap();
        assert!(
            first.unwrap()
                == request_with_epoch(&dice, &workspace, root_epoch(&workspace), observed)
                    .await
                    .unwrap()
        );
    }
    fs::remove_dir_all(workspace).unwrap();
}
