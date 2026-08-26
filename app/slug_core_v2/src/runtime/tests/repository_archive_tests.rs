use std::cell::Cell;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

use base64::Engine;
use compact_str::CompactString;
use sha2::Digest;
use sha2::Sha256;
use slug_bzlmod_v2::OverrideAttributeKey;
use slug_bzlmod_v2::RepoRuleId;
use slug_bzlmod_v2::RepositoryMaterializationGeneration;
use slug_bzlmod_v2::RepositoryMaterializationKind;
use slug_bzlmod_v2::RepositoryMaterializationRequest;
use slug_bzlmod_v2::RepositoryMaterializationRequestId;
use slug_bzlmod_v2::RepositoryMaterializationResult;
use slug_bzlmod_v2::RepositoryMaterializationSuccess;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::CanonicalRepoName;
use slug_workspace_v2::NormalizedAbsolutePath;
use starlark_map::small_map::SmallMap;

use super::super::repository_io::ArchiveFailureStage;
use super::super::repository_io::RepositoryMaterializer;
use super::*;

fn archive_spec(url: String, sha256: String) -> RepoSpec {
    let attributes: [(CompactString, OverrideAttributeValue); 3] = [
        (
            "urls".into(),
            OverrideAttributeValue::Iterable(Arc::new([OverrideAttributeValue::String(
                url.into(),
            )])),
        ),
        (
            "sha256".into(),
            OverrideAttributeValue::String(sha256.into()),
        ),
        ("type".into(), OverrideAttributeValue::String("tar".into())),
    ];
    RepoSpec {
        rule_id: RepoRuleId {
            bzl_file: CanonicalLabel::parse("@@bazel_tools//tools/build_defs/repo:http.bzl")
                .unwrap(),
            rule_name: "http_archive".into(),
        },
        attributes: Arc::new(SmallMap::from_iter(attributes)),
    }
}

fn archive_spec_with_prefix(url: String, sha256: String, prefix: &str) -> RepoSpec {
    let mut spec = archive_spec(url, sha256);
    Arc::make_mut(&mut spec.attributes).insert(
        "strip_prefix".into(),
        OverrideAttributeValue::String(prefix.into()),
    );
    spec
}

#[derive(Clone, Copy)]
struct TarEntry<'a> {
    name: &'a [u8],
    prefix: &'a [u8],
    typeflag: u8,
    data: &'a [u8],
}

