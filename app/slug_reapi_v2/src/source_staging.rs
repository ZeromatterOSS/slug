//! CAS preparation for a closure-owned source-only input set. No action is run.

use std::collections::BTreeMap;

use slug_build_api_v2::AnalysisArtifact;
use slug_core_v2::runtime::NativeSourceActionInputs;
use slug_reapi_cache_v2::CacheClient;
use slug_reapi_cache_v2::CacheError;

use crate::InputTreeEntryKind;
use crate::ReapiDigest;
use crate::ReapiInputTree;
use crate::ReapiInputTreeEntry;

#[derive(Debug, Clone)]
pub struct SourceInputReapiPlan {
    prepared: NativeSourceActionInputs,
    tree: ReapiInputTree,
    source_digests: Vec<ReapiDigest>,
    expanded_argv: Vec<String>,
}

impl SourceInputReapiPlan {
    /// Project only a Core-owned complete declared source set. This plan grants
    /// no Execute or publication authority; its frontier must be validated again
    /// by the future execution request after transfer.
    pub fn from_native(
        prepared: NativeSourceActionInputs,
        args_context: &dyn slug_build_api_v2::ArgsDirectoryContext,
    ) -> Result<Self, String> {
        prepared.check_active().map_err(|e| e.to_string())?;
        // Resolve callbacks before constructing any input-tree or digest state.
        // This execution path has no tracked tree-child metadata capability.
        let command = prepared
            .spawn()
            .expand_forced_param_files_with_context(args_context)
            .map_err(|error| error.to_string())?;
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

    pub(crate) async fn run_while_active<F: std::future::Future>(
        &self,
        future: F,
    ) -> Result<F::Output, CacheError> {
        self.prepared
            .run_while_active(future)
            .await
            .map_err(|e| CacheError::Source(e.to_string()))
    }
    pub(crate) fn check_active(&self) -> Result<(), CacheError> {
        self.prepared
            .check_active()
            .map_err(|e| CacheError::Source(e.to_string()))
    }

    /// Stage missing content once per digest. CAS hits never open source paths.
    /// This performs no Action Cache, Execute or output-publication operation.
    pub async fn upload_missing(
        &self,
        cache: &CacheClient,
    ) -> Result<Vec<ReapiDigest>, CacheError> {
        self.check_active()?;
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
        self.check_active()?;
        let missing = self
            .run_while_active(cache.find_missing(&requested))
            .await??;
        self.check_active()?;
        for digest in &missing {
            self.check_active()?;
            if let Some(blob) = blobs.get(digest) {
                self.run_while_active(cache.upload_missing(&[(*blob).clone()]))
                    .await??;
            } else {
                let index = *sources
                    .get(digest)
                    .expect("validated missing response is requested");
                let file = self
                    .prepared
                    .open_source(index)
                    .map_err(|error| CacheError::Source(error.to_string()))?;
                self.run_while_active(cache.upload_reader_verified(digest, file.into_async()))
                    .await??;
            }
            self.check_active()?;
        }
        self.check_active()?;
        Ok(missing.into_iter().collect())
    }
}

#[cfg(test)]
mod tests;
