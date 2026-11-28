//! Child-owned launch and publication for the installed daemon.
use std::ffi::CString;
use std::fs;
use std::fs::File;
use std::fs::OpenOptions;
use std::io;
use std::io::Read;
use std::io::Write;
use std::os::fd::AsRawFd;
use std::os::fd::FromRawFd;
use std::os::fd::OwnedFd;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::DirBuilderExt;
use std::os::unix::fs::FileTypeExt;
use std::os::unix::fs::MetadataExt;
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::net::SocketAddr;
use std::os::unix::net::UnixListener;
use std::os::unix::net::UnixStream;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;
use std::time::Duration;
use std::time::Instant;

use anyhow::Context;
use slug_server_v2::ManagedDaemonConnection;
use slug_server_v2::ManagedWorkspace;

use super::RuntimeStartup;

pub(super) const CHILD_ROLE: &str = "--installed-daemon-startup-v1";
const CONTROL: &str = ".slugd-installed";
const STAGE: &str = "stage";
const LIVE: &str = "live";
const SOCKET: &str = "s";
const PID: &str = "pid";
const MAX_FAILURE_BYTES: usize = 1024;
const DEADLINE: Duration = Duration::from_secs(10);
const BOUND: u8 = 1;
const COMMIT: u8 = 2;
const READY: u8 = 3;
const BUSY: u8 = 4;
const FAILED: u8 = 5;

pub(super) fn socket_path(base: &Path) -> PathBuf {
    base.join(CONTROL).join(LIVE).join(SOCKET)
}

#[cfg(test)]
pub(super) fn pid_path(base: &Path) -> PathBuf {
    base.join(CONTROL).join(LIVE).join(PID)
}

pub(super) fn connect(base: &Path, workspace: &Path) -> anyhow::Result<ManagedDaemonConnection> {
    let workspace = ManagedWorkspace::pin(
        workspace,
        open_workspace_root(Path::new("."))?,
        open_workspace_root,
    )?;
    let socket = ensure(base, &workspace)?;
    ManagedDaemonConnection::connect(&socket, workspace)
}

fn open_workspace_root(workspace: &Path) -> io::Result<File> {
    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_PATH | libc::O_DIRECTORY | libc::O_NOFOLLOW)
        .open(workspace)
}

fn ensure(base: &Path, workspace: &ManagedWorkspace) -> anyhow::Result<PathBuf> {
    let base = protected_base(base)?;
    let socket = socket_path(&base);
    let deadline = Instant::now() + DEADLINE;
    loop {
        let left = remaining(deadline)?;
        if probe(&base, &socket)? {
            return Ok(socket);
        }
        // The current child may still be publishing or may have failed before
        // bind. A vacant lock permits another exact-child startup attempt.
        if lock_vacant(&base)? && start(&base, workspace, deadline)? {
            return Ok(socket);
        }
        std::thread::sleep(left.min(Duration::from_millis(20)));
    }
}

fn start(base: &Path, workspace: &ManagedWorkspace, deadline: Instant) -> anyhow::Result<bool> {
    let (parent, child) = UnixStream::pair()?;
    let mut child = Command::new("/proc/self/exe")
        .arg(CHILD_ROLE)
        .arg(base)
        .arg(workspace.path())
        .stdin(Stdio::from(OwnedFd::from(child)))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("spawning installed daemon")?;
    let mut channel = Channel {
        stream: parent,
        deadline,
    };
    let mut committed = false;
    let result = (|| {
        if !commit_child_binding(workspace, &mut channel)? {
            return Ok(false);
        }
        committed = true;
        anyhow::ensure!(
            channel.receive()? == READY,
            "unexpected installed daemon post-commit status"
        );
        Ok(true)
    })();
    drop(channel);
    match result {
        Ok(true) => Ok(true),
        Ok(false) => {
            // A contender has no generation and may be retired immediately.
            let _ = child.kill();
            child.wait().context("reaping installed daemon contender")?;
            Ok(false)
        }
        Err(error) if !committed => {
            let error = super::build::terminate_and_reap(&mut child, error);
            if matches!(child.try_wait(), Ok(Some(_))) {
                if let Err(cleanup) = retire_aborted_stage(base) {
                    let message =
                        format!("{error:#}; retiring installed daemon stage: {cleanup:#}");
                    return Err(error.context(message));
                }
            }
            Err(error)
        }
        Err(error) => {
            // Commit may have published a healthy daemon. A missing Ready
            // frame does not revoke that publication.
            let _ = child.try_wait();
            Err(error.context("waiting for the committed installed daemon"))
        }
    }
}

