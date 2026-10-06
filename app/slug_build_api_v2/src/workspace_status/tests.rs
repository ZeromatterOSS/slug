use super::*;
use crate::AnalysisArtifact;
use crate::AnalysisArtifactRoot;
use crate::AnalysisDepset;
use crate::AnalysisValue;
use crate::DepsetOrder;

#[test]
fn workspace_status_identity_metadata_and_projections_are_structural() {
    let pair = WorkspaceStatusArtifacts::new(NormalizedAbsolutePath::new("/workspace").unwrap());
    let equivalent =
        WorkspaceStatusArtifacts::new(NormalizedAbsolutePath::new("/workspace/child/..").unwrap());
    let other = WorkspaceStatusArtifacts::new(NormalizedAbsolutePath::new("/other").unwrap());
    assert_eq!(pair, equivalent);
    assert_ne!(pair, other);
    assert_eq!(pair.stable().root(), pair.volatile().root());
    assert_ne!(pair.stable(), pair.volatile());
    assert!(!pair.stable().is_constant_metadata());
    assert!(pair.volatile().is_constant_metadata());
    assert_eq!(pair.stable().kind(), WorkspaceStatusArtifactKind::Stable);
    assert_eq!(
        pair.volatile().kind(),
        WorkspaceStatusArtifactKind::Volatile
    );
    assert_eq!(
        pair.stable().root().workspace().as_path(),
        std::path::Path::new("/workspace")
    );
    for value in [pair.stable(), pair.volatile()] {
        let artifact = AnalysisArtifact::WorkspaceStatus(value.clone());
        assert_eq!(artifact.path(), value.path());
        assert_eq!(artifact.short_path(), value.short_path());
        assert_eq!(artifact.dirname(), "bazel-out");
        assert_eq!(
            artifact.root(),
            AnalysisArtifactRoot::WorkspaceStatus(value.root().clone())
        );
        assert_eq!(artifact.root().path(), "bazel-out");
    }
    assert_eq!(pair.stable().path(), other.stable().path());
    assert_ne!(pair.stable().root(), other.stable().root());
    let values = [
        pair.stable(),
        equivalent.stable(),
        pair.volatile(),
        other.stable(),
    ]
    .into_iter()
    .map(|v| AnalysisValue::artifact(AnalysisArtifact::WorkspaceStatus(v.clone())))
    .collect();
    let depset = AnalysisDepset::new(DepsetOrder::Default, values, vec![]).unwrap();
    assert_eq!(depset.to_list().len(), 3);
}

#[cfg(unix)]
#[test]
fn workspace_status_preserves_nonunicode_workspace_identity() {
    use std::os::unix::ffi::OsStringExt;
    let make = |byte| {
        WorkspaceStatusArtifacts::new(
            NormalizedAbsolutePath::new(std::path::PathBuf::from(std::ffi::OsString::from_vec(
                vec![b'/', byte],
            )))
            .unwrap(),
        )
    };
    assert_eq!(make(0xff), make(0xff));
    assert_ne!(make(0xff), make(0xfe));
    assert_eq!(make(0xff).stable().path(), make(0xfe).stable().path());
}

#[test]
fn workspace_status_runfiles_refuse_before_resolver() {
    let status = AnalysisArtifact::WorkspaceStatus(
        WorkspaceStatusArtifacts::new(NormalizedAbsolutePath::new("/workspace").unwrap())
            .stable()
            .clone(),
    );
    let mut runfiles = crate::RetainedRunfiles::empty();
    runfiles.files = AnalysisDepset::new(
        DepsetOrder::Default,
        vec![AnalysisValue::artifact(status.clone())],
        vec![],
    )
    .unwrap();
    // The support output slots are irrelevant to source-manifest projection;
    // status is deliberately present as a leaf in the ordinary runfiles set.
    let support = crate::RunfilesSupport {
        runfiles,
        tree: status.clone(),
        input_manifest: status,
        manifest: None,
        repo_mapping_manifest: None,
    };
    let layout = support.layout().unwrap();
    let result =
        layout.source_manifest_bytes::<String>(|_| panic!("must reject before resolution"));
    assert_eq!(
        result.unwrap_err(),
        crate::SourceManifestError::WorkspaceStatusUnsupported
    );
}

#[test]
fn supplied_status_latin1_decoder_maps_every_byte_without_utf8_interpretation() {
    for maximum in [127u8, 255] {
        let bytes = (0..=maximum).collect::<Vec<_>>();
        let decoded = decode_latin1(&bytes);
        assert_eq!(decoded.chars().count(), usize::from(maximum) + 1);
        for (index, character) in decoded.chars().enumerate() {
            assert_eq!(u32::from(character), index as u32);
        }
    }
    assert_eq!(decode_latin1(b""), "");
    assert_eq!(decode_latin1(b"\xC3\xA9"), "\u{00C3}\u{00A9}");
    assert_eq!(decode_latin1(b"\xFF\x80"), "\u{00FF}\u{0080}");
}

