/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use slug_build_api_v2::ActionInput;
use slug_build_api_v2::ActionSpec;
use slug_build_api_v2::ParamFile;
use slug_build_api_v2::ParamFileFormat;

use crate::digest::ReapiDigest;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum InputTreeEntryKind {
    Input,
    Tool,
    ParamFile,
}

impl InputTreeEntryKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Input => "input",
            Self::Tool => "tool",
            Self::ParamFile => "paramfile",
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ReapiInputTreeEntry {
    path: String,
    digest: ReapiDigest,
    kind: InputTreeEntryKind,
}

impl ReapiInputTreeEntry {
    pub fn new(path: impl Into<String>, digest: ReapiDigest, kind: InputTreeEntryKind) -> Self {
        Self {
            path: path.into(),
            digest,
            kind,
        }
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn digest(&self) -> &ReapiDigest {
        &self.digest
    }

    pub fn kind(&self) -> InputTreeEntryKind {
        self.kind
    }

    fn stable_serialize(&self) -> String {
        format!("{} {} {}", self.kind.as_str(), self.path, self.digest)
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ReapiInputTree {
    entries: Vec<ReapiInputTreeEntry>,
    root_digest: ReapiDigest,
}

impl ReapiInputTree {
    pub fn from_action(action: &ActionSpec) -> Result<Self, InputTreeError> {
        let mut entries = BTreeMap::new();
        for input in action.inputs() {
            insert_action_input(&mut entries, input, InputTreeEntryKind::Input)?;
        }
        for tool in action.tools() {
            insert_action_input(&mut entries, tool, InputTreeEntryKind::Tool)?;
        }
        for param_file in action.param_files() {
            let content = render_param_file(param_file);
            let entry = ReapiInputTreeEntry::new(
                param_file.path().to_owned(),
                ReapiDigest::of_bytes(content.as_bytes()),
                InputTreeEntryKind::ParamFile,
            );
            insert_entry(&mut entries, entry)?;
        }

        let entries = entries.into_values().collect::<Vec<_>>();
        let root_digest = root_digest(&entries);
        Ok(Self {
            entries,
            root_digest,
        })
    }

    pub fn entries(&self) -> &[ReapiInputTreeEntry] {
        &self.entries
    }

    pub fn root_digest(&self) -> &ReapiDigest {
        &self.root_digest
    }

    pub fn stable_serialize(&self) -> String {
        self.entries
            .iter()
            .map(ReapiInputTreeEntry::stable_serialize)
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum InputTreeError {
    MissingDigest { path: String },
    InvalidDigest { path: String, error: String },
    ConflictingPath { path: String },
}

impl fmt::Display for InputTreeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingDigest { path } => write!(f, "REAPI input {path} is missing a digest"),
            Self::InvalidDigest { path, error } => {
                write!(f, "REAPI input {path} has an invalid digest: {error}")
            }
            Self::ConflictingPath { path } => write!(
                f,
                "REAPI input path declared twice with different digests: {path}"
            ),
        }
    }
}

impl Error for InputTreeError {}

fn insert_action_input(
    entries: &mut BTreeMap<String, ReapiInputTreeEntry>,
    input: &ActionInput,
    kind: InputTreeEntryKind,
) -> Result<(), InputTreeError> {
    let digest = input
        .digest()
        .ok_or_else(|| InputTreeError::MissingDigest {
            path: input.path().to_owned(),
        })
        .and_then(|digest| {
            ReapiDigest::parse(digest).map_err(|error| InputTreeError::InvalidDigest {
                path: input.path().to_owned(),
                error,
            })
        })?;
    insert_entry(
        entries,
        ReapiInputTreeEntry::new(input.path(), digest, kind),
    )
}

fn insert_entry(
    entries: &mut BTreeMap<String, ReapiInputTreeEntry>,
    entry: ReapiInputTreeEntry,
) -> Result<(), InputTreeError> {
    match entries.get(entry.path()) {
        Some(existing) if existing.digest() != entry.digest() => {
            Err(InputTreeError::ConflictingPath {
                path: entry.path().to_owned(),
            })
        }
        Some(_) => Ok(()),
        None => {
            entries.insert(entry.path().to_owned(), entry);
            Ok(())
        }
    }
}

fn root_digest(entries: &[ReapiInputTreeEntry]) -> ReapiDigest {
    ReapiDigest::of_bytes(
        entries
            .iter()
            .map(ReapiInputTreeEntry::stable_serialize)
            .collect::<Vec<_>>()
            .join("\n")
            .as_bytes(),
    )
}

fn render_param_file(param_file: &ParamFile) -> String {
    match param_file.format() {
        ParamFileFormat::Multiline => format!("{}\n", param_file.args().join("\n")),
        ParamFileFormat::ShellQuoted => format!(
            "{}\n",
            param_file
                .args()
                .iter()
                .map(|arg| shell_quote(arg))
                .collect::<Vec<_>>()
                .join(" ")
        ),
    }
}

fn shell_quote(value: &str) -> String {
    if !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | '/' | ':'))
    {
        return value.to_owned();
    }
    format!("'{}'", value.replace('\'', "'\\''"))
}
