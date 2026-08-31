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

use prost::Message;
use slug_build_api_v2::ActionInput;
use slug_build_api_v2::ActionSpec;
use slug_build_api_v2::ParamFile;
use slug_build_api_v2::ParamFileFormat;

use crate::command::digest_to_proto;
use crate::digest::ReapiDigest;
use crate::proto;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum InputTreeEntryKind {
    FileWriteContent,
    Input,
    Tool,
    ParamFile,
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
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ReapiInputTree {
    entries: Vec<ReapiInputTreeEntry>,
    root_digest: ReapiDigest,
    directory_blobs: Vec<ReapiBlob>,
    inline_blobs: Vec<ReapiBlob>,
}

/// A byte-bearing REAPI CAS object owned by the action projection.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ReapiBlob {
    digest: ReapiDigest,
    data: Vec<u8>,
}

impl ReapiBlob {
    pub fn from_bytes(data: Vec<u8>) -> Self {
        let digest = ReapiDigest::of_bytes(&data);
        Self { digest, data }
    }

    pub fn digest(&self) -> &ReapiDigest {
        &self.digest
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }
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
        let (root_digest, directory_blobs) = merkle_directories(&entries)?;
        Ok(Self {
            entries,
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
        let (root_digest, directory_blobs) = merkle_directories(&entries)?;
        Ok(Self {
            entries,
            root_digest,
            directory_blobs,
            inline_blobs: vec![blob],
        })
    }

    pub fn entries(&self) -> &[ReapiInputTreeEntry] {
        &self.entries
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
                write!(f, "typed Spawn/Symlink REAPI input trees are not admitted")
            }
            Self::MissingDigest { path } => write!(f, "REAPI input {path} is missing a digest"),
            Self::InvalidDigest { path, error } => {
                write!(f, "REAPI input {path} has an invalid digest: {error}")
            }
            Self::ConflictingPath { path } => write!(
                f,
                "REAPI input path declared twice with different digests: {path}"
            ),
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

#[derive(Default)]
struct DirectoryBuilder {
    files: BTreeMap<String, ReapiDigest>,
    directories: BTreeMap<String, DirectoryBuilder>,
}

fn merkle_directories(
    entries: &[ReapiInputTreeEntry],
) -> Result<(ReapiDigest, Vec<ReapiBlob>), InputTreeError> {
    let mut root = DirectoryBuilder::default();
    for entry in entries {
        let mut segments = entry.path().split('/').peekable();
        let mut directory = &mut root;
        while let Some(segment) = segments.next() {
            if segments.peek().is_none() {
                if directory.directories.contains_key(segment)
                    || directory
                        .files
                        .insert(segment.to_owned(), entry.digest().clone())
                        .is_some()
                {
                    return Err(InputTreeError::ConflictingPath {
                        path: entry.path().to_owned(),
                    });
                }
            } else {
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
        })
        .collect();
    let files = directory
        .files
        .iter()
        .map(|(name, digest)| proto::FileNode {
            name: name.clone(),
            digest: Some(digest_to_proto(digest)),
            is_executable: false,
        })
        .collect();
    let blob = ReapiBlob::from_bytes(proto::Directory { files, directories }.encode_to_vec());
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
