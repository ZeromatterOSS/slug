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
use kuro_core::cells::name::CellName;
use kuro_fs::paths::file_name::FileNameBuf;

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
) -> kuro_error::Result<Vec<FileNameBuf>> {
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
    ) -> impl Future<Output = kuro_error::Result<Arc<[FileNameBuf]>>>;
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
    type Value = kuro_error::Result<Arc<[FileNameBuf]>>;

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
    async fn get_buildfiles(&mut self, cell: CellName) -> kuro_error::Result<Arc<[FileNameBuf]>> {
        self.compute(&BuildfilesKey(cell)).await?
    }
}

#[cfg(test)]
mod tests {
    use gazebo::prelude::SliceExt;
    use indoc::indoc;
    use kuro_core::cells::name::CellName;

    use crate::buildfiles::parse_buildfile_name;
    use crate::legacy_configs::cells::BuckConfigBasedCells;
    use crate::legacy_configs::configs::testing::TestConfigParserFileOps;

    #[tokio::test]
    async fn test_buildfiles() -> kuro_error::Result<()> {
        let mut file_ops = TestConfigParserFileOps::new(&[
            (
                ".buckconfig",
                indoc!(
                    r#"
                            [cells]
                                root = .
                                other = other/
                                third_party = third_party/
                        "#
                ),
            ),
            (
                "other/.buckconfig",
                indoc!(
                    r#"
                            [cells]
                                other = .
                            [buildfile]
                                name = TARGETS
                                extra_for_test = TARGETS.test
                        "#
                ),
            ),
            (
                "third_party/.buckconfig",
                indoc!(
                    r#"
                            [cells]
                                third_party = .
                            [buildfile]
                                name = BUILD.bazel,BUILD
                        "#
                ),
            ),
        ])?;

        let cells = BuckConfigBasedCells::testing_parse_with_file_ops(&mut file_ops, &[]).await?;

        // Default buildfiles are BUILD.bazel, BUILD (Bazel-compatible).
        // BUCK is NOT in the default list — Plan 35.2 retired the Buck-shaped
        // naming opt-in; the only place BUCK is honored now is via an explicit
        // [buildfile] name = BUCK,... in a workspace's own .buckconfig.
        let config = cells
            .parse_single_cell_with_file_ops(CellName::testing_new("root"), &mut file_ops)
            .await?;
        let default_buildfiles = parse_buildfile_name(&config)?;
        assert_eq!(
            vec!["BUILD.bazel", "BUILD"],
            default_buildfiles.map(|f| f.as_str()),
        );
        assert!(
            !default_buildfiles.iter().any(|f| f.as_str() == "BUCK"),
            "BUCK must not be in the default buildfile list",
        );

        // Custom buildfile names are used directly (no .v2 suffix added)
        let config = cells
            .parse_single_cell_with_file_ops(CellName::testing_new("other"), &mut file_ops)
            .await?;
        assert_eq!(
            vec!["TARGETS", "TARGETS.test"],
            parse_buildfile_name(&config)?.map(|f| f.as_str()),
        );

        // Explicit list in config
        let config = cells
            .parse_single_cell_with_file_ops(CellName::testing_new("third_party"), &mut file_ops)
            .await?;
        assert_eq!(
            vec!["BUILD.bazel", "BUILD"],
            parse_buildfile_name(&config)?.map(|f| f.as_str()),
        );

        Ok(())
    }
}
