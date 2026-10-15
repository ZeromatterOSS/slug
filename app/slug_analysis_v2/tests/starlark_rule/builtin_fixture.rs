//! Actual local MODULE inputs for the bundled bazel_tools dependency closure.
//! This prepares files, not discovery/mapping results or a bazel_tools override.
use std::fs;
use std::path::Path;

use slug_bzlmod_v2::Directive;
use slug_bzlmod_v2::ModuleFile;

#[path = "builtin_fixture/registered_sources.rs"]
mod registered_sources;

#[path = "builtin_fixture/host_inputs.rs"]
mod host_inputs;
pub(super) use host_inputs::RepositoryHostInputs;

#[path = "builtin_fixture/generated_registration.rs"]
mod generated_registration;
pub(super) use generated_registration::FixtureObservations;
pub(super) use generated_registration::SavedInputs;
pub(super) use generated_registration::capture as capture_generated_inputs;
pub(super) use generated_registration::prepare as prepare_generated_registrations;

#[path = "builtin_fixture/requests.rs"]
mod requests;
pub(super) use requests::data as request_data;
pub(super) use requests::ready as ready_transaction;

#[path = "builtin_fixture/platform_observations.rs"]
mod platform_observations;
pub(super) use platform_observations::observations as platform_constraint_observations;

const MODULES: [(&str, &str); 14] = [
    ("rules_license", "1.0.0"),
    ("buildozer", "8.5.1"),
    ("platforms", "1.0.0"),
    ("zlib", "1.3.1.bcr.5"),
    ("bazel_features", "1.42.1"),
    ("protobuf", "33.4"),
    ("rules_java", "9.1.0"),
    ("rules_cc", "0.2.17"),
    ("rules_python", "1.7.0"),
    ("rules_shell", "0.6.1"),
    ("apple_support", "1.24.2"),
    ("rules_apple", "4.1.0"),
    ("rules_swift", "3.1.2"),
    ("abseil-cpp", "20250814.1"),
];

/// Automatic positive-fixture preparation must leave missing, malformed, and
/// Include-bearing MODULE inputs to the production preparation path unchanged.
pub(super) fn prepare_if_ordinary(workspace: &Path) -> Vec<(String, String)> {
    let source = match fs::read_to_string(workspace.join("MODULE.bazel")) {
        Ok(source) => source,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(error) => panic!("read fixture MODULE.bazel: {error}"),
    };
    let Ok(parsed) = ModuleFile::parse(&source) else {
        return Vec::new();
    };
    if parsed
        .directives
        .iter()
        .any(|d| matches!(d, Directive::Include(_)))
    {
        return Vec::new();
    }
    prepare(workspace)
}

/// Augment an ordinary root MODULE without replacing any authored directive or
/// file. Existing local overrides retain their own paths; nonlocal overrides
/// retain their own source preparation and receive no fake materialization.
/// Included MODULE fragments must be prepared by their caller, not guessed.
pub(super) fn prepare(workspace: &Path) -> Vec<(String, String)> {
    let root = workspace.join("MODULE.bazel");
    let mut source = fs::read_to_string(&root).unwrap();
    let original_len = source.len();
    let parsed = ModuleFile::parse(&source).unwrap();
    assert!(
        !parsed
            .directives
            .iter()
            .any(|d| matches!(d, Directive::Include(_))),
        "builtin fixture preparation requires explicit root directives; prepare included fragments at the caller"
    );
    let mut repositories = Vec::new();
    for (name, pinned_version) in MODULES
        .into_iter()
        .chain(registered_sources::EXTRA_MODULES.iter().copied())
    {
        let dependency = parsed.directives.iter().find_map(|d| match d {
            Directive::BazelDep(dep) if dep.name == name => Some(dep),
            _ => None,
        });
        let local = parsed.directives.iter().find_map(|d| match d {
            Directive::LocalPathOverride(value) if value.module_name == name => Some(value),
            _ => None,
        });
        let nonlocal = parsed.directives.iter().any(|d| match d {
            Directive::SingleVersionOverride(v) => v.module_name == name,
            Directive::MultipleVersionOverride(v) => v.module_name == name,
            Directive::ArchiveOverride(v) => v.module_name == name,
            Directive::GitOverride(v) => v.module_name == name,
            _ => false,
        });
        if nonlocal {
            continue;
        }
        let path = local.map_or(name, |value| value.path.as_str());
        let directory = workspace.join(path);
        fs::create_dir_all(&directory).unwrap();
        let mut module = format!(
            "module(name = {name:?}, version = {:?})\n",
            dependency.map_or(pinned_version, |dep| dep.version.as_str())
        );
        if name == "rules_shell" {
            // Its verbatim sh_executable.bzl constructs this real dependency
            // label while declaring sh_binary. Match the upstream dependency,
            // without activating unrelated shell-config extensions/toolchains.
            module.push_str("bazel_dep(name = 'platforms', version = '0.0.10')\n");
        }
        module.push_str(registered_sources::module_dependencies(name));
        write_if_missing(&directory.join("MODULE.bazel"), &module);
        if local.is_none() {
            source.push_str(&format!(
                "\nlocal_path_override(module_name = {name:?}, path = {name:?})\n"
            ));
        }
        if dependency.is_none()
            && (matches!(
                name,
                "bazel_features" | "rules_apple" | "rules_swift" | "abseil-cpp"
            ) || registered_sources::EXTRA_MODULES
                .iter()
                .any(|(extra, _)| *extra == name))
        {
            source.push_str(&format!(
                "bazel_dep(name = {name:?}, version = {pinned_version:?})\n"
            ));
        }
        if name == "platforms" {
            prepare_platforms(&directory);
            repositories.push((name.to_owned(), path.to_owned()));
        }
        if name == "rules_shell" {
            prepare_shell_loads(&directory);
        }
        registered_sources::prepare(name, &directory);
        repositories.push((format!("{name}+"), path.to_owned()));
    }
    // Validate the augmented text through the same MODULE grammar before use.
    ModuleFile::parse(&source).unwrap();
    if source.len() != original_len {
        fs::write(root, source).unwrap();
    }
    repositories
}

