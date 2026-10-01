//! Native Solib construction; execution and exact Bazel output identity are deferred.

use std::fmt;

use allocative::Allocative;
use sha2::Digest;
use sha2::Sha256;
use slug_identity_v2::CanonicalLabel;

use crate::ActionOutput;
use crate::ActionOutputKind;
use crate::AnalysisArtifact;
use crate::actions::registry::validate_output_path;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SolibSymlinkError(&'static str);

impl fmt::Display for SolibSymlinkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SolibSymlink {}", self.0)
    }
}

impl std::error::Error for SolibSymlinkError {}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub struct SolibSymlinkSpec {
    input: AnalysisArtifact,
    output: ActionOutput,
}

impl SolibSymlinkSpec {
    /// Naming uses Slug's source/derived paths and canonical consumer spelling,
    /// without Bazel ST mnemonic bytes.
    /// The unchanged artifact, including its complete configured owner, is identity.
    pub fn new(
        input: AnalysisArtifact,
        output_root: crate::DerivedArtifactRoot,
        solib_directory: &str,
        consumer: &CanonicalLabel,
        preserve_name: bool,
        prefix_consumer: bool,
    ) -> Result<Self, SolibSymlinkError> {
        let path = match &input {
            AnalysisArtifact::Source(_) => input.path(),
            AnalysisArtifact::Derived { output, .. } if output.kind() == ActionOutputKind::File => {
                std::borrow::Cow::Borrowed(output.root_relative_path())
            }
            _ => return Err(SolibSymlinkError("requires a regular File input")),
        };
        validate_relative(&path)?;
        validate_relative(solib_directory)?;
        if path.starts_with("_solib_") {
            return Err(SolibSymlinkError("does not accept Solib-derived inputs"));
        }
        let (parent, basename) = path.rsplit_once('/').unwrap_or(("", &path));
        if !shared_library_name(basename) {
            return Err(SolibSymlinkError(
                "input does not match the native shared-library filetypes",
            ));
        }
        let component = if preserve_name {
            escape(&format!("_{parent}"))
        } else {
            Self::slug_native_soname(&path)
        };
        let component = if prefix_consumer {
            format!("{}__{component}", escape(&format!("_{consumer}")))
        } else {
            component
        };
        let component = shorten(&component);
        let filename = if preserve_name {
            format!("{component}/{basename}")
        } else {
            component
        };
        let output = ActionOutput::new(
            output_root,
            format!("{solib_directory}/{filename}"),
            ActionOutputKind::File,
        );
        Ok(Self { input, output })
    }

    pub fn input(&self) -> &AnalysisArtifact {
        &self.input
    }

    /// Unshortened naming over the caller's normalized path. This shares Slug's
    /// existing Solib policy without Bazel configuration ST mnemonic bytes.
    pub fn slug_native_soname(normalized_path: &str) -> String {
        format!("lib{}", escape(normalized_path))
    }

    pub fn output(&self) -> &ActionOutput {
        &self.output
    }
}

fn validate_relative(path: &str) -> Result<(), SolibSymlinkError> {
    if path.contains('\0') || validate_output_path(path).is_err() {
        return Err(SolibSymlinkError("requires canonical relative paths"));
    }
    Ok(())
}

// Bazel 8220c619: Link.SHARED_LIBRARY_FILETYPES; CppFileTypes.java:243–268;
// FileType.NO_EXTENSION. The Java versioned regex uses ASCII digit/word classes.
fn shared_library_name(name: &str) -> bool {
    !name.contains('.')
        || [
            ".so", ".dylib", ".dll", ".pyd", ".wasm", ".tgt", ".vpi", ".ifso", ".tbd", ".lib",
            ".dll.a",
        ]
        .iter()
        .any(|suffix| name.ends_with(suffix))
        || [".so.", ".dylib."].iter().any(|marker| {
            name.rsplit_once(marker).is_some_and(|(prefix, version)| {
                !prefix.is_empty()
                    && !prefix.contains(['\n', '\r', '\u{85}', '\u{2028}', '\u{2029}'])
                    && version.split('.').all(|part| {
                        part.as_bytes().first().is_some_and(u8::is_ascii_digit)
                            && part
                                .bytes()
                                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
                    })
            })
        })
}

