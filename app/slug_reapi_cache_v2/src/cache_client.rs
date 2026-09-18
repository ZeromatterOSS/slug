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

use futures::stream;
use sha2::Digest as _;
use sha2::Sha256;
use tonic::Code;
use tonic::transport::Channel;

use crate::ReapiBlob;
use crate::ReapiDigest;
use crate::proto;
use crate::proto::google::bytestream;

mod reader_upload;

#[derive(Debug, Clone, Copy)]
pub struct TransferPolicy {
    pub max_batch_bytes: usize,
    pub chunk_bytes: usize,
}

impl Default for TransferPolicy {
    fn default() -> Self {
        Self {
            max_batch_bytes: 1024 * 1024,
            chunk_bytes: 64 * 1024,
        }
    }
}

#[derive(Debug)]
pub enum CacheError {
    Transport(tonic::Status),
    Protocol(String),
    Sink(String),
    Source(String),
}

impl std::fmt::Display for CacheError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Transport(error) => write!(f, "{error}"),
            Self::Protocol(error) | Self::Sink(error) | Self::Source(error) => f.write_str(error),
        }
    }
}

impl std::error::Error for CacheError {}

/// Request-local REAPI transport. The channel and credentials remain caller-owned.
pub struct CacheClient {
    channel: Channel,
    instance_name: String,
    policy: TransferPolicy,
}

impl CacheClient {
    pub fn new(
        channel: Channel,
        instance_name: String,
        policy: TransferPolicy,
    ) -> Result<Self, CacheError> {
        if policy.max_batch_bytes == 0
            || policy.chunk_bytes == 0
            || policy.chunk_bytes > 4 * 1024 * 1024
        {
            return Err(CacheError::Protocol(
                "invalid REAPI transfer policy".to_owned(),
            ));
        }
        Ok(Self {
            channel,
            instance_name,
            policy,
        })
    }

    /// A capability lookup is optional for servers that do not implement it.
    /// When present, its advertised batch limit can only reduce our local bound.
    pub async fn apply_server_batch_limit(&mut self) -> Result<(), CacheError> {
        let mut client = proto::capabilities_client::CapabilitiesClient::new(self.channel.clone());
        let response = client
            .get_capabilities(proto::GetCapabilitiesRequest {
                instance_name: self.instance_name.clone(),
            })
            .await;
        let response = match response {
            Ok(response) => response.into_inner(),
            Err(status) if status.code() == Code::Unimplemented => return Ok(()),
            Err(status) => return Err(CacheError::Transport(status)),
        };
        if let Some(cache) = response.cache_capabilities {
            if cache.max_batch_total_size_bytes > 0 {
                let limit = usize::try_from(cache.max_batch_total_size_bytes).unwrap_or(usize::MAX);
                self.policy.max_batch_bytes = self.policy.max_batch_bytes.min(limit);
            }
        }
        Ok(())
    }

    pub async fn find_missing(
        &self,
        digests: &BTreeSet<ReapiDigest>,
    ) -> Result<BTreeSet<ReapiDigest>, CacheError> {
        if digests.is_empty() {
            return Ok(BTreeSet::new());
        }
        let mut cas =
            proto::content_addressable_storage_client::ContentAddressableStorageClient::new(
                self.channel.clone(),
            );
        let response = cas
            .find_missing_blobs(proto::FindMissingBlobsRequest {
                instance_name: self.instance_name.clone(),
                blob_digests: digests.iter().map(to_proto).collect(),
                ..Default::default()
            })
            .await
            .map_err(CacheError::Transport)?
            .into_inner();
        validate_missing_response(digests, &response)
    }

    pub async fn upload_missing(&self, blobs: &[ReapiBlob]) -> Result<(), CacheError> {
        let mut seen = BTreeSet::new();
        for blob in blobs {
            if !seen.insert(blob.digest().clone()) {
                return Err(CacheError::Protocol(
                    "duplicate CAS upload digest".to_owned(),
                ));
            }
            if blob.digest().verify_bytes(blob.data()).is_err() {
                return Err(CacheError::Protocol(
                    "CAS upload bytes do not match digest".to_owned(),
                ));
            }
            // Include conservative protobuf overhead in the bound.
            if blob.data().len().saturating_add(256) <= self.policy.max_batch_bytes {
                self.batch_upload(blob).await?;
            } else {
                self.stream_upload(blob).await?;
            }
        }
        Ok(())
    }

