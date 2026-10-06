use std::hash::Hash;
use std::hash::Hasher;

use slug_workspace_v2::NormalizedAbsolutePath;

use super::*;
use crate::ActionKind;
use crate::ActionRegistry;
use crate::ActionSpec;
use crate::CtxActions;
use crate::ReapiCommandProjection;
use crate::WorkspaceStatusArtifacts;
use crate::artifact_root::test_support as roots;

fn recipe(kind: BuildInfoTransformKind) -> BuildInfoCallbackRecipe {
    BuildInfoCallbackRecipe::new(
        kind,
        CanonicalLabel::parse(BuildInfoCallbackRecipe::SOURCE_LABEL).unwrap(),
        BuildInfoCallbackRecipe::SOURCE_DIGEST,
    )
    .unwrap()
}
fn template(name: &str) -> AnalysisArtifact {
    AnalysisArtifact::Source(
        CanonicalLabel::parse(&format!("@@bazel_tools//templates:{name}")).unwrap(),
    )
}
fn spec(
    kind: BuildInfoTransformKind,
    workspace: &str,
    name: &str,
    root: crate::DerivedArtifactRoot,
    output: &str,
) -> BuildInfoTransformSpec {
    let status = WorkspaceStatusArtifacts::new(NormalizedAbsolutePath::new(workspace).unwrap());
    let (status, output) = match kind {
        BuildInfoTransformKind::Info => (
            status.stable(),
            ActionOutput::new(root, output, ActionOutputKind::File),
        ),
        BuildInfoTransformKind::Version => {
            (status.volatile(), ActionOutput::new_constant(root, output))
        }
    };
    BuildInfoTransformSpec::new(status.dupe(), template(name), output, recipe(kind)).unwrap()
}

#[test]
fn build_info_specs_preserve_all_owned_inputs_and_metadata() {
    let a = spec(
        BuildInfoTransformKind::Version,
        "/workspace",
        "volatile",
        roots::root(),
        "pkg/v.h",
    );
    assert_eq!(
        a.inputs(),
        &[
            AnalysisArtifact::WorkspaceStatus(a.status().dupe()),
            template("volatile")
        ]
    );
    assert!(a.is_volatile() && a.execute_unconditionally());
    let status = WorkspaceStatusArtifacts::new(NormalizedAbsolutePath::new("/workspace").unwrap());
    for (template, count) in [(status.volatile(), 1), (status.stable(), 2)] {
        let input = AnalysisArtifact::WorkspaceStatus(template.dupe());
        let same_role = BuildInfoTransformSpec::new(
            status.volatile().dupe(),
            input.clone(),
            a.output().dupe(),
            recipe(BuildInfoTransformKind::Version),
        )
        .unwrap();
        assert_eq!(same_role.inputs().len(), count);
        assert_eq!(
            same_role.template(),
            &input,
            "role retention is independent of input deduplication"
        );
        assert_eq!(
            same_role.inputs()[0],
            AnalysisArtifact::WorkspaceStatus(status.volatile().dupe())
        );
    }
    assert_eq!(a.output().metadata(), ActionOutputMetadata::Constant);
    let action = ActionSpec::build_info_transform(a.dupe());
    assert_eq!(action.kind(), &ActionKind::BuildInfoTransform);
    assert_eq!(action.mnemonic(), "TranslateBuildInfo");
    assert_eq!(action.outputs(), &[a.output().dupe()]);
    assert_eq!(
        action.progress_message(),
        Some("Translating volatile BuildInfo file")
    );
    assert!(action.render_argv().is_err());
    assert!(ReapiCommandProjection::from_action(&action).is_err());
    for other in [
        spec(
            BuildInfoTransformKind::Info,
            "/workspace",
            "volatile",
            roots::root(),
            "pkg/v.h",
        ),
        spec(
            BuildInfoTransformKind::Version,
            "/other",
            "volatile",
            roots::root(),
            "pkg/v.h",
        ),
        spec(
            BuildInfoTransformKind::Version,
            "/workspace",
            "different",
            roots::root(),
            "pkg/v.h",
        ),
        spec(
            BuildInfoTransformKind::Version,
            "/workspace",
            "volatile",
            roots::root_for(b"B"),
            "pkg/v.h",
        ),
        spec(
            BuildInfoTransformKind::Version,
            "/workspace",
            "volatile",
            roots::root(),
            "pkg/other.h",
        ),
    ] {
        assert_ne!(a, other);
    }
    assert_eq!(
        a,
        spec(
            BuildInfoTransformKind::Version,
            "/workspace",
            "volatile",
            roots::root(),
            "pkg/v.h"
        )
    );
    assert!(
        BuildInfoCallbackRecipe::new(
            BuildInfoTransformKind::Info,
            CanonicalLabel::parse("@@foreign//tools/build_defs/build_info:bazel_cc_build_info.bzl")
                .unwrap(),
            BuildInfoCallbackRecipe::SOURCE_DIGEST
        )
        .is_err()
    );
    assert!(
        BuildInfoCallbackRecipe::new(
            BuildInfoTransformKind::Info,
            CanonicalLabel::parse(BuildInfoCallbackRecipe::SOURCE_LABEL).unwrap(),
            [0; 32]
        )
        .is_err()
    );
}