// Bazel Actions.java:45–52. Backslash is rejected by the admitted path contract,
// but the scratch escaper keeps the complete pinned substitution table.
fn escape(path: &str) -> String {
    let mut escaped = String::with_capacity(path.len());
    for character in path.chars() {
        match character {
            '_' => escaped.push_str("_U"),
            '/' => escaped.push_str("_S"),
            '\\' => escaped.push_str("_B"),
            ':' => escaped.push_str("_C"),
            '@' => escaped.push_str("_A"),
            other => escaped.push(other),
        }
    }
    escaped
}

// SolibSymlinkAction.java:249–258: Java String.length counts UTF-16 units,
// SHA256 hashes UTF-8, and only the final extension survives long-name hashing.
fn shorten(filename: &str) -> String {
    if filename.encode_utf16().count() <= 255 {
        return filename.to_owned();
    }
    let hash = format!("{:x}", Sha256::digest(filename.as_bytes()));
    match filename.rsplit_once('.') {
        Some((_, extension)) if !extension.is_empty() => format!("{hash}.{extension}"),
        _ => hash,
    }
}

#[cfg(test)]
mod tests {
    use slug_identity_v2::CanonicalLabel;

    use super::*;
    use crate::artifact_root::test_support as root_support;
    fn solib(
        input: AnalysisArtifact,
        directory: &str,
    ) -> Result<SolibSymlinkSpec, SolibSymlinkError> {
        SolibSymlinkSpec::new(
            input,
            root_support::root(),
            directory,
            &CanonicalLabel::parse("@@//:consumer").unwrap(),
            false,
            false,
        )
    }
    use crate::ActionError;
    use crate::ActionKind;
    use crate::ActionSpec;
    use crate::AnalysisConfiguredTargetKey;
    use crate::CtxActions;
    use crate::ReapiCommandProjection;
    use crate::SymlinkSpec;
    use crate::SymlinkTarget;

    fn library(label: &str, configuration: &'static [u8], path: &str) -> AnalysisArtifact {
        AnalysisArtifact::Derived {
            owner: AnalysisConfiguredTargetKey::new(
                CanonicalLabel::parse(label).unwrap(),
                configuration,
            ),
            output: ActionOutput::new(
                root_support::root_for(configuration),
                crate::repository_relative_output_path(
                    CanonicalLabel::parse(label).unwrap().package().repo(),
                    path,
                ),
                ActionOutputKind::File,
            ),
        }
    }

