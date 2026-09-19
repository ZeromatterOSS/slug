use std::sync::Arc;

use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::CanonicalRepoName;
use slug_identity_v2::PackageIdentifier;
use slug_identity_v2::PackagePath;

use super::*;
use crate::ActionOutput;
use crate::AnalysisConfiguredTargetKey;
use crate::AnalysisDepset;
use crate::AnalysisValue;
use crate::Depset;
use crate::DepsetOrder;
use crate::RetainedRunfiles;
use crate::RunfilesPackageDepset;
use crate::RunfilesPackageMetadata;
use crate::RunfilesSupport;
use crate::RunfilesSymlink;

fn source(path: &str) -> AnalysisArtifact {
    AnalysisArtifact::Source(CanonicalLabel::parse(&format!("@@//:{path}")).unwrap())
}

fn generated(owner: &str, config: &[u8], path: &str, kind: ActionOutputKind) -> AnalysisArtifact {
    AnalysisArtifact::Derived {
        owner: AnalysisConfiguredTargetKey::new(CanonicalLabel::parse(owner).unwrap(), config),
        output: ActionOutput::new(path, kind),
    }
}

fn file(path: &str) -> AnalysisArtifact {
    generated("@@//:owner", b"A", path, ActionOutputKind::File)
}

fn support() -> RunfilesSupport {
    RunfilesSupport {
        runfiles: RetainedRunfiles::empty(),
        tree: generated(
            "@@//:owner",
            b"A",
            "bin.runfiles",
            ActionOutputKind::RunfilesTree,
        ),
        input_manifest: file("bin.runfiles_manifest"),
        manifest: Some(file("bin.runfiles/MANIFEST")),
        repo_mapping_manifest: None,
    }
}

fn links(values: Vec<(&str, AnalysisArtifact)>) -> crate::RunfilesSymlinkDepset {
    Depset::from_direct(
        DepsetOrder::Default,
        values
            .into_iter()
            .map(|(path, artifact)| RunfilesSymlink::new(path, artifact))
            .collect(),
    )
    .unwrap()
}

fn files(values: Vec<AnalysisArtifact>) -> AnalysisDepset {
    AnalysisDepset::new(
        DepsetOrder::Default,
        values.into_iter().map(AnalysisValue::artifact).collect(),
        vec![],
    )
    .unwrap()
}

#[test]
fn source_manifest_conditional_escaping_empty_entries_and_utf8_bytes() {
    let mut support = support();
    support.runfiles.root_symlinks = links(vec![
        ("_main/a\\b", source("a")),
        ("_main/b space", source("b")),
        ("_main/c", source("c")),
        ("_main/d\nline", source("d")),
        ("_main/é", source("unicode")),
        ("_main/\u{10000}", source("supplementary")),
        ("_main/\u{e000}", source("bmp")),
        ("MANIFEST", source("authored")),
    ]);
    support.runfiles.empty_filenames = Depset::from_direct(
        DepsetOrder::Default,
        vec!["empty".to_owned(), "empty space".to_owned()],
    )
    .unwrap();
    let bytes = support
        .layout()
        .unwrap()
        .source_manifest_bytes(|artifact| {
            Ok::<_, &'static str>(
                match artifact.path().as_ref() {
                    "a" => "/target\\with space",
                    "b" => "/target\\plain",
                    "c" => "/new\nline\\x",
                    "d" => "/space and\\slash",
                    "unicode" => "/é",
                    "supplementary" => "/\u{10000}",
                    "bmp" => "/\u{e000}",
                    "authored" => "/authored",
                    other => panic!("unexpected resolver call: {other}"),
                }
                .to_owned(),
            )
        })
        .unwrap();
    assert_eq!(
        bytes,
        concat!(
            "MANIFEST /authored\n",
            "_main/a\\b /target\\with space\n",
            " _main/b\\sspace /target\\bplain\n",
            " _main/c /new\\nline\\bx\n",
            " _main/d\\nline /space and\\bslash\n",
            "_main/empty \n",
            " _main/empty\\sspace \n",
            "_main/é /é\n",
            "_main/\u{e000} /\u{e000}\n",
            "_main/\u{10000} /\u{10000}\n",
        )
        .as_bytes()
    );
    assert!(bytes.windows(2).any(|pair| pair == [0xc3, 0xa9]));
}

