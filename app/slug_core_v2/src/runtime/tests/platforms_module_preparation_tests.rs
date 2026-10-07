use std::mem::Discriminant;
use std::os::unix::ffi::OsStrExt;
use std::sync::Weak;

use sha2::Digest;
use slug_bzlmod_v2::HostRootRepositoryMappingObservationKey;
use slug_bzlmod_v2::ObservedHostRootRepositoryMapping;
use slug_bzlmod_v2::OverrideAttributeValue;
use slug_bzlmod_v2::RepositoryMaterializationKind;
use slug_bzlmod_v2::RootModuleCommandPolicyKey;
use slug_bzlmod_v2::RootModuleLoadingAnchorObservationKey;
use slug_bzlmod_v2::RootModuleRegistryUrls;
use slug_bzlmod_v2::RootModuleRegistryUrlsKey;
use slug_bzlmod_v2::SelectedSourceRefusal;
use slug_bzlmod_v2::SourcePreparationNeeds;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_workspace_v2::NeedPathObservations;
use slug_workspace_v2::ObservedPathFrontierError;

use super::*;

mod public_probe_preparation_tests {
    include!("platforms_module_preparation_tests/public_probe_preparation_tests.rs");
}
use public_probe_preparation_tests::CommandChoice;
use public_probe_preparation_tests::ObservationOutcome;
use public_probe_preparation_tests::ProbeReport;

const PLATFORMS_SHA: [u8; 32] = [
    0x1c, 0x0c, 0x09, 0xf5, 0xbd, 0xcf, 0x4b, 0x3f, 0x92, 0x47, 0x20, 0xd2, 0x47, 0x8a, 0x37, 0x11,
    0xcb, 0x39, 0xf4, 0x97, 0x70, 0x19, 0xca, 0x59, 0x88, 0x68, 0x5e, 0x5b, 0x7e, 0x18, 0xb3, 0xd2,
];
fn platforms_bytes(bytes: &[u8]) -> bool {
    bytes.len() == 416 && sha2::Sha256::digest(bytes).as_slice() == PLATFORMS_SHA
}

const ROOT_SHA: [u8; 32] = [
    0xa3, 0x57, 0x13, 0x30, 0x30, 0xb9, 0x43, 0x60, 0xb3, 0x70, 0x61, 0xf3, 0x80, 0x98, 0x4c, 0xaa,
    0x37, 0x4f, 0x73, 0x83, 0x84, 0x1f, 0xab, 0xdc, 0x8c, 0x5d, 0x26, 0xbc, 0x11, 0x4e, 0x3b, 0x10,
];
fn root_bytes(bytes: &[u8]) -> bool {
    bytes.len() == 426 && sha2::Sha256::digest(bytes).as_slice() == ROOT_SHA
}

const FEATURES_SHA: [u8; 32] = [
    0x20, 0x83, 0xef, 0x9c, 0x7a, 0x46, 0x9f, 0x52, 0x08, 0x90, 0x48, 0x3c, 0xcf, 0x8e, 0x01, 0x89,
    0xd6, 0xe7, 0x1e, 0x21, 0x17, 0xe7, 0x75, 0x2e, 0x15, 0xe6, 0x55, 0x44, 0x33, 0xd5, 0xae, 0x3e,
];
const RULES_SHA: [u8; 32] = [
    0x25, 0xe3, 0xb0, 0x77, 0x12, 0x86, 0x12, 0x75, 0x4c, 0x4a, 0xdd, 0x1b, 0x4c, 0x90, 0xd2, 0x0a,
    0x6b, 0xe0, 0x65, 0x66, 0xb6, 0x23, 0xde, 0xe6, 0xe3, 0x20, 0x38, 0xd0, 0xe8, 0xf9, 0x30, 0x62,
];
const SKYLIB_SHA: [u8; 32] = [
    0x69, 0xad, 0x69, 0x27, 0x09, 0x83, 0x16, 0x84, 0x8b, 0x34, 0xa9, 0x14, 0x2b, 0xcc, 0x97, 0x5e,
    0x01, 0x8b, 0xa2, 0x7f, 0x08, 0xc4, 0xff, 0x40, 0x3f, 0x50, 0xc1, 0xb6, 0xe6, 0x46, 0xca, 0x67,
];
// This exact local body declares 0.2.17; it is not the prior registry 0.2.4 source.
const CC_SHA: [u8; 32] = [
    0x18, 0x49, 0x60, 0x2c, 0x86, 0xcb, 0x60, 0xda, 0x86, 0x13, 0xd2, 0xde, 0x88, 0x7f, 0x95, 0x66,
    0xa6, 0xd3, 0x54, 0xa6, 0xdf, 0x6d, 0x70, 0x09, 0xf9, 0xd0, 0x4a, 0x14, 0x40, 0x2f, 0x9a, 0x84,
];
const LICENSE_SHA: [u8; 32] = [
    0xa7, 0xfd, 0xa6, 0x0e, 0xef, 0xdf, 0x3d, 0x8c, 0x82, 0x72, 0x62, 0xba, 0x49, 0x99, 0x57, 0xe4,
    0xdf, 0x06, 0xf6, 0x59, 0x33, 0x0b, 0xbe, 0x6c, 0xdb, 0xdb, 0x97, 0x5b, 0x76, 0x8b, 0xb6, 0x5c,
];
const SHELL_SHA: [u8; 32] = [
    0x72, 0xe7, 0x6b, 0x0e, 0xea, 0x4e, 0x81, 0x61, 0x1e, 0xf5, 0x45, 0x2a, 0xa8, 0x2b, 0x3d, 0xa3,
    0x4c, 0xac, 0xa0, 0xc8, 0xb7, 0xb5, 0xc0, 0xc9, 0x58, 0x43, 0x38, 0xaa, 0x93, 0xba, 0xe2, 0x6b,
];
// This Local body declares 1.24.2 and repo_name build_bazel_apple_support, not registry 1.24.1.
const APPLE_SUPPORT_SHA: [u8; 32] = [
    0x0e, 0x62, 0x47, 0x18, 0x18, 0xaf, 0xfb, 0x9f, 0x0b, 0x26, 0xf1, 0x28, 0x83, 0x1d, 0x5c, 0x40,
    0xb0, 0x74, 0xd3, 0x2e, 0x6d, 0xda, 0x5a, 0x0d, 0x38, 0x52, 0x84, 0x72, 0x15, 0xa4, 0x1c, 0xa4,
];
const PACKAGE_METADATA_SHA: [u8; 32] = [
    0x77, 0x89, 0x05, 0x52, 0xec, 0xea, 0x9e, 0x28, 0x4b, 0x54, 0x24, 0xc9, 0xde, 0x82, 0x7a, 0x58,
    0x09, 0x93, 0x48, 0x76, 0x3a, 0x43, 0x59, 0xe9, 0x75, 0xc3, 0x59, 0xa8, 0x3d, 0x4f, 0xaa, 0x83,
];
const BUILDOZER_SHA: [u8; 32] = [
    0xa3, 0x5d, 0x95, 0x61, 0xb3, 0xfc, 0x5b, 0x18, 0x79, 0x7c, 0x33, 0x07, 0x93, 0xe9, 0x9e, 0x3b,
    0x83, 0x4a, 0x47, 0x3d, 0x5f, 0xbd, 0x3d, 0x7d, 0x76, 0x34, 0xaa, 0xfc, 0x9b, 0xdb, 0x6f, 0x8f,
];
const ADDITIONAL_IDENTITIES: [(&str, &str); 8] = [
    ("bazel_features", "bazel_features+"),
    ("bazel_skylib", "bazel_skylib+"),
    ("rules_cc", "rules_cc+"),
    ("rules_license", "rules_license+"),
    ("rules_shell", "rules_shell+"),
    ("apple_support", "apple_support+"),
    ("package_metadata", "package_metadata+"),
    ("buildozer", "buildozer+"),
];
const ADDITIONAL_ENV: [&str; 8] = [
    "SLUG_NATIVE_BAZEL_FEATURES_ROOT",
    "SLUG_NATIVE_BAZEL_SKYLIB_ROOT",
    "SLUG_NATIVE_RULES_CC_ROOT",
    "SLUG_NATIVE_RULES_LICENSE_ROOT",
    "SLUG_NATIVE_RULES_SHELL_ROOT",
    "SLUG_NATIVE_APPLE_SUPPORT_ROOT",
    "SLUG_NATIVE_PACKAGE_METADATA_ROOT",
    "SLUG_NATIVE_BUILDOZER_ROOT",
];
const ADDITIONAL_MARKERS: [(&str, &str); 8] = [
    ("WP945", "bazel_features_root"),
    ("WP946", "bazel_skylib_root"),
    ("WP947", "rules_cc_root"),
    ("WP948", "rules_license_root"),
    ("WP949", "rules_shell_root"),
    ("WP950", "apple_support_root"),
    ("WP951", "package_metadata_root"),
    ("WP952", "buildozer_root"),
];
fn additional_bytes(slot: usize, bytes: &[u8]) -> bool {
    let (size, hash) = match slot {
        0 => (346, FEATURES_SHA),
        1 => (389, SKYLIB_SHA),
        2 => (1181, CC_SHA),
        3 => (753, LICENSE_SHA),
        4 => (485, SHELL_SHA),
        5 => (1104, APPLE_SUPPORT_SHA),
        6 => (235, PACKAGE_METADATA_SHA),
        7 => (1122, BUILDOZER_SHA),
        _ => unreachable!(),
    };
    bytes.len() == size && sha2::Sha256::digest(bytes).as_slice() == hash
}
#[derive(Clone, Copy)]
enum ModuleMode {
    Three,
    Four,
    Five,
    Six,
    Seven,
    Eight,
    Nine,
    Ten,
    Eleven,
}
impl ModuleMode {
    fn additional_count(self) -> usize {
        match self {
            Self::Three => 0,
            Self::Four => 1,
            Self::Five => 2,
            Self::Six => 3,
            Self::Seven => 4,
            Self::Eight => 5,
            Self::Nine => 6,
            Self::Ten => 7,
            Self::Eleven => 8,
        }
    }
}
#[derive(Clone, Copy)]
enum RegistryChoice {
    DefaultBcr,
    PreparedFile,
}
impl RegistryChoice {
    fn registry_attempts(self) -> u64 {
        match self {
            Self::DefaultBcr => 16,
            Self::PreparedFile => 128,
        }
    }
    fn urls(self, prepared: impl FnOnce() -> RegistryUrls) -> RegistryUrls {
        match self {
            Self::DefaultBcr => RegistryUrls::new(["https://bcr.bazel.build/"]),
            Self::PreparedFile => prepared(),
        }
    }
}
fn prepared_file_registry(spelling: &str, expected: &Path) -> Result<RegistryUrls, ()> {
    if spelling.len() > 1024
        || !spelling
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/._-".contains(&b))
    {
        return Err(());
    }
    let root = NormalizedAbsolutePath::new(Path::new(spelling)).map_err(|_| ())?;
    if root.as_path().as_os_str() != Path::new(spelling).as_os_str() || root.as_path() != expected {
        return Err(());
    }
    RegistryUrls::from_request(expected, &[format!("file://{spelling}")]).map_err(|_| ())
}
fn registry_policy_matches(actual: &RootModuleRegistryUrls, expected: &RegistryUrls) -> bool {
    actual == &RootModuleRegistryUrls::from(expected.dupe())
}
#[test]
fn prepared_file_registry_policy_is_opt_in_and_exact() {
    assert_eq!(RegistryChoice::DefaultBcr.registry_attempts(), 16);
    assert_eq!(RegistryChoice::PreparedFile.registry_attempts(), 128);
    let expected = Path::new("/task/registry");
    let file = prepared_file_registry("/task/registry", expected).unwrap();
    assert_eq!(file.as_slice().len(), 1);
    assert_eq!(file.as_slice()[0].as_str(), "file:///task/registry");
    let bcr = RegistryChoice::DefaultBcr.urls(|| panic!("default must not obtain prepared root"));
    assert_eq!(bcr.as_slice().len(), 1);
    assert_eq!(bcr.as_slice()[0].as_str(), "https://bcr.bazel.build");
    assert!(registry_policy_matches(
        &RootModuleRegistryUrls::from(file.dupe()),
        &file
    ));
    for wrong in [
        RegistryUrls::new(["file:///other/registry"]),
        bcr,
        RegistryUrls::new(["file:///task/registry", "file:///other/registry"]),
        RegistryUrls::new(std::iter::empty::<&str>()),
    ] {
        assert!(!registry_policy_matches(
            &RootModuleRegistryUrls::from(wrong),
            &file
        ));
    }
    for spelling in [
        "task/registry",
        "/task/../task/registry",
        "/task/registry/",
        "/task/other",
        "/task/reg istry",
        "/task/registry?",
        "/task/registry#",
        "/task/%workspace%",
        "",
    ] {
        assert!(prepared_file_registry(spelling, expected).is_err());
    }
    assert!(prepared_file_registry(&"a".repeat(1025), expected).is_err());
}
struct AdditionalModule {
    root: NormalizedAbsolutePath,
    file: NormalizedAbsolutePath,
}
// Finite imports declared by the observed defs/toolchain dependency family.
const RULES_RUST_DEFS_IMPORTS: [&str; 28] = [
    "rust/toolchain.bzl",
    "rust/private/clippy.bzl",
    "rust/private/common.bzl",
    "rust/private/lints.bzl",
    "rust/private/rust.bzl",
    "rust/private/rust_analyzer.bzl",
    "rust/private/rustc.bzl",
    "rust/private/rustdoc.bzl",
    "rust/private/rustdoc_test.bzl",
    "rust/private/rustfmt.bzl",
    "rust/private/unpretty.bzl",
    "rust/rust_analyzer_toolchain.bzl",
    "rust/rust_stdlib_filegroup.bzl",
    "rust/rust_toolchain.bzl",
    "rust/rustfmt_toolchain.bzl",
    "rust/platform/triple_mappings.bzl",
    "rust/platform/triple.bzl",
    "rust/private/lint_test.bzl",
    "rust/private/providers.bzl",
    "rust/private/utils.bzl",
    "rust/settings/incompatible.bzl",
    "rust/private/rust_allocator_libraries.bzl",
    "rust/private/pic_utils.bzl",
    "rust/private/toolchain.bzl",
    "rust/private/lto.bzl",
    "rust/private/semver.bzl",
    "rust/private/rustc_resource_set.bzl",
    "rust/private/stamp.bzl",
];
const RULES_RUST_IMPORT_METADATA: [&str; 3] = ["rust/private", "rust/platform", "rust/settings"];
const BAZEL_FEATURES_IMPORTS: [&str; 8] = [
    "features.bzl",
    "private/util.bzl",
    "private/parse.bzl",
    "private/extensions.bzl",
    "private/repos.bzl",
    "private/globals.bzl",
    "private/globals_repo.bzl",
    "private/version_repo.bzl",
];
const BAZEL_FEATURES_IMPORT_METADATA: [&str; 1] = ["private"];
const BAZEL_SKYLIB_IMPORTS: [&str; 4] = [
    "lib/paths.bzl",
    "lib/structs.bzl",
    "rules/common_settings.bzl",
    "lib/modules.bzl",
];
const BAZEL_SKYLIB_IMPORT_METADATA: [&str; 2] = ["lib", "rules"];
const RULES_CC_IMPORTS: [&str; 8] = [
    "cc/find_cc_toolchain.bzl",
    "cc/common/cc_common.bzl",
    "cc/common/cc_info.bzl",
    "cc/extensions.bzl",
    "cc/private/toolchain/cc_configure.bzl",
    "cc/private/toolchain/lib_cc_configure.bzl",
    "cc/private/toolchain/unix_cc_configure.bzl",
    "cc/private/toolchain/windows_cc_configure.bzl",
];
const RULES_CC_IMPORT_METADATA: [&str; 4] =
    ["cc", "cc/common", "cc/private", "cc/private/toolchain"];
