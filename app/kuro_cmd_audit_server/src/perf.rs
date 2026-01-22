/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Starlark debugging.

mod configured_graph_size;

use async_trait::async_trait;
use kuro_cli_proto::ClientContext;
use kuro_cmd_audit_client::perf::AuditPerfCommand;
use kuro_server_ctx::ctx::ServerCommandContextTrait;
use kuro_server_ctx::partial_result_dispatcher::PartialResultDispatcher;

use crate::ServerAuditSubcommand;

#[async_trait]
impl ServerAuditSubcommand for AuditPerfCommand {
    async fn server_execute(
        &self,
        server_ctx: &dyn ServerCommandContextTrait,
        stdout: PartialResultDispatcher<kuro_cli_proto::StdoutBytes>,
        client_ctx: ClientContext,
    ) -> kuro_error::Result<()> {
        match self {
            AuditPerfCommand::ConfiguredGraphSize(cmd) => {
                configured_graph_size::server_execute(cmd, server_ctx, stdout, client_ctx).await
            }
        }
    }
}
