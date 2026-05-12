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

use crate::process_stats::process_stats;

const ENV: &str = "SLUG_MEMORY_CHECKPOINTS";

pub fn enabled() -> bool {
    std::env::var_os(ENV).is_some()
}

pub fn checkpoint(name: &'static str, fields: impl IntoIterator<Item = (&'static str, usize)>) {
    if !enabled() {
        return;
    }

    let stats = process_stats();
    let rss = stats
        .rss_bytes
        .map(|v| v.to_string())
        .unwrap_or_else(|| "unknown".to_owned());
    let max_rss = stats
        .max_rss_bytes
        .map(|v| v.to_string())
        .unwrap_or_else(|| "unknown".to_owned());

    let fields = FormatFields {
        fields: fields.into_iter().collect(),
    };

    tracing::warn!(
        target: "slug_memory",
        checkpoint = name,
        rss_bytes = %rss,
        max_rss_bytes = %max_rss,
        "{name}: rss_bytes={rss} max_rss_bytes={max_rss}{fields}"
    );
}

struct FormatFields {
    fields: Vec<(&'static str, usize)>,
}

impl fmt::Display for FormatFields {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (name, value) in &self.fields {
            write!(f, " {name}={value}")?;
        }
        Ok(())
    }
}
