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

use allocative::Allocative;
use starlark::any::ProvidesStaticType;
use starlark::environment::GlobalsBuilder;
use starlark::environment::GlobalsStatic;
use starlark::eval::Arguments;
use starlark::eval::Evaluator;
use starlark::values::FrozenHeap;
use starlark::values::FrozenValue;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::StarlarkValue;
use starlark::values::Value;
use starlark::values::starlark_value;

const EXECUTION_INFO: &str = "ExecutionInfo";
const INSTRUMENTED_FILES_INFO: &str = "InstrumentedFilesInfo";
const ANALYSIS_FAILURE_INFO: &str = "AnalysisFailureInfo";
const ANALYSIS_TEST_RESULT_INFO: &str = "AnalysisTestResultInfo";

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
struct CallableTestingProvider {
    name: &'static str,
}

impl CallableTestingProvider {
    const fn new(name: &'static str) -> Self {
        Self { name }
    }
}

impl fmt::Display for CallableTestingProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<function {}>", self.name)
    }
}

starlark::starlark_simple_value!(CallableTestingProvider);

#[starlark_value(type = "Provider")]
impl<'v> StarlarkValue<'v> for CallableTestingProvider {
    fn invoke(
        &self,
        _me: Value<'v>,
        _args: &Arguments<'v, '_>,
        _eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        Err(starlark::Error::new_other(anyhow::anyhow!(
            "{} construction is unsupported until configured analysis semantics are admitted",
            self.name
        )))
    }
}

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
struct NonCallableTestingProvider {
    name: &'static str,
}

impl NonCallableTestingProvider {
    const fn new(name: &'static str) -> Self {
        Self { name }
    }
}

impl fmt::Display for NonCallableTestingProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<function {}>", self.name)
    }
}

starlark::starlark_simple_value!(NonCallableTestingProvider);

#[starlark_value(type = "Provider")]
impl<'v> StarlarkValue<'v> for NonCallableTestingProvider {}

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
struct UnsupportedTestingOperation {
    name: &'static str,
}

impl UnsupportedTestingOperation {
    const fn new(name: &'static str) -> Self {
        Self { name }
    }
}

impl fmt::Display for UnsupportedTestingOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<function {}>", self.name)
    }
}

starlark::starlark_simple_value!(UnsupportedTestingOperation);

#[starlark_value(type = "function")]
impl<'v> StarlarkValue<'v> for UnsupportedTestingOperation {
    fn invoke(
        &self,
        _me: Value<'v>,
        _args: &Arguments<'v, '_>,
        _eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        Err(starlark::Error::new_other(anyhow::anyhow!(
            "{} is unsupported until configured analysis semantics are admitted",
            self.name
        )))
    }
}

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
struct TestingModule {
    execution_info: FrozenValue,
    test_environment: FrozenValue,
    analysis_test: FrozenValue,
}

impl fmt::Display for TestingModule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("testing")
    }
}

starlark::starlark_simple_value!(TestingModule);

#[starlark_value(type = "testing")]
impl<'v> StarlarkValue<'v> for TestingModule {
    fn get_attr(&self, attribute: &str, _heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "ExecutionInfo" => Some(self.execution_info.to_value()),
            "TestEnvironment" => Some(self.test_environment.to_value()),
            "analysis_test" => Some(self.analysis_test.to_value()),
            _ => None,
        }
    }

    fn dir_attr(&self) -> Vec<String> {
        ["ExecutionInfo", "TestEnvironment", "analysis_test"]
            .into_iter()
            .map(str::to_owned)
            .collect()
    }
}

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
struct CoverageCommonModule {
    instrumented_files_info: FrozenValue,
}

impl fmt::Display for CoverageCommonModule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("coverage_common")
    }
}

starlark::starlark_simple_value!(CoverageCommonModule);

#[starlark_value(type = "coverage_common")]
impl<'v> StarlarkValue<'v> for CoverageCommonModule {
    fn get_attr(&self, attribute: &str, _heap: Heap<'v>) -> Option<Value<'v>> {
        (attribute == "instrumented_files_info").then(|| self.instrumented_files_info.to_value())
    }

    fn dir_attr(&self) -> Vec<String> {
        vec!["instrumented_files_info".to_owned()]
    }
}

pub(crate) fn testing_bootstrap_globals(builder: &mut GlobalsBuilder) {
    static BOOTSTRAP: GlobalsStatic = GlobalsStatic::new();
    BOOTSTRAP.populate(
        |bootstrap| {
            let execution_info = bootstrap.alloc(CallableTestingProvider::new(EXECUTION_INFO));
            let test_environment =
                bootstrap.alloc(UnsupportedTestingOperation::new("testing.TestEnvironment"));
            let analysis_test =
                bootstrap.alloc(UnsupportedTestingOperation::new("testing.analysis_test"));
            let instrumented_files_info = bootstrap.alloc(UnsupportedTestingOperation::new(
                "coverage_common.instrumented_files_info",
            ));

            bootstrap.set(
                "testing",
                TestingModule {
                    execution_info,
                    test_environment,
                    analysis_test,
                },
            );
            bootstrap.set(
                "coverage_common",
                CoverageCommonModule {
                    instrumented_files_info,
                },
            );
            bootstrap.set(
                INSTRUMENTED_FILES_INFO,
                NonCallableTestingProvider::new(INSTRUMENTED_FILES_INFO),
            );
            bootstrap.set(
                ANALYSIS_FAILURE_INFO,
                NonCallableTestingProvider::new(ANALYSIS_FAILURE_INFO),
            );
            bootstrap.set(
                ANALYSIS_TEST_RESULT_INFO,
                CallableTestingProvider::new(ANALYSIS_TEST_RESULT_INFO),
            );
        },
        builder,
    );
}

pub(crate) fn testing_provider_identity(value: Value<'_>) -> Option<&'static str> {
    if let Some(provider) = CallableTestingProvider::from_value(value) {
        return Some(provider.name);
    }
    NonCallableTestingProvider::from_value(value).map(|provider| provider.name)
}

pub(crate) fn alloc_testing_provider_token(
    heap: &FrozenHeap,
    name: &'static str,
) -> Option<FrozenValue> {
    match name {
        EXECUTION_INFO | ANALYSIS_TEST_RESULT_INFO => {
            Some(heap.alloc(CallableTestingProvider::new(name)))
        }
        INSTRUMENTED_FILES_INFO | ANALYSIS_FAILURE_INFO => {
            Some(heap.alloc(NonCallableTestingProvider::new(name)))
        }
        _ => None,
    }
}
