/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License found in the LICENSE-APACHE file in the root directory of this
 * source tree. You may select the license that applies to you.
 */

use std::path::Component;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::sync::Mutex;

use async_trait::async_trait;
use dice::DiceDataBuilder;
use sha2::Digest;
use sha2::Sha256;
use slug_bzlmod_v2::OverrideAttributeValue;
use slug_bzlmod_v2::RepoSpec;
use slug_bzlmod_v2::RepositoryIo;
use slug_bzlmod_v2::RepositoryIoOutcome;
use slug_bzlmod_v2::RepositoryTransportError;
use slug_bzlmod_v2::install_repository_io;
use slug_bzlmod_v2::source_identity;

struct LocalRepositoryIo {
    immutable_roots: Mutex<Vec<tempfile::TempDir>>,
}

impl LocalRepositoryIo {
    fn new() -> Self {
        Self {
            immutable_roots: Mutex::new(Vec::new()),
        }
    }

    fn retain(&self, root: tempfile::TempDir) -> PathBuf {
        let path = root.path().to_path_buf();
        self.immutable_roots
            .lock()
            .expect("immutable repository root mutex poisoned")
            .push(root);
        path
    }
}

#[async_trait]
impl RepositoryIo for LocalRepositoryIo {
    async fn materialize(
        &self,
        workspace: &Path,
        repo_spec: &RepoSpec,
    ) -> Result<RepositoryIoOutcome, RepositoryTransportError> {
        let workspace = workspace.to_path_buf();
        let repo_spec = repo_spec.clone();
        let result = tokio::task::spawn_blocking(move || materialize(&workspace, &repo_spec))
            .await
            .map_err(|error| RepositoryTransportError {
                message: format!("joining repository materializer: {error}").into(),
            })??;
        match result {
            Materialized::Local { source_root } => Ok(RepositoryIoOutcome::Local { source_root }),
            Materialized::Immutable { bytes, root } => {
                let source_identity = source_identity(&bytes);
                let generation_root = self.retain(root);
                Ok(RepositoryIoOutcome::Immutable {
                    source_identity,
                    generation_root,
                })
            }
        }
    }
}

enum Materialized {
    Local {
        source_root: PathBuf,
    },
    Immutable {
        bytes: Vec<u8>,
        root: tempfile::TempDir,
    },
}

fn materialize(
    workspace: &Path,
    repo_spec: &RepoSpec,
) -> Result<Materialized, RepositoryTransportError> {
    let bzl_file = repo_spec.rule_id.bzl_file.to_string();
    match (bzl_file.as_str(), repo_spec.rule_id.rule_name.as_str()) {
        ("@@bazel_tools//tools/build_defs/repo:local.bzl", "local_repository") => {
            materialize_local(workspace, repo_spec)
        }
        ("@@bazel_tools//tools/build_defs/repo:http.bzl", "http_archive") => {
            materialize_archive(repo_spec)
        }
        ("@@bazel_tools//tools/build_defs/repo:git.bzl", "git_repository") => {
            materialize_git(repo_spec)
        }
        _ => Err(unsupported("unsupported repository override rule")),
    }
}

fn materialize_local(
    workspace: &Path,
    repo_spec: &RepoSpec,
) -> Result<Materialized, RepositoryTransportError> {
    reject_extra_attributes(repo_spec, &["path"])?;
    let path = Path::new(required_string(repo_spec, "path")?);
    if path.is_absolute() || !normalized_relative(path) {
        return Err(unsupported(
            "local_repository path must be normalized and workspace-relative",
        ));
    }
    let source_root =
        workspace
            .join(path)
            .canonicalize()
            .map_err(|error| RepositoryTransportError {
                message: format!(
                    "canonicalizing local_repository path {}: {error}",
                    path.display()
                )
                .into(),
            })?;
    if !source_root.starts_with(workspace) {
        return Err(unsupported("local_repository path escapes the workspace"));
    }
    Ok(Materialized::Local { source_root })
}

