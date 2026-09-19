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
use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use prost::Message;
use slug_build_api_v2::ActionInput;
use slug_build_api_v2::ActionSpec;
use slug_build_api_v2::ExpandedSpawnCommandLine;
use slug_build_api_v2::ParamFile;
use slug_build_api_v2::ParamFileFormat;
pub use slug_reapi_cache_v2::ReapiBlob;

use crate::command::digest_to_proto;
use crate::digest::ReapiDigest;
use crate::proto;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum InputTreeEntryKind {
    FileWriteContent,
    /// Ordinary source inputs follow Bazel's executable FileNode policy.
    Source,
    Input,
    Tool,
    ParamFile,
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ReapiInputTreeEntry {
    path: String,
    digest: ReapiDigest,
    kind: InputTreeEntryKind,
    is_executable: bool,
}

impl ReapiInputTreeEntry {
    pub fn new(path: impl Into<String>, digest: ReapiDigest, kind: InputTreeEntryKind) -> Self {
        Self {
            path: path.into(),
            digest,
            kind,
            is_executable: kind == InputTreeEntryKind::Source,
        }
    }

    pub fn with_executable(mut self, is_executable: bool) -> Self {
        self.is_executable = is_executable;
        self
    }

    pub fn is_executable(&self) -> bool {
        self.is_executable
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
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ReapiInputTree {
    entries: Vec<ReapiInputTreeEntry>,
    directories: Vec<String>,
    root_digest: ReapiDigest,
    directory_blobs: Vec<ReapiBlob>,
    inline_blobs: Vec<ReapiBlob>,
}

impl ReapiInputTree {
    pub fn from_action(action: &ActionSpec) -> Result<Self, InputTreeError> {
        if action.is_typed_payload() {
            return Err(InputTreeError::TypedActionUnsupported);
        }
        let mut entries = BTreeMap::new();
        for input in action.inputs() {
            insert_action_input(&mut entries, input, InputTreeEntryKind::Input)?;
        }
        for tool in action.tools() {
            insert_action_input(&mut entries, tool, InputTreeEntryKind::Tool)?;
        }
        let mut inline_blobs = Vec::new();
        for param_file in action.param_files() {
            let content = render_param_file(param_file);
            let blob = ReapiBlob::from_bytes(content.into_bytes());
            let entry = ReapiInputTreeEntry::new(
                param_file.path().to_owned(),
                blob.digest().clone(),
                InputTreeEntryKind::ParamFile,
            );
            insert_entry(&mut entries, entry)?;
            inline_blobs.push(blob);
        }

        let entries = entries.into_values().collect::<Vec<_>>();
        let (root_digest, directory_blobs) = merkle_directories(&entries, &[])?;
        Ok(Self {
            entries,
            directories: Vec::new(),
            root_digest,
            directory_blobs,
            inline_blobs,
        })
    }

    /// Atomically compose expansion-owned virtual files with resolved input entries.
    /// This component does not authorize typed Spawn execution.
    pub fn with_spawn_param_files(
        &self,
        command: &ExpandedSpawnCommandLine,
    ) -> Result<Self, InputTreeError> {
        self.with_spawn_param_files_executable(command, false)
    }

    /// Compose param files with an explicit input executable policy, retaining
    /// directory roots even when they contain no files.
    pub fn with_spawn_param_files_executable(
        &self,
        command: &ExpandedSpawnCommandLine,
        is_executable: bool,
    ) -> Result<Self, InputTreeError> {
        let mut entries: BTreeMap<_, _> = self
            .entries
            .iter()
            .map(|entry| (entry.path().to_owned(), entry.clone()))
            .collect();
        let mut inline_blobs = self.inline_blobs.clone();
        for file in command.param_files() {
            if entries.contains_key(file.path()) {
                return Err(InputTreeError::ConflictingPath {
                    path: file.path().to_owned(),
                });
            }
            let blob = ReapiBlob::from_bytes(file.bytes().to_vec());
            insert_entry(
                &mut entries,
                ReapiInputTreeEntry::new(
                    file.path(),
                    blob.digest().clone(),
                    InputTreeEntryKind::ParamFile,
                )
                .with_executable(is_executable),
            )?;
            inline_blobs.push(blob);
        }
        let entries = entries.into_values().collect::<Vec<_>>();
        let (root_digest, directory_blobs) = merkle_directories(&entries, &self.directories)?;
        Ok(Self {
            entries,
            directories: self.directories.clone(),
            root_digest,
            directory_blobs,
            inline_blobs,
        })
    }

    pub(crate) fn from_inline_file(
        path: &str,
        data: &[u8],
        kind: InputTreeEntryKind,
    ) -> Result<Self, InputTreeError> {
        let blob = ReapiBlob::from_bytes(data.to_vec());
        let entry = ReapiInputTreeEntry::new(path, blob.digest().clone(), kind);
        let mut entries = BTreeMap::new();
        insert_entry(&mut entries, entry)?;
        let entries = entries.into_values().collect::<Vec<_>>();
        let (root_digest, directory_blobs) = merkle_directories(&entries, &[])?;
        Ok(Self {
            entries,
            directories: Vec::new(),
            root_digest,
            directory_blobs,
            inline_blobs: vec![blob],
        })
    }

    pub(crate) fn from_source_entries(
        sources: impl IntoIterator<Item = ReapiInputTreeEntry>,
    ) -> Result<Self, InputTreeError> {
        Self::from_entries_and_directories(sources, std::iter::empty())
    }

    /// Build canonical input Directory messages from files and explicit
    /// directory paths. Explicit directories may contain ordinary descendants.
    /// Logical artifact namespace ownership is checked by the request owner.
    pub fn from_entries_and_directories(
        inputs: impl IntoIterator<Item = ReapiInputTreeEntry>,
        directories: impl IntoIterator<Item = String>,
    ) -> Result<Self, InputTreeError> {
        let mut entries = BTreeMap::new();
        for entry in inputs {
            insert_entry(&mut entries, entry)?;
        }
        let directories = directories.into_iter().collect::<BTreeSet<_>>();
        for directory in &directories {
            validate_path(directory)?;
        }
        let directories = directories.into_iter().collect::<Vec<_>>();
        let entries = entries.into_values().collect::<Vec<_>>();
        let (root_digest, directory_blobs) = merkle_directories(&entries, &directories)?;
        Ok(Self {
            entries,
            directories,
            root_digest,
            directory_blobs,
            inline_blobs: Vec::new(),
        })
    }

    pub fn entries(&self) -> &[ReapiInputTreeEntry] {
        &self.entries
    }

    /// Explicit directory paths supplied by the projection, sorted and unique.
    pub fn directories(&self) -> &[String] {
        &self.directories
    }

    pub fn root_digest(&self) -> &ReapiDigest {
        &self.root_digest
    }

    /// Serialized Directory messages, from leaves to the root, that must be in
    /// CAS before an Action can refer to `root_digest`.
    pub fn directory_blobs(&self) -> &[ReapiBlob] {
        &self.directory_blobs
    }

    /// Inline action inputs (currently param files) whose bytes are owned by
    /// the action projection.
    pub fn inline_blobs(&self) -> &[ReapiBlob] {
        &self.inline_blobs
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum InputTreeError {
    TypedActionUnsupported,
    MissingDigest { path: String },
    InvalidDigest { path: String, error: String },
    ConflictingPath { path: String },
    InvalidPath { path: String },
}

impl fmt::Display for InputTreeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TypedActionUnsupported => {
                f.write_str("typed Spawn/Symlink/ArgsWrite REAPI input trees are not admitted")
            }
            Self::MissingDigest { path } => write!(f, "REAPI input {path} is missing a digest"),
            Self::InvalidDigest { path, error } => {
                write!(f, "REAPI input {path} has an invalid digest: {error}")
            }
            Self::ConflictingPath { path } => write!(f, "conflicting REAPI input path: {path}"),
            Self::InvalidPath { path } => write!(
                f,
                "REAPI input path must contain non-empty normal segments: {path}"
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
    validate_path(entry.path())?;
    match entries.get(entry.path()) {
        Some(existing)
            if existing.digest() != entry.digest()
                || existing.is_executable() != entry.is_executable() =>
        {
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

#[derive(Default)]
struct DirectoryBuilder {
    files: BTreeMap<String, (ReapiDigest, bool)>,
    directories: BTreeMap<String, DirectoryBuilder>,
}

fn merkle_directories(
    entries: &[ReapiInputTreeEntry],
    directories: &[String],
) -> Result<(ReapiDigest, Vec<ReapiBlob>), InputTreeError> {
    let mut root = DirectoryBuilder::default();
    for path in directories {
        let mut directory = &mut root;
        for segment in path.split('/') {
            directory = directory.directories.entry(segment.to_owned()).or_default();
        }
    }
    for entry in entries {
        let mut segments = entry.path().split('/').peekable();
        let mut directory = &mut root;
        while let Some(segment) = segments.next() {
            if segments.peek().is_none() {
                if directory.directories.contains_key(segment)
                    || directory
                        .files
                        .insert(
                            segment.to_owned(),
                            (entry.digest().clone(), entry.is_executable()),
                        )
                        .is_some()
                {
                    return Err(InputTreeError::ConflictingPath {
                        path: entry.path().to_owned(),
                    });
                }
            } else {
                if directory.files.contains_key(segment) {
                    return Err(InputTreeError::ConflictingPath {
                        path: entry.path().to_owned(),
                    });
                }
                directory = directory.directories.entry(segment.to_owned()).or_default();
            }
        }
    }

    let mut blobs = Vec::new();
    let root = serialize_directory(&root, &mut blobs);
    Ok((root, blobs))
}

fn serialize_directory(directory: &DirectoryBuilder, blobs: &mut Vec<ReapiBlob>) -> ReapiDigest {
    let directories = directory
        .directories
        .iter()
        .map(|(name, child)| proto::DirectoryNode {
            name: name.clone(),
            digest: Some(digest_to_proto(&serialize_directory(child, blobs))),
            ..Default::default()
        })
        .collect();
    let files = directory
        .files
        .iter()
        .map(|(name, (digest, executable))| proto::FileNode {
            name: name.clone(),
            digest: Some(digest_to_proto(digest)),
            is_executable: *executable,
            ..Default::default()
        })
        .collect();
    let blob = ReapiBlob::from_bytes(
        proto::Directory {
            files,
            directories,
            ..Default::default()
        }
        .encode_to_vec(),
    );
    let digest = blob.digest().clone();
    blobs.push(blob);
    digest
}

fn validate_path(path: &str) -> Result<(), InputTreeError> {
    if path.is_empty()
        || path.starts_with('/')
        || path.ends_with('/')
        || path
            .split('/')
            .any(|segment| segment.is_empty() || matches!(segment, "." | ".."))
    {
        return Err(InputTreeError::InvalidPath {
            path: path.to_owned(),
        });
    }
    Ok(())
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

#[cfg(test)]
mod tests;
