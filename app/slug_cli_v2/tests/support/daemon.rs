/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::fs;
use std::os::unix::fs::FileTypeExt;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use super::INSTALLED_DAEMON;

pub(crate) fn daemon_fixture() -> (DaemonCleanup, PathBuf) {
    use std::os::unix::fs::DirBuilderExt;
    use std::os::unix::net::SocketAddr;

    static NEXT: AtomicU64 = AtomicU64::new(0);
    let parent = std::env::temp_dir().canonicalize().unwrap();
    loop {
        let index = NEXT.fetch_add(1, Ordering::Relaxed);
        let directory = parent.join(format!("s-{}-{index}", std::process::id()));
        let output_base = directory.join("o");
        for socket in [
            daemon_socket(&output_base),
            daemon_stage_socket(&output_base),
        ] {
            SocketAddr::from_pathname(&socket)
                .unwrap_or_else(|error| panic!("test socket {}: {error}", socket.display()));
        }
        match std::fs::DirBuilder::new().mode(0o700).create(&directory) {
            Ok(()) => return (DaemonCleanup(output_base.clone()), output_base),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => panic!("creating daemon fixture {}: {error}", directory.display()),
        }
    }
}

pub(crate) fn daemon_socket(base: &Path) -> PathBuf {
    if INSTALLED_DAEMON {
        base.join(".slugd-installed/live/s")
    } else {
        slug_server_v2::socket_path(base)
    }
}

fn daemon_stage_socket(base: &Path) -> PathBuf {
    if INSTALLED_DAEMON {
        base.join(".slugd-installed/stage/s")
    } else {
        slug_server_v2::socket_path(base)
    }
}

pub(crate) fn daemon_pid_path(base: &Path) -> PathBuf {
    if INSTALLED_DAEMON {
        base.join(".slugd-installed/live/pid")
    } else {
        slug_server_v2::pid_path(base)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DaemonGeneration {
    pub(crate) pid: u32,
    socket: (u64, u64),
    live: Option<(u64, u64)>,
}

pub(crate) fn daemon_generation(base: &Path) -> anyhow::Result<DaemonGeneration> {
    let socket_path = daemon_socket(base);
    let socket = fs::symlink_metadata(&socket_path)?;
    anyhow::ensure!(socket.file_type().is_socket());
    let pid = fs::read_to_string(daemon_pid_path(base))?.parse()?;
    #[cfg(all(
        target_os = "linux",
        target_arch = "x86_64",
        target_pointer_width = "64"
    ))]
    let live = {
        let stream = std::os::unix::net::UnixStream::connect(&socket_path)?;
        anyhow::ensure!(authenticate_installed_peer(base, &stream)? == pid);
        let live = fs::symlink_metadata(socket_path.parent().unwrap())?;
        anyhow::ensure!(live.is_dir());
        Some((live.dev(), live.ino()))
    };
    #[cfg(not(all(
        target_os = "linux",
        target_arch = "x86_64",
        target_pointer_width = "64"
    )))]
    let live = None;
    Ok(DaemonGeneration {
        pid,
        socket: (socket.dev(), socket.ino()),
        live,
    })
}

pub(crate) struct DaemonCleanup(PathBuf);

impl DaemonCleanup {
    fn retire(&self) -> anyhow::Result<()> {
        #[cfg(all(
            target_os = "linux",
            target_arch = "x86_64",
            target_pointer_width = "64"
        ))]
        shutdown_installed_daemon(&self.0)?;
        if !INSTALLED_DAEMON {
            let socket = daemon_socket(&self.0);
            if socket.exists() {
                let _ = slug_server_v2::send_shutdown(&socket);
            }
        }
        fs::remove_dir_all(self.0.parent().unwrap())?;
        Ok(())
    }
}

impl Drop for DaemonCleanup {
    fn drop(&mut self) {
        if let Err(error) = self.retire() {
            let message = format!("retiring daemon at {}: {error:#}", self.0.display());
            if std::thread::panicking() {
                eprintln!("{message}");
            } else {
                panic!("{message}");
            }
        }
    }
}

#[cfg(all(
    target_os = "linux",
    target_arch = "x86_64",
    target_pointer_width = "64"
))]
fn authenticate_installed_peer(
    base: &Path,
    stream: &std::os::unix::net::UnixStream,
) -> anyhow::Result<u32> {
    use std::os::fd::AsRawFd;

    let mut credentials = std::mem::MaybeUninit::<libc::ucred>::uninit();
    let mut length = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    let status = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            credentials.as_mut_ptr().cast(),
            &mut length,
        )
    };
    if status != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    anyhow::ensure!(length as usize == std::mem::size_of::<libc::ucred>());
    let credentials = unsafe { credentials.assume_init() };
    anyhow::ensure!(credentials.uid == unsafe { libc::geteuid() } && credentials.pid > 0);
    anyhow::ensure!(fs::read_to_string(daemon_pid_path(base))? == credentials.pid.to_string());
    let peer = fs::metadata(format!("/proc/{}/exe", credentials.pid))?;
    let executable = fs::metadata(super::executable::slug_executable())?;
    anyhow::ensure!((peer.dev(), peer.ino()) == (executable.dev(), executable.ino()));
    Ok(credentials.pid as u32)
}

#[cfg(all(
    target_os = "linux",
    target_arch = "x86_64",
    target_pointer_width = "64"
))]
fn shutdown_installed_daemon(base: &Path) -> anyhow::Result<()> {
    use std::io;
    use std::io::Read;
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    use std::os::unix::net::UnixStream;
    use std::time::Duration;
    use std::time::Instant;

    let control = base.join(".slugd-installed");
    let socket = daemon_socket(base);
    let deadline = Instant::now() + Duration::from_secs(10);
    let remaining = || {
        let left = deadline.saturating_duration_since(Instant::now());
        anyhow::ensure!(!left.is_zero(), "installed daemon retirement timed out");
        Ok::<_, anyhow::Error>(left)
    };
    let vacant = || {
        let lock = match fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(control.join("lock"))
        {
            Ok(lock) => lock,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(true),
            Err(error) => return Err(error),
        };
        match lock.try_lock() {
            Ok(()) => Ok(true),
            Err(fs::TryLockError::WouldBlock) => Ok(false),
            Err(fs::TryLockError::Error(error)) => Err(error),
        }
    };
    // A failed request can return while its child is publishing or retiring.
    while !socket.try_exists()? && !vacant()? {
        std::thread::sleep(remaining()?.min(Duration::from_millis(20)));
    }
    match UnixStream::connect(&socket) {
        Ok(mut stream) => {
            authenticate_installed_peer(base, &stream)?;
            stream.set_write_timeout(Some(remaining()?))?;
            stream.write_all(b"shutdown\n")?;
            stream.set_read_timeout(Some(remaining()?))?;
            anyhow::ensure!(
                stream.read(&mut [0u8; 1])? == 0,
                "unexpected shutdown reply"
            );
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    while !vacant()? {
        std::thread::sleep(remaining()?.min(Duration::from_millis(20)));
    }
    for generation in ["stage", "live"] {
        match fs::symlink_metadata(control.join(generation)) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
            Ok(_) => anyhow::bail!("installed daemon retained its {generation} generation"),
        }
    }
    Ok(())
}