fn materialize_archive(repo_spec: &RepoSpec) -> Result<Materialized, RepositoryTransportError> {
    reject_extra_attributes(repo_spec, &["urls", "sha256", "type", "strip_prefix"])?;
    let urls = repo_spec
        .attributes
        .get("urls")
        .ok_or_else(|| unsupported("http_archive requires exactly one file URL"))?;
    let OverrideAttributeValue::Iterable(urls) = urls else {
        return Err(unsupported(
            "http_archive urls must contain exactly one file URL",
        ));
    };
    let [OverrideAttributeValue::String(url)] = urls.as_ref() else {
        return Err(unsupported(
            "http_archive urls must contain exactly one file URL",
        ));
    };
    let archive = local_file_uri(url)?;
    let expected_sha256 = required_string(repo_spec, "sha256")?;
    if expected_sha256.len() != 64 || !expected_sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(unsupported(
            "http_archive sha256 must be an exact 64-character hexadecimal digest",
        ));
    }
    if optional_string(repo_spec, "type")? != Some("tar") {
        return Err(unsupported("http_archive type must be exactly tar"));
    }
    let strip_prefix = optional_string(repo_spec, "strip_prefix")?;
    let root = tempfile::tempdir().map_err(|error| RepositoryTransportError {
        message: format!("creating archive materialization root: {error}").into(),
    })?;
    let bytes = std::fs::read(&archive).map_err(|error| RepositoryTransportError {
        message: format!("reading http_archive {}: {error}", archive.display()).into(),
    })?;
    let actual_sha256 = format!("{:x}", Sha256::digest(&bytes));
    if !actual_sha256.eq_ignore_ascii_case(expected_sha256) {
        return Err(unsupported(
            "http_archive sha256 does not match the local tar",
        ));
    }
    extract_tar(&archive, root.path(), strip_prefix.map(Path::new))?;
    Ok(Materialized::Immutable { bytes, root })
}

fn materialize_git(repo_spec: &RepoSpec) -> Result<Materialized, RepositoryTransportError> {
    reject_extra_attributes(repo_spec, &["remote", "commit"])?;
    let remote = local_file_uri(required_string(repo_spec, "remote")?)?;
    let commit = required_string(repo_spec, "commit")?;
    if commit.len() != 40 || !commit.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(unsupported(
            "git_repository commit must be an exact 40-character hexadecimal commit",
        ));
    }
    if !remote.join("HEAD").is_file() {
        return Err(unsupported(
            "git_repository remote must be a local bare repository",
        ));
    }
    let output = Command::new("git")
        .arg(format!("--git-dir={}", remote.display()))
        .args(["archive", "--format=tar", commit])
        .output()
        .map_err(|error| RepositoryTransportError {
            message: format!("running git archive for {}: {error}", remote.display()).into(),
        })?;
    if !output.status.success() {
        return Err(RepositoryTransportError {
            message: format!(
                "git archive for {} at {} failed: {}",
                remote.display(),
                commit,
                String::from_utf8_lossy(&output.stderr).trim()
            )
            .into(),
        });
    }
    let root = tempfile::tempdir().map_err(|error| RepositoryTransportError {
        message: format!("creating git materialization root: {error}").into(),
    })?;
    let archive = tempfile::NamedTempFile::new().map_err(|error| RepositoryTransportError {
        message: format!("creating temporary git archive: {error}").into(),
    })?;
    std::fs::write(archive.path(), &output.stdout).map_err(|error| RepositoryTransportError {
        message: format!("writing temporary git archive: {error}").into(),
    })?;
    extract_tar(archive.path(), root.path(), None)?;
    Ok(Materialized::Immutable {
        bytes: output.stdout,
        root,
    })
}

