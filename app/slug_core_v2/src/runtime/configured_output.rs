/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Sole owner for Slug-native configured output paths.
//!
//! The projection is a path spelling, never semantic identity. Structural
//! configuration remains in DICE; this owner only prevents an unequal
//! structure from ever reusing the same projected directory.

use std::fmt;
use std::fs;
use std::io::Write as _;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Mutex;

use slug_configuration_v2::SlugConfiguration;
use slug_configuration_v2::SlugConfigurationProjection;
use starlark_map::small_map::SmallMap;
use tempfile::NamedTempFile;

#[derive(Debug)]
pub(super) struct ConfiguredOutputOwner {
    workspace: PathBuf,
    claimed: Mutex<SmallMap<SlugConfigurationProjection, SlugConfiguration>>,
}

#[derive(Debug)]
pub(super) enum ConfiguredOutputError {
    ProjectionCollision {
        projection: SlugConfigurationProjection,
    },
    Io {
        operation: &'static str,
        path: PathBuf,
        source: std::io::Error,
    },
}

impl ConfiguredOutputOwner {
    pub(super) fn new(workspace: PathBuf) -> Self {
        Self {
            workspace,
            claimed: Mutex::new(SmallMap::new()),
        }
    }

    /// Claims the projection first in memory and then durably on disk.
    ///
    /// The mutex is deliberately released before the first filesystem call.
    pub(super) fn claim(
        &self,
        configuration: &SlugConfiguration,
    ) -> Result<(), ConfiguredOutputError> {
        let projection = configuration.projection();
        self.register(projection, configuration)?;
        self.claim_sidecar(projection, configuration.canonical_bytes())
    }

    fn register(
        &self,
        projection: SlugConfigurationProjection,
        configuration: &SlugConfiguration,
    ) -> Result<(), ConfiguredOutputError> {
        let mut claimed = self.claimed.lock().map_err(|_| ConfiguredOutputError::Io {
            operation: "locking configured-output collision registry",
            path: self.workspace.clone(),
            source: std::io::Error::other("configured-output collision registry is poisoned"),
        })?;
        if let Some(existing) = claimed.get(&projection) {
            if existing != configuration {
                return Err(ConfiguredOutputError::ProjectionCollision { projection });
            }
            return Ok(());
        }
        claimed.insert(projection, configuration.clone());
        Ok(())
    }

    fn claim_sidecar(
        &self,
        projection: SlugConfigurationProjection,
        canonical_bytes: &[u8],
    ) -> Result<(), ConfiguredOutputError> {
        let marker_dir = self
            .workspace
            .join("bazel-out")
            .join(".slug-configurations");
        fs::create_dir_all(&marker_dir).map_err(|source| {
            io_error(
                "creating configuration marker directory",
                &marker_dir,
                source,
            )
        })?;
        let marker = marker_dir.join(format!("{}.canonical", projection.path_component()));

        let mut temporary = NamedTempFile::new_in(&marker_dir).map_err(|source| {
            io_error(
                "creating temporary configuration marker",
                &marker_dir,
                source,
            )
        })?;
        temporary.write_all(canonical_bytes).map_err(|source| {
            io_error(
                "writing temporary configuration marker",
                temporary.path(),
                source,
            )
        })?;
        temporary.as_file().sync_all().map_err(|source| {
            io_error(
                "syncing temporary configuration marker",
                temporary.path(),
                source,
            )
        })?;

        match temporary.persist_noclobber(&marker) {
            Ok(_) => sync_directory(&marker_dir),
            Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
                let existing = fs::read(&marker)
                    .map_err(|source| io_error("reading configuration marker", &marker, source))?;
                if existing == canonical_bytes {
                    Ok(())
                } else {
                    Err(ConfiguredOutputError::ProjectionCollision { projection })
                }
            }
            Err(error) => Err(io_error(
                "publishing configuration marker",
                &marker,
                error.error,
            )),
        }
    }
}

pub fn configured_output_root(workspace: &Path, configuration: &SlugConfiguration) -> PathBuf {
    workspace
        .join("bazel-out")
        .join(configuration.projection().path_component())
        .join("bin")
}

fn io_error(operation: &'static str, path: &Path, source: std::io::Error) -> ConfiguredOutputError {
    ConfiguredOutputError::Io {
        operation,
        path: path.to_path_buf(),
        source,
    }
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<(), ConfiguredOutputError> {
    fs::File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|source| io_error("syncing configuration marker directory", path, source))
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> Result<(), ConfiguredOutputError> {
    Ok(())
}

impl fmt::Display for ConfiguredOutputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProjectionCollision { projection } => write!(
                formatter,
                "configuration projection collision for {projection}: unequal structural configurations cannot share an output path"
            ),
            Self::Io {
                operation,
                path,
                source,
            } => write!(formatter, "{operation} {}: {source}", path.display()),
        }
    }
}

impl std::error::Error for ConfiguredOutputError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ProjectionCollision { .. } => None,
            Self::Io { source, .. } => Some(source),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use slug_configuration_v2::RootStringSettingValue;
    use slug_configuration_v2::native::host::AutoCpuToken;
    use slug_configuration_v2::native::host::HostConversionInputs;
    use slug_configuration_v2::native::host::HostPathFlavor;

    use super::*;

    fn configurations() -> (SlugConfiguration, SlugConfiguration) {
        let host = HostConversionInputs::new(
            Some(AutoCpuToken::K8),
            Some(HostPathFlavor::Unix),
            None,
            Arc::from([]),
            Arc::from([]),
        )
        .unwrap();
        let base = SlugConfiguration::default_target(&host).unwrap();
        let transitioned =
            base.with_root_string_setting(RootStringSettingValue::new("transitioned"));
        (base, transitioned)
    }

    #[test]
    fn c0_c1_c0_restores_the_exact_configured_root_and_sidecar() {
        let workspace = tempfile::tempdir().unwrap();
        let owner = ConfiguredOutputOwner::new(workspace.path().to_path_buf());
        let (c0, c1) = configurations();

        owner.claim(&c0).unwrap();
        owner.claim(&c1).unwrap();
        owner.claim(&c0).unwrap();

        let c0_root = configured_output_root(workspace.path(), &c0);
        let c1_root = configured_output_root(workspace.path(), &c1);
        assert_ne!(c0_root, c1_root);
        assert_eq!(c0_root, configured_output_root(workspace.path(), &c0));
        assert_eq!(
            fs::read(
                workspace
                    .path()
                    .join("bazel-out/.slug-configurations")
                    .join(format!("{}.canonical", c0.projection().path_component()))
            )
            .unwrap(),
            c0.canonical_bytes()
        );
    }

    #[test]
    fn unequal_structure_reusing_a_projection_fails_in_memory_and_on_disk() {
        let workspace = tempfile::tempdir().unwrap();
        let owner = ConfiguredOutputOwner::new(workspace.path().to_path_buf());
        let (c0, c1) = configurations();
        let projection = c0.projection();

        owner.claim(&c0).unwrap();
        assert!(matches!(
            owner.register(projection, &c1),
            Err(ConfiguredOutputError::ProjectionCollision { .. })
        ));
        let fresh_owner = ConfiguredOutputOwner::new(workspace.path().to_path_buf());
        assert!(matches!(
            fresh_owner.claim_sidecar(projection, c1.canonical_bytes()),
            Err(ConfiguredOutputError::ProjectionCollision { .. })
        ));
    }
}