fn commit_child_binding(
    workspace: &ManagedWorkspace,
    channel: &mut Channel,
) -> anyhow::Result<bool> {
    match channel.receive()? {
        BUSY => return Ok(false),
        BOUND => {}
        _ => anyhow::bail!("unexpected installed daemon pre-commit status"),
    }
    workspace.require_peer_binding(channel)?;
    channel.send(COMMIT, "")?;
    Ok(true)
}

pub(super) fn child(args: &[String]) -> anyhow::Result<()> {
    let stream = take_channel()?;
    let mut channel = Channel {
        stream,
        deadline: Instant::now() + DEADLINE,
    };
    let prepared = match args {
        [base, workspace] => prepare(Path::new(base), Path::new(workspace), &mut channel),
        _ => Err(anyhow::anyhow!(
            "invalid installed daemon startup arguments"
        )),
    };
    let (listener, socket, runtime, workspace, _generation) = match prepared {
        Ok(Some(prepared)) => prepared,
        Ok(None) => return Ok(()),
        Err(error) => {
            let message = error.to_string();
            let mut end = message.len().min(MAX_FAILURE_BYTES);
            while !message.is_char_boundary(end) {
                end -= 1;
            }
            let _ = channel.send(FAILED, &message[..end]);
            return Err(error);
        }
    };
    let _ = channel.send(READY, "");
    drop(channel);
    slug_server_v2::serve_managed_listener(listener, &socket, runtime, workspace)
}

type Prepared = (
    UnixListener,
    PathBuf,
    slug_core_v2::runtime::WorkspaceRuntime,
    ManagedWorkspace,
    Generation,
);

fn prepare(
    base: &Path,
    workspace: &Path,
    channel: &mut Channel,
) -> anyhow::Result<Option<Prepared>> {
    let base = protected_base(base)?;
    let control = base.join(CONTROL);
    match fs::DirBuilder::new().mode(0o700).create(&control) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error.into()),
    }
    require_private_dir(&control)?;
    let lock = open_lock(&control, true)?;
    match lock.try_lock() {
        Ok(()) => {}
        Err(fs::TryLockError::WouldBlock) => {
            channel.send(BUSY, "")?;
            return Ok(None);
        }
        Err(fs::TryLockError::Error(error)) => return Err(error.into()),
    }

    let workspace = ManagedWorkspace::pin(
        workspace,
        open_workspace_root(workspace)?,
        open_workspace_root,
    )?;
    let runtime = RuntimeStartup::Installed
        .new_runtime(workspace.path())
        .map_err(anyhow::Error::msg)?;
    workspace.require_runtime_workspace(runtime.workspace())?;
    let stage = control.join(STAGE);
    let live = control.join(LIVE);
    // Only the lifetime lock holder may retire a previous generation. Socket
    // probing and the PID file never grant removal authority.
    clean_generation(&stage)?;
    clean_generation(&live)?;
    fs::DirBuilder::new().mode(0o700).create(&stage)?;
    let mut generation = Generation {
        path: stage,
        _lock: lock,
    };
    require_private_dir(&generation.path)?;
    let listener =
        UnixListener::bind(generation.path.join(SOCKET)).context("binding installed daemon")?;
    write_pid(&generation.path)?;
    generation.commit(live, channel, &workspace, runtime.workspace())?;
    Ok(Some((
        listener,
        socket_path(&base),
        runtime,
        workspace,
        generation,
    )))
}

fn write_pid(stage: &Path) -> anyhow::Result<()> {
    let mut pid = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(stage.join(PID))
        .context("creating installed daemon PID")?;
    write!(pid, "{}", std::process::id()).context("writing installed daemon PID")?;
    Ok(())
}

