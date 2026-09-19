use slug_identity_v2::CanonicalLabel;

use super::*;
use crate::ActionOutput;
use crate::AnalysisConfiguredTargetKey;
use crate::AnalysisDepset;
use crate::AnalysisValue;
use crate::DepsetOrder;
use crate::FilesToRunProvider;
use crate::RetainedArgCall;
use crate::RetainedArgsDepset;
use crate::RetainedArgsRecipe;
use crate::RetainedArtifactInputs;
use crate::RetainedParamFileFormat;
use crate::RetainedVectorArg;
use crate::RetainedVectorOptions;
use crate::RetainedVectorSource;

fn source(label: &str) -> AnalysisArtifact {
    AnalysisArtifact::Source(CanonicalLabel::parse(label).unwrap())
}
fn generated(owner: &str, config: &[u8], path: &str, kind: ActionOutputKind) -> AnalysisArtifact {
    AnalysisArtifact::Derived {
        owner: AnalysisConfiguredTargetKey::new(CanonicalLabel::parse(owner).unwrap(), config),
        output: ActionOutput::new(path, kind),
    }
}
fn files(direct: Vec<AnalysisArtifact>, children: Vec<AnalysisDepset>) -> AnalysisDepset {
    AnalysisDepset::new(
        DepsetOrder::Default,
        direct.into_iter().map(AnalysisValue::artifact).collect(),
        children,
    )
    .unwrap()
}
fn inputs(depset: AnalysisDepset) -> ArtifactInputs {
    ArtifactInputs::new(vec![ArtifactInputSource::Depset(
        RetainedArtifactInputs::new(depset).unwrap(),
    )])
}
fn options() -> RetainedVectorOptions {
    RetainedVectorOptions {
        arg_name: None,
        format_each: None,
        before_each: None,
        join_with: None,
        format_joined: None,
        omit_if_empty: true,
        uniquify: false,
        expand_directories: true,
        terminate_with: None,
    }
}
fn call(recipe: RetainedCargoRunfilesArgs, options: RetainedVectorOptions) -> RetainedArgCall {
    RetainedArgCall::AddAll(RetainedVectorArg::new(
        RetainedVectorSource::CargoRunfiles(recipe),
        options,
    ))
}

#[test]
fn unexpanded_directory_depset_keeps_literal_path_and_configured_identity() {
    let make = |configuration: &[u8]| {
        files(
            Vec::new(),
            vec![files(
                vec![generated(
                    "@@//:tree",
                    configuration,
                    "pkg/tree",
                    ActionOutputKind::Directory,
                )],
                Vec::new(),
            )],
        )
    };
    let first = make(b"a");
    assert_eq!(
        RetainedArgsDepset::new(first.clone())
            .unwrap_err()
            .to_string(),
        "Args vector directory expansion is not supported"
    );
    let recipe = |depset| {
        let mut options = options();
        options.expand_directories = false;
        RetainedArgsRecipe::new(
            vec![RetainedArgCall::AddAll(RetainedVectorArg::new(
                RetainedVectorSource::Depset(RetainedArgsDepset::new_unexpanded(depset).unwrap()),
                options,
            ))],
            RetainedParamFileFormat::Multiline,
        )
    };
    let a = recipe(first);
    let b = recipe(make(b"b"));
    assert_eq!(a.render(), ["pkg/tree"]);
    assert_eq!(a.render(), b.render());
    assert_ne!(a, b);
    assert_eq!(a, recipe(make(b"a")));
    let labels = AnalysisDepset::new(
        DepsetOrder::Default,
        vec![AnalysisValue::label(
            CanonicalLabel::parse("@@//:label").unwrap(),
        )],
        Vec::new(),
    )
    .unwrap();
    assert!(RetainedArgsDepset::new_unexpanded(labels).is_err());
}

#[test]
fn cargo_mapping_preserves_depset_order_paths_and_exact_fake_identity() {
    let fake = generated("@@//:fake", b"a", "pkg/fake", ActionOutputKind::File);
    let same_path_other_owner = generated("@@//:other", b"a", "pkg/fake", ActionOutputKind::File);
    let same_path_other_config = generated("@@//:fake", b"b", "pkg/fake", ActionOutputKind::File);
    let leaf = files(vec![fake.clone(), source("@@//pkg:data")], vec![]);
    let left = files(vec![source("@@dep+//pkg:external")], vec![leaf.clone()]);
    let right = files(
        vec![
            generated(
                "@@dep+//:owner",
                b"a",
                "pkg/generated",
                ActionOutputKind::File,
            ),
            same_path_other_owner,
            same_path_other_config,
        ],
        vec![leaf],
    );
    let retained = inputs(files(vec![], vec![left, right]));
    let recipe =
        RetainedCargoRunfilesArgs::new(retained.clone(), fake.clone(), "workspace".into()).unwrap();
    assert_eq!(
        recipe.render(),
        [
            "pkg/data=workspace/pkg/data",
            "external/dep+/pkg/external=dep+/pkg/external",
            // Generated projections deliberately retain the existing Slug-native paths.
            "pkg/generated=workspace/pkg/generated",
            "pkg/fake=workspace/pkg/fake",
            "pkg/fake=workspace/pkg/fake",
        ]
    );
    let mut originals = Vec::new();
    retained
        .visit(|artifact| originals.push(artifact.clone()))
        .unwrap();
    assert_eq!(originals.len(), 6);
    assert_eq!(originals[0], fake); // Filtering affects arguments, never action inputs.
}

