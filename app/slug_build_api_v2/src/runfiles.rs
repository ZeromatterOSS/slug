/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::error::Error;
use std::fmt;
use std::sync::LazyLock;

use crate::ActionOutputKind;
use crate::AnalysisArtifact;
use crate::AnalysisDepset;
use crate::AnalysisValue;
use crate::AnalysisValueError;
use crate::AnalysisValueKind;
use crate::AnalysisValueType;
use crate::Depset;
use crate::DepsetError;
use crate::DepsetOrder;
use crate::DepsetSuccessor;
use crate::RetainedRunfiles;
use crate::RunfilesConflictPolicy;
use crate::RunfilesSymlink;
use crate::RunfilesSymlinkDepset;
use crate::analysis_value::PublicationEqState;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum RunfilesError {
    InvalidFilesType(AnalysisValueType),
    InvalidArtifactKind(ActionOutputKind),
    IncompatiblePrefix { left: String, right: String },
    Analysis(AnalysisValueError),
    Depset(DepsetError),
}

impl fmt::Display for RunfilesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFilesType(kind) => {
                write!(f, "runfiles files must be Files, got {kind:?}")
            }
            Self::InvalidArtifactKind(kind) => {
                write!(f, "runfiles does not admit {kind:?} Artifacts")
            }
            Self::IncompatiblePrefix { left, right } => {
                write!(f, "runfiles repository prefixes differ: {left} != {right}")
            }
            Self::Analysis(error) => error.fmt(f),
            Self::Depset(error) => error.fmt(f),
        }
    }
}

impl Error for RunfilesError {}

impl From<DepsetError> for RunfilesError {
    fn from(value: DepsetError) -> Self {
        Self::Depset(value)
    }
}

impl RetainedRunfiles {
    pub fn empty() -> Self {
        static EMPTY: LazyLock<RetainedRunfiles> = LazyLock::new(|| RetainedRunfiles {
            files: AnalysisDepset::empty(DepsetOrder::Default),
            symlinks: Depset::empty(),
            root_symlinks: Depset::empty(),
            empty_filenames: Depset::empty(),
            conflict_policy: RunfilesConflictPolicy::Warn,
            repository_prefix: "_main".into(),
        });
        EMPTY.clone()
    }

    pub fn from_parts(
        direct_files: Vec<AnalysisArtifact>,
        transitive_files: Vec<AnalysisDepset>,
        direct_symlinks: Vec<RunfilesSymlink>,
        transitive_symlinks: Vec<RunfilesSymlinkDepset>,
        direct_root_symlinks: Vec<RunfilesSymlink>,
        transitive_root_symlinks: Vec<RunfilesSymlinkDepset>,
        conflict_policy: RunfilesConflictPolicy,
    ) -> Result<Self, RunfilesError> {
        for artifact in &direct_files {
            ensure_regular_artifact(artifact)?;
        }
        for files in &transitive_files {
            ensure_file_depset(files)?;
        }
        for symlink in direct_symlinks.iter().chain(&direct_root_symlinks) {
            ensure_regular_artifact(&symlink.artifact)?;
        }
        for symlinks in transitive_symlinks.iter().chain(&transitive_root_symlinks) {
            symlinks.visit(|symlink| ensure_regular_artifact(&symlink.artifact))?;
        }
        let files = AnalysisDepset::new(
            DepsetOrder::Default,
            direct_files
                .into_iter()
                .map(AnalysisValue::artifact)
                .collect(),
            transitive_files,
        )
        .map_err(RunfilesError::Analysis)?;
        Ok(Self {
            files,
            symlinks: Depset::new(DepsetOrder::Default, direct_symlinks, transitive_symlinks)?,
            root_symlinks: Depset::new(
                DepsetOrder::Default,
                direct_root_symlinks,
                transitive_root_symlinks,
            )?,
            empty_filenames: Depset::empty(),
            conflict_policy,
            repository_prefix: "_main".into(),
        })
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
            && self.symlinks.is_empty()
            && self.root_symlinks.is_empty()
            && self.empty_filenames.is_empty()
    }

    pub fn with_artifact(&self, artifact: AnalysisArtifact) -> Result<Self, RunfilesError> {
        ensure_regular_artifact(&artifact)?;
        Ok(Self {
            files: AnalysisDepset::new(
                DepsetOrder::Default,
                vec![AnalysisValue::artifact(artifact)],
                vec![self.files.clone()],
            )
            .map_err(RunfilesError::Analysis)?,
            symlinks: self.symlinks.clone(),
            root_symlinks: self.root_symlinks.clone(),
            empty_filenames: self.empty_filenames.clone(),
            conflict_policy: self.conflict_policy,
            repository_prefix: self.repository_prefix.clone(),
        })
    }

