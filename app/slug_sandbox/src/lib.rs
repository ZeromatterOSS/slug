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
//! ensures that actions can only write to their declared output directories and can
//! only read their declared input files from buck-out, catching both undeclared writes
//! and undeclared build artifact reads.
//!
//! On Linux, the sandbox uses user namespaces and mount namespaces to:
//! 1. Make the entire filesystem read-only within the action's execution context
//! 2. Allow writes only to declared output directories (via writable bind mounts)
//! 3. Expose only declared input files from buck-out (hiding undeclared build artifacts)
//!
//! Input isolation details:
//! - Source files (in the project root, not buck-out) remain fully accessible
//! - System tools (/usr/bin/gcc, etc.) remain accessible
//! - Only buck-out build artifacts are restricted to declared inputs

use std::path::PathBuf;

/// Configuration for sandbox execution.
#[derive(Clone, Debug, Default)]
pub struct SandboxSpec {
    /// Absolute paths to directories where the action is allowed to write.
    /// These will be made writable even when the rest of the filesystem is read-only.
    pub output_dirs: Vec<PathBuf>,

    /// Absolute paths to declared input files within buck-out.
    /// Only these files will be visible under `buck_out_root` during the action.
    /// If empty, no input isolation is performed for buck-out.
    pub input_files: Vec<PathBuf>,

    /// Absolute path to the buck-out root directory (e.g., /proj/buck-out/v2).
    /// Required for input isolation. If None, input isolation is skipped.
    pub buck_out_root: Option<PathBuf>,
}

