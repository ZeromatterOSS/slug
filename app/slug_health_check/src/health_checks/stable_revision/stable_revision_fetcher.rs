/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

#![cfg_attr(not(fbcode_build), allow(dead_code))] // TODO(slug-bazel-healthcheck): Remove after GraphQL-backed stable revision fetch is enabled in OSS.

use crate::health_checks::stable_revision::bookmark_revision_fetcher::BookmarkRevisionFetcher;
use crate::health_checks::stable_revision::historical_stables_client::get_recent_revisions_for_bookmark;

pub struct StableRevisionFetcher {
    fb: fbinit::FacebookInit,
}

#[async_trait::async_trait]
impl BookmarkRevisionFetcher for StableRevisionFetcher {
    async fn get_recent_revisions_for_bookmark(
        &self,
        bookmark: &str,
        lookback_hours: u64,
    ) -> slug_error::Result<Option<Vec<String>>> {
        get_recent_revisions_for_bookmark(self.fb, bookmark, lookback_hours).await
    }
}

impl StableRevisionFetcher {
    pub fn new() -> Self {
        Self {
            // This should have already been initialized by the slug client.
            fb: slug_common::fbinit::get_or_init_fbcode_globals(),
        }
    }
}
