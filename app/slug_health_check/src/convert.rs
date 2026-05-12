/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

// gRPC to rust converters

use slug_error::BuckErrorContext;

use crate::interface::HealthCheckContextEvent;
use crate::interface::HealthCheckSnapshotData;
use crate::interface::HealthCheckType;
use crate::report::DisplayReport;
use crate::report::HealthIssue;
use crate::report::Message;
use crate::report::Remediation;
use crate::report::Report;
use crate::report::Severity;

impl TryFrom<i32> for Severity {
    type Error = slug_error::Error;
    fn try_from(s: i32) -> slug_error::Result<Self> {
        let severity = slug_health_check_proto::Severity::try_from(s)
            .buck_error_context("Invalid `severity`")?;
        Ok(match severity {
            slug_health_check_proto::Severity::Info => Severity::Info,
            slug_health_check_proto::Severity::Warning => Severity::Warning,
        })
    }
}

impl TryInto<i32> for Severity {
    type Error = slug_error::Error;
    fn try_into(self) -> slug_error::Result<i32> {
        Ok(match self {
            Severity::Info => slug_health_check_proto::Severity::Info,
            Severity::Warning => slug_health_check_proto::Severity::Warning,
        } as i32)
    }
}

impl TryFrom<slug_health_check_proto::Remediation> for Remediation {
    type Error = slug_error::Error;

    fn try_from(value: slug_health_check_proto::Remediation) -> slug_error::Result<Self> {
        Ok(
            match value.data.buck_error_context("Invalid `remediation`")? {
                slug_health_check_proto::remediation::Data::Message(message) => {
                    Remediation::Message(message)
                }
                slug_health_check_proto::remediation::Data::Link(link) => Remediation::Link(link),
            },
        )
    }
}

impl TryInto<slug_health_check_proto::Remediation> for Remediation {
    type Error = slug_error::Error;

    fn try_into(self) -> slug_error::Result<slug_health_check_proto::Remediation> {
        let value = match self {
            Remediation::Message(message) => {
                slug_health_check_proto::remediation::Data::Message(message)
            }
            Remediation::Link(link) => slug_health_check_proto::remediation::Data::Link(link),
        };
        Ok(slug_health_check_proto::Remediation { data: Some(value) })
    }
}

impl TryFrom<i32> for HealthCheckType {
    type Error = slug_error::Error;

    fn try_from(value: i32) -> slug_error::Result<Self> {
        let value = slug_health_check_proto::HealthCheckType::try_from(value)
            .buck_error_context("Invalid `health_check_type`")?;
        Ok(match value {
            slug_health_check_proto::HealthCheckType::MemoryPressure => {
                HealthCheckType::MemoryPressure
            }
            slug_health_check_proto::HealthCheckType::LowDiskSpace => HealthCheckType::LowDiskSpace,
            slug_health_check_proto::HealthCheckType::SlowDownloadSpeed => {
                HealthCheckType::SlowDownloadSpeed
            }
            slug_health_check_proto::HealthCheckType::SlowBuild => HealthCheckType::SlowBuild,
            slug_health_check_proto::HealthCheckType::VpnEnabled => HealthCheckType::VpnEnabled,
            slug_health_check_proto::HealthCheckType::StableRevision => {
                HealthCheckType::StableRevision
            }
        })
    }
}

impl TryInto<i32> for HealthCheckType {
    type Error = slug_error::Error;

    fn try_into(self) -> slug_error::Result<i32> {
        Ok(match self {
            HealthCheckType::MemoryPressure => {
                slug_health_check_proto::HealthCheckType::MemoryPressure
            }
            HealthCheckType::LowDiskSpace => slug_health_check_proto::HealthCheckType::LowDiskSpace,
            HealthCheckType::SlowDownloadSpeed => {
                slug_health_check_proto::HealthCheckType::SlowDownloadSpeed
            }
            HealthCheckType::VpnEnabled => slug_health_check_proto::HealthCheckType::VpnEnabled,
            HealthCheckType::StableRevision => {
                slug_health_check_proto::HealthCheckType::StableRevision
            }
            HealthCheckType::SlowBuild => slug_health_check_proto::HealthCheckType::SlowBuild,
        } as i32)
    }
}