struct Bodies {
    roots: [NormalizedAbsolutePath; 3],
    files: [NormalizedAbsolutePath; 3],
    additional: [Option<AdditionalModule>; 8],
    visible_lockfile_metadata: bool,
    repo_marker_metadata: bool,
    ignore_file_bytes: bool,
    rust_package_metadata: bool,
    nightly_package_metadata: bool,
    nightly_build_source: bool,
    nightly_channel_source: bool,
    probe_package_source: bool,
    rules_rust_defs_source: bool,
    rules_rust_defs_import_sources: bool,
    bazel_features_import_sources: bool,
    generated_features_file_effects: bool,
    bazel_skylib_import_sources: bool,
    rules_cc_import_sources: bool,
}
#[derive(Debug)]
struct Denied {
    family: &'static str,
    path: Option<NormalizedAbsolutePath>,
    operation: &'static str,
}
fn path_operation_code(operation: &PathObservationOperation) -> &'static str {
    match operation {
        PathObservationOperation::InputImage(_) => "input_image",
        PathObservationOperation::Lstat => "lstat",
        PathObservationOperation::ReadLink => "read_link",
        PathObservationOperation::FileBytes => "file_bytes",
        PathObservationOperation::FileDigest => "file_digest",
        PathObservationOperation::DirectoryEntries => "directory_entries",
        PathObservationOperation::WindowsLongPath => "windows_long_path",
        PathObservationOperation::WindowsOptionPathLongName => "windows_option_path_long_name",
    }
}
const MATERIALIZATION_FIELD_BYTES: usize = 1024;
const MATERIALIZATION_AGGREGATE_BYTES: usize = 4096;
const MATERIALIZATION_MARKER_BYTES: usize = 10240;
struct MaterializationDiagnostic<'a> {
    kind: &'static str,
    reason: &'static str,
    fields: [Option<&'a [u8]>; 7],
}
impl<'a> MaterializationDiagnostic<'a> {
    fn new(
        request: &'a slug_bzlmod_v2::RepositoryMaterializationRequest,
        reason: &'static str,
    ) -> Self {
        let (kind, logical_root) = match &request.kind {
            RepositoryMaterializationKind::Local { logical_root } => {
                ("local", Some(logical_root.as_path().as_os_str().as_bytes()))
            }
            RepositoryMaterializationKind::Immutable => ("immutable", None),
            RepositoryMaterializationKind::GeneratedFileEffects(_) => {
                ("generated_file_effects", None)
            }
        };
        let label = &request.repo_spec.rule_id.bzl_file;
        Self {
            kind,
            reason,
            fields: [
                Some(request.id.workspace.as_path().as_os_str().as_bytes()),
                Some(request.id.canonical_repo.as_str().as_bytes()),
                Some(label.package().repo().as_str().as_bytes()),
                Some(label.package().package().as_str().as_bytes()),
                Some(label.target().as_str().as_bytes()),
                Some(request.repo_spec.rule_id.rule_name.as_bytes()),
                logical_root,
            ],
        }
    }
    fn captured(&self) -> bool {
        self.fields
            .iter()
            .flatten()
            .try_fold(0usize, |sum, bytes| {
                (bytes.len() <= MATERIALIZATION_FIELD_BYTES).then_some(())?;
                sum.checked_add(bytes.len())
                    .filter(|sum| *sum <= MATERIALIZATION_AGGREGATE_BYTES)
            })
            .is_some()
    }
}
struct MaterializationWriter<'a, W> {
    writer: &'a mut W,
    bytes: usize,
    limit: usize,
}
impl<W: fmt::Write> fmt::Write for MaterializationWriter<'_, W> {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        self.bytes = self
            .bytes
            .checked_add(text.len())
            .filter(|n| *n < self.limit)
            .ok_or(fmt::Error)?;
        self.writer.write_str(text)
    }
}
impl fmt::Display for MaterializationDiagnostic<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        use fmt::Write;
        let captured = self.captured();
        let mut writer = MaterializationWriter {
            writer: formatter,
            bytes: 0,
            limit: MATERIALIZATION_MARKER_BYTES,
        };
        write!(
            writer,
            "WP982_MATERIALIZATION capture={} kind={} reason={}",
            if captured {
                "captured"
            } else {
                "bounds_refused"
            },
            self.kind,
            self.reason
        )?;
        for (name, bytes) in [
            "workspace",
            "canonical",
            "rule_repo",
            "rule_package",
            "rule_target",
            "rule_name",
            "logical_root",
        ]
        .into_iter()
        .zip(self.fields)
        {
            match bytes {
                Some(bytes) => write!(
                    writer,
                    " {name}_len={} {name}_hex={}",
                    bytes.len(),
                    RegistryHex(captured.then_some(bytes))
                )?,
                None => write!(writer, " {name}_len=none {name}_hex=none")?,
            }
        }
        Ok(())
    }
}
const GENERATED_PLAN_ROWS: usize = 8;
const GENERATED_PLAN_PATH_BYTES: usize = 512;
const GENERATED_PLAN_PATH_SUM: usize = 2048;
const GENERATED_PLAN_CONTENT_BYTES: usize = 1 << 20;
const GENERATED_PLAN_CONTENT_SUM: usize = 4 << 20;
const GENERATED_PLAN_MARKER_BYTES: usize = 16 << 10;
struct OptionalCount(Option<usize>);
impl fmt::Display for OptionalCount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Some(value) => write!(f, "{value}"),
            None => f.write_str("none"),
        }
    }
}
struct GeneratedPlanSummary {
    rows: usize,
    counts: Option<[usize; 3]>,
    paths: Option<usize>,
    content: Option<usize>,
    captured: bool,
}
struct GeneratedPlanDiagnostic<'a> {
    request: &'a slug_bzlmod_v2::RepositoryMaterializationRequest,
    plan: &'a slug_bzlmod_v2::GeneratedRepositoryFileEffectPlan,
}
impl GeneratedPlanDiagnostic<'_> {
    fn mapping(&self) -> Option<&[u8]> {
        self.request
            .repo_spec
            .rule_id
            .bzl_file
            .mapping_id()
            .map(|id| id.as_str().as_bytes())
    }
    fn summary(&self) -> GeneratedPlanSummary {
        use slug_bzlmod_v2::GeneratedRepositoryOrderedEffectRef as Effect;
        let mut result = GeneratedPlanSummary {
            rows: 0,
            counts: Some([0; 3]),
            paths: Some(0),
            content: Some(0),
            captured: true,
        };
        for effect in self.plan.ordered_effects().take(GENERATED_PLAN_ROWS + 1) {
            result.rows += 1;
            if result.rows > GENERATED_PLAN_ROWS {
                return GeneratedPlanSummary {
                    rows: result.rows,
                    counts: None,
                    paths: None,
                    content: None,
                    captured: false,
                };
            }
            let kind = match effect {
                Effect::File(file) => {
                    result.paths = result
                        .paths
                        .and_then(|sum| sum.checked_add(file.path().len()));
                    result.content = result
                        .content
                        .and_then(|sum| sum.checked_add(file.content().len()));
                    result.captured &= file.path().len() <= GENERATED_PLAN_PATH_BYTES
                        && file.content().len() <= GENERATED_PLAN_CONTENT_BYTES;
                    0
                }
                Effect::Archive(_) => 1,
                Effect::Symlink(_) => 2,
            };
            result.counts.as_mut().unwrap()[kind] += 1;
        }
        let owner = MaterializationDiagnostic::new(self.request, "generated_kind");
        let owner_sum = owner
            .fields
            .iter()
            .flatten()
            .try_fold(0usize, |sum, bytes| sum.checked_add(bytes.len()));
        let mapping = self.mapping();
        result.captured &= owner.captured()
            && mapping.is_none_or(|bytes| bytes.len() <= MATERIALIZATION_FIELD_BYTES)
            && owner_sum
                .and_then(|sum| sum.checked_add(mapping.map_or(0, <[u8]>::len)))
                .is_some_and(|sum| sum <= MATERIALIZATION_AGGREGATE_BYTES)
            && result
                .paths
                .is_some_and(|sum| sum <= GENERATED_PLAN_PATH_SUM)
            && result
                .content
                .is_some_and(|sum| sum <= GENERATED_PLAN_CONTENT_SUM);
        result
    }
}
impl fmt::Display for GeneratedPlanDiagnostic<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use fmt::Write;
        use slug_bzlmod_v2::GeneratedRepositoryOrderedEffectRef as Effect;
        let summary = self.summary();
        let mapping = self.mapping();
        let mut writer = MaterializationWriter {
            writer: f,
            bytes: 0,
            limit: GENERATED_PLAN_MARKER_BYTES,
        };
        write!(
            writer,
            "WP983_GENERATED_PLAN capture={} counts_complete={} ordered_rows={} file_count={} archive_count={} symlink_count={} path_bytes={} content_bytes={} mapping_len={} mapping_hex={} attribute_count={}",
            if summary.captured {
                "captured"
            } else {
                "bounds_refused"
            },
            summary.counts.is_some(),
            summary.rows,
            OptionalCount(summary.counts.map(|n| n[0])),
            OptionalCount(summary.counts.map(|n| n[1])),
            OptionalCount(summary.counts.map(|n| n[2])),
            OptionalCount(summary.paths),
            OptionalCount(summary.content),
            OptionalCount(mapping.map(<[u8]>::len)),
            RegistryHex(mapping.filter(|_| summary.captured)),
            self.request.repo_spec.attributes.len()
        )?;
        if summary.captured {
            for (ordinal, effect) in self.plan.ordered_effects().enumerate() {
                match effect {
                    Effect::File(file) => {
                        let mut digest = sha2::Sha256::new();
                        digest.update(file.content());
                        let digest = digest.finalize();
                        write!(
                            writer,
                            "\nWP983_GENERATED_EFFECT ordinal={ordinal} kind=file path_len={} path_hex={} content_len={} content_sha256={} executable={}",
                            file.path().len(),
                            RegistryHex(Some(file.path().as_bytes())),
                            file.content().len(),
                            RegistryHex(Some(digest.as_slice())),
                            file.executable()
                        )?;
                    }
                    Effect::Archive(_) | Effect::Symlink(_) => {
                        let kind = if matches!(effect, Effect::Archive(_)) {
                            "archive"
                        } else {
                            "symlink"
                        };
                        write!(
                            writer,
                            "\nWP983_GENERATED_EFFECT ordinal={ordinal} kind={kind} path_len=none path_hex=none content_len=none content_sha256=none executable=none"
                        )?;
                    }
                }
            }
        }
        Ok(())
    }
}
type GeneratedFileSignature = (&'static str, usize, bool, [u8; 32]);
const GENERATED_FEATURES_FILES: [GeneratedFileSignature; 2] = [
    (
        "BUILD.bazel",
        266,
        true,
        [
            0xab, 0x75, 0x99, 0x2e, 0xeb, 0xb5, 0xa7, 0x56, 0x5c, 0x45, 0x29, 0xc3, 0x50, 0x89,
            0x91, 0xf3, 0xc1, 0x2f, 0x20, 0x35, 0xe9, 0xa9, 0x35, 0xad, 0xca, 0x1f, 0x62, 0x1d,
            0x65, 0x1e, 0x3d, 0xcc,
        ],
    ),
    (
        "globals.bzl",
        1585,
        true,
        [
            0xe3, 0xd6, 0x78, 0x0b, 0xb1, 0xa8, 0x69, 0xff, 0x03, 0x2e, 0x2c, 0x2b, 0x88, 0x2b,
            0xa3, 0xea, 0xc6, 0x01, 0xb0, 0xd2, 0x12, 0x80, 0x50, 0x55, 0x59, 0xe7, 0x7a, 0xe2,
            0x77, 0x3e, 0x4a, 0x6f,
        ],
    ),
];
fn generated_features_plan_matches(
    plan: &slug_bzlmod_v2::GeneratedRepositoryFileEffectPlan,
    expected: &[GeneratedFileSignature; 2],
) -> bool {
    use slug_bzlmod_v2::GeneratedRepositoryOrderedEffectRef as Effect;
    let mut ordered = plan.ordered_effects();
    let (Some(Effect::File(first)), Some(Effect::File(second))) = (ordered.next(), ordered.next())
    else {
        return false;
    };
    if ordered.next().is_some() {
        return false;
    }
    let files = [first, second];
    // Check every shape/length before inspecting any content digest.
    if files
        .iter()
        .zip(expected)
        .any(|(file, &(path, bytes, executable, _))| {
            file.path() != path || file.content().len() != bytes || file.executable() != executable
        })
    {
        return false;
    }
    files.iter().zip(expected).all(|(file, expected)| {
        let mut digest = sha2::Sha256::new();
        digest.update(file.content());
        digest.finalize().as_slice() == expected.3
    })
}
impl Bodies {
    fn first_denied_materialization<'a>(
        &self,
        needs: &'a SourcePreparationNeeds,
    ) -> Option<(
        &'a slug_bzlmod_v2::RepositoryMaterializationRequest,
        &'static str,
    )> {
        self.first_denied_materialization_with_plan_matcher(needs, &|plan| {
            generated_features_plan_matches(plan, &GENERATED_FEATURES_FILES)
        })
    }
    fn generated_features_owner_is_admitted(
        &self,
        request: &slug_bzlmod_v2::RepositoryMaterializationRequest,
    ) -> bool {
        let label = &request.repo_spec.rule_id.bzl_file;
        self.generated_features_file_effects
            && self.bazel_features_import_sources
            && self.additional[0].is_some()
            && request.id.workspace == self.roots[0]
            && request.id.canonical_repo.as_str()
                == "bazel_features++version_extension+bazel_features_globals"
            && label.package().repo().as_str() == "bazel_features+"
            && label.package().package().as_str() == "private"
            && label.target().as_str() == "globals_repo.bzl"
            && label.mapping_id().is_none()
            && request.repo_spec.rule_id.rule_name == "globals_repo"
    }
    fn first_denied_materialization_with_plan_matcher<'a>(
        &self,
        needs: &'a SourcePreparationNeeds,
        plan_matches: &impl Fn(&slug_bzlmod_v2::GeneratedRepositoryFileEffectPlan) -> bool,
    ) -> Option<(
        &'a slug_bzlmod_v2::RepositoryMaterializationRequest,
        &'static str,
    )> {
        for (_, request) in needs.repository_materializations().iter() {
            let logical_root = match &request.kind {
                RepositoryMaterializationKind::Local { logical_root } => logical_root,
                RepositoryMaterializationKind::Immutable => {
                    return Some((request, "immutable_kind"));
                }
                RepositoryMaterializationKind::GeneratedFileEffects(plan) => {
                    if self.generated_features_owner_is_admitted(request) && plan_matches(plan) {
                        continue;
                    }
                    return Some((request, "generated_kind"));
                }
            };
            let label = &request.repo_spec.rule_id.bzl_file;
            let attributes = &request.repo_spec.attributes;
            let checks = [
                (request.id.workspace == self.roots[0], "workspace"),
                (
                    self.bindings().any(|(_, canonical, root)| {
                        request.id.canonical_repo.as_str() == canonical && logical_root == root
                    }),
                    "binding",
                ),
                (attributes.len() == 1, "attributes_count"),
                (
                    matches!(attributes.get("path"), Some(OverrideAttributeValue::String(path)) if Path::new(path.as_str()) == logical_root.as_path()),
                    "path_attribute",
                ),
                (
                    request.repo_spec.rule_id.rule_name == "local_repository",
                    "rule_name",
                ),
                (label.mapping_id().is_none(), "rule_mapping"),
                (
                    label.package().repo().as_str() == "bazel_tools",
                    "rule_repository",
                ),
                (
                    label.package().package().as_str() == "tools/build_defs/repo",
                    "rule_package",
                ),
                (label.target().as_str() == "local.bzl", "rule_target"),
            ];
            if let Some((_, reason)) = checks.into_iter().find(|(valid, _)| !valid) {
                return Some((request, reason));
            }
        }
        None
    }
    fn materialization_diagnostic<'a>(
        &self,
        needs: &'a SourcePreparationNeeds,
        denied: &Denied,
    ) -> Option<MaterializationDiagnostic<'a>> {
        if denied.family != "materialization" {
            return None;
        }
        let (request, reason) = self
            .first_denied_materialization(needs)
            .expect("materialization refusal preserves its original offender");
        Some(MaterializationDiagnostic::new(request, reason))
    }
    fn generated_plan_diagnostic<'a>(
        &self,
        needs: &'a SourcePreparationNeeds,
        denied: &Denied,
    ) -> Option<GeneratedPlanDiagnostic<'a>> {
        if denied.family != "materialization" {
            return None;
        }
        let (request, reason) = self.first_denied_materialization(needs)?;
        let label = &request.repo_spec.rule_id.bzl_file;
        if reason != "generated_kind"
            || request.id.workspace != self.roots[0]
            || request.id.canonical_repo.as_str()
                != "bazel_features++version_extension+bazel_features_globals"
            || label.package().repo().as_str() != "bazel_features+"
            || label.package().package().as_str() != "private"
            || label.target().as_str() != "globals_repo.bzl"
            || request.repo_spec.rule_id.rule_name != "globals_repo"
        {
            return None;
        }
        let RepositoryMaterializationKind::GeneratedFileEffects(plan) = &request.kind else {
            unreachable!()
        };
        Some(GeneratedPlanDiagnostic { request, plan })
    }
    fn new(roots: [NormalizedAbsolutePath; 3]) -> Self {
        let files = roots
            .each_ref()
            .map(|root| NormalizedAbsolutePath::new(root.as_path().join("MODULE.bazel")).unwrap());
        Self {
            roots,
            files,
            additional: [None, None, None, None, None, None, None, None],
            visible_lockfile_metadata: false,
            repo_marker_metadata: false,
            ignore_file_bytes: false,
            rust_package_metadata: false,
            nightly_package_metadata: false,
            nightly_build_source: false,
            nightly_channel_source: false,
            probe_package_source: false,
            rules_rust_defs_source: false,
            rules_rust_defs_import_sources: false,
            bazel_features_import_sources: false,
            generated_features_file_effects: false,
            bazel_skylib_import_sources: false,
            rules_cc_import_sources: false,
        }
    }
    fn with_visible_lockfile_metadata(mut self, choice: RegistryChoice) -> Self {
        self.visible_lockfile_metadata = matches!(choice, RegistryChoice::PreparedFile);
        self
    }
    fn with_repo_marker_metadata(
        mut self,
        registry: RegistryChoice,
        command: CommandChoice,
    ) -> Self {
        self.repo_marker_metadata = matches!(registry, RegistryChoice::PreparedFile)
            && matches!(command, CommandChoice::PublicProbe);
        self
    }
    fn with_ignore_file_bytes(mut self, registry: RegistryChoice, command: CommandChoice) -> Self {
        self.ignore_file_bytes = matches!(registry, RegistryChoice::PreparedFile)
            && matches!(command, CommandChoice::PublicProbe);
        self
    }
    fn with_rust_package_metadata(
        mut self,
        registry: RegistryChoice,
        command: CommandChoice,
    ) -> Self {
        self.rust_package_metadata = matches!(registry, RegistryChoice::PreparedFile)
            && matches!(command, CommandChoice::PublicProbe);
        self
    }
    fn with_nightly_package_metadata(
        mut self,
        registry: RegistryChoice,
        command: CommandChoice,
    ) -> Self {
        self.nightly_package_metadata = matches!(registry, RegistryChoice::PreparedFile)
            && matches!(command, CommandChoice::PublicProbe);
        self
    }
    fn with_nightly_build_source(
        mut self,
        registry: RegistryChoice,
        command: CommandChoice,
    ) -> Self {
        self.nightly_build_source = matches!(registry, RegistryChoice::PreparedFile)
            && matches!(command, CommandChoice::PublicProbe);
        self
    }
    fn admits_nightly_build_source(&self, path: &Path) -> bool {
        self.nightly_build_source
            && path
                .strip_prefix(self.roots[1].as_path())
                .is_ok_and(|relative| {
                    relative == Path::new("rust/toolchain/channel/BUILD.bazel")
                        || relative == Path::new("rust/toolchain/channel/BUILD")
                })
    }
    fn with_nightly_channel_source(
        mut self,
        registry: RegistryChoice,
        command: CommandChoice,
    ) -> Self {
        self.nightly_channel_source = matches!(registry, RegistryChoice::PreparedFile)
            && matches!(command, CommandChoice::PublicProbe);
        self
    }
    fn admits_nightly_channel_source(&self, path: &Path) -> bool {
        self.nightly_channel_source
            && path
                .strip_prefix(self.roots[1].as_path())
                .is_ok_and(|relative| relative == Path::new("rust/toolchain/channel/channel.bzl"))
    }
    fn with_probe_package_source(
        mut self,
        registry: RegistryChoice,
        command: CommandChoice,
    ) -> Self {
        self.probe_package_source = matches!(registry, RegistryChoice::PreparedFile)
            && matches!(command, CommandChoice::PublicProbe);
        self
    }
    fn admits_probe_package_source(&self, path: &Path, metadata: bool) -> bool {
        self.probe_package_source
            && path
                .strip_prefix(self.roots[0].as_path())
                .is_ok_and(|relative| {
                    (metadata && relative == Path::new("pkg"))
                        || relative == Path::new("pkg/BUILD.bazel")
                        || relative == Path::new("pkg/BUILD")
                })
    }
    fn with_rules_rust_defs_source(
        mut self,
        registry: RegistryChoice,
        command: CommandChoice,
    ) -> Self {
        self.rules_rust_defs_source = matches!(registry, RegistryChoice::PreparedFile)
            && matches!(command, CommandChoice::PublicProbe);
        self
    }
    fn admits_rules_rust_defs_source(&self, path: &Path) -> bool {
        self.rules_rust_defs_source
            && path
                .strip_prefix(self.roots[1].as_path())
                .is_ok_and(|relative| relative == Path::new("rust/defs.bzl"))
    }
    fn with_rules_rust_defs_import_sources(
        mut self,
        registry: RegistryChoice,
        command: CommandChoice,
    ) -> Self {
        self.rules_rust_defs_import_sources = matches!(registry, RegistryChoice::PreparedFile)
            && matches!(command, CommandChoice::PublicProbe);
        self
    }
    fn admits_rules_rust_defs_import_source(&self, path: &Path, metadata: bool) -> bool {
        self.rules_rust_defs_import_sources
            && path
                .strip_prefix(self.roots[1].as_path())
                .is_ok_and(|relative| {
                    (metadata
                        && RULES_RUST_IMPORT_METADATA
                            .iter()
                            .any(|directory| relative == Path::new(directory)))
                        || RULES_RUST_DEFS_IMPORTS
                            .iter()
                            .any(|file| relative == Path::new(file))
                })
    }
    fn with_generated_features_file_effects(
        mut self,
        registry: RegistryChoice,
        command: CommandChoice,
    ) -> Self {
        self.generated_features_file_effects = matches!(registry, RegistryChoice::PreparedFile)
            && matches!(command, CommandChoice::PublicProbe);
        self
    }
    fn with_bazel_features_import_sources(
        mut self,
        registry: RegistryChoice,
        command: CommandChoice,
    ) -> Self {
        self.bazel_features_import_sources = matches!(registry, RegistryChoice::PreparedFile)
            && matches!(command, CommandChoice::PublicProbe);
        self
    }
    fn admits_bazel_features_import_source(&self, path: &Path, metadata: bool) -> bool {
        self.bazel_features_import_sources
            && self.additional[0].as_ref().is_some_and(|module| {
                path.strip_prefix(module.root.as_path())
                    .is_ok_and(|relative| {
                        (metadata
                            && BAZEL_FEATURES_IMPORT_METADATA
                                .iter()
                                .any(|directory| relative == Path::new(directory)))
                            || BAZEL_FEATURES_IMPORTS
                                .iter()
                                .any(|file| relative == Path::new(file))
                    })
            })
    }
    fn with_bazel_skylib_import_sources(
        mut self,
        registry: RegistryChoice,
        command: CommandChoice,
    ) -> Self {
        self.bazel_skylib_import_sources = matches!(registry, RegistryChoice::PreparedFile)
            && matches!(command, CommandChoice::PublicProbe);
        self
    }
    fn admits_bazel_skylib_import_source(&self, path: &Path, metadata: bool) -> bool {
        self.bazel_skylib_import_sources
            && self.additional[1].as_ref().is_some_and(|module| {
                path.strip_prefix(module.root.as_path())
                    .is_ok_and(|relative| {
                        (metadata
                            && BAZEL_SKYLIB_IMPORT_METADATA
                                .iter()
                                .any(|directory| relative == Path::new(directory)))
                            || BAZEL_SKYLIB_IMPORTS
                                .iter()
                                .any(|file| relative == Path::new(file))
                    })
            })
    }
    fn with_rules_cc_import_sources(
        mut self,
        registry: RegistryChoice,
        command: CommandChoice,
    ) -> Self {
        self.rules_cc_import_sources = matches!(registry, RegistryChoice::PreparedFile)
            && matches!(command, CommandChoice::PublicProbe);
        self
    }
    fn admits_rules_cc_import_source(&self, path: &Path, metadata: bool) -> bool {
        self.rules_cc_import_sources
            && self.additional[2].as_ref().is_some_and(|module| {
                path.strip_prefix(module.root.as_path())
                    .is_ok_and(|relative| {
                        (metadata
                            && RULES_CC_IMPORT_METADATA
                                .iter()
                                .any(|directory| relative == Path::new(directory)))
                            || RULES_CC_IMPORTS
                                .iter()
                                .any(|file| relative == Path::new(file))
                    })
            })
    }
    fn source_limits(&self, registry: RegistryChoice) -> SourceServiceLimits {
        SourceServiceLimits {
            allow_network: false,
            registry_object_bytes: 1 << 20,
            registry_attempts: registry.registry_attempts(),
            registry_admitted_bytes: 16 << 20,
            source_attempts: if self.rules_rust_defs_import_sources {
                64
            } else {
                16
            },
            transferred_bytes: 16 << 20,
            decoded_bytes: 16 << 20,
            written_bytes: 16 << 20,
            live_buffer_bytes: 16 << 20,
            blocking_threads: Some(2),
        }
    }
    fn with_fourth(mut self, root: NormalizedAbsolutePath) -> Self {
        let file = NormalizedAbsolutePath::new(root.as_path().join("MODULE.bazel")).unwrap();
        self.additional[0] = Some(AdditionalModule { root, file });
        self
    }
    fn with_fifth(mut self, root: NormalizedAbsolutePath) -> Self {
        assert!(
            self.additional[0].is_some(),
            "fifth binding requires fourth"
        );
        let file = NormalizedAbsolutePath::new(root.as_path().join("MODULE.bazel")).unwrap();
        self.additional[1] = Some(AdditionalModule { root, file });
        self
    }
    fn with_sixth(mut self, root: NormalizedAbsolutePath) -> Self {
        assert!(
            self.additional[..2].iter().all(Option::is_some),
            "sixth binding requires fourth and fifth"
        );
        let file = NormalizedAbsolutePath::new(root.as_path().join("MODULE.bazel")).unwrap();
        self.additional[2] = Some(AdditionalModule { root, file });
        self
    }
    fn with_seventh(mut self, root: NormalizedAbsolutePath) -> Self {
        assert!(
            self.additional[..3].iter().all(Option::is_some),
            "seventh binding requires fourth, fifth and sixth"
        );
        let file = NormalizedAbsolutePath::new(root.as_path().join("MODULE.bazel")).unwrap();
        self.additional[3] = Some(AdditionalModule { root, file });
        self
    }
    fn with_eighth(mut self, root: NormalizedAbsolutePath) -> Self {
        assert!(
            self.additional[..4].iter().all(Option::is_some),
            "eighth binding requires fourth through seventh"
        );
        let file = NormalizedAbsolutePath::new(root.as_path().join("MODULE.bazel")).unwrap();
        self.additional[4] = Some(AdditionalModule { root, file });
        self
    }
    fn with_ninth(mut self, root: NormalizedAbsolutePath) -> Self {
        assert!(
            self.additional[..5].iter().all(Option::is_some),
            "ninth binding requires fourth through eighth"
        );
        let file = NormalizedAbsolutePath::new(root.as_path().join("MODULE.bazel")).unwrap();
        self.additional[5] = Some(AdditionalModule { root, file });
        self
    }
    fn with_tenth(mut self, root: NormalizedAbsolutePath) -> Self {
        assert!(
            self.additional[..6].iter().all(Option::is_some),
            "tenth binding requires fourth through ninth"
        );
        let file = NormalizedAbsolutePath::new(root.as_path().join("MODULE.bazel")).unwrap();
        self.additional[6] = Some(AdditionalModule { root, file });
        self
    }
    fn with_eleventh(mut self, root: NormalizedAbsolutePath) -> Self {
        assert!(
            self.additional[..7].iter().all(Option::is_some),
            "eleventh binding requires fourth through tenth"
        );
        let file = NormalizedAbsolutePath::new(root.as_path().join("MODULE.bazel")).unwrap();
        self.additional[7] = Some(AdditionalModule { root, file });
        self
    }
    fn roots(&self) -> impl Iterator<Item = &NormalizedAbsolutePath> {
        self.roots.iter().chain(
            self.additional
                .iter()
                .flatten()
                .map(|binding| &binding.root),
        )
    }
    fn files(&self) -> impl Iterator<Item = &NormalizedAbsolutePath> {
        self.files.iter().chain(
            self.additional
                .iter()
                .flatten()
                .map(|binding| &binding.file),
        )
    }
    fn bindings(
        &self,
    ) -> impl Iterator<Item = (&'static str, &'static str, &NormalizedAbsolutePath)> {
        [
            ("rules_rust", "rules_rust+", &self.roots[1]),
            ("platforms", "platforms+", &self.roots[2]),
        ]
        .into_iter()
        .chain(
            self.additional
                .iter()
                .enumerate()
                .filter_map(|(slot, binding)| {
                    binding.as_ref().map(|binding| {
                        (
                            ADDITIONAL_IDENTITIES[slot].0,
                            ADDITIONAL_IDENTITIES[slot].1,
                            &binding.root,
                        )
                    })
                }),
        )
    }
    fn admit(&self, needs: &SourcePreparationNeeds) -> Result<(), Denied> {
        self.admit_with_plan_matcher(needs, &|plan| {
            generated_features_plan_matches(plan, &GENERATED_FEATURES_FILES)
        })
    }
    fn admit_with_plan_matcher(
        &self,
        needs: &SourcePreparationNeeds,
        plan_matches: &impl Fn(&slug_bzlmod_v2::GeneratedRepositoryFileEffectPlan) -> bool,
    ) -> Result<(), Denied> {
        let refusal = |family| Denied {
            family,
            path: None,
            operation: "none",
        };
        if needs.scratch_elf_metadata_requests().is_some() {
            return Err(refusal("scratch"));
        }
        if needs
            .root_module_bootstrap_request()
            .is_some_and(|r| r.workspace != self.roots[0])
        {
            return Err(refusal("bootstrap"));
        }
        if needs.repository_environment().is_some() {
            return Err(refusal("environment"));
        }
        if !needs.selected_extension_downloads().is_empty() {
            return Err(refusal("download"));
        }
        if !needs.repository_executes().is_empty() {
            return Err(refusal("execute"));
        }
        if self
            .first_denied_materialization_with_plan_matcher(needs, plan_matches)
            .is_some()
        {
            return Err(refusal("materialization"));
        }
        if let Some(paths) = needs.path_observations() {
            for demand in paths.demands() {
                let operation = demand.operation();
                let deny = |family| Denied {
                    family,
                    path: Some(demand.path().dupe()),
                    operation: path_operation_code(&operation),
                };
                if demand.namespace() != PathObservationNamespace::Host {
                    return Err(deny("namespace"));
                }
                let path = demand.path().as_path();
                let allowed = match operation {
                    PathObservationOperation::FileBytes | PathObservationOperation::FileDigest => {
                        self.files().any(|file| file.as_path() == path)
                            || (operation == PathObservationOperation::FileBytes
                                && (self.admits_nightly_build_source(path)
                                    || self.admits_nightly_channel_source(path)
                                    || self.admits_probe_package_source(path, false)
                                    || self.admits_rules_rust_defs_source(path)
                                    || self.admits_rules_rust_defs_import_source(path, false)
                                    || self.admits_bazel_features_import_source(path, false)
                                    || self.admits_bazel_skylib_import_source(path, false)
                                    || self.admits_rules_cc_import_source(path, false)))
                            || (self.ignore_file_bytes
                                && operation == PathObservationOperation::FileBytes
                                && path.file_name().is_some_and(|name| name == ".bazelignore")
                                && self
                                    .roots()
                                    .any(|root| path.parent() == Some(root.as_path())))
                    }
                    PathObservationOperation::Lstat => {
                        self.files().any(|file| file.as_path().starts_with(path))
                            || self.admits_nightly_build_source(path)
                            || self.admits_nightly_channel_source(path)
                            || self.admits_probe_package_source(path, true)
                            || self.admits_rules_rust_defs_source(path)
                            || self.admits_rules_rust_defs_import_source(path, true)
                            || self.admits_bazel_features_import_source(path, true)
                            || self.admits_bazel_skylib_import_source(path, true)
                            || self.admits_rules_cc_import_source(path, true)
                            || (self.visible_lockfile_metadata
                                && path.parent() == Some(self.roots[0].as_path())
                                && path
                                    .file_name()
                                    .is_some_and(|name| name == "MODULE.bazel.lock"))
                            || (self.repo_marker_metadata
                                && path.file_name().is_some_and(|name| name == "REPO.bazel")
                                && self
                                    .roots()
                                    .any(|root| path.parent() == Some(root.as_path())))
                            || (self.rust_package_metadata
                                && path.parent() == Some(self.roots[1].as_path())
                                && path.file_name().is_some_and(|name| name == "rust"))
                            || (self.nightly_package_metadata
                                && path.strip_prefix(self.roots[1].as_path()).is_ok_and(
                                    |relative| {
                                        relative == Path::new("rust/toolchain")
                                            || relative == Path::new("rust/toolchain/channel")
                                    },
                                ))
                            || self.roots().any(|root| {
                                path.parent() == Some(root.as_path())
                                    && matches!(
                                        path.file_name().and_then(|n| n.to_str()),
                                        Some(".bazelignore" | "BUILD" | "BUILD.bazel")
                                    )
                            })
                    }
                    _ => return Err(deny("operation")),
                };
                if !allowed {
                    return Err(deny("path"));
                }
            }
        }
        Ok(())
    }
}

