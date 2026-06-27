/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

use globset::Glob;
use globset::GlobSet;
use globset::GlobSetBuilder;

use crate::file_discovery::find_build_file;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobSpec {
    pub includes: Vec<String>,
    pub excludes: Vec<String>,
    pub allow_empty: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobExpansion {
    pub matches: Vec<String>,
    pub watched_dirs: Vec<String>,
    pub skipped_subpackages: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GlobError {
    InvalidPattern { pattern: String, message: String },
    Io { path: PathBuf, message: String },
    EmptyPattern { pattern: String },
}

impl fmt::Display for GlobError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPattern { pattern, message } => {
                write!(f, "invalid glob pattern {pattern:?}: {message}")
            }
            Self::Io { path, message } => write!(f, "failed to read {}: {message}", path.display()),
            Self::EmptyPattern { pattern } => write!(
                f,
                "glob pattern '{pattern}' didn't match anything, but allow_empty is set to False"
            ),
        }
    }
}

impl std::error::Error for GlobError {}

pub fn expand_glob(package_dir: &Path, spec: &GlobSpec) -> Result<GlobExpansion, GlobError> {
    let include_set = compile_patterns(&spec.includes)?;
    let exclude_set = compile_patterns(&spec.excludes)?;
    let mut include_matched = vec![false; spec.includes.len()];
    let mut state = ExpansionState::default();

    walk_package_dir(
        package_dir,
        Path::new(""),
        &include_set,
        &exclude_set,
        &mut include_matched,
        &mut state,
    )?;

    if !spec.allow_empty {
        for (index, matched) in include_matched.iter().enumerate() {
            if !matched {
                return Err(GlobError::EmptyPattern {
                    pattern: spec.includes[index].clone(),
                });
            }
        }
    }

    Ok(state.finish())
}

#[derive(Default)]
struct ExpansionState {
    matches: BTreeSet<String>,
    watched_dirs: BTreeSet<String>,
    skipped_subpackages: BTreeSet<String>,
}

impl ExpansionState {
    fn finish(self) -> GlobExpansion {
        GlobExpansion {
            matches: self.matches.into_iter().collect(),
            watched_dirs: self.watched_dirs.into_iter().collect(),
            skipped_subpackages: self.skipped_subpackages.into_iter().collect(),
        }
    }
}

fn compile_patterns(patterns: &[String]) -> Result<GlobSet, GlobError> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        let glob = Glob::new(pattern).map_err(|error| GlobError::InvalidPattern {
            pattern: pattern.clone(),
            message: error.to_string(),
        })?;
        builder.add(glob);
    }
    builder.build().map_err(|error| GlobError::InvalidPattern {
        pattern: patterns.join(","),
        message: error.to_string(),
    })
}

fn walk_package_dir(
    package_dir: &Path,
    relative_dir: &Path,
    include_set: &GlobSet,
    exclude_set: &GlobSet,
    include_matched: &mut [bool],
    state: &mut ExpansionState,
) -> Result<(), GlobError> {
    let dir = if relative_dir.as_os_str().is_empty() {
        package_dir.to_path_buf()
    } else {
        package_dir.join(relative_dir)
    };
    state
        .watched_dirs
        .insert(relative_path_to_slash(relative_dir));

    let mut entries = fs::read_dir(&dir)
        .map_err(|error| io_error(&dir, error))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| io_error(&dir, error))?;
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let file_type = entry
            .file_type()
            .map_err(|error| io_error(entry.path(), error))?;
        let child_relative = relative_dir.join(entry.file_name());
        if file_type.is_dir() {
            let child = entry.path();
            let child_relative_slash = relative_path_to_slash(&child_relative);
            state.watched_dirs.insert(child_relative_slash.clone());
            if find_build_file(&child).is_some() {
                state.skipped_subpackages.insert(child_relative_slash);
            } else {
                walk_package_dir(
                    package_dir,
                    &child_relative,
                    include_set,
                    exclude_set,
                    include_matched,
                    state,
                )?;
            }
        } else if file_type.is_file() {
            let child_relative_slash = relative_path_to_slash(&child_relative);
            let include_matches = include_set.matches(&child_relative_slash);
            if include_matches.is_empty() {
                continue;
            }
            for index in include_matches {
                include_matched[index] = true;
            }
            if !exclude_set.is_match(&child_relative_slash) {
                state.matches.insert(child_relative_slash);
            }
        }
    }

    Ok(())
}

fn relative_path_to_slash(path: &Path) -> String {
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_string_lossy().into_owned()),
            Component::CurDir => {}
            _ => parts.push(component.as_os_str().to_string_lossy().into_owned()),
        }
    }
    if parts.is_empty() {
        ".".to_owned()
    } else {
        parts.join("/")
    }
}

fn io_error(path: impl Into<PathBuf>, error: std::io::Error) -> GlobError {
    GlobError::Io {
        path: path.into(),
        message: error.to_string(),
    }
}