fn extract_tar(
    archive: &Path,
    root: &Path,
    strip_prefix: Option<&Path>,
) -> Result<(), RepositoryTransportError> {
    let strip_components = match strip_prefix {
        Some(path) if !path.as_os_str().is_empty() && normalized_relative(path) => {
            path.components().count().to_string()
        }
        Some(_) => {
            return Err(unsupported(
                "http_archive strip_prefix must be normalized and relative",
            ));
        }
        None => "0".to_owned(),
    };
    let listing = Command::new("tar")
        .args(["-tf"])
        .arg(archive)
        .output()
        .map_err(|error| RepositoryTransportError {
            message: format!("listing archive {}: {error}", archive.display()).into(),
        })?;
    if !listing.status.success()
        || String::from_utf8_lossy(&listing.stdout)
            .lines()
            .any(|line| !normalized_relative(Path::new(line.trim_end_matches('/'))))
    {
        return Err(unsupported("http_archive contains a non-normalized path"));
    }
    let detailed_listing = Command::new("tar")
        .args(["-tvf"])
        .arg(archive)
        .output()
        .map_err(|error| RepositoryTransportError {
            message: format!("inspecting archive {}: {error}", archive.display()).into(),
        })?;
    if !detailed_listing.status.success()
        || String::from_utf8_lossy(&detailed_listing.stdout)
            .lines()
            .any(|line| !matches!(line.as_bytes().first(), Some(b'-' | b'd')))
    {
        return Err(unsupported(
            "http_archive contains an unsupported tar entry type",
        ));
    }
    let output = Command::new("tar")
        .args(["-xf"])
        .arg(archive)
        .args(["-C"])
        .arg(root)
        .arg(format!("--strip-components={strip_components}"))
        .output()
        .map_err(|error| RepositoryTransportError {
            message: format!("extracting archive {}: {error}", archive.display()).into(),
        })?;
    if !output.status.success() {
        return Err(RepositoryTransportError {
            message: format!(
                "extracting archive {} failed: {}",
                archive.display(),
                String::from_utf8_lossy(&output.stderr).trim()
            )
            .into(),
        });
    }
    Ok(())
}

fn local_file_uri(value: &str) -> Result<PathBuf, RepositoryTransportError> {
    let url = url::Url::parse(value)
        .map_err(|_| unsupported("repository source must use an absolute file:// URI"))?;
    if url.scheme() != "file"
        || url.host_str().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(unsupported(
            "repository source must use an absolute file:// URI",
        ));
    }
    url.to_file_path()
        .map_err(|_| unsupported("repository source must use an absolute file:// URI"))
}

fn required_string<'a>(
    repo_spec: &'a RepoSpec,
    name: &str,
) -> Result<&'a str, RepositoryTransportError> {
    match repo_spec.attributes.get(name) {
        Some(OverrideAttributeValue::String(value)) => Ok(value),
        _ => Err(unsupported(&format!(
            "repository override requires string attribute {name}"
        ))),
    }
}

fn optional_string<'a>(
    repo_spec: &'a RepoSpec,
    name: &str,
) -> Result<Option<&'a str>, RepositoryTransportError> {
    match repo_spec.attributes.get(name) {
        None => Ok(None),
        Some(OverrideAttributeValue::String(value)) => Ok(Some(value)),
        Some(_) => Err(unsupported(&format!(
            "repository override attribute {name} must be a string"
        ))),
    }
}

fn reject_extra_attributes(
    repo_spec: &RepoSpec,
    allowed: &[&str],
) -> Result<(), RepositoryTransportError> {
    if repo_spec
        .attributes
        .keys()
        .any(|name| !allowed.contains(&name.as_str()))
    {
        return Err(unsupported(
            "repository override has unsupported attributes",
        ));
    }
    Ok(())
}