/// Apply sandbox to a `std::process::Command` before spawning.
///
/// On Linux: sets up a user+mount namespace via `pre_exec` that makes the filesystem
/// read-only except for declared output directories, and optionally restricts buck-out
/// access to only declared input files.
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
        let input_files = spec.input_files;
        let buck_out_root = spec.buck_out_root;
        unsafe {
            cmd.pre_exec(move || {
                if let Err(e) = setup_sandbox(&output_dirs, &input_files, buck_out_root.as_deref())
                {
                    // Write error directly to stderr (tracing is not available in pre_exec).
                    // Continue without isolation rather than blocking the build.
                    let msg = format!("[slug-sandbox] setup failed (continuing): {}\n", e);
                    let _ = libc::write(2, msg.as_ptr() as *const libc::c_void, msg.len());
                }
                Ok(())
            });
        }
    }

    fn setup_sandbox(
        output_dirs: &[PathBuf],
        input_files: &[PathBuf],
        buck_out_root: Option<&Path>,
    ) -> io::Result<()> {
        // Step 1: Get real UID/GID BEFORE unshare (after unshare they become 65534/nobody)
        let uid = unsafe { libc::getuid() };
        let gid = unsafe { libc::getgid() };

        // Step 2: (Input isolation) If buck_out_root and input_files are provided,
        // create a staging directory and bind-mount declared inputs into it.
        // This must happen BEFORE unshare so we can access the real files.
        let staging_dir = if let (Some(buck_out), true) = (buck_out_root, !input_files.is_empty()) {
            match create_input_staging_dir(buck_out, input_files) {
                Ok(dir) => Some(dir),
                Err(e) => {
                    let msg = format!(
                        "[slug-sandbox] input staging failed (continuing without input isolation): {}\n",
                        e
                    );
                    let _ =
                        unsafe { libc::write(2, msg.as_ptr() as *const libc::c_void, msg.len()) };
                    None
                }
            }
        } else {
            None
        };

        // Step 3: Create new user namespace + mount namespace.
        // CLONE_NEWUSER: allows unprivileged namespace creation; UID 0 in namespace = real UID
        // CLONE_NEWNS: creates a new mount namespace isolated from the parent
        let ret = unsafe { libc::unshare(libc::CLONE_NEWUSER | libc::CLONE_NEWNS) };
        if ret < 0 {
            return Err(io::Error::last_os_error());
        }

        // Step 4: Set up UID/GID mappings for the user namespace.
        // Must write "deny" to setgroups before writing gid_map (kernel requirement)
        std::fs::write("/proc/self/setgroups", "deny")?;
        std::fs::write("/proc/self/uid_map", format!("0 {uid} 1\n"))?;
        std::fs::write("/proc/self/gid_map", format!("0 {gid} 1\n"))?;

        // Step 5: Make all inherited mounts MS_SLAVE so our mount changes
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

        // Step 6: Bind-mount root onto itself, then remount as read-only.
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

        // Step 7: (Input isolation) Bind-mount staging dir over buck-out root.
        // Now only declared inputs are visible under buck-out.
        if let (Some(staging), Some(buck_out)) = (&staging_dir, buck_out_root) {
            if let Err(e) = bind_mount(staging, buck_out) {
                let msg = format!(
                    "[slug-sandbox] buck-out overlay failed (input isolation disabled): {}\n",
                    e
                );
                let _ = unsafe { libc::write(2, msg.as_ptr() as *const libc::c_void, msg.len()) };
            }
        }

        // Step 8: For each output directory, create a writable bind mount on top
        // of the read-only root. This "punches holes" for declared outputs.
        for output_dir in output_dirs {
            if !output_dir.exists() {
                // Output dir was cleaned before exec; skip it (action will create it)
                continue;
            }
            if let Err(e) = make_writable_bind_mount(output_dir) {
                // Non-fatal: if we can't make output dir writable, the action will fail
                // on its own with a permission error (which is actually correct behavior!)
                let msg = format!(
                    "[slug-sandbox] writable bind mount failed for {}: {}\n",
                    output_dir.display(),
                    e
                );
                let _ = unsafe { libc::write(2, msg.as_ptr() as *const libc::c_void, msg.len()) };
            }
        }

        // Step 9: Mount a fresh tmpfs on /tmp so actions can use it as scratch space.
        // (Even though /tmp is already accessible, making it a fresh tmpfs ensures
        // the action can write there and doesn't see other processes' tmp files.)
        // Note: this hides the staging_dir that was created under /tmp, which is fine
        // since we already did the bind-mount in step 7.
        let tmp_cstr = CString::new("/tmp").unwrap();
        if Path::new("/tmp").exists() {
            unsafe {
                // Ignore errors - /tmp might already be a tmpfs, or not needed
                libc::mount(
                    b"slug-sandbox\0".as_ptr() as *const libc::c_char,
                    tmp_cstr.as_ptr(),
                    b"tmpfs\0".as_ptr() as *const libc::c_char,
                    0,
                    std::ptr::null(),
                );
            }
        }

        Ok(())
    }

    /// Create a staging directory at /tmp/slug-sandbox-XXXXXX/ and bind-mount
    /// each declared input file from buck-out into the corresponding path.
    /// Returns the staging directory path.
    fn create_input_staging_dir(
        buck_out_root: &Path,
        input_files: &[PathBuf],
    ) -> io::Result<PathBuf> {
        // Create unique temp directory using mkdtemp.
        // We need a mutable buffer because mkdtemp modifies the template in-place.
        let mut template = b"/tmp/slug-sandbox-XXXXXX\0".to_vec();
        let ptr = unsafe { libc::mkdtemp(template.as_mut_ptr() as *mut libc::c_char) };
        if ptr.is_null() {
            return Err(io::Error::last_os_error());
        }
        // Find the null terminator to get the path bytes
        let null_pos = template
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(template.len());
        let staging_path =
            PathBuf::from(std::str::from_utf8(&template[..null_pos]).map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidData, "non-UTF8 staging path")
            })?);

        for input_file in input_files {
            // Only handle files that are within buck_out_root
            let Ok(relative) = input_file.strip_prefix(buck_out_root) else {
                continue;
            };

            let target = staging_path.join(relative);

            // Create parent directories in the staging tmpfs
            if let Some(parent) = target.parent() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    // Non-fatal: skip this input if we can't create parent dirs
                    let msg = format!(
                        "[slug-sandbox] create_dir_all failed for {}: {}\n",
                        parent.display(),
                        e
                    );
                    let _ =
                        unsafe { libc::write(2, msg.as_ptr() as *const libc::c_void, msg.len()) };
                    continue;
                }
            }

            // Create bind mount target (file or directory)
            if input_file.is_dir() {
                if let Err(e) = std::fs::create_dir_all(&target) {
                    let msg = format!(
                        "[slug-sandbox] mkdir failed for {}: {}\n",
                        target.display(),
                        e
                    );
                    let _ =
                        unsafe { libc::write(2, msg.as_ptr() as *const libc::c_void, msg.len()) };
                    continue;
                }
            } else {
                // Create empty file as bind-mount target
                if let Err(e) = std::fs::File::create(&target) {
                    let msg = format!(
                        "[slug-sandbox] touch failed for {}: {}\n",
                        target.display(),
                        e
                    );
                    let _ =
                        unsafe { libc::write(2, msg.as_ptr() as *const libc::c_void, msg.len()) };
                    continue;
                }
            }

            // Bind-mount the real input file to the staging target
            if let Err(e) = bind_mount(input_file, &target) {
                let msg = format!(
                    "[slug-sandbox] bind-mount input failed {} -> {}: {}\n",
                    input_file.display(),
                    target.display(),
                    e
                );
                let _ = unsafe { libc::write(2, msg.as_ptr() as *const libc::c_void, msg.len()) };
            }
        }

        Ok(staging_path)
    }

    fn bind_mount(source: &Path, target: &Path) -> io::Result<()> {
        let src_cstr = CString::new(source.as_os_str().as_encoded_bytes())?;
        let tgt_cstr = CString::new(target.as_os_str().as_encoded_bytes())?;

        let ret = unsafe {
            libc::mount(
                src_cstr.as_ptr(),
                tgt_cstr.as_ptr(),
                std::ptr::null(),
                libc::MS_BIND | libc::MS_REC,
                std::ptr::null(),
            )
        };
        if ret < 0 {
            return Err(io::Error::last_os_error());
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
