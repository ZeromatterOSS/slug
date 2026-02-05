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
use std::pin::Pin;

use dice::DiceComputations;
use kuro_core::cells::CellResolver;
use kuro_core::fs::project_rel_path::ProjectRelativePath;
use kuro_core::global_cfg_options::GlobalCfgOptions;
use kuro_core::target::label::label::TargetLabel;
use kuro_util::late_binding::LateBinding;

use crate::actions::query::ActionQueryNode;

/// The result of audit output.
pub enum AuditOutputResult {
    /// The exact action that matched the buck-out path.
    Match(ActionQueryNode),
    /// If the platform configuration of the buck-out path doesn't match the platform used when calling
    /// audit output, then we return the unconfigured target label.
    MaybeRelevantForConfigurationHashPath(TargetLabel),
    /// If we were given a content-based path, we cannot tell which configured node(s) it came from,
    /// so just return the unconfigured target label.
    MatchContentBasedPath(TargetLabel),
}

pub static AUDIT_OUTPUT: LateBinding<
    for<'v> fn(
        &'v str,
        &'v ProjectRelativePath,
        &'v CellResolver,
        &'v mut DiceComputations,
        &'v GlobalCfgOptions,
    )
        -> Pin<Box<dyn Future<Output = kuro_error::Result<Option<AuditOutputResult>>> + 'v>>,
> = LateBinding::new("AUDIT_OUTPUT");

pub async fn audit_output<'v>(
    output_path: &'v str,
    working_dir: &'v ProjectRelativePath,
    cell_resolver: &'v CellResolver,
    dice_ctx: &'v mut DiceComputations<'_>,
    global_cfg_options: &'v GlobalCfgOptions,
) -> kuro_error::Result<Option<AuditOutputResult>> {
    (AUDIT_OUTPUT.get()?)(
        output_path,
        working_dir,
        cell_resolver,
        dice_ctx,
        global_cfg_options,
    )
    .await
}
