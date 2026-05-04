/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::str::FromStr;
use std::time::Duration;

use allocative::Allocative;
use kuro_core::kuro_env;
use kuro_error::BuckErrorContext;
use serde::Deserialize;
use serde::Serialize;

use crate::legacy_configs::configs::LegacyBuckConfig;
use crate::legacy_configs::key::BuckconfigKeyRef;

pub const DEFAULT_RETAINED_EVENT_LOGS: usize = 12;

/// Helper enum to categorize the kind of timeout we get from the startup config.
#[derive(Clone, Debug)]
pub enum Timeout {
    /// Timeout value is set in the config, use that.
    Value(Duration),
    /// Timeout value was not set in config, apply the default.
    Default,
    /// Timeout value was explicitly set to 0, meaning we shouldn't use a timeout.
    NoTimeout,
}

impl Timeout {
    pub fn new(value: Option<Duration>) -> Self {
        match value {
            Some(Duration::ZERO) => Self::NoTimeout,
            Some(value) => Self::Value(value),
            None => Self::Default,
        }
    }
}

#[derive(Allocative, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct HttpConfig {
    pub http2: bool,
    pub max_redirects: Option<usize>,
    pub max_concurrent_requests: Option<usize>,
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            http2: true,
            max_redirects: None,
            max_concurrent_requests: None,
        }
    }
}

impl HttpConfig {
    pub fn connect_timeout(&self) -> Timeout {
        Timeout::Default
    }

    pub fn read_timeout(&self) -> Timeout {
        Timeout::Default
    }

    pub fn write_timeout(&self) -> Timeout {
        Timeout::Default
    }
}

#[derive(
    Allocative,
    Clone,
    Debug,
    Default,
    Serialize,
    Deserialize,
    PartialEq,
    Eq
)]
pub struct SystemWarningConfig {
    /// A threshold that is used to determine the percent of memory kuro uses to display memory pressure warnings.
    /// If None, we don't warn the user.
    /// The corresponding buckconfig is `kuro_system_warning.memory_pressure_threshold_percent`.
    pub memory_pressure_threshold_percent: Option<u64>,
    /// A threshold that is used to determine remaining disk space kuro uses to display disk space warnings.
    /// If None, we don't warn the user.
    /// The corresponding buckconfig is `kuro_system_warning.remaining_disk_space_threshold`.
    pub remaining_disk_space_threshold_gb: Option<u64>,
    /// Minimum number of bytes downloaded to measure average download speed.
    /// If None, we don't warn the user.
    /// The corresponding buckconfig is `kuro_system_warning.min_re_download_bytes_threshold`.
    pub min_re_download_bytes_threshold: Option<u64>,
    /// A threshold that is used to determine if download speed is too low and display a warning.
    /// If None, we don't warn the user.
    /// The corresponding buckconfig is `kuro_system_warning.avg_re_download_bytes_per_sec_threshold`.
    pub avg_re_download_bytes_per_sec_threshold: Option<u64>,
    /// A regex that controls which targets are opted into the vpn check.
    /// The corresponding buckconfig is `kuro_health_check.optin_vpn_check_targets_regex`.
    pub optin_vpn_check_targets_regex: Option<String>,
    /// Whether to enable the stable revision check.
    pub enable_stable_revision_check: Option<bool>,
    /// Run the health checks in a separate process.
    pub enable_health_check_process_isolation: Option<bool>,
}

impl SystemWarningConfig {
    pub fn serialize(&self) -> kuro_error::Result<String> {
        serde_json::to_string(&self).buck_error_context("Error serializing SystemWarningConfig")
    }

    pub fn deserialize(s: &str) -> kuro_error::Result<Self> {
        serde_json::from_str::<Self>(s)
            .buck_error_context("Error deserializing SystemWarningConfig")
    }
}

#[derive(Allocative, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResourceControlConfig {
    /// A config to determine if the resource control should be activated or not.
    /// The corresponding buckconfig is `kuro_resource_control.status` that can take
    /// one of `{off | if_available | required}`.
    pub status: ResourceControlStatus,
    /// Maximum allowed memory usage for all work kuro manages.
    ///
    /// Accepts either a number of bytes or a percentage of the available resources.
    ///
    /// The corresponding buckconfig is `kuro_resource_control.memory_max`.
    pub memory_max: Option<String>,
    /// Like `memory_max`, but controls cgroupv2's `memory.high`
    ///
    /// The corresponding buckconfig is `kuro_resource_control.memory_high`.
    pub memory_high: Option<String>,
    /// A memory threshold that any action is allowed to allocate.
    pub memory_max_per_action: Option<String>,
    /// A memory threshold that any action is allowed to reach before being throttled.
    pub memory_high_per_action: Option<String>,
    /// Memory high limit for action cgroup pool. Used when enable_action_cgroup_pool is true.
    /// The corresponding buckconfig is `kuro_resource_control.memory_high_action_cgroup_pool`.
    /// Mainly for testing purpose.
    pub memory_high_action_cgroup_pool: Option<String>,
    /// If provided and above the threshold, the cgroups will enforce this memory pressure and will freeze/kill actions
    /// to stay under this pressure limit. (Currently only used for logging purposes and doesn't actually do the above)
    pub memory_pressure_threshold_percent: u64,
    /// Enable suspension when memory pressure is high.
    pub enable_suspension: bool,
    pub preferred_action_suspend_strategy: ActionSuspendStrategy,
}

