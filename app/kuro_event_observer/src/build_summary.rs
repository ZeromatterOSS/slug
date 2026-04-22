/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Rollup aggregator for the Kuro event stream.
//!
//! Produces a [`BuildSummary`] from a single pass over the events. Powers
//! both the live end-of-build summary (Plan 16.2) and the offline
//! `kuro log summary` / `kuro log diff` commands (Plan 16.3 / 16.4). The
//! struct is stable: downstream tooling (the harness in Plan 16.8, CI
//! regression gates) depends on its shape.
//!
//! No re-reads of the event log — callers feed events in order and call
//! [`BuildSummaryBuilder::finalize`] once, at BuildFinished or EOF.

use std::collections::HashMap;

use kuro_data::ActionExecutionEnd;
use kuro_data::ActionKind;
use kuro_events::BuckEvent;

use crate::action_stats::ActionStats;
use crate::last_command_execution_kind::LastCommandExecutionKind;
use crate::last_command_execution_kind::get_last_command_execution_kind;

/// Consolidated rollup of one build invocation. Stable shape — Plans 16.3,
/// 16.4 and 16.8 tooling all consume this.
#[derive(Debug, Default, Clone)]
pub struct BuildSummary {
    // Phase durations, derived from matching SpanStart/End pairs.
    pub load_wall_us: u64,
    pub analyze_wall_us: u64,
    pub execute_wall_us: u64,
    pub materialize_wall_us: u64,
    /// Total wall clock: first SpanStart → last SpanEnd of any top-level
    /// `Command` span.
    pub total_wall_us: u64,

    /// Action rollup keyed by mnemonic (category from `ActionName.category`).
    /// Sorted by `total_wall_us` descending.
    pub by_mnemonic: Vec<MnemonicRow>,

    /// Top-N slowest leaves by duration. Default N = 10; see
    /// [`BuildSummaryBuilder::new`].
    pub slowest_actions: Vec<ActionRow>,
    pub slowest_analyses: Vec<AnalysisRow>,

    // Cache breakdown. Matches `ActionStats::total_cache_hit_percentage`.
    pub cache_hit_pct: f64,
    pub cache_hit_pct_by_mnemonic: Vec<(String, f64)>,

    // Parallelism indicators.
    pub peak_in_flight_actions: u32,
    pub total_action_count: u64,

    // Graph sizing, lifted from the terminal `BuildGraphExecutionInfo`.
    pub num_dice_nodes: u64,
    pub num_dice_edges: u64,
    pub action_graph_size: u64,

    // Critical + slowest path totals, summed from `BuildGraphExecutionInfo`.
    pub critical_path_wall_us: u64,
    pub slowest_path_wall_us: u64,
}

#[derive(Debug, Clone)]
pub struct MnemonicRow {
    pub category: String,
    pub count: u64,
    /// Actions in this mnemonic that were served by any cache (local,
    /// remote, or remote dep-file).
    pub cached: u64,
    /// Sum of exec-only wall (`ActionExecutionEnd.wall_time`).
    pub total_wall_us: u64,
    /// Sum of queue wait per action (outer span duration minus exec wall).
    /// Non-zero means the scheduler admitted work faster than it could
    /// dispatch it — the signal for Plan 17.2.
    pub total_queue_us: u64,
    /// Sum of wall durations for entries of this mnemonic that appeared on
    /// the critical path. Zero if the critical path wasn't emitted.
    pub critical_wall_us: u64,
    pub p50_us: u64,
    pub p95_us: u64,
    pub p99_us: u64,
}

#[derive(Debug, Clone)]
pub struct ActionRow {
    pub category: String,
    pub identifier: String,
    pub wall_us: u64,
    pub cached: bool,
}

#[derive(Debug, Clone)]
pub struct AnalysisRow {
    pub target: String,
    pub wall_us: u64,
}

/// Streaming aggregator. Feed events in any order; call
/// [`finalize`](Self::finalize) when you're done.
pub struct BuildSummaryBuilder {
    top_n: usize,
    per_mnemonic: HashMap<String, MnemonicAcc>,