    pub fn merge(&self, other: &Self) -> Result<Self, RunfilesError> {
        Self::merge_all([self, other])
    }

    pub fn merge_all<'a>(
        values: impl IntoIterator<Item = &'a Self>,
    ) -> Result<Self, RunfilesError> {
        let values = values
            .into_iter()
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();
        let Some(first) = values.first() else {
            return Ok(Self::empty());
        };
        if values.len() == 1 {
            return Ok((*first).clone());
        }
        for value in &values[1..] {
            if value.repository_prefix != first.repository_prefix {
                return Err(RunfilesError::IncompatiblePrefix {
                    left: first.repository_prefix.to_string(),
                    right: value.repository_prefix.to_string(),
                });
            }
        }
        let conflict_policy = values
            .iter()
            .map(|value| value.conflict_policy)
            .find(|policy| *policy == RunfilesConflictPolicy::Error)
            .unwrap_or(RunfilesConflictPolicy::Warn);
        Ok(Self {
            files: AnalysisDepset::new(
                DepsetOrder::Default,
                Vec::new(),
                values.iter().map(|value| value.files.clone()).collect(),
            )
            .map_err(RunfilesError::Analysis)?,
            symlinks: Depset::new(
                DepsetOrder::Default,
                Vec::new(),
                values.iter().map(|value| value.symlinks.clone()).collect(),
            )?,
            root_symlinks: Depset::new(
                DepsetOrder::Default,
                Vec::new(),
                values
                    .iter()
                    .map(|value| value.root_symlinks.clone())
                    .collect(),
            )?,
            empty_filenames: Depset::new(
                DepsetOrder::Default,
                Vec::new(),
                values
                    .iter()
                    .map(|value| value.empty_filenames.clone())
                    .collect(),
            )?,
            conflict_policy,
            repository_prefix: first.repository_prefix.clone(),
        })
    }

    pub(crate) fn publication_eq_with(&self, other: &Self, state: &mut PublicationEqState) -> bool {
        self.files.publication_eq_with(&other.files, state)
            && symlink_depset_publication_eq(&self.symlinks, &other.symlinks, state)
            && symlink_depset_publication_eq(&self.root_symlinks, &other.root_symlinks, state)
            && self.empty_filenames == other.empty_filenames
            && self.conflict_policy == other.conflict_policy
            && self.repository_prefix == other.repository_prefix
    }
}

fn ensure_regular_artifact(artifact: &AnalysisArtifact) -> Result<(), RunfilesError> {
    if let AnalysisArtifact::Derived { output, .. } = artifact
        && output.kind() != ActionOutputKind::File
    {
        return Err(RunfilesError::InvalidArtifactKind(output.kind()));
    }
    Ok(())
}

fn ensure_file_depset(files: &AnalysisDepset) -> Result<(), RunfilesError> {
    if !matches!(
        files.element_type(),
        AnalysisValueType::Empty | AnalysisValueType::Artifact
    ) {
        return Err(RunfilesError::InvalidFilesType(files.element_type()));
    }
    for value in files.to_list() {
        let AnalysisValueKind::Artifact(artifact) = value.kind() else {
            continue;
        };
        ensure_regular_artifact(artifact)?;
    }
    Ok(())
}

fn symlink_depset_publication_eq(
    left: &RunfilesSymlinkDepset,
    right: &RunfilesSymlinkDepset,
    state: &mut PublicationEqState,
) -> bool {
    let mut stack = vec![(left.clone(), right.clone())];
    while let Some((left, right)) = stack.pop() {
        let left_key = left.node_key();
        let right_key = right.node_key();
        match state.enter_runfiles_depset_pair(left_key, right_key) {
            Ok(false) => continue,
            Err(()) => return false,
            Ok(true) => {}
        }
        if left.order() != right.order() || left.depth() != right.depth() {
            return false;
        }
        let mut left = left.successors();
        let mut right = right.successors();
        loop {
            match (left.next(), right.next()) {
                (None, None) => break,
                (Some(DepsetSuccessor::Direct(left)), Some(DepsetSuccessor::Direct(right)))
                    if left.publication_eq(right) => {}
                (
                    Some(DepsetSuccessor::Transitive(left)),
                    Some(DepsetSuccessor::Transitive(right)),
                ) => stack.push((left, right)),
                _ => return false,
            }
        }
    }
    true
}
