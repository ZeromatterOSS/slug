//! Request-local transport for Core's source action operation. No materialization.

use std::sync::Arc;

use slug_core_v2::runtime::PreparedSourceActionInputs;
use slug_core_v2::runtime::SourceActionTransport;
use slug_reapi_cache_v2::CacheClient;
use slug_reapi_cache_v2::TransferPolicy;

use crate::ReapiBlob;
use crate::ReapiDigest;
use crate::RemoteConfig;
use crate::RemoteExecutionError;
use crate::RemoteExecutionResult;
use crate::SourceSpawnReapiPlan;
use crate::executor::cache_error;
use crate::executor::execute_staged;
use crate::executor::required_digests;
use crate::executor::tonic_endpoint;

/// Immutable policy for a single Core-owned execution request. Currently only a
/// shared executor/CAS endpoint without custom headers, retries or timeouts is
/// admitted; unsupported supplied policy is rejected before any network effect.
pub struct SourceReapiTransport {
    config: RemoteConfig,
}

/// Completion evidence and its channel/instance stay inseparable. Fields are
/// private, so callers cannot substitute a plan, CAS, or execution endpoint.
pub struct StagedSourceAction {
    config: RemoteConfig,
    plan: SourceSpawnReapiPlan,
    cache: CacheClient,
    channel: tonic::transport::Channel,
    uploaded: Vec<ReapiDigest>,
}

impl SourceReapiTransport {
    pub fn new(config: RemoteConfig) -> Result<Self, RemoteExecutionError> {
        let executor = config
            .executor
            .as_deref()
            .ok_or(RemoteExecutionError::MissingExecutor)?;
        if !config.headers.is_empty()
            || config.retry_attempts.is_some()
            || config.timeout_seconds.is_some()
        {
            return Err(RemoteExecutionError::Protocol(
                "source execution does not yet admit headers, retries or timeout policy".to_owned(),
            ));
        }
        let endpoint = tonic_endpoint(executor)?;
        if let Some(cache) = &config.cache {
            if tonic_endpoint(cache)? != endpoint {
                return Err(RemoteExecutionError::Protocol(
                    "source execution requires the same executor and CAS endpoint".to_owned(),
                ));
            }
        }
        Ok(Self { config })
    }
}

impl SourceActionTransport for SourceReapiTransport {
    type Staged = StagedSourceAction;
    type Output = RemoteExecutionResult;
    type Error = RemoteExecutionError;

    async fn stage(
        &self,
        inputs: Arc<PreparedSourceActionInputs>,
    ) -> Result<Self::Staged, Self::Error> {
        let plan =
            SourceSpawnReapiPlan::from_prepared(inputs, &self.config.default_exec_properties)
                .map_err(RemoteExecutionError::Command)?;
        let endpoint =
            tonic_endpoint(self.config.executor.as_deref().expect("validated executor"))?;
        let channel = tonic::transport::Endpoint::from_shared(endpoint)
            .map_err(|error| RemoteExecutionError::Transport(error.to_string()))?
            .connect()
            .await
            .map_err(|error| RemoteExecutionError::Transport(error.to_string()))?;
        let mut cache = CacheClient::new(
            channel.clone(),
            self.config.instance_name.clone().unwrap_or_default(),
            TransferPolicy::default(),
        )
        .map_err(cache_error)?;
        cache
            .apply_server_batch_limit()
            .await
            .map_err(cache_error)?;
        let mut uploaded = plan
            .inputs()
            .upload_missing(&cache)
            .await
            .map_err(cache_error)?;
        let blobs = [
            ReapiBlob::from_bytes(plan.command().serialized()),
            ReapiBlob::from_bytes(plan.identity().action_bytes().to_vec()),
        ];
        let required = required_digests(plan.inputs().input_tree(), &blobs);
        // required_digests includes file entries; Directory digests must also be
        // checked even when FindMissing reported them present.
        let required = required
            .into_iter()
            .chain(
                plan.inputs()
                    .input_tree()
                    .directory_blobs()
                    .iter()
                    .map(|blob| blob.digest().clone()),
            )
            .collect::<std::collections::BTreeSet<_>>();
        let missing = cache
            .find_missing(&blobs.iter().map(|blob| blob.digest().clone()).collect())
            .await
            .map_err(cache_error)?;
        let missing_blobs = blobs
            .into_iter()
            .filter(|blob| missing.contains(blob.digest()))
            .collect::<Vec<_>>();
        cache
            .upload_missing(&missing_blobs)
            .await
            .map_err(cache_error)?;
        uploaded.extend(missing);
        // Presence is not completion (NativeLink may advertise in-flight writes).
        // Reads verify size and hash with bounded transfer buffers and no retained
        // input bytes. Any incomplete/corrupt value prevents a staged capability.
        for digest in required {
            cache
                .read_blob_verified(&digest, |_| Ok(()))
                .await
                .map_err(cache_error)?;
        }
        Ok(StagedSourceAction {
            config: self.config.clone(),
            plan,
            cache,
            channel,
            uploaded,
        })
    }

    async fn execute(&self, staged: Self::Staged) -> Result<Self::Output, Self::Error> {
        execute_staged(
            &staged.config,
            staged.plan.command(),
            staged.plan.identity(),
            &staged.cache,
            staged.channel,
            Vec::new(),
            staged.uploaded,
        )
        .await
    }
}

#[cfg(test)]
mod tests;