#[test]
fn constant_metadata_is_orthogonal_and_cannot_enter_other_action_families() {
    let ordinary = ActionOutput::new(roots::root(), "out", ActionOutputKind::File);
    let constant = ActionOutput::new_constant(roots::root(), "out");
    assert_eq!(ordinary.path(), constant.path());
    assert_eq!(ordinary.kind(), constant.kind());
    assert_ne!(ordinary, constant);
    assert_ne!(ordinary.cmp(&constant), std::cmp::Ordering::Equal);
    let hash = |output: &ActionOutput| {
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        output.hash(&mut hash);
        hash.finish()
    };
    assert_ne!(hash(&ordinary), hash(&constant));
    let status = WorkspaceStatusArtifacts::new(NormalizedAbsolutePath::new("/workspace").unwrap());
    for (status, output, kind) in [
        (
            status.volatile(),
            ordinary.dupe(),
            BuildInfoTransformKind::Version,
        ),
        (
            status.stable(),
            constant.dupe(),
            BuildInfoTransformKind::Info,
        ),
        (
            status.stable(),
            constant.dupe(),
            BuildInfoTransformKind::Version,
        ),
        (
            status.stable(),
            ActionOutput::new(roots::root(), "dir", ActionOutputKind::Directory),
            BuildInfoTransformKind::Info,
        ),
        (
            status.stable(),
            ActionOutput::new(roots::root(), "../escape", ActionOutputKind::File),
            BuildInfoTransformKind::Info,
        ),
    ] {
        assert!(
            BuildInfoTransformSpec::new(status.dupe(), template("t"), output, recipe(kind))
                .is_err()
        );
    }
    for action in [
        ActionSpec::new(
            ActionKind::BuildInfoTransform,
            "TranslateBuildInfo",
            vec![ordinary],
        ),
        ActionSpec::new(
            ActionKind::Write {
                content: "fake".into(),
                is_executable: false,
            },
            "FileWrite",
            vec![constant],
        ),
    ] {
        assert!(ActionRegistry::new().register(action.clone()).is_err());
        assert!(ReapiCommandProjection::from_action(&action).is_err());
    }
}

#[test]
fn build_info_registration_is_ordered_atomic_and_conflicts_across_metadata() {
    let mut actions = CtxActions::new(roots::root());
    let volatile = spec(
        BuildInfoTransformKind::Version,
        "/workspace",
        "v",
        roots::root(),
        "pkg/v.h",
    );
    let stable = spec(
        BuildInfoTransformKind::Info,
        "/workspace",
        "i",
        roots::root(),
        "pkg/i.h",
    );
    assert_eq!(
        actions
            .register_build_info_transform(volatile.dupe())
            .unwrap(),
        0
    );
    assert_eq!(
        actions
            .register_build_info_transform(stable.dupe())
            .unwrap(),
        1
    );
    let before = actions.clone();
    assert!(
        actions
            .write(
                ActionOutput::new(roots::root(), "pkg/v.h", ActionOutputKind::File),
                "collision",
                false
            )
            .is_err()
    );
    assert_eq!(before, actions);
    assert!(actions.register_build_info_transform(volatile).is_err());
    assert_eq!(before, actions);
    let mut registry = ActionRegistry::new();
    let stable = ActionSpec::build_info_transform(stable);
    assert!(registry.register_batch([stable.clone(), stable]).is_err());
    assert!(registry.actions().is_empty());
    let malformed = ActionSpec::new(
        ActionKind::Write {
            content: "x".into(),
            is_executable: false,
        },
        "FileWrite",
        vec![ActionOutput::new_constant(roots::root(), "bad")],
    );
    assert!(actions.replace_actions(vec![malformed]).is_err());
    assert_eq!(before, actions);
}

