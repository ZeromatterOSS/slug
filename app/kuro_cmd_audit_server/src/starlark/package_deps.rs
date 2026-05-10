/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::collections::HashSet;
use std::io::Write;

use kuro_cli_proto::ClientContext;
use kuro_cmd_audit_client::starlark::package_deps::StarlarkPackageDepsCommand;
use kuro_common::dice::cells::HasCellResolver;
use kuro_core::pattern::parse_package::parse_package;
use kuro_error::kuro_error;
use kuro_interpreter::file_loader::LoadedModule;
use kuro_interpreter::load_module::INTERPRETER_CALCULATION_IMPL;
use kuro_interpreter::paths::module::StarlarkModulePath;
use kuro_server_ctx::ctx::ServerCommandContextTrait;
use kuro_server_ctx::ctx::ServerCommandDiceContext;
use kuro_server_ctx::partial_result_dispatcher::PartialResultDispatcher;

pub(crate) async fn server_execute(
    command: &StarlarkPackageDepsCommand,
    server_ctx: &dyn ServerCommandContextTrait,
    mut stdout: PartialResultDispatcher<kuro_cli_proto::StdoutBytes>,
    _client_ctx: ClientContext,
) -> kuro_error::Result<()> {
    server_ctx
        .with_dice_ctx(|server_ctx, mut dice_ctx| async move {
            let cell_resolver = dice_ctx.get_cell_resolver().await?;
            let cwd = server_ctx.working_dir();
            let current_cell_path = cell_resolver.get_cell_path(cwd);
            let cell_alias_resolver = dice_ctx
                .get_cell_alias_resolver(current_cell_path.cell())
                .await?;

            let package = parse_package(&command.package, &cell_alias_resolver)?;

            let module_deps = INTERPRETER_CALCULATION_IMPL
                .get()?
                .get_module_deps(&mut dice_ctx, package)
                .await?;

            let mut stdout = stdout.as_writer();
            let mut visited = HashSet::new();
            let mut ordered_modules = Vec::new();
            let mut stack = module_deps
                .0
                .into_iter()
                .map(|module| (module, false))
                .collect::<Vec<_>>();

            while let Some((module, expanded)) = stack.pop() {
                let path = match module.path() {
                    StarlarkModulePath::LoadFile(path)
                    | StarlarkModulePath::JsonFile(path)
                    | StarlarkModulePath::TomlFile(path) => path,
                    StarlarkModulePath::BxlFile(_) => {
                        return Err(kuro_error!(kuro_error::ErrorTag::Tier0, "bxl be here"));
                    }
                };

                if expanded {
                    ordered_modules.push(module);
                    continue;
                }

                if !visited.insert(path.clone()) {
                    continue;
                }

                stack.push((module.clone(), true));
                for import in module.direct_imports().iter().rev() {
                    let import_module = INTERPRETER_CALCULATION_IMPL
                        .get()?
                        .get_loaded_module(&mut dice_ctx, import.borrow())
                        .await?;
                    stack.push((import_module, false));
                }
            }

            struct Printer {
                first: bool,
            }

            impl Printer {
                fn print_module(
                    &mut self,
                    module: &LoadedModule,
                    stdout: &mut dyn Write,
                ) -> kuro_error::Result<()> {
                    let path = match module.path() {
                        StarlarkModulePath::LoadFile(path)
                        | StarlarkModulePath::JsonFile(path)
                        | StarlarkModulePath::TomlFile(path) => path,
                        StarlarkModulePath::BxlFile(_) => {
                            return Err(kuro_error!(kuro_error::ErrorTag::Tier0, "bxl be here"));
                        }
                    };

                    if !self.first {
                        writeln!(stdout)?;
                        writeln!(stdout)?;
                    }
                    self.first = false;

                    writeln!(stdout, "# {path}")?;
                    writeln!(stdout)?;
                    write!(stdout, "{}", module.env().dump_debug())?;

                    Ok(())
                }
            }

            let mut printer = Printer { first: true };

            for module in ordered_modules {
                printer.print_module(&module, &mut stdout)?;
            }

            Ok(())
        })
        .await
}
