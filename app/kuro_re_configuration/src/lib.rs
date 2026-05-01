/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

#![feature(error_generic_member_access)]

use std::str::FromStr;

use allocative::Allocative;
use kuro_common::legacy_configs::configs::LegacyBuckConfig;
use kuro_common::legacy_configs::key::BuckconfigKeyRef;

static BUCK2_RE_CLIENT_CFG_SECTION: &str = "kuro_re_client";

pub trait RemoteExecutionStaticMetadataImpl: Sized {
    fn from_legacy_config(legacy_config: &LegacyBuckConfig) -> kuro_error::Result<Self>;
    fn cas_semaphore_size(&self) -> usize;
    /// Returns true when an RE backend address is configured. The
    /// executor-config defaulting path uses this to decide whether to
    /// promote the open-source `Executor::Local` default to a hybrid
    /// local/remote executor (Plan 25.2).
    fn is_re_configured(&self) -> bool;
    /// User-supplied default platform properties to attach to every
    /// remote action (Bazel `--remote_default_exec_properties`).
    /// Empty when not configured. Plan 25.3.E.
    fn default_exec_properties(&self) -> Vec<(String, String)>;
}

#[derive(Clone, Debug, Allocative)]
pub enum CASdAddress {
    Tcp(u16),
    Uds(String),
}

impl FromStr for CASdAddress {
    type Err = kuro_error::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(path) = s.strip_prefix("unix://") {
            Ok(CASdAddress::Uds(path.to_owned()))
        } else {
            Ok(CASdAddress::Tcp(s.parse()?))
        }
    }
}

#[derive(Clone, Debug, Allocative)]
pub enum CASdMode {
    LocalWithSync,
    LocalWithoutSync,
    Remote,
    RemoteToDest,
}

impl FromStr for CASdMode {
    type Err = kuro_error::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "local_with_sync" => Ok(CASdMode::LocalWithSync),
            "local_without_sync" => Ok(CASdMode::LocalWithoutSync),
            "remote" => Ok(CASdMode::Remote),
            "remote_to_dest" => Ok(CASdMode::RemoteToDest),
            _ => Err(kuro_error::kuro_error!(
                kuro_error::ErrorTag::Input,
                "Invalid CASd mode: {}",
                s
            )),
        }
    }
}

#[derive(Clone, Debug, Allocative)]
pub enum CopyPolicy {
    Copy,
    Reflink,
    Hybrid,
}

impl FromStr for CopyPolicy {
    type Err = kuro_error::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "hybrid" => Ok(CopyPolicy::Hybrid),
            "reflink" => Ok(CopyPolicy::Reflink),
            _ => Ok(CopyPolicy::Copy),
        }
    }
}

/// Metadata that doesn't change between executions.
#[derive(Clone, Debug, Default, Allocative)]
pub struct RemoteExecutionStaticMetadata(pub KuroOssReConfiguration);

impl RemoteExecutionStaticMetadataImpl for RemoteExecutionStaticMetadata {
    fn from_legacy_config(legacy_config: &LegacyBuckConfig) -> kuro_error::Result<Self> {
        Ok(Self(KuroOssReConfiguration::from_legacy_config(
            legacy_config,
        )?))
    }

    fn cas_semaphore_size(&self) -> usize {
        // FIXME: make this configurable?
        1024
    }

    fn is_re_configured(&self) -> bool {
        self.0.engine_address.is_some()
    }

    fn default_exec_properties(&self) -> Vec<(String, String)> {
        self.0.default_exec_properties.clone()
    }
}

