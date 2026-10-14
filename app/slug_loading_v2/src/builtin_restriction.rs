//! Bazel 9.2 restricted-native caller checks shared by analysis facades.

use std::sync::Arc;

use compact_str::CompactString;
use starlark::eval::Evaluator;

use crate::BzlModuleIdentity;
use crate::BzlModuleSourceProvenance;
use crate::provider::BzlEvaluationContext;
use crate::subrule_invocation::AnalysisEvaluationContext;

#[derive(Clone, Copy)]
struct AllowlistEntry {
    apparent_repo: &'static str,
    package_prefix: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CustomAllowlistEntry {
    pub(crate) apparent_repo: CompactString,
    pub(crate) package_prefix: CompactString,
}

pub(crate) fn source_identities_for_evaluator(
    eval: &Evaluator<'_, '_, '_>,
) -> anyhow::Result<Arc<[(CompactString, BzlModuleSourceProvenance)]>> {
    if let Ok(context) = BzlEvaluationContext::from_evaluator(eval) {
        return Ok(context.source_identities_by_filename());
    }
    if let Ok(context) = AnalysisEvaluationContext::from_evaluator(eval) {
        return Ok(context.source_identities_by_filename().clone());
    }
    anyhow::bail!("source-sensitive builtin requires loading or configured evaluation")
}

const INTERNAL_STARLARK_API_ALLOWLIST: &[AllowlistEntry] = &[
    AllowlistEntry {
        apparent_repo: "",
        package_prefix: "test",
    },
    AllowlistEntry {
        apparent_repo: "",
        package_prefix: "bazel_internal/test_rules",
    },
    AllowlistEntry {
        apparent_repo: "",
        package_prefix: "tools/build_defs/build_info",
    },
    AllowlistEntry {
        apparent_repo: "bazel_tools",
        package_prefix: "tools/build_defs/build_info",
    },
    AllowlistEntry {
        apparent_repo: "",
        package_prefix: "bazel_internal/test_rules/cc",
    },
    AllowlistEntry {
        apparent_repo: "",
        package_prefix: "tools/build_defs/android",
    },
    AllowlistEntry {
        apparent_repo: "",
        package_prefix: "third_party/bazel_rules/rules_android",
    },
    AllowlistEntry {
        apparent_repo: "rules_android",
        package_prefix: "",
    },
    AllowlistEntry {
        apparent_repo: "build_bazel_rules_android",
        package_prefix: "",
    },
    AllowlistEntry {
        apparent_repo: "",
        package_prefix: "third_party/apple_crosstool",
    },
    AllowlistEntry {
        apparent_repo: "",
        package_prefix: "third_party/cpptoolchains/portable_llvm/build_defs",
    },
    AllowlistEntry {
        apparent_repo: "",
        package_prefix: "third_party/bazel_rules/rules_apple",
    },
    AllowlistEntry {
        apparent_repo: "rules_apple",
        package_prefix: "",
    },
    AllowlistEntry {
        apparent_repo: "build_bazel_rules_apple",
        package_prefix: "",
    },
    AllowlistEntry {
        apparent_repo: "",
        package_prefix: "third_party/bazel_rules/rules_cc",
    },
    AllowlistEntry {
        apparent_repo: "",
        package_prefix: "tools/build_defs/cc",
    },
    AllowlistEntry {
        apparent_repo: "rules_cc",
        package_prefix: "",
    },
    AllowlistEntry {
        apparent_repo: "",
        package_prefix: "third_party/bazel_rules/rules_java/java",
    },
    AllowlistEntry {
        apparent_repo: "rules_java",
        package_prefix: "java",
    },
    AllowlistEntry {
        apparent_repo: "",
        package_prefix: "third_party/bazel_rules/rules_rust/rust/private",
    },
    AllowlistEntry {
        apparent_repo: "",
        package_prefix: "third_party/crubit",
    },
    AllowlistEntry {
        apparent_repo: "rules_rust",
        package_prefix: "rust/private",
    },
    AllowlistEntry {
        apparent_repo: "",
        package_prefix: "third_party/gpus/cuda",
    },
    AllowlistEntry {
        apparent_repo: "",
        package_prefix: "tools/build_defs/packaging",
    },
    AllowlistEntry {
        apparent_repo: "",
        package_prefix: "tools/build_defs/go",
    },
    AllowlistEntry {
        apparent_repo: "",
        package_prefix: "third_party/protobuf",
    },
    AllowlistEntry {
        apparent_repo: "protobuf",
        package_prefix: "",
    },
    AllowlistEntry {
        apparent_repo: "com_google_protobuf",
        package_prefix: "",
    },
    AllowlistEntry {
        apparent_repo: "rules_shell",
        package_prefix: "",
    },
];

pub(crate) fn check_default_allowlist(
    eval: &Evaluator<'_, '_, '_>,
    identities: &Arc<[(CompactString, BzlModuleSourceProvenance)]>,
) -> anyhow::Result<()> {
    let data = eval
        .native_call_defining_module_client_data()
        .ok_or_else(|| {
            anyhow::anyhow!("restricted private API requires actual defining .bzl provenance")
        })?;
    let identity = crate::starlark_label::validate_defining_module_source(data, identities)?;
    if allows(identity) {
        return Ok(());
    }
    anyhow::bail!("file '{}' cannot use private API", identity.label)
}

pub(crate) fn check_custom_allowlist(
    eval: &Evaluator<'_, '_, '_>,
    identities: &Arc<[(CompactString, BzlModuleSourceProvenance)]>,
    allowlist: &[CustomAllowlistEntry],
    depth: usize,
) -> anyhow::Result<()> {
    let Some(data) = eval.native_call_defining_module_client_data_at_depth(depth) else {
        // Bazel admits execution callbacks with no enclosing Starlark function.
        if eval
            .native_caller_function_filename_at_depth(depth)
            .is_none()
        {
            return Ok(());
        }
        anyhow::bail!("restricted private API requires actual defining .bzl provenance");
    };
    let identity = crate::starlark_label::validate_defining_module_source(data, identities)?;
    if allowlist.iter().any(|entry| {
        repository_matches(identity, &entry.apparent_repo)
            && package_starts_with(
                identity.label.package().package().as_str(),
                &entry.package_prefix,
            )
    }) {
        return Ok(());
    }
    anyhow::bail!("file '{}' cannot use private API", identity.label)
}

fn allows(identity: &BzlModuleIdentity) -> bool {
    let package = identity.label.package();
    let repo = package.repo().as_str();
    if repo == "_builtins" {
        return true;
    }
    let path = package.package().as_str();
    INTERNAL_STARLARK_API_ALLOWLIST.iter().any(|entry| {
        repository_matches(identity, entry.apparent_repo)
            && package_starts_with(path, entry.package_prefix)
    })
}

fn repository_matches(identity: &BzlModuleIdentity, apparent: &str) -> bool {
    let repo = identity.label.package().repo();
    if repo.is_root() {
        return apparent.is_empty()
            || identity
                .repository_mapping
                .iter()
                .any(|(name, canonical)| name.as_str() == apparent && canonical.is_root());
    }
    if repo.as_str() == "bazel_tools" {
        return apparent == "bazel_tools";
    }
    !apparent.is_empty()
        && (repo.as_str() == apparent
            || repo
                .as_str()
                .strip_prefix(apparent)
                .is_some_and(|suffix| suffix.starts_with('+')))
}

fn package_starts_with(package: &str, prefix: &str) -> bool {
    prefix.is_empty()
        || package == prefix
        || package
            .strip_prefix(prefix)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

#[cfg(test)]
#[path = "builtin_restriction_tests.rs"]
mod tests;