fn take_channel() -> io::Result<UnixStream> {
    // The launcher installs only its unnamed Unix stream endpoint at stdin.
    // Duplication makes the endpoint close-on-exec and restores null stdin.
    let duplicated = unsafe { libc::fcntl(libc::STDIN_FILENO, libc::F_DUPFD_CLOEXEC, 3) };
    if duplicated < 0 {
        return Err(io::Error::last_os_error());
    }
    let stream = UnixStream::from(unsafe { OwnedFd::from_raw_fd(duplicated) });
    if !stream.local_addr()?.is_unnamed() || !stream.peer_addr()?.is_unnamed() {
        return Err(io::Error::other(
            "expected an unnamed installed daemon startup pair",
        ));
    }
    let null = File::open("/dev/null")?;
    if unsafe { libc::dup2(null.as_raw_fd(), libc::STDIN_FILENO) } == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(stream)
}

struct Channel {
    stream: UnixStream,
    deadline: Instant,
}

impl Channel {
    fn send(&mut self, kind: u8, text: &str) -> io::Result<()> {
        let [high, low] = (text.len() as u16).to_be_bytes();
        self.write_all(&[1, kind, high, low])?;
        self.write_all(text.as_bytes())
    }

    fn receive(&mut self) -> anyhow::Result<u8> {
        let mut header = [0; 4];
        self.read_exact(&mut header)?;
        let [version, kind, high, low] = header;
        let len = usize::from(u16::from_be_bytes([high, low]));
        anyhow::ensure!(
            version == 1
                && (BOUND..=FAILED).contains(&kind)
                && len <= MAX_FAILURE_BYTES
                && (len == 0 || kind == FAILED),
            "invalid installed daemon startup frame"
        );
        if kind == FAILED {
            let mut bytes = [0; MAX_FAILURE_BYTES];
            self.read_exact(&mut bytes[..len])?;
            anyhow::bail!("{}", std::str::from_utf8(&bytes[..len])?);
        }
        Ok(kind)
    }
}

impl Read for Channel {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.stream
            .set_read_timeout(Some(remaining(self.deadline)?))?;
        self.stream.read(bytes)
    }
}

impl Write for Channel {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.stream
            .set_write_timeout(Some(remaining(self.deadline)?))?;
        self.stream.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stream.flush()
    }
}

fn remaining(deadline: Instant) -> io::Result<Duration> {
    let left = deadline.saturating_duration_since(Instant::now());
    if left.is_zero() {
        Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "installed daemon startup deadline exceeded",
        ))
    } else {
        Ok(left)
    }
}

fn uid() -> u32 {
    unsafe { libc::geteuid() }
}

fn protected_base(base: &Path) -> anyhow::Result<PathBuf> {
    let base = std::path::absolute(base)?;
    anyhow::ensure!(
        uid() != 0 && uid() == unsafe { libc::getuid() },
        "installed daemon requires an unprivileged CLI user"
    );
    for socket in [
        socket_path(&base),
        base.join(CONTROL).join(STAGE).join(SOCKET),
    ] {
        SocketAddr::from_pathname(socket).context("installed daemon socket path does not fit")?;
    }
    let mut path = PathBuf::new();
    for component in base.components() {
        anyhow::ensure!(
            matches!(component, Component::RootDir | Component::Normal(_)),
            "installed daemon requires a normalized output base"
        );
        path.push(component);
        let metadata = fs::symlink_metadata(&path)?;
        let mode = metadata.mode();
        let owner = metadata.uid();
        // A root-owned sticky ancestor protects the next checked owned entry.
        let protected = mode & 0o022 == 0 || (owner == 0 && mode & libc::S_ISVTX != 0);
        anyhow::ensure!(
            metadata.is_dir() && (owner == 0 || owner == uid()) && protected,
            "unprotected installed daemon output-base ancestry"
        );
    }
    let metadata = fs::symlink_metadata(&base)?;
    anyhow::ensure!(
        metadata.uid() == uid() && metadata.mode() & 0o022 == 0,
        "installed daemon output base must be owned by the CLI user"
    );
    Ok(base)
}