    /// Upload a caller-owned reader with bounded buffers. EOF, size and SHA-256
    /// must match before finalizing the write. The caller owns source provenance
    /// and missing-blob policy; this method neither opens paths nor retries them.
    /// Cancellation drops the reader. An early server response fails closed.
    pub async fn upload_reader_verified<R>(
        &self,
        digest: &ReapiDigest,
        reader: R,
    ) -> Result<(), CacheError>
    where
        R: tokio::io::AsyncRead + Unpin + Send,
    {
        let resource = self.resource_name(digest, true);
        let mut client =
            bytestream::byte_stream_client::ByteStreamClient::new(self.channel.clone());
        let result = reader_upload::upload(
            reader,
            digest,
            resource.clone(),
            self.policy.chunk_bytes,
            |requests| async {
                let requests = stream::unfold(requests, |mut requests| async {
                    requests.recv().await.map(|request| (request, requests))
                });
                client
                    .write(requests)
                    .await
                    .map(tonic::Response::into_inner)
                    .map_err(CacheError::Transport)
            },
        )
        .await;
        // The coordination future has already dropped its producer and reader.
        // Status is diagnostic only, never a substitute for a verified upload.
        match result {
            Err(CacheError::Transport(error)) => Err(interrupted_write_error(error, || {
                client.query_write_status(bytestream::QueryWriteStatusRequest {
                    resource_name: resource,
                })
            })
            .await),
            other => other,
        }
    }

    async fn batch_upload(&self, blob: &ReapiBlob) -> Result<(), CacheError> {
        let mut cas =
            proto::content_addressable_storage_client::ContentAddressableStorageClient::new(
                self.channel.clone(),
            );
        let response = cas
            .batch_update_blobs(proto::BatchUpdateBlobsRequest {
                instance_name: self.instance_name.clone(),
                requests: vec![proto::batch_update_blobs_request::Request {
                    digest: Some(to_proto(blob.digest())),
                    data: blob.data().to_vec(),
                    ..Default::default()
                }],
                ..Default::default()
            })
            .await
            .map_err(CacheError::Transport)?
            .into_inner();
        validate_upload_response(blob.digest(), &response)
    }

    async fn stream_upload(&self, blob: &ReapiBlob) -> Result<(), CacheError> {
        let size = i64::try_from(blob.data().len())
            .map_err(|_| CacheError::Protocol("blob exceeds ByteStream offset range".to_owned()))?;
        let resource = self.resource_name(blob.digest(), true);
        let requests = write_request_stream(
            blob.shared_data(),
            resource.clone(),
            self.policy.chunk_bytes,
        );
        let mut client =
            bytestream::byte_stream_client::ByteStreamClient::new(self.channel.clone());
        let response = match client.write(requests).await {
            Ok(response) => response.into_inner(),
            Err(error) => {
                return Err(interrupted_write_error(error, || {
                    client.query_write_status(bytestream::QueryWriteStatusRequest {
                        resource_name: resource,
                    })
                })
                .await);
            }
        };
        if response.committed_size != size {
            return Err(CacheError::Protocol(format!(
                "ByteStream committed {} of {size} bytes",
                response.committed_size
            )));
        }
        Ok(())
    }

    pub async fn get_action_result(
        &self,
        digest: &ReapiDigest,
        inline_output_files: Vec<String>,
    ) -> Result<Option<proto::ActionResult>, CacheError> {
        let mut ac = proto::action_cache_client::ActionCacheClient::new(self.channel.clone());
        classify_ac_response(
            ac.get_action_result(proto::GetActionResultRequest {
                instance_name: self.instance_name.clone(),
                action_digest: Some(to_proto(digest)),
                inline_stdout: true,
                inline_stderr: true,
                inline_output_files,
                ..Default::default()
            })
            .await,
        )
    }

