//! CAS preparation for a closure-owned source-only input set. No action is run.

use std::collections::BTreeMap;
use std::sync::Arc;

use slug_build_api_v2::AnalysisArtifact;
use slug_core_v2::runtime::PreparedSourceActionInputs;
use slug_reapi_cache_v2::CacheClient;
use slug_reapi_cache_v2::CacheError;

use crate::InputTreeEntryKind;
use crate::ReapiDigest;
use crate::ReapiInputTree;
use crate::ReapiInputTreeEntry;

#[derive(Debug, Clone)]
pub struct SourceInputReapiPlan {
    prepared: Arc<PreparedSourceActionInputs>,
    tree: ReapiInputTree,
    source_digests: Vec<ReapiDigest>,
    expanded_argv: Vec<String>,
}

impl SourceInputReapiPlan {
    /// Project only a Core-owned complete declared source set. This plan grants
    /// no Execute or publication authority; its frontier must be validated again
    /// by the future execution request after transfer.
    pub fn from_prepared(prepared: Arc<PreparedSourceActionInputs>) -> Result<Self, String> {
        let source_digests = prepared
            .sources()
            .map(|source| {
                let digest = source.digest();
                let mut hash = String::with_capacity(64);
                for byte in digest.sha256() {
                    use std::fmt::Write;
                    write!(hash, "{byte:02x}").expect("writing to String");
                }
                ReapiDigest::new(hash, digest.size_bytes())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let entries = prepared
            .sources()
            .zip(&source_digests)
            .map(|(source, digest)| {
                ReapiInputTreeEntry::new(
                    AnalysisArtifact::Source(source.label().clone()).path(),
                    digest.clone(),
                    InputTreeEntryKind::Source,
                )
            });
        let base =
            ReapiInputTree::from_source_entries(entries).map_err(|error| error.to_string())?;
        let command = prepared
            .spawn()
            .expand_forced_param_files()
            .map_err(|error| error.to_string())?;
        let tree = base
            .with_spawn_param_files(&command)
            .map_err(|error| error.to_string())?;
        Ok(Self {
            prepared,
            tree,
            source_digests,
            expanded_argv: command.argv().to_vec(),
        })
    }

    pub fn input_tree(&self) -> &ReapiInputTree {
        &self.tree
    }

    /// Move argv from the same expansion that supplied this tree's virtual bytes.
    pub(crate) fn take_expanded_argv(&mut self) -> Vec<String> {
        std::mem::take(&mut self.expanded_argv)
    }

    /// Stage missing content once per digest. CAS hits never open source paths.
    /// This performs no Action Cache, Execute or output-publication operation.
    pub async fn upload_missing(
        &self,
        cache: &CacheClient,
    ) -> Result<Vec<ReapiDigest>, CacheError> {
        let sources = self
            .source_digests
            .iter()
            .enumerate()
            .map(|(index, digest)| (digest.clone(), index))
            .collect::<BTreeMap<_, _>>();
        let blobs = self
            .tree
            .directory_blobs()
            .iter()
            .chain(self.tree.inline_blobs())
            .map(|blob| (blob.digest().clone(), blob))
            .collect::<BTreeMap<_, _>>();
        let requested = sources.keys().chain(blobs.keys()).cloned().collect();
        let missing = cache.find_missing(&requested).await?;
        for digest in &missing {
            if let Some(blob) = blobs.get(digest) {
                cache.upload_missing(&[(*blob).clone()]).await?;
            } else {
                let index = *sources
                    .get(digest)
                    .expect("validated missing response is requested");
                let file = self
                    .prepared
                    .open_source(index)
                    .map_err(|error| CacheError::Source(error.to_string()))?;
                cache
                    .upload_reader_verified(digest, tokio::fs::File::from_std(file))
                    .await?;
            }
        }
        Ok(missing.into_iter().collect())
    }
}

#[cfg(test)]
mod tests;