    // Phase wall tracking. For each phase, the earliest SpanStart
    // timestamp and latest SpanEnd timestamp. Elapsed = last - first.
    // This gives the true wall slice attributed to the phase; summing
    // overlapping span durations would wildly overcount in parallel
    // phases (observed: execute_wall=23m58s when total_wall=22.8s).
    load_phase: PhaseSpan,
    analyze_phase: PhaseSpan,
    execute_phase: PhaseSpan,
    materialize_phase: PhaseSpan,
    command_first_ts_us: Option<u64>,
    command_last_ts_us: Option<u64>,

    // Top-N kept sorted descending by wall_us. Small N (default 10) makes
    // the re-sort cost negligible — simpler than a heap and avoids
    // requiring Ord on the row types (which carry Strings).
    slowest_actions: Vec<ActionRow>,
    slowest_analyses: Vec<AnalysisRow>,

    // Live parallelism counter for ActionExecution spans.
    in_flight_actions: u32,
    peak_in_flight_actions: u32,

    action_stats: ActionStats,
    total_action_count: u64,

    // Graph + critical-path data lifted from BuildGraphExecutionInfo.
    num_dice_nodes: u64,
    num_dice_edges: u64,
    action_graph_size: u64,
    critical_path_wall_us: u64,
    slowest_path_wall_us: u64,
    critical_wall_us_by_category: HashMap<String, u64>,
}

#[derive(Debug, Default)]
struct MnemonicAcc {
    count: u64,
    cached: u64,
    /// Sum of exec-only wall times (ActionExecutionEnd.wall_time).
    total_wall_us: u64,
    /// Sum of queue waits (span duration − exec wall). Non-zero values
    /// here are the scheduler-pressure signal we were missing before.
    total_queue_us: u64,
    durations_us: Vec<u64>,
}

#[derive(Debug, Default)]
struct PhaseSpan {
    first_start_us: Option<u64>,
    last_end_us: Option<u64>,
}

impl PhaseSpan {
    fn observe_start(&mut self, ts_us: Option<u64>) {
        if let Some(ts) = ts_us {
            self.first_start_us = Some(match self.first_start_us {
                Some(prev) => prev.min(ts),
                None => ts,
            });
        }
    }

    fn observe_end(&mut self, ts_us: Option<u64>) {
        if let Some(ts) = ts_us {
            self.last_end_us = Some(match self.last_end_us {
                Some(prev) => prev.max(ts),
                None => ts,
            });
        }
    }

    fn elapsed_us(&self) -> u64 {
        match (self.first_start_us, self.last_end_us) {
            (Some(s), Some(e)) if e >= s => e - s,
            _ => 0,
        }
    }
}

impl BuildSummaryBuilder {
    pub fn new() -> Self {
        Self::with_top_n(10)
    }

    pub fn with_top_n(top_n: usize) -> Self {
        Self {
            top_n,
            per_mnemonic: HashMap::new(),
            load_phase: PhaseSpan::default(),
            analyze_phase: PhaseSpan::default(),
            execute_phase: PhaseSpan::default(),
            materialize_phase: PhaseSpan::default(),
            command_first_ts_us: None,
            command_last_ts_us: None,
            slowest_actions: Vec::new(),
            slowest_analyses: Vec::new(),
            in_flight_actions: 0,
            peak_in_flight_actions: 0,
            action_stats: ActionStats::default(),
            total_action_count: 0,
            num_dice_nodes: 0,
            num_dice_edges: 0,
            action_graph_size: 0,
            critical_path_wall_us: 0,
            slowest_path_wall_us: 0,
            critical_wall_us_by_category: HashMap::new(),
        }
    }

    pub fn handle_event(&mut self, event: &BuckEvent) {
        use kuro_data::buck_event::Data;

        match event.data() {
            Data::SpanStart(span) => {
                self.on_span_start(event, span);
            }
            Data::SpanEnd(span) => {
                self.on_span_end(event, span);
            }
            Data::Instant(instant) => {
                if let Some(kuro_data::instant_event::Data::BuildGraphInfo(info)) = &instant.data {
                    self.on_build_graph_info(info);
                }
            }
            _ => {}
        }
    }

