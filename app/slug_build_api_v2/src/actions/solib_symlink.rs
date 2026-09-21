//! Native Solib construction; execution and exact Bazel output identity are deferred.

use std::fmt;

use allocative::Allocative;
use sha2::Digest;
use sha2::Sha256;

use crate::ActionOutput;
use crate::ActionOutputKind;
use crate::AnalysisArtifact;
use crate::actions::registry::validate_output;

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
    /// The admitted `preserve_name=False, prefix_consumer=False` constructor.
    /// Naming uses Slug's ordinary derived paths, without Bazel ST mnemonic bytes.
    /// The unchanged artifact, including its complete configured owner, is identity.
    pub fn new(input: AnalysisArtifact, solib_directory: &str) -> Result<Self, SolibSymlinkError> {
        let AnalysisArtifact::Derived { owner, output } = &input else {
            return Err(SolibSymlinkError("requires a generated regular File input"));
        };
        if output.kind() != ActionOutputKind::File {
            return Err(SolibSymlinkError("requires a generated regular File input"));
        }
        validate_relative(output.path())?;
        validate_relative(solib_directory)?;
        if output.path().starts_with("_solib_") {
            return Err(SolibSymlinkError("does not accept Solib-derived inputs"));
        }
        let basename = output.path().rsplit('/').next().unwrap();
        if !shared_library_name(basename) {
            return Err(SolibSymlinkError(
                "input does not match the native shared-library filetypes",
            ));
        }
        let repository = owner.label().package().repo();
        let naming_path = if repository.is_root() {
            output.path().to_owned()
        } else {
            format!(
                "external/{}/{path}",
                repository.as_str(),
                path = output.path()
            )
        };
        let filename = shorten(&format!("lib{}", escape(&naming_path)));
        let output = ActionOutput::new(
            format!("{solib_directory}/{filename}"),
            ActionOutputKind::File,
        );
        Ok(Self { input, output })
    }

    pub fn input(&self) -> &AnalysisArtifact {
        &self.input
    }

    pub fn output(&self) -> &ActionOutput {
        &self.output
    }
}

fn validate_relative(path: &str) -> Result<(), SolibSymlinkError> {
    if path.contains('\0')
        || validate_output(&ActionOutput::new(path, ActionOutputKind::File)).is_err()
    {
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
            output: ActionOutput::new(path, ActionOutputKind::File),
        }
    }

    #[test]
    fn naming_preserves_repository_and_full_configured_input_identity() {
        let input = library("@@//pkg:lib", b"target", "pkg/lib_a@b:c.so");
        let spec = SolibSymlinkSpec::new(input.clone(), "_solib_k8").unwrap();
        assert_eq!(spec.input(), &input);
        assert_eq!(spec.output().path(), "_solib_k8/libpkg_Slib_Ua_Ab_Cc.so");
        let external = SolibSymlinkSpec::new(
            library(
                "@@rules_rust+//ffi/cc/allocator_library:cc_allocator_library",
                b"target",
                "ffi/cc/allocator_library/libcc_allocator_library.so",
            ),
            "_solib_k8",
        )
        .unwrap();
        assert_eq!(
            external.output().path(),
            "_solib_k8/libexternal_Srules_Urust+_Sffi_Scc_Sallocator_Ulibrary_Slibcc_Uallocator_Ulibrary.so"
        );
        let different_config = SolibSymlinkSpec::new(
            library("@@//pkg:lib", b"exec", "pkg/lib_a@b:c.so"),
            "_solib_k8",
        )
        .unwrap();
        assert_eq!(spec.output(), different_config.output());
        assert_ne!(spec, different_config);
        let different_owner = SolibSymlinkSpec::new(
            library("@@//pkg:other", b"target", "pkg/lib_a@b:c.so"),
            "_solib_k8",
        )
        .unwrap();
        assert_eq!(spec.output(), different_owner.output());
        assert_ne!(spec, different_owner);
        let different_repo = SolibSymlinkSpec::new(
            library("@@other+//pkg:lib", b"target", "pkg/lib_a@b:c.so"),
            "_solib_k8",
        )
        .unwrap();
        assert_ne!(spec.output(), different_repo.output());
        assert_eq!(escape("_/:@\\"), "_U_S_C_A_B");
    }

    #[test]
    fn long_names_count_utf16_and_hash_utf8_preserving_final_extension() {
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
        let spec = SolibSymlinkSpec::new(
            library("@@//:lib", b"target", &format!("{}.so", "_".repeat(125))),
            "_solib_k8",
        )
        .unwrap();
        assert_eq!(spec.output().path().rsplit('/').next().unwrap().len(), 67);
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
                SolibSymlinkSpec::new(library("@@//:lib", b"target", name), "_solib_k8").is_ok(),
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
                SolibSymlinkSpec::new(library("@@//:lib", b"target", name), "_solib_k8").is_err(),
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
                SolibSymlinkSpec::new(library("@@//:lib", b"target", path), "_solib_k8").is_err(),
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
            assert!(
                SolibSymlinkSpec::new(input.clone(), directory).is_err(),
                "{directory:?}"
            );
        }
        assert!(
            SolibSymlinkSpec::new(
                AnalysisArtifact::Source(CanonicalLabel::parse("@@//:lib.so").unwrap()),
                "_solib_k8"
            )
            .is_err()
        );
        for kind in [
            ActionOutputKind::Directory,
            ActionOutputKind::Symlink,
            ActionOutputKind::RunfilesTree,
        ] {
            let AnalysisArtifact::Derived { owner, .. } = input.clone() else {
                unreachable!()
            };
            assert!(
                SolibSymlinkSpec::new(
                    AnalysisArtifact::Derived {
                        owner,
                        output: ActionOutput::new("lib.so", kind),
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
        let spec = SolibSymlinkSpec::new(input.clone(), "_solib_k8").unwrap();
        let mut actions = CtxActions::new();
        assert_eq!(actions.register_solib_symlink(spec.clone()).unwrap(), 0);
        assert_eq!(actions.register_solib_symlink(spec.clone()).unwrap(), 0);
        assert_eq!(actions.registry().actions().len(), 1);
        let changed =
            SolibSymlinkSpec::new(library("@@//pkg:lib", b"exec", "pkg/lib.so"), "_solib_k8")
                .unwrap();
        assert!(matches!(
            actions.register_solib_symlink(changed),
            Err(ActionError::ConflictingOutput { .. })
        ));
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
        let mut ordinary_first = CtxActions::new();
        ordinary_first.register_symlink(ordinary).unwrap();
        assert!(ordinary_first.register_solib_symlink(spec.clone()).is_err());
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
        let mut decorated = CtxActions::new();
        decorated
            .register_batch([ActionSpec::solib_symlink(spec.clone()).with_exec_group("group")])
            .unwrap();
        assert!(decorated.register_solib_symlink(spec).is_err());
    }
}
