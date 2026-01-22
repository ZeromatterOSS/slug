/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use kuro_client_ctx::daemon::client::BuckdLifecycleLock;
use kuro_client_ctx::daemon::client::connect::buckd_startup_timeout;
use kuro_client_ctx::daemon::client::kill::kill_command_impl;
use kuro_client_ctx::startup_deadline::StartupDeadline;
use kuro_common::init::DaemonStartupConfig;
use kuro_common::invocation_paths::InvocationPaths;
use kuro_core::logging::LogConfigurationReloadHandle;
use kuro_error::kuro_error;
use kuro_util::threads::thread_spawn;

use crate::daemon::DaemonCommand;

pub fn start_in_process_daemon(
    daemon_startup_config: &DaemonStartupConfig,
    paths: InvocationPaths,
    runtime: &tokio::runtime::Runtime,
) -> kuro_error::Result<Option<Box<dyn FnOnce() -> kuro_error::Result<()> + Send + Sync>>> {
    let daemon_dir = paths.daemon_dir()?;
    // Using --no-buckd must kill the existing daemon if there is one running.
    // This adds a few extra prints to stderr for killing the daemon, but that should be
    // OK given that --no-buckd should only be used for testing purposes.
    runtime.block_on(async move {
        let lifecycle_lock = BuckdLifecycleLock::lock_with_timeout(
            daemon_dir,
            StartupDeadline::duration_from_now(buckd_startup_timeout()?)?,
        )
        .await?;

        kill_command_impl(&lifecycle_lock, "A command with `--no-buckd` is invoked").await
    })?;

    let daemon_startup_config = daemon_startup_config.clone();
    // Create a function which spawns an in-process daemon.
    Ok(Some(Box::new(move || {
        let (tx, rx) = std::sync::mpsc::channel();
        // Spawn a thread which runs the daemon.
        thread_spawn("kuro-no-buckd", move || {
            let tx_clone = tx.clone();
            let result = DaemonCommand::new_in_process(daemon_startup_config).exec(
                <dyn LogConfigurationReloadHandle>::noop(),
                paths,
                true,
                move || drop(tx_clone.send(Ok(()))),
            );
            // Since `tx` is unbounded, there's race here: it is possible
            // that error message will be lost in the channel and not reported anywhere.
            // Not an issue practically, because daemon does not usually error
            // after it started listening.
            if let Err(e) = tx.send(result) {
                match e.0 {
                    Ok(()) => drop(kuro_client_ctx::eprintln!(
                        "In-process daemon gracefully stopped"
                    )),
                    Err(e) => drop(kuro_client_ctx::eprintln!(
                        "In-process daemon run failed: {:#}",
                        e
                    )),
                }
            }
        })?;
        // Wait for listener to start (or to fail).
        match rx.recv() {
            Ok(r) => r,
            Err(_) => Err(kuro_error!(
                kuro_error::ErrorTag::Tier0,
                "In-process daemon failed to start and we don't know why"
            )),
        }
    })))
}
