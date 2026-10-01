// WP877 source-only assembly: exact members of existing tracked release blobs.
// Five-string JSON rows are also a Rust array expression, avoiding a test-only
// parser dependency. Archive rows are [archive, repo, path, sha256, size]; member
// rows are [repo, member, destination, sha256, size]. The inventory was reviewed
// independently against configured-cli-authentic/fixture.toml. No generated
// repository output or accepted materialization receipt is authored here.
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::io::Read;
use std::path::Component;

use base64::Engine as _;
use sha2::Digest;

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../slug_analysis_v2/tests/rustc_map_each/sources.rs"
));

const CALLBACK_SOURCE_ROWS: &[[&str; 5]] = &include!("rust_toolchain_callback_sources.json");
const CALLBACK_MODULES: &[(&str, &str)] = &[
    ("rules_rust", "0.73.0"),
    ("rules_cc", "0.2.17"),
    ("bazel_skylib", "1.8.2"),
    ("rules_license", "1.0.0"),
    ("buildozer", "8.5.1"),
    ("platforms", "1.0.0"),
    ("zlib", "1.3.1.bcr.5"),
    ("bazel_features", "1.42.1"),
    ("protobuf", "33.4"),
    ("rules_java", "9.1.0"),
    ("rules_python", "1.7.0"),
    ("rules_shell", "0.6.1"),
    ("apple_support", "1.24.2"),
    ("rules_apple", "4.1.0"),
    ("rules_swift", "3.1.2"),
    ("abseil-cpp", "20250814.1"),
];

fn callback_write(root: &Path, relative: &str, bytes: impl AsRef<[u8]>) {
    assert!(
        Path::new(relative)
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
    );
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

fn callback_verify(bytes: &[u8], row: &[&str; 5]) {
    assert_eq!(bytes.len(), row[4].parse::<usize>().unwrap(), "{}", row[1]);
    assert_eq!(
        hex::encode(sha2::Sha256::digest(bytes)),
        row[3],
        "{}",
        row[1]
    );
}

fn callback_archive_members(root: &Path) {
    let checkout = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = 0;
    let mut total_bytes = 0;
    for archive_row in CALLBACK_SOURCE_ROWS
        .iter()
        .filter(|row| row[0] == "archive")
    {
        let bytes = fs::read(checkout.join(archive_row[2])).unwrap();
        callback_verify(&bytes, archive_row);
        let mut required = CALLBACK_SOURCE_ROWS
            .iter()
            .filter(|row| row[0] == archive_row[1])
            .map(|row| (row[1], row))
            .collect::<BTreeMap<_, _>>();
        let expected = required.len();
        let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(bytes.as_slice()));
        for entry in archive.entries().unwrap() {
            let mut entry = entry.unwrap();
            let path = entry.path().unwrap().into_owned();
            let Some(row) = path.to_str().and_then(|path| required.remove(path)) else {
                continue;
            };
            assert!(entry.header().entry_type().is_file(), "{}", row[1]);
            assert_eq!(entry.size(), row[4].parse::<u64>().unwrap(), "{}", row[1]);
            let mut member = Vec::new();
            entry.read_to_end(&mut member).unwrap();
            callback_verify(&member, row);
            let parent = Path::new(row[2]).parent().unwrap();
            callback_write(root, &parent.join("BUILD.bazel").to_string_lossy(), "");
            callback_write(root, row[2], &member);
            files += 1;
            total_bytes += member.len();
        }
        assert!(
            required.is_empty(),
            "missing {expected} expected members: {required:?}"
        );
    }
    assert_eq!((files, total_bytes), (47, 418_603));
}