#[derive(Default)]
struct Milestones {
    calls: u64,
    preparations: u64,
    effective_empty: bool,
    source: Option<Weak<[u8]>>,
    additional_source: [Option<Weak<[u8]>>; 8],
    additional_bytes: [Option<(usize, bool)>; 8],
    selected_reported: bool,
    registry_policy_checked: bool,
    bytes: usize,
    hash_match: bool,
    anchor: Option<&'static str>,
    anchor_outer: Option<Discriminant<ObservedPathFrontierError>>,
    mapping: Option<&'static str>,
    refusal: Option<SelectedSourceRefusal>,
    denied: Option<Denied>,
}
#[derive(Clone)]
struct ModuleRoot {
    bodies: Arc<Bodies>,
    expected_registry: Option<RegistryUrls>,
    milestones: Arc<Mutex<Milestones>>,
}
#[derive(Clone)]
struct ModuleTerminal {
    mapping: ObservedHostRootRepositoryMapping,
    source: Arc<[u8]>,
    certificate: SourceCertificate,
}
fn refusal() -> NativeDemandSessionError {
    NativeDemandSessionError::Computation(anyhow::anyhow!("bounded module prerequisite refusal"))
}
impl ModuleRoot {
    fn need(
        &self,
        needs: SourcePreparationNeeds,
    ) -> Result<SourcePreparationOutcome<ModuleTerminal>, NativeDemandSessionError> {
        if let Err(denied) = self.bodies.admit(&needs) {
            self.milestones.lock().unwrap().denied = Some(denied);
            return Err(refusal());
        }
        Ok(SourcePreparationOutcome::Need(needs))
    }
}
#[async_trait]
impl NativeCommandRoot for ModuleRoot {
    type Terminal = ModuleTerminal;
    fn initializes_request_revision(&self) -> bool {
        true
    }
    fn allows_empty_terminal(&self) -> bool {
        true
    }
    fn observed_selection_association(&self) -> ObservedSelectionAssociation {
        ObservedSelectionAssociation::SelectedDependencySuperset
    }
    fn event_reconciliation_policy(&self, _: &Self::Terminal) -> EventReconciliationPolicy {
        EventReconciliationPolicy::SourceCertifiedCurrentClosure
    }
    fn observations<'a>(&self, terminal: &'a Self::Terminal) -> Option<&'a PathObservationEpoch> {
        Some(terminal.mapping.observations())
    }
    fn source_certificate<'a>(
        &self,
        terminal: &'a Self::Terminal,
    ) -> Option<&'a SourceCertificate> {
        Some(&terminal.certificate)
    }
    async fn compute(
        &self,
        tx: &mut dice::DiceTransaction,
    ) -> Result<SourcePreparationOutcome<ModuleTerminal>, NativeDemandSessionError> {
        self.milestones.lock().unwrap().calls += 1;
        let policy = tx
            .compute(&RootModuleCommandPolicyKey {
                workspace: self.bodies.roots[0].as_path().to_owned(),
            })
            .await
            .map_err(|_| refusal())?;
        let exact = self.bodies.bindings().all(|(name, _, root)| {
            policy
                .command_module_overrides()
                .any(|(actual, path)| actual == name && path == root.as_path())
        });
        let mode = tx
            .compute(&slug_bzlmod_v2::RootModuleLockfileModeKey {
                workspace: self.bodies.roots[0].as_path().to_owned(),
            })
            .await
            .map_err(|_| refusal())?;
        if !exact || mode != slug_bzlmod_v2::RootModuleLockfileMode::from(LockfileMode::Off) {
            return Err(refusal());
        }
        if let Some(expected) = &self.expected_registry {
            let actual = tx
                .compute(&RootModuleRegistryUrlsKey {
                    workspace: self.bodies.roots[0].as_path().to_owned(),
                })
                .await
                .map_err(|_| refusal())?;
            if !registry_policy_matches(&actual, expected) {
                return Err(refusal());
            }
            self.milestones.lock().unwrap().registry_policy_checked = true;
        }
        self.milestones.lock().unwrap().effective_empty = true;
        self.milestones.lock().unwrap().anchor = Some("need");
        let anchor = tx
            .compute(&RootModuleLoadingAnchorObservationKey::new(
                self.bodies.roots[0].dupe(),
            ))
            .await
            .map_err(|_| {
                self.milestones.lock().unwrap().anchor = Some("compute_refusal");
                refusal()
            })?;
        let anchor = match anchor {
            SourcePreparationOutcome::Need(needs) => return self.need(needs),
            SourcePreparationOutcome::Complete(Err(error)) => {
                let mut state = self.milestones.lock().unwrap();
                state.anchor = Some("outer_refusal");
                state.anchor_outer = Some(std::mem::discriminant(&error));
                return Err(refusal());
            }
            SourcePreparationOutcome::Complete(Ok(anchor)) => anchor,
        };
        if anchor.result().is_err() {
            self.milestones.lock().unwrap().anchor = Some("semantic_refusal");
            return Err(refusal());
        }
        let source = anchor
            .observations()
            .observations()
            .iter()
            .find_map(|(demand, value)| {
                if demand.namespace() == PathObservationNamespace::Host
                    && demand.path() == &self.bodies.files[0]
                    && demand.operation() == PathObservationOperation::FileBytes
                {
                    if let PathObservationResult::FileBytes(PathOperationResult::Present(bytes)) =
                        value.as_ref()
                    {
                        if root_bytes(bytes) {
                            return Some(bytes.clone());
                        }
                    }
                }
                None
            });
        let Some(source) = source else {
            self.milestones.lock().unwrap().anchor = Some("coverage_refusal");
            return Err(refusal());
        };
        {
            let mut state = self.milestones.lock().unwrap();
            state.preparations += 1;
            state.source.get_or_insert_with(|| Arc::downgrade(&source));
            state.bytes = source.len();
            state.hash_match = root_bytes(&source);
            state.anchor = Some("complete");
            if !state.hash_match {
                return Err(refusal());
            }
            state.mapping = Some("need");
        }
        let observed = tx
            .compute(&HostRootRepositoryMappingObservationKey::new(
                self.bodies.roots[0].dupe(),
            ))
            .await
            .map_err(|_| refusal())?;
        let mapping = match observed {
            SourcePreparationOutcome::Need(needs) => return self.need(needs),
            SourcePreparationOutcome::Complete(Err(error)) => {
                let mut state = self.milestones.lock().unwrap();
                state.mapping = Some("outer_refusal");
                state.refusal = Some(error.selected_source_refusal());
                return Err(refusal());
            }
            SourcePreparationOutcome::Complete(Ok(mapping)) => mapping,
        };
        for (slot, binding) in self.bodies.additional.iter().enumerate() {
            if let Some(bytes) = binding
                .as_ref()
                .and_then(|binding| observed_bytes(mapping.observations(), &binding.file))
            {
                let mut state = self.milestones.lock().unwrap();
                state.additional_source[slot].get_or_insert_with(|| Arc::downgrade(bytes));
                state.additional_bytes[slot] = Some((bytes.len(), additional_bytes(slot, bytes)));
            }
        }
        if let Err(error) = mapping.result().as_ref() {
            let mut state = self.milestones.lock().unwrap();
            state.mapping = Some("semantic_refusal");
            state.refusal = Some(error.selected_source_refusal());
            if self.bodies.additional[0].is_some() {
                state.selected_reported = true;
                drop(state);
                report_selected(error.selected_module_registry_file());
            }
            return Err(refusal());
        }
        let covered = mapping.observations().observations().iter().any(|(demand, value)| {
            demand.namespace() == PathObservationNamespace::Host && demand.path() == &self.bodies.files[2]
                && demand.operation() == PathObservationOperation::FileBytes
                && matches!(value.as_ref(), PathObservationResult::FileBytes(PathOperationResult::Present(bytes)) if platforms_bytes(bytes))
        });
        let root_covered = mapping.observations().observations().iter().any(|(demand, value)| {
            demand.namespace() == PathObservationNamespace::Host
                && demand.path() == &self.bodies.files[0]
                && demand.operation() == PathObservationOperation::FileBytes
                && matches!(value.as_ref(), PathObservationResult::FileBytes(PathOperationResult::Present(bytes)) if root_bytes(bytes) && Arc::ptr_eq(bytes, &source))
        });
        let additional_covered =
            self.bodies
                .additional
                .iter()
                .enumerate()
                .all(|(slot, binding)| {
                    binding.as_ref().is_none_or(|binding| {
                        observed_bytes(mapping.observations(), &binding.file)
                            .is_some_and(|bytes| additional_bytes(slot, bytes))
                    })
                })
                && (self.bodies.additional[0].is_none()
                    || observed_bytes(mapping.observations(), &self.bodies.files[1]).is_some_and(
                        |bytes| {
                            bytes.len() == 4481
                                && sha2::Sha256::digest(bytes).as_slice() == RULES_SHA
                        },
                    ));
        if !covered || !root_covered || !additional_covered {
            self.milestones.lock().unwrap().mapping = Some("coverage_refusal");
            return Err(refusal());
        }
        let certificate =
            SourceCertificate::from_epoch(mapping.observations().dupe()).map_err(|_| {
                self.milestones.lock().unwrap().mapping = Some("certificate_refusal");
                refusal()
            })?;
        self.milestones.lock().unwrap().mapping = Some("complete");
        Ok(SourcePreparationOutcome::Complete(ModuleTerminal {
            mapping,
            source,
            certificate,
        }))
    }
}

