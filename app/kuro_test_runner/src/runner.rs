/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::time::Duration;

use clap::Parser;
use futures::StreamExt;
use futures::TryStreamExt;
use futures::channel::mpsc::UnboundedReceiver;
use host_sharing::HostSharingRequirements;
use kuro_error::BuckErrorContext;
use kuro_test_api::data::ArgValue;
use kuro_test_api::data::ArgValueContent;
use kuro_test_api::data::ConfiguredTargetHandle;
use kuro_test_api::data::ExecuteResponse;
use kuro_test_api::data::ExecutionResult2;
use kuro_test_api::data::ExecutionStatus;
use kuro_test_api::data::ExecutionStream;
use kuro_test_api::data::ExternalRunnerSpec;
use kuro_test_api::data::ExternalRunnerSpecValue;
use kuro_test_api::data::RequiredLocalResources;
use kuro_test_api::data::TestResult;
use kuro_test_api::data::TestStage;
use kuro_test_api::data::TestStatus;
use kuro_test_api::grpc::TestOrchestratorClient;
use parking_lot::Mutex;
use sorted_vector_map::SortedVectorMap;

use crate::config::Config;
use crate::config::EnvValue;

pub type SpecReceiver = UnboundedReceiver<ExternalRunnerSpec>;

/// Internal test runner implementation for Kuro.
///
/// This is a basic test runner intended to be used by the open-source Kuro build
/// if no external test runner is provided. This ensures that `kuro test` works
/// out-of-the-box for open-source users.
///
/// **This is intended for open-source use only.**
pub struct KuroTestRunner {
    orchestrator_client: TestOrchestratorClient,
    spec_receiver: Mutex<Option<SpecReceiver>>,
    config: Config,
}

impl KuroTestRunner {
    pub fn new(
        orchestrator_client: TestOrchestratorClient,
        spec_receiver: SpecReceiver,
        args: Vec<String>,
    ) -> kuro_error::Result<Self> {
        let config = Config::try_parse_from(args)
            .buck_error_context("Error parsing test runner arguments")?;
        Ok(Self {
            orchestrator_client,
            spec_receiver: Mutex::new(Some(spec_receiver)),
            config,
        })
    }

    pub async fn run_all_tests(&self) -> kuro_error::Result<()> {
        let receiver;
        {
            let mut maybe_receiver = self.spec_receiver.lock();
            receiver = maybe_receiver
                .take()
                .buck_error_context("Spec channel has already been consumed")?;
            drop(maybe_receiver);
        }
        let run_verdict = receiver
            .map(|spec| async move {
                let name = format!(
                    "{}//{}:{} - main",
                    spec.target.cell, spec.target.package, spec.target.target
                );
                let target_handle = spec.target.handle.to_owned();

                let execution_response = self
                    .execute_test_from_spec(spec)
                    .await
                    .buck_error_context("Test execution request failed")?;

                let execution_result = match execution_response {
                    ExecuteResponse::Result(r) => r,
                    ExecuteResponse::Cancelled(_) => return Ok(TestStatus::OMITTED),
                };

                let test_result = get_test_result(name, target_handle, execution_result);
                let test_status = test_result.status.clone();

                self.report_test_result(test_result)
                    .await
                    .buck_error_context("Test result reporting failed")?;

                Ok(test_status)
            })
            // Use an arbitrarily large buffer -- execution throttling will be handled by the Kuro
            // executor, so no need to hold back on requests here.
            .buffer_unordered(10000)
            // If any individual test failed, consider the entire run to have failed.
            .try_fold(
                RunVerdict::Pass,
                |mut run_verdict, test_status| async move {
                    if test_status != TestStatus::PASS {
                        run_verdict = RunVerdict::Fail;
                    }
                    kuro_error::Ok(run_verdict)
                },
            )
            .await;

        self.orchestrator_client
            .end_of_test_results(run_verdict?.exit_code())
            .await
    }

