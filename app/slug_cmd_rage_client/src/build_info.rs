/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::fmt;
use std::time::Duration;
use std::time::SystemTime;

use chrono::DateTime;
use chrono::Local;
use futures::TryStreamExt;
use humantime::format_duration;
use slug_event_log::read::EventLogPathBuf;
use slug_event_log::stream_value::StreamValue;
use slug_event_log::utils::Invocation;
use slug_events::BuckEvent;
use slug_util::truncate::truncate;
use slug_wrapper_common::invocation_id::TraceId;

#[derive(Debug, slug_error::Error)]
#[slug(tag = Tier0)]
enum BuildInfoError {
    #[error("Failed to read event log")]
    EventLogReadFail,
}

struct LogInfo {
    revision: Option<String>,
    daemon_uptime_s: Option<u64>,
    timestamp_end: Option<SystemTime>,
    re_session_id: Option<String>,
}

pub(crate) struct BuildInfo {
    uuid: TraceId,
    pub timestamp: DateTime<Local>,
    pub command: String,
    working_dir: String,
    pub slug_revision: String,
    pub command_duration: Option<Duration>,
    pub daemon_uptime_s: Option<u64>,
    pub re_session_id: Option<String>,
}

impl fmt::Display for BuildInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "slug UI: https://www.internalfb.com/slug/{}
timestamp: {}
command: {}
working dir: {}
slug_revision: {}
command duration: {}
daemon uptime: {}
RE session id: {}
        ",
            self.uuid,
            self.timestamp.format("%c %Z"),
            self.command,
            self.working_dir,
            self.slug_revision,
            seconds_to_string(self.command_duration.map(|d| d.as_secs())),
            seconds_to_string(self.daemon_uptime_s),
            self.re_session_id
                .as_ref()
                .map_or_else(|| "", |s| s.as_str()),
        )
    }
}

pub(crate) async fn get(log: &EventLogPathBuf) -> slug_error::Result<BuildInfo> {
    let (invocation, events) = log.unpack_stream().await?;
    let mut filtered_events = events.try_filter_map(|log| {
        let maybe_buck_event = match log {
            StreamValue::Result(_) | StreamValue::PartialResult(_) => None,
            StreamValue::Event(buck_event) => Some(buck_event),
        };
        futures::future::ready(Ok(maybe_buck_event))
    });

    let first_event: BuckEvent = filtered_events
        .try_next()
        .await?
        .ok_or(BuildInfoError::EventLogReadFail)?
        .try_into()?;

    let mut info = LogInfo {
        revision: None,
        daemon_uptime_s: None,
        timestamp_end: None,
        re_session_id: None,
    };
    loop {
        let res = match filtered_events.try_next().await {
            Ok(Some(event)) => extract_info(&mut info, event),
            Ok(None) => break,
            Err(e) => Err(e.into()),
        };
        if let Err(e) = res {
            slug_client_ctx::eprintln!("Error found when iterating through logs: {:#}", e)?;
            break;
        }
    }

    let timestamp_start = first_event.timestamp();
    let duration = {
        if let Some(end) = info.timestamp_end {
            Some(end.duration_since(timestamp_start)?)
        } else {
            None
        }
    };

    let t_start: DateTime<Local> = timestamp_start.into();

    let output = BuildInfo {
        uuid: first_event.trace_id()?,
        timestamp: t_start,
        command: format_cmd(&invocation),
        working_dir: invocation.working_dir,
        slug_revision: info.revision.unwrap_or_else(|| "".to_owned()),
        command_duration: duration,
        daemon_uptime_s: info.daemon_uptime_s,
        re_session_id: info.re_session_id,
    };

    Ok(output)
}

fn extract_info(info: &mut LogInfo, event: Box<slug_data::BuckEvent>) -> slug_error::Result<()> {
    match event.data {
        Some(slug_data::buck_event::Data::SpanStart(span)) => match &span.data {
            Some(slug_data::span_start_event::Data::Command(action)) => {
                if info.revision.is_none() && action.metadata.contains_key("slug_revision") {
                    if let Some(slug_revision) = action.metadata.get("slug_revision") {
                        info.revision.get_or_insert(slug_revision.clone());
                    }
                }
            }
            _ => (),
        },
        Some(slug_data::buck_event::Data::Instant(span)) => match &span.data {
            Some(slug_data::instant_event::Data::Snapshot(snapshot)) => {
                info.daemon_uptime_s.get_or_insert(snapshot.daemon_uptime_s);
            }
            Some(slug_data::instant_event::Data::ReSession(session)) => {
                info.re_session_id.get_or_insert(session.session_id.clone());
            }
            _ => (),
        },

        _ => (),
    }
    if let Some(timestamp) = event.timestamp {
        info.timestamp_end = Some(SystemTime::try_from(timestamp)?)
    };
    Ok(())
}

fn seconds_to_string(seconds: Option<u64>) -> String {
    if let Some(seconds) = seconds {
        let duration = Duration::from_secs(seconds);
        format_duration(duration).to_string()
    } else {
        "".to_owned()
    }
}

pub fn format_cmd(cmd: &Invocation) -> String {
    truncate(&cmd.display_expanded_command_line(), 256)
}
