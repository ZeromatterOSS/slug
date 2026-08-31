use std::path::PathBuf;
use std::sync::Arc;

use sha2::Digest;
use sha2::Sha256;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::CanonicalRepoName;

use super::*;

fn identity(repo: &str, package: &str, mapping: &[(&str, &str)]) -> BzlModuleIdentity {
    BzlModuleIdentity {
        label: CanonicalLabel::parse(&format!("@@{repo}//{package}:defs.bzl")).unwrap(),
        workspace_path: PathBuf::from(format!("/workspace/{package}/defs.bzl")),
        repository_mapping: mapping
            .iter()
            .map(|(apparent, canonical)| {
                (
                    ApparentRepoName::new(*apparent).unwrap(),
                    if canonical.is_empty() {
                        CanonicalRepoName::root()
                    } else {
                        CanonicalRepoName::new(*canonical).unwrap()
                    },
                )
            })
            .collect::<Vec<_>>()
            .into(),
    }
}

#[test]
fn pinned_allowlist_inventory_and_repository_branches_are_exact() {
    assert_eq!(
        INTERNAL_STARLARK_API_ALLOWLIST
            .iter()
            .filter(|entry| entry.apparent_repo.is_empty())
            .count(),
        18
    );
    assert_eq!(
        INTERNAL_STARLARK_API_ALLOWLIST
            .iter()
            .filter(|entry| !entry.apparent_repo.is_empty())
            .count(),
        11
    );

    for entry in INTERNAL_STARLARK_API_ALLOWLIST {
        let package = if entry.package_prefix.is_empty() {
            "child"
        } else {
            entry.package_prefix
        };
        let candidate = if entry.apparent_repo.is_empty() {
            identity("", package, &[])
        } else if entry.apparent_repo == "bazel_tools" {
            identity("bazel_tools", package, &[])
        } else {
            identity(&format!("{}+1.2.3", entry.apparent_repo), package, &[])
        };
        assert!(allows(&candidate), "{}/{package}", entry.apparent_repo);
    }

    assert!(allows(&identity("_builtins", "private", &[])));
    assert!(allows(&identity(
        "",
        "rust/private/toolchain",
        &[("rules_rust", "")]
    )));
    assert!(!allows(&identity("rules_cc_evil+1.0", "cc", &[])));
    assert!(!allows(&identity("rules_rust+1.0", "rust/public", &[])));
    assert!(allows(&identity(
        "bazel_tools+1.0",
        "tools/build_defs/build_info",
        &[]
    )));
    assert!(!allows(&identity(
        "bazel_tools_evil+1.0",
        "tools/build_defs/build_info",
        &[]
    )));
    assert!(!allows(&identity("", "tools/build_defs/cc_evil", &[])));
}

#[test]
fn caller_manifest_participates_structurally() {
    let first: Arc<[(CompactString, BzlModuleIdentity)]> =
        Arc::from([("defs.bzl".into(), identity("", "user", &[("rules_cc", "")]))]);
    let second: Arc<[(CompactString, BzlModuleIdentity)]> = Arc::from([(
        "defs.bzl".into(),
        identity("", "user", &[("rules_rust", "")]),
    )]);
    assert_ne!(first, second);
    assert!(allows(&first[0].1));
    assert!(!allows(&second[0].1));
    assert!(allows(&first[0].1));
}

#[test]
#[ignore = "requires the realized rules_cc 0.2.17 BCR source tree"]
fn authenticated_rules_cc_fdo_context_has_the_reviewed_fragment_call_ledger() {
    let source_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
        "../../target/v2o/ob/registry-yanked-lockfile-mode/bazel/external/rules_cc+/cc/private/rules_impl/fdo/fdo_context.bzl",
    );
    let source = std::fs::read(&source_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", source_path.display()));
    assert_eq!(
        format!("{:x}", Sha256::digest(&source)),
        "91b7b46c515b4773d5a241e699027212f679ab93160cc79218bd687eac51d5b7"
    );
    let source = std::str::from_utf8(&source).unwrap();
    let mut cursor = 0;
    for method in [
        "compilation_mode()",
        "propeller_optimize_absolute_cc_profile()",
        "propeller_optimize_absolute_ld_profile()",
        "fdo_path()",
        "cs_fdo_path()",
        "proto_profile()",
    ] {
        let offset = source[cursor..]
            .find(method)
            .unwrap_or_else(|| panic!("missing reviewed C++ fragment call {method}"));
        cursor += offset + method.len();
    }
    let first_action = source.find("ctx.actions.args()").unwrap();
    assert!(cursor < first_action);
}