#[test]
fn cargo_capture_and_shared_graph_identity_survive_publication_and_restore() {
    let fake = source("@@//:fake");
    let leaf = || files(vec![source("@@dep+//pkg:external")], vec![]);
    let make = |a: AnalysisDepset, b: AnalysisDepset, fake: AnalysisArtifact, workspace: &str| {
        RetainedArgsRecipe::new(
            vec![
                call(
                    RetainedCargoRunfilesArgs::new(inputs(a), fake.clone(), workspace.into())
                        .unwrap(),
                    options(),
                ),
                call(
                    RetainedCargoRunfilesArgs::new(inputs(b), fake, workspace.into()).unwrap(),
                    options(),
                ),
            ],
            RetainedParamFileFormat::Multiline,
        )
    };
    let shared = leaf();
    let original = make(shared.clone(), shared, fake.clone(), "A");
    let shared = leaf();
    let restored = make(shared.clone(), shared, fake.clone(), "A");
    assert_eq!(original, restored);
    let split = make(leaf(), leaf(), fake.clone(), "A");
    assert_eq!(original.render(), split.render());
    assert_ne!(original, split); // Two equal but independently owned depsets.
    let shared = leaf();
    let workspace = make(shared.clone(), shared, fake, "B");
    assert_eq!(original.render(), workspace.render()); // External path ignores workspace.
    assert_ne!(original, workspace); // The captured value remains structural state.
    let shared = leaf();
    let changed_fake = make(shared.clone(), shared, source("@@other+//:fake"), "A");
    assert_eq!(original.render(), changed_fake.render());
    assert_ne!(original, changed_fake);
}

#[test]
fn cargo_vector_options_follow_mapping_and_nonregular_inputs_fail_closed() {
    let fake = source("@@//:fake");
    let data = source("@@//pkg:data");
    let recipe = RetainedCargoRunfilesArgs::new(
        ArtifactInputs::new(vec![
            ArtifactInputSource::Direct(fake.clone()),
            ArtifactInputSource::Direct(data.clone()),
            ArtifactInputSource::Direct(data),
        ]),
        fake.clone(),
        "_main".into(),
    )
    .unwrap();
    let mut selected = options();
    selected.arg_name = Some("--mappings".into());
    selected.format_each = Some("[%s]".into());
    selected.before_each = Some("--entry".into());
    selected.terminate_with = Some("--end".into());
    selected.uniquify = true;
    let args = RetainedArgsRecipe::new(
        vec![call(recipe, selected.clone())],
        RetainedParamFileFormat::Multiline,
    );
    assert_eq!(
        args.render(),
        [
            "--mappings",
            "--entry",
            "[pkg/data=_main/pkg/data]",
            "--end"
        ]
    );
    assert_eq!(
        args.render_write_content(),
        "--mappings\n--entry\n[pkg/data=_main/pkg/data]\n--end\n"
    );
    let empty = || {
        RetainedCargoRunfilesArgs::new(
            inputs(files(vec![fake.clone()], vec![])),
            fake.clone(),
            "_main".into(),
        )
        .unwrap()
    };
    assert!(
        RetainedArgsRecipe::new(
            vec![call(empty(), selected.clone())],
            RetainedParamFileFormat::Multiline
        )
        .render()
        .is_empty()
    );
    selected.omit_if_empty = false;
    assert_eq!(
        RetainedArgsRecipe::new(
            vec![call(empty(), selected)],
            RetainedParamFileFormat::Multiline
        )
        .render(),
        ["--mappings", "--end"]
    );
    for kind in [
        ActionOutputKind::Directory,
        ActionOutputKind::Symlink,
        ActionOutputKind::RunfilesTree,
    ] {
        let invalid = generated("@@//:owner", b"a", "invalid", kind);
        for source in [
            ArtifactInputSource::Direct(invalid.clone()),
            ArtifactInputSource::Depset(
                RetainedArtifactInputs::new(files(vec![invalid.clone()], vec![])).unwrap(),
            ),
        ] {
            assert!(
                RetainedCargoRunfilesArgs::new(
                    ArtifactInputs::new(vec![source]),
                    fake.clone(),
                    "_main".into()
                )
                .is_err()
            );
        }
        assert!(
            RetainedCargoRunfilesArgs::new(ArtifactInputs::new(vec![]), invalid, "_main".into())
                .is_err()
        );
    }
    assert!(
        RetainedCargoRunfilesArgs::new(
            ArtifactInputs::new(vec![ArtifactInputSource::FilesToRun(
                FilesToRunProvider::single_executable_without_support(fake.clone()),
            )]),
            fake,
            "_main".into()
        )
        .is_err()
    );
}