    /// Chunks written to the sink are provisional until this method succeeds.
    pub async fn read_blob_verified<F>(
        &self,
        digest: &ReapiDigest,
        mut sink: F,
    ) -> Result<(), CacheError>
    where
        F: FnMut(&[u8]) -> Result<(), CacheError>,
    {
        let mut hasher = Sha256::new();
        let mut size = 0u64;
        if digest.size_bytes().saturating_add(256) <= self.policy.max_batch_bytes as u64 {
            let mut cas =
                proto::content_addressable_storage_client::ContentAddressableStorageClient::new(
                    self.channel.clone(),
                );
            let response = cas
                .batch_read_blobs(proto::BatchReadBlobsRequest {
                    instance_name: self.instance_name.clone(),
                    digests: vec![to_proto(digest)],
                    ..Default::default()
                })
                .await
                .map_err(CacheError::Transport)?
                .into_inner();
            let bytes = validate_read_response(digest, &response)?;
            consume_chunk(bytes, digest, &mut size, &mut hasher, &mut sink)?;
        } else {
            let mut client =
                bytestream::byte_stream_client::ByteStreamClient::new(self.channel.clone());
            let mut responses = client
                .read(bytestream::ReadRequest {
                    resource_name: self.resource_name(digest, false),
                    read_offset: 0,
                    read_limit: 0,
                })
                .await
                .map_err(CacheError::Transport)?
                .into_inner();
            while let Some(response) = responses.message().await.map_err(CacheError::Transport)? {
                for chunk in response.data.chunks(self.policy.chunk_bytes) {
                    consume_chunk(chunk, digest, &mut size, &mut hasher, &mut sink)?;
                }
            }
        }
        verify_download(digest, size, hasher)
    }

    fn resource_name(&self, digest: &ReapiDigest, upload: bool) -> String {
        let prefix = if self.instance_name.is_empty() {
            String::new()
        } else {
            format!("{}/", self.instance_name.trim_end_matches('/'))
        };
        if upload {
            format!(
                "{prefix}uploads/{:032x}/blobs/{digest}",
                rand::random::<u128>()
            )
        } else {
            format!("{prefix}blobs/{digest}")
        }
    }
}

fn write_request_stream(
    data: std::sync::Arc<[u8]>,
    resource: String,
    chunk_bytes: usize,
) -> impl futures::Stream<Item = bytestream::WriteRequest> + Send + 'static {
    stream::unfold(0usize, move |offset| {
        let data = data.clone();
        let resource = resource.clone();
        async move {
            if offset > data.len() {
                return None;
            }
            let end = offset.saturating_add(chunk_bytes).min(data.len());
            let request = bytestream::WriteRequest {
                resource_name: if offset == 0 { resource } else { String::new() },
                write_offset: offset as i64,
                finish_write: end == data.len(),
                data: data[offset..end].to_vec(),
            };
            Some((
                request,
                if end == data.len() {
                    data.len() + 1
                } else {
                    end
                },
            ))
        }
    })
}

fn verify_download(digest: &ReapiDigest, size: u64, hasher: Sha256) -> Result<(), CacheError> {
    let actual_hash = format!("{:x}", hasher.finalize());
    if size != digest.size_bytes() || actual_hash != digest.hash() {
        return Err(CacheError::Protocol(format!(
            "download digest mismatch: expected {digest}, got {actual_hash}/{size}"
        )));
    }
    Ok(())
}

fn consume_chunk<F>(
    chunk: &[u8],
    expected: &ReapiDigest,
    size: &mut u64,
    hasher: &mut Sha256,
    sink: &mut F,
) -> Result<(), CacheError>
where
    F: FnMut(&[u8]) -> Result<(), CacheError>,
{
    *size = size
        .checked_add(chunk.len() as u64)
        .ok_or_else(|| CacheError::Protocol("download size overflow".to_owned()))?;
    if *size > expected.size_bytes() {
        return Err(CacheError::Protocol(
            "download exceeds declared digest size".to_owned(),
        ));
    }
    hasher.update(chunk);
    sink(chunk)
}