fn ustar(entries: &[TarEntry<'_>], terminator: bool) -> Vec<u8> {
    let mut archive = Vec::new();
    for entry in entries {
        assert!(entry.name.len() <= 100);
        assert!(entry.prefix.len() <= 155);
        let mut header = [0u8; 512];
        header[..entry.name.len()].copy_from_slice(entry.name);
        write_octal(&mut header[100..108], 0o644);
        write_octal(&mut header[108..116], 0);
        write_octal(&mut header[116..124], 0);
        write_octal(&mut header[124..136], entry.data.len());
        write_octal(&mut header[136..148], 0);
        header[148..156].fill(b' ');
        header[156] = entry.typeflag;
        header[257..263].copy_from_slice(b"ustar\0");
        header[263..265].copy_from_slice(b"00");
        header[345..345 + entry.prefix.len()].copy_from_slice(entry.prefix);
        let checksum = header.iter().map(|byte| u32::from(*byte)).sum::<u32>();
        write_octal(&mut header[148..156], checksum as usize);
        archive.extend_from_slice(&header);
        archive.extend_from_slice(entry.data);
        let padding = (512 - entry.data.len() % 512) % 512;
        archive.resize(archive.len() + padding, 0);
    }
    if terminator {
        archive.resize(archive.len() + 1024, 0);
    }
    archive
}

fn write_octal(field: &mut [u8], value: usize) {
    let digits = format!("{value:0width$o}", width = field.len() - 1);
    assert!(digits.len() < field.len());
    field.fill(0);
    field[..digits.len()].copy_from_slice(digits.as_bytes());
}

#[derive(Default)]
struct RecordingDestination {
    calls: Vec<(PlannedUstarKind, PathBuf, Vec<u8>)>,
    fail: bool,
}

impl ArchiveDestination for RecordingDestination {
    fn create_parent(&mut self, _path: &Path) -> std::io::Result<()> {
        if self.fail {
            return Err(std::io::Error::other("scripted parent failure"));
        }
        Ok(())
    }

    fn create_directory(&mut self, path: &Path) -> std::io::Result<()> {
        if self.fail {
            return Err(std::io::Error::other("scripted extraction failure"));
        }
        self.calls
            .push((PlannedUstarKind::Directory, path.to_owned(), Vec::new()));
        Ok(())
    }

    fn write_regular(&mut self, path: &Path, bytes: &[u8]) -> std::io::Result<()> {
        if self.fail {
            return Err(std::io::Error::other("scripted extraction failure"));
        }
        self.calls
            .push((PlannedUstarKind::Regular, path.to_owned(), bytes.to_vec()));
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum ScriptedFailure {
    None,
    Root,
    Capture,
    Read,
    Write,
    Flush,
}

struct ScriptedArchiveIo {
    source: Vec<u8>,
    failure: ScriptedFailure,
    reads: usize,
    root_path: Option<PathBuf>,
    capture_path: Option<PathBuf>,
    replace_source: Option<Vec<u8>>,
    delete_source: bool,
    destination_calls: usize,
    destination_failure: Option<&'static str>,
}

impl ScriptedArchiveIo {
    fn new(source: Vec<u8>, failure: ScriptedFailure) -> Self {
        Self {
            source,
            failure,
            reads: 0,
            root_path: None,
            capture_path: None,
            replace_source: None,
            delete_source: false,
            destination_calls: 0,
            destination_failure: None,
        }
    }
}

impl ArchiveIo for ScriptedArchiveIo {
    fn create_root(&mut self) -> std::io::Result<tempfile::TempDir> {
        if matches!(self.failure, ScriptedFailure::Root) {
            return Err(std::io::Error::other("scripted root failure"));
        }
        let root = tempfile::tempdir()?;
        self.root_path = Some(root.path().to_owned());
        Ok(root)
    }

    fn create_capture(&mut self) -> std::io::Result<tempfile::NamedTempFile> {
        if matches!(self.failure, ScriptedFailure::Capture) {
            return Err(std::io::Error::other("scripted capture failure"));
        }
        let capture = tempfile::NamedTempFile::new()?;
        self.capture_path = Some(capture.path().to_owned());
        Ok(capture)
    }

    fn read_source(&mut self, source: &Path) -> std::io::Result<Vec<u8>> {
        self.reads += 1;
        if matches!(self.failure, ScriptedFailure::Read) {
            return Err(std::io::Error::other("scripted read failure"));
        }
        if let Some(replacement) = self.replace_source.take() {
            std::fs::write(source, replacement)?;
        } else if self.delete_source {
            std::fs::remove_file(source)?;
        }
        Ok(self.source.clone())
    }

    fn write_capture(
        &mut self,
        capture: &mut tempfile::NamedTempFile,
        bytes: &[u8],
    ) -> std::io::Result<()> {
        if matches!(self.failure, ScriptedFailure::Write) {
            return Err(std::io::Error::other("scripted write failure"));
        }
        capture.write_all(bytes)
    }

    fn flush_capture(&mut self, capture: &mut tempfile::NamedTempFile) -> std::io::Result<()> {
        if matches!(self.failure, ScriptedFailure::Flush) {
            return Err(std::io::Error::other("scripted flush failure"));
        }
        capture.flush()
    }
}

impl ArchiveDestination for ScriptedArchiveIo {
    fn create_parent(&mut self, _path: &Path) -> std::io::Result<()> {
        self.destination_calls += 1;
        if self.destination_failure == Some("parent") {
            return Err(std::io::Error::other("scripted parent failure"));
        }
        Ok(())
    }

    fn create_directory(&mut self, path: &Path) -> std::io::Result<()> {
        self.destination_calls += 1;
        if self.destination_failure == Some("directory") {
            return Err(std::io::Error::other("scripted directory failure"));
        }
        std::fs::create_dir_all(path)
    }

    fn write_regular(&mut self, path: &Path, bytes: &[u8]) -> std::io::Result<()> {
        self.destination_calls += 1;
        if self.destination_failure == Some("write") {
            return Err(std::io::Error::other("scripted file failure"));
        }
        let mut output = File::create(path)?;
        output.write_all(bytes)
    }
}
#[test]
fn archive_path_flavors_and_latin1_prefix_are_exact() {
    assert_eq!(latin1_bytes("\u{ff}root"), b"\xffroot");
    assert_eq!(latin1_bytes("\u{100}root"), b"?root");
    assert_eq!(
        normalize_raw_tar_path(br"root\f", PathFlavor::Unix).unwrap(),
        vec![br"root\f".to_vec()]
    );
    for (input, expected) in [
        (&b"C:/root/f"[..], vec![b"root".to_vec(), b"f".to_vec()]),
        (&br"C:\root\f"[..], vec![b"root".to_vec(), b"f".to_vec()]),
        (
            &br"\\server\share\f"[..],
            vec![b"server".to_vec(), b"share".to_vec(), b"f".to_vec()],
        ),
        (&br"root\f"[..], vec![b"root".to_vec(), b"f".to_vec()]),
        (&b"C:foo"[..], vec![b"C:foo".to_vec()]),
        (&br"a\..\b"[..], vec![b"b".to_vec()]),
    ] {
        assert_eq!(
            normalize_raw_tar_path(input, PathFlavor::Windows).unwrap(),
            expected
        );
    }
    assert!(
        join_raw_components(Path::new("root"), &[b"C:foo".to_vec()], PathFlavor::Windows).is_err()
    );
    assert!(
        join_raw_components(
            Path::new("root"),
            &[b"..".to_vec(), b"escape".to_vec()],
            PathFlavor::Unix
        )
        .is_err()
    );
    for (prefix, flavor) in [
        (Vec::<Vec<u8>>::new(), PathFlavor::Unix),
        (vec![b"..".to_vec()], PathFlavor::Unix),
        (vec![b"C:foo".to_vec()], PathFlavor::Windows),
    ] {
        let error = validate_strip_prefix(&prefix, flavor).err().unwrap();
        assert_eq!(error.stage, ArchiveFailureStage::Spec);
    }
    assert!(validate_strip_prefix(&[b"safe".to_vec()], PathFlavor::Unix).is_ok());
}

#[test]
fn archive_short_headers_and_declared_bounds_match_commons() {
    for length in [0, 1, 511] {
        let short = vec![b'x'; length];
        assert!(
            inspect_and_plan_ustar_for_flavor(&short, None, PathFlavor::Unix, Path::new(""))
                .unwrap()
                .entries
                .is_empty()
        );
        assert!(
            inspect_and_plan_ustar_for_flavor(
                &short,
                Some(&[b"missing".to_vec()]),
                PathFlavor::Unix,
                Path::new("")
            )
            .is_err()
        );
    }
    let entry = TarEntry {
        name: b"file",
        prefix: b"",
        typeflag: b'0',
        data: b"x",
    };
    let complete = ustar(&[entry], false);
    assert_eq!(
        inspect_and_plan_ustar_for_flavor(&complete, None, PathFlavor::Unix, Path::new(""))
            .unwrap()
            .entries
            .len(),
        1
    );
    for length in [1, 511] {
        let mut trailing = complete.clone();
        trailing.extend(std::iter::repeat_n(b'x', length));
        assert_eq!(
            inspect_and_plan_ustar_for_flavor(&trailing, None, PathFlavor::Unix, Path::new(""))
                .unwrap()
                .entries
                .len(),
            1
        );
    }
    let mut truncated_payload = ustar(
        &[TarEntry {
            name: b"file",
            prefix: b"",
            typeflag: b'0',
            data: &[1; 513],
        }],
        false,
    );
    truncated_payload.truncate(512 + 512);
    assert!(
        inspect_and_plan_ustar_for_flavor(
            &truncated_payload,
            None,
            PathFlavor::Unix,
            Path::new("")
        )
        .is_err()
    );
    let mut truncated_padding = complete.clone();
    truncated_padding.truncate(513);
    assert!(
        inspect_and_plan_ustar_for_flavor(
            &truncated_padding,
            None,
            PathFlavor::Unix,
            Path::new("")
        )
        .is_err()
    );
}

#[test]
fn archive_numeric_and_header_format_boundaries_are_explicit() {
    assert_eq!(parse_ustar_octal(b"\x001          ").unwrap(), 0);
    assert_eq!(parse_ustar_octal(b"       17\0 ").unwrap(), 15);
    assert!(parse_ustar_octal(b"000000008\0  ").is_err());
    assert!(parse_ustar_octal(&[b'7'; 30]).is_err());
    let mut binary = [0u8; 12];
    binary[0] = 0x80;
    assert!(parse_ustar_octal(&binary).is_err());

    let mut leading_nul = ustar(
        &[TarEntry {
            name: b"empty",
            prefix: b"",
            typeflag: b'0',
            data: b"",
        }],
        false,
    );
    leading_nul[124..136].copy_from_slice(b"\x001          ");
    let plan =
        inspect_and_plan_ustar_for_flavor(&leading_nul, None, PathFlavor::Unix, Path::new(""))
            .unwrap();
    assert!(plan.entries[0].payload.is_empty());

    for selected in [true, false] {
        let mut binary_archive = ustar(
            &[TarEntry {
                name: b"file",
                prefix: if selected {
                    b"wanted".as_slice()
                } else {
                    b"other".as_slice()
                },
                typeflag: b'0',
                data: b"",
            }],
            false,
        );
        binary_archive[124] = 0x80;
        assert!(
            inspect_and_plan_ustar_for_flavor(
                &binary_archive,
                Some(&[b"wanted".to_vec()]),
                PathFlavor::Unix,
                Path::new("")
            )
            .is_err()
        );
    }

    let entry = TarEntry {
        name: b"file",
        prefix: b"prefix",
        typeflag: b'0',
        data: b"x",
    };
    let mut legacy = ustar(&[entry], false);
    legacy[257..265].fill(0);
    let plan = inspect_and_plan_ustar_for_flavor(
        &legacy,
        Some(&[b"prefix".to_vec()]),
        PathFlavor::Unix,
        Path::new(""),
    )
    .unwrap();
    assert_eq!(plan.entries[0].components, vec![b"file".to_vec()]);

    let mut odd_version = ustar(&[entry], false);
    odd_version[263..265].copy_from_slice(b"!?");
    assert!(
        inspect_and_plan_ustar_for_flavor(&odd_version, None, PathFlavor::Unix, Path::new(""))
            .is_ok()
    );
    let mut gnu = ustar(&[entry], false);
    gnu[257..263].copy_from_slice(b"ustar ");
    assert!(
        inspect_and_plan_ustar_for_flavor(&gnu, None, PathFlavor::Unix, Path::new("")).is_err()
    );
    let mut xstar = ustar(&[entry], false);
    xstar[508..512].copy_from_slice(b"tar\0");
    assert!(
        inspect_and_plan_ustar_for_flavor(&xstar, None, PathFlavor::Unix, Path::new("")).is_err()
    );
    let long_prefix = [b'p'; 140];
    let mut discriminating_xstar = ustar(
        &[TarEntry {
            name: b"file",
            prefix: &long_prefix,
            typeflag: b'0',
            data: b"x",
        }],
        false,
    );
    discriminating_xstar[508..512].copy_from_slice(b"tar\0");
    assert!(
        inspect_and_plan_ustar_for_flavor(
            &discriminating_xstar,
            Some(&[long_prefix.to_vec()]),
            PathFlavor::Unix,
            Path::new("")
        )
        .is_err()
    );
    let mut xustar = ustar(&[entry], false);
    xustar[476..488].copy_from_slice(b"00000000000 ");
    xustar[488..500].copy_from_slice(b"00000000000 ");
    assert!(
        inspect_and_plan_ustar_for_flavor(&xustar, None, PathFlavor::Unix, Path::new("")).is_err()
    );
    let mut checksum_corrupt = ustar(&[entry], false);
    checksum_corrupt[148..156].fill(b'7');
    assert!(
        inspect_and_plan_ustar_for_flavor(&checksum_corrupt, None, PathFlavor::Unix, Path::new(""))
            .is_ok()
    );
    let mut checksum_leading_nul = ustar(&[entry], false);
    checksum_leading_nul[148..156].copy_from_slice(b"\0xxxxxxx");
    assert!(
        inspect_and_plan_ustar_for_flavor(
            &checksum_leading_nul,
            None,
            PathFlavor::Unix,
            Path::new("")
        )
        .is_ok()
    );
    let mut checksum_invalid = ustar(&[entry], false);
    checksum_invalid[148..156].copy_from_slice(b"000x000\0");
    assert!(
        inspect_and_plan_ustar_for_flavor(&checksum_invalid, None, PathFlavor::Unix, Path::new(""))
            .is_err()
    );
}

#[test]
fn archive_raw_prefix_normalization_and_type_rows_are_discriminating() {
    let archive = ustar(
        &[
            TarEntry {
                name: b"raw-\xff",
                prefix: b"\xffroot",
                typeflag: b'0',
                data: b"raw",
            },
            TarEntry {
                name: b"./dir/",
                prefix: b"\xffroot",
                typeflag: b'0',
                data: b"",
            },
            TarEntry {
                name: b"typed",
                prefix: b"\xffroot",
                typeflag: b'5',
                data: b"",
            },
            TarEntry {
                name: b"/absolute/../normalized",
                prefix: b"\xffroot",
                typeflag: 0,
                data: b"normalized",
            },
        ],
        false,
    );
    let plan = inspect_and_plan_ustar_for_flavor(
        &archive,
        Some(&[b"\xffroot".to_vec()]),
        PathFlavor::Unix,
        Path::new(""),
    )
    .unwrap();
    assert_eq!(plan.entries.len(), 4);
    assert_eq!(plan.entries[0].components[0], b"raw-\xff");
    assert_eq!(plan.entries[1].kind, PlannedUstarKind::Directory);
    assert_eq!(plan.entries[2].kind, PlannedUstarKind::Directory);
    assert_eq!(plan.entries[3].components, vec![b"normalized".to_vec()]);

    let source = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(source.path(), &archive).unwrap();
    let url = url::Url::from_file_path(source.path()).unwrap().to_string();
    let digest = format!("{:x}", Sha256::digest(&archive));
    let mut io = ScriptedArchiveIo::new(archive, ScriptedFailure::None);
    let Materialized::Immutable { root, .. } = materialize_archive_with(
        &archive_spec_with_prefix(url, digest, "\u{ff}root"),
        &mut io,
    )
    .unwrap() else {
        panic!("archive must be immutable");
    };
    assert_eq!(
        std::fs::read(root.path().join("normalized")).unwrap(),
        b"normalized"
    );

    let question_archive = ustar(
        &[TarEntry {
            name: b"file",
            prefix: b"?root",
            typeflag: b'0',
            data: b"question",
        }],
        false,
    );
    let question_source = tempfile::NamedTempFile::new().unwrap();
    let question_url = url::Url::from_file_path(question_source.path())
        .unwrap()
        .to_string();
    let question_digest = format!("{:x}", Sha256::digest(&question_archive));
    let mut io = ScriptedArchiveIo::new(question_archive, ScriptedFailure::None);
    assert!(
        materialize_archive_with(
            &archive_spec_with_prefix(question_url, question_digest, "\u{100}root"),
            &mut io
        )
        .is_ok()
    );
}

#[test]
fn archive_selection_types_and_planning_are_atomic() {
    let entries = [
        TarEntry {
            name: b"root/",
            prefix: b"",
            typeflag: b'0',
            data: b"",
        },
        TarEntry {
            name: b"file",
            prefix: b"root",
            typeflag: 0,
            data: b"first",
        },
        TarEntry {
            name: b"file",
            prefix: b"root",
            typeflag: b'0',
            data: b"last",
        },
        TarEntry {
            name: b"directory",
            prefix: b"root",
            typeflag: b'5',
            data: b"",
        },
        TarEntry {
            name: b"ignored",
            prefix: b"other",
            typeflag: b'3',
            data: b"",
        },
    ];
    let archive = ustar(&entries, true);
    let plan = inspect_and_plan_ustar_for_flavor(
        &archive,
        Some(&[b"root".to_vec()]),
        PathFlavor::Unix,
        Path::new(""),
    )
    .unwrap();
    assert_eq!(plan.entries.len(), 3);
    let mut destination = RecordingDestination::default();
    extract_ustar_plan(&archive, &plan, &mut destination).unwrap();
    assert_eq!(destination.calls[0].2, b"first");
    assert_eq!(destination.calls[1].2, b"last");

    let mut late_failure = ustar(&entries[..2], false);
    late_failure.extend(ustar(
        &[TarEntry {
            name: b"bad",
            prefix: b"root",
            typeflag: b'3',
            data: b"",
        }],
        false,
    ));
    let destination = RecordingDestination::default();
    assert!(
        inspect_and_plan_ustar_for_flavor(
            &late_failure,
            Some(&[b"root".to_vec()]),
            PathFlavor::Unix,
            Path::new("")
        )
        .is_err()
    );
    assert!(destination.calls.is_empty());

    let collision = ustar(
        &[
            TarEntry {
                name: b"same",
                prefix: b"",
                typeflag: b'5',
                data: b"",
            },
            TarEntry {
                name: b"same",
                prefix: b"",
                typeflag: b'0',
                data: b"x",
            },
        ],
        false,
    );
    assert!(
        inspect_and_plan_ustar_for_flavor(&collision, None, PathFlavor::Unix, Path::new(""))
            .is_err()
    );
    let reverse_collision = ustar(
        &[
            TarEntry {
                name: b"same",
                prefix: b"",
                typeflag: b'0',
                data: b"x",
            },
            TarEntry {
                name: b"same",
                prefix: b"",
                typeflag: b'5',
                data: b"",
            },
        ],
        false,
    );
    assert!(
        inspect_and_plan_ustar_for_flavor(
            &reverse_collision,
            None,
            PathFlavor::Unix,
            Path::new("")
        )
        .is_err()
    );
    for entries in [
        [
            TarEntry {
                name: b"a",
                prefix: b"",
                typeflag: b'0',
                data: b"x",
            },
            TarEntry {
                name: b"a/b",
                prefix: b"",
                typeflag: b'0',
                data: b"y",
            },
        ],
        [
            TarEntry {
                name: b"a/b",
                prefix: b"",
                typeflag: b'0',
                data: b"y",
            },
            TarEntry {
                name: b"a",
                prefix: b"",
                typeflag: b'0',
                data: b"x",
            },
        ],
    ] {
        let ancestor_collision = ustar(&entries, false);
        assert!(
            inspect_and_plan_ustar_for_flavor(
                &ancestor_collision,
                None,
                PathFlavor::Unix,
                Path::new("")
            )
            .is_err()
        );
    }
    for entries in [
        [
            TarEntry {
                name: b"a",
                prefix: b"",
                typeflag: b'5',
                data: b"",
            },
            TarEntry {
                name: b"a",
                prefix: b"",
                typeflag: b'5',
                data: b"",
            },
        ],
        [
            TarEntry {
                name: b"a",
                prefix: b"",
                typeflag: b'5',
                data: b"",
            },
            TarEntry {
                name: b"a/b",
                prefix: b"",
                typeflag: b'0',
                data: b"x",
            },
        ],
        [
            TarEntry {
                name: b"a/b",
                prefix: b"",
                typeflag: b'0',
                data: b"x",
            },
            TarEntry {
                name: b"a",
                prefix: b"",
                typeflag: b'5',
                data: b"",
            },
        ],
    ] {
        let compatible = ustar(&entries, false);
        assert!(
            inspect_and_plan_ustar_for_flavor(&compatible, None, PathFlavor::Unix, Path::new(""))
                .is_ok()
        );
    }

    for (typeflag, name, expected) in [
        (0, b"nul".as_slice(), PlannedUstarKind::Regular),
        (b'0', b"zero".as_slice(), PlannedUstarKind::Regular),
        (b'5', b"typed".as_slice(), PlannedUstarKind::Directory),
        (b'5', b"typed/".as_slice(), PlannedUstarKind::Directory),
        (b'0', b"implicit/".as_slice(), PlannedUstarKind::Directory),
    ] {
        let typed = ustar(
            &[TarEntry {
                name,
                prefix: b"",
                typeflag,
                data: b"",
            }],
            false,
        );
        let typed =
            inspect_and_plan_ustar_for_flavor(&typed, None, PathFlavor::Unix, Path::new(""))
                .unwrap();
        assert_eq!(typed.entries[0].kind, expected);
    }
    for typeflag in [b'x', b'g', b'L', b'K', b'1', b'2', b'3', b'4', b'6'] {
        for name in [b"bad".as_slice(), b"bad/".as_slice()] {
            let selected = ustar(
                &[TarEntry {
                    name,
                    prefix: b"root",
                    typeflag,
                    data: b"",
                }],
                false,
            );
            let error = inspect_and_plan_ustar_for_flavor(
                &selected,
                Some(&[b"root".to_vec()]),
                PathFlavor::Unix,
                Path::new(""),
            )
            .err()
            .unwrap();
            assert!(error.message.contains("unsupported tar entry type"));
            let outside = ustar(
                &[TarEntry {
                    name,
                    prefix: b"rooted",
                    typeflag,
                    data: b"",
                }],
                false,
            );
            let error = inspect_and_plan_ustar_for_flavor(
                &outside,
                Some(&[b"wanted".to_vec()]),
                PathFlavor::Unix,
                Path::new(""),
            )
            .err()
            .unwrap();
            assert!(error.message.contains("strip_prefix"));
        }
        let prefix_root = ustar(
            &[TarEntry {
                name: b"root",
                prefix: b"",
                typeflag,
                data: b"",
            }],
            false,
        );
        let error = inspect_and_plan_ustar_for_flavor(
            &prefix_root,
            Some(&[b"root".to_vec()]),
            PathFlavor::Unix,
            Path::new(""),
        )
        .err()
        .unwrap();
        assert!(error.message.contains("unsupported tar entry type"));
    }

    let escaping = TarEntry {
        name: b"../escape",
        prefix: b"",
        typeflag: b'0',
        data: b"x",
    };
    let outside_escape = ustar(
        &[
            escaping,
            TarEntry {
                name: b"good",
                prefix: b"wanted",
                typeflag: b'0',
                data: b"good",
            },
        ],
        false,
    );
    assert_eq!(
        inspect_and_plan_ustar_for_flavor(
            &outside_escape,
            Some(&[b"wanted".to_vec()]),
            PathFlavor::Unix,
            Path::new("")
        )
        .unwrap()
        .entries
        .len(),
        1
    );
    let selected_escape = ustar(&[escaping], false);
    assert!(
        inspect_and_plan_ustar_for_flavor(&selected_escape, None, PathFlavor::Unix, Path::new(""))
            .is_err()
    );

    let mut failed_destination = RecordingDestination {
        fail: true,
        ..RecordingDestination::default()
    };
    let error = extract_ustar_plan(&archive, &plan, &mut failed_destination)
        .err()
        .unwrap();
    assert_eq!(error.stage, ArchiveFailureStage::Materialization);
}

#[test]
fn archive_capture_stage_precedence_and_mutation_barrier_are_exact() {
    let archive = ustar(
        &[TarEntry {
            name: b"file",
            prefix: b"",
            typeflag: b'0',
            data: b"captured",
        }],
        false,
    );
    let source = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(source.path(), b"caller").unwrap();
    let url = url::Url::from_file_path(source.path()).unwrap().to_string();
    for (failure, stage, reads) in [
        (
            ScriptedFailure::Root,
            ArchiveFailureStage::Materialization,
            0,
        ),
        (
            ScriptedFailure::Capture,
            ArchiveFailureStage::Materialization,
            0,
        ),
        (ScriptedFailure::Read, ArchiveFailureStage::Transport, 1),
        (ScriptedFailure::Write, ArchiveFailureStage::Transport, 1),
        (ScriptedFailure::Flush, ArchiveFailureStage::Transport, 1),
    ] {
        let mut io = ScriptedArchiveIo::new(archive.clone(), failure);
        let error = materialize_archive_with(&archive_spec(url.clone(), "bad".into()), &mut io)
            .err()
            .unwrap();
        assert_eq!(error.stage, stage);
        assert_eq!(io.reads, reads);
        if let Some(path) = io.root_path {
            assert!(!path.exists());
        }
        if let Some(path) = io.capture_path {
            assert!(!path.exists());
        }
    }
    let mut io = ScriptedArchiveIo::new(archive.clone(), ScriptedFailure::None);
    let error = materialize_archive_with(&archive_spec(url.clone(), "bad".into()), &mut io)
        .err()
        .unwrap();
    assert_eq!(error.stage, ArchiveFailureStage::Spec);
    assert_eq!(io.reads, 1);
    assert!(!io.root_path.unwrap().exists());
    assert!(!io.capture_path.unwrap().exists());

    let mut io = ScriptedArchiveIo::new(archive.clone(), ScriptedFailure::Root);
    let error = materialize_archive_with(&archive_spec("not a URL".into(), "bad".into()), &mut io)
        .err()
        .unwrap();
    assert_eq!(error.stage, ArchiveFailureStage::Spec);
    assert!(io.root_path.is_none());
    assert_eq!(io.reads, 0);
    for prefix in ["", ".."] {
        let mut io = ScriptedArchiveIo::new(archive.clone(), ScriptedFailure::Root);
        let error = materialize_archive_with(
            &archive_spec_with_prefix(url.clone(), "bad".into(), prefix),
            &mut io,
        )
        .err()
        .unwrap();
        assert_eq!(error.stage, ArchiveFailureStage::Spec);
        assert!(io.root_path.is_none());
        assert_eq!(io.reads, 0);
    }

    let mut io = ScriptedArchiveIo::new(archive.clone(), ScriptedFailure::None);
    let error = materialize_archive_with(&archive_spec(url.clone(), "0".repeat(64)), &mut io)
        .err()
        .unwrap();
    assert_eq!(error.stage, ArchiveFailureStage::Transport);

    let malformed = vec![b'x'; 512];
    let malformed_digest = format!("{:x}", Sha256::digest(&malformed));
    let mut io = ScriptedArchiveIo::new(malformed, ScriptedFailure::None);
    let error = materialize_archive_with(&archive_spec(url.clone(), malformed_digest), &mut io)
        .err()
        .unwrap();
    assert_eq!(error.stage, ArchiveFailureStage::Materialization);
    assert!(!io.root_path.unwrap().exists());
    assert!(!io.capture_path.unwrap().exists());

    let mut io = ScriptedArchiveIo::new(vec![b'x'; 512], ScriptedFailure::None);
    let error = materialize_archive_with(&archive_spec(url.clone(), "0".repeat(64)), &mut io)
        .err()
        .unwrap();
    assert_eq!(error.stage, ArchiveFailureStage::Transport);
    assert_eq!(io.destination_calls, 0);

    let late_failure = ustar(
        &[
            TarEntry {
                name: b"early",
                prefix: b"",
                typeflag: b'0',
                data: b"early",
            },
            TarEntry {
                name: b"bad",
                prefix: b"",
                typeflag: b'3',
                data: b"",
            },
        ],
        false,
    );
    let late_digest = format!("{:x}", Sha256::digest(&late_failure));
    let mut io = ScriptedArchiveIo::new(late_failure, ScriptedFailure::None);
    let error = materialize_archive_with(&archive_spec(url.clone(), late_digest), &mut io)
        .err()
        .unwrap();
    assert_eq!(error.stage, ArchiveFailureStage::Materialization);
    assert_eq!(io.destination_calls, 0);
    assert!(!io.root_path.unwrap().exists());
    assert!(!io.capture_path.unwrap().exists());

    let extraction_archive = ustar(
        &[
            TarEntry {
                name: b"file",
                prefix: b"",
                typeflag: b'0',
                data: b"file",
            },
            TarEntry {
                name: b"directory",
                prefix: b"",
                typeflag: b'5',
                data: b"",
            },
        ],
        false,
    );
    let extraction_digest = format!("{:x}", Sha256::digest(&extraction_archive));
    for failure in ["parent", "write", "directory"] {
        let mut io = ScriptedArchiveIo::new(extraction_archive.clone(), ScriptedFailure::None);
        io.destination_failure = Some(failure);
        let error = materialize_archive_with(
            &archive_spec(url.clone(), extraction_digest.clone()),
            &mut io,
        )
        .err()
        .unwrap();
        assert_eq!(error.stage, ArchiveFailureStage::Materialization);
        assert!(io.destination_calls > 0);
        assert!(!io.root_path.unwrap().exists());
        assert!(!io.capture_path.unwrap().exists());
    }

    let digest = format!("{:x}", Sha256::digest(&archive));
    let mut io = ScriptedArchiveIo::new(archive, ScriptedFailure::None);
    io.replace_source = Some(b"changed after capture".to_vec());
    let Materialized::Immutable { bytes, root } =
        materialize_archive_with(&archive_spec(url, digest), &mut io).unwrap()
    else {
        panic!("archive must be immutable");
    };
    assert_eq!(io.reads, 1);
    assert_eq!(bytes.len(), 1024);
    assert_eq!(
        std::fs::read(root.path().join("file")).unwrap(),
        b"captured"
    );

    let source = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(source.path(), b"caller").unwrap();
    let url = url::Url::from_file_path(source.path()).unwrap().to_string();
    let archive = ustar(
        &[TarEntry {
            name: b"file",
            prefix: b"",
            typeflag: b'0',
            data: b"deleted source",
        }],
        false,
    );
    let digest = format!("{:x}", Sha256::digest(&archive));
    let mut io = ScriptedArchiveIo::new(archive, ScriptedFailure::None);
    io.delete_source = true;
    let Materialized::Immutable { root, .. } =
        materialize_archive_with(&archive_spec(url, digest), &mut io).unwrap()
    else {
        panic!("archive must be immutable");
    };
    assert!(!source.path().exists());
    assert_eq!(
        std::fs::read(root.path().join("file")).unwrap(),
        b"deleted source"
    );
}
#[test]
fn archive_requires_the_fixed_tar_shape_and_decodes_file_uris() {
    let source = tempfile::tempdir().unwrap();
    let content = source.path().join("space name");
    std::fs::create_dir(&content).unwrap();
    std::fs::write(content.join("MODULE.bazel"), b"module(name = 'archive')").unwrap();
    let archive = source.path().join("source archive.tar");
    assert!(
        Command::new("tar")
            .args(["--format=ustar", "-cf"])
            .arg(&archive)
            .args(["-C"])
            .arg(source.path())
            .arg("space name")
            .status()
            .unwrap()
            .success()
    );
    let bytes = std::fs::read(&archive).unwrap();
    let digest = format!("{:x}", Sha256::digest(&bytes));
    let url = url::Url::from_file_path(&archive).unwrap().to_string();
    let Materialized::Immutable { root, .. } =
        materialize_local_tar(&archive_spec(url, digest)).unwrap()
    else {
        panic!("archive source must materialize immutably");
    };
    assert_eq!(
        std::fs::read(root.path().join("space name/MODULE.bazel")).unwrap(),
        b"module(name = 'archive')"
    );
}
#[rustfmt::skip]
fn selected_bcr_spec() -> RepoSpec {
    let sri = format!("sha256-{}", base64::engine::general_purpose::STANDARD.encode([7; 32]));
    let empty = || OverrideAttributeValue::Map(Arc::new(SmallMap::new()));
    let attributes: Vec<(CompactString, OverrideAttributeValue)> = vec![
        ("urls".into(), OverrideAttributeValue::Iterable(Arc::new([OverrideAttributeValue::String("https://mirror.test/archive.tar.gz".into()), OverrideAttributeValue::String("https://origin.test/archive.tar.gz".into())]))),
        ("integrity".into(), OverrideAttributeValue::String(sri.clone().into())), ("type".into(), OverrideAttributeValue::String("tar.gz".into())),
        ("strip_prefix".into(), OverrideAttributeValue::String("".into())), ("remote_patches".into(), empty()),
        ("remote_file_urls".into(), empty()), ("remote_file_integrity".into(), empty()),
        ("remote_patch_strip".into(), OverrideAttributeValue::Int(0)),
        ("remote_module_file_urls".into(), OverrideAttributeValue::Iterable(Arc::new([OverrideAttributeValue::String("https://registry.test/MODULE.bazel".into())]))),
        ("remote_module_file_integrity".into(), OverrideAttributeValue::String(sri.into())),
    ];
    RepoSpec { rule_id: RepoRuleId { bzl_file: CanonicalLabel::parse("@@bazel_tools//tools/build_defs/repo:http.bzl").unwrap(), rule_name: "http_archive".into() }, attributes: Arc::new(SmallMap::from_iter(attributes)) }
}

fn selected_bcr_request(
    workspace: &NormalizedAbsolutePath,
    repo: &str,
    mirror: &str,
) -> Arc<RepositoryMaterializationRequest> {
    let mut spec = selected_bcr_spec();
    Arc::make_mut(&mut spec.attributes).insert(
        "urls".into(),
        OverrideAttributeValue::Iterable(Arc::new([
            OverrideAttributeValue::String(format!("https://{mirror}.test/archive.tar.gz").into()),
            OverrideAttributeValue::String("https://origin.test/archive.tar.gz".into()),
        ])),
    );
    Arc::new(RepositoryMaterializationRequest {
        id: RepositoryMaterializationRequestId {
            workspace: workspace.clone(),
            canonical_repo: CanonicalRepoName::new(repo).unwrap(),
        },
        repo_spec: spec,
        kind: RepositoryMaterializationKind::Immutable,
    })
}

fn reject_bcr(key: &str, value: OverrideAttributeValue) {
    let mut spec = selected_bcr_spec();
    Arc::make_mut(&mut spec.attributes).insert(key.into(), value);
    assert!(parse_archive_plan(&spec).is_err(), "invalid {key}");
}

#[test]
fn selected_bcr_plan_is_exact_and_never_falls_back() {
    let ArchivePlan::SelectedBcrTarGz(plan) = parse_archive_plan(&selected_bcr_spec()).unwrap()
    else {
        panic!("selected BCR shape must not fall through to local tar")
    };
    assert_eq!(
        plan.urls.as_ref(),
        [
            "https://mirror.test/archive.tar.gz",
            "https://origin.test/archive.tar.gz"
        ]
    );
    assert_eq!(
        (plan.integrity, plan.module_url, plan.module_integrity),
        (
            [7; 32],
            "https://registry.test/MODULE.bazel".into(),
            [7; 32]
        )
    );

    for key in [
        "urls",
        "integrity",
        "type",
        "strip_prefix",
        "remote_patches",
        "remote_file_urls",
        "remote_file_integrity",
        "remote_patch_strip",
        "remote_module_file_urls",
        "remote_module_file_integrity",
    ] {
        let mut malformed = selected_bcr_spec();
        Arc::make_mut(&mut malformed.attributes).shift_remove(key);
        assert!(parse_archive_plan(&malformed).is_err(), "missing {key}");
    }
    let mut partial = selected_bcr_spec();
    for key in [
        "integrity",
        "strip_prefix",
        "remote_patches",
        "remote_file_urls",
        "remote_file_integrity",
        "remote_patch_strip",
        "remote_module_file_urls",
        "remote_module_file_integrity",
    ] {
        Arc::make_mut(&mut partial.attributes).shift_remove(key);
    }
    assert!(
        parse_archive_plan(&partial).is_err(),
        "tar.gz cannot become local"
    );
    #[rustfmt::skip]
    let invalid = [
        ("urls", OverrideAttributeValue::String("https://origin.test/archive.tar.gz".into())),
        ("urls", OverrideAttributeValue::Iterable(Arc::new([]))),
        ("urls", OverrideAttributeValue::Iterable(Arc::new([OverrideAttributeValue::Int(0)]))),
        ("integrity", OverrideAttributeValue::Iterable(Arc::new([]))),
        ("type", OverrideAttributeValue::String("tar".into())), ("type", OverrideAttributeValue::Int(0)),
        ("strip_prefix", OverrideAttributeValue::String("nonempty".into())), ("strip_prefix", OverrideAttributeValue::Int(0)),
        ("remote_patch_strip", OverrideAttributeValue::Int(1)), ("remote_patch_strip", OverrideAttributeValue::String("0".into())),
        ("remote_module_file_urls", OverrideAttributeValue::String("https://registry.test/MODULE.bazel".into())),
        ("remote_module_file_urls", OverrideAttributeValue::Iterable(Arc::new([]))),
        ("remote_module_file_urls", OverrideAttributeValue::Iterable(Arc::new([OverrideAttributeValue::Int(0)]))),
        ("remote_module_file_urls", OverrideAttributeValue::Iterable(Arc::new([OverrideAttributeValue::String("https://registry.test/a".into()), OverrideAttributeValue::String("https://registry.test/b".into())]))),
        ("urls", OverrideAttributeValue::Iterable(Arc::new([OverrideAttributeValue::String("http://origin.test/archive.tar.gz".into())]))),
        ("remote_module_file_urls", OverrideAttributeValue::Iterable(Arc::new([OverrideAttributeValue::String("http://registry.test/MODULE.bazel".into())]))),
    ];
    for (key, value) in invalid {
        reject_bcr(key, value);
    }
    for key in [
        "remote_patches",
        "remote_file_urls",
        "remote_file_integrity",
    ] {
        reject_bcr(
            key,
            OverrideAttributeValue::Map(Arc::new(SmallMap::from_iter([(
                OverrideAttributeKey::String("file".into()),
                OverrideAttributeValue::String("value".into()),
            )]))),
        );
        reject_bcr(key, OverrideAttributeValue::String("not a map".into()));
    }
    for key in ["integrity", "remote_module_file_integrity"] {
        reject_bcr(key, OverrideAttributeValue::String("not-an-sri".into()));
        reject_bcr(key, OverrideAttributeValue::String("sha256-AA==".into()));
    }
    reject_bcr(
        "sha256",
        OverrideAttributeValue::String("0".repeat(64).into()),
    );

    let mut local_extra = archive_spec("file:///tmp/local.tar".into(), "0".repeat(64));
    Arc::make_mut(&mut local_extra.attributes).insert(
        "unknown_local_attribute".into(),
        OverrideAttributeValue::String("value".into()),
    );
    assert!(matches!(
        parse_archive_plan(&local_extra),
        Ok(ArchivePlan::LocalTar)
    ));
}

#[test]
fn native_selected_bcr_is_generation_scoped_transport_without_success() {
    let workspace_root = tempfile::tempdir().unwrap();
    let workspace = NormalizedAbsolutePath::new(workspace_root.path().to_path_buf()).unwrap();
    let materializer = RepositoryMaterializer::new(workspace.clone());
    let token = materializer.begin().unwrap();
    materializer.preflight_native(token, []).unwrap();
    materializer
        .materialize_native(
            token,
            selected_bcr_request(&workspace, "valid", "mirror"),
            RepositoryMaterializationGeneration(9),
        )
        .unwrap();
    assert!(matches!(
        materializer.active_result_for_test(token, "valid"),
        RepositoryMaterializationResult::TransportError { generation: RepositoryMaterializationGeneration(9), ref message }
            if message.as_str() == "selected-registry BCR archive transport is deferred"
    ));
    materializer.discard(token).unwrap();

    let mut malformed = selected_bcr_spec();
    Arc::make_mut(&mut malformed.attributes).shift_remove("integrity");
    let token = materializer.begin().unwrap();
    materializer.preflight_native(token, []).unwrap();
    materializer
        .materialize_native(
            token,
            Arc::new(RepositoryMaterializationRequest {
                id: RepositoryMaterializationRequestId {
                    workspace: workspace.clone(),
                    canonical_repo: CanonicalRepoName::new("malformed").unwrap(),
                },
                repo_spec: malformed,
                kind: RepositoryMaterializationKind::Immutable,
            }),
            RepositoryMaterializationGeneration(10),
        )
        .unwrap();
    assert!(matches!(
        materializer.active_result_for_test(token, "malformed"),
        RepositoryMaterializationResult::SpecError(_)
    ));
    materializer.discard(token).unwrap();
}

#[test]
fn selected_bcr_capture_has_session_scoped_stage_reuse_recapture_and_stale_ownership() {
    let workspace_root = tempfile::tempdir().unwrap();
    let workspace = NormalizedAbsolutePath::new(workspace_root.path().to_path_buf()).unwrap();
    let materializer = RepositoryMaterializer::new(workspace.clone());
    let captures = Cell::new(0);

    for (mirror, generation) in [("a", 9), ("b", 10), ("a", 11)] {
        let token = materializer.begin().unwrap();
        materializer.preflight_native(token, []).unwrap();
        let request = selected_bcr_request(&workspace, "cycling", mirror);
        let expected_identity: Arc<str> = Arc::from(format!("identity-{mirror}"));
        materializer
            .materialize_selected_bcr_capture_for_test(
                token,
                request.clone(),
                RepositoryMaterializationGeneration(generation),
                |active| {
                    assert!(active());
                    captures.set(captures.get() + 1);
                    Ok(Materialized::AssociatedImmutable {
                        source_identity: expected_identity.clone(),
                        root: tempfile::tempdir().unwrap(),
                    })
                },
            )
            .unwrap();
        assert!(matches!(
            materializer.active_result_for_test(token, "cycling"),
            RepositoryMaterializationResult::Success(
                RepositoryMaterializationSuccess::Immutable {
                    ref source_identity,
                    ..
                }
            ) if source_identity.as_ref() == format!("identity-{mirror}")
        ));
        if generation == 9 {
            materializer
                .materialize_selected_bcr_capture_for_test(
                    token,
                    request,
                    RepositoryMaterializationGeneration(99),
                    |_| panic!("same-session duplicate must reuse the terminal"),
                )
                .unwrap();
        }
        if generation != 11 {
            materializer.discard(token).unwrap();
            continue;
        }

        let replacement = Cell::new(None);
        let stale = selected_bcr_request(&workspace, "stale", "stale");
        let error = materializer
            .materialize_selected_bcr_capture_for_test(
                token,
                stale,
                RepositoryMaterializationGeneration(12),
                |active| {
                    materializer.discard(token).unwrap();
                    let next = materializer.begin().unwrap();
                    materializer.preflight_native(next, []).unwrap();
                    replacement.set(Some(next));
                    assert!(!active());
                    Ok(Materialized::AssociatedImmutable {
                        source_identity: Arc::from("identity-stale"),
                        root: tempfile::tempdir().unwrap(),
                    })
                },
            )
            .unwrap_err();
        assert!(matches!(
            error,
            super::super::repository_io::RepositorySessionError::StaleToken {
                active: Some(_),
                supplied,
            } if supplied == token
        ));
        materializer.discard(replacement.get().unwrap()).unwrap();
    }
    assert_eq!(captures.get(), 3, "changed A/B/A commands must recapture");
}