#[test]
fn supplied_status_values_preserve_literal_separators_duplicates_and_ownership() {
    // Full independently written sorted rows, not a second parser. LF/space
    // are delimiters here; the private decoder's all-byte test is separate.
    let cases: &[(&[u8], &[(&str, &str)])] = &[
        (b"", &[]),
        (b"no-separator\n\t\r\n", &[]),
        (b" ", &[("", "")]),
        (
            b"\nseparatorless\nA first\nA  last \r\nA\nEMPTY \n empty first\n empty last\nTAB\tKEY v\t x\nCR\r value\r\nNUL\0KEY value\0\nLAST end",
            &[
                ("", "empty last"),
                ("A", " last \r"),
                ("CR\r", "value\r"),
                ("EMPTY", ""),
                ("LAST", "end"),
                ("NUL\0KEY", "value\0"),
                ("TAB\tKEY", "v\t x"),
            ],
        ),
        (
            b"UTF8 \xC3\xA9\nBAD \xFF\xFE\n\xFF \x80\0\r\n",
            &[
                ("BAD", "\u{00FF}\u{00FE}"),
                ("UTF8", "\u{00C3}\u{00A9}"),
                ("\u{00FF}", "\u{0080}\0\r"),
            ],
        ),
    ];
    for &(bytes, expected) in cases {
        let input = bytes.to_vec();
        let before = input.clone();
        let values = WorkspaceStatusArtifacts::parse_supplied_values(&input);
        assert_eq!(values.iter().collect::<Vec<_>>(), expected);
        assert_eq!(
            WorkspaceStatusArtifacts::parse_supplied_values(&input),
            values
        );
        assert_eq!(input, before);
    }
    let retained = {
        let key = "LONG_KEY_".repeat(64);
        let mut input = key.as_bytes().to_vec();
        input.push(b' ');
        input.extend(std::iter::repeat_n(0xE9, 512));
        input.extend_from_slice(b"\0\r");
        let before = input.clone();
        let values = WorkspaceStatusArtifacts::parse_supplied_values(&input);
        let snapshot = values.clone();
        assert_eq!(
            values.get(&key),
            Some(format!("{}\0\r", "é".repeat(512)).as_str())
        );
        assert_eq!(input, before);
        assert_eq!(values, snapshot);
        drop(snapshot);
        drop(before);
        drop(input);
        drop(key);
        values
    };
    // Only the returned map owns the long decoded input fields now.
    assert_eq!(retained.iter().count(), 1);
    let (key, value) = retained.iter().next().unwrap();
    assert_eq!(key.as_bytes(), "LONG_KEY_".repeat(64).as_bytes());
    assert_eq!(
        value.as_bytes(),
        format!("{}\0\r", "é".repeat(512)).as_bytes()
    );
}

#[test]
fn supplied_status_values_compose_with_checked_build_info_recipes() {
    use slug_identity_v2::CanonicalLabel;

    use crate::BuildInfoCallbackRecipe;
    use crate::BuildInfoTransformKind;

    let checked = |kind| {
        BuildInfoCallbackRecipe::new(
            kind,
            CanonicalLabel::parse(BuildInfoCallbackRecipe::SOURCE_LABEL).unwrap(),
            BuildInfoCallbackRecipe::SOURCE_DIGEST,
        )
        .unwrap()
    };
    let info = checked(BuildInfoTransformKind::Info);
    let version = checked(BuildInfoTransformKind::Version);
    let info_template = "#define BUILD_EMBED_LABEL \"{BUILD_EMBED_LABEL}\"\n#define BUILD_HOST \"{BUILD_HOST}\"\n#define BUILD_USER \"{BUILD_USER}\"\n";
    let version_template = "#define BUILD_SCM_REVISION \"{BUILD_SCM_REVISION}\"\n#define BUILD_SCM_STATUS \"{BUILD_SCM_STATUS}\"\n#define BUILD_TIMESTAMP {BUILD_TIMESTAMP}\n";
    let parsed = WorkspaceStatusArtifacts::parse_supplied_values(
        b"BUILD_EMBED_LABEL caf\xE9\nBUILD_HOST builder\r\nBUILD_USER user\nBUILD_SCM_REVISION abc\nBUILD_SCM_STATUS clean\nBUILD_TIMESTAMP 1\nBUILD_TIMESTAMP 2\r\nBUILD_TIMESTAMP\n",
    );
    let before = parsed.clone();
    assert_eq!(
        info.render_template(&parsed, info_template).unwrap(),
        "#define BUILD_EMBED_LABEL \"café\"\n#define BUILD_HOST \"builder\r\"\n#define BUILD_USER \"user\"\n"
    );
    assert_eq!(
        version.render_template(&parsed, version_template).unwrap(),
        "#define BUILD_SCM_REVISION \"abc\"\n#define BUILD_SCM_STATUS \"clean\"\n#define BUILD_TIMESTAMP 2\r\n"
    );
    assert_eq!(parsed, before);
    let empty_timestamp = WorkspaceStatusArtifacts::parse_supplied_values(b"BUILD_TIMESTAMP \n");
    assert_eq!(
        version
            .render_template(&empty_timestamp, version_template)
            .unwrap(),
        "#define BUILD_SCM_REVISION \"0\"\n#define BUILD_SCM_STATUS \"\"\n#define BUILD_TIMESTAMP \n"
    );
    let missing =
        WorkspaceStatusArtifacts::parse_supplied_values(b"BUILD_TIMESTAMP\nBUILD_TIMESTAMP\t123\n");
    for template in [version_template, "", "no timestamp placeholder"] {
        assert_eq!(
            version.render_template(&missing, template),
            Err(
                "timestamp information is missing from workspace status file, BUILD_TIMESTAMP key not found."
            )
        );
    }
    // Supplied bytes plus checked recipes do not prove status production,
    // template observation or original Latin1 output-byte encoding.
}
