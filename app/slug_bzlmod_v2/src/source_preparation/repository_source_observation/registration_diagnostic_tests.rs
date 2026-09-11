use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

use super::*;

fn relative(path: PathBuf) -> HostRepositoryRelativePath {
    host_repository_relative_path(path).unwrap()
}

fn render_kind(kind: HostRepositorySourceObservationErrorKind) -> String {
    let input = HostRepositorySourceObservationInput::Root(
        host_repository_source_input(
            RootRepositoryRoute::builtin_for_test(
                NormalizedAbsolutePath::new("/workspace").unwrap(),
            )
            .source_capability(),
        )
        .unwrap(),
    );
    let error = HostRepositorySourceObservationError {
        input,
        relative_path: relative("leaf.bzl".into()),
        kind,
    };
    let mut text = String::new();
    error.write_registration_diagnostic(&mut text).unwrap();
    text
}

fn render_request(error: RepositorySourceFileError) -> String {
    let mut text = String::new();
    request(&mut text, &error).unwrap();
    text
}

fn repo_path() -> Arc<PathBuf> {
    Arc::new(PathBuf::from("leaf.bzl"))
}

fn request_spec() -> RepoSpec {
    let mut attributes = starlark_map::small_map::SmallMap::new();
    attributes.insert(
        CompactString::new("path"),
        OverrideAttributeValue::String("dep".into()),
    );
    RepoSpec {
        rule_id: crate::RepoRuleId {
            bzl_file: CanonicalLabel::parse("@@bazel_tools//tools/build_defs/repo:local.bzl")
                .unwrap(),
            rule_name: "local_repository".into(),
        },
        attributes: Arc::new(attributes),
    }
}

#[test]
fn registration_diagnostic_source_classes_are_exhaustive() {
    let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
    let root_builtin = HostRepositorySourceObservationInput::Root(
        host_repository_source_input(
            RootRepositoryRoute::builtin_for_test(workspace.dupe()).source_capability(),
        )
        .unwrap(),
    );
    let root_request = HostRepositorySourceObservationInput::Root(
        host_repository_source_input(
            RootRepositoryRoute::for_test(
                workspace.dupe(),
                ApparentRepoName::new("dep").unwrap(),
                "dep".into(),
                CanonicalRepoName::new("dep+").unwrap(),
                request_spec(),
            )
            .source_capability(),
        )
        .unwrap(),
    );
    let canonical_builtin = HostRepositorySourceObservationInput::Canonical(
        host_canonical_repository_source_input(
            Arc::new(crate::HostCanonicalRepositoryRoute::builtin(
                workspace.dupe(),
                crate::HostBuiltinBazelToolsRepositoryMapping::testing(),
            )),
            None,
        )
        .unwrap(),
    );
    let canonical_repo = CanonicalRepoName::new("generated+").unwrap();
    let generated = crate::HostCanonicalRepositoryRoute::generated(
        workspace,
        canonical_repo.clone(),
        Arc::new(crate::HostSelectedExtensionOwner::testing("+extension")),
        0,
        "generated",
        request_spec(),
        canonical_repo,
        starlark_map::small_map::SmallMap::new(),
    )
    .unwrap();
    let canonical_request = HostRepositorySourceObservationInput::Canonical(
        host_canonical_repository_source_input(
            Arc::new(generated),
            Some(GeneratedRepositoryFileEffectPlan::build([]).unwrap()),
        )
        .unwrap(),
    );

    for (input, expected) in [
        (root_builtin, "RootBuiltin"),
        (root_request, "RootRequest"),
        (canonical_builtin, "CanonicalBuiltin"),
        (canonical_request, "CanonicalRequest"),
    ] {
        let mut text = String::new();
        source_class(&mut text, &input).unwrap();
        assert_eq!(text, expected);
    }
}

