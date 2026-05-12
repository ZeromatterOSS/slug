/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use slug_common::legacy_configs::configs::LegacyBuckConfig;
use slug_execute_impl::executors::local::ForkserverAccess;
use slug_fs::paths::abs_norm_path::AbsNormPath;
use slug_resource_control::buck_cgroup_tree::BuckCgroupTree;

#[cfg(unix)]
pub async fn maybe_launch_forkserver(
    _root_config: &LegacyBuckConfig,
    forkserver_state_dir: &AbsNormPath,
    cgroup_tree: Option<&BuckCgroupTree>,
) -> slug_error::Result<ForkserverAccess> {
    use slug_error::BuckErrorContext;

    let exe = std::env::current_exe().buck_error_context("Cannot access current_exe")?;
    Ok(ForkserverAccess::Client(
        slug_forkserver::launch::launch_forkserver(
            exe,
            &["forkserver"],
            forkserver_state_dir,
            cgroup_tree,
        )
        .await?,
    ))
}

#[cfg(not(unix))]
pub async fn maybe_launch_forkserver(
    _root_config: &LegacyBuckConfig,
    _forkserver_state_dir: &AbsNormPath,
    _cgroup_tree: Option<&BuckCgroupTree>,
) -> slug_error::Result<ForkserverAccess> {
    Ok(ForkserverAccess::None)
}
