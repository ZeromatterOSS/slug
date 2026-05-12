/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::io::Write;

use async_trait::async_trait;
use slug_cli_proto::ClientContext;
use slug_cmd_audit_client::prelude::AuditPreludeCommand;
use slug_interpreter::load_module::INTERPRETER_CALCULATION_IMPL;
use slug_server_ctx::ctx::ServerCommandContextTrait;
use slug_server_ctx::ctx::ServerCommandDiceContext;
use slug_server_ctx::partial_result_dispatcher::PartialResultDispatcher;

use crate::ServerAuditSubcommand;

#[async_trait]
impl ServerAuditSubcommand for AuditPreludeCommand {
    async fn server_execute(
        &self,
        server_ctx: &dyn ServerCommandContextTrait,
        mut stdout: PartialResultDispatcher<slug_cli_proto::StdoutBytes>,
        _client_ctx: ClientContext,
    ) -> slug_error::Result<()> {
        // Prints the top-level globals env. The prelude-env half of
        // this audit command is gone; the prelude pipeline no longer
        // exists.
        Ok(server_ctx
            .with_dice_ctx(|_server_ctx, mut ctx| async move {
                let mut stdout = stdout.as_writer();
                writeln!(
                    stdout,
                    "{}",
                    INTERPRETER_CALCULATION_IMPL
                        .get()?
                        .global_env(&mut ctx)
                        .await?
                        .describe()
                )?;

                Ok(())
            })
            .await?)
    }
}
