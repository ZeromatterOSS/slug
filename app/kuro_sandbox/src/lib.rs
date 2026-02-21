/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Local build sandboxing for hermetic action execution.
//!
//! This crate implements filesystem sandboxing for local build actions. The sandbox
//! ensures that actions can only write to their declared output directories, catching
//! accidental writes to undeclared locations and improving build hermeticity.
//!
//! On Linux, the sandbox uses user namespaces and mount namespaces to:
//! 1. Make the entire filesystem read-only within the action's execution context
//! 2. Allow writes only to declared output directories (via writable bind mounts)
//!
//! This provides write isolation while keeping reads unrestricted (actions can still
//! read any file on the system). Full read isolation (via chroot) is future work.

use std::path::PathBuf;

/// Configuration for sandbox execution.
#[derive(Clone, Debug, Default)]
pub struct SandboxSpec {
    /// Absolute paths to directories where the action is allowed to write.
    /// These will be made writable even when the rest of the filesystem is read-only.
    pub output_dirs: Vec<PathBuf>,
}

/// Apply sandbox to a `std::process::Command` before spawning.
///
/// On Linux: sets up a user+mount namespace via `pre_exec` that makes the filesystem
/// read-only except for declared output directories.
///
/// On other platforms: this is a no-op (the command is unchanged).
///
/// # Safety
/// This function uses `pre_exec` which runs in the forked child process before exec.
/// The hook is technically unsafe (not async-signal-safe), but this pattern is widely
/// used in practice (e.g., Bazel's linux-sandbox, Docker, etc.).
pub fn apply_sandbox(cmd: &mut std::process::Command, spec: SandboxSpec) {
    #[cfg(target_os = "linux")]
    linux::apply_sandbox_linux(cmd, spec);
    #[cfg(not(target_os = "linux"))]
    {
        let _ = cmd;
        let _ = spec;
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use std::ffi::CString;
    use std::io;
    use std::os::unix::process::CommandExt;
    use std::path::Path;
    use std::path::PathBuf;

    use super::SandboxSpec;

    pub(crate) fn apply_sandbox_linux(cmd: &mut std::process::Command, spec: SandboxSpec) {
        let output_dirs = spec.output_dirs;
        unsafe {
            cmd.pre_exec(move || {
                if let Err(e) = setup_sandbox(&output_dirs) {
                    // Log the error but don't fail - sandbox failure shouldn't block builds
                    // unless --sandbox_failure_is_error is set (future work)
                    tracing::warn!("Sandbox setup failed (continuing without sandbox): {}", e);
                }
                Ok(())
            });
        }
    }

    fn setup_sandbox(output_dirs: &[PathBuf]) -> io::Result<()> {
        // Step 1: Create new user namespace + mount namespace.
        // CLONE_NEWUSER: allows unprivileged namespace creation; UID 0 in namespace = real UID
        // CLONE_NEWNS: creates a new mount namespace isolated from the parent
        let ret = unsafe { libc::unshare(libc::CLONE_NEWUSER | libc::CLONE_NEWNS) };
        if ret < 0 {
            return Err(io::Error::last_os_error());
        }

        // Step 2: Set up UID/GID mappings for the user namespace.
        // Map UID 0 in namespace -> our real UID; required before mounting.
        let uid = unsafe { libc::getuid() };
        let gid = unsafe { libc::getgid() };

        // Must write "deny" to setgroups before writing gid_map (kernel requirement)
        std::fs::write("/proc/self/setgroups", "deny")?;
        std::fs::write("/proc/self/uid_map", format!("0 {uid} 1\n"))?;
        std::fs::write("/proc/self/gid_map", format!("0 {gid} 1\n"))?;

        // Step 3: Make all inherited mounts MS_SLAVE so our mount changes
        // don't propagate back to the parent namespace.
        let root_cstr = CString::new("/").unwrap();
        let ret = unsafe {
            libc::mount(
                b"none\0".as_ptr() as *const libc::c_char,
                root_cstr.as_ptr(),
                std::ptr::null(),
                libc::MS_SLAVE | libc::MS_REC,
                std::ptr::null(),
            )
        };
        if ret < 0 {
            return Err(io::Error::last_os_error());
        }

        // Step 4: Bind-mount root onto itself, then remount as read-only.
        // This makes the entire filesystem read-only within this namespace.
        // Two-step: first bind-mount (to get a new mount entry), then remount rdonly.
        let ret = unsafe {
            libc::mount(
                root_cstr.as_ptr(),
                root_cstr.as_ptr(),
                std::ptr::null(),
                libc::MS_BIND | libc::MS_REC,
                std::ptr::null(),
            )
        };
        if ret < 0 {
            return Err(io::Error::last_os_error());
        }
        let ret = unsafe {
            libc::mount(
                root_cstr.as_ptr(),
                root_cstr.as_ptr(),
                std::ptr::null(),
                libc::MS_BIND | libc::MS_REMOUNT | libc::MS_RDONLY | libc::MS_REC,
                std::ptr::null(),
            )
        };
        if ret < 0 {
            return Err(io::Error::last_os_error());
        }

        // Step 5: For each output directory, create a writable bind mount on top
        // of the read-only root. This "punches holes" for declared outputs.
        for output_dir in output_dirs {
            if !output_dir.exists() {
                // Output dir was cleaned before exec; skip it (action will create it)
                continue;
            }
            if let Err(e) = make_writable_bind_mount(output_dir) {
                // Non-fatal: if we can't make output dir writable, the action will fail
                // on its own with a permission error (which is actually correct behavior!)
                tracing::debug!(
                    "Failed to make output dir writable in sandbox ({}): {}",
                    output_dir.display(),
                    e
                );
            }
        }

        // Step 6: Mount a fresh tmpfs on /tmp so actions can use it as scratch space.
        // (Even though /tmp is already accessible, making it a fresh tmpfs ensures
        // the action can write there and doesn't see other processes' tmp files.)
        let tmp_cstr = CString::new("/tmp").unwrap();
        if Path::new("/tmp").exists() {
            unsafe {
                // Ignore errors - /tmp might already be a tmpfs, or not needed
                libc::mount(
                    b"kuro-sandbox\0".as_ptr() as *const libc::c_char,
                    tmp_cstr.as_ptr(),
                    b"tmpfs\0".as_ptr() as *const libc::c_char,
                    0,
                    std::ptr::null(),
                );
            }
        }

        Ok(())
    }

    fn make_writable_bind_mount(path: &Path) -> io::Result<()> {
        let path_cstr = CString::new(path.as_os_str().as_encoded_bytes())?;

        // First: bind-mount the directory to itself (creates a new mount entry)
        let ret = unsafe {
            libc::mount(
                path_cstr.as_ptr(),
                path_cstr.as_ptr(),
                std::ptr::null(),
                libc::MS_BIND,
                std::ptr::null(),
            )
        };
        if ret < 0 {
            return Err(io::Error::last_os_error());
        }

        // Second: remount without MS_RDONLY to make it writable
        let ret = unsafe {
            libc::mount(
                path_cstr.as_ptr(),
                path_cstr.as_ptr(),
                std::ptr::null(),
                libc::MS_BIND | libc::MS_REMOUNT,
                std::ptr::null(),
            )
        };
        if ret < 0 {
            return Err(io::Error::last_os_error());
        }

        Ok(())
    }
}
