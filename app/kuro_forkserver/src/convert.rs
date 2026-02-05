/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use futures::stream::Stream;
use futures::stream::StreamExt;
use kuro_common::convert::ProstDurationExt;
use kuro_error::BuckErrorContext;
use kuro_execute_local::CommandEvent;
use kuro_execute_local::GatherOutputStatus;

pub(crate) fn encode_event_stream<S>(
    s: S,
) -> impl Stream<Item = Result<kuro_forkserver_proto::CommandEvent, tonic::Status>>
where
    S: Stream<Item = kuro_error::Result<CommandEvent>>,
{
    fn convert_event(e: CommandEvent) -> kuro_forkserver_proto::CommandEvent {
        use kuro_forkserver_proto::command_event::Data;

        let data = match e {
            CommandEvent::Stdout(bytes) => Data::Stdout(kuro_forkserver_proto::StreamEvent {
                data: bytes.to_vec(),
            }),
            CommandEvent::Stderr(bytes) => Data::Stderr(kuro_forkserver_proto::StreamEvent {
                data: bytes.to_vec(),
            }),
            CommandEvent::Exit(GatherOutputStatus::Finished {
                exit_code,
                execution_stats,
            }) => Data::Exit(kuro_forkserver_proto::ExitEvent {
                exit_code,
                execution_stats: execution_stats.map(|s| {
                    kuro_forkserver_proto::CollectedExecutionStats {
                        cpu_instructions_user: s.cpu_instructions_user,
                        cpu_instructions_kernel: s.cpu_instructions_kernel,
                        userspace_events: s.userspace_events,
                        kernel_events: s.kernel_events,
                    }
                }),
            }),
            CommandEvent::Exit(GatherOutputStatus::TimedOut(duration)) => {
                Data::Timeout(kuro_forkserver_proto::TimeoutEvent {
                    duration: duration.try_into().ok(),
                })
            }
            CommandEvent::Exit(GatherOutputStatus::Cancelled) => {
                Data::Cancel(kuro_forkserver_proto::CancelEvent {})
            }
            CommandEvent::Exit(GatherOutputStatus::SpawnFailed(reason)) => {
                Data::SpawnFailed(kuro_forkserver_proto::SpawnFailedEvent { reason })
            }
        };

        kuro_forkserver_proto::CommandEvent { data: Some(data) }
    }

    fn convert_err(e: kuro_error::Error) -> tonic::Status {
        tonic::Status::unknown(format!("{e:#}"))
    }

    s.map(|r| r.map(convert_event).map_err(convert_err))
}

pub(crate) fn decode_event_stream<S>(s: S) -> impl Stream<Item = kuro_error::Result<CommandEvent>>
where
    S: Stream<Item = Result<kuro_forkserver_proto::CommandEvent, tonic::Status>>,
{
    fn convert_event(e: kuro_forkserver_proto::CommandEvent) -> kuro_error::Result<CommandEvent> {
        use kuro_forkserver_proto::command_event::Data;

        let event = match e.data.buck_error_context("Missing `data`")? {
            Data::Stdout(kuro_forkserver_proto::StreamEvent { data }) => {
                CommandEvent::Stdout(data.into())
            }
            Data::Stderr(kuro_forkserver_proto::StreamEvent { data }) => {
                CommandEvent::Stderr(data.into())
            }
            Data::Exit(kuro_forkserver_proto::ExitEvent {
                exit_code,
                execution_stats,
            }) => CommandEvent::Exit(GatherOutputStatus::Finished {
                exit_code,
                execution_stats: execution_stats.map(|s| {
                    kuro_execute_local::CollectedExecutionStats {
                        cpu_instructions_user: s.cpu_instructions_user,
                        cpu_instructions_kernel: s.cpu_instructions_kernel,
                        userspace_events: s.userspace_events,
                        kernel_events: s.kernel_events,
                    }
                }),
            }),
            Data::Timeout(kuro_forkserver_proto::TimeoutEvent { duration }) => {
                CommandEvent::Exit(GatherOutputStatus::TimedOut(
                    duration
                        .buck_error_context("Missing `duration`")?
                        .try_into_duration()
                        .buck_error_context("Invalid `duration`")?,
                ))
            }
            Data::Cancel(kuro_forkserver_proto::CancelEvent {}) => {
                CommandEvent::Exit(GatherOutputStatus::Cancelled)
            }
            Data::SpawnFailed(kuro_forkserver_proto::SpawnFailedEvent { reason }) => {
                CommandEvent::Exit(GatherOutputStatus::SpawnFailed(reason))
            }
        };

        Ok(event)
    }

    fn convert_err(e: tonic::Status) -> kuro_error::Error {
        kuro_error::kuro_error!(
            kuro_error::ErrorTag::Tier0,
            "forkserver error: {}",
            e.message()
        )
    }

    s.map(|r| r.map_err(convert_err).and_then(convert_event))
}