#[test]
fn build_info_callback_render_preserves_defaults_order_and_errors() {
    use BuildInfoTransformKind::Info;
    use BuildInfoTransformKind::Version;
    use slug_configuration_v2::CanonicalStringMap;

    // Whole literal expectations, independent of the renderer and replacement
    // helper. Map order must not choose the authenticated callback's pass order.
    let cases: &[(BuildInfoTransformKind, &[(&str, &str)], &str, &str)] = &[
        (
            Info,
            &[],
            "#define BUILD_EMBED_LABEL \"{BUILD_EMBED_LABEL}\"\n#define BUILD_HOST \"{BUILD_HOST}\"\n#define BUILD_USER \"{BUILD_USER}\"\n",
            "#define BUILD_EMBED_LABEL \"\"\n#define BUILD_HOST \"hostname\"\n#define BUILD_USER \"username\"\n",
        ),
        (
            Version,
            &[("BUILD_TIMESTAMP", "123")],
            "#define BUILD_SCM_REVISION \"{BUILD_SCM_REVISION}\"\n#define BUILD_SCM_STATUS \"{BUILD_SCM_STATUS}\"\n#define BUILD_TIMESTAMP {BUILD_TIMESTAMP}\n",
            "#define BUILD_SCM_REVISION \"0\"\n#define BUILD_SCM_STATUS \"\"\n#define BUILD_TIMESTAMP 123\n",
        ),
        (
            Info,
            &[
                ("BUILD_USER", ""),
                ("BUILD_HOST", ""),
                ("BUILD_EMBED_LABEL", ""),
                ("OTHER", "ignored"),
            ],
            "{BUILD_EMBED_LABEL}{BUILD_HOST}{BUILD_USER}|{OTHER}|{BUILD_TIMESTAMP}",
            "|{OTHER}|{BUILD_TIMESTAMP}",
        ),
        (
            Version,
            &[
                ("BUILD_TIMESTAMP", ""),
                ("BUILD_SCM_REVISION", ""),
                ("BUILD_SCM_STATUS", ""),
            ],
            "{BUILD_SCM_REVISION}{BUILD_SCM_STATUS}{BUILD_TIMESTAMP}|{BUILD_HOST}",
            "|{BUILD_HOST}",
        ),
        (
            Info,
            &[
                ("BUILD_USER", "u{BUILD_EMBED_LABEL}{BUILD_USER}"),
                ("BUILD_HOST", "{BUILD_USER}"),
                ("BUILD_EMBED_LABEL", "{BUILD_HOST}"),
            ],
            "{BUILD_EMBED_LABEL}|{BUILD_HOST}|{BUILD_USER}|{BUILD_EMBED_LABEL}",
            "u{BUILD_EMBED_LABEL}{BUILD_USER}|u{BUILD_EMBED_LABEL}{BUILD_USER}|u{BUILD_EMBED_LABEL}{BUILD_USER}|u{BUILD_EMBED_LABEL}{BUILD_USER}",
        ),
        (
            Version,
            &[
                ("BUILD_TIMESTAMP", "t{BUILD_SCM_REVISION}{BUILD_TIMESTAMP}"),
                ("BUILD_SCM_STATUS", "{BUILD_TIMESTAMP}"),
                ("BUILD_SCM_REVISION", "{BUILD_SCM_STATUS}"),
            ],
            "{BUILD_SCM_REVISION}|{BUILD_SCM_STATUS}|{BUILD_TIMESTAMP}|{BUILD_SCM_REVISION}",
            "t{BUILD_SCM_REVISION}{BUILD_TIMESTAMP}|t{BUILD_SCM_REVISION}{BUILD_TIMESTAMP}|t{BUILD_SCM_REVISION}{BUILD_TIMESTAMP}|t{BUILD_SCM_REVISION}{BUILD_TIMESTAMP}",
        ),
        (
            Info,
            &[("BUILD_EMBED_LABEL", "{BUILD_"), ("BUILD_HOST", "later")],
            "{BUILD_EMBED_LABEL}HOST}|{BUILD_EMBED_LABEL}EMBED_LABEL}",
            "later|{BUILD_EMBED_LABEL}",
        ),
        (
            Version,
            &[
                ("BUILD_SCM_REVISION", "{BUILD_"),
                ("BUILD_SCM_STATUS", "later"),
                ("BUILD_TIMESTAMP", "present"),
            ],
            "{BUILD_SCM_REVISION}SCM_STATUS}|{BUILD_SCM_REVISION}SCM_REVISION}",
            "later|{BUILD_SCM_REVISION}",
        ),
        (
            Info,
            &[
                ("BUILD_EMBED_LABEL", "{BUILD_EMBED_LABEL}"),
                ("BUILD_HOST", "{BUILD_HOST}"),
                ("BUILD_USER", "{BUILD_USER}"),
            ],
            "{BUILD_EMBED_LABEL}{BUILD_EMBED_LABEL}|{BUILD_HOST}|{BUILD_USER}",
            "{BUILD_EMBED_LABEL}{BUILD_EMBED_LABEL}|{BUILD_HOST}|{BUILD_USER}",
        ),
        (
            Version,
            &[
                ("BUILD_TIMESTAMP", "{BUILD_TIMESTAMP}"),
                ("BUILD_SCM_REVISION", "{BUILD_SCM_REVISION}"),
                ("BUILD_SCM_STATUS", "{BUILD_SCM_STATUS}"),
            ],
            "{BUILD_SCM_REVISION}|{BUILD_SCM_STATUS}|{BUILD_TIMESTAMP}{BUILD_TIMESTAMP}",
            "{BUILD_SCM_REVISION}|{BUILD_SCM_STATUS}|{BUILD_TIMESTAMP}{BUILD_TIMESTAMP}",
        ),
        (Info, &[], "plain {UNKNOWN}\n", "plain {UNKNOWN}\n"),
        (Info, &[], "", ""),
        (Version, &[("BUILD_TIMESTAMP", "")], "", ""),
        (
            Version,
            &[("BUILD_TIMESTAMP", " not-a-number \r\n")],
            "T={BUILD_TIMESTAMP};T={BUILD_TIMESTAMP}",
            "T= not-a-number \r\n;T= not-a-number \r\n",
        ),
    ];
    for &(kind, pairs, template, expected) in cases {
        let callback = recipe(kind);
        let status = CanonicalStringMap::from_pairs(pairs.iter().copied());
        let before = status.clone();
        let callback_before = callback.clone();
        assert_eq!(
            callback.render_template(&status, template).unwrap(),
            expected
        );
        assert_eq!(
            callback.render_template(&status, template).unwrap(),
            expected
        );
        assert_eq!(status, before);
        assert_eq!(callback, callback_before);
    }
    let callback = recipe(Version);
    let status = CanonicalStringMap::from_pairs([("BUILD_SCM_REVISION", "9"), ("UNKNOWN", "x")]);
    for template in ["{BUILD_TIMESTAMP}", "", "no recognized placeholder"] {
        assert_eq!(
            callback.render_template(&status, template),
            Err(
                "timestamp information is missing from workspace status file, BUILD_TIMESTAMP key not found."
            )
        );
    }
}