#[derive(Clone, Debug, Default, Allocative)]
pub struct KuroOssReConfiguration {
    /// Address for RBE Content Addresable Storage service (including bytestream uploads service).
    pub cas_address: Option<String>,
    /// Address for RBE Engine service (including capabilities service).
    pub engine_address: Option<String>,
    /// Address for RBE Action Cache service.
    pub action_cache_address: Option<String>,
    /// Whether to use TLS to interact with remote execution.
    pub tls: bool,
    /// Path to a CA certificates bundle (PEM). Bazel-compat:
    /// `--tls_certificate`. Read by `remote_execution/oss/re_grpc`.
    pub tls_ca_certs: Option<String>,
    /// Path to a client certificate (PEM). Bazel-compat:
    /// `--tls_client_certificate`.
    pub tls_client_cert: Option<String>,
    /// HTTP headers to inject in all requests to RE. Bazel-compat:
    /// `--remote_header=K=V` (repeated).
    pub http_headers: Vec<HttpHeader>,
    /// Whether to query capabilities from the RBE backend.
    pub capabilities: Option<bool>,
    /// The instance name to use in requests. Bazel-compat:
    /// `--remote_instance_name`.
    pub instance_name: Option<String>,
    /// Use the Meta version of the request metadata. Read by re_grpc
    /// for the Meta-internal RE backend used by
    /// `examples/remote_execution/internal/`.
    pub use_fbcode_metadata: bool,
    /// gRPC max-decoded-message-size. Kuro-specific knob.
    pub max_decoding_message_size: Option<usize>,
    /// gRPC `BatchReadBlobs` cumulative-blob-size cap. Kuro-specific knob.
    pub max_total_batch_size: Option<usize>,
    /// Per-action upload concurrency. Kuro-specific knob.
    pub max_concurrent_uploads_per_action: Option<usize>,
    /// CAS TTL hint in seconds. Kuro-specific knob.
    pub cas_ttl_secs: Option<i64>,
    /// HTTP/2 ping interval. Kuro-specific knob.
    pub grpc_keepalive_time_secs: Option<u64>,
    /// HTTP/2 ping ack timeout. Kuro-specific knob.
    pub grpc_keepalive_timeout_secs: Option<u64>,
    /// Send HTTP/2 pings while idle. Kuro-specific knob.
    pub grpc_keepalive_while_idle: Option<bool>,
    /// Default `(key, value)` properties to attach to every remote
    /// action's `Platform` message. Populated from
    /// `--remote_default_exec_properties=KEY=VALUE` via the daemon
    /// startup-config overlay (Plan 25.3.E).
    pub default_exec_properties: Vec<(String, String)>,
}

#[derive(Clone, Debug, Default, Allocative)]
pub struct HttpHeader {
    pub key: String,
    pub value: String,
}

impl FromStr for HttpHeader {
    type Err = kuro_error::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut iter = s.splitn(2, ':');
        match (iter.next(), iter.next()) {
            (Some(key), Some(value)) => Ok(Self {
                key: key.trim().to_owned(),
                value: value.trim().to_owned(),
            }),
            _ => Err(kuro_error::kuro_error!(
                kuro_error::ErrorTag::Input,
                "Invalid header (expect name and value separated by `:`): `{}`",
                s
            )),
        }
    }
}

impl KuroOssReConfiguration {
    pub fn from_legacy_config(legacy_config: &LegacyBuckConfig) -> kuro_error::Result<Self> {
        macro_rules! key {
            ($property:literal) => {
                BuckconfigKeyRef {
                    section: BUCK2_RE_CLIENT_CFG_SECTION,
                    property: $property,
                }
            };
        }
        Ok(Self {
            cas_address: legacy_config.parse(key!("cas_address"))?,
            engine_address: legacy_config.parse(key!("engine_address"))?,
            action_cache_address: legacy_config.parse(key!("action_cache_address"))?,
            tls: legacy_config.parse(key!("tls"))?.unwrap_or(true),
            tls_ca_certs: legacy_config.parse(key!("tls_ca_certs"))?,
            tls_client_cert: legacy_config.parse(key!("tls_client_cert"))?,
            http_headers: legacy_config
                .parse_list(key!("http_headers"))?
                .unwrap_or_default(),
            capabilities: legacy_config.parse(key!("capabilities"))?,
            instance_name: legacy_config.parse(key!("instance_name"))?,
            use_fbcode_metadata: legacy_config
                .parse(key!("use_fbcode_metadata"))?
                .unwrap_or(false),
            max_decoding_message_size: legacy_config.parse(key!("max_decoding_message_size"))?,
            max_total_batch_size: legacy_config.parse(key!("max_total_batch_size"))?,
            max_concurrent_uploads_per_action: legacy_config
                .parse(key!("max_concurrent_uploads_per_action"))?,
            cas_ttl_secs: legacy_config.parse(key!("cas_ttl_secs"))?,
            grpc_keepalive_time_secs: legacy_config.parse(key!("grpc_keepalive_time_secs"))?,
            grpc_keepalive_timeout_secs: legacy_config
                .parse(key!("grpc_keepalive_timeout_secs"))?,
            grpc_keepalive_while_idle: legacy_config.parse(key!("grpc_keepalive_while_idle"))?,
            // CLI-flag-only; populated by the daemon startup-config overlay.
            default_exec_properties: Vec::new(),
        })
    }
}
