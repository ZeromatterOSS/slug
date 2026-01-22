/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use kuro_error::BuckErrorContext;
use kuro_grpc::DuplexChannel;
use kuro_test_api::grpc::TestOrchestratorClient;
use kuro_test_api::grpc::spawn_executor_server;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;

use crate::executor::KuroTestExecutor;
use crate::runner::KuroTestRunner;

pub async fn run<OC, ER, EW>(
    orchestrator_channel: OC,
    executor_channel: DuplexChannel<ER, EW>,
    args: Vec<String>,
) -> kuro_error::Result<()>
where
    OC: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    ER: AsyncRead + Send + Unpin + 'static,
    EW: AsyncWrite + Send + Unpin + 'static,
{
    let (spec_sender, spec_receiver) = futures::channel::mpsc::unbounded();

    let executor_server =
        spawn_executor_server(executor_channel, KuroTestExecutor::new(spec_sender));

    let orchestrator_client = TestOrchestratorClient::new(orchestrator_channel)
        .await
        .buck_error_context("Failed to TestOrchestratorClient")?;

    let runner = KuroTestRunner::new(orchestrator_client, spec_receiver, args)?;

    runner.run_all_tests().await?;

    executor_server
        .shutdown()
        .await
        .buck_error_context("Failed to shutdown server")?;

    Ok(())
}
