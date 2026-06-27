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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildEventId {
    BuildStarted,
    ConfiguredTarget {
        label: String,
    },
    ActionCompleted {
        label: String,
        primary_output: String,
    },
    TestResult {
        label: String,
        attempt: u32,
    },
    BuildFinished,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildEventPayload {
    BuildStarted {
        command: String,
    },
    ConfiguredTarget {
        label: String,
    },
    ActionCompleted {
        label: String,
        primary_output: String,
        success: bool,
    },
    TestResult {
        label: String,
        status: String,
        attempt: u32,
    },
    BuildFinished {
        success: bool,
        exit_code: i32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildEvent {
    pub id: BuildEventId,
    pub children: Vec<BuildEventId>,
    pub payload: BuildEventPayload,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BuildEventStream {
    events: Vec<BuildEvent>,
}

impl BuildEvent {
    pub fn build_started(command: impl Into<String>) -> Self {
        Self {
            id: BuildEventId::BuildStarted,
            children: Vec::new(),
            payload: BuildEventPayload::BuildStarted {
                command: command.into(),
            },
        }
    }

    pub fn configured_target(label: impl Into<String>) -> Self {
        let label = label.into();
        Self {
            id: BuildEventId::ConfiguredTarget {
                label: label.clone(),
            },
            children: Vec::new(),
            payload: BuildEventPayload::ConfiguredTarget { label },
        }
    }

    pub fn action_completed(
        label: impl Into<String>,
        primary_output: impl Into<String>,
        success: bool,
    ) -> Self {
        let label = label.into();
        let primary_output = primary_output.into();
        Self {
            id: BuildEventId::ActionCompleted {
                label: label.clone(),
                primary_output: primary_output.clone(),
            },
            children: Vec::new(),
            payload: BuildEventPayload::ActionCompleted {
                label,
                primary_output,
                success,
            },
        }
    }

    pub fn test_result(label: impl Into<String>, status: impl Into<String>, attempt: u32) -> Self {
        let label = label.into();
        Self {
            id: BuildEventId::TestResult {
                label: label.clone(),
                attempt,
            },
            children: Vec::new(),
            payload: BuildEventPayload::TestResult {
                label,
                status: status.into(),
                attempt,
            },
        }
    }

    pub fn build_finished(success: bool, exit_code: i32) -> Self {
        Self {
            id: BuildEventId::BuildFinished,
            children: Vec::new(),
            payload: BuildEventPayload::BuildFinished { success, exit_code },
        }
    }

    pub fn with_children(mut self, children: Vec<BuildEventId>) -> Self {
        self.children = children;
        self
    }

    pub fn to_json_line(&self) -> String {
        format!(
            "{{\"id\":{},\"children\":[{}],\"payload\":{}}}",
            self.id.to_json(),
            self.children
                .iter()
                .map(BuildEventId::to_json)
                .collect::<Vec<_>>()
                .join(","),
            self.payload.to_json(),
        )
    }
}

impl BuildEventStream {
    pub fn new(events: Vec<BuildEvent>) -> Self {
        Self { events }
    }

    pub fn minimal_build_test(
        label: impl Into<String>,
        primary_output: impl Into<String>,
        test_status: impl Into<String>,
    ) -> Self {
        let label = label.into();
        let primary_output = primary_output.into();
        let configured_id = BuildEventId::ConfiguredTarget {
            label: label.clone(),
        };
        let action_id = BuildEventId::ActionCompleted {
            label: label.clone(),
            primary_output: primary_output.clone(),
        };
        let test_id = BuildEventId::TestResult {
            label: label.clone(),
            attempt: 1,
        };
        Self {
            events: vec![
                BuildEvent::build_started("test").with_children(vec![configured_id.clone()]),
                BuildEvent::configured_target(label.clone())
                    .with_children(vec![action_id.clone(), test_id.clone()]),
                BuildEvent::action_completed(label.clone(), primary_output, true),
                BuildEvent::test_result(label, test_status, 1)
                    .with_children(vec![BuildEventId::BuildFinished]),
                BuildEvent::build_finished(true, 0),
            ],
        }
    }

    pub fn events(&self) -> &[BuildEvent] {
        &self.events
    }

    pub fn to_json_lines(&self) -> String {
        let mut lines = self
            .events
            .iter()
            .map(BuildEvent::to_json_line)
            .collect::<Vec<_>>()
            .join("\n");
        lines.push('\n');
        lines
    }
}

impl BuildEventId {
    fn to_json(&self) -> String {
        match self {
            Self::BuildStarted => "{\"kind\":\"started\"}".to_owned(),
            Self::ConfiguredTarget { label } => {
                format!(
                    "{{\"kind\":\"configuredTarget\",\"label\":\"{}\"}}",
                    json_escape(label)
                )
            }
            Self::ActionCompleted {
                label,
                primary_output,
            } => {
                format!(
                    "{{\"kind\":\"actionCompleted\",\"label\":\"{}\",\"primaryOutput\":\"{}\"}}",
                    json_escape(label),
                    json_escape(primary_output)
                )
            }
            Self::TestResult { label, attempt } => {
                format!(
                    "{{\"kind\":\"testResult\",\"label\":\"{}\",\"attempt\":{attempt}}}",
                    json_escape(label)
                )
            }
            Self::BuildFinished => "{\"kind\":\"finished\"}".to_owned(),
        }
    }
}

impl BuildEventPayload {
    fn to_json(&self) -> String {
        match self {
            Self::BuildStarted { command } => {
                format!(
                    "{{\"buildStarted\":{{\"command\":\"{}\"}}}}",
                    json_escape(command)
                )
            }
            Self::ConfiguredTarget { label } => {
                format!(
                    "{{\"configuredTarget\":{{\"label\":\"{}\"}}}}",
                    json_escape(label)
                )
            }
            Self::ActionCompleted {
                label,
                primary_output,
                success,
            } => {
                format!(
                    "{{\"actionCompleted\":{{\"label\":\"{}\",\"primaryOutput\":\"{}\",\"success\":{success}}}}}",
                    json_escape(label),
                    json_escape(primary_output)
                )
            }
            Self::TestResult {
                label,
                status,
                attempt,
            } => {
                format!(
                    "{{\"testResult\":{{\"label\":\"{}\",\"status\":\"{}\",\"attempt\":{attempt}}}}}",
                    json_escape(label),
                    json_escape(status)
                )
            }
            Self::BuildFinished { success, exit_code } => {
                format!("{{\"buildFinished\":{{\"success\":{success},\"exitCode\":{exit_code}}}}}")
            }
        }
    }
}

impl fmt::Display for BuildEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_json_line())
    }
}

fn json_escape(value: &str) -> String {
    let mut escaped = String::new();
    for ch in value.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            ch if ch.is_control() => escaped.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => escaped.push(ch),
        }
    }
    escaped
}
