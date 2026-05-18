/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory.
 */

use std::io::Write;

use async_trait::async_trait;
use slug_bzlmod::bzlmod_event_counters;
use slug_cli_proto::ClientContext;
use slug_cmd_audit_client::bzlmod_counters::AuditBzlmodCountersCommand;
use slug_server_ctx::ctx::ServerCommandContextTrait;
use slug_server_ctx::partial_result_dispatcher::PartialResultDispatcher;

use crate::ServerAuditSubcommand;

#[async_trait]
impl ServerAuditSubcommand for AuditBzlmodCountersCommand {
    async fn server_execute(
        &self,
        _server_ctx: &dyn ServerCommandContextTrait,
        mut stdout: PartialResultDispatcher<slug_cli_proto::StdoutBytes>,
        _client_ctx: ClientContext,
    ) -> slug_error::Result<()> {
        let mut stdout = stdout.as_writer();
        serde_json::to_writer_pretty(&mut stdout, &bzlmod_event_counters())?;
        writeln!(stdout)?;
        Ok(())
    }
}