impl TryFrom<slug_health_check_proto::Message> for Message {
    type Error = slug_error::Error;

    fn try_from(value: slug_health_check_proto::Message) -> slug_error::Result<Self> {
        match value.data.buck_error_context("Invalid message format")? {
            slug_health_check_proto::message::Data::Simple(text) => Ok(Message::Simple(text)),
            slug_health_check_proto::message::Data::Rich(rich_msg) => Ok(Message::Rich {
                header: rich_msg.header,
                body: rich_msg.body,
                footer: rich_msg.footer,
            }),
        }
    }
}

impl TryInto<slug_health_check_proto::Message> for Message {
    type Error = slug_error::Error;

    fn try_into(self) -> slug_error::Result<slug_health_check_proto::Message> {
        let data = match self {
            Message::Simple(text) => slug_health_check_proto::message::Data::Simple(text),
            Message::Rich {
                header,
                body,
                footer,
            } => {
                slug_health_check_proto::message::Data::Rich(slug_health_check_proto::RichMessage {
                    header,
                    body,
                    footer,
                })
            }
        };
        Ok(slug_health_check_proto::Message { data: Some(data) })
    }
}

impl TryFrom<slug_health_check_proto::HealthIssue> for HealthIssue {
    type Error = slug_error::Error;

    fn try_from(value: slug_health_check_proto::HealthIssue) -> slug_error::Result<Self> {
        Ok(HealthIssue {
            severity: value.severity.try_into()?,
            message: value
                .message
                .buck_error_context("Missing message")?
                .try_into()?,
            remediation: value.remediation.map(|r| r.try_into()).transpose()?,
        })
    }
}

impl TryInto<slug_health_check_proto::HealthIssue> for HealthIssue {
    type Error = slug_error::Error;

    fn try_into(self) -> slug_error::Result<slug_health_check_proto::HealthIssue> {
        Ok(slug_health_check_proto::HealthIssue {
            severity: self.severity.try_into()?,
            message: Some(self.message.try_into()?),
            remediation: self.remediation.map(|r| r.try_into()).transpose()?,
        })
    }
}

impl TryFrom<slug_health_check_proto::DisplayReport> for DisplayReport {
    type Error = slug_error::Error;

    fn try_from(value: slug_health_check_proto::DisplayReport) -> slug_error::Result<Self> {
        Ok(DisplayReport {
            health_check_type: value.health_check_type.try_into()?,
            health_issue: value.health_issue.map(|i| i.try_into()).transpose()?,
        })
    }
}
impl TryInto<slug_health_check_proto::DisplayReport> for DisplayReport {
    type Error = slug_error::Error;

    fn try_into(self) -> slug_error::Result<slug_health_check_proto::DisplayReport> {
        Ok(slug_health_check_proto::DisplayReport {
            health_check_type: self.health_check_type.try_into()?,
            health_issue: self.health_issue.map(|i| i.try_into()).transpose()?,
        })
    }
}

impl TryFrom<slug_health_check_proto::Report> for Report {
    type Error = slug_error::Error;

    fn try_from(value: slug_health_check_proto::Report) -> slug_error::Result<Self> {
        Ok(Report {
            display_report: value.display_report.map(|d| d.try_into()).transpose()?,
            tag: value.tag,
        })
    }
}

impl TryInto<slug_health_check_proto::Report> for Report {
    type Error = slug_error::Error;

    fn try_into(self) -> slug_error::Result<slug_health_check_proto::Report> {
        Ok(slug_health_check_proto::Report {
            display_report: self.display_report.map(|d| d.try_into()).transpose()?,
            tag: self.tag,
        })
    }
}

impl TryInto<slug_health_check_proto::HealthCheckContextEvent> for HealthCheckContextEvent {
    type Error = slug_error::Error;

