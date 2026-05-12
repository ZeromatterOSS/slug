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
use slug_build_api::audit_dep_files::AUDIT_DEP_FILES;
use slug_cli_proto::ClientContext;
use slug_cmd_audit_client::dep_files::AuditDepFilesCommand;
use slug_common::pattern::parse_from_cli::parse_patterns_from_cli_args;
use slug_core::category::CategoryRef;
use slug_core::pattern::pattern_type::TargetPatternExtra;
use slug_error::BuckErrorContext;
use slug_node::target_calculation::ConfiguredTargetCalculation;
use slug_server_ctx::ctx::ServerCommandContextTrait;
use slug_server_ctx::ctx::ServerCommandDiceContext;
use slug_server_ctx::global_cfg_options::global_cfg_options_from_client_context;
use slug_server_ctx::partial_result_dispatcher::PartialResultDispatcher;

use crate::ServerAuditSubcommand;

#[async_trait]
impl ServerAuditSubcommand for AuditDepFilesCommand {
    async fn server_execute(
        &self,
        server_ctx: &dyn ServerCommandContextTrait,
        mut stdout: PartialResultDispatcher<slug_cli_proto::StdoutBytes>,
        _client_ctx: ClientContext,
    ) -> slug_error::Result<()> {
        Ok(server_ctx
            .with_dice_ctx(|server_ctx, mut ctx| async move {
                let global_cfg_options = global_cfg_options_from_client_context(
                    &self.target_cfg.target_cfg(),
                    server_ctx,
                    &mut ctx,
                )
                .await?;

                let label = parse_patterns_from_cli_args::<TargetPatternExtra>(
                    &mut ctx,
                    std::slice::from_ref(&self.pattern),
                    server_ctx.working_dir(),
                )
                .await?
                .into_iter()
                .next()
                .buck_error_context("Parsing patterns returned nothing")?
                .as_target_label(&self.pattern)?;

                let label = ctx
                    .get_configured_target_post_transition(&label, &global_cfg_options)
                    .await?;

                let category = CategoryRef::new(self.category.as_str())?.to_owned();

                (AUDIT_DEP_FILES.get()?)(
                    &ctx,
                    label,
                    category,
                    self.identifier.clone(),
                    &mut stdout.as_writer(),
                )
                .await?;

                Ok(())
            })
            .await?)
    }
}
