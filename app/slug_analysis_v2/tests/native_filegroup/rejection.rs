use super::*;

#[tokio::test]
async fn native_filegroup_rejects_unadmitted_own_attribute_capabilities() {
    let workspace = Workspace::new();
    let attributes = [
        ("transitive_configs", "[':single']"),
        ("deprecation", "'deprecated'"),
        ("tags", "['manual']"),
        ("testonly", "True"),
        ("features", "['custom']"),
        ("compatible_with", "[':single']"),
        ("restricted_to", "[':single']"),
        ("package_metadata", "[':single']"),
        ("aspect_hints", "[':single']"),
        ("licenses", "['notice']"),
        ("distribs", "['internal']"),
        ("target_compatible_with", "[':single']"),
        ("output_licenses", "['notice']"),
    ];
    for package in [&workspace.0, &workspace.0.join("dep")] {
        let path = package.join("BUILD.bazel");
        let mut build = fs::read_to_string(&path).unwrap();
        for (name, value) in attributes {
            build.push_str(&format!(
                "\nfilegroup(name='unsupported_{name}',srcs=['a.txt'],{name}={value})\n"
            ));
        }
        // Package defaults apply to all filegroups, including ones declared above.
        build.push_str("\nlicenses(['notice'])\n");
        fs::write(path, build).unwrap();
    }
    let dice = Dice::builder().build(DetectCycles::Enabled);
    for external in [false, true] {
        for (name, _) in attributes {
            let error = workspace
                .request(
                    &dice,
                    external,
                    &format!("unsupported_{name}"),
                    configuration(&[]),
                )
                .await
                .unwrap_err();
            assert!(
                error.contains("unsupported") && error.contains(name),
                "{name}: {error}"
            );
        }
        let result = workspace
            .request(&dice, external, "single", configuration(&[]))
            .await
            .unwrap();
        assert!(result.actions().is_empty());
        let source = source_paths(external, &["a.txt"]);
        assert_eq!(info(&result).file_artifacts().len(), 1);
        assert_eq!(info(&result).file_artifacts()[0].path().as_ref(), source[0]);
        assert_eq!(
            info(&result).executable.as_ref().unwrap().path().as_ref(),
            source[0]
        );
        assert_eq!(paths(&info(&result).data_runfiles.files), source);
    }
}

#[tokio::test]
async fn native_filegroup_rejects_nonfile_and_testonly_prerequisite_capabilities() {
    let workspace = Workspace::new();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    for external in [false, true] {
        for (name, diagnostic) in [
            ("from_package_group", "misplaced"),
            ("from_package_group_alias", "unsupported"),
        ] {
            let error = workspace
                .request(&dice, external, name, configuration(&[]))
                .await
                .unwrap_err();
            assert!(
                error.contains("package group") && error.contains(diagnostic),
                "{name}: {error}"
            );
        }
        for name in ["from_testonly_origin", "from_hidden_testonly"] {
            let error = workspace
                .request(&dice, external, name, configuration(&[]))
                .await
                .unwrap_err();
            assert!(
                error.contains("testonly") && error.contains("unsupported"),
                "{name}: {error}"
            );
        }
        // CommonPrerequisiteValidator checks the actual target, not the alias's
        // own testonly attribute; the ordinary source remains admissible.
        let result = workspace
            .request(&dice, external, "from_testonly_alias", configuration(&[]))
            .await
            .unwrap();
        assert_eq!(
            info(&result).file_artifacts()[0].path().as_ref(),
            source_paths(external, &["a.txt"])[0]
        );
    }
    // Main generated-label reconstruction is admitted; external generated labels
    // are still independently deferred, so use the genuine main owner here.
    let error = workspace
        .request(&dice, false, "from_testonly_generated", configuration(&[]))
        .await
        .unwrap_err();
    assert!(
        error.contains("testonly") && error.contains("unsupported"),
        "{error}"
    );
}
