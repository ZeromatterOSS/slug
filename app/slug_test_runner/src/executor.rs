/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use futures::channel::mpsc::UnboundedSender;
use slug_test_api::data::ExternalRunnerSpec;
use slug_test_api::protocol::TestExecutor;

pub type SpecSender = UnboundedSender<ExternalRunnerSpec>;

pub struct SlugTestExecutor {
    pub sender: SpecSender,
}

impl SlugTestExecutor {
    pub fn new(sender: SpecSender) -> Self {
        Self { sender }
    }
}

#[async_trait::async_trait]
impl TestExecutor for SlugTestExecutor {
    async fn external_runner_spec(&self, spec: ExternalRunnerSpec) -> slug_error::Result<()> {
        self.sender
            .clone()
            .start_send(spec)
            .expect("Sending to not fail if all core invariants are held.");
        Ok(())
    }

    async fn end_of_test_requests(&self) -> slug_error::Result<()> {
        // This ensures that all senders are dropped so the receiver can terminate
        self.sender.close_channel();
        Ok(())
    }
}
