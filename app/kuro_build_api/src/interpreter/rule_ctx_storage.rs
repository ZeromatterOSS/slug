/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Thread-local storage for the current rule's `ctx` value.
//!
//! Set by the analysis pipeline in `kuro_analysis::analysis::env::get_user_defined_rule_spec`
//! before calling a rule's implementation function, and cleared afterwards. Consumers:
//!
//! - `kuro_interpreter_for_build::subrule`: looks it up to inject `ctx` as the first
//!   positional argument to subrule implementation functions.
//! - `kuro_build_api::interpreter::rule_defs::provider::builtin::default_info`:
//!   looks it up to synthesize a runfiles symlink tree when a rule returns
//!   `DefaultInfo(executable = ..., default_runfiles = ...)`.

use starlark::values::Value;

thread_local! {
    static CURRENT_RULE_CTX: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
    /// Plan 28.4 Stage 5: subrule-wrapper `Value` for the current rule
    /// analysis. Set alongside `CURRENT_RULE_CTX` so subrule invocations
    /// can route through the bundled
    /// `subrule_implementation_wrapper(impl, ctx, **kwargs)`. `None`
    /// when @kuro_builtins isn't registered or doesn't expose the
    /// hook — subrule.rs falls back to direct invocation.
    static CURRENT_SUBRULE_WRAPPER: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
}

const _: () = assert!(std::mem::size_of::<Value<'_>>() == std::mem::size_of::<usize>());

/// Set the current rule context for subrule invocations.
///
/// SAFETY: The caller must ensure the `Value` outlives any callers of
/// `get_current_rule_ctx`. In practice this is the analysis pipeline, which sets
/// the cell immediately before `eval.eval_function(rule_impl, &[ctx_val], ...)`
/// and clears it after the call returns.
pub fn set_current_rule_ctx_raw(ctx_bits: usize) {
    CURRENT_RULE_CTX.with(|cell| cell.set(Some(ctx_bits)));
}

/// Clear the current rule context after rule implementation completes.
pub fn clear_current_rule_ctx() {
    CURRENT_RULE_CTX.with(|cell| cell.set(None));
}

/// Get the current rule context `Value` if a rule implementation is currently executing.
///
/// SAFETY: The stored bits must have been set by `set_current_rule_ctx_raw`
/// from a valid `Value` that is still alive on the heap.
pub fn get_current_rule_ctx<'v>() -> Option<Value<'v>> {
    CURRENT_RULE_CTX.with(|cell| {
        cell.get().map(|bits| {
            // SAFETY: bits were stored from a valid Value<'v> via transmute,
            // and the Value is alive on the evaluator's heap which outlives this call.
            unsafe { std::mem::transmute::<usize, Value<'v>>(bits) }
        })
    })
}

/// Plan 28.4 Stage 5: stash the bundled `subrule_implementation_wrapper`
/// alongside the current rule's ctx, so subrule invocations can route
/// through the same Starlark facade rule contexts already see.
///
/// SAFETY: same contract as `set_current_rule_ctx_raw` — the caller
/// must ensure the wrapper `Value` outlives any callers of
/// `get_current_subrule_wrapper`.
pub fn set_current_subrule_wrapper_raw(wrapper_bits: usize) {
    CURRENT_SUBRULE_WRAPPER.with(|cell| cell.set(Some(wrapper_bits)));
}

/// Clear the current subrule wrapper after the rule's eval completes.
pub fn clear_current_subrule_wrapper() {
    CURRENT_SUBRULE_WRAPPER.with(|cell| cell.set(None));
}

/// Get the current subrule wrapper `Value` if one is registered.
///
/// SAFETY: same contract as `get_current_rule_ctx`.
pub fn get_current_subrule_wrapper<'v>() -> Option<Value<'v>> {
    CURRENT_SUBRULE_WRAPPER.with(|cell| {
        cell.get()
            .map(|bits| unsafe { std::mem::transmute::<usize, Value<'v>>(bits) })
    })
}