/// Read the currently authored local topology without preparing files or
/// restoring overrides deliberately removed by selected-registry fixtures.
pub(super) fn local_repositories(workspace: &Path) -> Vec<(String, String)> {
    let source = fs::read_to_string(workspace.join("MODULE.bazel")).unwrap();
    let parsed = ModuleFile::parse(&source).unwrap();
    let mut repositories = Vec::new();
    for directive in &parsed.directives {
        let Directive::LocalPathOverride(local) = directive else {
            continue;
        };
        if !MODULES
            .iter()
            .chain(registered_sources::EXTRA_MODULES.iter())
            .any(|(name, _)| *name == local.module_name)
        {
            continue;
        }
        if local.module_name == "platforms" {
            repositories.push((local.module_name.clone(), local.path.clone()));
        }
        repositories.push((format!("{}+", local.module_name), local.path.clone()));
    }
    repositories
}

fn write_if_missing(path: &Path, text: &str) {
    if !path.try_exists().unwrap() {
        fs::write(path, text).unwrap();
    }
}

// Verbatim rules_shell 0.6.1 archive members. The actual bazel_tools tools/test
// package loads sh_binary while resolving its registered toolchain declarations.
// Keep both real package boundaries and the complete three-file load closure;
// this supplies source files rather than a replacement sh_binary implementation.
fn prepare_shell_loads(directory: &Path) {
    for (relative, source) in [
        (
            "shell/BUILD",
            include_str!("builtin_fixture/rules_shell/shell/BUILD"),
        ),
        (
            "shell/private/BUILD",
            include_str!("builtin_fixture/rules_shell/shell/private/BUILD"),
        ),
        (
            "shell/sh_binary.bzl",
            include_str!("builtin_fixture/rules_shell/shell/sh_binary.bzl"),
        ),
        (
            "shell/private/sh_binary.bzl",
            include_str!("builtin_fixture/rules_shell/shell/private/sh_binary.bzl"),
        ),
        (
            "shell/private/sh_executable.bzl",
            include_str!("builtin_fixture/rules_shell/shell/private/sh_executable.bzl"),
        ),
    ] {
        let path = directory.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        write_if_missing(&path, source);
    }
}

