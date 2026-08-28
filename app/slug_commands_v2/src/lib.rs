/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

pub mod aquery;
pub mod build;
pub mod common;
pub mod cquery;
pub mod query;
pub mod run;
pub mod test;

pub use common::CommandKind;
pub use common::CommandParseError;
pub use common::CommandPlaceholderError;
pub use common::FlagDisposition;
pub use common::ParsedFlag;
pub use common::QueryOutputFormat;
pub use common::normalize_bzlmod_environment_value;
pub use slug_configuration_v2::CommandConfigurationOccurrence;
pub use slug_configuration_v2::CommandConfigurationOverlay;

pub const HELP_SUMMARY: &str = "\
Slug V2 commands:\n\
  build <target-pattern>...\n\
  test <target-pattern>...\n\
  run <target> [-- <args>...]\n\
  query <expr>\n\
  cquery <expr>\n\
  aquery <expr>\n\
";
