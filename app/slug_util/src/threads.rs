/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::cell::Cell;
use std::future::Future;
use std::hint;
use std::pin::Pin;
use std::pin::pin;
use std::sync::OnceLock;
use std::task::Poll;
use std::thread;

use slug_error::BuckErrorContext;
use slug_error::internal_error;

/// Get the available parallelism
///
/// This value is cached for the lifetime of the process. The reason is that there are various
/// components that cannot be updated to reflect the new value if it changes during the lifetime of
/// the daemon. Caching this sacrifices some accuracy of this value in exchange for putting the
/// daemon into a more predictable state.
///
/// Use `available_parallelism_fresh` if the caching is not desired
pub fn available_parallelism() -> usize {
    static PARALLELISM: OnceLock<usize> = OnceLock::new();

    *PARALLELISM.get_or_init(available_parallelism_fresh)
}

/// Get the available parallelism
///
/// Unlike `available_parallelism`, this is not cached - callers using this should ensure that this
/// value is logged somewhere
pub fn available_parallelism_fresh() -> usize {
    // NB: num_cpus and tokio both also use 1 as the default in case of an error
    std::thread::available_parallelism().map_or(1, |v| v.get())
}

/// Get the default concurrency for action execution, accounting for available RAM.
///
/// Bazel's `--jobs` defaults to `HOST_CPUS` but its resource-aware scheduler further constrains
/// based on `HOST_RAM * 0.67` with per-action RAM budgets. Slug does not yet have a resource-aware
/// scheduler, so we bake RAM-awareness into the default concurrency to avoid OOM on machines with
/// fewer GB per core.
///
/// The formula: `min(cpu_cores, available_ram_mb / PER_ACTION_RAM_MB)`.
///
/// - `PER_ACTION_RAM_MB = 1024`: a conservative estimate covering heavy C++ compiles (~1–1.5 GB
///   each for LLVM/libc++) while not over-throttling lighter workloads. Bazel uses 250 MB per
///   rule action but that only works because the resource scheduler can reject over-subscription;
///   without one, we need a bigger per-slot budget.
/// - We use 67% of total RAM (matching Bazel's default `HOST_RAM*.67`) as the portion available
///   for build actions, leaving headroom for the daemon, OS, and other processes.
pub fn default_concurrency_for_actions() -> usize {
    let cpu_cores = available_parallelism_fresh();

    // Try to read total RAM in MB from /proc/meminfo (Linux-only; on other platforms we fall
    // back to cpu_cores).
    let ram_mb = read_total_ram_mb();
    let Some(ram_mb) = ram_mb else {
        return cpu_cores;
    };

    const USABLE_RAM_FRACTION: f64 = 0.67;
    const PER_ACTION_RAM_MB: usize = 1024;

    let usable_ram_mb = (ram_mb as f64 * USABLE_RAM_FRACTION) as usize;
    let ram_limited = std::cmp::max(usable_ram_mb / PER_ACTION_RAM_MB, 1);

    std::cmp::min(cpu_cores, ram_limited)
}

/// Read total RAM in MB from `/proc/meminfo` on Linux. Returns `None` on non-Linux or on error.
fn read_total_ram_mb() -> Option<usize> {
    if !cfg!(target_os = "linux") {
        return None;
    }

    let meminfo = std::fs::read_to_string("/proc/meminfo").ok()?;
    for line in meminfo.lines() {
        if let Some(rest) = line.strip_prefix("MemTotal:") {
            // Format: "MemTotal:       65736424 kB"
            let kb: usize = rest.trim().split_whitespace().next()?.parse().ok()?;
            return Some(kb / 1024);
        }
    }
    None
}

/// Default stack size for slug.
///
/// We want to be independent of possible future changes to the default stack size in Rust.
pub(crate) const THREAD_DEFAULT_STACK_SIZE: usize = {
    if cfg!(slug_asan) {
        // ASAN requires much larger stack size.
        8 << 20
    } else if cfg!(debug_assertions) {
        // Need 4MB for windows-debug according to D60449433.
        4 << 20
    } else {
        2 << 20
    }
};