    fn on_span_start(&mut self, event: &BuckEvent, span: &kuro_data::SpanStartEvent) {
        use kuro_data::span_start_event::Data;

        let ts_us = timestamp_us(event);
        match &span.data {
            Some(Data::Command(_)) => {
                self.command_first_ts_us
                    .get_or_insert(ts_us.unwrap_or_default());
            }
            Some(Data::ActionExecution(_)) => {
                self.in_flight_actions += 1;
                self.peak_in_flight_actions =
                    self.peak_in_flight_actions.max(self.in_flight_actions);
                self.execute_phase.observe_start(ts_us);
            }
            Some(Data::Load(_)) | Some(Data::LoadPackage(_)) => {
                self.load_phase.observe_start(ts_us);
            }
            Some(Data::Analysis(_)) => {
                self.analyze_phase.observe_start(ts_us);
            }
            Some(Data::FinalMaterialization(_)) | Some(Data::Materialization(_)) => {
                self.materialize_phase.observe_start(ts_us);
            }
            _ => {}
        }
    }

    fn on_span_end(&mut self, event: &BuckEvent, span: &kuro_data::SpanEndEvent) {
        use kuro_data::span_end_event::Data;

        let duration_us = span
            .duration
            .as_ref()
            .map(|d| duration_to_us(d))
            .unwrap_or(0);

        let ts_us = timestamp_us(event);

        match &span.data {
            Some(Data::Command(_)) => {
                // Track last Command end for total wall. Using ts at span
                // end (rather than SpanEnd duration) tolerates nested
                // commands without double-counting.
                if let Some(ts) = ts_us {
                    let end = self.command_last_ts_us.get_or_insert(ts);
                    if ts > *end {
                        *end = ts;
                    }
                }
            }
            Some(Data::Load(_)) | Some(Data::LoadPackage(_)) => {
                self.load_phase.observe_end(ts_us);
            }
            Some(Data::Analysis(analysis)) => {
                self.analyze_phase.observe_end(ts_us);
                let target = match &analysis.target {
                    Some(kuro_data::analysis_end::Target::StandardTarget(t)) => {
                        format_target_label(t)
                    }
                    Some(kuro_data::analysis_end::Target::AnonTarget(_)) => "<anon>".to_owned(),
                    Some(kuro_data::analysis_end::Target::DynamicLambda(_)) => {
                        "<dynamic_lambda>".to_owned()
                    }
                    None => "<unknown>".to_owned(),
                };
                self.push_top_n_analysis(AnalysisRow {
                    target,
                    wall_us: duration_us,
                });
            }
            Some(Data::ActionExecution(action)) => {
                self.execute_phase.observe_end(ts_us);
                self.in_flight_actions = self.in_flight_actions.saturating_sub(1);
                self.action_stats.update(action);
                self.total_action_count += 1;
                // ActionExecutionEnd.wall_time is the exec-only wall
                // ("Omits queue time", see data.proto:1634). Prefer it over
                // SpanEndEvent.duration which wraps the whole calculation
                // future — queue wait + dispatch + exec. Using the outer
                // span's duration inflates every per-mnemonic total by the
                // queue-depth factor on throughput-bound builds.
                let exec_wall_us = action
                    .wall_time
                    .as_ref()
                    .map(duration_to_us)
                    .unwrap_or(duration_us);
                let queue_wall_us = duration_us.saturating_sub(exec_wall_us);
                self.record_action(action, exec_wall_us, queue_wall_us);
            }
            Some(Data::FinalMaterialization(_)) | Some(Data::Materialization(_)) => {
                self.materialize_phase.observe_end(ts_us);
            }
            _ => {}
        }
    }

