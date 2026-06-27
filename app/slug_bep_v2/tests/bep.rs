/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use slug_bep_v2::BuildEventId;
use slug_bep_v2::BuildEventStream;

#[test]
fn minimal_build_test_stream_has_stable_event_order() {
    let stream =
        BuildEventStream::minimal_build_test("//pkg:probe_test", "bazel-bin/pkg/probe", "PASSED");

    let ids = stream
        .events()
        .iter()
        .map(|event| &event.id)
        .collect::<Vec<_>>();
    assert_eq!(ids[0], &BuildEventId::BuildStarted);
    assert!(matches!(ids[1], BuildEventId::ConfiguredTarget { .. }));
    assert!(matches!(ids[2], BuildEventId::ActionCompleted { .. }));
    assert!(matches!(ids[3], BuildEventId::TestResult { .. }));
    assert_eq!(ids[4], &BuildEventId::BuildFinished);
}

#[test]
fn json_lines_include_configured_action_test_and_finish_payloads() {
    let lines =
        BuildEventStream::minimal_build_test("//pkg:probe_test", "bazel-bin/pkg/probe", "PASSED")
            .to_json_lines();

    assert!(lines.contains("\"kind\":\"configuredTarget\""));
    assert!(lines.contains("\"kind\":\"actionCompleted\""));
    assert!(lines.contains("\"kind\":\"testResult\""));
    assert!(lines.contains("\"kind\":\"finished\""));
    assert!(lines.ends_with('\n'));
}