fn thread_builder(name: &str) -> thread::Builder {
    thread::Builder::new()
        .stack_size(THREAD_DEFAULT_STACK_SIZE)
        .name(name.to_owned())
}

pub fn thread_spawn<T, F>(name: &str, code: F) -> std::io::Result<thread::JoinHandle<T>>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    thread_builder(name).spawn(move || {
        on_thread_start();
        let r = code();
        on_thread_stop();
        r
    })
}

pub fn thread_spawn_scoped<'scope, 'env: 'scope, T, F>(
    name: &str,
    scope: &'scope thread::Scope<'scope, 'env>,
    code: F,
) -> std::io::Result<thread::ScopedJoinHandle<'scope, T>>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'scope,
{
    thread_builder(name).spawn_scoped(scope, move || {
        on_thread_start();
        let r = code();
        on_thread_stop();
        r
    })
}

pub(crate) fn stack_pointer() -> *const () {
    let mut x: u32 = 0;
    hint::black_box(&mut x as *const u32 as *const ())
}

#[derive(Copy, Clone)]
struct ValidStackRange {
    start: *const (),
    end: *const (),
}

impl ValidStackRange {
    fn full_range() -> ValidStackRange {
        let start = usize::MAX as *const ();
        let end = usize::MIN as *const ();
        ValidStackRange { start, end }
    }
}

thread_local! {
    static STACK_RANGE: Cell<Option<ValidStackRange >> = const { Cell::new(None) };
}

pub(crate) fn on_thread_start() {
    assert!(
        STACK_RANGE.get().is_none(),
        "stack range must not be set in a new thread"
    );
    let stack_pointer = stack_pointer();
    // Stack grows downwards. So we add to the start and subtract from the end.
    // Add a little bit to the start because we don't really know where the stack starts.
    let start = (stack_pointer as usize).checked_add(0x1000).unwrap() as *const ();
    // Subtract 3/4 to catch stack overflow before program crashes.
    let end = (stack_pointer as usize)
        .checked_sub(THREAD_DEFAULT_STACK_SIZE / 4 * 3)
        .unwrap() as *const ();
    let stack_range = ValidStackRange { start, end };
    STACK_RANGE.set(Some(stack_range));
}

pub(crate) fn on_thread_stop() {
    let range = STACK_RANGE.replace(None);
    assert!(range.is_some(), "stack range must be set in a thread");
}

pub fn check_stack_overflow() -> slug_error::Result<()> {
    let stack_range = STACK_RANGE.get().internal_error("stack range not set")?;
    let stack_pointer = stack_pointer();
    if stack_pointer > stack_range.start {
        return Err(internal_error!("stack underflow, should not happen"));
    }
    if stack_pointer < stack_range.end {
        return Err(internal_error!("stack overflow"));
    }
    Ok(())
}

/// Returns a process-wide monotonic id for the calling OS thread.
/// First thread to call gets `1`, the next `2`, etc.; subsequent
/// calls from the same thread return the same value. `0` is the
/// documented "not captured" sentinel used by code paths that
/// synthesize events without a real thread context (test fixtures,
/// etc.).
///
/// Used as the chrome trace `tid` for action events. This is exactly
/// what `java.lang.Thread.getId()` does (a JVM-wide incrementing
/// counter assigned at thread construction, not the OS tid), so
/// Bazel's chrome traces and ours have matching semantics for the
/// `tid` field. For remote actions Bazel records the
/// *submitting/awaiting* thread (not the BB worker that ran the
/// compute); the same falls out here because the tokio worker that
/// polls the future to completion is what calls this function.
pub fn thread_index() -> u64 {
    use std::sync::atomic::AtomicU64;
    use std::sync::atomic::Ordering;
    static NEXT: AtomicU64 = AtomicU64::new(1);
    thread_local! {
        static INDEX: u64 = NEXT.fetch_add(1, Ordering::Relaxed);
    }
    INDEX.with(|i| *i)
}