pub fn to_proto(digest: &ReapiDigest) -> proto::Digest {
    proto::Digest {
        hash: digest.hash().to_owned(),
        size_bytes: i64::try_from(digest.size_bytes()).expect("REAPI digest fits i64"),
    }
}

pub fn from_proto(digest: &proto::Digest) -> Result<ReapiDigest, CacheError> {
    let size = u64::try_from(digest.size_bytes)
        .map_err(|_| CacheError::Protocol("negative REAPI digest size".to_owned()))?;
    ReapiDigest::new(digest.hash.clone(), size).map_err(CacheError::Protocol)
}

fn check_status(status: Option<&proto::google::rpc::Status>) -> Result<(), CacheError> {
    match status {
        Some(status) if status.code == 0 => Ok(()),
        Some(status) => Err(CacheError::Protocol(format!(
            "CAS status {}: {}",
            status.code, status.message
        ))),
        None => Err(CacheError::Protocol(
            "CAS response omitted status".to_owned(),
        )),
    }
}

fn validate_missing_response(
    requested: &BTreeSet<ReapiDigest>,
    response: &proto::FindMissingBlobsResponse,
) -> Result<BTreeSet<ReapiDigest>, CacheError> {
    let mut missing = BTreeSet::new();
    for wire in &response.missing_blob_digests {
        let digest = from_proto(wire)?;
        if !requested.contains(&digest) || !missing.insert(digest) {
            return Err(CacheError::Protocol(
                "FindMissingBlobs returned an unexpected or duplicate digest".to_owned(),
            ));
        }
    }
    Ok(missing)
}

fn validate_read_response<'a>(
    expected: &ReapiDigest,
    batch: &'a proto::BatchReadBlobsResponse,
) -> Result<&'a [u8], CacheError> {
    if batch.responses.len() != 1 {
        return Err(CacheError::Protocol(
            "BatchReadBlobs omitted or duplicated response".to_owned(),
        ));
    }
    let response = &batch.responses[0];
    if response
        .digest
        .as_ref()
        .map(from_proto)
        .transpose()?
        .as_ref()
        != Some(expected)
        || response.compressor != 0
    {
        return Err(CacheError::Protocol(
            "BatchReadBlobs returned a different digest or compressor".to_owned(),
        ));
    }
    check_status(response.status.as_ref())?;
    Ok(&response.data)
}

async fn interrupted_write_error<F, Fut>(error: tonic::Status, query: F) -> CacheError
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<
            Output = Result<tonic::Response<bytestream::QueryWriteStatusResponse>, tonic::Status>,
        >,
{
    let status = query().await;
    CacheError::Protocol(format!(
        "ByteStream Write interrupted: {error}; QueryWriteStatus: {status:?}"
    ))
}

fn validate_upload_response(
    expected: &ReapiDigest,
    batch: &proto::BatchUpdateBlobsResponse,
) -> Result<(), CacheError> {
    if batch.responses.len() != 1 {
        return Err(CacheError::Protocol(
            "BatchUpdateBlobs omitted or duplicated response".to_owned(),
        ));
    }
    let response = &batch.responses[0];
    if response
        .digest
        .as_ref()
        .map(from_proto)
        .transpose()?
        .as_ref()
        != Some(expected)
    {
        return Err(CacheError::Protocol(
            "BatchUpdateBlobs returned a different digest".to_owned(),
        ));
    }
    check_status(response.status.as_ref())
}

fn classify_ac_response(
    response: Result<tonic::Response<proto::ActionResult>, tonic::Status>,
) -> Result<Option<proto::ActionResult>, CacheError> {
    match response {
        Ok(response) => Ok(Some(response.into_inner())),
        Err(status) if status.code() == Code::NotFound => Ok(None),
        Err(status) => Err(CacheError::Transport(status)),
    }
}

#[cfg(test)]
mod tests {
    use futures::StreamExt;

    use super::*;