// MODULE/BUILD inputs are authored fixture metadata, not upstream metadata
// parity. All body imports use normal selected mappings. The original builtin
// bazel_tools MODULE/registrations and generated repository writers stay intact.
fn callback_modules(root: &Path) {
    let mut root_module = String::from("module(name='root')\n");
    for &(name, version) in CALLBACK_MODULES {
        // Non-root extension definitions require normal selected registry
        // provenance. Only modules without those definitions remain mutable.
        if !matches!(name, "rules_cc" | "bazel_features" | "platforms") {
            root_module.push_str(&format!(
                "local_path_override(module_name='{name}',path='{name}')\n"
            ));
        }
        if matches!(
            name,
            "rules_rust"
                | "rules_cc"
                | "bazel_skylib"
                | "platforms"
                | "bazel_features"
                | "rules_apple"
                | "rules_swift"
                | "abseil-cpp"
        ) {
            root_module.push_str(&format!("bazel_dep(name='{name}',version='{version}')\n"));
        }
        let mut module = format!("module(name='{name}',version='{version}')\n");
        let dependencies: &[(&str, &str)] = match name {
            "rules_rust" => &[("rules_cc", "rules_cc"), ("bazel_skylib", "bazel_skylib")],
            "rules_cc" => &[
                ("bazel_features", "bazel_features"),
                ("bazel_skylib", "bazel_skylib"),
                ("platforms", "platforms"),
                ("protobuf", "com_google_protobuf"),
            ],
            "protobuf" => &[("bazel_features", "proto_bazel_features")],
            "bazel_features" => &[("bazel_skylib", "bazel_skylib")],
            // The original sh_executable creates an eager Windows Label.
            "rules_shell" => &[("platforms", "platforms")],
            _ => &[],
        };
        for &(dependency, apparent) in dependencies {
            let version = CALLBACK_MODULES
                .iter()
                .find(|(name, _)| *name == dependency)
                .unwrap()
                .1;
            module.push_str(&format!(
                "bazel_dep(name='{dependency}',version='{version}',repo_name='{apparent}')\n"
            ));
        }
        module.push_str(match name {
            "rules_cc" => "compat=use_extension('//cc:extensions.bzl','compatibility_proxy')\nuse_repo(compat,'cc_compatibility_proxy')\n",
            "bazel_features" => "version=use_extension('//private:extensions.bzl','version_extension')\nuse_repo(version,'bazel_features_globals','bazel_features_version')\n",
            "platforms" => "host=use_extension('//host:extension.bzl','host_platform')\nuse_repo(host,'host_platform')\n",
            _ => "",
        });
        for (file, contents) in [
            ("MODULE.bazel", module.as_str()),
            ("REPO.bazel", ""),
            (".bazelignore", ""),
            ("BUILD.bazel", ""),
        ] {
            callback_write(root, &format!("{name}/{file}"), contents);
        }
    }
    callback_write(root, "MODULE.bazel", root_module);
    // The three source-owning extension modules receive real source entries
    // after the complete authored fixture metadata has been assembled.
    callback_write(root, "registry/bazel_registry.json", "{}");
}

fn callback_registry_sources(root: &Path) -> tempfile::TempDir {
    // Archive capture is owned by the production materializer. Keep binary
    // fixture archives outside the workspace's UTF-8 source observation walk.
    let archives = tempfile::Builder::new()
        .prefix("wp877-callback-archives-")
        .tempdir()
        .unwrap();
    for repo in ["rules_cc", "bazel_features", "platforms"] {
        let prefix = format!("{repo}/");
        let mut paths = RUSTC_MAP_EACH_SOURCES
            .iter()
            .filter_map(|(path, _)| path.strip_prefix(prefix.as_str()).map(PathBuf::from))
            .chain(
                CALLBACK_SOURCE_ROWS
                    .iter()
                    .filter(|row| row[0] == repo)
                    .map(|row| PathBuf::from(row[2].strip_prefix(prefix.as_str()).unwrap())),
            )
            .collect::<BTreeSet<_>>();
        for path in paths.clone() {
            paths.insert(path.parent().unwrap().join("BUILD.bazel"));
        }
        paths.extend(
            ["MODULE.bazel", "REPO.bazel", ".bazelignore", "BUILD.bazel"].map(PathBuf::from),
        );
        if repo == "rules_cc" {
            paths.insert(PathBuf::from("cc/private/toolchain_proof.bzl"));
        } else if repo == "platforms" {
            paths.extend(
                ["cpu/BUILD.bazel", "os/BUILD.bazel", "host/BUILD.bazel"].map(PathBuf::from),
            );
        }
        // Closed explicit paths, stable ordering and fixed headers make this an
        // immutable authored fixture source, with unchanged validated bodies.
        let compressed = flate2::GzBuilder::new()
            .mtime(0)
            .write(Vec::new(), flate2::Compression::default());
        let mut archive = tar::Builder::new(compressed);
        for path in paths {
            let bytes = fs::read(root.join(repo).join(&path)).unwrap();
            let mut header = tar::Header::new_gnu();
            header.set_size(bytes.len() as u64);
            header.set_mode(0o644);
            header.set_uid(0);
            header.set_gid(0);
            header.set_mtime(0);
            header.set_cksum();
            archive
                .append_data(&mut header, path, bytes.as_slice())
                .unwrap();
        }
        let bytes = archive.into_inner().unwrap().finish().unwrap();
        let filename = format!("{repo}.tar.gz");
        callback_write(archives.path(), &filename, &bytes);
        let url = url::Url::from_file_path(archives.path().join(filename)).unwrap();
        let integrity = format!(
            "sha256-{}",
            base64::engine::general_purpose::STANDARD.encode(sha2::Sha256::digest(&bytes))
        );
        let version = CALLBACK_MODULES
            .iter()
            .find(|(name, _)| *name == repo)
            .unwrap()
            .1;
        let registry = format!("registry/modules/{repo}/{version}");
        callback_write(
            root,
            &format!("{registry}/MODULE.bazel"),
            fs::read(root.join(repo).join("MODULE.bazel")).unwrap(),
        );
        callback_write(
            root,
            &format!("{registry}/source.json"),
            format!(
                "{{\"url\":\"{url}\",\"integrity\":\"{integrity}\",\"archive_type\":\"tar.gz\",\"strip_prefix\":\"\",\"patches\":{{}},\"patch_strip\":0}}"
            ),
        );
    }
    archives
}