#[test]
fn source_manifest_preserves_artifact_identity_and_rejects_unsupported_before_resolution() {
    let first = file("same");
    let second = generated("@@//:other", b"B", "same", ActionOutputKind::File);
    let mut support = support();
    support.runfiles.root_symlinks = links(vec![
        ("_main/a", first.clone()),
        ("_main/b", second.clone()),
    ]);
    let layout = support.layout().unwrap();
    let mut calls = Vec::new();
    let bytes = layout
        .source_manifest_bytes(|artifact| {
            calls.push(artifact.clone());
            Ok::<_, &'static str>(
                if artifact == &first {
                    "/A/same"
                } else {
                    "/B/same"
                }
                .to_owned(),
            )
        })
        .unwrap();
    assert_eq!(calls, [first.clone(), second]);
    assert_eq!(bytes, b"_main/a /A/same\n_main/b /B/same\n");
    assert_eq!(
        layout.source_manifest_bytes(|_| Err::<String, _>("missing target")),
        Err(SourceManifestError::Resolve("missing target"))
    );
    for path in ["relative", "/nul\0path"] {
        assert_eq!(
            layout.source_manifest_bytes(|_| Ok::<_, &'static str>(path.to_owned())),
            Err(SourceManifestError::InvalidTarget(first.clone()))
        );
    }
    let symlink = generated("@@//:owner", b"A", "link", ActionOutputKind::Symlink);
    support.runfiles.root_symlinks = links(vec![("_main/a", first), ("_main/z", symlink.clone())]);
    let result =
        support
            .layout()
            .unwrap()
            .source_manifest_bytes(|_| -> Result<String, &'static str> {
                panic!("unsupported kind must preflight every entry")
            });
    assert_eq!(result, Err(SourceManifestError::UnresolvedSymlink(symlink)));
}

#[test]
fn source_manifest_rejects_nested_and_error_diagnostics_but_retains_warn() {
    let mut support = support();
    support.runfiles.symlinks = links(vec![
        ("parent", source("one")),
        ("parent/child", source("other")),
    ]);
    let warned = support.layout().unwrap();
    assert_eq!(warned.diagnostics().len(), 1);
    assert_eq!(
        warned
            .source_manifest_bytes(|_| Ok::<_, &'static str>("/one".to_owned()))
            .unwrap(),
        b"_main/parent /one\n"
    );
    assert_eq!(warned.diagnostics().len(), 1);
    support.runfiles.conflict_policy = RunfilesConflictPolicy::Error;
    let rejected = support.layout().unwrap();
    assert_eq!(
        rejected.source_manifest_bytes(|_| -> Result<String, &'static str> {
            panic!("error diagnostic precedes resolver")
        }),
        Err(SourceManifestError::Diagnostic(
            rejected.diagnostics()[0].clone()
        ))
    );
    support.runfiles.symlinks = links(vec![("nested", support.tree.clone())]);
    let nested = support.layout().unwrap();
    assert!(matches!(
        nested.source_manifest_bytes(|_| -> Result<String, &'static str> {
            panic!("nested diagnostic precedes resolver")
        }),
        Err(SourceManifestError::Diagnostic(
            RunfilesLayoutDiagnostic::NestedRunfilesTree { .. }
        ))
    ));
}

fn mapping(entries: &[(&str, &str)], group: Option<&str>) -> Arc<RunfilesRepositoryMapping> {
    Arc::new(RunfilesRepositoryMapping::new(
        entries
            .iter()
            .map(|(apparent, canonical)| {
                (
                    if apparent.is_empty() {
                        ApparentRepoName::root()
                    } else {
                        ApparentRepoName::new(*apparent).unwrap()
                    },
                    if canonical.is_empty() {
                        CanonicalRepoName::root()
                    } else {
                        CanonicalRepoName::new(*canonical).unwrap()
                    },
                )
            })
            .collect::<Vec<_>>()
            .into(),
        group.map(Into::into),
    ))
}

fn package(
    repo: &str,
    path: &str,
    mapping: Arc<RunfilesRepositoryMapping>,
) -> Arc<RunfilesPackageMetadata> {
    Arc::new(RunfilesPackageMetadata::new(
        PackageIdentifier::new(
            if repo.is_empty() {
                CanonicalRepoName::root()
            } else {
                CanonicalRepoName::new(repo).unwrap()
            },
            PackagePath::parse(path).unwrap(),
        ),
        mapping,
    ))
}

