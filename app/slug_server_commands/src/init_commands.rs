/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use async_trait::async_trait;
use slug_cli_proto::new_generic::CompleteRequest;
use slug_cli_proto::new_generic::CompleteResponse;
use slug_cli_proto::new_generic::DebugEvalRequest;
use slug_cli_proto::new_generic::DebugEvalResponse;
use slug_cli_proto::new_generic::ExpandExternalCellsRequest;
use slug_cli_proto::new_generic::ExpandExternalCellsResponse;
use slug_cli_proto::new_generic::ExplainRequest;
use slug_cli_proto::new_generic::ExplainResponse;
use slug_server_ctx::ctx::ServerCommandContextTrait;
use slug_server_ctx::late_bindings::OTHER_SERVER_COMMANDS;
use slug_server_ctx::late_bindings::OtherServerCommands;
use slug_server_ctx::partial_result_dispatcher::NoPartialResult;
use slug_server_ctx::partial_result_dispatcher::PartialResultDispatcher;

use crate::build::build_command;
use crate::complete::complete_command;
use crate::debug_eval::debug_eval_command;
use crate::expand_external_cells::expand_external_cells_command;
use crate::explain::explain_command;
use crate::install::install_command;

struct OtherServerCommandsInstance;

#[async_trait]
impl OtherServerCommands for OtherServerCommandsInstance {
    async fn build(
        &self,
        ctx: &dyn ServerCommandContextTrait,
        partial_result_dispatcher: PartialResultDispatcher<NoPartialResult>,
        req: slug_cli_proto::BuildRequest,
    ) -> slug_error::Result<slug_cli_proto::BuildResponse> {
        build_command(ctx, partial_result_dispatcher, req).await
    }
    async fn install(
        &self,
        ctx: &dyn ServerCommandContextTrait,
        partial_result_dispatcher: PartialResultDispatcher<NoPartialResult>,
        req: slug_cli_proto::InstallRequest,
    ) -> slug_error::Result<slug_cli_proto::InstallResponse> {
        install_command(ctx, partial_result_dispatcher, req).await
    }
    async fn complete(
        &self,
        ctx: &dyn ServerCommandContextTrait,
        partial_result_dispatcher: PartialResultDispatcher<NoPartialResult>,
        req: CompleteRequest,
    ) -> slug_error::Result<CompleteResponse> {
        complete_command(ctx, partial_result_dispatcher, req).await
    }

    async fn debug_eval(
        &self,
        ctx: &dyn ServerCommandContextTrait,
        req: DebugEvalRequest,
    ) -> slug_error::Result<DebugEvalResponse> {
        debug_eval_command(ctx, req).await
    }

    async fn explain(
        &self,
        ctx: &dyn ServerCommandContextTrait,
        partial_result_dispatcher: PartialResultDispatcher<NoPartialResult>,
        req: ExplainRequest,
    ) -> slug_error::Result<ExplainResponse> {
        explain_command(ctx, partial_result_dispatcher, req).await
    }

    async fn expand_external_cells(
        &self,
        ctx: &dyn ServerCommandContextTrait,
        partial_result_dispatcher: PartialResultDispatcher<NoPartialResult>,
        req: ExpandExternalCellsRequest,
    ) -> slug_error::Result<ExpandExternalCellsResponse> {
        expand_external_cells_command(ctx, partial_result_dispatcher, req).await
    }
}

pub(crate) fn init_other_server_commands() {
    OTHER_SERVER_COMMANDS.init(&OtherServerCommandsInstance);
}