    #[test]
    fn all_flag_branches_preserve_source_generated_and_consumer_coordinates() {
        let consumer = CanonicalLabel::parse("@@//consumer:use").unwrap();
        let other = CanonicalLabel::parse("@@other+//consumer:use").unwrap();
        for (label, parent, soname) in [
            ("@@//pkg:lib_a@b.so", "_Upkg", "libpkg_Slib_Ua_Ab.so"),
            (
                "@@repo+//pkg:lib_a@b.so",
                "_Uexternal_Srepo+_Spkg",
                "libexternal_Srepo+_Spkg_Slib_Ua_Ab.so",
            ),
        ] {
            let source = AnalysisArtifact::Source(CanonicalLabel::parse(label).unwrap());
            let generated = library(label, b"target", "pkg/lib_a@b.so");
            for input in [source.clone(), generated.clone()] {
                let mut actions = CtxActions::new(root_support::root());
                for (preserve, prefix, expected) in [
                    (false, false, format!("_solib_k8/{soname}")),
                    (
                        false,
                        true,
                        format!("_solib_k8/_U_A_A_S_Sconsumer_Cuse__{soname}"),
                    ),
                    (true, false, format!("_solib_k8/{parent}/lib_a@b.so")),
                    (
                        true,
                        true,
                        format!("_solib_k8/_U_A_A_S_Sconsumer_Cuse__{parent}/lib_a@b.so"),
                    ),
                ] {
                    let spec = SolibSymlinkSpec::new(
                        input.clone(),
                        root_support::root(),
                        "_solib_k8",
                        &consumer,
                        preserve,
                        prefix,
                    )
                    .unwrap();
                    assert_eq!(spec.input(), &input);
                    assert_eq!(spec.output().root_relative_path(), expected);
                    let index = actions.registry().actions().len();
                    assert_eq!(actions.register_solib_symlink(spec.clone()).unwrap(), index);
                    let original = actions.registry().clone();
                    assert_eq!(actions.register_solib_symlink(spec.clone()).unwrap(), index);
                    assert_eq!(actions.registry(), &original);
                    let changed_consumer = SolibSymlinkSpec::new(
                        input.clone(),
                        root_support::root(),
                        "_solib_k8",
                        &other,
                        preserve,
                        prefix,
                    )
                    .unwrap();
                    assert_eq!(spec == changed_consumer, !prefix);
                    let counterpart = if input == source {
                        generated.clone()
                    } else {
                        source.clone()
                    };
                    let collision = SolibSymlinkSpec::new(
                        counterpart,
                        root_support::root(),
                        "_solib_k8",
                        &consumer,
                        preserve,
                        prefix,
                    )
                    .unwrap();
                    assert_eq!(collision.output(), spec.output());
                    assert_ne!(collision, spec);
                    assert!(matches!(
                        actions.register_solib_symlink(collision),
                        Err(ActionError::ConflictingOutput { .. })
                    ));
                    assert_eq!(actions.registry(), &original);
                    let identity_changed = match &input {
                        AnalysisArtifact::Source(_) => AnalysisArtifact::Source(
                            CanonicalLabel::parse(&label.replace("//pkg:", "//:pkg/")).unwrap(),
                        ),
                        AnalysisArtifact::Derived { output, .. } => AnalysisArtifact::Derived {
                            owner: AnalysisConfiguredTargetKey::new(
                                CanonicalLabel::parse("@@//pkg:other").unwrap(),
                                b"other-config".as_slice(),
                            ),
                            output: output.clone(),
                        },
                        _ => unreachable!(),
                    };
                    let collision = SolibSymlinkSpec::new(
                        identity_changed,
                        root_support::root(),
                        "_solib_k8",
                        &consumer,
                        preserve,
                        prefix,
                    )
                    .unwrap();
                    assert_eq!(collision.output(), spec.output());
                    assert!(actions.register_solib_symlink(collision).is_err());
                    assert_eq!(actions.registry(), &original);
                }
            }
        }
        let root_source = AnalysisArtifact::Source(CanonicalLabel::parse("@@//:lib.so").unwrap());
        let spec = SolibSymlinkSpec::new(
            root_source,
            root_support::root(),
            "_solib_k8",
            &consumer,
            true,
            false,
        )
        .unwrap();
        assert_eq!(spec.output().root_relative_path(), "_solib_k8/_U/lib.so");
    }

