/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::collections::BTreeMap;

use dice::DiceTransaction;
use dupe::Dupe;
use kuro_cli_proto::new_generic::ExpandExternalCellsRequest;
use kuro_cli_proto::new_generic::ExpandExternalCellsResponse;
use kuro_common::dice::cells::HasCellResolver;
use kuro_common::external_cells::EXTERNAL_CELLS_IMPL;
use kuro_core::cells::name::CellName;
use kuro_server_ctx::ctx::ServerCommandContextTrait;
use kuro_server_ctx::partial_result_dispatcher::NoPartialResult;
use kuro_server_ctx::partial_result_dispatcher::PartialResultDispatcher;
use kuro_server_ctx::template::ServerCommandTemplate;
use kuro_server_ctx::template::run_server_command;

pub(crate) async fn expand_external_cells_command(
    ctx: &dyn ServerCommandContextTrait,
    partial_result_dispatcher: PartialResultDispatcher<NoPartialResult>,
    req: ExpandExternalCellsRequest,
) -> kuro_error::Result<ExpandExternalCellsResponse> {
    run_server_command(
        ExpandExternalCellsServerCommand { req },
        ctx,
        partial_result_dispatcher,
    )
    .await
}

struct ExpandExternalCellsServerCommand {
    req: ExpandExternalCellsRequest,
}

#[derive(kuro_error::Error, Debug)]
#[kuro(tag = Input)]
enum ExpandExternalCellError {
    #[error("Cell `{0}` is not an external cell")]
    CellNotExternal(CellName),
}

#[async_trait::async_trait]
impl ServerCommandTemplate for ExpandExternalCellsServerCommand {
    type StartEvent = kuro_data::ExpandExternalCellsCommandStart;
    type EndEvent = kuro_data::ExpandExternalCellsCommandEnd;
    type Response = kuro_cli_proto::new_generic::ExpandExternalCellsResponse;
    type PartialResult = NoPartialResult;

    async fn command(
        &self,
        server_ctx: &dyn ServerCommandContextTrait,
        _partial_result_dispatcher: PartialResultDispatcher<Self::PartialResult>,
        mut ctx: DiceTransaction,
    ) -> kuro_error::Result<Self::Response> {
        let cell_resolver = ctx.get_cell_resolver().await?;
        let cell_alias_resolver = ctx
            .get_cell_alias_resolver_for_dir(server_ctx.working_dir())
            .await?;

        let cell_aliases: Vec<String> = match &self.req {
            ExpandExternalCellsRequest::All => cell_resolver
                .cells()
                .filter_map(|(cell, instance)| {
                    if instance.external().is_some() {
                        Some(cell.as_str().to_owned())
                    } else {
                        None
                    }
                })
                .collect(),
            ExpandExternalCellsRequest::Specific(cells) => cells.iter().cloned().collect(),
        };
        let mut cell_to_path: BTreeMap<CellName, String> = BTreeMap::new();
        let mut cell_alias_to_path: BTreeMap<String, String> = BTreeMap::new();

        for cell_alias in cell_aliases.into_iter() {
            let cell = cell_alias_resolver.resolve(&cell_alias)?;

            if let Some(path) = cell_to_path.get(&cell) {
                cell_alias_to_path.insert(cell_alias, path.clone());
                continue;
            }

            let instance = cell_resolver.get(cell)?;
            let Some(origin) = instance.external() else {
                return Err(ExpandExternalCellError::CellNotExternal(cell).into());
            };
            EXTERNAL_CELLS_IMPL
                .get()?
                .expand(&mut ctx, cell, origin.dupe(), instance.path())
                .await?;

            let path = instance.path().to_string();
            cell_to_path.insert(cell, path.clone());
            cell_alias_to_path.insert(cell_alias, path);
        }

        Ok(ExpandExternalCellsResponse {
            paths: cell_alias_to_path,
        })
    }

    fn exclusive_command_name(&self) -> Option<String> {
        Some("expand-external-cell".to_owned())
    }
}