fn private_dir(metadata: &fs::Metadata) -> anyhow::Result<()> {
    anyhow::ensure!(
        metadata.is_dir() && metadata.uid() == uid() && metadata.mode() & 0o777 == 0o700,
        "unprotected installed daemon private directory"
    );
    Ok(())
}

fn require_private_dir(path: &Path) -> anyhow::Result<()> {
    private_dir(&fs::symlink_metadata(path)?)
}

fn open_lock(control: &Path, create: bool) -> io::Result<File> {
    let file = OpenOptions::new()
        .create(create)
        .truncate(false)
        .read(true)
        .write(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(control.join("lock"))?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.uid() != uid() || metadata.mode() & 0o777 != 0o600 {
        return Err(io::Error::other("invalid installed daemon lock"));
    }
    Ok(file)
}

fn lock_vacant(base: &Path) -> anyhow::Result<bool> {
    let control = base.join(CONTROL);
    if !exists(&control)? {
        return Ok(true);
    }
    require_private_dir(&control)?;
    let lock = match open_lock(&control, false) {
        Ok(lock) => lock,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(true),
        Err(error) => return Err(error.into()),
    };
    match lock.try_lock() {
        Ok(()) => Ok(true),
        Err(fs::TryLockError::WouldBlock) => Ok(false),
        Err(fs::TryLockError::Error(error)) => Err(error.into()),
    }
}

fn exists(path: &Path) -> io::Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

fn retire_aborted_stage(base: &Path) -> anyhow::Result<()> {
    let control = base.join(CONTROL);
    if !exists(&control)? {
        return Ok(());
    }
    require_private_dir(&control)?;
    let lock = match open_lock(&control, false) {
        Ok(lock) => lock,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    match lock.try_lock() {
        Ok(()) => clean_generation(&control.join(STAGE)),
        Err(fs::TryLockError::WouldBlock) => Ok(()),
        Err(fs::TryLockError::Error(error)) => Err(error.into()),
    }
}

fn probe(base: &Path, socket: &Path) -> anyhow::Result<bool> {
    let control = base.join(CONTROL);
    let live = control.join(LIVE);
    if !exists(&control)? {
        return Ok(false);
    }
    require_private_dir(&control)?;
    match fs::symlink_metadata(&live) {
        Ok(metadata) => private_dir(&metadata)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    }
    match fs::symlink_metadata(socket) {
        Ok(metadata) => anyhow::ensure!(
            metadata.file_type().is_socket() && metadata.uid() == uid(),
            "invalid installed daemon socket"
        ),
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    }
    probe_socket(socket).map_err(Into::into)
}

fn probe_socket(path: &Path) -> io::Result<bool> {
    // A full listener queue must never block startup probing.
    let raw = unsafe {
        libc::socket(
            libc::AF_UNIX,
            libc::SOCK_STREAM | libc::SOCK_CLOEXEC | libc::SOCK_NONBLOCK,
            0,
        )
    };
    if raw == -1 {
        return Err(io::Error::last_os_error());
    }
    let fd = unsafe { OwnedFd::from_raw_fd(raw) };
    let mut address: libc::sockaddr_un = unsafe { std::mem::zeroed() };
    address.sun_family = libc::AF_UNIX as libc::sa_family_t;
    let bytes = path.as_os_str().as_bytes();
    for (slot, &byte) in address.sun_path.iter_mut().zip(bytes) {
        *slot = byte as libc::c_char;
    }
    let len = std::mem::offset_of!(libc::sockaddr_un, sun_path) + bytes.len() + 1;
    let result = unsafe {
        libc::connect(
            fd.as_raw_fd(),
            (&raw const address).cast(),
            len as libc::socklen_t,
        )
    };
    if result == 0 {
        return Ok(true);
    }
    let error = io::Error::last_os_error();
    match error.raw_os_error() {
        Some(libc::ENOENT | libc::ECONNREFUSED | libc::EAGAIN | libc::EINPROGRESS) => Ok(false),
        _ => Err(error),
    }
}

fn publish(stage: &Path, live: &Path) -> io::Result<()> {
    let stage = CString::new(stage.as_os_str().as_bytes())?;
    let live = CString::new(live.as_os_str().as_bytes())?;
    if unsafe {
        libc::renameat2(
            libc::AT_FDCWD,
            stage.as_ptr(),
            libc::AT_FDCWD,
            live.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    } == 0
    {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

fn clean_generation(path: &Path) -> anyhow::Result<()> {
    if !exists(path)? {
        return Ok(());
    }
    require_private_dir(path)?;
    let mut socket = false;
    let mut pid = false;
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let metadata = fs::symlink_metadata(entry.path())?;
        anyhow::ensure!(metadata.uid() == uid(), "foreign installed daemon entry");
        match entry.file_name().as_bytes() {
            b"s" if !socket && metadata.file_type().is_socket() => socket = true,
            b"pid" if !pid && metadata.is_file() && metadata.mode() & 0o077 == 0 => pid = true,
            _ => anyhow::bail!("unexpected installed daemon generation contents"),
        }
    }
    if socket {
        fs::remove_file(path.join(SOCKET))?;
    }
    if pid {
        fs::remove_file(path.join(PID))?;
    }
    fs::remove_dir(path)?;
    Ok(())
}

struct Generation {
    path: PathBuf,
    _lock: File,
}

impl Generation {
    fn commit(
        &mut self,
        live: PathBuf,
        channel: &mut Channel,
        workspace: &ManagedWorkspace,
        runtime_workspace: &Path,
    ) -> anyhow::Result<()> {
        channel.send(BOUND, "")?;
        workspace.write_binding(channel)?;
        anyhow::ensure!(
            channel.receive()? == COMMIT,
            "invalid installed daemon commit"
        );
        workspace.require_runtime_workspace(runtime_workspace)?;
        publish(&self.path, &live)?;
        self.path = live;
        Ok(())
    }
}

impl Drop for Generation {
    fn drop(&mut self) {
        let _ = clean_generation(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use std::net::Shutdown;
    use std::time::SystemTime;
    use std::time::UNIX_EPOCH;

    use super::*;

    struct TestRoot(PathBuf);

    impl TestRoot {
        fn new() -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!("slg-{}-{nonce:x}", std::process::id()));
            fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
            Self(path)
        }

        fn stage(&self) -> (Generation, UnixListener) {
            let lock = open_lock(&self.0, true).unwrap();
            lock.try_lock().unwrap();
            let path = self.0.join(STAGE);
            fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
            let listener = UnixListener::bind(path.join(SOCKET)).unwrap();
            let mut pid = OpenOptions::new()
                .create_new(true)
                .write(true)
                .mode(0o600)
                .open(path.join(PID))
                .unwrap();
            pid.write_all(b"123").unwrap();
            (Generation { path, _lock: lock }, listener)
        }
    }

    impl Drop for TestRoot {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    fn channel_pair() -> (Channel, Channel) {
        let (parent, child) = UnixStream::pair().unwrap();
        let deadline = Instant::now() + DEADLINE;
        (
            Channel {
                stream: parent,
                deadline,
            },
            Channel {
                stream: child,
                deadline,
            },
        )
    }

    fn pin_workspace(path: &Path) -> ManagedWorkspace {
        ManagedWorkspace::pin(
            path,
            open_workspace_root(path).unwrap(),
            open_workspace_root,
        )
        .unwrap()
    }

    #[test]
    fn commit_publishes_the_bound_listener_and_pid_as_one_generation() {
        let root = TestRoot::new();
        let (mut generation, listener) = root.stage();
        let live = root.0.join(LIVE);
        let (mut parent, mut child) = channel_pair();
        let workspace = pin_workspace(&root.0);
        assert!(!live.exists());
        parent.send(COMMIT, "").unwrap();
        generation
            .commit(live.clone(), &mut child, &workspace, workspace.path())
            .unwrap();
        assert_eq!(parent.receive().unwrap(), BOUND);
        workspace.require_peer_binding(&mut parent).unwrap();
        assert_eq!(fs::read(live.join(PID)).unwrap(), b"123");
        UnixStream::connect(live.join(SOCKET)).unwrap();
        let contender = open_lock(&root.0, false).unwrap();
        assert!(matches!(
            contender.try_lock(),
            Err(fs::TryLockError::WouldBlock)
        ));
        drop(listener);
        drop(generation);
        assert!(!live.exists());
        contender.try_lock().unwrap();
    }

    #[test]
    fn eof_before_commit_retires_only_the_private_stage() {
        let root = TestRoot::new();
        let (mut generation, listener) = root.stage();
        let live = root.0.join(LIVE);
        let (mut parent, mut child) = channel_pair();
        let workspace = pin_workspace(&root.0);
        parent.stream.shutdown(Shutdown::Write).unwrap();
        assert!(
            generation
                .commit(live.clone(), &mut child, &workspace, workspace.path())
                .is_err()
        );
        assert_eq!(parent.receive().unwrap(), BOUND);
        workspace.require_peer_binding(&mut parent).unwrap();
        assert!(!live.exists());
        drop(listener);
        drop(generation);
        assert!(!root.0.join(STAGE).exists());
    }

    #[test]
    fn parent_refuses_replacement_workspace_before_commit() {
        let root = TestRoot::new();
        let (mut generation, listener) = root.stage();
        let workspace = root.0.join("workspace");
        fs::create_dir(&workspace).unwrap();
        let expected = pin_workspace(&workspace);
        fs::rename(&workspace, root.0.join("moved")).unwrap();
        fs::create_dir(&workspace).unwrap();
        let actual = pin_workspace(&workspace);
        assert_eq!(expected.path(), actual.path());

        let live = root.0.join(LIVE);
        let child_live = live.clone();
        let (mut parent, mut child) = channel_pair();
        let worker = std::thread::spawn(move || {
            let result = generation.commit(child_live, &mut child, &actual, actual.path());
            drop(listener);
            result
        });
        let error = commit_child_binding(&expected, &mut parent).unwrap_err();
        assert_eq!(error.to_string(), "managed daemon workspace mismatch");
        drop(parent);
        assert!(worker.join().unwrap().is_err());
        assert!(!live.exists());
        assert!(!root.0.join(STAGE).exists());
    }

    #[test]
    fn pid_create_failure_keeps_the_bound_stage_private() {
        let root = TestRoot::new();
        let (generation, listener) = root.stage();
        let error = write_pid(&generation.path).unwrap_err();
        assert_eq!(
            error.downcast_ref::<io::Error>().unwrap().kind(),
            io::ErrorKind::AlreadyExists
        );
        assert!(!root.0.join(LIVE).exists());
        assert_eq!(fs::read(generation.path.join(PID)).unwrap(), b"123");
        drop(listener);
        drop(generation);
        assert!(!root.0.join(STAGE).exists());
    }

    #[test]
    fn retiring_an_aborted_stage_respects_the_lifetime_lock_and_preserves_live() {
        let root = TestRoot::new();
        let control = root.0.join(CONTROL);
        let stage = control.join(STAGE);
        let live = control.join(LIVE);
        for path in [&control, &stage, &live] {
            fs::DirBuilder::new().mode(0o700).create(path).unwrap();
        }
        write_pid(&stage).unwrap();
        write_pid(&live).unwrap();
        let lock = open_lock(&control, true).unwrap();
        lock.try_lock().unwrap();
        retire_aborted_stage(&root.0).unwrap();
        assert!(stage.join(PID).is_file());
        drop(lock);
        retire_aborted_stage(&root.0).unwrap();
        assert!(!stage.exists());
        assert!(live.join(PID).is_file());
    }

    #[test]
    fn oversized_failure_frame_is_rejected_before_reading_payload() {
        let (mut parent, mut child) = channel_pair();
        let [high, low] = ((MAX_FAILURE_BYTES + 1) as u16).to_be_bytes();
        child.write_all(&[1, FAILED, high, low]).unwrap();
        assert!(
            parent
                .receive()
                .unwrap_err()
                .to_string()
                .contains("invalid installed daemon startup frame")
        );
    }
}