    fn record_action(
        &mut self,
        action: &ActionExecutionEnd,
        exec_wall_us: u64,
        queue_wall_us: u64,
    ) {
        // Only count real Run actions in per-mnemonic rollup. Skip the
        // synthetic action kinds (copy/symlink) which would pollute the
        // mnemonic list.
        if action.kind != ActionKind::Run as i32 {
            return;
        }
        let name = match action.name.as_ref() {
            Some(n) => n,
            None => return,
        };
        let category = name.category.clone();

        let is_cached = matches!(
            get_last_command_execution_kind(action),
            LastCommandExecutionKind::Cached | LastCommandExecutionKind::RemoteDepFileCached
        );

        let acc = self.per_mnemonic.entry(category.clone()).or_default();
        acc.count += 1;
        if is_cached {
            acc.cached += 1;
        }
        acc.total_wall_us += exec_wall_us;
        acc.total_queue_us += queue_wall_us;
        acc.durations_us.push(exec_wall_us);

        self.push_top_n_action(ActionRow {
            category,
            identifier: name.identifier.clone(),
            wall_us: exec_wall_us,
            cached: is_cached,
        });
    }

    fn push_top_n_action(&mut self, row: ActionRow) {
        if self.top_n == 0 {
            return;
        }
        if self.slowest_actions.len() < self.top_n {
            self.slowest_actions.push(row);
        } else {
            let min_wall = self.slowest_actions.last().map(|r| r.wall_us).unwrap_or(0);
            if row.wall_us <= min_wall {
                return;
            }
            self.slowest_actions.pop();
            self.slowest_actions.push(row);
        }
        self.slowest_actions
            .sort_by(|a, b| b.wall_us.cmp(&a.wall_us));
    }

    fn push_top_n_analysis(&mut self, row: AnalysisRow) {
        if self.top_n == 0 {
            return;
        }
        if self.slowest_analyses.len() < self.top_n {
            self.slowest_analyses.push(row);
        } else {
            let min_wall = self.slowest_analyses.last().map(|r| r.wall_us).unwrap_or(0);
            if row.wall_us <= min_wall {
                return;
            }
            self.slowest_analyses.pop();
            self.slowest_analyses.push(row);
        }
        self.slowest_analyses
            .sort_by(|a, b| b.wall_us.cmp(&a.wall_us));
    }

    fn on_build_graph_info(&mut self, info: &kuro_data::BuildGraphExecutionInfo) {
        self.num_dice_nodes = info.num_nodes;
        self.num_dice_edges = info.num_edges;

        let sum_path = |entries: &[kuro_data::CriticalPathEntry2]| -> u64 {
            entries
                .iter()
                .map(|e| e.duration.as_ref().map(duration_to_us).unwrap_or(0))
                .sum()
        };
        self.critical_path_wall_us = sum_path(&info.critical_path2);
        self.slowest_path_wall_us = sum_path(&info.slowest_path);

        for entry in &info.critical_path2 {
            let dur = entry.duration.as_ref().map(duration_to_us).unwrap_or(0);
            let category = match &entry.entry {
                Some(kuro_data::critical_path_entry2::Entry::ActionExecution(a)) => a
                    .name
                    .as_ref()
                    .map(|n| n.category.clone())
                    .unwrap_or_else(|| "action".to_owned()),
                Some(kuro_data::critical_path_entry2::Entry::Analysis(_)) => "analysis".to_owned(),
                Some(kuro_data::critical_path_entry2::Entry::Load(_)) => "load".to_owned(),
                Some(kuro_data::critical_path_entry2::Entry::Listing(_)) => "listing".to_owned(),
                _ => continue,
            };
            *self
                .critical_wall_us_by_category
                .entry(category)
                .or_insert(0) += dur;
        }

        self.action_graph_size = info
            .critical_path2
            .iter()
            .filter(|e| {
                matches!(
                    e.entry,
                    Some(kuro_data::critical_path_entry2::Entry::ActionExecution(_))
                )
            })
            .count() as u64;
    }

