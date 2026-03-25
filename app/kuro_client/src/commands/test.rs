/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use kuro_cli_proto::CounterWithExamples;
use kuro_cli_proto::TestRequest;
use kuro_cli_proto::TestSessionOptions;
use kuro_client_ctx::client_ctx::ClientCommandContext;
use kuro_client_ctx::common::BuckArgMatches;
use kuro_client_ctx::common::CommonBuildConfigurationOptions;
use kuro_client_ctx::common::CommonCommandOptions;
use kuro_client_ctx::common::CommonEventLogOptions;
use kuro_client_ctx::common::CommonStarlarkOptions;
use kuro_client_ctx::common::build::CommonBuildOptions;
use kuro_client_ctx::common::target_cfg::TargetCfgOptions;
use kuro_client_ctx::common::timeout::CommonTimeoutOptions;
use kuro_client_ctx::common::ui::CommonConsoleOptions;
use kuro_client_ctx::daemon::client::BuckdClientConnector;
use kuro_client_ctx::daemon::client::NoPartialResultHandler;
use kuro_client_ctx::events_ctx::EventsCtx;
use kuro_client_ctx::exit_result::ExitResult;
use kuro_client_ctx::final_console::FinalConsole;
use kuro_client_ctx::output_destination_arg::OutputDestinationArg;
use kuro_client_ctx::path_arg::PathArg;
use kuro_client_ctx::stdio::eprint_line;
use kuro_client_ctx::streaming::StreamingCommand;
use kuro_client_ctx::subscribers::subscriber::EventSubscriber;
use kuro_client_ctx::subscribers::superconsole::test::TestCounterColumn;
use kuro_client_ctx::subscribers::superconsole::test::span_from_build_failure_count;
use kuro_error::BuckErrorContext;
use kuro_error::ErrorTag;
use kuro_error::ExitCode;
use kuro_error::kuro_error;
use kuro_event_observer::display::TestOutputMode;
use kuro_event_observer::display::set_test_output_mode;
use kuro_event_observer::unpack_event::unpack_event;
use kuro_fs::fs_util;
use kuro_fs::working_dir::AbsWorkingDir;
use superconsole::Line;
use superconsole::Span;

/// Writes JUnit-compatible XML test results for e2e test framework compatibility.
///
/// The `kuro test --xml <path>` flag triggers collection of test result events
/// and writes them to the specified XML file when finalized.
struct XmlTestResultWriter {
    xml_path: PathBuf,
    results: Vec<(String, String, String)>, // (name, status, result_type)
}

impl XmlTestResultWriter {
    fn new(xml_path: impl Into<PathBuf>) -> Self {
        Self {
            xml_path: xml_path.into(),
            results: Vec::new(),
        }
    }
}

#[async_trait]
impl EventSubscriber for XmlTestResultWriter {
    fn name(&self) -> &'static str {
        "xml-test-result-writer"
    }

    async fn handle_events(
        &mut self,
        events: &[Arc<kuro_events::BuckEvent>],
    ) -> kuro_error::Result<()> {
        for event in events {
            if let Ok(kuro_event_observer::unpack_event::UnpackedBuckEvent::Instant(
                _,
                _,
                kuro_data::instant_event::Data::TestResult(result),
            )) = unpack_event(event)
            {
                let status_enum = kuro_data::TestStatus::try_from(result.status)
                    .unwrap_or(kuro_data::TestStatus::Unknown);
                // Listing results are metadata, not test executions — skip them.
                if matches!(
                    status_enum,
                    kuro_data::TestStatus::ListingSuccess | kuro_data::TestStatus::ListingFailed
                ) {
                    continue;
                }
                let (status_str, type_str) = match status_enum {
                    kuro_data::TestStatus::Pass => ("pass", "SUCCESS"),
                    kuro_data::TestStatus::Fail => ("fail", "FAILURE"),
                    kuro_data::TestStatus::Fatal => ("fail", "FAILURE"),
                    kuro_data::TestStatus::Timeout => ("fail", "FAILURE"),
                    kuro_data::TestStatus::InfraFailure => ("fail", "FAILURE"),
                    kuro_data::TestStatus::Skip => ("skip", "EXCLUDED"),
                    kuro_data::TestStatus::Omitted => ("skip", "EXCLUDED"),
                    _ => ("fail", "FAILURE"),
                };
                let name = result
                    .name
                    .replace('&', "&amp;")
                    .replace('<', "&lt;")
                    .replace('>', "&gt;")
                    .replace('"', "&quot;");
                self.results
                    .push((name, status_str.to_owned(), type_str.to_owned()));
            }
        }
        Ok(())
    }

    async fn finalize(&mut self) -> kuro_error::Result<()> {
        if self.results.is_empty() {
            // Write an empty-but-valid XML file so the test framework doesn't error.
            let xml = "<results>\n  <testsuite/>\n</results>\n";
            std::fs::write(&self.xml_path, xml)
                .buck_error_context("Failed to write XML test results")?;
            return Ok(());
        }
        let mut xml = String::from("<results>\n  <testsuite>\n");
        for (name, status, result_type) in &self.results {
            xml.push_str(&format!(
                "    <testresult name=\"{}\" status=\"{}\" type=\"{}\"/>\n",
                name, status, result_type
            ));
        }
        xml.push_str("  </testsuite>\n</results>\n");
        std::fs::write(&self.xml_path, xml)
            .buck_error_context("Failed to write XML test results")?;
        Ok(())
    }
}