    fn try_into(self) -> slug_error::Result<slug_health_check_proto::HealthCheckContextEvent> {
        Ok(match self {
            HealthCheckContextEvent::BranchedFromRevision(rev) => {
                slug_health_check_proto::HealthCheckContextEvent {
                    data: Some(slug_health_check_proto::health_check_context_event::Data::BranchedFromRevision(rev)),
                }
            }
            HealthCheckContextEvent::CommandStart(cmd) => {
                slug_health_check_proto::HealthCheckContextEvent {
                    data: Some(slug_health_check_proto::health_check_context_event::Data::CommandStart(cmd.clone())),
                }
            }
            HealthCheckContextEvent::ParsedTargetPatterns(patterns) => {
                slug_health_check_proto::HealthCheckContextEvent {
                    data: Some(slug_health_check_proto::health_check_context_event::Data::ParsedTargetPatterns(patterns.clone())),
                }
            }
            HealthCheckContextEvent::HasExcessCacheMisses() => {
                slug_health_check_proto::HealthCheckContextEvent {
                    data: Some(slug_health_check_proto::health_check_context_event::Data::HasExcessCacheMisses(true)),
                }
            }
            HealthCheckContextEvent::ExperimentConfigurations(system_info) => {
                slug_health_check_proto::HealthCheckContextEvent {
                    data: Some(slug_health_check_proto::health_check_context_event::Data::ExperimentConfigurations(system_info.clone())),
                }
            }
        })
    }
}

impl TryFrom<slug_health_check_proto::HealthCheckContextEvent> for HealthCheckContextEvent {
    type Error = slug_error::Error;
    fn try_from(
        value: slug_health_check_proto::HealthCheckContextEvent,
    ) -> slug_error::Result<Self> {
        Ok( match value.data.buck_error_context("Invalid `health_check_context_event`")? {
            slug_health_check_proto::health_check_context_event::Data::BranchedFromRevision(rev) => {
                HealthCheckContextEvent::BranchedFromRevision(rev)
            }
            slug_health_check_proto::health_check_context_event::Data::CommandStart(cmd) => {
                HealthCheckContextEvent::CommandStart(cmd)
            }
            slug_health_check_proto::health_check_context_event::Data::ParsedTargetPatterns(patterns) => {
                HealthCheckContextEvent::ParsedTargetPatterns(patterns)
            }
            slug_health_check_proto::health_check_context_event::Data::HasExcessCacheMisses(_) => {
                HealthCheckContextEvent::HasExcessCacheMisses()
            }
            slug_health_check_proto::health_check_context_event::Data::ExperimentConfigurations(system_info) => {
                HealthCheckContextEvent::ExperimentConfigurations(system_info)
            }
        }
    )
    }
}

impl TryFrom<slug_health_check_proto::HealthCheckSnapshotData> for HealthCheckSnapshotData {
    type Error = slug_error::Error;

    fn try_from(
        value: slug_health_check_proto::HealthCheckSnapshotData,
    ) -> slug_error::Result<Self> {
        use std::time::Duration;
        use std::time::UNIX_EPOCH;

        let proto_timestamp = value.timestamp.ok_or_else(|| {
            slug_error::slug_error!(
                slug_error::ErrorTag::HealthCheck,
                "Missing timestamp in HealthCheckSnapshotData"
            )
        })?;

        // Convert protobuf Timestamp to SystemTime
        let duration = Duration::new(proto_timestamp.seconds as u64, proto_timestamp.nanos as u32);
        let timestamp = UNIX_EPOCH + duration;

        Ok(HealthCheckSnapshotData { timestamp })
    }
}

impl TryInto<slug_health_check_proto::HealthCheckSnapshotData> for HealthCheckSnapshotData {
    type Error = slug_error::Error;

    fn try_into(self) -> slug_error::Result<slug_health_check_proto::HealthCheckSnapshotData> {
        // Convert SystemTime to protobuf Timestamp
        let duration_since_epoch = self
            .timestamp
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_e| {
                slug_error::slug_error!(
                    slug_error::ErrorTag::HealthCheck,
                    "Invalid timestamp in HealthCheckSnapshotData"
                )
            })?;

        let timestamp = Some(prost_types::Timestamp {
            seconds: duration_since_epoch.as_secs() as i64,
            nanos: duration_since_epoch.subsec_nanos() as i32,
        });

        Ok(slug_health_check_proto::HealthCheckSnapshotData { timestamp })
    }
}