    pub fn finalize(mut self) -> BuildSummary {
        let cache_hit_pct = self.action_stats.total_cache_hit_percentage() as f64;

        // Compute per-mnemonic stats + rollup.
        let mut by_mnemonic: Vec<MnemonicRow> = self
            .per_mnemonic
            .into_iter()
            .map(|(category, mut acc)| {
                acc.durations_us.sort_unstable();
                let p = |q: f64| percentile(&acc.durations_us, q);
                let critical_wall_us = self
                    .critical_wall_us_by_category
                    .remove(&category)
                    .unwrap_or(0);
                MnemonicRow {
                    category,
                    count: acc.count,
                    cached: acc.cached,
                    total_wall_us: acc.total_wall_us,
                    total_queue_us: acc.total_queue_us,
                    critical_wall_us,
                    p50_us: p(0.50),
                    p95_us: p(0.95),
                    p99_us: p(0.99),
                }
            })
            .collect();
        by_mnemonic.sort_by(|a, b| b.total_wall_us.cmp(&a.total_wall_us));

        let cache_hit_pct_by_mnemonic: Vec<(String, f64)> = by_mnemonic
            .iter()
            .map(|row| {
                let pct = if row.count == 0 {
                    0.0
                } else {
                    (row.cached as f64 / row.count as f64) * 100.0
                };
                (row.category.clone(), pct)
            })
            .collect();

        // push_top_n_* already keeps these sorted descending.
        let slowest_actions = self.slowest_actions;
        let slowest_analyses = self.slowest_analyses;

        let load_wall_us = self.load_phase.elapsed_us();
        let analyze_wall_us = self.analyze_phase.elapsed_us();
        let execute_wall_us = self.execute_phase.elapsed_us();
        let materialize_wall_us = self.materialize_phase.elapsed_us();

        let total_wall_us = match (self.command_first_ts_us, self.command_last_ts_us) {
            (Some(start), Some(end)) if end > start => end - start,
            _ => load_wall_us + analyze_wall_us + execute_wall_us,
        };

        BuildSummary {
            load_wall_us,
            analyze_wall_us,
            execute_wall_us,
            materialize_wall_us,
            total_wall_us,
            by_mnemonic,
            slowest_actions,
            slowest_analyses,
            cache_hit_pct,
            cache_hit_pct_by_mnemonic,
            peak_in_flight_actions: self.peak_in_flight_actions,
            total_action_count: self.total_action_count,
            num_dice_nodes: self.num_dice_nodes,
            num_dice_edges: self.num_dice_edges,
            action_graph_size: self.action_graph_size,
            critical_path_wall_us: self.critical_path_wall_us,
            slowest_path_wall_us: self.slowest_path_wall_us,
        }
    }

    pub fn action_stats(&self) -> &ActionStats {
        &self.action_stats
    }
}

impl Default for BuildSummaryBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl BuildSummary {
    /// Compact two-line summary suitable for the live end-of-build TTY
    /// output. Empty Vec if nothing useful to show.
    pub fn short_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();

        if self.total_wall_us > 0
            || self.load_wall_us > 0
            || self.analyze_wall_us > 0
            || self.execute_wall_us > 0
        {
            lines.push(format!(
                "Phases: load={} analyze={} execute={} materialize={} total={}",
                fmt_duration_us(self.load_wall_us),
                fmt_duration_us(self.analyze_wall_us),
                fmt_duration_us(self.execute_wall_us),
                fmt_duration_us(self.materialize_wall_us),
                fmt_duration_us(self.total_wall_us),
            ));
        }

        if !self.by_mnemonic.is_empty() {
            const SHOW: usize = 3;
            let top: Vec<String> = self
                .by_mnemonic
                .iter()
                .take(SHOW)
                .map(|row| {
                    format!(
                        "{}={}/{}",
                        row.category,
                        fmt_duration_us(row.total_wall_us),
                        row.count
                    )
                })
                .collect();
            let extra = self.by_mnemonic.len().saturating_sub(SHOW);
            let suffix = if extra > 0 {
                format!(" ({extra} more)")
            } else {
                String::new()
            };
            lines.push(format!("Top mnemonics: {}{}", top.join(" "), suffix));
        }

