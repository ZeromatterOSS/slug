/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use tokio::runtime::Builder;

use crate::threads::THREAD_DEFAULT_STACK_SIZE;
use crate::threads::on_thread_start;
use crate::threads::on_thread_stop;

pub fn new_tokio_runtime(thread_name: &str) -> Builder {
    use std::sync::atomic::AtomicU64;
    use std::sync::atomic::Ordering;
    let mut builder = Builder::new_multi_thread();
    builder.thread_stack_size(THREAD_DEFAULT_STACK_SIZE);
    // Per-worker numbered names (`kuro-rt-0`, `kuro-rt-1`, …) instead
    // of a single shared name. The chrome trace's lane labels read
    // `std::thread::current().name()` so each lane shows the actual
    // worker that ran the action — matching Bazel's `Thread.getName()`
    // shape (e.g. "skyframe-evaluator-N") rather than a bare integer
    // counter.
    let prefix = thread_name.to_owned();
    let counter = std::sync::Arc::new(AtomicU64::new(0));
    builder.thread_name_fn(move || {
        let n = counter.fetch_add(1, Ordering::Relaxed);
        format!("{prefix}-{n}")
    });
    builder.on_thread_start(on_thread_start);
    builder.on_thread_stop(on_thread_stop);
    builder.worker_threads(crate::threads::available_parallelism());
    builder
}

#[cfg(test)]
mod tests {
    use crate::threads::tests::recursive_function;
    use crate::tokio_runtime::new_tokio_runtime;

    #[test]
    fn test_stack_overflow() {
        let rt = new_tokio_runtime("test_stack_overflow").build().unwrap();
        let error = rt
            .block_on(async {
                tokio::spawn(async { recursive_function(u32::MAX) })
                    .await
                    .unwrap()
            })
            .unwrap_err();
        assert!(error.to_string().contains("stack overflow"), "{error:?}");
    }

    #[test]
    fn test_no_stack_overflow() {
        let rt = new_tokio_runtime("test_stack_overflow").build().unwrap();
        let () = rt
            .block_on(async {
                tokio::spawn(async { recursive_function(1000) })
                    .await
                    .unwrap()
            })
            .unwrap();
    }
}
