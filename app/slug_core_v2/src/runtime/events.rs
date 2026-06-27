/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventRecord {
    pub kind: &'static str,
    pub message: String,
}

pub trait EventSink {
    fn emit(&mut self, event: EventRecord);
}

#[derive(Debug, Default)]
pub struct VecEventSink {
    events: Vec<EventRecord>,
}

impl VecEventSink {
    pub fn events(&self) -> &[EventRecord] {
        &self.events
    }
}

impl EventSink for VecEventSink {
    fn emit(&mut self, event: EventRecord) {
        self.events.push(event);
    }
}