fn observed_bytes<'a>(
    epoch: &'a PathObservationEpoch,
    path: &NormalizedAbsolutePath,
) -> Option<&'a Arc<[u8]>> {
    epoch.observations().iter().find_map(|(demand, value)| {
        if demand.namespace() == PathObservationNamespace::Host
            && demand.path() == path
            && demand.operation() == PathObservationOperation::FileBytes
        {
            if let PathObservationResult::FileBytes(PathOperationResult::Present(bytes)) =
                value.as_ref()
            {
                return Some(bytes);
            }
        }
        None
    })
}
fn report_selected(selected: Option<(&str, &str, &slug_bzlmod_v2::RegistryFileUrl)>) {
    let Some((name, version, url)) = selected else {
        println!(
            "WP945_SELECTED capture=missing name_len=none name_hex=none version_len=none version_hex=none url_len=none url_hex=none"
        );
        return;
    };
    let values = [name.as_bytes(), version.as_bytes(), url.as_str().as_bytes()];
    let captured = values
        .iter()
        .all(|value| value.len() <= REGISTRY_IDENTITY_BYTES);
    println!(
        "WP945_SELECTED capture={} name_len={} name_hex={} version_len={} version_hex={} url_len={} url_hex={}",
        if captured {
            "captured"
        } else {
            "bounds_refused"
        },
        values[0].len(),
        RegistryHex(captured.then_some(values[0])),
        values[1].len(),
        RegistryHex(captured.then_some(values[1])),
        values[2].len(),
        RegistryHex(captured.then_some(values[2]))
    );
}

