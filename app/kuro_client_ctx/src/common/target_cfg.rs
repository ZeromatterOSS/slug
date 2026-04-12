/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use kuro_cli_proto::TargetCfg;

const HELP_HEADING: &str = "Target Configuration Options";

/// Defines options related to commands that involves a streaming daemon command.
#[derive(Debug, clap::Parser, serde::Serialize, serde::Deserialize, Default)]
#[clap(next_help_heading = HELP_HEADING)]
pub struct TargetCfgOptions {
    #[clap(
        long = "target-platforms",
        alias = "platforms",
        help = "Configuration target (one) to use to configure targets (Bazel: --platforms)",
        num_args = 1,
        value_name = "PLATFORM"
    )]
    pub target_platforms: Option<String>,

    #[clap(
        value_name = "VALUE",
        long = "modifier",
        short = 'm',
        help = "A configuration modifier to configure all targets on the command line. This may be a constraint value target."
    )]
    pub cli_modifier: Vec<String>,
}

#[derive(Debug, clap::Parser, serde::Serialize, serde::Deserialize, Default)]
pub struct TargetCfgUnusedOptions {
    /// This option is not used.
    #[clap(
        long = "target-platforms",
        alias = "platforms",
        num_args = 1,
        hide = true,
        value_name = "PLATFORM"
    )]
    pub target_platforms: Option<String>,

    /// This option is not used.
    #[clap(value_name = "VALUE", long = "modifier", short = 'm')]
    pub cli_modifier: Vec<String>,
}

impl TargetCfgOptions {
    pub fn target_cfg(&self) -> TargetCfg {
        self.target_cfg_with_host_fallback(None)
    }

    /// Create a TargetCfg, using `host_platform` as a fallback for `target_platforms`.
    /// In Bazel, `--host_platform` is used as the default target platform when
    /// `--platforms` is not explicitly set.
    pub fn target_cfg_with_host_fallback(&self, host_platform: Option<&str>) -> TargetCfg {
        let target_platform = self
            .target_platforms
            .clone()
            .or_else(|| host_platform.map(|s| s.to_owned()))
            .unwrap_or_default();
        TargetCfg {
            target_platform,
            cli_modifiers: self.cli_modifiers(),
        }
    }

    fn cli_modifiers(&self) -> Vec<String> {
        self.cli_modifier.clone()
    }
}

#[derive(Debug, clap::Parser, serde::Serialize, serde::Deserialize, Default)]
#[clap(next_help_heading = HELP_HEADING)]
pub struct TargetCfgWithUniverseOptions {
    /// Comma separated list of targets to construct a configured target universe.
    ///
    /// When the option is specified, command targets are be resolved in this universe.
    /// Additionally, `--target-platforms=` and `--modifier=` flags are be used to configure the
    /// universe targets, not the command targets.
    ///
    /// This argument is particularly recommended on most non-trivial cqueries. In the absence of
    /// this argument, kuro will use the target literals in your cquery expression as the value
    /// for this argument, which may not be what you want.
    #[clap(long, short = 'u', use_value_delimiter = true, verbatim_doc_comment)]
    pub target_universe: Vec<String>,

    #[clap(flatten)]
    pub target_cfg: TargetCfgOptions,
}

#[cfg(test)]
mod tests {
    use assert_matches::assert_matches;
    use clap::CommandFactory;
    use clap::Parser;

    use super::*;

    fn parse(args: &[&str]) -> kuro_error::Result<TargetCfgOptions> {
        Ok(TargetCfgOptions::try_parse_from(
            std::iter::once("program").chain(args.iter().copied()),
        )?)
    }

    #[test]
    fn opt_multiple() -> kuro_error::Result<()> {
        let opts = parse(&["--modifier", "value1", "--modifier", "value2"])?;

        assert_eq!(opts.cli_modifiers(), vec!["value1", "value2"]);

        Ok(())
    }

    #[test]
    fn space_separated_fails() -> kuro_error::Result<()> {
        assert_matches!(parse(&["--modifier", "value1", "value2"]), Err(..));

        Ok(())
    }

    #[test]
    fn test_target_cfg_unused() {
        #[derive(Debug, Eq, PartialEq)]
        struct ReducedArg {
            name: String,
            long: Option<String>,
            value_delimiter: Option<char>,
            number_of_values: Option<clap::builder::ValueRange>,
        }

        fn args<C: CommandFactory>() -> Vec<ReducedArg> {
            C::command()
                .get_arguments()
                .map(|a| ReducedArg {
                    name: a.get_id().as_str().to_owned(),
                    long: a.get_long().map(|s| s.to_owned()),
                    value_delimiter: a.get_value_delimiter(),
                    number_of_values: a.get_num_args(),
                })
                .collect()
        }

        let a = args::<TargetCfgOptions>();
        let b = args::<TargetCfgUnusedOptions>();

        assert_eq!(a, b);
        assert!(!a.is_empty());
    }
}
