/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! DICE compute-outcome side channel (Plan 21.1).
//!
//! When `SLUG_LOG_DICE` is set to a path, every key passing through
//! `DiceTaskWorker::do_work` emits one CSV row describing whether the key was
//! a cache hit (direct version match), reused (deps re-checked, unchanged),
//! or a miss (compute function ran). Diffing two back-to-back warm runs
//! isolates keys that are being spuriously invalidated.
//!
//! Disabled by default; zero cost when the env var is unset — the
//! `now_if_enabled()` hot-path is a single relaxed atomic load through the
//! `OnceLock` cache.

use std::fs::File;
use std::io::BufWriter;
use std::io::Write;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::time::Instant;

use crate::impls::key::DiceKeyErased;

static START: OnceLock<Instant> = OnceLock::new();
static WRITER: OnceLock<Option<Mutex<BufWriter<File>>>> = OnceLock::new();

fn writer() -> Option<&'static Mutex<BufWriter<File>>> {
    WRITER
        .get_or_init(|| match std::env::var("SLUG_LOG_DICE") {
            Ok(path) if !path.is_empty() => {
                let file = File::create(&path).ok()?;
                let mut w = BufWriter::new(file);
                let _ = writeln!(w, "key_type,key_display,outcome,start_us,dur_us");
                Some(Mutex::new(w))
            }
            _ => None,
        })
        .as_ref()
}

/// Captures `Instant::now()` iff DICE logging is enabled. Zero cost when
/// disabled — single atomic load through the `OnceLock`.
pub(crate) fn now_if_enabled() -> Option<Instant> {
    if writer().is_some() {
        Some(Instant::now())
    } else {
        None
    }
}

/// Emits a CSV row for one DICE key outcome. No-ops when logging is disabled
/// or when `started` is `None` (the zero-cost path). Safe to call from any
/// thread; writes are serialised by a mutex around the BufWriter.
pub(crate) fn record(key: &DiceKeyErased, outcome: &str, started: Option<Instant>) {
    let Some(started) = started else {
        return;
    };
    let Some(w) = writer() else {
        return;
    };
    let base = *START.get_or_init(|| started);
    let now = Instant::now();
    let start_us = started.saturating_duration_since(base).as_micros();
    let dur_us = now.saturating_duration_since(started).as_micros();
    let key_type = key.key_type_name();
    let display = format!("{key}");
    let display_esc = display.replace('"', "\"\"");
    if let Ok(mut w) = w.lock() {
        let _ = writeln!(
            w,
            "{key_type},\"{display_esc}\",{outcome},{start_us},{dur_us}"
        );
    }
}