#[test]
fn visible_lockfile_metadata_need_is_opt_in_and_root_lstat_only() {
    let normalized = |path: &str| NormalizedAbsolutePath::new(Path::new(path)).unwrap();
    let make = || {
        Bodies::new([
            normalized("/fixture"),
            normalized("/rules"),
            normalized("/platforms"),
        ])
    };
    let need = |namespace, path: &str, operation| {
        let path = normalized(path);
        let demand = match operation {
            PathObservationOperation::WindowsLongPath => {
                assert_eq!(namespace, PathObservationNamespace::Host);
                PathObservationDemand::windows_long_path(path, Arc::from([b'x' as u16]))
            }
            PathObservationOperation::WindowsOptionPathLongName => {
                assert_eq!(namespace, PathObservationNamespace::Host);
                PathObservationDemand::windows_option_path_long_name(path, Arc::from([b'x' as u16]))
            }
            _ => PathObservationDemand::new(namespace, path, operation),
        };
        SourcePreparationNeeds::path(NeedPathObservations::singleton(demand))
    };
    let lock = need(
        PathObservationNamespace::Host,
        "/fixture/MODULE.bazel.lock",
        PathObservationOperation::Lstat,
    );
    assert!(make().admit(&lock).is_err());
    let closed = make().with_visible_lockfile_metadata(RegistryChoice::DefaultBcr);
    let denied = closed.admit(&lock).unwrap_err();
    assert_eq!((denied.family, denied.operation), ("path", "lstat"));
    let admitted = make().with_visible_lockfile_metadata(RegistryChoice::PreparedFile);
    assert!(admitted.admit(&lock).is_ok());
    let mut progress_calls = 0;
    for path in [
        "/other/MODULE.bazel.lock",
        "/rules/MODULE.bazel.lock",
        "/platforms/MODULE.bazel.lock",
        "/fixture/MODULE.bazel.lock.other",
        "/fixture/MODULE.bazel.lock/child",
    ] {
        let foreign = need(
            PathObservationNamespace::Host,
            path,
            PathObservationOperation::Lstat,
        );
        let mixed = lock.try_union(&foreign).unwrap();
        if admitted.admit(&mixed).is_ok() {
            progress_calls += 1;
        }
        let denied = admitted.admit(&mixed).unwrap_err();
        assert_eq!((denied.family, denied.operation), ("path", "lstat"));
        assert_eq!(denied.path.unwrap().as_path(), Path::new(path));
    }
    for operation in [
        PathObservationOperation::FileBytes,
        PathObservationOperation::FileDigest,
        PathObservationOperation::ReadLink,
        PathObservationOperation::DirectoryEntries,
        PathObservationOperation::WindowsLongPath,
        PathObservationOperation::WindowsOptionPathLongName,
    ] {
        let code = path_operation_code(&operation);
        let denied = admitted
            .admit(&need(
                PathObservationNamespace::Host,
                "/fixture/MODULE.bazel.lock",
                operation,
            ))
            .unwrap_err();
        assert_eq!(denied.operation, code);
        assert_eq!(
            denied.path.unwrap().as_path(),
            Path::new("/fixture/MODULE.bazel.lock")
        );
    }
    let denied = admitted
        .admit(&need(
            PathObservationNamespace::Materialization(
                slug_workspace_v2::PathObservationInstanceId::new(1),
            ),
            "/fixture/MODULE.bazel.lock",
            PathObservationOperation::Lstat,
        ))
        .unwrap_err();
    assert_eq!((denied.family, denied.operation), ("namespace", "lstat"));
    let mixed = lock
        .try_union(&SourcePreparationNeeds::root_module_bootstrap(
            slug_bzlmod_v2::RootModuleBootstrapRequest {
                workspace: normalized("/other"),
            },
        ))
        .unwrap();
    if admitted.admit(&mixed).is_ok() {
        progress_calls += 1;
    }
    let denied = admitted.admit(&mixed).unwrap_err();
    assert_eq!((denied.family, denied.operation), ("bootstrap", "none"));
    assert!(denied.path.is_none());
    assert_eq!(progress_calls, 0);
}