    #[tokio::test]
    async fn tiny_chunks_keep_offsets_and_finish_once() {
        let blob = ReapiBlob::from_bytes(b"abcde".to_vec());
        let requests = write_request_stream(
            blob.shared_data(),
            "instance/uploads/id/blobs/hash/5".to_owned(),
            2,
        )
        .collect::<Vec<_>>()
        .await;
        assert_eq!(
            requests
                .iter()
                .map(|request| request.write_offset)
                .collect::<Vec<_>>(),
            [0, 2, 4]
        );
        assert_eq!(
            requests
                .iter()
                .map(|request| request.data.as_slice())
                .collect::<Vec<_>>(),
            [b"ab".as_slice(), b"cd", b"e"]
        );
        assert_eq!(
            requests
                .iter()
                .map(|request| request.finish_write)
                .collect::<Vec<_>>(),
            [false, false, true]
        );
        assert!(!requests[0].resource_name.is_empty());
        assert!(
            requests[1..]
                .iter()
                .all(|request| request.resource_name.is_empty())
        );
    }

    #[tokio::test]
    async fn resource_names_are_scoped_to_the_client_instance() {
        let channel = tonic::transport::Endpoint::from_static("http://127.0.0.1:1").connect_lazy();
        let first = CacheClient::new(
            channel.clone(),
            "first".to_owned(),
            TransferPolicy::default(),
        )
        .unwrap();
        let second =
            CacheClient::new(channel, "second".to_owned(), TransferPolicy::default()).unwrap();
        let digest = ReapiDigest::of_bytes(b"same bytes");
        assert_eq!(
            first.resource_name(&digest, false),
            format!("first/blobs/{digest}")
        );
        assert_eq!(
            second.resource_name(&digest, false),
            format!("second/blobs/{digest}")
        );
        assert!(
            first
                .resource_name(&digest, true)
                .starts_with("first/uploads/")
        );
        assert!(
            second
                .resource_name(&digest, true)
                .starts_with("second/uploads/")
        );
    }

    #[test]
    fn provisional_download_rejects_extra_short_and_corrupt_chunks() {
        let expected = ReapiDigest::of_bytes(b"abc");
        let mut size = 0;
        let mut hasher = Sha256::new();
        let mut scratch = Vec::new();
        let mut sink = |chunk: &[u8]| {
            scratch.extend_from_slice(chunk);
            Ok(())
        };
        consume_chunk(b"ab", &expected, &mut size, &mut hasher, &mut sink).unwrap();
        assert!(verify_download(&expected, size, hasher.clone()).is_err());
        consume_chunk(b"c", &expected, &mut size, &mut hasher, &mut sink).unwrap();
        assert!(verify_download(&expected, size, hasher).is_ok());
        assert_eq!(scratch, b"abc");

        let mut size = 0;
        let mut hasher = Sha256::new();
        assert!(
            consume_chunk(b"abcd", &expected, &mut size, &mut hasher, &mut |_| Ok(())).is_err()
        );
        assert_eq!(size, 4);
        let mut size = 0;
        let mut hasher = Sha256::new();
        consume_chunk(b"abd", &expected, &mut size, &mut hasher, &mut |_| Ok(())).unwrap();
        assert!(verify_download(&expected, size, hasher).is_err());
    }

    #[test]
    fn cas_status_must_be_present_and_successful() {
        assert!(check_status(None).is_err());
        assert!(
            check_status(Some(&proto::google::rpc::Status {
                code: 7,
                message: "denied".into(),
                details: Vec::new()
            }))
            .is_err()
        );
        assert!(check_status(Some(&proto::google::rpc::Status::default())).is_ok());
    }

    #[test]
    fn action_cache_only_not_found_is_a_miss() {
        let hit =
            classify_ac_response(Ok(tonic::Response::new(proto::ActionResult::default()))).unwrap();
        assert!(hit.is_some());
        let miss = classify_ac_response(Err(tonic::Status::not_found("absent"))).unwrap();
        assert!(miss.is_none());
        assert!(matches!(
            classify_ac_response(Err(tonic::Status::permission_denied("denied"))),
            Err(CacheError::Transport(_))
        ));
    }

