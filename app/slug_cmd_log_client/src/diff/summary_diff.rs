/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! `slug log diff summary`: rollup-vs-rollup delta between two builds.
//!
//! Reads two event logs, runs each through [`BuildSummaryBuilder`], and
//! prints a side-by-side table with absolute + percentage deltas. Plan
//! 16.4 regression gate: rows with |Δ%| over a threshold are flagged with
//! `!`; `--fail-on-regression` exits non-zero if any are flagged.

use futures::TryStreamExt;
use slug_client_ctx::client_ctx::BuckSubcommand;
use slug_client_ctx::client_ctx::ClientCommandContext;
use slug_client_ctx::common::BuckArgMatches;
use slug_client_ctx::events_ctx::EventsCtx;
use slug_client_ctx::exit_result::ExitResult;
use slug_error::ErrorTag;
use slug_event_log::read::EventLogPathBuf;
use slug_event_log::stream_value::StreamValue;
use slug_event_observer::build_summary::BuildSummary;
use slug_event_observer::build_summary::BuildSummaryBuilder;
use slug_events::BuckEvent;

use crate::diff::diff_options::DiffEventLogOptions;

/// Diff two build rollups. Metrics above `--threshold` (default 5%) are
/// flagged with `!`; `--fail-on-regression` exits non-zero when any
/// `total_wall_us` / `*_wall_us` / `*_count` row is flagged as a
/// regression (positive delta = worse).
#[derive(Debug, clap::Parser)]
pub struct SummaryDiffCommand {
    #[clap(flatten)]
    diff_event_log: DiffEventLogOptions,

    /// Percentage delta above which a row is flagged.
    #[clap(long, default_value_t = 5.0)]
    threshold: f64,

    /// Exit with code 1 if any flagged row represents a regression.
    #[clap(long)]
    fail_on_regression: bool,

    /// Keep this many rows in slowest-action / slowest-analysis lists
    /// while building the rollup.
    #[clap(long, default_value_t = 10)]
    top_n: usize,
}

impl BuckSubcommand for SummaryDiffCommand {
    const COMMAND_NAME: &'static str = "log-diff-summary";

    async fn exec_impl(
        self,
        _matches: BuckArgMatches<'_>,
        ctx: ClientCommandContext<'_>,
        _events_ctx: &mut EventsCtx,
    ) -> ExitResult {
        let (path1, path2) = self.diff_event_log.get(&ctx).await?;
        let before = rollup_log(&path1, self.top_n).await?;
        let after = rollup_log(&path2, self.top_n).await?;

        let mut had_regression = false;
        slug_client_ctx::println!(
            "{}",
            format_diff(&before, &after, self.threshold, &mut had_regression)
        )?;

        if self.fail_on_regression && had_regression {
            ExitResult::err(slug_error::slug_error!(
                ErrorTag::Bail,
                "Detected regressions above {}% threshold",
                self.threshold
            ))
        } else {
            ExitResult::success()
        }
    }
}

async fn rollup_log(path: &EventLogPathBuf, top_n: usize) -> slug_error::Result<BuildSummary> {
    let (_invocation, mut events) = path.unpack_stream().await?;
    let mut builder = BuildSummaryBuilder::with_top_n(top_n);
    while let Some(event) = events.try_next().await? {
        if let StreamValue::Event(proto) = event {
            if let Ok(wrapped) = BuckEvent::try_from(proto) {
                builder.handle_event(&wrapped);
            }
        }
    }
    Ok(builder.finalize())
}