fn mapping_action(
    support: RunfilesSupport,
    packages: Vec<Arc<RunfilesPackageMetadata>>,
    compact: bool,
) -> RunfilesSupportActionSpec {
    RunfilesSupportActionSpec::RepoMappingManifest {
        support: Arc::new(support),
        packages: RunfilesPackageDepset::from_direct(DepsetOrder::Default, packages).unwrap(),
        workspace_name: "workspace".into(),
        emit_compact_repo_mapping: compact,
        output: ActionOutput::new("bin.repo_mapping", ActionOutputKind::File),
    }
}

#[test]
fn repository_mapping_uses_raw_presence_first_package_and_sorted_filtered_rows() {
    let mut support = support();
    let external = AnalysisArtifact::Source(CanonicalLabel::parse("@@raw+//:file").unwrap());
    support.runfiles.files = files(vec![
        external.clone(),
        generated(
            "@@generated+//:owner",
            b"config",
            "unqualified",
            ActionOutputKind::File,
        ),
    ]);
    support.runfiles.symlinks = links(vec![("normal", source("input"))]);
    // Root override obscures the sole raw+ artifact, which still controls presence.
    support.runfiles.root_symlinks = links(vec![
        ("raw+/file", source("override")),
        ("root+/name", source("root")),
    ]);
    assert!(
        !support
            .layout()
            .unwrap()
            .entries()
            .iter()
            .any(|entry| entry.target() == &RunfilesLayoutTarget::Artifact(external.clone()))
    );
    let entries = mapping(
        &[
            ("z", "raw+"),
            ("generated", "generated+"),
            ("", ""),
            ("unused", "unused+"),
            ("main", ""),
            ("root", "root+"),
        ],
        None,
    );
    let action = mapping_action(
        support,
        vec![
            package("z+", "one", entries.clone()),
            package("", "", entries.clone()),
            package("z+", "two", mapping(&[("different", "raw+")], None)),
            package("a+", "", entries),
        ],
        true,
    );
    assert_eq!(
        action.repo_mapping_manifest_bytes().unwrap(),
        concat!(
            ",generated,generated+\n,main,workspace\n,root,root+\n,z,raw+\n",
            "a+,generated,generated+\na+,main,workspace\na+,root,root+\na+,z,raw+\n",
            "z+,generated,generated+\nz+,main,workspace\nz+,root,root+\nz+,z,raw+\n",
        )
        .as_bytes()
    );
}

#[test]
fn repository_mapping_compacts_only_shared_identical_consecutive_prefixes() {
    let mut support = support();
    support.runfiles.files = files(vec![source("input")]);
    let shared = mapping(&[("main", "")], Some("extension"));
    let packages = vec![
        package("ext+repo1", "", shared.clone()),
        package("ext+repo2", "", shared.clone()),
        package("ext+repo3", "", mapping(&[("main", "")], Some("other"))),
        package(
            "ext+repo4",
            "",
            mapping(&[("renamed", "")], Some("extension")),
        ),
        package("other+repo5", "", shared.clone()),
        package("ordinary", "", shared),
        package("plain+one", "", mapping(&[("main", "")], None)),
        package("plain+two", "", mapping(&[("main", "")], None)),
    ];
    let compact = mapping_action(support.clone(), packages.clone(), true);
    assert_eq!(
        compact.repo_mapping_manifest_bytes().unwrap(),
        concat!(
            "ext+*,main,workspace\n",
            "ext+repo3,main,workspace\n",
            "ext+repo4,renamed,workspace\n",
            "ordinary,main,workspace\n",
            "other+repo5,main,workspace\n",
            "plain+one,main,workspace\nplain+two,main,workspace\n",
        )
        .as_bytes()
    );
    let expanded = mapping_action(support, packages, false)
        .repo_mapping_manifest_bytes()
        .unwrap();
    assert!(expanded.starts_with(b"ext+repo1,main,workspace\next+repo2,main,workspace\n"));
    assert!(!expanded.contains(&b'*'));
    let wrong_action = RunfilesSupportActionSpec::SourceSymlinkManifest {
        support: Arc::new(super::tests::support()),
        remotable: false,
        output: ActionOutput::new("manifest", ActionOutputKind::File),
    };
    assert_eq!(
        wrong_action.repo_mapping_manifest_bytes(),
        Err(RunfilesManifestError::NotRepositoryMappingAction)
    );
}
