/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::time::Duration;

use futures::future::FutureExt;
use gazebo::prelude::*;
use kuro_error::BuckErrorContext;
use kuro_events::dispatch::span_async;
use kuro_server_ctx::commands::command_end;
use kuro_server_ctx::ctx::ServerCommandContextTrait;
use kuro_server_ctx::partial_result_dispatcher::PartialResultDispatcher;
use kuro_server_ctx::streaming_request_handler::StreamingRequestHandler;
use tokio::time::MissedTickBehavior;

use crate::active_commands;

pub(crate) async fn run_subscription_server_command(
    ctx: &dyn ServerCommandContextTrait,
    mut partial_result_dispatcher: PartialResultDispatcher<
        kuro_cli_proto::SubscriptionResponseWrapper,
    >,
    mut req: StreamingRequestHandler<kuro_cli_proto::SubscriptionRequestWrapper>,
) -> kuro_error::Result<kuro_cli_proto::SubscriptionCommandResponse> {
    let start_event = ctx
        .command_start_event(kuro_data::SubscriptionCommandStart {}.into())
        .await?;
    span_async(start_event, async move {
        let result: kuro_error::Result<kuro_cli_proto::SubscriptionCommandResponse> = try {
            // NOTE: Long term if we expose more things here then we should probably move this error to
            // only occur when we try to actually interact with materializer subscriptioons
            let materializer = ctx
                .materializer();

            let materializer = materializer
                .as_deferred_materializer_extension()
                .buck_error_context("Subscriptions only work with the deferred materializer")?;

            let mut materializer_subscription = materializer
                .create_subscription()
                .await
                .buck_error_context("Error creating a materializer subscription")?;

            let mut wants_active_commands = false;

            let mut ticker = tokio::time::interval(Duration::from_millis(100));
            ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

            let disconnect = loop {
                futures::select! {
                    message = req.message().fuse() => {
                        use kuro_subscription_proto::subscription_request::Request;

                        match message?.request.buck_error_context("Empty message").tag(kuro_error::ErrorTag::Input)?.request.buck_error_context("Empty request").tag(kuro_error::ErrorTag::Input)? {
                            Request::Disconnect(disconnect) => {
                                break disconnect;
                            }
                            Request::SubscribeToPaths(kuro_subscription_proto::SubscribeToPaths { paths }) => {
                                let paths = paths.into_try_map(|path| path.try_into())?;
                                materializer_subscription.subscribe_to_paths(paths);
                            }
                            Request::UnsubscribeFromPaths(kuro_subscription_proto::UnsubscribeFromPaths { paths }) => {
                                let paths = paths.into_try_map(|path| path.try_into())?;
                                materializer_subscription.unsubscribe_from_paths(paths);
                            }
                            Request::SubscribeToActiveCommands(kuro_subscription_proto::SubscribeToActiveCommands {}) => {
                                wants_active_commands = true;
                            }
                        }
                    }
                    path = materializer_subscription.next_materialization().fuse() => {
                        let path = path.buck_error_context("Materializer hung up")?;
                        partial_result_dispatcher.emit(kuro_cli_proto::SubscriptionResponseWrapper {
                            response: Some(kuro_subscription_proto::SubscriptionResponse {
                                response: Some(kuro_subscription_proto::Materialized { path: path.to_string() }.into())
                            })
                        });
                    }
                    _ = ticker.tick().fuse() => {
                        if wants_active_commands {
                            let snapshot = active_commands_snapshot();
                            partial_result_dispatcher.emit(kuro_cli_proto::SubscriptionResponseWrapper {
                                response: Some(kuro_subscription_proto::SubscriptionResponse {
                                    response: Some(snapshot.into())
                                })
                            });
                        }
                    }
                }
            };

            partial_result_dispatcher.emit(kuro_cli_proto::SubscriptionResponseWrapper {
                response: Some(kuro_subscription_proto::SubscriptionResponse {
                    response: Some(kuro_subscription_proto::Goodbye {
                        reason: disconnect.reason,
                        ok: disconnect.ok,
                    }.into())
                })
            });

            kuro_cli_proto::SubscriptionCommandResponse {}
        };
        let result = result.map_err(Into::into);

        let end_event = command_end(&result, kuro_data::SubscriptionCommandEnd {});
        (result.map_err(Into::into), end_event)
    })
    .await
}

fn active_commands_snapshot() -> kuro_subscription_proto::ActiveCommandsSnapshot {
    let active_commands = active_commands::active_commands()
        .iter()
        .map(|(trace_id, handle)| {
            let state = handle.state();
            let spans = state.spans();

            kuro_subscription_proto::ActiveCommand {
                trace_id: trace_id.to_string(),
                argv: state.argv.clone(),
                stats: Some(kuro_subscription_proto::ActiveCommandStats {
                    open_spans: spans.open,
                    closed_spans: spans.closed,
                    pending_spans: spans.pending,
                }),
            }
        })
        .collect();

    kuro_subscription_proto::ActiveCommandsSnapshot { active_commands }
}