use crate::commands::build::print_build_result;

fn forward_output_to_path(
    output: &str,
    path_arg: &PathArg,
    working_dir: &AbsWorkingDir,
) -> kuro_error::Result<()> {
    fs_util::write(path_arg.resolve(working_dir), output)
        .buck_error_context("Failed to write test executor output to path")
}

fn print_error_counter(
    console: &FinalConsole,
    counter: &CounterWithExamples,
    error_type: &str,
    symbol: &str,
) -> kuro_error::Result<()> {
    if counter.count > 0 {
        console.print_error(&format!("{} {}", counter.count, error_type))?;
        for test_name in &counter.example_tests {
            console.print_error(&format!("  {symbol} {test_name}"))?;
        }
        if counter.count > counter.max {
            console.print_error(&format!(
                "  ...and {} more not shown...",
                counter.count - counter.max
            ))?;
        }
    }
    Ok(())
}
#[derive(Debug, clap::Parser)]
#[clap(name = "test", about = "Build and test the specified targets")]
pub struct TestCommand {
    #[clap(
        long = "exclude",
        num_args = 1..,
        help = "Labels on targets to exclude from tests"
    )]
    exclude: Vec<String>,

    #[clap(
        long = "include",
        alias = "labels",
        help = "Labels on targets to include from tests. Prefixing with `!` means to exclude. First match wins unless overridden by `always-exclude` flag.\n\
If include patterns are present, regardless of whether exclude patterns are present, then all targets are by default excluded unless explicitly included.",
        num_args=1..,
    )]
    include: Vec<String>,

    #[clap(
        long = "test_tag_filters",
        alias = "test-tag-filters",
        value_name = "TAGS",
        help = "Comma-separated list of test tags to filter on (Bazel compatibility). \
Positive tags include only matching tests; prefix with '-' to exclude. \
Example: --test_tag_filters=small,-slow (include 'small', exclude 'slow')"
    )]
    test_tag_filters: Option<String>,

    #[clap(
        long = "always-exclude",
        alias = "always_exclude",
        help = "Whether to always exclude if the label appears in `exclude`, regardless of which appears first"
    )]
    always_exclude: bool,

    #[clap(
        long = "build-filtered",
        help = "Whether to build tests that are excluded via labels."
    )]
    build_filtered_targets: bool, // TODO(bobyf) this flag should always override the buckconfig option when we use it

    /// Will allow tests that are compatible with RE (setup to run from the repo root and
    /// use relative paths) to run from RE.
    #[clap(long, group = "re_options", alias = "unstable-allow-tests-on-re")]
    unstable_allow_compatible_tests_on_re: bool,

    /// Will run tests to on RE even if they are missing required settings (running from the root +
    /// relative paths). Those required settings just get overridden.
    #[clap(long, group = "re_options", alias = "unstable-force-tests-on-re")]
    unstable_allow_all_tests_on_re: bool,

    #[clap(name = "TARGET_PATTERNS", help = "Patterns to test", value_hint = clap::ValueHint::Other)]
    patterns: Vec<String>,

    /// Writes the test executor stdout to the provided path
    ///
    /// --test-executor-stdout=- will write to stdout
    ///
    /// --test-executor-stdout=FILEPATH will write to the provided filepath, overwriting the current
    /// file if it exists
    ///
    /// By default the test executor's stdout stream is captured
    #[clap(long)]
    test_executor_stdout: Option<OutputDestinationArg>,

    /// Normally testing will follow the `tests` attribute of all targets, to find their associated tests.
    /// When passed, this flag will disable that, and only run the directly supplied targets.
    #[clap(long)]
    ignore_tests_attribute: bool,

    /// Writes the test executor stderr to the provided path
    ///
    /// --test-executor-stderr=- will write to stderr
    ///
    /// --test-executor-stderr=FILEPATH will write to the provided filepath, overwriting the current
    /// file if it exists
    ///
    /// By default test executor's stderr stream is captured
    #[clap(long)]
    test_executor_stderr: Option<OutputDestinationArg>,

    /// Filter tests matching a regex pattern (Bazel compatibility).
    ///
    /// Sets TESTBRIDGE_TEST_ONLY=<pattern> in the test environment. Test
    /// frameworks (e.g., gtest, pytest, cargo test) read this variable to
    /// filter which tests to run. Equivalent to Bazel's --test_filter flag.
    ///
    /// Examples:
    ///   kuro test //foo:bar --test_filter=MyClass.TestMethod
    ///   kuro test //foo:bar --test_filter=test_
    #[clap(long = "test_filter", alias = "test-filter", value_name = "PATTERN")]
    test_filter: Option<String>,

    /// Additional arguments passed to the test executor.
    ///
    /// Test executor is expected to have `--env` flag to pass environment variables.
    /// Can be used like this:
    ///
    /// kuro test //foo:bar -- --env PRIVATE_KEY=123
    #[clap(name = "TEST_EXECUTOR_ARGS", raw = true)]
    test_executor_args: Vec<String>,

    /// Also build DefaultInfo provider, which is what `kuro build` command builds (this is not the default)
    #[clap(long, group = "default-info")]
    build_default_info: bool,

    /// Do not build DefaultInfo provider (this is the default)
    #[allow(unused)]
    #[clap(long, group = "default-info")]
    skip_default_info: bool,

    /// Also build RunInfo provider, which builds artifacts needed for `kuro run` (this is not the default)
    #[clap(long, group = "run-info")]
    build_run_info: bool,

    /// Do not build RunInfo provider (this is the default)
    #[allow(unused)]
    #[clap(long, group = "run-info")]
    skip_run_info: bool,

    /// This option does nothing. It is here to keep compatibility with Buck1 and ci
    #[clap(long = "deep", hide = true)]
    _deep: bool,

    /// Write test results to XML file (JUnit format for e2e test framework compatibility).
    #[clap(long = "xml", hide = true)]
    xml: Option<String>,

    // ---- Bazel compatibility flags (accepted, some are no-ops) ----
    /// Control test output verbosity (Bazel compatibility).
    ///
    /// Bazel's --test_output controls whether test stdout/stderr is shown:
    /// - `summary`: Show only test status and timing (default)
    /// - `errors`: Show output for failed tests only
    /// - `all`: Show output for all tests
    /// - `short`: Show first few lines for failed tests
    /// - `streamed`: Stream all output in real-time
    ///
    #[clap(long = "test-output", alias = "test_output", value_name = "MODE")]
    test_output: Option<String>,

    /// Control test summary format (Bazel compatibility).
    ///
    /// Accepted for compatibility with Bazel's --test_summary flag.
    #[clap(
        long = "test-summary",
        alias = "test_summary",
        hide = true,
        value_name = "FORMAT"
    )]
    test_summary: Option<String>,

    /// Per-test timeout in seconds (Bazel compatibility).
    ///
    /// Accepted for compatibility with Bazel's --test_timeout flag.
    /// Sets the maximum time each test is allowed to run.
    #[clap(
        long = "test-timeout",
        alias = "test_timeout",
        hide = true,
        value_name = "SECONDS"
    )]
    test_timeout: Option<u32>,

    #[clap(flatten)]
    build_opts: CommonBuildOptions,

    #[clap(flatten)]
    target_cfg: TargetCfgOptions,

    #[clap(flatten)]
    timeout_options: CommonTimeoutOptions,

    #[clap(flatten)]
    common_opts: CommonCommandOptions,
}

