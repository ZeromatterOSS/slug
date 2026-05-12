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
use slug_common::legacy_configs::configs::LegacyBuckConfig;

pub trait RemoteExecutionStaticMetadataImpl: Sized {
    fn from_legacy_config(legacy_config: &LegacyBuckConfig) -> slug_error::Result<Self>;
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
    type Err = slug_error::Error;

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
    type Err = slug_error::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "local_with_sync" => Ok(CASdMode::LocalWithSync),
            "local_without_sync" => Ok(CASdMode::LocalWithoutSync),
            "remote" => Ok(CASdMode::Remote),
            "remote_to_dest" => Ok(CASdMode::RemoteToDest),
            _ => Err(slug_error::slug_error!(
                slug_error::ErrorTag::Input,
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
    type Err = slug_error::Error;

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
pub struct RemoteExecutionStaticMetadata(pub SlugOssReConfiguration);

impl RemoteExecutionStaticMetadataImpl for RemoteExecutionStaticMetadata {
    fn from_legacy_config(legacy_config: &LegacyBuckConfig) -> slug_error::Result<Self> {
        Ok(Self(SlugOssReConfiguration::from_legacy_config(
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

#[derive(Clone, Debug, Allocative)]
pub struct SlugOssReConfiguration {
    /// Address for RBE Content Addresable Storage service (including bytestream uploads service).
    pub cas_address: Option<String>,
    /// Address for RBE Engine service (including capabilities service).
    pub engine_address: Option<String>,
    /// Address for RBE Action Cache service.
    pub action_cache_address: Option<String>,
    /// Whether to use TLS to interact with remote execution.
    pub tls: bool,
    /// Path to a client certificate (PEM). Bazel-compat:
    /// `--tls_client_certificate`.
    pub tls_client_cert: Option<String>,
    /// HTTP headers to inject in all requests to RE. Bazel-compat:
    /// `--remote_header=K=V` (repeated).
    pub http_headers: Vec<HttpHeader>,
    /// The instance name to use in requests. Bazel-compat:
    /// `--remote_instance_name`.
    pub instance_name: Option<String>,
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
    type Err = slug_error::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut iter = s.splitn(2, ':');
        match (iter.next(), iter.next()) {
            (Some(key), Some(value)) => Ok(Self {
                key: key.trim().to_owned(),
                value: value.trim().to_owned(),
            }),
            _ => Err(slug_error::slug_error!(
                slug_error::ErrorTag::Input,
                "Invalid header (expect name and value separated by `:`): `{}`",
                s
            )),
        }
    }
}

impl Default for SlugOssReConfiguration {
    fn default() -> Self {
        // `tls = true` matches the prior `[slug_re_client] tls`
        // buckconfig default. Local insecure backends (buildbarn,
        // nativelink, vscode) flip it via URL-scheme detection in
        // `apply_re_config_overlay`.
        Self {
            cas_address: None,
            engine_address: None,
            action_cache_address: None,
            tls: true,
            tls_client_cert: None,
            http_headers: Vec::new(),
            instance_name: None,
            default_exec_properties: Vec::new(),
        }
    }
}

impl SlugOssReConfiguration {
    /// `[slug_re_client]` is no longer parsed from `.buckconfig`. RE
    /// settings arrive per-invocation via the CLI flags
    /// (`--remote_executor`, `--remote_header`,
    /// `--remote_instance_name`, `--tls_client_certificate`,
    /// `--remote_default_exec_properties`) and are layered onto this
    /// default in `slug_server::daemon::state::apply_re_config_overlay`.
    pub fn from_legacy_config(_legacy_config: &LegacyBuckConfig) -> slug_error::Result<Self> {
        Ok(Self::default())
    }
}
