use super::*;

#[tokio::test]
async fn native_files_and_prerequisite_providers_restore_in_one_dice() {
    let workspace = Workspace::new();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let config = configuration(&[]);
    for external in [false, true] {
        let package = if external {
            workspace.0.join("dep")
        } else {
            workspace.0.clone()
        };
        let build_path = package.join("BUILD.bazel");
        let original = fs::read_to_string(&build_path).unwrap();
        let first = workspace
            .request(&dice, external, "runfiles", config.clone())
            .await
            .unwrap();
        fs::write(
            &build_path,
            original.replace("default_files=['default.txt']", "default_files=['d.txt']"),
        )
        .unwrap();
        let changed = workspace
            .request(&dice, external, "runfiles", config.clone())
            .await
            .unwrap();
        assert_ne!(first, changed);
        assert_eq!(
            paths(&info(&changed).default_runfiles.files),
            source_paths(external, &["d.txt"])
        );
        fs::write(&build_path, &original).unwrap();
        assert_eq!(
            first,
            workspace
                .request(&dice, external, "runfiles", config.clone())
                .await
                .unwrap()
        );

        let single = workspace
            .request(&dice, external, "single", config.clone())
            .await
            .unwrap();
        fs::write(
            &build_path,
            original.replace(
                "filegroup(name='single',srcs=['a.txt'])",
                "filegroup(name='single',srcs=['b.txt'])",
            ),
        )
        .unwrap();
        let changed = workspace
            .request(&dice, external, "single", config.clone())
            .await
            .unwrap();
        assert_ne!(single, changed);
        assert_eq!(
            info(&changed).executable.as_ref().unwrap().path().as_ref(),
            source_paths(external, &["b.txt"])[0]
        );
        fs::write(&build_path, &original).unwrap();
        assert_eq!(
            single,
            workspace
                .request(&dice, external, "single", config.clone())
                .await
                .unwrap()
        );

        let source = package.join("a.txt");
        fs::remove_file(&source).unwrap();
        assert!(
            workspace
                .request(&dice, external, "single", config.clone())
                .await
                .is_err()
        );
        fs::write(&source, "authored source; not executable\n").unwrap();
        assert_eq!(
            single,
            workspace
                .request(&dice, external, "single", config.clone())
                .await
                .unwrap()
        );
    }
}

#[tokio::test]
async fn native_filegroup_inherited_notice_restores_after_other_package_license() {
    let workspace = Workspace::new();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let config = configuration(&[]);
    for external in [false, true] {
        let package = if external {
            workspace.0.join("dep")
        } else {
            workspace.0.clone()
        };
        let build_path = package.join("BUILD.bazel");
        let original = fs::read_to_string(&build_path).unwrap();
        let notice = format!("{original}\nlicenses(['notice'])\n");
        fs::write(&build_path, &notice).unwrap();
        let first = workspace
            .request(&dice, external, "single", config.clone())
            .await
            .unwrap();
        assert!(first.actions().is_empty());
        assert_eq!(
            info(&first).file_artifacts()[0].path().as_ref(),
            source_paths(external, &["a.txt"])[0]
        );

        fs::write(
            &build_path,
            notice.replace("licenses(['notice'])", "licenses(['restricted'])"),
        )
        .unwrap();
        let error = workspace
            .request(&dice, external, "single", config.clone())
            .await
            .unwrap_err();
        assert!(
            error.contains("native filegroup nondefault licenses is unsupported"),
            "{error}"
        );

        fs::write(&build_path, &notice).unwrap();
        assert_eq!(
            first,
            workspace
                .request(&dice, external, "single", config.clone())
                .await
                .unwrap()
        );
    }
}