#[async_trait(?Send)]
impl StreamingCommand for TestCommand {
    const COMMAND_NAME: &'static str = "test";

    async fn exec_impl(
        self,
        buckd: &mut BuckdClientConnector,
        matches: BuckArgMatches<'_>,
        ctx: &mut ClientCommandContext<'_>,
        events_ctx: &mut EventsCtx,
    ) -> ExitResult {
        // Set the test output mode so display formatting respects --test_output.
        let output_mode = self
            .test_output
            .as_deref()
            .map(TestOutputMode::from_str)
            .unwrap_or(TestOutputMode::Errors);
        set_test_output_mode(output_mode);

        let context = ctx.client_context(matches, &self)?;

        // Bazel --test_tag_filters=tag1,tag2,-tag3 compat: split positive/negative entries.
        let mut excluded_labels = self.exclude;
        let mut included_labels = self.include;
        if let Some(tag_filters) = self.test_tag_filters {
            for tag in tag_filters.split(',') {
                let tag = tag.trim();
                if tag.is_empty() {
                    continue;
                }
                if let Some(neg) = tag.strip_prefix('-') {
                    excluded_labels.push(neg.to_owned());
                } else {
                    included_labels.push(tag.to_owned());
                }
            }
        }

        // Bazel --test_filter=REGEX compat: set TESTBRIDGE_TEST_ONLY env var so test
        // frameworks can filter individual test cases. We inject it as --env to the
        // test executor so the internal test runner passes it to the test binary.
        let mut test_executor_args = self.test_executor_args;
        if let Some(filter) = self.test_filter {
            test_executor_args.insert(0, "--env".to_owned());
            test_executor_args.insert(1, format!("TESTBRIDGE_TEST_ONLY={filter}"));
        }

        let response = buckd
            .with_flushing()
            .test(
                TestRequest {
                    context: Some(context),
                    target_patterns: self.patterns.clone(),
                    target_cfg: Some(self.target_cfg.target_cfg()),
                    test_executor_args,
                    excluded_labels,
                    included_labels,
                    always_exclude: self.always_exclude,
                    build_filtered_targets: self.build_filtered_targets,
                    // we don't currently have a different flag for this, so just use the build one.
                    concurrency: self.build_opts.num_threads.unwrap_or(0),
                    build_opts: Some(self.build_opts.to_proto()),
                    session_options: Some(TestSessionOptions {
                        allow_re: self.unstable_allow_compatible_tests_on_re
                            || self.unstable_allow_all_tests_on_re,
                        force_use_project_relative_paths: self.unstable_allow_all_tests_on_re,
                        force_run_from_project_root: self.unstable_allow_all_tests_on_re,
                    }),
                    timeout: self.timeout_options.overall_timeout()?,
                    ignore_tests_attribute: self.ignore_tests_attribute,
                    build_default_info: self.build_default_info,
                    build_run_info: self.build_run_info,
                    test_output_mode: self.test_output.clone().unwrap_or_default(),
                },
                events_ctx,
                ctx.console_interaction_stream(&self.common_opts.console_opts),
                &mut NoPartialResultHandler,
            )
            .await??;

        let statuses = response
            .test_statuses
            .as_ref()
            .expect("Daemon to not return empty statuses");

        let listing_failed = statuses
            .listing_failed
            .as_ref()
            .buck_error_context("Missing `listing_failed`")?;
        let passed = statuses
            .passed
            .as_ref()
            .buck_error_context("Missing `passed`")?;
        let failed = statuses
            .failed
            .as_ref()
            .buck_error_context("Missing `failed`")?;
        let fatals = statuses
            .fatals
            .as_ref()
            .buck_error_context("Missing `fatals`")?;
        let skipped = statuses
            .skipped
            .as_ref()
            .buck_error_context("Missing `skipped`")?;
        let omitted = statuses
            .omitted
            .as_ref()
            .buck_error_context("Missing `omitted`")?;
        let infra_failure = statuses
            .infra_failure
            .as_ref()
            .buck_error_context("Missing `infra failure`")?;

        let console = self.common_opts.console_opts.final_console();
        print_build_result(&console, &response.errors)?;

        if statuses.build_errors != 0 {
            console.print_error(&format!("{} BUILDS FAILED", statuses.build_errors))?;
        }

        let mut line = Line::default();
        line.push(Span::new_unstyled_lossy("Tests finished: "));
        if listing_failed.count > 0 {
            line.push(TestCounterColumn::LISTING_FAIL.to_span_from_test_statuses(statuses)?);
            line.push(Span::new_unstyled_lossy(". "));
        }
        let columns = [
            TestCounterColumn::PASS,
            TestCounterColumn::FAIL,
            TestCounterColumn::FATAL,
            TestCounterColumn::SKIP,
            TestCounterColumn::OMIT,
            TestCounterColumn::INFRA_FAILURE,
        ];
        for column in columns {
            line.push(column.to_span_from_test_statuses(statuses)?);
            line.push(Span::new_unstyled_lossy(". "));
        }
        line.push(span_from_build_failure_count(statuses.build_errors)?);
        eprint_line(&line)?;

        print_error_counter(&console, listing_failed, "LISTINGS FAILED", "⚠")?;
        print_error_counter(&console, failed, "TESTS FAILED", "✗")?;
        print_error_counter(&console, fatals, "TESTS FATALS", "⚠")?;
        print_error_counter(&console, infra_failure, "TESTS Infra Failed", "🛠")?;

        if passed.count
            + failed.count
            + fatals.count
            + skipped.count
            + omitted.count
            + infra_failure.count
            == 0
        {
            console.print_warning("NO TESTS RAN")?;
        }

        let info_messages = response.executor_info_messages;
        for message in info_messages {
            console.print_stderr(message.as_str())?;
        }

        match self.test_executor_stderr {
            Some(OutputDestinationArg::Path(path)) => {
                forward_output_to_path(&response.executor_stderr, &path, &ctx.working_dir)?;
            }
            Some(OutputDestinationArg::Stream) => {
                console.print_error(&response.executor_stderr)?;
            }
            None => {}
        }

        if let Some(build_report) = response.serialized_build_report {
            kuro_client_ctx::println!("{}", build_report)?;
        }

        let exit_result = if let Some(exit_code) = response.exit_code {
            // If exit code is set in response, it should be used and not derived from command errors.
            let exit_code = if let Ok(code) = exit_code.try_into() {
                match code {
                    0 => ExitCode::Success,
                    _ => ExitCode::TestRunner(code),
                }
            } else {
                // The exit code isn't an allowable value, so just switch to generic failure
                ExitCode::UnknownFailure
            };
            ExitResult::status_with_emitted_errors(exit_code, response.errors)
        } else if !response.errors.is_empty() {
            // If we had build errors return their exit code.
            ExitResult::from_command_result_errors(response.errors)
        } else {
            // But if we had no build errors, and Tpx did not provide an exit code, then that's
            // going to be an error.
            kuro_error!(
                ErrorTag::TestExecutor,
                "Test executor did not provide an exit code"
            )
            .into()
        };

        match self.test_executor_stdout {
            Some(OutputDestinationArg::Path(path)) => {
                forward_output_to_path(&response.executor_stdout, &path, &ctx.working_dir)?;
                exit_result
            }
            Some(OutputDestinationArg::Stream) => {
                exit_result.with_stdout(response.executor_stdout.into_bytes())
            }
            _ => exit_result,
        }
    }

    fn console_opts(&self) -> &CommonConsoleOptions {
        &self.common_opts.console_opts
    }

    fn event_log_opts(&self) -> &CommonEventLogOptions {
        &self.common_opts.event_log_opts
    }

    fn build_config_opts(&self) -> &CommonBuildConfigurationOptions {
        &self.common_opts.config_opts
    }

    fn starlark_opts(&self) -> &CommonStarlarkOptions {
        &self.common_opts.starlark_opts
    }

    fn extra_subscribers(&self) -> Vec<Box<dyn EventSubscriber>> {
        if let Some(xml_path) = &self.xml {
            vec![Box::new(XmlTestResultWriter::new(xml_path))]
        } else {
            vec![]
        }
    }
}