#[test]
fn complete_need_admission_refuses_mixed_and_wrong_operation_before_progress() {
    let normalized = |path: &str| NormalizedAbsolutePath::new(Path::new(path)).unwrap();
    let bodies = Bodies::new([
        normalized("/fixture"),
        normalized("/rules"),
        normalized("/platforms"),
    ]);
    let paths = |path: &str, operation| {
        SourcePreparationNeeds::path(NeedPathObservations::singleton(PathObservationDemand::new(
            PathObservationNamespace::Host,
            normalized(path),
            operation,
        )))
    };
    let body = paths(
        "/platforms/MODULE.bazel",
        PathObservationOperation::FileBytes,
    );
    assert!(bodies.admit(&body).is_ok());
    for path in [
        "/",
        "/platforms",
        "/platforms/MODULE.bazel",
        "/rules/BUILD.bazel",
        "/fixture/.bazelignore",
    ] {
        assert!(
            bodies
                .admit(&paths(path, PathObservationOperation::Lstat))
                .is_ok()
        );
    }
    assert!(
        bodies
            .admit(&paths(
                "/rules/MODULE.bazel",
                PathObservationOperation::FileDigest
            ))
            .is_ok()
    );
    let mut progress_calls = 0;
    for excluded in [
        paths("/platforms/REPO.bazel", PathObservationOperation::Lstat),
        paths(
            "/platforms/BUILD.bazel",
            PathObservationOperation::FileBytes,
        ),
        paths(
            "/platforms/MODULE.bazel",
            PathObservationOperation::ReadLink,
        ),
        paths("/platforms", PathObservationOperation::DirectoryEntries),
        paths(
            "/platforms/MODULE.bazel",
            PathObservationOperation::FileBytes,
        )
        .try_union(&SourcePreparationNeeds::root_module_bootstrap(
            slug_bzlmod_v2::RootModuleBootstrapRequest {
                workspace: normalized("/other"),
            },
        ))
        .unwrap(),
        SourcePreparationNeeds::path(NeedPathObservations::singleton(PathObservationDemand::new(
            PathObservationNamespace::Materialization(
                slug_workspace_v2::PathObservationInstanceId::new(1),
            ),
            normalized("/platforms/MODULE.bazel"),
            PathObservationOperation::FileBytes,
        ))),
    ] {
        let mixed = body.try_union(&excluded).unwrap();
        if bodies.admit(&mixed).is_ok() {
            progress_calls += 1;
        }
        assert!(bodies.admit(&mixed).is_err());
    }
    let mut local = (*local_native_request(&bodies.roots[0], "platforms+", "/platforms")).clone();
    assert!(
        bodies
            .admit(&SourcePreparationNeeds::repository(local.clone()))
            .is_ok()
    );
    local.id.canonical_repo = slug_identity_v2::CanonicalRepoName::new("platforms").unwrap();
    assert!(
        bodies
            .admit(
                &body
                    .try_union(&SourcePreparationNeeds::repository(local.clone()))
                    .unwrap()
            )
            .is_err()
    );
    local.id.canonical_repo = slug_identity_v2::CanonicalRepoName::new("platforms+").unwrap();
    local.kind = RepositoryMaterializationKind::Immutable;
    assert!(
        bodies
            .admit(&SourcePreparationNeeds::repository(local.clone()))
            .is_err()
    );
    local.kind = RepositoryMaterializationKind::Local {
        logical_root: normalized("/other"),
    };
    assert!(
        bodies
            .admit(&SourcePreparationNeeds::repository(local.clone()))
            .is_err()
    );
    local.kind = RepositoryMaterializationKind::Local {
        logical_root: normalized("/platforms"),
    };
    local.id.workspace = normalized("/other");
    assert!(
        bodies
            .admit(&SourcePreparationNeeds::repository(local.clone()))
            .is_err()
    );
    local.id.workspace = normalized("/fixture");
    local.repo_spec.rule_id.rule_name = "http_archive".into();
    assert!(
        bodies
            .admit(&SourcePreparationNeeds::repository(local))
            .is_err()
    );
    let budget = slug_workspace_v2::ImageDescriptorBudget::new(4096);
    let anchor =
        slug_workspace_v2::ImageChargedPath::pin_existing(normalized("/platforms"), &budget)
            .unwrap();
    let leaf = slug_workspace_v2::ImageChargedPath::pin_existing(
        normalized("/platforms/MODULE.bazel"),
        &budget,
    )
    .unwrap();
    let image = PathObservationDemand::input_image(
        PathObservationNamespace::Host,
        leaf,
        slug_workspace_v2::ImageObservationRequest::new(
            anchor,
            slug_workspace_v2::ImageObservationOperation::Lstat,
        ),
    )
    .unwrap();
    assert!(
        bodies
            .admit(
                &body
                    .try_union(&SourcePreparationNeeds::path(
                        NeedPathObservations::singleton(image)
                    ))
                    .unwrap()
            )
            .is_err()
    );
    assert_eq!(progress_calls, 0);
    assert!(
        bodies
            .admit(&SourcePreparationNeeds::root_module_bootstrap(
                slug_bzlmod_v2::RootModuleBootstrapRequest {
                    workspace: normalized("/fixture")
                }
            ))
            .is_ok()
    );
}

#[test]
fn fourth_module_need_admission_is_opt_in_and_exact() {
    let normalized = |path: &str| NormalizedAbsolutePath::new(Path::new(path)).unwrap();
    let bodies = Bodies::new([
        normalized("/fixture"),
        normalized("/rules"),
        normalized("/platforms"),
    ]);
    let need = |path: &str, operation| {
        SourcePreparationNeeds::path(NeedPathObservations::singleton(PathObservationDemand::new(
            PathObservationNamespace::Host,
            normalized(path),
            operation,
        )))
    };
    let body = need(
        "/features/MODULE.bazel",
        PathObservationOperation::FileBytes,
    );
    let local = (*local_native_request(&bodies.roots[0], "bazel_features+", "/features")).clone();
    assert!(bodies.admit(&body).is_err());
    assert!(
        bodies
            .admit(&SourcePreparationNeeds::repository(local.clone()))
            .is_err()
    );
    let bodies = bodies.with_fourth(normalized("/features"));
    assert!(bodies.admit(&body).is_ok());
    assert!(
        bodies
            .admit(&need(
                "/features/MODULE.bazel",
                PathObservationOperation::FileDigest
            ))
            .is_ok()
    );
    for path in [
        "/features",
        "/features/MODULE.bazel",
        "/features/BUILD",
        "/features/BUILD.bazel",
        "/features/.bazelignore",
    ] {
        assert!(
            bodies
                .admit(&need(path, PathObservationOperation::Lstat))
                .is_ok()
        );
    }
    assert!(
        bodies
            .admit(&SourcePreparationNeeds::repository(local.clone()))
            .is_ok()
    );
    let original = need("/fixture/MODULE.bazel", PathObservationOperation::FileBytes);
    let foreign =
        SourcePreparationNeeds::path(NeedPathObservations::singleton(PathObservationDemand::new(
            PathObservationNamespace::Materialization(
                slug_workspace_v2::PathObservationInstanceId::new(2),
            ),
            normalized("/features/MODULE.bazel"),
            PathObservationOperation::FileBytes,
        )));
    let mut progress_calls = 0;
    for excluded in [
        need(
            "/features_extra/MODULE.bazel",
            PathObservationOperation::FileBytes,
        ),
        need("/features/BUILD.bazel", PathObservationOperation::FileBytes),
        need("/features/REPO.bazel", PathObservationOperation::Lstat),
        need("/features/MODULE.bazel", PathObservationOperation::ReadLink),
        need("/features", PathObservationOperation::DirectoryEntries),
        foreign,
    ] {
        let mixed = original.try_union(&excluded).unwrap();
        if bodies.admit(&mixed).is_ok() {
            progress_calls += 1;
        }
        assert!(bodies.admit(&mixed).is_err());
    }
    for case in 0..7 {
        let mut wrong = local.clone();
        match case {
            0 => {
                wrong.id.canonical_repo =
                    slug_identity_v2::CanonicalRepoName::new("bazel_features").unwrap()
            }
            1 => wrong.id.workspace = normalized("/other"),
            2 => wrong.kind = RepositoryMaterializationKind::Immutable,
            3 => {
                wrong.kind = RepositoryMaterializationKind::Local {
                    logical_root: normalized("/other"),
                }
            }
            4 => wrong.repo_spec.rule_id.rule_name = "http_archive".into(),
            5 => {
                Arc::make_mut(&mut wrong.repo_spec.attributes).insert(
                    "path".into(),
                    OverrideAttributeValue::String("/other".into()),
                );
            }
            6 => {
                wrong.repo_spec.rule_id.bzl_file =
                    CanonicalLabel::parse("@@other//tools/build_defs/repo:local.bzl").unwrap()
            }
            _ => unreachable!(),
        }
        let mixed = original
            .try_union(&SourcePreparationNeeds::repository(wrong))
            .unwrap();
        if bodies.admit(&mixed).is_ok() {
            progress_calls += 1;
        }
        assert!(bodies.admit(&mixed).is_err());
    }
    assert_eq!(progress_calls, 0);
}

#[test]
fn fifth_module_need_admission_is_opt_in_and_exact() {
    additional_module_need_admission(ModuleMode::Five);
}

#[test]
fn sixth_module_need_admission_is_opt_in_and_exact() {
    additional_module_need_admission(ModuleMode::Six);
}

#[test]
fn seventh_module_need_admission_is_opt_in_and_exact() {
    additional_module_need_admission(ModuleMode::Seven);
}

#[test]
fn eighth_module_need_admission_is_opt_in_and_exact() {
    additional_module_need_admission(ModuleMode::Eight);
}

#[test]
fn ninth_module_need_admission_is_opt_in_and_exact() {
    additional_module_need_admission(ModuleMode::Nine);
}

#[test]
fn tenth_module_need_admission_is_opt_in_and_exact() {
    additional_module_need_admission(ModuleMode::Ten);
}

#[test]
fn eleventh_module_need_admission_is_opt_in_and_exact() {
    additional_module_need_admission(ModuleMode::Eleven);
}