        // Scheduler-pressure signal: ratio of total queue-wait to
        // total exec-wall, summed over all actions. >1× means the
        // scheduler admitted more work than cores could run. Only
        // surface when non-trivial — a cold incremental rebuild with
        // zero actions would otherwise always show "0ms / 0ms".
        let total_exec: u64 = self.by_mnemonic.iter().map(|r| r.total_wall_us).sum();
        let total_queue: u64 = self.by_mnemonic.iter().map(|r| r.total_queue_us).sum();
        if total_exec > 0 && total_queue > 0 {
            let ratio = total_queue as f64 / total_exec as f64;
            lines.push(format!(
                "Queue: {} wait / {} exec ({:.2}× ratio)",
                fmt_duration_us(total_queue),
                fmt_duration_us(total_exec),
                ratio,
            ));
        }

        if self.critical_path_wall_us > 0 {
            lines.push(format!(
                "Critical path: {}  Slowest path: {}",
                fmt_duration_us(self.critical_path_wall_us),
                fmt_duration_us(self.slowest_path_wall_us),
            ));
        }

        lines
    }
}

fn fmt_duration_us(us: u64) -> String {
    // Mirror bazel's "12.3s" / "1m34s" short style. No humanized library
    // dependency — the existing crate's HumanizedBytes is bytes-only.
    if us == 0 {
        return "0s".to_owned();
    }
    let total_ms = us / 1_000;
    if total_ms < 1_000 {
        return format!("{total_ms}ms");
    }
    let total_secs = total_ms as f64 / 1_000.0;
    if total_secs < 60.0 {
        return format!("{total_secs:.1}s");
    }
    let minutes = (total_secs / 60.0) as u64;
    let seconds = (total_secs - (minutes as f64) * 60.0) as u64;
    format!("{minutes}m{seconds:02}s")
}

fn timestamp_us(event: &BuckEvent) -> Option<u64> {
    event
        .event()
        .timestamp
        .as_ref()
        .map(|ts| (ts.seconds as u64) * 1_000_000 + (ts.nanos as u64) / 1_000)
}

fn duration_to_us(d: &prost_types::Duration) -> u64 {
    (d.seconds.max(0) as u64) * 1_000_000 + (d.nanos.max(0) as u64) / 1_000
}

/// Linear-interpolating percentile on a sorted slice of sample values.
fn percentile(sorted: &[u64], q: f64) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let rank = ((sorted.len() as f64 - 1.0) * q).round() as usize;
    sorted[rank.min(sorted.len() - 1)]
}