fn normalized_relative(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn unsupported(message: &str) -> RepositoryTransportError {
    RepositoryTransportError {
        message: message.into(),
    }
}

pub(crate) fn install(builder: &mut DiceDataBuilder) {
    install_repository_io(builder, Arc::new(LocalRepositoryIo::new()));
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use compact_str::CompactString;
    use sha2::Digest;
    use slug_bzlmod_v2::RepoRuleId;
    use slug_identity_v2::CanonicalLabel;
    use starlark_map::small_map::SmallMap;

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

    fn git_spec(remote: String, commit: String) -> RepoSpec {
        let attributes: [(CompactString, OverrideAttributeValue); 2] = [
            (
                "remote".into(),
                OverrideAttributeValue::String(remote.into()),
            ),
            (
                "commit".into(),
                OverrideAttributeValue::String(commit.into()),
            ),
        ];
        RepoSpec {
            rule_id: RepoRuleId {
                bzl_file: CanonicalLabel::parse("@@bazel_tools//tools/build_defs/repo:git.bzl")
                    .unwrap(),
                rule_name: "git_repository".into(),
            },
            attributes: Arc::new(SmallMap::from_iter(attributes)),
        }
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
                .args(["-cf"])
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
            materialize_archive(&archive_spec(url, digest)).unwrap()
        else {
            panic!("archive source must materialize immutably");
        };
        assert_eq!(
            std::fs::read(root.path().join("space name/MODULE.bazel")).unwrap(),
            b"module(name = 'archive')"
        );
    }

    #[tokio::test]
    async fn immutable_materializations_retain_prior_equal_generations() {
        let source = tempfile::tempdir().unwrap();
        let content = source.path().join("content");
        std::fs::create_dir(&content).unwrap();
        std::fs::write(content.join("MODULE.bazel"), b"module(name = 'archive')").unwrap();
        let archive = source.path().join("source.tar");
        assert!(
            Command::new("tar")
                .args(["-cf"])
                .arg(&archive)
                .args(["-C"])
                .arg(source.path())
                .arg("content")
                .status()
                .unwrap()
                .success()
        );
        let bytes = std::fs::read(&archive).unwrap();
        let spec = archive_spec(
            url::Url::from_file_path(&archive).unwrap().to_string(),
            format!("{:x}", Sha256::digest(&bytes)),
        );
        let io = LocalRepositoryIo::new();
        let RepositoryIoOutcome::Immutable {
            source_identity: first_identity,
            generation_root: first_root,
        } = io.materialize(source.path(), &spec).await.unwrap()
        else {
            panic!("archive source must materialize immutably");
        };
        let RepositoryIoOutcome::Immutable {
            source_identity: second_identity,
            generation_root: second_root,
        } = io.materialize(source.path(), &spec).await.unwrap()
        else {
            panic!("archive source must materialize immutably");
        };

        assert_eq!(first_identity, second_identity);
        assert_ne!(first_root, second_root);
        assert_eq!(
            std::fs::read(first_root.join("content/MODULE.bazel")).unwrap(),
            b"module(name = 'archive')"
        );
        assert_eq!(
            std::fs::read(second_root.join("content/MODULE.bazel")).unwrap(),
            b"module(name = 'archive')"
        );
    }

    #[test]
    fn git_requires_a_local_bare_repository_at_an_exact_commit() {
        let directory = tempfile::tempdir().unwrap();
        let checkout = directory.path().join("checkout");
        assert!(
            Command::new("git")
                .args(["init"])
                .arg(&checkout)
                .status()
                .unwrap()
                .success()
        );
        std::fs::write(checkout.join("MODULE.bazel"), b"module(name = 'git')").unwrap();
        for args in [
            vec!["-C", checkout.to_str().unwrap(), "add", "MODULE.bazel"],
            vec![
                "-C",
                checkout.to_str().unwrap(),
                "-c",
                "user.name=Slug test",
                "-c",
                "user.email=slug@example.com",
                "commit",
                "-m",
                "source",
            ],
        ] {
            assert!(Command::new("git").args(args).status().unwrap().success());
        }
        let commit = String::from_utf8(
            Command::new("git")
                .args(["-C"])
                .arg(&checkout)
                .args(["rev-parse", "HEAD"])
                .output()
                .unwrap()
                .stdout,
        )
        .unwrap()
        .trim()
        .to_owned();
        let bare = directory.path().join("source.git");
        assert!(
            Command::new("git")
                .args(["clone", "--bare"])
                .arg(&checkout)
                .arg(&bare)
                .status()
                .unwrap()
                .success()
        );
        let remote = url::Url::from_file_path(&bare).unwrap().to_string();
        let Materialized::Immutable { root, .. } =
            materialize_git(&git_spec(remote, commit)).unwrap()
        else {
            panic!("git source must materialize immutably");
        };
        assert_eq!(
            std::fs::read(root.path().join("MODULE.bazel")).unwrap(),
            b"module(name = 'git')"
        );
    }
}
