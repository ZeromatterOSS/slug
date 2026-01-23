/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::fmt;
use std::str::FromStr;
use std::sync::Arc;

use allocative::Allocative;
use kuro_error::kuro_error;
use derive_more::Display;
use dupe::Dupe;

use crate::cells::name::CellName;

#[derive(Debug, Clone, Dupe, Allocative, PartialEq, Eq)]
pub enum ExternalCellOrigin {
    Bundled(CellName),
    Git(GitCellSetup),
    /// Local path module from bzlmod `local_path_override()`.
    LocalPath(LocalPathCellSetup),
}

/// Setup for a local path external cell (from bzlmod local_path_override).
#[derive(
    Debug,
    derive_more::Display,
    Clone,
    Dupe,
    Allocative,
    PartialEq,
    Eq,
    Hash
)]
#[display("local_path({}, {})", module_name, path)]
pub struct LocalPathCellSetup {
    /// The bzlmod module name this local path provides.
    pub module_name: Arc<str>,
    /// The local filesystem path (relative to workspace root).
    pub path: Arc<str>,
}

#[derive(
    Debug,
    derive_more::Display,
    Clone,
    Dupe,
    allocative::Allocative,
    PartialEq,
    Eq,
    Hash
)]
#[display("git({}, {})", git_origin, commit)]
pub struct GitCellSetup {
    pub git_origin: Arc<str>,
    // Guaranteed to be a valid commit hash
    pub commit: Arc<str>,
    pub object_format: Option<GitObjectFormat>,
}

impl fmt::Display for ExternalCellOrigin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bundled(cell) => write!(f, "bundled({cell})"),
            Self::Git(git) => write!(f, "{git}"),
            Self::LocalPath(local) => write!(f, "local_path({}, {})", local.module_name, local.path),
        }
    }
}

#[derive(Debug, Display, Eq, PartialEq, Clone, Dupe, Hash, Allocative)]
pub enum GitObjectFormat {
    #[display("sha1")]
    Sha1,
    #[display("sha256")]
    Sha256,
}

impl FromStr for GitObjectFormat {
    type Err = kuro_error::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "sha1" => Ok(GitObjectFormat::Sha1),
            "sha256" => Ok(GitObjectFormat::Sha256),
            _ => Err(kuro_error!(
                kuro_error::ErrorTag::Input,
                "object_format must be one of `sha1` or `sha256` (got: {})",
                &s,
            )),
        }
    }
}

impl GitObjectFormat {
    pub fn check(&self, s: &str) -> Result<(), kuro_error::Error> {
        match self {
            Self::Sha1 => {
                if s.len() == 40 && s.chars().all(|c| c.is_ascii_hexdigit()) {
                    Ok(())
                } else {
                    Err(kuro_error!(
                        kuro_error::ErrorTag::Input,
                        "not a valid SHA1 digest (got: {})",
                        &s,
                    ))
                }
            }
            Self::Sha256 => {
                if s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit()) {
                    Ok(())
                } else {
                    Err(kuro_error!(
                        kuro_error::ErrorTag::Input,
                        "not a valid SHA256 digest (got: {})",
                        &s,
                    ))
                }
            }
        }
    }
}