#[test]
fn registration_diagnostic_top_level_and_builtin_grammar_is_exhaustive() {
    use BuiltinBazelToolsSourceFileError as B;
    use HostRepositorySourceObservationErrorKind as E;

    use crate::BuiltinBazelToolsSourceKind as K;

    for (kind, expected) in [
        (
            E::BuiltinPath,
            "RootBuiltin: BuiltinPath path=hex:6c6561662e627a6c",
        ),
        (
            E::Builtin(B::InvalidPath { path: "bad".into() }),
            "RootBuiltin: Builtin.InvalidPath path=bad",
        ),
        (
            E::BuiltinCompute("compute".into()),
            "RootBuiltin: BuiltinCompute path=hex:6c6561662e627a6c message=compute",
        ),
        (
            E::Request(RepositorySourceFileError::Cycle {
                repo_relative_path: repo_path(),
            }),
            "RootBuiltin: Request.Cycle path=hex:6c6561662e627a6c",
        ),
        (
            E::RequestCompute("compute".into()),
            "RootBuiltin: RequestCompute path=hex:6c6561662e627a6c message=compute",
        ),
    ] {
        assert_eq!(render_kind(kind), expected);
    }

    for (error, expected) in [
        (
            B::InvalidPath { path: "bad".into() },
            "Builtin.InvalidPath path=bad",
        ),
        (
            B::WrongKind {
                path: "dir".into(),
                actual: K::File,
            },
            "Builtin.WrongKind path=dir actual=File",
        ),
        (
            B::WrongKind {
                path: "dir".into(),
                actual: K::Directory,
            },
            "Builtin.WrongKind path=dir actual=Directory",
        ),
        (
            B::UnsupportedCatalog {
                path: "outside".into(),
            },
            "Builtin.UnsupportedCatalog path=outside",
        ),
        (
            B::Integrity {
                path: "file".into(),
                expected_sha256: "expected".into(),
                actual_sha256: "actual".into(),
            },
            "Builtin.Integrity path=file actual_sha256=actual expected_sha256=expected",
        ),
    ] {
        let mut text = String::new();
        builtin(&mut text, &error).unwrap();
        assert_eq!(text, expected);
    }
}

#[test]
fn registration_diagnostic_request_grammar_is_exhaustive() {
    use PathNodeKind as K;
    use PathObservationOperation as O;
    use RepositorySourceFileError as E;

    let stat = PathLstat::new(K::RegularFile, 1, 2, 3, 4, 5);
    for (error, expected) in [
        (
            E::InvalidRepoRelativePath {
                requested_path: repo_path(),
            },
            "Request.InvalidRepoRelativePath path=hex:6c6561662e627a6c",
        ),
        (
            E::MaterializationCompute {
                repo_relative_path: repo_path(),
                message: "compute".into(),
            },
            "Request.MaterializationCompute path=hex:6c6561662e627a6c message=compute",
        ),
        (
            E::Materialization {
                repo_relative_path: repo_path(),
                error: Arc::new(RepositoryMaterializationError::Transport(
                    "transport".into(),
                )),
            },
            "Request.Materialization path=hex:6c6561662e627a6c kind=Transport message=transport",
        ),
        (
            E::InvalidMaterializedPath {
                repo_relative_path: repo_path(),
            },
            "Request.InvalidMaterializedPath path=hex:6c6561662e627a6c",
        ),
        (
            E::Observation {
                repo_relative_path: repo_path(),
                operation: O::FileBytes,
                error: PathObservationError::NotALink,
            },
            "Request.Observation path=hex:6c6561662e627a6c operation=FileBytes error=NotALink",
        ),
        (
            E::InconsistentState {
                repo_relative_path: repo_path(),
                operation: O::Lstat,
                before: None,
                after: Some(stat),
            },
            "Request.InconsistentState path=hex:6c6561662e627a6c operation=Lstat before=none after=RegularFile",
        ),
        (
            E::WrongKind {
                repo_relative_path: repo_path(),
                actual: K::Directory,
            },
            "Request.WrongKind path=hex:6c6561662e627a6c actual=Directory",
        ),
        (
            E::Cycle {
                repo_relative_path: repo_path(),
            },
            "Request.Cycle path=hex:6c6561662e627a6c",
        ),
        (
            E::InfiniteExpansion {
                repo_relative_path: repo_path(),
            },
            "Request.InfiniteExpansion path=hex:6c6561662e627a6c",
        ),
        (
            E::ResolutionCompute {
                repo_relative_path: repo_path(),
                message: "resolution".into(),
            },
            "Request.ResolutionCompute path=hex:6c6561662e627a6c message=resolution",
        ),
        (
            E::FileCompute {
                repo_relative_path: repo_path(),
                message: "file".into(),
            },
            "Request.FileCompute path=hex:6c6561662e627a6c message=file",
        ),
    ] {
        assert_eq!(render_request(error), expected);
    }
}