#[test]
fn build_info_callback_render_outputs_outlive_borrowed_unicode_inputs() {
    use BuildInfoTransformKind::Info;
    use BuildInfoTransformKind::Version;
    use slug_configuration_v2::CanonicalStringMap;

    let (info, version, unchanged) = {
        let info_recipe = recipe(Info);
        let version_recipe = recipe(Version);
        let status = CanonicalStringMap::from_pairs([
            ("BUILD_EMBED_LABEL", "  é😀\"\\\r\n\0  "),
            ("BUILD_HOST", "主机"),
            ("BUILD_USER", ""),
            ("BUILD_SCM_REVISION", "rév😀"),
            ("BUILD_SCM_STATUS", "\"\\\r\n\0"),
            ("BUILD_TIMESTAMP", " not numeric 😀 "),
        ]);
        let template =
            "前[{BUILD_EMBED_LABEL}][{BUILD_EMBED_LABEL}]|{BUILD_HOST}|{BUILD_USER}|{OTHER}\n"
                .to_owned();
        let version_template =
            "前{BUILD_SCM_REVISION}|{BUILD_SCM_STATUS}|{BUILD_TIMESTAMP}\n".to_owned();
        let unchanged_template = "unchanged é😀\0\r\n{OTHER}".to_owned();
        let original = (
            status.clone(),
            template.clone(),
            version_template.clone(),
            unchanged_template.clone(),
        );
        let info = info_recipe.render_template(&status, &template).unwrap();
        let version = version_recipe
            .render_template(&status, &version_template)
            .unwrap();
        let unchanged = info_recipe
            .render_template(&status, &unchanged_template)
            .unwrap();
        assert_eq!(
            info,
            "前[  é😀\"\\\r\n\0  ][  é😀\"\\\r\n\0  ]|主机||{OTHER}\n"
        );
        assert_eq!(version, "前rév😀|\"\\\r\n\0| not numeric 😀 \n");
        assert_eq!(unchanged, "unchanged é😀\0\r\n{OTHER}");
        assert_eq!(
            info_recipe.render_template(&status, &template).unwrap(),
            info
        );
        assert_eq!(
            version_recipe
                .render_template(&status, &version_template)
                .unwrap(),
            version
        );
        assert_eq!(
            info_recipe.source().to_string(),
            BuildInfoCallbackRecipe::SOURCE_LABEL
        );
        assert_eq!(
            version_recipe.digest(),
            &BuildInfoCallbackRecipe::SOURCE_DIGEST
        );
        assert_eq!(
            (&status, &template, &version_template, &unchanged_template),
            (&original.0, &original.1, &original.2, &original.3)
        );
        // A value longer than its key exercises growing passes; the empty USER
        // above shrinks. Neither observation implies an allocation bound.
        let growing = CanonicalStringMap::from_pairs([("BUILD_HOST", "界😀".repeat(64))]);
        let grown = info_recipe
            .render_template(&growing, "{BUILD_HOST}{BUILD_HOST}")
            .unwrap();
        assert_eq!(grown, "界😀".repeat(128));
        drop(growing);
        drop(grown);
        drop(original);
        drop(status);
        drop(template);
        drop(version_template);
        drop(unchanged_template);
        drop(info_recipe);
        drop(version_recipe);
        (info, version, unchanged)
    };
    // Every recipe, input and snapshot owner has now been released.
    assert_eq!(
        info.as_bytes(),
        "前[  é😀\"\\\r\n\0  ][  é😀\"\\\r\n\0  ]|主机||{OTHER}\n".as_bytes()
    );
    assert_eq!(
        version.as_bytes(),
        "前rév😀|\"\\\r\n\0| not numeric 😀 \n".as_bytes()
    );
    assert_eq!(
        unchanged.as_bytes(),
        "unchanged é😀\0\r\n{OTHER}".as_bytes()
    );
}

