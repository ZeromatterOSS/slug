//! Verified, bounded output-tree manifests. File contents remain in CAS.

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use prost::Message;
use slug_reapi_cache_v2::CacheClient;
use slug_reapi_cache_v2::CacheError;

use crate::GeneratedDirectory;
use crate::GeneratedOutput;
use crate::ReapiDigest;
use crate::RemoteExecutionError;
use crate::executor::cache_error;
use crate::executor::digest_from_proto;
use crate::proto;

struct Budget {
    wire: u64,
    entries: usize,
    records: usize,
    paths: usize,
    depth: usize,
}
impl Default for Budget {
    fn default() -> Self {
        Self {
            wire: 16 * 1024 * 1024,
            entries: 100_000,
            records: 100_000,
            paths: 16 * 1024 * 1024,
            depth: 256,
        }
    }
}
fn invalid(message: impl Into<String>) -> RemoteExecutionError {
    RemoteExecutionError::Protocol(message.into())
}
impl Budget {
    fn record(&mut self) -> Result<(), RemoteExecutionError> {
        self.records = self
            .records
            .checked_sub(1)
            .ok_or_else(|| invalid("output Tree decode record resource limit"))?;
        Ok(())
    }
    fn tree(&mut self, bytes: u64) -> Result<(), RemoteExecutionError> {
        self.wire = self
            .wire
            .checked_sub(bytes)
            .ok_or_else(|| invalid("output Tree wire resource limit"))?;
        Ok(())
    }
    fn path(
        &mut self,
        base: usize,
        parent: usize,
        name: usize,
        depth: usize,
    ) -> Result<(), RemoteExecutionError> {
        if depth > self.depth {
            return Err(invalid("output Tree depth resource limit"));
        }
        let size = base
            .checked_add(parent)
            .and_then(|v| v.checked_add(name))
            .and_then(|v| v.checked_add(1))
            .ok_or_else(|| invalid("output Tree path resource limit"))?;
        self.entries = self
            .entries
            .checked_sub(1)
            .ok_or_else(|| invalid("output Tree entry resource limit"))?;
        self.paths = self
            .paths
            .checked_sub(size)
            .ok_or_else(|| invalid("output Tree path resource limit"))?;
        Ok(())
    }
}

pub(crate) async fn fetch_output_trees(
    cache: &CacheClient,
    outputs: &[proto::OutputDirectory],
) -> Result<Vec<GeneratedDirectory>, RemoteExecutionError> {
    let mut budget = Budget::default();
    let mut manifests = Vec::new();
    let mut verified = BTreeSet::new();
    for output in outputs {
        let digest = digest_from_proto(
            output
                .tree_digest
                .as_ref()
                .ok_or_else(|| invalid("output directory requires Tree digest"))?,
        )?;
        // Charge the whole action before fetching or allocating this Tree.
        budget.tree(digest.size_bytes())?;
        let mut bytes = Vec::new();
        cache
            .read_blob_verified(&digest, |chunk| {
                if bytes.len().saturating_add(chunk.len()) as u64 > digest.size_bytes() {
                    return Err(CacheError::Sink("output Tree wire resource limit".into()));
                }
                bytes.extend_from_slice(chunk);
                Ok(())
            })
            .await
            .map_err(cache_error)?;
        let manifest = decode_tree(output, &digest, &bytes, &mut budget)?;
        drop(bytes);
        for file in manifest.files() {
            if verified.insert(file.digest().clone()) {
                cache
                    .read_blob_verified(file.digest(), |_| Ok(()))
                    .await
                    .map_err(cache_error)?;
            }
        }
        manifests.push(manifest);
    }
    Ok(manifests)
}

fn component(name: &str) -> Result<(), RemoteExecutionError> {
    if name.is_empty() || matches!(name, "." | "..") || name.contains(['/', '\\', '\0']) {
        return Err(invalid(format!("malformed output Tree component {name:?}")));
    }
    Ok(())
}
fn validate_directory(directory: &proto::Directory) -> Result<(), RemoteExecutionError> {
    if !directory.symlinks.is_empty()
        || directory.node_properties.is_some()
        || directory
            .files
            .iter()
            .any(|file| file.node_properties.is_some())
    {
        return Err(invalid(
            "output Tree symlinks and node properties are unsupported",
        ));
    }
    if !directory
        .files
        .windows(2)
        .all(|pair| pair[0].name < pair[1].name)
        || !directory
            .directories
            .windows(2)
            .all(|pair| pair[0].name < pair[1].name)
    {
        return Err(invalid("output Tree nodes must be sorted and unique"));
    }
    let mut names = BTreeSet::new();
    for (name, digest) in directory
        .files
        .iter()
        .map(|node| (&node.name, &node.digest))
        .chain(
            directory
                .directories
                .iter()
                .map(|node| (&node.name, &node.digest)),
        )
    {
        component(name)?;
        if !names.insert(name) {
            return Err(invalid("duplicate output Tree child name"));
        }
        digest_from_proto(
            digest
                .as_ref()
                .ok_or_else(|| invalid("output Tree node has no digest"))?,
        )?;
    }
    Ok(())
}