fn additional_module_need_admission(mode: ModuleMode) {
    let slot = match mode {
        ModuleMode::Five => 1,
        ModuleMode::Six => 2,
        ModuleMode::Seven => 3,
        ModuleMode::Eight => 4,
        ModuleMode::Nine => 5,
        ModuleMode::Ten => 6,
        ModuleMode::Eleven => 7,
        _ => unreachable!(),
    };
    let roots = [
        "/features",
        "/skylib",
        "/cc",
        "/license",
        "/shell",
        "/apple",
        "/metadata",
        "/buildozer",
    ];
    let paths = match mode {
        ModuleMode::Five => [
            "/",
            "/skylib",
            "/skylib/MODULE.bazel",
            "/skylib/BUILD",
            "/skylib/BUILD.bazel",
            "/skylib/.bazelignore",
        ],
        ModuleMode::Six => [
            "/",
            "/cc",
            "/cc/MODULE.bazel",
            "/cc/BUILD",
            "/cc/BUILD.bazel",
            "/cc/.bazelignore",
        ],
        ModuleMode::Seven => [
            "/",
            "/license",
            "/license/MODULE.bazel",
            "/license/BUILD",
            "/license/BUILD.bazel",
            "/license/.bazelignore",
        ],
        ModuleMode::Eight => [
            "/",
            "/shell",
            "/shell/MODULE.bazel",
            "/shell/BUILD",
            "/shell/BUILD.bazel",
            "/shell/.bazelignore",
        ],
        ModuleMode::Nine => [
            "/",
            "/apple",
            "/apple/MODULE.bazel",
            "/apple/BUILD",
            "/apple/BUILD.bazel",
            "/apple/.bazelignore",
        ],
        ModuleMode::Ten => [
            "/",
            "/metadata",
            "/metadata/MODULE.bazel",
            "/metadata/BUILD",
            "/metadata/BUILD.bazel",
            "/metadata/.bazelignore",
        ],
        ModuleMode::Eleven => [
            "/",
            "/buildozer",
            "/buildozer/MODULE.bazel",
            "/buildozer/BUILD",
            "/buildozer/BUILD.bazel",
            "/buildozer/.bazelignore",
        ],
        _ => unreachable!(),
    };
    let excluded_paths = match mode {
        ModuleMode::Five => [
            "/skylib_extra/MODULE.bazel",
            "/skylib/BUILD.bazel",
            "/skylib/REPO.bazel",
            "/skylib/MODULE.bazel",
        ],
        ModuleMode::Six => [
            "/cc_extra/MODULE.bazel",
            "/cc/BUILD.bazel",
            "/cc/REPO.bazel",
            "/cc/MODULE.bazel",
        ],
        ModuleMode::Seven => [
            "/license_extra/MODULE.bazel",
            "/license/BUILD.bazel",
            "/license/REPO.bazel",
            "/license/MODULE.bazel",
        ],
        ModuleMode::Eight => [
            "/shell_extra/MODULE.bazel",
            "/shell/BUILD.bazel",
            "/shell/REPO.bazel",
            "/shell/MODULE.bazel",
        ],
        ModuleMode::Nine => [
            "/apple_extra/MODULE.bazel",
            "/apple/BUILD.bazel",
            "/apple/REPO.bazel",
            "/apple/MODULE.bazel",
        ],
        ModuleMode::Ten => [
            "/metadata_extra/MODULE.bazel",
            "/metadata/BUILD.bazel",
            "/metadata/REPO.bazel",
            "/metadata/MODULE.bazel",
        ],
        ModuleMode::Eleven => [
            "/buildozer_extra/MODULE.bazel",
            "/buildozer/BUILD.bazel",
            "/buildozer/REPO.bazel",
            "/buildozer/MODULE.bazel",
        ],
        _ => unreachable!(),
    };
    let (name, canonical) = ADDITIONAL_IDENTITIES[slot];
    let normalized = |path: &str| NormalizedAbsolutePath::new(Path::new(path)).unwrap();
    let base = || {
        Bodies::new([
            normalized("/fixture"),
            normalized("/rules"),
            normalized("/platforms"),
        ])
    };
    let need = |path: &str, operation| {
        SourcePreparationNeeds::path(NeedPathObservations::singleton(PathObservationDemand::new(
            PathObservationNamespace::Host,
            normalized(path),
            operation,
        )))
    };
    let configured = |count| {
        let mut bodies = base();
        if count >= 1 {
            bodies = bodies.with_fourth(normalized(roots[0]));
        }
        if count >= 2 {
            bodies = bodies.with_fifth(normalized(roots[1]));
        }
        if count >= 3 {
            bodies = bodies.with_sixth(normalized(roots[2]));
        }
        if count >= 4 {
            bodies = bodies.with_seventh(normalized(roots[3]));
        }
        if count >= 5 {
            bodies = bodies.with_eighth(normalized(roots[4]));
        }
        if count >= 6 {
            bodies = bodies.with_ninth(normalized(roots[5]));
        }
        if count >= 7 {
            bodies = bodies.with_tenth(normalized(roots[6]));
        }
        if count >= 8 {
            bodies = bodies.with_eleventh(normalized(roots[7]));
        }
        bodies
    };
    let body = need(paths[2], PathObservationOperation::FileBytes);
    let local = (*local_native_request(&normalized("/fixture"), canonical, roots[slot])).clone();
    for count in 0..=slot {
        let bodies = configured(count);
        assert!(bodies.admit(&body).is_err());
        assert!(
            bodies
                .admit(&SourcePreparationNeeds::repository(local.clone()))
                .is_err()
        );
    }
    for count in 0..slot {
        assert!(
            std::panic::catch_unwind(|| match mode {
                ModuleMode::Five => configured(count).with_fifth(normalized(roots[slot])),
                ModuleMode::Six => configured(count).with_sixth(normalized(roots[slot])),
                ModuleMode::Seven => configured(count).with_seventh(normalized(roots[slot])),
                ModuleMode::Eight => configured(count).with_eighth(normalized(roots[slot])),
                ModuleMode::Nine => configured(count).with_ninth(normalized(roots[slot])),
                ModuleMode::Ten => configured(count).with_tenth(normalized(roots[slot])),
                ModuleMode::Eleven => configured(count).with_eleventh(normalized(roots[slot])),
                _ => unreachable!(),
            })
            .is_err()
        );
    }
    let bodies = configured(slot + 1);
    assert!(bodies.admit(&body).is_ok());
    assert!(
        bodies
            .admit(&need(paths[2], PathObservationOperation::FileDigest))
            .is_ok()
    );
    for path in paths {
        assert!(
            bodies
                .admit(&need(path, PathObservationOperation::Lstat))
                .is_ok()
        );
    }
    assert!(
        bodies
            .admit(&SourcePreparationNeeds::repository(local.clone()))
            .is_ok()
    );
    let original = need("/fixture/MODULE.bazel", PathObservationOperation::FileBytes);
    for excluded in [
        need(excluded_paths[0], PathObservationOperation::FileBytes),
        need(excluded_paths[1], PathObservationOperation::FileBytes),
        need(excluded_paths[2], PathObservationOperation::Lstat),
        need(excluded_paths[3], PathObservationOperation::ReadLink),
    ] {
        assert!(
            bodies
                .admit(&original.try_union(&excluded).unwrap())
                .is_err()
        );
    }
    let reject_local = |wrong| {
        assert!(
            bodies
                .admit(
                    &original
                        .try_union(&SourcePreparationNeeds::repository(wrong))
                        .unwrap()
                )
                .is_err()
        );
    };
    for case in [0, 1, 2, 3, 4, 5, 6, 9] {
        let mut wrong = local.clone();
        match case {
            0 => wrong.id.canonical_repo = slug_identity_v2::CanonicalRepoName::new(name).unwrap(),
            1 => wrong.id.workspace = normalized("/other"),
            2 => wrong.kind = RepositoryMaterializationKind::Immutable,
            3 => {
                let path = "/other";
                wrong.kind = RepositoryMaterializationKind::Local {
                    logical_root: normalized(path),
                };
                Arc::make_mut(&mut wrong.repo_spec.attributes)
                    .insert("path".into(), OverrideAttributeValue::String(path.into()));
            }
            4 => wrong.repo_spec.rule_id.rule_name = "http_archive".into(),
            5 => {
                Arc::make_mut(&mut wrong.repo_spec.attributes).insert(
                    "path".into(),
                    OverrideAttributeValue::String("/other".into()),
                );
            }
            6 => {
                wrong.repo_spec.rule_id.bzl_file =
                    CanonicalLabel::parse("@@other//tools/build_defs/repo:local.bzl").unwrap()
            }
            9 => {
                Arc::make_mut(&mut wrong.repo_spec.attributes)
                    .insert("extra".into(), OverrideAttributeValue::String("x".into()));
            }
            _ => unreachable!(),
        }
        reject_local(wrong);
    }
    for prior in 0..slot {
        let mut wrong = local.clone();
        wrong.id.canonical_repo =
            slug_identity_v2::CanonicalRepoName::new(ADDITIONAL_IDENTITIES[prior].1).unwrap();
        reject_local(wrong);
        let mut wrong = local.clone();
        wrong.kind = RepositoryMaterializationKind::Local {
            logical_root: normalized(roots[prior]),
        };
        Arc::make_mut(&mut wrong.repo_spec.attributes).insert(
            "path".into(),
            OverrideAttributeValue::String(roots[prior].into()),
        );
        reject_local(wrong);
    }
}

#[test]
#[ignore = "frozen scoped native MODULE preparation observation"]
fn native_root_module_and_normal_mapping_prerequisite() {
    observe_native_modules(ModuleMode::Three);
}

#[test]
#[ignore = "frozen scoped four-root native MODULE readiness observation"]
fn native_four_root_local_module_and_mapping_prerequisite() {
    observe_native_modules(ModuleMode::Four);
}

#[test]
#[ignore = "frozen scoped five-root native MODULE readiness observation"]
fn native_five_root_local_module_and_mapping_prerequisite() {
    observe_native_modules(ModuleMode::Five);
}

#[test]
#[ignore = "frozen scoped six-root explicit local MODULE readiness observation"]
fn native_six_root_local_module_and_mapping_prerequisite() {
    observe_native_modules(ModuleMode::Six);
}

#[test]
#[ignore = "frozen scoped seven-root normal Local MODULE readiness observation"]
fn native_seven_root_local_module_and_mapping_prerequisite() {
    observe_native_modules(ModuleMode::Seven);
}

#[test]
#[ignore = "frozen scoped eight-root normal Local MODULE readiness observation"]
fn native_eight_root_local_module_and_mapping_prerequisite() {
    observe_native_modules(ModuleMode::Eight);
}

#[test]
#[ignore = "frozen scoped nine-root explicit Local MODULE readiness observation"]
fn native_nine_root_local_module_and_mapping_prerequisite() {
    observe_native_modules(ModuleMode::Nine);
}

#[test]
#[ignore = "frozen scoped ten-root normal Local MODULE readiness observation"]
fn native_ten_root_local_module_and_mapping_prerequisite() {
    observe_native_modules(ModuleMode::Ten);
}

#[test]
#[ignore = "frozen scoped eleven-root normal Local MODULE readiness observation"]
fn native_eleven_root_local_module_and_mapping_prerequisite() {
    observe_native_modules(ModuleMode::Eleven);
}

