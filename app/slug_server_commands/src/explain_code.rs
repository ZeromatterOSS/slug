/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use core::iter::Iterator;
use dice::DiceTransaction;
use dupe::Dupe;
use dupe::IterDupedExt;
use slug_build_api::query::oneshot::QUERY_FRONTEND;
use slug_cli_proto::new_generic::ExplainRequest;
use slug_data::CommandInvalidationInfo;
use slug_data::action_key;
use slug_event_log::read::EventLogPathBuf;
use slug_event_log::stream_value::StreamValue;
use slug_event_observer::display::TargetDisplayOptions;
use slug_event_observer::display::display_anon_target;
use slug_event_observer::display::display_bxl_key;
use slug_event_observer::display::display_configured_target_label;
use slug_event_observer::projection::collect_buck_events;
use slug_event_observer::projection::project_actions_and_file_changes;
use slug_event_observer::what_ran::WhatRanOptions;
use slug_event_observer::what_ran::CommandReproducer;
use slug_event_observer::what_ran::WhatRanRelevantAction;
use slug_explain::ActionEntryData;
use slug_explain::ChangedFilesEntryData;
use slug_node::nodes::configured::ConfiguredTargetNode;
use slug_query::query::syntax::simple::eval::label_indexed::LabelIndexedSet;
use slug_server_ctx::ctx::ServerCommandContextTrait;
use slug_server_ctx::global_cfg_options::global_cfg_options_from_client_context;


fn format_projected_action(
    action: &WhatRanRelevantAction,
    reproducers: &[CommandReproducer],
    event: &slug_data::SpanEndEvent,
) -> slug_error::Result<Option<(String, ActionEntryData)>> {
    let action_execution = match &event.data {
        Some(slug_data::span_end_event::Data::ActionExecution(action_exec)) => action_exec,
        _ => return Ok(None),
    };

    let failed = action_execution.failed;
    let execution_kind = slug_data::ActionExecutionKind::try_from(action_execution.execution_kind)
        .ok()
        .map(|v| v.as_str_name().to_owned());
    let input_files_bytes = action_execution.input_files_bytes;
    let affected_by_file_changes = matches!(
        &action_execution.invalidation_info,
        Some(CommandInvalidationInfo {
            changed_file: Some(_),
            ..
        })
    );

    let (target, mut entry) = match action {
        WhatRanRelevantAction::ActionExecution(act) => {
            let category = act.name.as_ref().map(|n| n.category.clone());
            let identifier = act.name.as_ref().map(|n| n.identifier.clone());
            let owner = match act.key.as_ref() {
                Some(key) => key.owner.as_ref(),
                None => return Ok(None),
            };

            let opts = TargetDisplayOptions::for_log();
            let target = match owner {
                Some(o) => match o {
                    action_key::Owner::TargetLabel(target_label)
                    | action_key::Owner::TestTargetLabel(target_label)
                    | action_key::Owner::LocalResourceSetup(target_label) => {
                        display_configured_target_label(target_label, opts)
                    }
                    action_key::Owner::BxlKey(bxl_key) => display_bxl_key(bxl_key),
                    action_key::Owner::AnonTarget(anon_target) => display_anon_target(anon_target),
                }?,
                None => return Ok(None),
            };

            (
                target,
                ActionEntryData {
                    category,
                    failed,
                    repros: vec![],
                    execution_kind,
                    identifier,
                    input_files_bytes,
                    affected_by_file_changes,
                },
            )
        }
        _ => return Ok(None),
    };

    entry.repros = reproducers.iter().map(ToString::to_string).collect();

    Ok(Some((target, entry)))
}

pub(crate) async fn explain(
    server_ctx: &dyn ServerCommandContextTrait,
    mut ctx: DiceTransaction,
    req: &ExplainRequest,
) -> slug_error::Result<()> {
    let build_log = EventLogPathBuf::infer(req.log_path.clone())?;
    let (_, mut events) = build_log.unpack_stream().await?;

    let options = WhatRanOptions {
        skip_cache_hits: true,
        emit_cache_queries: false,
        ..Default::default()
    };

    let build_events = collect_buck_events(events).await?;

    let projection = project_actions_and_file_changes(build_events.iter().map(|e| e.as_ref()), &options)?;

    let mut executed_actions = vec![];
    for action in projection.actions {
        let Some(span_end) = action.span_end else {
            continue;
        };
        if let Some(entry) = format_projected_action(&action.action, &action.reproducers, &span_end)? {
            executed_actions.push(entry);
        }
    }
    let changed_files = projection
        .changed_files
        .into_iter()
        .map(|event| event.path)
        .collect::<Vec<_>>();

    let target_universe: Option<&[String]> = if req.target_universe.is_empty() {
        None
    } else {
        Some(&req.target_universe)
    };

    let global_cfg_options =
        global_cfg_options_from_client_context(&req.target_cfg, server_ctx, &mut ctx).await?;

    let targets = {
        let (query_result, _universes) = QUERY_FRONTEND
            .get()?
            .eval_cquery(
                &mut ctx,
                server_ctx.working_dir(),
                &req.target,
                &[],
                global_cfg_options.dupe(),
                target_universe,
                false, // collect universes
            )
            .await?;

        query_result
            .targets()
            .map(|v| v.map(|v| v.dupe()))
            .collect::<Result<Vec<ConfiguredTargetNode>, _>>()?
    };

    let file_update_entries = {
        let mut file_update_entries = vec![];
        // TODO iguridi: one by one and serially is not very smart
        for file_change in changed_files {
            let (targets_with_file_updates, _universes) = QUERY_FRONTEND
                .get()?
                .eval_cquery(
                    &mut ctx,
                    server_ctx.working_dir(),
                    &format!("owner(\"{file_change}\")"),
                    &[],
                    global_cfg_options.dupe(),
                    Some(std::slice::from_ref(&req.target)), // target universe
                    false,
                )
                .await?;

            let targets = targets_with_file_updates
                .targets()
                .map(|v| v.map(|v| v.dupe()))
                .collect::<Result<Vec<ConfiguredTargetNode>, _>>()?;

            file_update_entries.push(ChangedFilesEntryData {
                path: file_change,
                targets: targets.into_iter().map(|t| t.label().to_string()).collect(),
            });
        }
        file_update_entries
    };

    let all_deps = {
        let mut stack = targets;
        let mut visited = LabelIndexedSet::new();
        while let Some(node) = stack.pop() {
            if visited.insert(node.dupe()) {
                stack.extend(node.deps().duped());
            }
        }
        visited.into_iter().collect::<Vec<ConfiguredTargetNode>>()
    };

    slug_explain::main(
        all_deps,
        executed_actions,
        file_update_entries,
        req.output.as_ref(),
        req.fbs_dump.as_ref(),
        req.manifold_path.as_deref(),
    )
    .await?;

    Ok(())
}