// Bound decoded-message allocation before prost allocates repeated fields. A
// small wire payload can otherwise contain millions of empty submessages.
// Reject unknown fields: prost would discard them before Directory hashing.
// Levels describe the finite schema, not output directory depth:
// Tree=0, Directory=1, FileNode=2, DirectoryNode=3, Digest=4.
fn scan_tree(bytes: &[u8], budget: &mut Budget, level: u8) -> Result<(), RemoteExecutionError> {
    use prost::encoding::DecodeContext;
    use prost::encoding::WireType;
    use prost::encoding::decode_key;
    use prost::encoding::decode_varint;
    use prost::encoding::skip_field;
    let mut remaining = bytes;
    while !remaining.is_empty() {
        let (tag, wire) = decode_key(&mut remaining).map_err(|error| invalid(error.to_string()))?;
        if (level == 1 && matches!(tag, 3 | 5)) || (level == 2 && tag == 6) {
            return Err(invalid(
                "output Tree symlinks and node properties are unsupported",
            ));
        }
        let (expected, nested) = match (level, tag) {
            (0, 1 | 2) => (WireType::LengthDelimited, Some(1)),
            (1, 1) => (WireType::LengthDelimited, Some(2)),
            (1, 2) => (WireType::LengthDelimited, Some(3)),
            (2 | 3, 2) => (WireType::LengthDelimited, Some(4)),
            (2 | 3 | 4, 1) => (WireType::LengthDelimited, None),
            (2, 4) | (4, 2) => (WireType::Varint, None),
            _ => return Err(invalid("unsupported output Tree protobuf field")),
        };
        if wire != expected {
            return Err(invalid("invalid output Tree protobuf wire type"));
        }
        if let Some(nested) = nested {
            budget.record()?;
            let length = usize::try_from(
                decode_varint(&mut remaining).map_err(|error| invalid(error.to_string()))?,
            )
            .map_err(|_| invalid("invalid output Tree record length"))?;
            let message = remaining
                .get(..length)
                .ok_or_else(|| invalid("truncated output Tree record"))?;
            remaining = &remaining[length..];
            scan_tree(message, budget, nested)?;
        } else {
            skip_field(wire, tag, &mut remaining, DecodeContext::default())
                .map_err(|error| invalid(error.to_string()))?;
        }
    }
    Ok(())
}

fn decode_tree(
    output: &proto::OutputDirectory,
    digest: &ReapiDigest,
    bytes: &[u8],
    budget: &mut Budget,
) -> Result<GeneratedDirectory, RemoteExecutionError> {
    digest.verify_bytes(bytes).map_err(|actual| {
        invalid(format!(
            "output Tree digest mismatch: expected {digest}, got {actual}"
        ))
    })?;
    scan_tree(bytes, budget, 0)?;
    let tree = proto::Tree::decode(bytes)
        .map_err(|error| invalid(format!("invalid output Tree: {error}")))?;
    let root = tree
        .root
        .ok_or_else(|| invalid("output Tree has no root"))?;
    validate_directory(&root)?;
    let root_digest = ReapiDigest::of_bytes(&root.encode_to_vec());
    if let Some(expected) = &output.root_directory_digest {
        if digest_from_proto(expected)? != root_digest {
            return Err(invalid("output Tree root digest mismatch"));
        }
    }
    let mut children = BTreeMap::new();
    for directory in tree.children {
        validate_directory(&directory)?;
        let key = ReapiDigest::of_bytes(&directory.encode_to_vec());
        // NativeLink includes the root in children as well. Its identical
        // duplicate adds no topology, but has already consumed decode budget.
        if key == root_digest {
            if directory != root {
                return Err(invalid("output Tree Directory digest collision"));
            }
            continue;
        }
        match children.entry(key) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(directory);
            }
            std::collections::btree_map::Entry::Occupied(entry) => {
                if entry.get() != &directory {
                    return Err(invalid("output Tree Directory digest collision"));
                }
            }
        }
    }
    budget.path(output.path.len(), 0, 0, 0)?;
    let mut pending = vec![(String::new(), &root, 0)];
    let mut directories = Vec::new();
    let mut files = Vec::new();
    let mut referenced = BTreeSet::new();
    while let Some((path, directory, depth)) = pending.pop() {
        for file in &directory.files {
            budget.path(output.path.len(), path.len(), file.name.len(), depth + 1)?;
            let child = if path.is_empty() {
                file.name.clone()
            } else {
                format!("{path}/{}", file.name)
            };
            files.push(GeneratedOutput::new(
                child,
                digest_from_proto(file.digest.as_ref().unwrap())?,
                file.is_executable,
            ));
        }
        for node in &directory.directories {
            budget.path(output.path.len(), path.len(), node.name.len(), depth + 1)?;
            let key = digest_from_proto(node.digest.as_ref().unwrap())?;
            let child_directory = children
                .get(&key)
                .ok_or_else(|| invalid(format!("output Tree missing Directory {key}")))?;
            referenced.insert(key);
            let child = if path.is_empty() {
                node.name.clone()
            } else {
                format!("{path}/{}", node.name)
            };
            pending.push((child, child_directory, depth + 1));
        }
        directories.push(path);
    }
    if referenced.len() != children.len() {
        return Err(invalid("output Tree has unreferenced child directories"));
    }
    directories.sort();
    files.sort();
    Ok(GeneratedDirectory {
        path: output.path.clone(),
        tree_digest: digest.clone(),
        root_digest,
        directories,
        files,
    })
}

#[cfg(test)]
mod tests;