#[test]
#[ignore = "frozen scoped eleven-root prepared file-registry readiness observation"]
fn native_eleven_root_prepared_file_registry_and_mapping_prerequisite() {
    observe_native_modules_with_registry(ModuleMode::Eleven, RegistryChoice::PreparedFile);
}
fn observe_native_modules(mode: ModuleMode) {
    observe_native_modules_with_registry(mode, RegistryChoice::DefaultBcr);
}
fn observe_native_modules_with_registry(mode: ModuleMode, registry_choice: RegistryChoice) {
    observe_native_command(mode, registry_choice, CommandChoice::Modules);
}
fn observe_native_command(
    mode: ModuleMode,
    registry_choice: RegistryChoice,
    command: CommandChoice,
) {
    phase("baseline");
    let workspace = NormalizedAbsolutePath::new(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/v2_oracle/fixtures/rules-rust-073-toolchain-owner/workspace"),
    )
    .unwrap();
    let registry_urls = registry_choice.urls(|| {
        let expected = NormalizedAbsolutePath::new(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/wp954/registry"),
        )
        .unwrap();
        prepared_file_registry(
            &std::env::var("SLUG_NATIVE_FILE_REGISTRY_ROOT").expect("frozen registry root"),
            expected.as_path(),
        )
        .expect("exact prepared file registry")
    });
    let request_registry = match registry_choice {
        RegistryChoice::DefaultBcr => "https://bcr.bazel.build/",
        RegistryChoice::PreparedFile => registry_urls.as_slice()[0].as_str(),
    };
    let root = |name| {
        let spelling = std::env::var(name).expect("frozen supervisor root");
        assert!(spelling.len() <= 1024 && !spelling.chars().any(char::is_whitespace));
        let normalized = NormalizedAbsolutePath::new(Path::new(&spelling)).unwrap();
        assert_eq!(normalized.as_path(), Path::new(&spelling));
        normalized
    };
    let mut bodies = Bodies::new([
        workspace.clone(),
        root("SLUG_NATIVE_RULES_RUST_ROOT"),
        root("SLUG_NATIVE_PLATFORMS_ROOT"),
    ])
    .with_visible_lockfile_metadata(registry_choice)
    .with_repo_marker_metadata(registry_choice, command)
    .with_ignore_file_bytes(registry_choice, command)
    .with_rust_package_metadata(registry_choice, command)
    .with_nightly_package_metadata(registry_choice, command)
    .with_nightly_build_source(registry_choice, command)
    .with_nightly_channel_source(registry_choice, command)
    .with_probe_package_source(registry_choice, command)
    .with_rules_rust_defs_source(registry_choice, command)
    .with_rules_rust_defs_import_sources(registry_choice, command)
    .with_bazel_features_import_sources(registry_choice, command)
    .with_generated_features_file_effects(registry_choice, command)
    .with_bazel_skylib_import_sources(registry_choice, command)
    .with_rules_cc_import_sources(registry_choice, command);
    for slot in 0..mode.additional_count() {
        let extra_root = root(ADDITIONAL_ENV[slot]);
        bodies = match slot {
            0 => bodies.with_fourth(extra_root),
            1 => bodies.with_fifth(extra_root),
            2 => bodies.with_sixth(extra_root),
            3 => bodies.with_seventh(extra_root),
            4 => bodies.with_eighth(extra_root),
            5 => bodies.with_ninth(extra_root),
            6 => bodies.with_tenth(extra_root),
            7 => bodies.with_eleventh(extra_root),
            _ => unreachable!(),
        };
        let (prefix, field) = ADDITIONAL_MARKERS[slot];
        println!(
            "{prefix}_REQUEST {field}={}",
            bodies.additional[slot]
                .as_ref()
                .unwrap()
                .root
                .as_path()
                .display()
        );
    }
    let bodies = Arc::new(bodies);
    println!(
        "WP942_REQUEST workspace={} rules_root={} platforms_root={} registry={} lockfile=off local_overrides=true",
        workspace.as_path().display(),
        bodies.roots[1].as_path().display(),
        bodies.roots[2].as_path().display(),
        request_registry
    );
    let visible_lockfile_metadata = bodies.visible_lockfile_metadata;
    let features_import_sources = bodies.bazel_features_import_sources;
    let features_root_present = bodies.additional[0].is_some();
    let start = Instant::now();
    let limits = bodies.source_limits(registry_choice);
    let scope = SourceServiceScope::new_for_test(
        workspace.clone(),
        start + Duration::from_secs(5),
        start + Duration::from_secs(8),
        limits,
    )
    .unwrap();
    if matches!(registry_choice, RegistryChoice::PreparedFile) {
        println!(
            "WP955_LIMITS registry_attempts={} source_attempts={} live_buffer_bytes={}",
            limits.registry_attempts, limits.source_attempts, limits.live_buffer_bytes
        );
    }
    let scope_weak = Arc::downgrade(&scope);
    let (recorder, arm_error) = match scope.arm_registry_association_for_test() {
        Ok(r) => (Some(r), None),
        Err(e) => (None, Some(e)),
    };
    let recorder_weak = recorder.as_ref().map(Arc::downgrade);
    let (mut runtime, constructor_error) = if arm_error.is_none() {
        match WorkspaceRuntime::new_with_source_services(
            workspace.as_path(),
            ProcessHostOwner::native(),
            WorkspaceSourceServices::Diagnostic(scope.clone()),
        ) {
            Ok(r) => (Some(r), None),
            Err(e) => (None, Some(e)),
        }
    } else {
        (None, None)
    };
    let budgets = runtime.as_mut().map(|runtime| {
        let descriptor = slug_workspace_v2::ImageDescriptorBudget::new(16 << 20);
        runtime.image_resources =
            crate::runtime::input_image::WorkspaceImageResources::new(descriptor.clone(), 16 << 20);
        runtime.scratch_resources = slug_bzlmod_v2::SelectedScratchResources::new(
            slug_bzlmod_v2::SelectedScratchPayloadBudget::new(16 << 20),
            descriptor,
        );
        assert!(
            runtime
                .image_resources
                .descriptor_budget()
                .same_instance(runtime.scratch_resources.descriptor_budget())
        );
        (
            runtime.image_resources.clone(),
            runtime.scratch_resources.clone(),
        )
    });
    phase("after_runtime");
    resources("before_command", budgets.as_ref());
    let milestones = Arc::new(Mutex::new(Milestones::default()));
    let mut policy_error = false;
    let mut outcome = None;
    let mut probe_report = ProbeReport::default();
    if matches!(command, CommandChoice::PublicProbe) {
        assert!(matches!(mode, ModuleMode::Eleven));
        assert!(matches!(registry_choice, RegistryChoice::PreparedFile));
        println!("WP958_REQUEST role=public_probe target=//pkg:probe channel=nightly");
    }
    if let Some(runtime) = &runtime {
        let mut overrides: [Option<String>; 10] = Default::default();
        for (slot, (name, _, root)) in bodies.bindings().enumerate() {
            overrides[slot] = Some(format!("{name}={}", root.as_path().display()));
        }
        match BzlmodCommandPolicyKey::from_flags_with_module_overrides(
            None,
            false,
            workspace.as_path(),
            overrides.iter().filter_map(Option::as_deref),
        ) {
            Err(_) => policy_error = true,
            Ok(policy) => {
                if matches!(command, CommandChoice::PublicProbe) {
                    outcome = Some(ObservationOutcome::Probe(
                        public_probe_preparation_tests::prepare_and_drive(
                            runtime,
                            bodies,
                            policy,
                            &registry_urls,
                            milestones.clone(),
                            &mut probe_report,
                        ),
                    ));
                } else {
                    let mut request = NativeDemandRequestInputBundle::normalized_initial();
                    request.command_policy = policy;
                    request.lockfile_mode = LockfileMode::Off;
                    request.registry_urls = registry_urls.dupe();
                    outcome = Some(ObservationOutcome::Modules(
                        runtime.drive_command(
                            request,
                            ModuleRoot {
                                bodies,
                                expected_registry: matches!(
                                    registry_choice,
                                    RegistryChoice::PreparedFile
                                )
                                .then(|| registry_urls.dupe()),
                                milestones: milestones.clone(),
                            },
                        ),
                    ));
                }
            }
        }
    }
    phase("after_command");
    let stage = if arm_error.is_some() {
        "arm_refusal"
    } else if constructor_error.is_some() {
        "constructor_refusal"
    } else if policy_error {
        "policy_refusal"
    } else if matches!(command, CommandChoice::PublicProbe) {
        outcome.as_ref().unwrap().stage()
    } else if outcome.as_ref().is_some_and(ObservationOutcome::is_ok) {
        "mapping_complete"
    } else {
        "native_refusal"
    };
    println!("WP942_STAGE {stage}");
    if let Some(outcome) = &outcome {
        outcome.report_native();
    } else {
        println!("WP942_NATIVE kind=unavailable attempts=unavailable roots=unavailable");
    }
    if matches!(command, CommandChoice::PublicProbe) {
        let early_kind = if arm_error.is_some() {
            "source_service"
        } else if let Some(error) = &constructor_error {
            if error.downcast_ref::<SourceServiceError>().is_some() {
                "source_service"
            } else if error.downcast_ref::<std::io::Error>().is_some() {
                "io"
            } else {
                "anyhow"
            }
        } else if policy_error {
            "module_overrides"
        } else {
            "unavailable"
        };
        probe_report.report(stage, early_kind, outcome.as_ref());
        println!(
            "WP980_FEATURES_IMPORT_SOURCES enabled={} policy_checked={} additional0_present={} source_attempt_limit={} bodies_size={}",
            features_import_sources,
            probe_report.policy_checked(),
            features_root_present,
            limits.source_attempts,
            std::mem::size_of::<Bodies>()
        );
    }
    let (source_weak, additional_weak) = {
        let state = milestones.lock().unwrap();
        if matches!(registry_choice, RegistryChoice::PreparedFile) {
            let url = registry_urls.as_slice()[0].as_str().as_bytes();
            println!(
                "WP954_REGISTRY choice=file url_len={} url_hex={} policy_checked={}",
                url.len(),
                RegistryHex((url.len() <= REGISTRY_IDENTITY_BYTES).then_some(url)),
                state.registry_policy_checked
            );
        }
        for (slot, (prefix, _)) in ADDITIONAL_MARKERS
            .into_iter()
            .take(mode.additional_count())
            .enumerate()
        {
            match state.additional_bytes[slot] {
                Some((bytes, hash_match)) => {
                    println!("{prefix}_SOURCE seen=true bytes={bytes} hash_match={hash_match}")
                }
                None => {
                    println!("{prefix}_SOURCE seen=false bytes=unavailable hash_match=unavailable")
                }
            }
        }
        if mode.additional_count() >= 1 && !state.selected_reported {
            report_selected(None);
        }
        println!(
            "WP942_ROOT_SOURCE seen={} calls={} policy_checked={} bytes={} hash_match={} anchor={} outer_variant={:?}",
            state.preparations != 0,
            state.calls,
            state.effective_empty,
            state.bytes,
            state.hash_match,
            state.anchor.unwrap_or("not_started"),
            state.anchor_outer
        );
        print!(
            "WP942_MAPPING state={} codes=",
            state.mapping.unwrap_or("not_started")
        );
        if let Some(refusal) = state.refusal {
            if refusal.codes().is_empty() {
                print!("none");
            }
            for (i, code) in refusal.codes().iter().enumerate() {
                if i != 0 {
                    print!(",");
                }
                print!("{code:?}");
            }
            println!(
                " termination={:?} native_detail={}",
                refusal.termination(),
                refusal.native_detail_observed()
            );
        } else {
            println!("none termination=none native_detail=false");
        }
        let bytes = state
            .denied
            .as_ref()
            .and_then(|d| d.path.as_ref())
            .map(|path| path.as_path().as_os_str().as_bytes());
        let captured = bytes.is_some_and(|bytes| bytes.len() <= REGISTRY_IDENTITY_BYTES);
        println!(
            "WP942_DENIED family={} path_len={} path_hex={} capture={}",
            state.denied.as_ref().map_or("none", |d| d.family),
            bytes.map_or_else(|| "none".to_owned(), |b| b.len().to_string()),
            RegistryHex(bytes.filter(|_| captured)),
            if captured {
                "captured"
            } else if bytes.is_some() {
                "bounds_refused"
            } else {
                "missing"
            }
        );
        if matches!(registry_choice, RegistryChoice::PreparedFile) {
            println!(
                "WP957_METADATA enabled={} denied_operation={}",
                visible_lockfile_metadata,
                state.denied.as_ref().map_or("none", |d| d.operation)
            );
        }
        (state.source.clone(), state.additional_source.clone())
    };
    let counters = scope.snapshot();
    println!(
        "WP938_SCOPE {:?} {} {} {} {} {} {} {} {}",
        scope.first_error_for_test(),
        counters.registry_attempts,
        counters.registry_admitted_bytes,
        counters.source_attempts,
        counters.transferred_bytes,
        counters.decoded_bytes,
        counters.written_bytes,
        counters.live_buffer_bytes,
        counters.peak_buffer_bytes
    );
    println!("WP940_ARM {arm_error:?}");
    if let Some(recorder) = &recorder {
        report_registry_association(recorder, &counters, scope.first_error_for_test());
    } else {
        println!("WP940_ASSOCIATION unavailable");
        println!("WP940_FIRST_ROW unavailable");
    }
    resources("after_command", budgets.as_ref());
    drop(outcome);
    drop(constructor_error);
    drop(milestones);
    phase("after_terminal_drop");
    drop(runtime);
    phase("after_runtime_drop");
    resources("after_runtime_drop", budgets.as_ref());
    drop(recorder);
    let deadline = Instant::now() + Duration::from_secs(2);
    while (budgets.as_ref().is_some_and(|(image, scratch)| {
        image.descriptor_budget().used_bytes() != 0
            || image.logical_bytes_used() != 0
            || scratch.payload_budget().used_bytes() != 0
    }) || scope.snapshot().live_buffer_bytes != 0
        || source_weak
            .as_ref()
            .is_some_and(|weak| weak.upgrade().is_some())
        || additional_weak
            .iter()
            .flatten()
            .any(|weak| weak.upgrade().is_some())
        || recorder_weak
            .as_ref()
            .is_some_and(|weak| weak.upgrade().is_some()))
        && Instant::now() < deadline
    {
        thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(scope.snapshot().live_buffer_bytes, 0);
    assert!(
        source_weak
            .as_ref()
            .is_none_or(|weak| weak.upgrade().is_none())
    );
    assert!(
        recorder_weak
            .as_ref()
            .is_none_or(|weak| weak.upgrade().is_none())
    );
    assert!(
        additional_weak
            .iter()
            .flatten()
            .all(|weak| weak.upgrade().is_none())
    );
    if let Some((image, scratch)) = &budgets {
        assert_eq!(image.descriptor_budget().used_bytes(), 0);
        assert_eq!(image.logical_bytes_used(), 0);
        assert_eq!(scratch.payload_budget().used_bytes(), 0);
    }
    drop(scope);
    while scope_weak.upgrade().is_some() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(1));
    }
    assert!(scope_weak.upgrade().is_none());
    phase("after_scope_drop");
    println!(
        "WP942_RETIREMENT source_weak_seen={} source_weak_expired=true live_buffer_zero=true scope_weak_expired=true recorder_weak_expired=true",
        source_weak.is_some()
    );
    for (slot, (prefix, _)) in ADDITIONAL_MARKERS
        .into_iter()
        .take(mode.additional_count())
        .enumerate()
    {
        println!(
            "{prefix}_RETIREMENT source_weak_seen={} source_weak_expired=true",
            additional_weak[slot].is_some()
        );
    }
}