    async fn execute_test_from_spec(
        &self,
        spec: ExternalRunnerSpec,
    ) -> kuro_error::Result<ExecuteResponse> {
        let suite = format!(
            "{}//{}:{}",
            spec.target.cell, spec.target.package, spec.target.target
        );
        let target_handle = spec.target.handle.clone();
        let host_sharing_requirements = HostSharingRequirements::default();

        let config_args: Vec<ArgValue> = self.config.test_arg.iter().map(|arg| ArgValue {
            content: ArgValueContent::ExternalRunnerSpecValue(ExternalRunnerSpecValue::Verbatim(
                arg.to_owned(),
            )),
            format: None,
        }).collect();

        let base_command: Vec<ArgValue> = spec
            .command
            .into_iter()
            .map(|spec_value| ArgValue {
                content: ArgValueContent::ExternalRunnerSpecValue(spec_value),
                format: None,
            })
            .chain(config_args)
            .collect();

        let config_env: Vec<_> = self
            .config
            .env
            .iter()
            .map(|s| s.parse())
            .collect::<kuro_error::Result<_>>()?;
        let env: SortedVectorMap<String, ArgValue> = spec
            .env
            .into_iter()
            .map(|(key, value)| {
                (
                    key,
                    ArgValue {
                        content: ArgValueContent::ExternalRunnerSpecValue(value),
                        format: None,
                    },
                )
            })
            .chain(config_env.iter().map(|EnvValue { name, value }| {
                (
                    name.to_owned(),
                    ArgValue {
                        content: ArgValueContent::ExternalRunnerSpecValue(
                            ExternalRunnerSpecValue::Verbatim(value.to_owned()),
                        ),
                        format: None,
                    },
                )
            }))
            .collect();

        // Step 1: Listing stage — run the command with --list to discover test cases.
        // This causes the orchestrator to emit a TestListing build signal for the
        // critical path, and allows the test runner protocol to discover test case names.
        let listing_command: Vec<ArgValue> = base_command
            .iter()
            .cloned()
            .chain(std::iter::once(ArgValue {
                content: ArgValueContent::ExternalRunnerSpecValue(
                    ExternalRunnerSpecValue::Verbatim("--list".to_owned()),
                ),
                format: None,
            }))
            .collect();

        let listing_stage = TestStage::Listing {
            suite: suite.clone(),
            cacheable: false,
        };

        let listing_response = self
            .orchestrator_client
            .execute2(
                listing_stage,
                target_handle.clone(),
                listing_command,
                env.clone(),
                Duration::from_secs(self.config.timeout),
                host_sharing_requirements.clone(),
                Vec::new(),
                None,
                RequiredLocalResources { resources: vec![] },
            )
            .await?;

        // Parse listing output to get test case names.
        let testcases = match &listing_response {
            ExecuteResponse::Result(result) => {
                // Report listing result to the orchestrator.
                let listing_status = match result.status {
                    ExecutionStatus::Finished { exitcode: 0 } => TestStatus::LISTING_SUCCESS,
                    _ => TestStatus::LISTING_FAILED,
                };
                let listing_result = TestResult {
                    target: target_handle.clone(),
                    name: format!("{suite} - listing"),
                    status: listing_status,
                    msg: None,
                    duration: Some(result.execution_time),
                    details: String::new(),
                    max_memory_used_bytes: None,
                };
                self.report_test_result(listing_result)
                    .await
                    .buck_error_context("Listing result reporting failed")?;

                // Extract test case names from listing stdout (one per line).
                let ExecutionStream::Inline(stdout) = &result.stdout;
                String::from_utf8_lossy(stdout)
                    .lines()
                    .map(|l| l.trim().to_owned())
                    .filter(|l| !l.is_empty())
                    .collect()
            }
            ExecuteResponse::Cancelled(_) => return Ok(listing_response),
        };

        // Step 2: Testing stage with discovered test cases.
        let stage = TestStage::Testing {
            suite,
            testcases,
            variant: None,
        };

        self.orchestrator_client
            .execute2(
                stage,
                target_handle,
                base_command,
                env,
                Duration::from_secs(self.config.timeout),
                host_sharing_requirements,
                Vec::new(),
                None,
                RequiredLocalResources { resources: vec![] },
            )
            .await
    }

    async fn report_test_result(&self, test_result: TestResult) -> kuro_error::Result<()> {
        self.orchestrator_client
            .report_test_result(test_result)
            .await
    }
}

fn get_test_result(
    name: String,
    target: ConfiguredTargetHandle,
    execution_result: ExecutionResult2,
) -> TestResult {
    let status = match execution_result.status {
        ExecutionStatus::Finished { exitcode } => match exitcode {
            0 => TestStatus::PASS,
            _ => TestStatus::FAIL,
        },
        ExecutionStatus::TimedOut { .. } => TestStatus::TIMEOUT,
    };
    TestResult {
        target,
        name,
        status,
        msg: None,
        duration: Some(execution_result.execution_time),
        details: format!(
            "---- STDOUT ----\n{:?}\n---- STDERR ----\n{:?}\n",
            execution_result.stdout, execution_result.stderr
        ),
        max_memory_used_bytes: execution_result.max_memory_used_bytes,
    }
}

#[derive(Debug)]
enum RunVerdict {
    Pass,
    Fail,
}

impl RunVerdict {
    fn exit_code(&self) -> i32 {
        match self {
            RunVerdict::Pass => 0,
            RunVerdict::Fail => 32,
        }
    }
}
