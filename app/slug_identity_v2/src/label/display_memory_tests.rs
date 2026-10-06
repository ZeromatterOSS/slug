/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory.
 * You may select, at your option, one of the above-listed licenses.
 */

use std::fmt;

use super::ApparentLabel;

struct SourceSink<'a> {
    repository: &'a str,
    shared: bool,
    output: String,
}

impl fmt::Write for SourceSink<'_> {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        if let Some(index) = value.find(self.repository) {
            self.shared |= value[index..].as_ptr() == self.repository.as_ptr();
        }
        self.output.push_str(value);
        Ok(())
    }
}

#[test]
fn nonroot_repository_display_uses_original_source_bytes() {
    let repository = format!("repository-{}", "owner".repeat(4096));
    let expected = format!("@{repository}//pkg:target");
    let label = ApparentLabel::parse(&expected).unwrap();
    let mut sink = SourceSink {
        repository: label.repo().as_str(),
        shared: false,
        output: String::new(),
    };
    fmt::write(&mut sink, format_args!("{label}")).unwrap();
    assert_eq!(sink.output, expected);
    println!(
        "LABEL_DISPLAY repository_bytes={} shared={}",
        repository.len(),
        sink.shared
    );
    assert!(sink.shared);
}

#[test]
fn display_preserves_root_unicode_flags_and_owned_results() {
    let cases = [
        ("//pkg:target", "//pkg:target"),
        ("@//pkg:target", "//pkg:target"),
        ("//:target", "//:target"),
        ("@repo//:target", "@repo//:target"),
        ("@repo+part~name//pkg:世界", "@repo+part~name//pkg:世界"),
    ];
    let outputs = cases.map(|(input, expected)| {
        let label = ApparentLabel::parse(input).unwrap();
        assert_eq!(format!("{label}"), expected);
        assert_eq!(format!("{label:>60.3}"), expected);
        assert_eq!(format!("{label:+#}"), expected);
        format!("prefix<{label}>suffix")
    });
    assert_eq!(
        outputs,
        cases.map(|(_, expected)| format!("prefix<{expected}>suffix"))
    );
}

struct BudgetSink {
    budget: usize,
    output: String,
    failed: bool,
    writes_after_error: usize,
}

impl fmt::Write for BudgetSink {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        if self.failed {
            self.writes_after_error += 1;
            return Err(fmt::Error);
        }
        let available = self.budget - self.output.len();
        if value.len() > available {
            // Error fixtures are ASCII so every byte budget is a character boundary.
            self.output.push_str(&value[..available]);
            self.failed = true;
            Err(fmt::Error)
        } else {
            self.output.push_str(value);
            Ok(())
        }
    }
}

#[test]
fn display_propagates_error_at_every_output_prefix_without_later_writes() {
    for expected in ["//pkg:target", "@repo//pkg:target"] {
        let label = ApparentLabel::parse(expected).unwrap();
        for budget in 0..expected.len() {
            let mut sink = BudgetSink {
                budget,
                output: String::new(),
                failed: false,
                writes_after_error: 0,
            };
            assert!(fmt::write(&mut sink, format_args!("{label}")).is_err());
            assert_eq!(sink.output, expected[..budget]);
            assert_eq!(sink.writes_after_error, 0);
        }
    }
}
