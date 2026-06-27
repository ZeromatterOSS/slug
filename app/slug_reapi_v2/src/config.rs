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

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum RemoteMode {
    Disabled,
    CacheOnly,
    Execute,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RemoteConfig {
    pub executor: Option<String>,
    pub cache: Option<String>,
    pub instance_name: Option<String>,
    pub headers: BTreeMap<String, String>,
    pub timeout_seconds: Option<u64>,
    pub retry_attempts: Option<u32>,
    pub default_exec_properties: BTreeMap<String, String>,
}

impl RemoteConfig {
    pub fn from_args(args: &[&str]) -> Result<Self, RemoteConfigError> {
        let mut config = Self {
            executor: None,
            cache: None,
            instance_name: None,
            headers: BTreeMap::new(),
            timeout_seconds: None,
            retry_attempts: None,
            default_exec_properties: BTreeMap::new(),
        };

        for arg in args {
            if let Some(value) = arg.strip_prefix("--remote_executor=") {
                config.executor = Some(non_empty("--remote_executor", value)?);
            } else if let Some(value) = arg.strip_prefix("--remote_cache=") {
                config.cache = Some(non_empty("--remote_cache", value)?);
            } else if let Some(value) = arg.strip_prefix("--remote_instance_name=") {
                config.instance_name = Some(non_empty("--remote_instance_name", value)?);
            } else if let Some(value) = arg.strip_prefix("--remote_header=") {
                let (key, value) = parse_key_value("--remote_header", value)?;
                config.headers.insert(key, value);
            } else if let Some(value) = arg.strip_prefix("--remote_timeout=") {
                config.timeout_seconds = Some(parse_u64("--remote_timeout", value)?);
            } else if let Some(value) = arg.strip_prefix("--remote_retries=") {
                config.retry_attempts = Some(parse_u32("--remote_retries", value)?);
            } else if let Some(value) = arg.strip_prefix("--remote_default_exec_properties=") {
                for item in value.split(',').filter(|item| !item.is_empty()) {
                    let (key, value) = parse_key_value("--remote_default_exec_properties", item)?;
                    config.default_exec_properties.insert(key, value);
                }
            }
        }

        if config.cache.is_none() {
            config.cache = config.executor.clone();
        }
        Ok(config)
    }

    pub fn mode(&self) -> RemoteMode {
        match (&self.executor, &self.cache) {
            (Some(_), _) => RemoteMode::Execute,
            (None, Some(_)) => RemoteMode::CacheOnly,
            (None, None) => RemoteMode::Disabled,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum RemoteConfigError {
    EmptyValue { flag: String },
    InvalidKeyValue { flag: String, value: String },
    InvalidInteger { flag: String, value: String },
}

impl fmt::Display for RemoteConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyValue { flag } => write!(f, "{flag} must not be empty"),
            Self::InvalidKeyValue { flag, value } => {
                write!(f, "{flag} expects key=value, got {value}")
            }
            Self::InvalidInteger { flag, value } => {
                write!(f, "{flag} expects an integer, got {value}")
            }
        }
    }
}

impl Error for RemoteConfigError {}

fn non_empty(flag: &str, value: &str) -> Result<String, RemoteConfigError> {
    if value.is_empty() {
        return Err(RemoteConfigError::EmptyValue {
            flag: flag.to_owned(),
        });
    }
    Ok(value.to_owned())
}

fn parse_key_value(flag: &str, value: &str) -> Result<(String, String), RemoteConfigError> {
    let Some((key, value)) = value.split_once('=') else {
        return Err(RemoteConfigError::InvalidKeyValue {
            flag: flag.to_owned(),
            value: value.to_owned(),
        });
    };
    if key.is_empty() || value.is_empty() {
        return Err(RemoteConfigError::InvalidKeyValue {
            flag: flag.to_owned(),
            value: format!("{key}={value}"),
        });
    }
    Ok((key.to_owned(), value.to_owned()))
}

fn parse_u64(flag: &str, value: &str) -> Result<u64, RemoteConfigError> {
    value
        .parse()
        .map_err(|_| RemoteConfigError::InvalidInteger {
            flag: flag.to_owned(),
            value: value.to_owned(),
        })
}

fn parse_u32(flag: &str, value: &str) -> Result<u32, RemoteConfigError> {
    value
        .parse()
        .map_err(|_| RemoteConfigError::InvalidInteger {
            flag: flag.to_owned(),
            value: value.to_owned(),
        })
}
