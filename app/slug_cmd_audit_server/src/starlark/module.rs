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

use slug_cli_proto::ClientContext;
use slug_cmd_audit_client::starlark::module::StarlarkModuleCommand;
use slug_common::dice::cells::HasCellResolver;
use slug_core::cells::build_file_cell::BuildFileCell;
use slug_core::cells::cell_path_with_allowed_relative_dir::CellPathWithAllowedRelativeDir;
use slug_interpreter::load_module::InterpreterCalculation;
use slug_interpreter::parse_import::ParseImportOptions;
use slug_interpreter::parse_import::RelativeImports;
use slug_interpreter::parse_import::parse_bzl_path_with_config;
use slug_interpreter::paths::module::StarlarkModulePath;
use slug_server_ctx::ctx::ServerCommandContextTrait;
use slug_server_ctx::ctx::ServerCommandDiceContext;
use slug_server_ctx::partial_result_dispatcher::PartialResultDispatcher;

pub(crate) async fn server_execute(
    command: &StarlarkModuleCommand,
    server_ctx: &dyn ServerCommandContextTrait,
    mut stdout: PartialResultDispatcher<slug_cli_proto::StdoutBytes>,
    _client_ctx: ClientContext,
) -> slug_error::Result<()> {
    server_ctx
        .with_dice_ctx(|server_ctx, mut dice_ctx| async move {
            let cell_resolver = dice_ctx.get_cell_resolver().await?;
            let cwd = server_ctx.working_dir();
            let current_cell_path = cell_resolver.get_cell_path(cwd);
            let current_cell = BuildFileCell::new(current_cell_path.cell());
            let cell_alias_resolver = dice_ctx
                .get_cell_alias_resolver(current_cell_path.cell())
                .await?;

            let import_path = parse_bzl_path_with_config(
                &cell_alias_resolver,
                &command.import_path,
                &ParseImportOptions {
                    relative_import_option: RelativeImports::Allow {
                        current_dir_with_allowed_relative: &CellPathWithAllowedRelativeDir::new(
                            current_cell_path,
                            None,
                        ),
                        package_dir: None,
                    },
                    // Otherwise `@arg` is expanded as mode file.
                    allow_missing_at_symbol: true,
                },
                current_cell,
            )?;

            let loaded_module = dice_ctx
                .get_loaded_module(StarlarkModulePath::LoadFile(&import_path))
                .await?;

            let mut stdout = stdout.as_writer();
            writeln!(stdout, "{}", loaded_module.path())?;
            writeln!(stdout)?;
            writeln!(stdout, "Imports:")?;
            for import in loaded_module.imports() {
                writeln!(stdout, "  {import}")?;
            }
            writeln!(stdout)?;
            write!(stdout, "{}", loaded_module.env().dump_debug())?;
            Ok(())
        })
        .await
}
