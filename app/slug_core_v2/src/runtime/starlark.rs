/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::path::Path;

use starlark::environment::Globals;
use starlark::environment::GlobalsBuilder;
use starlark::environment::Module;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;
use starlark::values::none::NoneType;

pub trait StarlarkEvaluator {
    fn implementation_name(&self) -> &'static str;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DeferredStarlarkEvaluator;

impl StarlarkEvaluator for DeferredStarlarkEvaluator {
    fn implementation_name(&self) -> &'static str {
        "starlark-rust-wrapper-pending"
    }
}

#[starlark_module]
fn module_file_globals(globals: &mut GlobalsBuilder) {
    fn module(name: String, version: Option<String>) -> anyhow::Result<NoneType> {
        let _ = (name, version);
        Ok(NoneType)
    }
}

/// Evaluate one root-level file with the intentionally small V2 global set.
///
/// Full Bazel globals, `load()` resolution, and file dependency keys belong to
/// Stages 4 and 5; this function only establishes the actual starlark-rust
/// parse/evaluation boundary required by the configured-build chain.
pub(crate) fn evaluate_file(path: &Path, source: &str, is_module: bool) -> anyhow::Result<()> {
    let dialect = if is_module {
        &Dialect::Standard
    } else {
        &Dialect::Bazel
    };
    let ast = AstModule::parse(&path.display().to_string(), source.to_owned(), dialect)
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    let globals = if is_module {
        GlobalsBuilder::standard().with(module_file_globals).build()
    } else {
        Globals::standard()
    };
    let module = Module::new();
    Evaluator::new(&module)
        .eval_module(ast, &globals)
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bazel_build_dialect_binds_keyword_only_parameters() {
        evaluate_file(
            Path::new("BUILD.bazel"),
            "def support(*, std = False, host_tools = False):\n    return std and not host_tools\ndef variadic(*args, enabled = False):\n    return enabled\nRESULT = support(std = True) and variadic(1, enabled = True) and (lambda *, value: value)(value = True)\n",
            false,
        )
        .unwrap();
        let positional = evaluate_file(
            Path::new("BUILD.bazel"),
            "def support(*, std = False): pass\nsupport(True)\n",
            false,
        )
        .unwrap_err();
        assert!(positional.to_string().contains("extra positional"));
        let missing = evaluate_file(
            Path::new("BUILD.bazel"),
            "def support(*, std): pass\nsupport()\n",
            false,
        )
        .unwrap_err();
        assert!(missing.to_string().contains("Missing named-only parameter"));
    }
    #[test]
    fn module_branch_does_not_admit_bare_star_parameters() {
        let error = evaluate_file(
            Path::new("MODULE.bazel"),
            "def support(*, std = False): pass\n",
            true,
        )
        .unwrap_err();
        assert!(error.to_string().contains("not allowed in this dialect"));
    }
}