#[must_use]
pub struct IgnoreStackOverflowChecksForCurrentThread {
    prev: Option<ValidStackRange>,
}

impl Drop for IgnoreStackOverflowChecksForCurrentThread {
    fn drop(&mut self) {
        STACK_RANGE.set(self.prev.take());
    }
}

/// For tests.
pub fn ignore_stack_overflow_checks_for_current_thread() -> IgnoreStackOverflowChecksForCurrentThread
{
    let prev = STACK_RANGE.replace(Some(ValidStackRange::full_range()));
    IgnoreStackOverflowChecksForCurrentThread { prev }
}

/// For tests.
pub async fn ignore_stack_overflow_checks_for_future<F: Future>(f: F) -> F::Output {
    let f = pin!(f);

    struct IgnoreStackOverflowChecksForFuture<'a, F> {
        f: Pin<&'a mut F>,
    }

    impl<F: Future> Future for IgnoreStackOverflowChecksForFuture<'_, F> {
        type Output = F::Output;

        fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
            let _ignore = ignore_stack_overflow_checks_for_current_thread();
            self.f.as_mut().poll(cx)
        }
    }

    IgnoreStackOverflowChecksForFuture { f }.await
}

#[cfg(test)]
pub(crate) mod tests {
    use std::hint;

    use crate::threads::check_stack_overflow;
    use crate::threads::thread_spawn;

    pub(crate) fn recursive_function(frames: u32) -> slug_error::Result<()> {
        let Some(frames) = frames.checked_sub(1) else {
            return Ok(());
        };

        check_stack_overflow()?;

        // Allocate a string on the stack so the compiler won't optimize the recursion away.
        let mut x = String::new();
        hint::black_box(&mut x);
        recursive_function(frames)?;
        hint::black_box(&mut x);
        Ok(())
    }

    #[test]
    fn test_catch_stack_overflow() {
        let error = thread_spawn("test", || recursive_function(u32::MAX))
            .unwrap()
            .join()
            .unwrap()
            .unwrap_err();
        assert!(error.to_string().contains("stack overflow"), "{error:?}");
    }

    #[test]
    fn test_no_stack_overflow() {
        let () = thread_spawn("test", || recursive_function(1000))
            .unwrap()
            .join()
            .unwrap()
            .unwrap();
    }

    #[test]
    fn test_read_total_ram_mb() {
        // On Linux, /proc/meminfo exists and we should get a positive value.
        // On non-Linux, the function returns None.
        let ram = crate::threads::read_total_ram_mb();
        if cfg!(target_os = "linux") {
            let ram = ram.expect("should read /proc/meminfo on Linux");
            assert!(ram > 0, "RAM should be positive, got {ram}");
            // A reasonable sanity check: most dev machines have >= 1GB.
            assert!(ram >= 1024, "RAM should be >= 1 GB, got {ram} MB");
        } else {
            assert!(ram.is_none());
        }
    }

    #[test]
    fn test_default_concurrency_for_actions() {
        let concurrency = crate::threads::default_concurrency_for_actions();
        assert!(concurrency >= 1, "concurrency should be >= 1, got {concurrency}");
        // On a 16-core / 62GB box: cpu_cores=16, ram_limited = (64254*0.67)/1024 ≈ 42.
        // So min(16, 42) = 16.  On a 16-core / 8GB box: ram_limited = (8192*0.67)/1024 ≈ 5.
        // So min(16, 5) = 5.  Either way, concurrency <= cpu_cores.
        let cpu_cores = crate::threads::available_parallelism_fresh();
        assert!(
            concurrency <= cpu_cores,
            "concurrency ({concurrency}) should not exceed cpu_cores ({cpu_cores})"
        );
    }
}