fn prepare_platforms(directory: &Path) {
    fs::create_dir_all(directory.join("host")).unwrap();
    fs::create_dir_all(directory.join("cpu")).unwrap();
    fs::create_dir_all(directory.join("os")).unwrap();
    for (path, text) in [
        ("REPO.bazel", ""),
        (".bazelignore", ""),
        (
            "host/constraints.bzl",
            "HOST_CONSTRAINTS = ['@platforms//cpu:x86_64']\n",
        ),
        (
            "host/BUILD.bazel",
            "load(':constraints.bzl', 'HOST_CONSTRAINTS')\nplatform(name = 'host', constraint_values = HOST_CONSTRAINTS)\n",
        ),
        (
            "cpu/BUILD",
            include_str!("builtin_fixture/platforms/cpu/BUILD"),
        ),
        (
            "os/BUILD",
            include_str!("builtin_fixture/platforms/os/BUILD"),
        ),
    ] {
        write_if_missing(&directory.join(path), text);
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::SystemTime;

    use super::*;

    fn snapshot(root: &Path) -> Vec<(PathBuf, bool, Vec<u8>, SystemTime)> {
        fn walk(root: &Path, dir: &Path, out: &mut Vec<(PathBuf, bool, Vec<u8>, SystemTime)>) {
            let mut entries = fs::read_dir(dir)
                .unwrap()
                .map(|e| e.unwrap().path())
                .collect::<Vec<_>>();
            entries.sort();
            for path in entries {
                let meta = fs::metadata(&path).unwrap();
                out.push((
                    path.strip_prefix(root).unwrap().to_owned(),
                    meta.is_dir(),
                    if meta.is_file() {
                        fs::read(&path).unwrap()
                    } else {
                        Vec::new()
                    },
                    meta.modified().unwrap(),
                ));
                if meta.is_dir() {
                    walk(root, &path, out);
                }
            }
        }
        let mut out = Vec::new();
        walk(root, root, &mut out);
        out
    }

    #[test]
    fn automatic_preparation_leaves_missing_invalid_and_included_module_inputs_untouched() {
        for source in [
            None,
            Some("module("),
            Some("module(name='root')\ninclude('//:fragment.MODULE.bazel')\n"),
        ] {
            let root = super::super::scratch();
            fs::write(root.join("fragment.MODULE.bazel"), "# authored fragment\n").unwrap();
            if let Some(source) = source {
                fs::write(root.join("MODULE.bazel"), source).unwrap();
            }
            let before = snapshot(&root);
            assert!(prepare_if_ordinary(&root).is_empty());
            assert!(
                platform_constraint_observations(&root)
                    .iter()
                    .all(Option::is_none)
            );
            assert_eq!(
                snapshot(&root),
                before,
                "eligibility must not repair the input or create dependencies"
            );
            assert_eq!(
                fs::read_to_string(root.join("MODULE.bazel"))
                    .ok()
                    .as_deref(),
                source
            );
        }
    }

    #[test]
    fn ordinary_preparation_is_idempotent_and_preserves_authored_overrides_and_files() {
        let root = super::super::scratch();
        let authored = "module(name='custom_root')\nbazel_dep(name='rules_cc',version='99')\nlocal_path_override(module_name='rules_cc',path='authored_cc')\nbazel_dep(name='extra',version='7')\nlocal_path_override(module_name='extra',path='authored_extra')\n";
        fs::write(root.join("MODULE.bazel"), authored).unwrap();
        fs::create_dir_all(root.join("authored_cc")).unwrap();
        let child = "module(name='rules_cc',version='99')\n# authored child\n";
        fs::write(root.join("authored_cc/MODULE.bazel"), child).unwrap();
        fs::create_dir_all(root.join("platforms/host")).unwrap();
        fs::write(
            root.join("platforms/host/BUILD.bazel"),
            "# authored platform\n",
        )
        .unwrap();
        fs::create_dir_all(root.join("rules_shell/shell/private")).unwrap();
        fs::write(
            root.join("rules_shell/shell/private/sh_binary.bzl"),
            "# authored shell source\n",
        )
        .unwrap();
        let first = prepare_if_ordinary(&root);
        assert!(
            first
                .iter()
                .any(|(name, path)| name == "rules_cc+" && path == "authored_cc")
        );
        assert!(
            !first
                .iter()
                .any(|(name, _)| name.starts_with("bazel_tools"))
        );
        assert!(
            fs::read_to_string(root.join("MODULE.bazel"))
                .unwrap()
                .starts_with(authored)
        );
        assert_eq!(
            fs::read_to_string(root.join("authored_cc/MODULE.bazel")).unwrap(),
            child
        );
        assert_eq!(
            fs::read_to_string(root.join("platforms/host/BUILD.bazel")).unwrap(),
            "# authored platform\n"
        );
        assert_eq!(
            fs::read_to_string(root.join("rules_shell/shell/private/sh_binary.bzl")).unwrap(),
            "# authored shell source\n"
        );
        let shell_module =
            ModuleFile::parse(&fs::read_to_string(root.join("rules_shell/MODULE.bazel")).unwrap())
                .unwrap();
        assert!(shell_module.directives.iter().any(|directive| {
            matches!(directive, Directive::BazelDep(dep) if dep.name == "platforms" && dep.version == "0.0.10")
        }));
        for (relative, expected) in [
            (
                "shell/BUILD",
                include_str!("builtin_fixture/rules_shell/shell/BUILD"),
            ),
            (
                "shell/private/BUILD",
                include_str!("builtin_fixture/rules_shell/shell/private/BUILD"),
            ),
            (
                "shell/sh_binary.bzl",
                include_str!("builtin_fixture/rules_shell/shell/sh_binary.bzl"),
            ),
            (
                "shell/private/sh_executable.bzl",
                include_str!("builtin_fixture/rules_shell/shell/private/sh_executable.bzl"),
            ),
        ] {
            assert_eq!(
                fs::read_to_string(root.join("rules_shell").join(relative)).unwrap(),
                expected
            );
        }
        assert!(
            !root.join("authored_extra").exists(),
            "unrelated override remains caller-owned"
        );
        for (relative, source) in [
            (
                "cpu/BUILD",
                include_str!("builtin_fixture/platforms/cpu/BUILD"),
            ),
            (
                "os/BUILD",
                include_str!("builtin_fixture/platforms/os/BUILD"),
            ),
        ] {
            assert_eq!(
                fs::read_to_string(root.join("platforms").join(relative)).unwrap(),
                source
            );
        }
        // An authored higher-precedence BUILD.bazel stays authoritative; the
        // helper never removes or rewrites it to activate the pinned default.
        fs::write(root.join("platforms/cpu/BUILD.bazel"), "# authored cpu\n").unwrap();
        let before = snapshot(&root);
        assert_eq!(prepare_if_ordinary(&root), first);
        assert_eq!(
            snapshot(&root),
            before,
            "repeat preparation performs no source rewrite"
        );
    }
}