#[test]
fn build_info_supplied_bytes_preserve_full_headers_raw_values_and_errors() {
    use BuildInfoTransformKind::Info;
    use BuildInfoTransformKind::Version;

    let info_header = b"#define BUILD_EMBED_LABEL \"{BUILD_EMBED_LABEL}\"\n#define BUILD_HOST \"{BUILD_HOST}\"\n#define BUILD_USER \"{BUILD_USER}\"\n";
    let version_header = b"#define BUILD_SCM_REVISION \"{BUILD_SCM_REVISION}\"\n#define BUILD_SCM_STATUS \"{BUILD_SCM_STATUS}\"\n#define BUILD_TIMESTAMP {BUILD_TIMESTAMP}\n";
    let cases: &[(BuildInfoTransformKind, &[u8], &[u8], &[u8])] = &[
        (Info, b"", info_header, b"#define BUILD_EMBED_LABEL \"\"\n#define BUILD_HOST \"hostname\"\n#define BUILD_USER \"username\"\n"),
        (Version, b"BUILD_TIMESTAMP 123\n", version_header, b"#define BUILD_SCM_REVISION \"0\"\n#define BUILD_SCM_STATUS \"\"\n#define BUILD_TIMESTAMP 123\n"),
        (Info, b"BUILD_EMBED_LABEL caf\xE9\nBUILD_HOST \xC3\xA9\nBUILD_USER \xFF\x80\0\r\n", info_header, b"#define BUILD_EMBED_LABEL \"caf\xE9\"\n#define BUILD_HOST \"\xC3\xA9\"\n#define BUILD_USER \"\xFF\x80\0\r\"\n"),
        (Version, b"BUILD_SCM_REVISION r\xE9\nBUILD_SCM_STATUS \xFF\0\r\nBUILD_TIMESTAMP first\nBUILD_TIMESTAMP  not numeric \r\nBUILD_TIMESTAMP\n", version_header, b"#define BUILD_SCM_REVISION \"r\xE9\"\n#define BUILD_SCM_STATUS \"\xFF\0\r\"\n#define BUILD_TIMESTAMP  not numeric \r\n"),
        (Info, b"BUILD_EMBED_LABEL old\nBUILD_EMBED_LABEL \nBUILD_EMBED_LABEL\nBUILD_HOST \nBUILD_USER \n", b"[{BUILD_EMBED_LABEL}][{BUILD_HOST}][{BUILD_USER}]\xFE\0\r", b"[][][]\xFE\0\r"),
        (Version, b"BUILD_TIMESTAMP \n", version_header, b"#define BUILD_SCM_REVISION \"0\"\n#define BUILD_SCM_STATUS \"\"\n#define BUILD_TIMESTAMP \n"),
        (Version, b"BUILD_TIMESTAMP \n", b"", b""),
        (Info, b"", b"", b""),
    ];
    for &(kind, status, template, expected) in cases {
        let callback = recipe(kind);
        let before = callback.clone();
        let status = status.to_vec();
        let template = template.to_vec();
        let inputs = (status.clone(), template.clone());
        for _ in 0..2 {
            assert_eq!(
                callback.render_supplied_bytes(&status, &template).unwrap(),
                expected
            );
        }
        assert_eq!((&status, &template), (&inputs.0, &inputs.1));
        assert_eq!(callback, before);
    }
    let callback = recipe(Version);
    for status in [b"".as_slice(), b"BUILD_TIMESTAMP\nBUILD_TIMESTAMP\t123\n"] {
        for template in [
            b"{BUILD_TIMESTAMP}".as_slice(),
            b"",
            b"no recognized key\xFF\0\r",
        ] {
            assert_eq!(
                callback.render_supplied_bytes(status, template),
                Err(
                    "timestamp information is missing from workspace status file, BUILD_TIMESTAMP key not found."
                )
            );
        }
    }
}

