/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::collections::HashMap;

use futures::TryStreamExt;
use futures::stream::Stream;
use slug_data::BuckEvent;
use slug_data::FileWatcherEvent;
use slug_events::span::SpanId;

use crate::what_ran::CommandReproducer;
use crate::what_ran::WhatRanOptions;
use crate::what_ran::WhatRanRelevantAction;

pub struct ProjectedAction {
    pub action: WhatRanRelevantAction,
    pub reproducers: Vec<CommandReproducer>,
    pub span_end: Option<slug_data::SpanEndEvent>,
}

#[derive(Default)]
pub struct EventProjection {
    pub actions: Vec<ProjectedAction>,
    pub unfinished_actions: Vec<ProjectedAction>,
    pub changed_files: Vec<FileWatcherEvent>,
}

pub async fn collect_buck_events(
    events: impl Stream<Item = slug_error::Result<slug_event_log::stream_value::StreamValue>>,
) -> slug_error::Result<Vec<Box<BuckEvent>>> {
    futures::pin_mut!(events);
    let mut out = Vec::new();
    while let Some(event) = events.try_next().await? {
        if let slug_event_log::stream_value::StreamValue::Event(event) = event {
            out.push(event);
        }
    }
    Ok(out)
}

pub fn project_actions_and_file_changes<'a>(
    events: impl IntoIterator<Item = &'a BuckEvent>,
    options: &WhatRanOptions,
) -> slug_error::Result<EventProjection> {
    let mut known_actions: HashMap<SpanId, (WhatRanRelevantAction, Vec<CommandReproducer>)> =
        HashMap::new();
    let mut projection = EventProjection::default();

    for event in events {
        if let Some(data) = &event.data {
            if let Some(action) = WhatRanRelevantAction::from_buck_data(data) {
                known_actions.insert(SpanId::from_u64(event.span_id)?, (action, Vec::new()));
            }

            if let Some(repro) = CommandReproducer::from_buck_data(data, options)
                && let Some(parent_id) = SpanId::from_u64_opt(event.parent_id)
                && let Some((_, reproducers)) = known_actions.get_mut(&parent_id)
            {
                reproducers.push(repro);
            }

            if let slug_data::buck_event::Data::SpanEnd(span) = data {
                if let Some((action, reproducers)) = known_actions.remove(&SpanId::from_u64(event.span_id)?) {
                    projection.actions.push(ProjectedAction {
                        action,
                        reproducers,
                        span_end: Some(span.clone()),
                    });
                }

                if let Some(slug_data::span_end_event::Data::FileWatcher(end)) = &span.data
                    && let Some(stats) = &end.stats
                {
                    projection.changed_files.extend(stats.events.clone());
                }
            }
        }
    }

    projection.unfinished_actions = known_actions
        .into_values()
        .map(|(action, reproducers)| ProjectedAction {
            action,
            reproducers,
            span_end: None,
        })
        .collect();

    Ok(projection)
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use slug_data::BuckEvent;
    use slug_data::SpanEndEvent;
    use slug_data::SpanStartEvent;
    use slug_data::span_end_event;
    use slug_data::span_start_event;
    use slug_event_log::stream_value::StreamValue;
    use slug_events::span::SpanId;
    use slug_wrapper_common::invocation_id::TraceId;

    use super::*;

    fn start_event(span_id: u64, parent_id: Option<u64>, data: span_start_event::Data) -> BuckEvent {
        BuckEvent::new(
            SystemTime::UNIX_EPOCH,
            TraceId::new(),
            Some(SpanId::from_u64(span_id).unwrap()),
            parent_id.map(|id| SpanId::from_u64(id).unwrap()),
            SpanStartEvent { data: Some(data) }.into(),
        )
    }

    fn end_event(span_id: u64, data: span_end_event::Data) -> BuckEvent {
        BuckEvent::new(
            SystemTime::UNIX_EPOCH,
            TraceId::new(),
            Some(SpanId::from_u64(span_id).unwrap()),
            None,
            SpanEndEvent {
                stats: None,
                duration: None,
                data: Some(data),
            }
            .into(),
        )
    }

    #[test]
    fn projects_finished_and_unfinished_actions() {
        let action_start = start_event(
            10,
            None,
            span_start_event::Data::ActionExecution(Box::new(slug_data::ActionExecutionStart::default())),
        );
        let action_end = end_event(
            10,
            span_end_event::Data::ActionExecution(Box::new(slug_data::ActionExecutionEnd::default())),
        );
        let unfinished_start = start_event(
            20,
            None,
            span_start_event::Data::ActionExecution(Box::new(slug_data::ActionExecutionStart::default())),
        );

        let projection = project_actions_and_file_changes(
            [&action_start, &action_end, &unfinished_start],
            &WhatRanOptions::default(),
        )
        .unwrap();

        assert_eq!(projection.actions.len(), 1);
        assert_eq!(projection.unfinished_actions.len(), 1);
        assert!(projection.actions[0].span_end.is_some());
        assert!(projection.unfinished_actions[0].span_end.is_none());
    }

    #[test]
    fn projects_file_watcher_changes() {
        let watcher_end = slug_data::FileWatcherEnd {
            stats: Some(slug_data::FileWatcherStats {
                events: vec![slug_data::FileWatcherEvent {
                    path: "foo/bar.txt".to_owned(),
                    kind: slug_data::FileWatcherKind::File as i32,
                }],
                ..Default::default()
            }),
            ..Default::default()
        };

        let projection = project_actions_and_file_changes(
            [&end_event(30, span_end_event::Data::FileWatcher(Box::new(watcher_end)))],
            &WhatRanOptions::default(),
        )
        .unwrap();

        assert_eq!(projection.changed_files.len(), 1);
        assert_eq!(projection.changed_files[0].path, "foo/bar.txt");
    }

    #[tokio::test]
    async fn collects_only_event_records() {
        let action_start = start_event(
            10,
            None,
            span_start_event::Data::ActionExecution(Box::new(slug_data::ActionExecutionStart::default())),
        );

        let values = vec![
            Ok(StreamValue::Result(Box::new(slug_cli_proto::CommandResult::default()))),
            Ok(StreamValue::Event(Box::new(action_start.clone()))),
            Ok(StreamValue::PartialResult(Box::new(slug_cli_proto::PartialResult::default()))),
            Ok(StreamValue::Event(Box::new(action_start))),
        ];

        let events = collect_buck_events(futures::stream::iter(values))
            .await
            .unwrap();
        assert_eq!(events.len(), 2);
    }

    #[tokio::test]
    async fn propagates_stream_errors() {
        let values = vec![Err(slug_error::slug_error!(
            slug_error::ErrorTag::Tier0,
            "stream broke"
        ))];

        let err = collect_buck_events(futures::stream::iter(values))
            .await
            .expect_err("collector must propagate stream errors");
        assert!(format!("{err:#}").contains("stream broke"));
    }
}