    #[test]
    fn preserved_name_hashes_only_parent_and_consumer_components() {
        let consumer = CanonicalLabel::parse("@@//consumer:use").unwrap();
        for (parent, prefix, expected) in [
            (
                "a".repeat(254),
                false,
                "a51bb45cb9c10bcb8687a2ad686ba64f00708301cd310dce2c4b4435c3a16c49",
            ),
            (
                "a".repeat(254),
                true,
                "3f527409d2b54509d1ff45c0cf14ea8233010cd462fbcdfc0e90f530736d24b7",
            ),
            (
                format!("{}.dir", "a".repeat(251)),
                false,
                "d82f41afd19135bd4d8663c62765c940c1e5d01285fe1f8468ca23badd90b949.dir",
            ),
            (
                "😀".repeat(127),
                false,
                "30a83c1339b8dbf085e4f539d95f48d1b7c91558af04c529957051d82c5bc987",
            ),
        ] {
            let spec = SolibSymlinkSpec::new(
                library("@@//:lib", b"target", &format!("{parent}/lib_a.so.12")),
                root_support::root(),
                "_solib_k8",
                &consumer,
                true,
                prefix,
            )
            .unwrap();
            assert_eq!(
                spec.output().root_relative_path(),
                format!("_solib_k8/{expected}/lib_a.so.12")
            );
        }
        let parent = format!("{}a", "😀".repeat(126));
        assert_eq!(format!("_U{parent}").encode_utf16().count(), 255);
        let spec = SolibSymlinkSpec::new(
            library("@@//:lib", b"target", &format!("{parent}/lib.so")),
            root_support::root(),
            "_solib_k8",
            &consumer,
            true,
            false,
        )
        .unwrap();
        assert_eq!(
            spec.output().root_relative_path(),
            format!("_solib_k8/_U{parent}/lib.so")
        );
        // Basename preservation does not run the long-name hash on the library itself.
        let basename = format!("{}.so", "a".repeat(256));
        let spec = SolibSymlinkSpec::new(
            library("@@//:lib", b"target", &format!("pkg/{basename}")),
            root_support::root(),
            "_solib_k8",
            &consumer,
            true,
            false,
        )
        .unwrap();
        assert_eq!(
            spec.output().root_relative_path(),
            format!("_solib_k8/_Upkg/{basename}")
        );
    }

    #[test]
    fn naming_preserves_repository_and_full_configured_input_identity() {
        let input = library("@@//pkg:lib", b"target", "pkg/lib_a@b:c.so");
        let spec = solib(input.clone(), "_solib_k8").unwrap();
        assert_eq!(spec.input(), &input);
        assert_eq!(
            spec.output().root_relative_path(),
            "_solib_k8/libpkg_Slib_Ua_Ab_Cc.so"
        );
        let external = solib(
            library(
                "@@rules_rust+//ffi/cc/allocator_library:cc_allocator_library",
                b"target",
                "ffi/cc/allocator_library/libcc_allocator_library.so",
            ),
            "_solib_k8",
        )
        .unwrap();
        assert_eq!(
            external.output().root_relative_path(),
            "_solib_k8/libexternal_Srules_Urust+_Sffi_Scc_Sallocator_Ulibrary_Slibcc_Uallocator_Ulibrary.so"
        );
        let different_config = solib(
            library("@@//pkg:lib", b"exec", "pkg/lib_a@b:c.so"),
            "_solib_k8",
        )
        .unwrap();
        assert_eq!(spec.output(), different_config.output());
        assert_ne!(spec, different_config);
        let different_owner = solib(
            library("@@//pkg:other", b"target", "pkg/lib_a@b:c.so"),
            "_solib_k8",
        )
        .unwrap();
        assert_eq!(spec.output(), different_owner.output());
        assert_ne!(spec, different_owner);
        let different_repo = solib(
            library("@@other+//pkg:lib", b"target", "pkg/lib_a@b:c.so"),
            "_solib_k8",
        )
        .unwrap();
        assert_ne!(spec.output(), different_repo.output());
        assert_eq!(escape("_/:@\\"), "_U_S_C_A_B");
        assert_eq!(
            SolibSymlinkSpec::slug_native_soname("_/:@\\"),
            "lib_U_S_C_A_B"
        );
    }

