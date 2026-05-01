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
    /// Path to a CA certificates bundle. This must be PEM-encoded. If none is set, a default
    /// bundle will be used.
    ///
    /// This can contain environment variables using shell interpolation syntax (i.e. $VAR). They
    /// will be substituted before using the value.
    pub tls_ca_certs: Option<String>,
    /// Path to a client certificate (and intermediate chain), as well as its associated private
    /// key. This must be PEM-encoded.
    ///
    /// This can contain environment variables using shell interpolation syntax (i.e. $VAR). They
    /// will be substituted before using the value.
    pub tls_client_cert: Option<String>,
    /// HTTP headers to inject in all requests to RE. This is a comma-separated list of `Header:
    /// Value` pairs. Minimal validation of those headers is done here.
    ///
    /// This can contain environment variables using shell interpolation syntax (i.e. $VAR). They
    /// will be substituted before using the value.
    pub http_headers: Vec<HttpHeader>,
    /// Whether to query capabilities from the RBE backend.
    pub capabilities: Option<bool>,
    /// The instance name to use in requests.
    pub instance_name: Option<String>,
    /// Use the Meta version of the request metadata
    pub use_fbcode_metadata: bool,
    /// The max size for a GRPC message to be decoded.
    pub max_decoding_message_size: Option<usize>,
    /// The max cumulative blob size for `Read` and `BatchReadBlobs` methods.
    pub max_total_batch_size: Option<usize>,
    /// Maximum number of concurrent upload requests for each action.
    pub max_concurrent_uploads_per_action: Option<usize>,
    /// Time that digests are assumed to live in CAS after being touched.
    pub cas_ttl_secs: Option<i64>,
    /// Interval in seconds for HTTP/2 ping frames to detect stale connections.
    pub grpc_keepalive_time_secs: Option<u64>,
    /// Timeout in seconds for receiving HTTP/2 ping acknowledgement.
    pub grpc_keepalive_timeout_secs: Option<u64>,
    /// Whether to send HTTP/2 pings when connection is idle.
    pub grpc_keepalive_while_idle: Option<bool>,
    /// Default `(key, value)` properties to attach to every remote
    /// action's `Platform` message. Sourced from
    /// `--remote_default_exec_properties=KEY=VALUE` (CLI) or
    /// `[kuro_re_client] default_exec_properties = ...` (buckconfig).
    /// Bazel uses this to steer RBE worker selection (e.g.
    /// `container-image=...`, `OSFamily=Linux`).
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
        // this is used for all three services by default, if given; if one of
        // them has an explicit address given as well though, use that instead
        let default_address: Option<String> = legacy_config.parse(BuckconfigKeyRef {
            section: BUCK2_RE_CLIENT_CFG_SECTION,
            property: "address",
        })?;

        Ok(Self {
            cas_address: legacy_config
                .parse(BuckconfigKeyRef {
                    section: BUCK2_RE_CLIENT_CFG_SECTION,
                    property: "cas_address",
                })?
                .or(default_address.clone()),
            engine_address: legacy_config
                .parse(BuckconfigKeyRef {
                    section: BUCK2_RE_CLIENT_CFG_SECTION,
                    property: "engine_address",
                })?
                .or(default_address.clone()),
            action_cache_address: legacy_config
                .parse(BuckconfigKeyRef {
                    section: BUCK2_RE_CLIENT_CFG_SECTION,
                    property: "action_cache_address",
                })?
                .or(default_address),
            tls: legacy_config
                .parse(BuckconfigKeyRef {
                    section: BUCK2_RE_CLIENT_CFG_SECTION,
                    property: "tls",
                })?
                .unwrap_or(true),
            tls_ca_certs: legacy_config.parse(BuckconfigKeyRef {
                section: BUCK2_RE_CLIENT_CFG_SECTION,
                property: "tls_ca_certs",
            })?,
            tls_client_cert: legacy_config.parse(BuckconfigKeyRef {
                section: BUCK2_RE_CLIENT_CFG_SECTION,
                property: "tls_client_cert",
            })?,
            http_headers: legacy_config
                .parse_list(BuckconfigKeyRef {
                    section: BUCK2_RE_CLIENT_CFG_SECTION,
                    property: "http_headers",
                })?
                .unwrap_or_default(), // Empty list is as good None.
            capabilities: legacy_config.parse(BuckconfigKeyRef {
                section: BUCK2_RE_CLIENT_CFG_SECTION,
                property: "capabilities",
            })?,
            instance_name: legacy_config.parse(BuckconfigKeyRef {
                section: BUCK2_RE_CLIENT_CFG_SECTION,
                property: "instance_name",
            })?,
            use_fbcode_metadata: legacy_config
                .parse(BuckconfigKeyRef {
                    section: BUCK2_RE_CLIENT_CFG_SECTION,
                    property: "use_fbcode_metadata",
                })?
                .unwrap_or(false),
            max_decoding_message_size: legacy_config.parse(BuckconfigKeyRef {
                section: BUCK2_RE_CLIENT_CFG_SECTION,
                property: "max_decoding_message_size",
            })?,
            max_total_batch_size: legacy_config.parse(BuckconfigKeyRef {
                section: BUCK2_RE_CLIENT_CFG_SECTION,
                property: "max_total_batch_size",
            })?,
            max_concurrent_uploads_per_action: legacy_config.parse(BuckconfigKeyRef {
                section: BUCK2_RE_CLIENT_CFG_SECTION,
                property: "max_concurrent_uploads_per_action",
            })?,
            cas_ttl_secs: legacy_config.parse(BuckconfigKeyRef {
                section: BUCK2_RE_CLIENT_CFG_SECTION,
                property: "cas_ttl_secs",
            })?,
            grpc_keepalive_time_secs: legacy_config.parse(BuckconfigKeyRef {
                section: BUCK2_RE_CLIENT_CFG_SECTION,
                property: "grpc_keepalive_time_secs",
            })?,
            grpc_keepalive_timeout_secs: legacy_config.parse(BuckconfigKeyRef {
                section: BUCK2_RE_CLIENT_CFG_SECTION,
                property: "grpc_keepalive_timeout_secs",
            })?,
            grpc_keepalive_while_idle: legacy_config.parse(BuckconfigKeyRef {
                section: BUCK2_RE_CLIENT_CFG_SECTION,
                property: "grpc_keepalive_while_idle",
            })?,
            default_exec_properties: legacy_config
                .parse_list::<String>(BuckconfigKeyRef {
                    section: BUCK2_RE_CLIENT_CFG_SECTION,
                    property: "default_exec_properties",
                })?
                .unwrap_or_default()
                .into_iter()
                .filter_map(|kv| {
                    let (k, v) = kv.split_once('=')?;
                    Some((k.to_owned(), v.to_owned()))
                })
                .collect(),
        })
    }
}
