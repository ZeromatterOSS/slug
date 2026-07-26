/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::fmt;

use allocative::Allocative;

use crate::repo::CanonicalRepoName;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Allocative)]
pub struct PackagePath(String);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Allocative)]
pub struct TargetName(String);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Allocative)]
pub struct PackageIdentifier {
    repo: CanonicalRepoName,
    package: PackagePath,
}

impl PackagePath {
    pub fn root() -> Self {
        Self(String::new())
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        if value.is_empty() {
            return Ok(Self::root());
        }
        if value.starts_with('/') || value.ends_with('/') || value.contains("//") {
            return Err(format!("invalid package path: {value}"));
        }
        for segment in value.split('/') {
            if matches!(segment, "." | "..") {
                return Err(format!(
                    "invalid package path segment {segment:?} in {value}"
                ));
            }
        }
        Ok(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn default_target_name(&self) -> Result<TargetName, String> {
        let Some(name) = self.0.rsplit('/').next().filter(|name| !name.is_empty()) else {
            return Err("root package labels require an explicit target name".to_owned());
        };
        TargetName::parse(name)
    }
}

impl TargetName {
    pub fn parse(value: &str) -> Result<Self, String> {
        if value.is_empty() {
            return Err("empty target name".to_owned());
        }
        if value.starts_with('/') {
            return Err("target names may not start with '/'".to_owned());
        }
        if value == ".." || value.starts_with("../") {
            return Err("target names may not contain up-level references '..'".to_owned());
        }
        if value == "." {
            return Ok(Self(value.to_owned()));
        }
        if value.starts_with("./") {
            return Err("target names may not contain '.' as a path segment".to_owned());
        }
        if value.ends_with('\r') {
            return Err(
                "target names may not end with carriage returns (perhaps the input source is CRLF-terminated)"
                    .to_owned(),
            );
        }

        for (index, character) in value.char_indices() {
            match character {
                '.' => {}
                '/' => {
                    let suffix = &value[index..];
                    if suffix.starts_with("/../") {
                        return Err(
                            "target names may not contain up-level references '..'".to_owned()
                        );
                    }
                    if suffix.starts_with("/./") {
                        return Err("target names may not contain '.' as a path segment".to_owned());
                    }
                    if suffix.starts_with("//") {
                        return Err("target names may not contain '//' path separators".to_owned());
                    }
                }
                '\0'..='\u{1f}' | '\u{7f}' => {
                    return Err(format!(
                        "target names may not contain non-printable characters: '\\x{:02X}'",
                        character as u32
                    ));
                }
                ':' | '\\' => {
                    return Err(format!("target names may not contain '{character}'"));
                }
                _ => {}
            }
        }

        if value.ends_with("/..") {
            return Err("target names may not contain up-level references '..'".to_owned());
        }
        if value.ends_with('/') {
            return Err("target names may not end with '/'".to_owned());
        }

        Ok(Self(value.strip_suffix("/.").unwrap_or(value).to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl PackageIdentifier {
    pub fn new(repo: CanonicalRepoName, package: PackagePath) -> Self {
        Self { repo, package }
    }

    /// Parses Bazel's package-identifier spelling without repository mapping.
    ///
    /// This matches `PackageIdentifier.parse`: unqualified and explicit-main
    /// spellings identify the main repository, while both one- and two-`@`
    /// nonmain spellings use the literal repository name.
    pub fn parse_bazel_package_identifier(value: &str) -> Result<Self, String> {
        if value.contains(':') {
            return Err(format!(
                "invalid package identifier {value:?}: contains ':'"
            ));
        }

        let (repo, package) = if let Some(rest) = value.strip_prefix("@@") {
            parse_repository_package(value, rest)?
        } else if let Some(rest) = value.strip_prefix('@') {
            parse_repository_package(value, rest)?
        } else if let Some(package) = value.strip_prefix("//") {
            (CanonicalRepoName::root(), package)
        } else {
            (CanonicalRepoName::root(), value)
        };
        let package = package
            .strip_suffix("/...")
            .or_else(|| (package == "...").then_some(""))
            .unwrap_or(package);
        Ok(Self::new(repo, parse_bazel_package_path(package)?))
    }

    pub fn repo(&self) -> &CanonicalRepoName {
        &self.repo
    }

    pub fn package(&self) -> &PackagePath {
        &self.package
    }
}

fn parse_repository_package<'a>(
    original: &str,
    rest: &'a str,
) -> Result<(CanonicalRepoName, &'a str), String> {
    let Some((repo, package)) = rest.split_once("//") else {
        return Err(format!(
            "invalid package identifier {original:?}: repository spelling must contain //"
        ));
    };
    let repo = if repo.is_empty() {
        CanonicalRepoName::root()
    } else {
        CanonicalRepoName::new_for_bazel_package_identifier(repo)?
    };
    Ok((repo, package))
}

fn parse_bazel_package_path(value: &str) -> Result<PackagePath, String> {
    if value.is_empty() {
        return Ok(PackagePath::root());
    }
    if value.starts_with('/') {
        return Err("package names may not start with '/'".to_owned());
    }
    if !value
        .bytes()
        .all(|byte| (b' '..=b'~').contains(&byte) && !matches!(byte, b':' | b'\\'))
    {
        return Err(
            "package names may contain only printable ASCII except ':' and '\\'".to_owned(),
        );
    }
    if value.ends_with('/') {
        return Err("package names may not end with '/'".to_owned());
    }
    if value.contains("//") {
        return Err("package names may not contain '//' path separators".to_owned());
    }
    if value
        .split('/')
        .any(|component| component.bytes().all(|byte| byte == b'.'))
    {
        return Err("package name component contains only '.' characters".to_owned());
    }
    Ok(PackagePath(value.to_owned()))
}

impl fmt::Display for PackagePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Display for TargetName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Display for PackageIdentifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}//{}", self.repo, self.package)
    }
}
