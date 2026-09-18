use super::*;

fn source(label: &str) -> AnalysisArtifact {
    AnalysisArtifact::Source(CanonicalLabel::parse(label).unwrap())
}

#[test]
fn source_execution_and_short_paths_preserve_repository_identity() {
    for (label, exec, short, directory) in [
        ("@@//:lib.rs", "lib.rs", "lib.rs", "."),
        (
            "@@//pkg:src/lib.rs",
            "pkg/src/lib.rs",
            "pkg/src/lib.rs",
            "pkg/src",
        ),
        (
            "@@dep+//:lib.rs",
            "external/dep+/lib.rs",
            "../dep+/lib.rs",
            "external/dep+",
        ),
        (
            "@@dep+//pkg:src/lib.rs",
            "external/dep+/pkg/src/lib.rs",
            "../dep+/pkg/src/lib.rs",
            "external/dep+/pkg/src",
        ),
        (
            "@@other+//pkg:src/lib.rs",
            "external/other+/pkg/src/lib.rs",
            "../other+/pkg/src/lib.rs",
            "external/other+/pkg/src",
        ),
    ] {
        let artifact = source(label);
        assert_eq!(artifact.path(), exec);
        assert_eq!(artifact.short_path(), short);
        assert_eq!(artifact.dirname(), directory);
        assert_eq!(artifact.path().rsplit('/').next(), Some("lib.rs"));
    }
    let generated = derived_artifact("pkg/out", ActionOutputKind::File);
    assert_eq!(generated.path(), "pkg/out");
    assert_eq!(generated.short_path(), "pkg/out");
    assert_ne!(source("@@dep+//:lib.rs"), source("@@other+//:lib.rs"));
}

#[test]
fn source_args_executable_and_param_bytes_share_execution_paths() {
    let files = [
        source("@@//:lib.rs"),
        source("@@dep+//:lib.rs"),
        source("@@other+//:lib.rs"),
    ];
    let recipe = RetainedArgsRecipe::new(
        vec![RetainedArgCall::AddAll(RetainedVectorArg::new(
            RetainedVectorSource::Sequence(
                files
                    .iter()
                    .cloned()
                    .map(RetainedScalarValue::Artifact)
                    .collect(),
            ),
            RetainedVectorOptions {
                uniquify: true,
                ..default_vector_options()
            },
        ))],
        RetainedParamFileFormat::Multiline,
    );
    assert_eq!(
        recipe.render(),
        ["lib.rs", "external/dep+/lib.rs", "external/other+/lib.rs"]
    );
    let make = |tool: AnalysisArtifact| {
        SpawnSpec::new(
            RetainedSpawnInvocation::Executable(SpawnExecutable::Artifact(tool)),
            RetainedCommandLine::new(vec![RetainedCommandLineSegment::ArgsSnapshot(
                RetainedSpawnArgsSnapshot::new(
                    recipe.clone(),
                    Some(RetainedSpawnParamFilePolicy::new("@%s", true)),
                ),
            )]),
            ArtifactInputs::new(
                files
                    .iter()
                    .cloned()
                    .map(ArtifactInputSource::Direct)
                    .collect::<Vec<_>>(),
            ),
            ArtifactInputs::new(Vec::new()),
            vec![ActionOutput::new("out", ActionOutputKind::File)],
            None,
            RetainedActionEnvironment::default(),
            CanonicalStringMap::default(),
            "Rustc",
            None::<&str>,
        )
    };
    let first = make(source("@@dep+//:rustc"));
    assert_eq!(
        first.render_argv(),
        [
            "external/dep+/rustc",
            "lib.rs",
            "external/dep+/lib.rs",
            "external/other+/lib.rs"
        ]
    );
    let expanded = first.expand_forced_param_files().unwrap();
    assert_eq!(expanded.argv(), ["external/dep+/rustc", "@out-0.params"]);
    assert_eq!(
        expanded.param_files()[0].bytes(),
        b"lib.rs\nexternal/dep+/lib.rs\nexternal/other+/lib.rs\n"
    );
    assert_ne!(first, make(source("@@other+//:rustc")));
}