fn format_diff(
    before: &BuildSummary,
    after: &BuildSummary,
    threshold: f64,
    had_regression: &mut bool,
) -> String {
    let mut s = String::new();

    s.push_str(&format!(
        "{:<28} {:>14} {:>14} {:>12} {:>8}\n",
        "metric", "before", "after", "Δ", "Δ%"
    ));
    s.push_str(&format!("{:-<80}\n", ""));

    let mut emit = |metric: &str, b: u64, a: u64, higher_is_worse: bool| {
        let line = diff_row(metric, b, a, threshold, higher_is_worse);
        if line.regression {
            *had_regression = true;
        }
        s.push_str(&line.text);
    };

    emit(
        "total_wall_us",
        before.total_wall_us,
        after.total_wall_us,
        true,
    );
    emit(
        "load_wall_us",
        before.load_wall_us,
        after.load_wall_us,
        true,
    );
    emit(
        "analyze_wall_us",
        before.analyze_wall_us,
        after.analyze_wall_us,
        true,
    );
    emit(
        "execute_wall_us",
        before.execute_wall_us,
        after.execute_wall_us,
        true,
    );
    emit(
        "materialize_wall_us",
        before.materialize_wall_us,
        after.materialize_wall_us,
        true,
    );
    emit(
        "critical_path_wall_us",
        before.critical_path_wall_us,
        after.critical_path_wall_us,
        true,
    );
    emit(
        "slowest_path_wall_us",
        before.slowest_path_wall_us,
        after.slowest_path_wall_us,
        true,
    );
    emit(
        "total_action_count",
        before.total_action_count,
        after.total_action_count,
        false,
    );
    emit(
        "peak_in_flight_actions",
        before.peak_in_flight_actions as u64,
        after.peak_in_flight_actions as u64,
        false,
    );
    emit(
        "num_dice_nodes",
        before.num_dice_nodes,
        after.num_dice_nodes,
        false,
    );
    emit(
        "num_dice_edges",
        before.num_dice_edges,
        after.num_dice_edges,
        false,
    );
    emit(
        "action_graph_size",
        before.action_graph_size,
        after.action_graph_size,
        false,
    );

    // cache_hit_pct is the one pct-valued metric — render inline.
    s.push_str(&diff_pct_row(
        "cache_hit_pct",
        before.cache_hit_pct,
        after.cache_hit_pct,
    ));

    // Per-mnemonic: union of keys, sorted by total_wall_us delta magnitude.
    let mut categories: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    for row in &before.by_mnemonic {
        categories.insert(row.category.as_str());
    }
    for row in &after.by_mnemonic {
        categories.insert(row.category.as_str());
    }

    if !categories.is_empty() {
        s.push_str("\nby_mnemonic\n");
        for category in categories {
            let b = before.by_mnemonic.iter().find(|r| r.category == category);
            let a = after.by_mnemonic.iter().find(|r| r.category == category);
            s.push_str(&format!("  {category}\n"));
            let b_count = b.map(|r| r.count).unwrap_or(0);
            let a_count = a.map(|r| r.count).unwrap_or(0);
            let b_total = b.map(|r| r.total_wall_us).unwrap_or(0);
            let a_total = a.map(|r| r.total_wall_us).unwrap_or(0);
            let b_p95 = b.map(|r| r.p95_us).unwrap_or(0);
            let a_p95 = a.map(|r| r.p95_us).unwrap_or(0);

            let mut sub = |metric: &str, b: u64, a: u64, hiw: bool| {
                let line = diff_row(&format!("    {metric}"), b, a, threshold, hiw);
                if line.regression {
                    *had_regression = true;
                }
                line.text
            };
            s.push_str(&sub("count", b_count, a_count, false));
            s.push_str(&sub("total_wall_us", b_total, a_total, true));
            s.push_str(&sub("p95_us", b_p95, a_p95, true));
        }
    }

    s
}

struct DiffLine {
    text: String,
    regression: bool,
}

fn diff_row(
    metric: &str,
    before: u64,
    after: u64,
    threshold: f64,
    higher_is_worse: bool,
) -> DiffLine {
    let delta = after as i128 - before as i128;
    let pct = if before == 0 {
        if after == 0 { 0.0 } else { f64::INFINITY }
    } else {
        (delta as f64 / before as f64) * 100.0
    };
    let over_threshold = pct.abs() > threshold;
    let regression = over_threshold
        && pct.is_finite()
        && ((higher_is_worse && delta > 0) || (!higher_is_worse && delta < 0));
    let flag = if over_threshold { "!" } else { " " };
    let pct_str = if pct.is_finite() {
        format!("{:+.1}%", pct)
    } else {
        "  inf".to_owned()
    };
    DiffLine {
        text: format!("{flag}{metric:<27} {before:>14} {after:>14} {delta:>+12} {pct_str:>8}\n"),
        regression,
    }
}

fn diff_pct_row(metric: &str, before_pct: f64, after_pct: f64) -> String {
    let delta = after_pct - before_pct;
    format!(" {metric:<27} {before_pct:>13.1}% {after_pct:>13.1}% {delta:>+12.1}    -\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_row_flags_regression() {
        let line = diff_row("x", 100, 120, 5.0, true);
        assert!(line.regression, "20% increase should flag: {}", line.text);
        assert!(line.text.starts_with('!'));
    }

    #[test]
    fn diff_row_small_change_is_quiet() {
        let line = diff_row("x", 100, 103, 5.0, true);
        assert!(!line.regression);
        assert!(line.text.starts_with(' '));
    }

    #[test]
    fn diff_row_improvement_is_not_regression() {
        let line = diff_row("x", 100, 80, 5.0, true);
        assert!(
            !line.regression,
            "improvement not a regression: {}",
            line.text
        );
        assert!(line.text.starts_with('!'));
    }

    #[test]
    fn diff_row_lower_is_worse_direction() {
        // total_action_count going DOWN may be suspicious.
        let line = diff_row("x", 100, 50, 5.0, false);
        assert!(line.regression);
    }
}
