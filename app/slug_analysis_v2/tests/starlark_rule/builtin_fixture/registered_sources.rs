//! Caller-provided local source topology for registered builtin toolchains.
//! Original/generated bodies execute normally; this is neither generator
//! execution nor a claim about generated-repository canonical identities.
use std::fs;
use std::path::Path;

#[path = "registered_sources/members.rs"]
mod members;

pub(super) const EXTRA_MODULES: &[(&str, &str)] = &[
    ("bazel_skylib", "1.8.2"),
    ("cc_compatibility_proxy", "1.0"),
    ("bazel_features_globals", "1.0"),
    ("bazel_features_version", "1.0"),
];

pub(super) fn module_dependencies(name: &str) -> &'static str {
    match name {
        "rules_cc" => {
            "bazel_dep(name = \"platforms\", version = \"1.0.0\")\nbazel_dep(name = \"bazel_skylib\", version = \"1.8.2\")\nbazel_dep(name = \"protobuf\", version = \"33.4\", repo_name = \"com_google_protobuf\")\nbazel_dep(name = \"cc_compatibility_proxy\", version = \"1.0\")\n"
        }
        "cc_compatibility_proxy" => {
            "bazel_dep(name = \"rules_cc\", version = \"0.2.17\")\nbazel_dep(name = \"bazel_skylib\", version = \"1.8.2\")\n"
        }
        "protobuf" => {
            "bazel_dep(name = \"bazel_features\", version = \"1.42.1\", repo_name = \"proto_bazel_features\")\nbazel_dep(name = \"bazel_skylib\", version = \"1.8.2\")\n"
        }
        "bazel_features" => {
            "bazel_dep(name = \"bazel_features_globals\", version = \"1.0\")\nbazel_dep(name = \"bazel_features_version\", version = \"1.0\")\nbazel_dep(name = \"bazel_skylib\", version = \"1.8.2\")\n"
        }
        "bazel_features_globals" => "bazel_dep(name = \"bazel_skylib\", version = \"1.8.2\")\n",
        "bazel_features_version" => "bazel_dep(name = \"bazel_skylib\", version = \"1.8.2\")\n",
        "bazel_skylib" => "bazel_dep(name = \"rules_license\", version = \"1.0.0\")\n",
        _ => "",
    }
}

pub(super) fn prepare(name: &str, directory: &Path) {
    for &(repo, relative, source) in members::SOURCES {
        if repo == name {
            let path = directory.join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            super::write_if_missing(&path, source);
        }
    }
}

#[cfg(test)]
#[path = "registered_sources/tests.rs"]
mod tests;
