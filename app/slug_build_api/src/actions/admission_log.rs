/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Scheduler admission-latency side channel (Plan 17.2-instrument).
//!
//! When the `SLUG_LOG_ADMISSION` env var is set to a path, every action
//! execution emits one CSV row to that file capturing when the action's
//! inputs resolved (`ready`) and when the build-api layer handed the
//! action to the executor (`start`). The delta approximates admission
//! latency — how long a ready action waits before slug's scheduler
//! dispatches it.
//!
//! Disabled by default; zero cost when the env var is unset.

use std::fs::File;
use std::io::BufWriter;
use std::io::Write;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::time::Instant;

static START: OnceLock<Instant> = OnceLock::new();
static WRITER: OnceLock<Option<Mutex<BufWriter<File>>>> = OnceLock::new();

fn writer() -> Option<&'static Mutex<BufWriter<File>>> {
    WRITER
        .get_or_init(|| match std::env::var("SLUG_LOG_ADMISSION") {
            Ok(path) if !path.is_empty() => {
                let file = File::create(&path).ok()?;
                let mut w = BufWriter::new(file);
                // Header. Micros since first `record` call.
                let _ = writeln!(w, "category,action_key,ready_us,start_us,delta_us");
                Some(Mutex::new(w))
            }
            _ => None,
        })
        .as_ref()
}

/// Captures `Instant::now()` iff admission logging is enabled. Zero cost
/// when disabled — the two `Instant::now()` calls on the hot path are gated
/// by a single atomic load through the OnceLock.
pub fn now_if_enabled() -> Option<Instant> {
    if writer().is_some() {
        Some(Instant::now())
    } else {
        None
    }
}

/// Emits a CSV row for an action's admission window. `ready` is when inputs
/// resolved, `start` is when the build-api layer handed off to the
/// executor. Both are `Option` so call sites can thread the same
/// `now_if_enabled()` value without branching.
pub fn record(category: &str, action_key: &str, ready: Option<Instant>, start: Option<Instant>) {
    let (Some(ready), Some(start)) = (ready, start) else {
        return;
    };
    let Some(w) = writer() else {
        return;
    };
    let base = *START.get_or_init(|| ready);
    let ready_us = ready.saturating_duration_since(base).as_micros();
    let start_us = start.saturating_duration_since(base).as_micros();
    let delta_us = start.saturating_duration_since(ready).as_micros();
    // CSV-escape commas in action_key by wrapping in quotes.
    let escaped = action_key.replace('"', "\"\"");
    if let Ok(mut w) = w.lock() {
        let _ = writeln!(
            w,
            "{category},\"{escaped}\",{ready_us},{start_us},{delta_us}"
        );
    }
}