impl ResourceControlConfig {
    pub fn testing_default() -> Self {
        Self::default_or_from_env().unwrap()
    }
}

#[derive(Allocative, Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ActionSuspendStrategy {
    CgroupFreeze,
    KillAndRetry,
}

impl FromStr for ActionSuspendStrategy {
    type Err = kuro_error::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "kill_and_retry" => Ok(Self::KillAndRetry),
            "cgroup_freeze" => Ok(Self::CgroupFreeze),
            _ => Err(kuro_error::kuro_error!(
                kuro_error::ErrorTag::Input,
                "Invalid suspend strategy: `{}`",
                s
            )),
        }
    }
}

#[derive(
    Allocative,
    Clone,
    Copy,
    Debug,
    Default,
    Serialize,
    Deserialize,
    PartialEq,
    Eq
)]
pub enum ResourceControlStatus {
    #[default]
    /// The resource is not controlled or limited.
    Off,
    /// The resource is controlled by `systemd` if it's available on the system, otherwise off.
    IfAvailable,
    /// The resource is controlled by `systemd`. If it is not available on the system,
    /// kuro errors it out and the command returns with an error exit code.
    Required,
}

impl FromStr for ResourceControlStatus {
    type Err = kuro_error::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "off" => Ok(Self::Off),
            "if_available" => Ok(Self::IfAvailable),
            "required" => Ok(Self::Required),
            _ => Err(kuro_error::kuro_error!(
                kuro_error::ErrorTag::Input,
                "Invalid resource control status: `{}`",
                s
            )),
        }
    }
}

impl ResourceControlConfig {
    pub fn default_or_from_env() -> kuro_error::Result<Self> {
        if let Some(env_conf) = kuro_env!(
            "BUCK2_TEST_RESOURCE_CONTROL_CONFIG",
            applicability = testing,
        )? {
            Self::deserialize(env_conf)
        } else {
            Ok(Self {
                status: ResourceControlStatus::Off,
                memory_max: None,
                memory_high: None,
                memory_max_per_action: None,
                memory_high_per_action: None,
                memory_high_action_cgroup_pool: None,
                memory_pressure_threshold_percent: 10,
                enable_suspension: false,
                preferred_action_suspend_strategy: ActionSuspendStrategy::KillAndRetry,
            })
        }
    }

    pub fn serialize(&self) -> kuro_error::Result<String> {
        serde_json::to_string(&self).buck_error_context("Error serializing ResourceControlConfig")
    }

    pub fn deserialize(s: &str) -> kuro_error::Result<Self> {
        serde_json::from_str::<Self>(s)
            .buck_error_context("Error deserializing ResourceControlConfig")
    }
}

#[derive(Allocative, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum LogDownloadMethod {
    Manifold,
    Curl(String),
    None,
}

#[derive(
    Allocative,
    Clone,
    Debug,
    Default,
    Serialize,
    Deserialize,
    PartialEq,
    Eq
)]
pub struct HealthCheckConfig {
    pub enable_health_checks: bool,
    pub disabled_health_check_names: Option<String>,
}

/// Configurations that are used at startup by the daemon. Those are actually read by the client,
/// and passed on to the daemon.
///
/// The fields here are often raw String we get from the buckconfig, the daemon will do
/// deserialization once it receives them. That said, this is not a requirement.
///
/// Backwards compatibility on Serialize / Deserialize is not required: if the client cannot read
/// the DaemonStartupConfig provided by the daemon when it tries to connect, it will reject that
/// daemon and restart (and in fact it will probably not get that far since a version check is done
/// before parsing DaemonStartupConfig).
#[derive(Allocative, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DaemonStartupConfig {
    pub num_tokio_workers: Option<usize>,
    pub daemon_buster: Option<String>,
    pub digest_algorithms: Option<String>,
    pub source_digest_algorithm: Option<String>,
    pub paranoid: bool,
    pub materializations: Option<String>,
    /// File-watcher backend (`watchman`, `notify`, `fs_hash_crawler`,
    /// `edenfs`). Sourced from `--kuro_file_watcher`. `None` means
    /// "use the host default" (`watchman` on Meta-internal builds,
    /// `notify` in OSS).
    #[serde(default)]
    pub file_watcher: Option<String>,
    pub http: HttpConfig,
    pub resource_control: ResourceControlConfig,
    pub log_download_method: LogDownloadMethod,
    pub health_check_config: HealthCheckConfig,
    pub retained_event_logs: usize,
    /// Snapshot of the `[kuro_re_client]` buckconfig section at daemon
    /// startup. Held here (rather than read fresh per-build) so that
    /// changing `--remote_executor` / `--remote_header` (which arrive as
    /// per-invocation buckconfig overrides) triggers a daemon restart via
    /// `DaemonConstraintsRequest::satisfied`. The daemon's
    /// `RemoteExecutionStaticMetadata` is bound at init time from the
    /// same underlying config keys; without this field a stale daemon
    /// would silently keep its old RE client.
    #[serde(default)]
    pub re_config: ReConfigSnapshot,
}