fn format_target_label(label: &kuro_data::ConfiguredTargetLabel) -> String {
    let name = label
        .label
        .as_ref()
        .map(|l| format!("//{}:{}", l.package, l.name))
        .unwrap_or_else(|| "<unknown>".to_owned());
    match &label.configuration {
        Some(cfg) => format!("{name} ({})", cfg.full_name),
        None => name,
    }
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use kuro_data::ActionKind;
    use kuro_data::CommandExecutionDetails;
    use kuro_data::CommandExecutionKind;
    use kuro_data::RemoteCommand;
    use kuro_data::SpanEndEvent;
    use kuro_data::SpanStartEvent;
    use kuro_data::command_execution_kind::Command as CmdKind;
    use kuro_events::span::SpanId;
    use kuro_wrapper_common::invocation_id::TraceId;

    use super::*;

    fn span_start(data: kuro_data::span_start_event::Data) -> BuckEvent {
        span_start_at(data, 0)
    }

    fn span_start_at(data: kuro_data::span_start_event::Data, ts_us: u64) -> BuckEvent {
        BuckEvent::new(
            SystemTime::UNIX_EPOCH + std::time::Duration::from_micros(ts_us),
            TraceId::new(),
            Some(SpanId::next()),
            None,
            SpanStartEvent { data: Some(data) }.into(),
        )
    }

    fn span_end(data: kuro_data::span_end_event::Data, duration_us: u64) -> BuckEvent {
        span_end_at(data, duration_us, 0)
    }

    fn span_end_at(
        data: kuro_data::span_end_event::Data,
        duration_us: u64,
        ts_us: u64,
    ) -> BuckEvent {
        BuckEvent::new(
            SystemTime::UNIX_EPOCH + std::time::Duration::from_micros(ts_us),
            TraceId::new(),
            Some(SpanId::next()),
            None,
            SpanEndEvent {
                stats: None,
                duration: Some(prost_types::Duration {
                    seconds: (duration_us / 1_000_000) as i64,
                    nanos: ((duration_us % 1_000_000) * 1000) as i32,
                }),
                data: Some(data),
            }
            .into(),
        )
    }

    fn remote_cache_hit() -> kuro_data::ActionExecutionEnd {
        kuro_data::ActionExecutionEnd {
            kind: ActionKind::Run as i32,
            name: Some(kuro_data::ActionName {
                category: "cxx_compile".to_owned(),
                identifier: "foo.o".to_owned(),
                progress_message: String::new(),
            }),
            commands: vec![kuro_data::CommandExecution {
                details: Some(CommandExecutionDetails {
                    command_kind: Some(CommandExecutionKind {
                        command: Some(CmdKind::RemoteCommand(RemoteCommand {
                            cache_hit: true,
                            ..Default::default()
                        })),
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    #[test]
    fn rollup_tracks_mnemonic_and_cache_hits() {
        let mut b = BuildSummaryBuilder::new();
        for _ in 0..3 {
            b.handle_event(&span_end(
                kuro_data::span_end_event::Data::ActionExecution(Box::new(remote_cache_hit())),
                2_000,
            ));
        }
        let summary = b.finalize();
        assert_eq!(summary.total_action_count, 3);
        assert_eq!(summary.by_mnemonic.len(), 1);
        let row = &summary.by_mnemonic[0];
        assert_eq!(row.category, "cxx_compile");
        assert_eq!(row.count, 3);
        assert_eq!(row.cached, 3);
        assert_eq!(row.total_wall_us, 6_000);
        assert_eq!(summary.cache_hit_pct, 100.0);
    }

    #[test]
    fn rollup_splits_exec_and_queue_wall() {
        // SpanEnd.duration = 5s (total). ActionExecutionEnd.wall_time = 1s
        // (exec-only). So queue_wait = 4s.
        let mut action = remote_cache_hit();
        action.wall_time = Some(prost_types::Duration {
            seconds: 1,
            nanos: 0,
        });
        let mut b = BuildSummaryBuilder::new();
        b.handle_event(&span_end(
            kuro_data::span_end_event::Data::ActionExecution(Box::new(action)),
            5_000_000, // outer span = 5s
        ));
        let summary = b.finalize();
        assert_eq!(summary.by_mnemonic.len(), 1);
        let row = &summary.by_mnemonic[0];
        assert_eq!(row.total_wall_us, 1_000_000, "exec-only wall");
        assert_eq!(row.total_queue_us, 4_000_000, "queue-wait remainder");
    }

    #[test]
    fn rollup_falls_back_to_span_duration_when_wall_time_absent() {
        // Pre-Plan-17.2-first-half event logs don't set wall_time — the
        // aggregator must not zero out total_wall_us on those.
        let mut action = remote_cache_hit();
        action.wall_time = None;
        let mut b = BuildSummaryBuilder::new();
        b.handle_event(&span_end(
            kuro_data::span_end_event::Data::ActionExecution(Box::new(action)),
            2_500_000,
        ));
        let summary = b.finalize();
        assert_eq!(summary.by_mnemonic[0].total_wall_us, 2_500_000);
        assert_eq!(summary.by_mnemonic[0].total_queue_us, 0);
    }

    #[test]
    fn rollup_peak_in_flight_actions() {
        let mut b = BuildSummaryBuilder::new();
        let start = kuro_data::span_start_event::Data::ActionExecution(
            kuro_data::ActionExecutionStart::default(),
        );
        let end = kuro_data::span_end_event::Data::ActionExecution(Box::new(
            kuro_data::ActionExecutionEnd::default(),
        ));

        b.handle_event(&span_start(start.clone()));
        b.handle_event(&span_start(start.clone()));
        b.handle_event(&span_start(start.clone()));
        b.handle_event(&span_end(end.clone(), 1_000));
        b.handle_event(&span_start(start.clone()));
        b.handle_event(&span_end(end, 1_000));

        let summary = b.finalize();
        assert_eq!(summary.peak_in_flight_actions, 3);
    }

    #[test]
    fn rollup_top_n_limits_slowest_actions() {
        let mut b = BuildSummaryBuilder::with_top_n(2);
        for wall_us in [100u64, 500, 300, 50, 900, 700] {
            let mut action = remote_cache_hit();
            action.name.as_mut().unwrap().identifier = format!("action_{wall_us}");
            b.handle_event(&span_end(
                kuro_data::span_end_event::Data::ActionExecution(Box::new(action)),
                wall_us,
            ));
        }
        let summary = b.finalize();
        assert_eq!(summary.slowest_actions.len(), 2);
        assert_eq!(summary.slowest_actions[0].wall_us, 900);
        assert_eq!(summary.slowest_actions[1].wall_us, 700);
    }

    #[test]
    fn short_lines_format() {
        let mut b = BuildSummaryBuilder::new();

        // Load span: starts at t=0, ends at t=1.5s (elapsed = 1.5s).
        b.handle_event(&span_start_at(
            kuro_data::span_start_event::Data::Load(kuro_data::LoadBuildFileStart {
                ..Default::default()
            }),
            0,
        ));
        b.handle_event(&span_end_at(
            kuro_data::span_end_event::Data::Load(kuro_data::LoadBuildFileEnd {
                ..Default::default()
            }),
            1_500_000,
            1_500_000,
        ));

        // Action spans: two overlapping actions. Earliest start t=2s,
        // latest end t=4.3s → elapsed 2.3s.
        let action_start = || {
            kuro_data::span_start_event::Data::ActionExecution(
                kuro_data::ActionExecutionStart::default(),
            )
        };
        b.handle_event(&span_start_at(action_start(), 2_000_000));
        b.handle_event(&span_start_at(action_start(), 2_500_000));
        b.handle_event(&span_end_at(
            kuro_data::span_end_event::Data::ActionExecution(Box::new(remote_cache_hit())),
            2_000_000,
            4_000_000,
        ));
        b.handle_event(&span_end_at(
            kuro_data::span_end_event::Data::ActionExecution(Box::new(remote_cache_hit())),
            1_800_000,
            4_300_000,
        ));

        let summary = b.finalize();
        let lines = summary.short_lines();
        assert!(
            lines.iter().any(|l| l.contains("load=1.5s")),
            "got: {lines:?}"
        );
        assert!(
            lines.iter().any(|l| l.contains("execute=2.3s")),
            "got: {lines:?}"
        );
        assert!(
            lines.iter().any(|l| l.contains("cxx_compile=")),
            "got: {lines:?}"
        );
    }

    #[test]
    fn short_lines_empty_summary() {
        let summary = BuildSummaryBuilder::new().finalize();
        assert!(summary.short_lines().is_empty());
    }

    #[test]
    fn fmt_duration_humanized() {
        assert_eq!(fmt_duration_us(0), "0s");
        assert_eq!(fmt_duration_us(999_000), "999ms");
        assert_eq!(fmt_duration_us(1_500_000), "1.5s");
        assert_eq!(fmt_duration_us(61_000_000), "1m01s");
        assert_eq!(fmt_duration_us(3_665_000_000), "61m05s");
    }

    #[test]
    fn percentile_on_small_inputs() {
        assert_eq!(percentile(&[], 0.5), 0);
        assert_eq!(percentile(&[42], 0.99), 42);
        assert_eq!(percentile(&[1, 2, 3, 4, 5], 0.5), 3);
        assert_eq!(percentile(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10], 0.95), 10);
    }
}
