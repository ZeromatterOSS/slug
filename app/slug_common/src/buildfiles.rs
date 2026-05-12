/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::future::Future;
use std::sync::Arc;

use dice::CancellationContext;
use dice::DiceComputations;
use dice::Key;
use gazebo::prelude::SliceExt as _;
use gazebo::prelude::VecExt as _;
use slug_core::cells::name::CellName;
use slug_fs::paths::file_name::FileNameBuf;

use crate::legacy_configs::dice::HasLegacyConfigs;
use crate::legacy_configs::key::BuckconfigKeyRef;
use crate::legacy_configs::view::LegacyBuckConfigView;

const DEFAULT_BUILDFILES: &[&str] = &["BUILD.bazel", "BUILD"];

/// Deal with the `buildfile.name` key
///
/// Bazel-compatible defaults: BUILD.bazel takes precedence over BUILD.
/// Custom buildfile names can be configured via [buildfile] name.
pub fn parse_buildfile_name(
    mut config: impl LegacyBuckConfigView,
) -> slug_error::Result<Vec<FileNameBuf>> {
    // Check [buildfile] name for custom buildfile names
    // If not provided, use Bazel-compatible defaults: BUILD.bazel, BUILD
    let mut base = if let Some(buildfiles_value) =
        config.parse_list::<String>(BuckconfigKeyRef {
            section: "buildfile",
            property: "name",
        })? {
        buildfiles_value.into_try_map(FileNameBuf::try_from)?
    } else {
        DEFAULT_BUILDFILES.map(|&n| FileNameBuf::try_from(n.to_owned()).unwrap())
    };

    if let Some(buildfile) = config.parse::<String>(BuckconfigKeyRef {
        section: "buildfile",
        property: "extra_for_test",
    })? {
        base.push(FileNameBuf::try_from(buildfile)?);
    }

    Ok(base)
}

pub trait HasBuildfiles {
    fn get_buildfiles(
        &mut self,
        cell: CellName,
    ) -> impl Future<Output = slug_error::Result<Arc<[FileNameBuf]>>>;
}

#[derive(
    Clone,
    derive_more::Display,
    Debug,
    Hash,
    Eq,
    PartialEq,
    allocative::Allocative
)]
#[display("BuildfilesKey({})", self.0)]
struct BuildfilesKey(CellName);

#[async_trait::async_trait]
impl Key for BuildfilesKey {
    type Value = slug_error::Result<Arc<[FileNameBuf]>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let config = ctx.get_legacy_config_on_dice(self.0).await?;
        Ok(parse_buildfile_name(config.view(ctx))?.into())
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
    }
}

impl HasBuildfiles for DiceComputations<'_> {
    async fn get_buildfiles(&mut self, cell: CellName) -> slug_error::Result<Arc<[FileNameBuf]>> {
        self.compute(&BuildfilesKey(cell)).await?
    }
}

#[cfg(test)]
mod tests {
    use gazebo::prelude::SliceExt;
    use slug_cli_proto::ConfigOverride;

    use crate::buildfiles::parse_buildfile_name;
    use crate::legacy_configs::configs::LegacyBuckConfig;

    #[test]
    fn test_buildfiles_defaults() -> slug_error::Result<()> {
        // No [buildfile] override → Bazel-compatible defaults.
        let config = LegacyBuckConfig::empty();
        let buildfiles = parse_buildfile_name(&config)?;
        assert_eq!(vec!["BUILD.bazel", "BUILD"], buildfiles.map(|f| f.as_str()));
        assert!(
            !buildfiles.iter().any(|f| f.as_str() == "BUCK"),
            "BUCK must not be in the default buildfile list",
        );
        Ok(())
    }

    #[test]
    fn test_buildfiles_custom_name() -> slug_error::Result<()> {
        // [buildfile] name=TARGETS via CLI override
        let config = LegacyBuckConfig::from_overrides_only(&[
            ConfigOverride::flag_no_cell("buildfile.name=TARGETS"),
            ConfigOverride::flag_no_cell("buildfile.extra_for_test=TARGETS.test"),
        ])?;
        assert_eq!(
            vec!["TARGETS", "TARGETS.test"],
            parse_buildfile_name(&config)?.map(|f| f.as_str()),
        );
        Ok(())
    }

    #[test]
    fn test_buildfiles_comma_list() -> slug_error::Result<()> {
        // Comma-separated list in [buildfile] name
        let config = LegacyBuckConfig::from_overrides_only(&[ConfigOverride::flag_no_cell(
            "buildfile.name=BUILD.bazel,BUILD",
        )])?;
        assert_eq!(
            vec!["BUILD.bazel", "BUILD"],
            parse_buildfile_name(&config)?.map(|f| f.as_str()),
        );
        Ok(())
    }
}