    #[test]
    fn upload_response_must_match_one_requested_digest() {
        let expected = ReapiDigest::of_bytes(b"x");
        let response = proto::batch_update_blobs_response::Response {
            digest: Some(to_proto(&expected)),
            status: Some(proto::google::rpc::Status::default()),
        };
        assert!(
            validate_upload_response(
                &expected,
                &proto::BatchUpdateBlobsResponse {
                    responses: vec![response.clone()]
                }
            )
            .is_ok()
        );
        assert!(
            validate_upload_response(
                &expected,
                &proto::BatchUpdateBlobsResponse { responses: vec![] }
            )
            .is_err()
        );
        assert!(
            validate_upload_response(
                &expected,
                &proto::BatchUpdateBlobsResponse {
                    responses: vec![response.clone(), response.clone()]
                }
            )
            .is_err()
        );
        let wrong = proto::batch_update_blobs_response::Response {
            digest: Some(to_proto(&ReapiDigest::of_bytes(b"y"))),
            ..response
        };
        assert!(
            validate_upload_response(
                &expected,
                &proto::BatchUpdateBlobsResponse {
                    responses: vec![wrong]
                }
            )
            .is_err()
        );
    }

    #[test]
    fn find_missing_rejects_unrequested_and_duplicate_digests() {
        let expected = ReapiDigest::of_bytes(b"requested");
        let requested = BTreeSet::from([expected.clone()]);
        let valid = proto::FindMissingBlobsResponse {
            missing_blob_digests: vec![to_proto(&expected)],
        };
        assert_eq!(
            validate_missing_response(&requested, &valid).unwrap(),
            requested
        );
        assert!(
            validate_missing_response(
                &requested,
                &proto::FindMissingBlobsResponse {
                    missing_blob_digests: vec![to_proto(&expected), to_proto(&expected)]
                }
            )
            .is_err()
        );
        assert!(
            validate_missing_response(
                &requested,
                &proto::FindMissingBlobsResponse {
                    missing_blob_digests: vec![to_proto(&ReapiDigest::of_bytes(b"other"))]
                }
            )
            .is_err()
        );
    }

    #[test]
    fn batch_read_rejects_missing_duplicate_wrong_and_failed_responses() {
        let expected = ReapiDigest::of_bytes(b"data");
        let valid = proto::batch_read_blobs_response::Response {
            digest: Some(to_proto(&expected)),
            data: b"data".to_vec(),
            status: Some(proto::google::rpc::Status::default()),
            ..Default::default()
        };
        let batch = |responses| proto::BatchReadBlobsResponse { responses };
        assert_eq!(
            validate_read_response(&expected, &batch(vec![valid.clone()])).unwrap(),
            b"data"
        );
        assert!(validate_read_response(&expected, &batch(vec![])).is_err());
        assert!(
            validate_read_response(&expected, &batch(vec![valid.clone(), valid.clone()])).is_err()
        );
        let wrong = proto::batch_read_blobs_response::Response {
            digest: Some(to_proto(&ReapiDigest::of_bytes(b"wrong"))),
            ..valid.clone()
        };
        assert!(validate_read_response(&expected, &batch(vec![wrong])).is_err());
        let failed = proto::batch_read_blobs_response::Response {
            status: Some(proto::google::rpc::Status {
                code: 5,
                message: "missing".into(),
                details: Vec::new(),
            }),
            ..valid
        };
        assert!(validate_read_response(&expected, &batch(vec![failed])).is_err());
    }

    #[tokio::test]
    async fn interrupted_write_queries_status_before_failing_closed() {
        let mut queried = false;
        let error = interrupted_write_error(tonic::Status::cancelled("interrupted"), || {
            queried = true;
            async {
                Ok(tonic::Response::new(bytestream::QueryWriteStatusResponse {
                    committed_size: 2,
                    complete: false,
                }))
            }
        })
        .await;
        assert!(queried);
        assert!(error.to_string().contains("committed_size: 2"));
    }
}