#[test]
fn registration_diagnostic_nested_discriminators_are_exhaustive() {
    use RepositoryMaterializationError as M;
    let materialization_cases = [
        (M::RootModuleFiles("m".into()), "RootModuleFiles"),
        (M::MissingOverride("m".into()), "MissingOverride"),
        (M::UnsupportedOverride("m".into()), "UnsupportedOverride"),
        (
            M::InvalidCanonicalRepository("m".into()),
            "InvalidCanonicalRepository",
        ),
        (M::InvalidWorkspace("m".into()), "InvalidWorkspace"),
        (M::ResultCompute("m".into()), "ResultCompute"),
        (M::MissingGeneration("m".into()), "MissingGeneration"),
        (M::Spec("m".into()), "Spec"),
        (M::Transport("m".into()), "Transport"),
        (M::Materialization("m".into()), "Materialization"),
    ];
    for (error, expected) in materialization_cases {
        assert_eq!(materialization(&error), (expected, "m"));
    }

    use PathObservationOperation as O;
    for (operation, expected) in [
        (O::Lstat, "Lstat"),
        (O::ReadLink, "ReadLink"),
        (O::FileBytes, "FileBytes"),
        (O::DirectoryEntries, "DirectoryEntries"),
        (O::WindowsLongPath, "WindowsLongPath"),
        (O::WindowsOptionPathLongName, "WindowsOptionPathLongName"),
    ] {
        assert_eq!(operation_name(operation), expected);
    }

    use PathNodeKind as K;
    for (kind, expected) in [
        (K::RegularFile, "RegularFile"),
        (K::Directory, "Directory"),
        (K::Symlink, "Symlink"),
        (K::SpecialFile, "SpecialFile"),
    ] {
        assert_eq!(node_kind(kind), expected);
    }
}

#[test]
fn registration_diagnostic_observation_and_io_discriminators_are_exhaustive() {
    use slug_workspace_v2::PathIoErrorKind as I;
    let io_kinds = [
        (I::NotFound, "NotFound"),
        (I::PermissionDenied, "PermissionDenied"),
        (I::ConnectionRefused, "ConnectionRefused"),
        (I::ConnectionReset, "ConnectionReset"),
        (I::HostUnreachable, "HostUnreachable"),
        (I::NetworkUnreachable, "NetworkUnreachable"),
        (I::ConnectionAborted, "ConnectionAborted"),
        (I::NotConnected, "NotConnected"),
        (I::AddrInUse, "AddrInUse"),
        (I::AddrNotAvailable, "AddrNotAvailable"),
        (I::NetworkDown, "NetworkDown"),
        (I::BrokenPipe, "BrokenPipe"),
        (I::AlreadyExists, "AlreadyExists"),
        (I::WouldBlock, "WouldBlock"),
        (I::NotADirectory, "NotADirectory"),
        (I::IsADirectory, "IsADirectory"),
        (I::DirectoryNotEmpty, "DirectoryNotEmpty"),
        (I::ReadOnlyFilesystem, "ReadOnlyFilesystem"),
        (I::StaleNetworkFileHandle, "StaleNetworkFileHandle"),
        (I::InvalidInput, "InvalidInput"),
        (I::InvalidData, "InvalidData"),
        (I::TimedOut, "TimedOut"),
        (I::WriteZero, "WriteZero"),
        (I::StorageFull, "StorageFull"),
        (I::NotSeekable, "NotSeekable"),
        (I::QuotaExceeded, "QuotaExceeded"),
        (I::FileTooLarge, "FileTooLarge"),
        (I::ResourceBusy, "ResourceBusy"),
        (I::ExecutableFileBusy, "ExecutableFileBusy"),
        (I::Deadlock, "Deadlock"),
        (I::CrossesDevices, "CrossesDevices"),
        (I::TooManyLinks, "TooManyLinks"),
        (I::InvalidFilename, "InvalidFilename"),
        (I::ArgumentListTooLong, "ArgumentListTooLong"),
        (I::Interrupted, "Interrupted"),
        (I::Unsupported, "Unsupported"),
        (I::UnexpectedEof, "UnexpectedEof"),
        (I::OutOfMemory, "OutOfMemory"),
        (I::Other, "Other"),
    ];
    for (kind, expected) in io_kinds {
        assert_eq!(io_kind(kind), expected);
    }

    let stat = PathLstat::new(PathNodeKind::Symlink, 1, 2, 3, 4, 5);
    for (error, expected) in [
        (
            PathObservationError::Io {
                kind: I::PermissionDenied,
                raw_os_error: Some(13),
            },
            "Io kind=PermissionDenied raw_os_error=13",
        ),
        (
            PathObservationError::Io {
                kind: I::NotFound,
                raw_os_error: None,
            },
            "Io kind=NotFound raw_os_error=none",
        ),
        (PathObservationError::NotALink, "NotALink"),
        (
            PathObservationError::WrongKind {
                expected: PathNodeKind::RegularFile,
                actual: PathNodeKind::Directory,
            },
            "WrongKind expected=RegularFile actual=Directory",
        ),
        (
            PathObservationError::InconsistentState {
                before: Some(stat),
                after: None,
            },
            "InconsistentState before=Symlink after=none",
        ),
    ] {
        let mut text = String::new();
        observation_error(&mut text, &error).unwrap();
        assert_eq!(text, expected);
    }
}