/// Subset of `[kuro_re_client]` buckconfig that's load-bearing for the
/// daemon's remote-execution client. Not the full
/// `KuroOssReConfiguration` — just the fields that, if changed,
/// require reconnecting the RE client (i.e. addresses, TLS, headers,
/// instance).
#[derive(
    Allocative,
    Clone,
    Debug,
    Default,
    Serialize,
    Deserialize,
    PartialEq,
    Eq
)]
pub struct ReConfigSnapshot {
    pub address: Option<String>,
    pub cas_address: Option<String>,
    pub engine_address: Option<String>,
    pub action_cache_address: Option<String>,
    pub tls: Option<bool>,
    /// PEM file used for both the TLS client certificate and key.
    /// Sourced from `--tls_client_certificate` (Bazel-compat).
    #[serde(default)]
    pub tls_client_cert: Option<String>,
    pub http_headers: Vec<String>,
    pub instance_name: Option<String>,
    /// Default `(key, value)` properties to attach to every remote
    /// action's `Platform` message. Populated from
    /// `--remote_default_exec_properties=KEY=VALUE` (Bazel
    /// compatibility) and merged with the kuro defaults at action
    /// upload time. Empty for daemons started without the flag.
    #[serde(default)]
    pub default_exec_properties: Vec<(String, String)>,
}

impl ReConfigSnapshot {
    /// `[kuro_re_client]` is no longer parsed from `.buckconfig`; the
    /// daemon receives RE settings via the per-invocation CLI overlay
    /// (`--remote_executor`, `--remote_header`, …) layered on top in
    /// `streaming.rs::exec_impl`.
    pub fn from_config(_config: &LegacyBuckConfig) -> kuro_error::Result<Self> {
        Ok(Self::default())
    }
}

impl DaemonStartupConfig {
    pub fn new(config: &LegacyBuckConfig) -> kuro_error::Result<Self> {
        let log_download_method = if cfg!(fbcode_build) {
            LogDownloadMethod::Manifold
        } else {
            LogDownloadMethod::None
        };

        Ok(Self {
            num_tokio_workers: None,
            daemon_buster: None,
            // `[kuro] digest_algorithms` is dead — the user-facing knob
            // is `--digest_function`, layered onto this field at
            // constraint-check time in `streaming.rs::exec_impl`.
            digest_algorithms: None,
            source_digest_algorithm: None,
            paranoid: false, // Setup later in ImmediateConfig
            materializations: None,
            file_watcher: None,
            http: HttpConfig::default(),
            resource_control: ResourceControlConfig::default_or_from_env()?,
            log_download_method,
            health_check_config: HealthCheckConfig::default(),
            retained_event_logs: DEFAULT_RETAINED_EVENT_LOGS,
            re_config: ReConfigSnapshot::from_config(config)?,
        })
    }

    pub fn serialize(&self) -> kuro_error::Result<String> {
        serde_json::to_string(&self).buck_error_context("Error serializing DaemonStartupConfig")
    }

    pub fn deserialize(s: &str) -> kuro_error::Result<Self> {
        serde_json::from_str::<Self>(s)
            .buck_error_context("Error deserializing DaemonStartupConfig")
    }

    pub fn testing_empty() -> Self {
        Self {
            num_tokio_workers: None,
            daemon_buster: None,
            digest_algorithms: None,
            source_digest_algorithm: None,
            paranoid: false,
            materializations: None,
            file_watcher: None,
            http: HttpConfig::default(),
            resource_control: ResourceControlConfig::testing_default(),
            log_download_method: if cfg!(fbcode_build) {
                LogDownloadMethod::Manifold
            } else {
                LogDownloadMethod::None
            },
            health_check_config: HealthCheckConfig::default(),
            retained_event_logs: DEFAULT_RETAINED_EVENT_LOGS,
            re_config: ReConfigSnapshot::default(),
        }
    }
}