    #[test]
    fn long_names_count_utf16_and_hash_utf8_preserving_final_extension() {
        let path = format!("{}.so", "a".repeat(256));
        assert_eq!(
            SolibSymlinkSpec::slug_native_soname(&path),
            format!("lib{path}")
        );
        let exactly = format!("lib{}", "a".repeat(252));
        assert_eq!(shorten(&exactly), exactly);
        assert_eq!(
            shorten(&format!("lib{}", "a".repeat(253))),
            "4d646660cb92382e6cb02964bf8a8c97674713b8a471a677865e7677f0d8a28e"
        );
        assert_eq!(
            shorten(&format!("lib{}.so.12", "a".repeat(250))),
            "eb3e3122e27c9cf56628fd24413a505917bfd5829ae6461beb724821660fb0fa.12"
        );
        let unicode_short = format!("lib{}a.so", "😀".repeat(124));
        assert_eq!(unicode_short.encode_utf16().count(), 255);
        assert!(unicode_short.len() > 255);
        assert_eq!(shorten(&unicode_short), unicode_short);
        assert_eq!(
            shorten(&format!("lib{}.so", "😀".repeat(125))),
            "46330948c71219f496b42336b7cab6fae0b2c212c3cbb43d4fc9d8f819159c81.so"
        );
        // Count after escaping, and preserve an empty extension exactly as Guava does.
        assert_eq!(shorten(&format!("{}.", "a".repeat(255))).len(), 64);
        let spec = solib(
            library("@@//:lib", b"target", &format!("{}.so", "_".repeat(125))),
            "_solib_k8",
        )
        .unwrap();
        assert_eq!(
            spec.output()
                .root_relative_path()
                .rsplit('/')
                .next()
                .unwrap()
                .len(),
            67
        );
    }

    #[test]
    fn native_filetypes_and_admitted_paths_reject_unsupported_inputs() {
        for name in [
            "library",
            "lib.so",
            "lib.dylib",
            "lib.dll",
            "lib.pyd",
            "lib.wasm",
            "lib.tgt",
            "lib.vpi",
            "lib.ifso",
            "lib.tbd",
            "lib.lib",
            "lib.dll.a",
            "lib.so.2.10",
            "lib.dylib.1a_b35",
            "lib.so.0_",
            ".so",
        ] {
            assert!(
                solib(library("@@//:lib", b"target", name), "_solib_k8").is_ok(),
                "{name}"
            );
        }
        for name in [
            "lib.a",
            "lib.o",
            ".hidden",
            "lib.",
            "lib.so.a",
            "lib.so.1-2",
            "lib.so.1é",
            "lib.so.١",
            "lib.so.1.",
            ".so.1",
            "a\n.so.1",
        ] {
            assert!(
                solib(library("@@//:lib", b"target", name), "_solib_k8").is_err(),
                "{name}"
            );
        }
        for path in [
            "",
            "/lib.so",
            "pkg//lib.so",
            "../lib.so",
            "pkg/./lib.so",
            "pkg\\lib.so",
            "pkg/\0lib.so",
            "_solib_k8/lib.so",
        ] {
            assert!(
                solib(library("@@//:lib", b"target", path), "_solib_k8").is_err(),
                "{path:?}"
            );
        }
        let input = library("@@//:lib", b"target", "lib.so");
        for directory in [
            "",
            "/_solib_k8",
            "../_solib_k8",
            "a//b",
            "a/./b",
            "a\\b",
            "a\0b",
        ] {
            assert!(solib(input.clone(), directory).is_err(), "{directory:?}");
        }
        assert!(
            solib(
                AnalysisArtifact::Source(CanonicalLabel::parse("@@//:lib.so").unwrap()),
                "_solib_k8"
            )
            .is_ok()
        );
        let status = crate::WorkspaceStatusArtifacts::new(
            slug_workspace_v2::NormalizedAbsolutePath::new("/workspace").unwrap(),
        );
        assert!(
            solib(
                AnalysisArtifact::WorkspaceStatus(status.stable().clone()),
                "_solib_k8"
            )
            .is_err()
        );
        for label in ["@@//:lib.a", "@@//:_solib_k8/lib.so"] {
            assert!(
                solib(
                    AnalysisArtifact::Source(CanonicalLabel::parse(label).unwrap()),
                    "_solib_k8"
                )
                .is_err()
            );
        }
        for kind in [
            ActionOutputKind::Directory,
            ActionOutputKind::Symlink,
            ActionOutputKind::RunfilesTree,
        ] {
            let AnalysisArtifact::Derived { owner, .. } = input.clone() else {
                unreachable!()
            };
            assert!(
                solib(
                    AnalysisArtifact::Derived {
                        owner,
                        output: ActionOutput::new(root_support::root(), "lib.so", kind),
                    },
                    "_solib_k8"
                )
                .is_err()
            );
        }
    }