#[test]
fn registration_diagnostic_same_display_preserves_typed_lstat_identity_a_b_a() {
    let input = HostRepositorySourceObservationInput::Root(
        host_repository_source_input(
            RootRepositoryRoute::builtin_for_test(
                NormalizedAbsolutePath::new("/workspace").unwrap(),
            )
            .source_capability(),
        )
        .unwrap(),
    );
    let make = |size| HostRepositorySourceObservationError {
        input: input.clone(),
        relative_path: relative("leaf.bzl".into()),
        kind: HostRepositorySourceObservationErrorKind::Request(
            RepositorySourceFileError::InconsistentState {
                repo_relative_path: repo_path(),
                operation: PathObservationOperation::Lstat,
                before: Some(PathLstat::new(PathNodeKind::RegularFile, size, 2, 3, 4, 5)),
                after: None,
            },
        ),
    };
    let a = make(1);
    let b = make(9);
    let restored = make(1);
    assert!(a != b && a == restored);
    let render = |error: &HostRepositorySourceObservationError| {
        let mut text = String::new();
        error.write_registration_diagnostic(&mut text).unwrap();
        text
    };
    assert_eq!(render(&a), render(&b));
    assert_eq!(render(&a), render(&restored));
}

#[cfg(unix)]
#[test]
fn registration_diagnostic_path_is_byte_exact_and_writer_stop_is_immediate() {
    use std::os::unix::ffi::OsStringExt;

    let value = PathBuf::from(std::ffi::OsString::from_vec(vec![b'a', 0xff, b'z']));
    let mut text = String::new();
    path(&mut text, &value).unwrap();
    assert_eq!(text, "hex:61ff7a");

    struct Stop {
        calls: usize,
    }
    impl fmt::Write for Stop {
        fn write_str(&mut self, _: &str) -> fmt::Result {
            self.calls += 1;
            Err(fmt::Error)
        }
    }
    let mut stop = Stop { calls: 0 };
    assert!(path(&mut stop, &value).is_err());
    assert_eq!(stop.calls, 1);

    struct Cap {
        bytes: usize,
    }
    impl fmt::Write for Cap {
        fn write_str(&mut self, text: &str) -> fmt::Result {
            self.bytes += text.len().min(64 - self.bytes);
            if self.bytes == 64 {
                Err(fmt::Error)
            } else {
                Ok(())
            }
        }
    }
    let mut cap = Cap { bytes: 0 };
    let error = HostRepositorySourceObservationErrorKind::RequestCompute(
        "message".repeat(1024 * 1024).into(),
    );
    assert!(render_kind_with_writer(error, &mut cap).is_err());
    assert_eq!(cap.bytes, 64);
}

#[cfg(unix)]
fn render_kind_with_writer(
    kind: HostRepositorySourceObservationErrorKind,
    out: &mut dyn fmt::Write,
) -> fmt::Result {
    let input = HostRepositorySourceObservationInput::Root(
        host_repository_source_input(
            RootRepositoryRoute::builtin_for_test(
                NormalizedAbsolutePath::new("/workspace").unwrap(),
            )
            .source_capability(),
        )
        .unwrap(),
    );
    HostRepositorySourceObservationError {
        input,
        relative_path: relative("leaf.bzl".into()),
        kind,
    }
    .write_registration_diagnostic(out)
}