pub(super) fn assemble_callback_base_workspace() -> (PathBuf, tempfile::TempDir) {
    let root = tempfile::Builder::new()
        .prefix("wp877-callback-")
        .tempdir()
        .unwrap()
        .keep();
    for &(relative, source) in RUSTC_MAP_EACH_SOURCES {
        // The old fixture's symbols snapshot is replaced by genuine extension
        // evaluation. Never create a cc_compatibility_proxy module or directory.
        if relative.starts_with("cc_compatibility_proxy/") {
            continue;
        }
        let parent = Path::new(relative).parent().unwrap();
        callback_write(&root, &parent.join("BUILD.bazel").to_string_lossy(), "");
        callback_write(&root, relative, source);
    }
    callback_archive_members(&root);
    callback_modules(&root);
    callback_write(
        &root,
        "rules_cc/cc/private/toolchain_proof.bzl",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../slug_analysis_v2/tests/rustc_map_each/cc_toolchain.bzl"
        )),
    );
    callback_write(
        &root,
        "rules_rust/proof.bzl",
        "load('@rules_cc//cc/private:toolchain_proof.bzl','make_subject')\nload('//rust/private:rustc.bzl','get_linker_and_args')\nsubject=make_subject(get_linker_and_args)\n",
    );
    callback_write(
        &root,
        "rules_rust/BUILD.bazel",
        "load(':proof.bzl','subject')\nsubject(name='producer',static_file='@@//:static.a',unused='A')\n",
    );
    for name in ["header.h", "static.a", "dynamic.so", "other.a"] {
        callback_write(&root, name, "authored inert input\n");
    }
    callback_write(
        &root,
        "BUILD.bazel",
        "exports_files(['header.h','static.a','dynamic.so','other.a'])\n",
    );
    // These native constraint declarations are authored. host/constraints.bzl
    // and extension.bzl remain exact archive members; host_platform output is
    // generated by its original repository rule under the production owner.
    for (path, contents) in [
        (
            "platforms/cpu/BUILD.bazel",
            "package(default_visibility=['//visibility:public'])\nconstraint_setting(name='cpu')\nconstraint_value(name='x86_64',constraint_setting=':cpu')\n",
        ),
        (
            "platforms/os/BUILD.bazel",
            "package(default_visibility=['//visibility:public'])\nconstraint_setting(name='os')\nconstraint_value(name='linux',constraint_setting=':os')\nconstraint_value(name='windows',constraint_setting=':os')\n",
        ),
        (
            "platforms/host/BUILD.bazel",
            "load(':constraints.bzl','HOST_CONSTRAINTS')\npackage(default_visibility=['//visibility:public'])\nexports_files(['constraints.bzl','extension.bzl'])\nplatform(name='host',constraint_values=HOST_CONSTRAINTS)\n",
        ),
    ] {
        callback_write(&root, path, contents);
    }
    assert!(!root.join("cc_compatibility_proxy").exists());
    let archives = callback_registry_sources(&root);
    (root, archives)
}