#[test]
fn build_info_supplied_bytes_keep_ordered_literal_and_cross_boundary_passes() {
    use BuildInfoTransformKind::Info;
    use BuildInfoTransformKind::Version;

    let cases: &[(BuildInfoTransformKind, &[u8], &[u8], &[u8])] = &[
        (Info, b"BUILD_EMBED_LABEL {BUILD_HOST}\nBUILD_HOST {BUILD_USER}\nBUILD_USER \xE9{BUILD_EMBED_LABEL}{BUILD_USER}\n", b"{BUILD_EMBED_LABEL}|{BUILD_HOST}|{BUILD_USER}", b"\xE9{BUILD_EMBED_LABEL}{BUILD_USER}|\xE9{BUILD_EMBED_LABEL}{BUILD_USER}|\xE9{BUILD_EMBED_LABEL}{BUILD_USER}"),
        (Version, b"BUILD_SCM_REVISION {BUILD_SCM_STATUS}\nBUILD_SCM_STATUS {BUILD_TIMESTAMP}\nBUILD_TIMESTAMP \xFF{BUILD_SCM_REVISION}{BUILD_TIMESTAMP}\n", b"{BUILD_SCM_REVISION}|{BUILD_SCM_STATUS}|{BUILD_TIMESTAMP}", b"\xFF{BUILD_SCM_REVISION}{BUILD_TIMESTAMP}|\xFF{BUILD_SCM_REVISION}{BUILD_TIMESTAMP}|\xFF{BUILD_SCM_REVISION}{BUILD_TIMESTAMP}"),
        (Info, b"BUILD_EMBED_LABEL {BUILD_\nBUILD_HOST later\x80\n", b"{BUILD_EMBED_LABEL}HOST}|{BUILD_EMBED_LABEL}EMBED_LABEL}", b"later\x80|{BUILD_EMBED_LABEL}"),
        (Version, b"BUILD_SCM_REVISION {BUILD_\nBUILD_SCM_STATUS later\xFE\nBUILD_TIMESTAMP \n", b"{BUILD_SCM_REVISION}SCM_STATUS}|{BUILD_SCM_REVISION}SCM_REVISION}", b"later\xFE|{BUILD_SCM_REVISION}"),
        (Info, b"BUILD_EMBED_LABEL {BUILD_EMBED_LABEL}\nBUILD_HOST {BUILD_HOST}\nBUILD_USER {BUILD_USER}\n", b"{BUILD_EMBED_LABEL}{BUILD_EMBED_LABEL}|{BUILD_HOST}|{BUILD_USER}|{OTHER}", b"{BUILD_EMBED_LABEL}{BUILD_EMBED_LABEL}|{BUILD_HOST}|{BUILD_USER}|{OTHER}"),
        (Version, b"BUILD_SCM_REVISION {BUILD_SCM_REVISION}\nBUILD_SCM_STATUS {BUILD_SCM_STATUS}\nBUILD_TIMESTAMP {BUILD_TIMESTAMP}\n", b"{BUILD_SCM_REVISION}|{BUILD_SCM_STATUS}|{BUILD_TIMESTAMP}{BUILD_TIMESTAMP}", b"{BUILD_SCM_REVISION}|{BUILD_SCM_STATUS}|{BUILD_TIMESTAMP}{BUILD_TIMESTAMP}"),
    ];
    for &(kind, status, template, expected) in cases {
        let callback = recipe(kind);
        assert_eq!(
            callback.render_supplied_bytes(status, template).unwrap(),
            expected
        );
        assert_eq!(
            callback.render_supplied_bytes(status, template).unwrap(),
            expected
        );
    }
}