    #[test]
    fn native_registration_is_idempotent_but_never_aliases_other_inputs_or_families() {
        let input = library("@@//pkg:lib", b"target", "pkg/lib.so");
        let spec = solib(input.clone(), "_solib_k8").unwrap();
        let mut actions = CtxActions::new(root_support::root());
        assert_eq!(actions.register_solib_symlink(spec.clone()).unwrap(), 0);
        assert_eq!(actions.register_solib_symlink(spec.clone()).unwrap(), 0);
        assert_eq!(actions.registry().actions().len(), 1);
        let original = actions.registry().clone();
        let changed = solib(library("@@//pkg:lib", b"exec", "pkg/lib.so"), "_solib_k8").unwrap();
        assert!(matches!(
            actions.register_solib_symlink(changed),
            Err(ActionError::ConflictingOutput { .. })
        ));
        assert_eq!(actions.registry(), &original);
        let AnalysisArtifact::Derived { owner, output } = input.clone() else {
            unreachable!()
        };
        let constant_input = AnalysisArtifact::Derived {
            owner,
            output: ActionOutput::new_constant(output.root().clone(), output.root_relative_path()),
        };
        let changed = solib(constant_input, "_solib_k8").unwrap();
        assert_eq!(changed.output(), spec.output());
        assert!(actions.register_solib_symlink(changed).is_err());
        assert_eq!(actions.registry(), &original);
        let ordinary = SymlinkSpec::new(
            spec.output().clone(),
            SymlinkTarget::Artifact {
                input,
                require_executable: false,
                use_exec_root_for_source: false,
            },
            None::<String>,
        );
        assert!(actions.register_symlink(ordinary.clone()).is_err());
        assert_eq!(actions.registry(), &original);
        let mut ordinary_first = CtxActions::new(root_support::root());
        ordinary_first.register_symlink(ordinary).unwrap();
        let ordinary_original = ordinary_first.registry().clone();
        assert!(ordinary_first.register_solib_symlink(spec.clone()).is_err());
        assert_eq!(ordinary_first.registry(), &ordinary_original);
        let native = &actions.registry().actions()[0];
        assert_eq!(native.kind(), &ActionKind::SolibSymlink);
        assert_eq!(native.mnemonic(), "SolibSymlink");
        assert_eq!(native.solib_symlink_spec(), Some(&spec));
        assert!(native.symlink_spec().is_none());
        assert!(native.progress_message().is_none());
        assert!(native.exec_group().is_none());
        assert!(native.exec_properties().is_empty());
        assert!(native.execution_requirements().is_empty());
        assert_eq!(
            ReapiCommandProjection::from_action(native).unwrap_err(),
            "native SolibSymlink REAPI projection is not admitted"
        );
        let mut decorated = CtxActions::new(root_support::root());
        decorated
            .register_batch([ActionSpec::solib_symlink(spec.clone()).with_exec_group("group")])
            .unwrap();
        let decorated_original = decorated.registry().clone();
        assert!(decorated.register_solib_symlink(spec.clone()).is_err());
        assert_eq!(decorated.registry(), &decorated_original);
        let mut decorated = CtxActions::new(root_support::root());
        decorated
            .register_batch([
                ActionSpec::solib_symlink(spec.clone()).with_exec_properties(
                    std::collections::BTreeMap::from([("key".to_owned(), "value".to_owned())]),
                ),
            ])
            .unwrap();
        let decorated_original = decorated.registry().clone();
        assert!(decorated.register_solib_symlink(spec).is_err());
        assert_eq!(decorated.registry(), &decorated_original);
    }
}