#[test]
fn build_info_supplied_bytes_roundtrip_all_bytes_and_outlive_input_owners() {
    use BuildInfoTransformKind::Info;
    use BuildInfoTransformKind::Version;

    let (matched, unchanged_info, unchanged_version) = {
        let info = recipe(Info);
        let version = recipe(Version);
        let mut status = b"BUILD_TIMESTAMP 1\nBUILD_HOST ".to_vec();
        status.extend(std::iter::repeat_n(0xE9, 512));
        status.extend_from_slice(b"\0\r\n");
        let template = b"\xFF[{BUILD_HOST}][{BUILD_HOST}]\0\r".to_vec();
        // This byte sequence contains no recognized placeholder, and includes
        // valid and invalid UTF8, raw delimiters and all control characters.
        let all_bytes = (0..=255u8).collect::<Vec<_>>();
        let snapshots = (
            status.clone(),
            template.clone(),
            all_bytes.clone(),
            info.clone(),
            version.clone(),
        );
        let matched = info.render_supplied_bytes(&status, &template).unwrap();
        let unchanged_info = info.render_supplied_bytes(&status, &all_bytes).unwrap();
        let unchanged_version = version.render_supplied_bytes(&status, &all_bytes).unwrap();
        assert_eq!(unchanged_info, all_bytes);
        assert_eq!(unchanged_version, all_bytes);
        assert_eq!(
            info.render_supplied_bytes(&status, &template).unwrap(),
            matched
        );
        assert_eq!(
            (&status, &template, &all_bytes, &info, &version),
            (
                &snapshots.0,
                &snapshots.1,
                &snapshots.2,
                &snapshots.3,
                &snapshots.4
            )
        );
        drop(snapshots);
        drop(status);
        drop(template);
        drop(all_bytes);
        drop(info);
        drop(version);
        (matched, unchanged_info, unchanged_version)
    };
    // Only the returned byte Vecs survive the supplied input/recipe owners.
    let mut expected = b"\xFF[".to_vec();
    expected.extend(std::iter::repeat_n(0xE9, 512));
    expected.extend_from_slice(b"\0\r][");
    expected.extend(std::iter::repeat_n(0xE9, 512));
    expected.extend_from_slice(b"\0\r]\0\r");
    assert_eq!(matched, expected);
    assert_eq!(unchanged_info, (0..=255u8).collect::<Vec<_>>());
    assert_eq!(unchanged_version, (0..=255u8).collect::<Vec<_>>());
}
