use std::time::Duration;
use std::time::Instant;

use slug_bzlmod_v2::HostRepositoryLabelPathValue;
use slug_bzlmod_v2::RepositoryEnvironmentEntry;
use slug_bzlmod_v2::RepositoryEnvironmentSnapshot;
use slug_bzlmod_v2::RepositoryWorkspaceArgument;
use slug_bzlmod_v2::RepositoryWorkspaceEnvironmentValue;
use slug_bzlmod_v2::RepositoryWorkspaceExecute;
use slug_bzlmod_v2::RepositoryWorkspaceOperation;
use slug_bzlmod_v2::RepositoryWorkspaceResult;
use slug_workspace_v2::NormalizedAbsolutePath;
use starlark_map::small_map::SmallMap;

use super::*;
use crate::runtime::repository_physical_root::native_tests::reservation;
use crate::runtime::repository_physical_root::process_owner::ProcessPlatform;
use crate::runtime::repository_physical_root::process_owner::{self};
fn platform() -> ProcessPlatform {
    ProcessPlatform::compiled_cli(
        std::env::var_os("SLUG_V2_BIN")
            .expect("supervisor must select actual compiled CLI")
            .into(),
        "/home/linuxbrew/.linuxbrew/Cellar/bubblewrap/0.11.2/bin/bwrap".into(),
    )
}
fn scope(selected: &RepositoryWorkspaceReservation) -> Arc<SourceServiceScope> {
    let deadline = Instant::now() + Duration::from_secs(26);
    SourceServiceScope::new_for_test(
        selected.workspace().clone(),
        deadline,
        deadline + Duration::from_secs(1),
        crate::runtime::dice::source_service::SourceServiceLimits::default(),
    )
    .unwrap()
}
fn bank() -> Arc<RepositoryRootResources> {
    RepositoryRootResources::new(
        RepositoryRootResourceLimits::new(512 * 1024 * 1024, 512, 4 * ARCHIVE_BYTES).unwrap(),
    )
    .unwrap()
}
fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}
fn execute(
    selected: &RepositoryWorkspaceReservation,
    argv: &[&str],
    cwd: &str,
) -> RepositoryWorkspaceOperation {
    let mut env = SmallMap::new();
    env.insert(
        "EMPTY".into(),
        RepositoryWorkspaceEnvironmentValue::Override("".into()),
    );
    env.insert("REMOVE".into(), RepositoryWorkspaceEnvironmentValue::Remove);
    RepositoryWorkspaceOperation::Execute(
        RepositoryWorkspaceExecute::new(
            argv.iter()
                .map(|s| RepositoryWorkspaceArgument::String((*s).into()))
                .collect::<Vec<_>>()
                .into(),
            RepositoryEnvironmentSnapshot::from_canonical(Arc::from([
                RepositoryEnvironmentEntry::new("BASE", "base-value"),
                RepositoryEnvironmentEntry::new("PATH", "/usr/bin:/bin"),
                RepositoryEnvironmentEntry::new("REMOVE", "gone"),
            ]))
            .unwrap(),
            env,
            600,
            true,
            HostRepositoryLabelPathValue::new(
                NormalizedAbsolutePath::new(selected.display_root().as_path().join(cwd)).unwrap(),
                PathObservationNamespace::Materialization(selected.instance()),
            )
            .unwrap(),
        )
        .unwrap(),
    )
}
fn start(
    selected: Arc<RepositoryWorkspaceReservation>,
    resources: Arc<RepositoryRootResources>,
    scope: Arc<SourceServiceScope>,
    runtime: &tokio::runtime::Runtime,
) -> RepositoryWorkspaceCommand {
    let launch =
        BootstrapLaunch::new(selected.clone(), resources.clone(), scope.clone(), &|| true).unwrap();
    let (builder, seed) = runtime
        .block_on(process_owner::run(launch, platform(), &|| true))
        .unwrap();
    RepositoryWorkspaceCommand::new(
        selected.workspace().clone(),
        selected.lease(),
        selected.limits(),
        resources,
        scope.clone(),
        &|| true,
    )
    .unwrap()
    .reserve_root(builder, seed, &scope, &|| true)
    .unwrap()
}
fn run(
    command: RepositoryWorkspaceCommand,
    selected: &RepositoryWorkspaceReservation,
    operation: RepositoryWorkspaceOperation,
    scope: &Arc<SourceServiceScope>,
    runtime: &tokio::runtime::Runtime,
) -> RepositoryWorkspaceCommand {
    let request = command
        .request(selected, operation, scope, &|| true)
        .unwrap();
    let ExecuteDecision::Launch(launch) = command.admit_execute(request, scope, &|| true).unwrap()
    else {
        panic!("new operation must execute")
    };
    runtime
        .block_on(process_owner::run(launch, platform(), &|| true))
        .unwrap()
}
fn release(bank: &RepositoryRootResources, baseline: usize) {
    let actual = bank.snapshot();
    assert_eq!(
        (
            actual.retained_bytes,
            actual.native_fds,
            actual.backing_bytes
        ),
        (baseline, 0, 0)
    );
}
#[test]
#[ignore = "actual CLI PID1/root/SCM native proof; run only under finite supervisor"]
fn native_general_runner_preserves_same_root_nonzero_env_cwd_replay_and_following_mutations() {
    let temporary = tempfile::tempdir().unwrap();
    let leaf = temporary.path().file_name().unwrap();
    let display =
        std::path::Path::new(crate::runtime::repository_process_wire::RESERVED_PREFIX).join(leaf);
    assert!(!display.exists());
    let selected = reservation(&display, 1901);
    let bank = bank();
    let baseline = bank.snapshot().retained_bytes;
    let scope = scope(&selected);
    let runtime = runtime();
    let command = start(selected.clone(), bank.clone(), scope.clone(), &runtime);
    let operation = execute(
        &selected,
        &[
            "sh",
            "-c",
            "printf '%s|%s|%s|%s' \"$1\" \"$BASE\" \"${EMPTY+x}:$EMPTY\" \"${REMOVE-unset}\"; printf '\\377' >&2; printf actual > value; exit 7",
            "marker",
            "literal ;$(untouched)",
        ],
        "new/sub",
    );
    let command = run(command, &selected, operation.clone(), &scope, &runtime);
    let transcript = command
        .epoch()
        .transcripts()
        .unwrap()
        .iter()
        .find(|t| t.reservation() == &selected)
        .unwrap();
    let first = transcript.records().last().unwrap();
    let RepositoryWorkspaceResult::Execute(result) = first.result() else {
        panic!("actual execute result")
    };
    assert_eq!(result.return_code(), 7);
    assert_eq!(
        result.stdout(),
        b"literal ;$(untouched)|base-value|x:|unset"
    );
    assert_eq!(result.stderr(), [0xff]);
    assert_eq!(bank.snapshot().backing_bytes, PROCESS_BYTES);
    let replay = command
        .request_at(&selected, 0, operation, &scope, &|| true)
        .unwrap();
    let epoch_credit = bank
        .reserve(command.epoch().retained_storage_bytes().unwrap(), 0, 0)
        .unwrap();
    let before = command.epoch().clone();
    let ExecuteDecision::Replay(command) = command.admit_execute(replay, &scope, &|| true).unwrap()
    else {
        panic!("exact completed request must replay")
    };
    assert_eq!(command.epoch(), &before);
    let write = command
        .request(
            &selected,
            RepositoryWorkspaceOperation::File {
                path: "after".into(),
                body: Arc::from(b"trusted mutation".as_slice()),
                mode: 0o644,
            },
            &scope,
            &|| true,
        )
        .unwrap();
    let command = command
        .apply(
            write,
            &runtime,
            &crate::runtime::ArchiveDistdirPolicy::default(),
            &scope,
            &|| true,
        )
        .unwrap();
    let command = run(
        command,
        &selected,
        execute(
            &selected,
            &[
                "/bin/sh",
                "-c",
                "test \"$(cat value)\" = actual; test \"$(cat ../../after)\" = 'trusted mutation'; printf second > value; exit 0",
            ],
            "new/sub",
        ),
        &scope,
        &runtime,
    );
    let count = command.epoch().transcripts().unwrap()[0].records().len();
    let (command, sealed) = command
        .seal_root(&selected, count, &scope, &|| true)
        .unwrap();
    assert_eq!(sealed.mode, RepositoryWorkspaceMode::Process);
    assert!(!display.exists());
    drop(before);
    drop(epoch_credit);
    drop(command);
    drop(sealed);
    release(&bank, baseline);
}

#[test]
#[ignore = "actual CLI confinement/capability/resource proof; finite supervisor required"]
fn native_general_runner_confines_host_proc_devices_sockets_fds_and_process_limits() {
    assert!(
        std::fs::read_link("/run/host/proc/self").is_ok(),
        "explicit nested Host proc fixture required"
    );
    assert!(
        std::fs::File::open("/run/host/proc/self/status").is_ok(),
        "Host proc view must be readable outside the sandbox"
    );
    let host = tempfile::Builder::new()
        .prefix("slug-native-host-")
        .tempdir_in(std::env::current_dir().unwrap())
        .unwrap();
    let protected = host.path().join("protected");
    std::fs::write(&protected, b"Host remains").unwrap();
    let leaf = host.path().file_name().unwrap();
    let display =
        std::path::Path::new(crate::runtime::repository_process_wire::RESERVED_PREFIX).join(leaf);
    let selected = reservation(&display, 1902);
    let bank = bank();
    let baseline = bank.snapshot().retained_bytes;
    let scope = scope(&selected);
    let runtime = runtime();
    let command = start(selected.clone(), bank.clone(), scope.clone(), &runtime);
    let script = r#"import os, errno, resource, socket, ctypes, fcntl, sys, signal, time
for fd in range(3, 16):
    try: os.fstat(fd)
    except OSError as e: assert e.errno == errno.EBADF
    else: raise AssertionError(('inherited fd',fd))
status = open('/proc/self/status').read()
for name in ('CapInh','CapPrm','CapEff','CapBnd','CapAmb'):
    assert int(next(x.split()[1] for x in status.splitlines() if x.startswith(name+':')),16) == 0
assert 'NoNewPrivs:\t1' in status and 'Seccomp:\t2' in status
assert not os.path.exists('/run/host/proc/self')
for path in ('/proc/1/fd/3', '/run/host/proc/self/fd/3'):
    try: fd=os.open(path, os.O_RDWR)
    except OSError as e: assert e.errno in (errno.EACCES,errno.EPERM,errno.ENOENT)
    else: os.close(fd); raise AssertionError('private control escape')
try: open(sys.argv[1],'wb').write(b'changed')
except OSError as e: assert e.errno in (errno.EROFS,errno.EACCES,errno.EPERM)
else: raise AssertionError('Host mutation')
try: socket.socket(socket.AF_UNIX,socket.SOCK_STREAM)
except OSError as e: assert e.errno == errno.EPERM
else: raise AssertionError('socket escape')
with open('/dev/null','rb') as dev:
    try: fcntl.ioctl(dev,0x5413,b'0'*8)
    except OSError as e: assert e.errno == errno.EPERM
    else: raise AssertionError('device ioctl escape')
libc=ctypes.CDLL(None,use_errno=True)
for number,args in ((272,(0x10000000,)),(165,(0,0,0,0,0)),(425,(1,0)),(101,(0,0,0,0))):
    assert libc.syscall(number,*args) == -1 and ctypes.get_errno() == errno.EPERM
try: os.kill(1,signal.SIGKILL)
except OSError as e: assert e.errno == errno.EPERM
else: raise AssertionError('supervisor signal')
assert resource.getrlimit(resource.RLIMIT_NPROC) == (64,64)
children=[]
try:
    for _ in range(65):
        try: child=os.fork()
        except OSError as e: assert e.errno == errno.EAGAIN; break
        if child == 0:
            while True: time.sleep(100)
        children.append(child)
    else: raise AssertionError('NPROC exemption')
    assert len(children)>0 and len(children)<64
finally:
    for child in children: os.kill(child,signal.SIGKILL)
    for child in children: os.waitpid(child,0)
with open('exact','wb') as exact:
    os.posix_fallocate(exact.fileno(),0,64*1024*1024)
    with open('plus','wb') as plus:
        try: os.posix_fallocate(plus.fileno(),0,4096)
        except OSError as e: assert e.errno == errno.ENOSPC
        else: raise AssertionError('physical root plus one')
os.unlink('exact');os.unlink('plus')
print('confinement-ok')
"#;
    let protected_text = protected.to_str().unwrap();
    let command = run(
        command,
        &selected,
        execute(
            &selected,
            &["/usr/bin/python3", "-c", script, protected_text],
            "",
        ),
        &scope,
        &runtime,
    );
    let transcript = &command.epoch().transcripts().unwrap()[0];
    let RepositoryWorkspaceResult::Execute(result) = transcript.records().last().unwrap().result()
    else {
        panic!()
    };
    assert_eq!(
        result.return_code(),
        0,
        "stderr {}",
        String::from_utf8_lossy(result.stderr())
    );
    assert_eq!(result.stdout(), b"confinement-ok\n");
    assert_eq!(std::fs::read(&protected).unwrap(), b"Host remains");
    drop(command);
    release(&bank, baseline);
}

fn fixture(
    instance: u64,
) -> (
    tempfile::TempDir,
    Arc<RepositoryWorkspaceReservation>,
    Arc<RepositoryRootResources>,
    Arc<SourceServiceScope>,
    tokio::runtime::Runtime,
) {
    let temporary = tempfile::tempdir().unwrap();
    let display = std::path::Path::new(crate::runtime::repository_process_wire::RESERVED_PREFIX)
        .join(temporary.path().file_name().unwrap());
    let selected = reservation(&display, instance);
    let resources = bank();
    let scope = scope(&selected);
    (temporary, selected, resources, scope, runtime())
}
fn launch(
    command: RepositoryWorkspaceCommand,
    selected: &RepositoryWorkspaceReservation,
    operation: RepositoryWorkspaceOperation,
    scope: &Arc<SourceServiceScope>,
) -> ExecuteLaunch {
    let request = command
        .request(selected, operation, scope, &|| true)
        .unwrap();
    let ExecuteDecision::Launch(launch) = command.admit_execute(request, scope, &|| true).unwrap()
    else {
        panic!("new request")
    };
    launch
}
fn released_eventually(bank: &RepositoryRootResources, baseline: usize) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let now = bank.snapshot();
        if (now.retained_bytes, now.native_fds, now.backing_bytes) == (baseline, 0, 0) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "cleanup owner still retained: {now:?}"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}
#[test]
#[ignore = "actual CLI detached writer and error-boundary proof; finite supervisor required"]
fn native_general_runner_reaps_detached_closed_stdio_writer_and_distinguishes_exec_errors_and_overflow()
 {
    let (_temporary, selected, bank, scope, runtime) = fixture(1910);
    let baseline = bank.snapshot().retained_bytes;
    let command = start(selected.clone(), bank.clone(), scope.clone(), &runtime);
    let script = r#"import os,time
ready_read,ready_write=os.pipe()
child=os.fork()
if child==0:
    os.close(ready_read)
    os.setsid()
    for fd in (0,1,2): os.close(fd)
    os.write(ready_write,b'R'); os.close(ready_write)
    time.sleep(.2)
    open('delayed','w').write('escaped writer')
    os._exit(0)
os.close(ready_write)
assert os.read(ready_read,1)==b'R'; os.close(ready_read)
print('main-exit',flush=True)
"#;
    let command = run(
        command,
        &selected,
        execute(&selected, &["/usr/bin/python3", "-c", script], ""),
        &scope,
        &runtime,
    );
    let transcript = &command.epoch().transcripts().unwrap()[0];
    let RepositoryWorkspaceResult::Execute(result) = transcript.records().last().unwrap().result()
    else {
        panic!()
    };
    assert_eq!(result.return_code(), 0);
    assert_eq!(result.stdout(), b"main-exit\n");
    assert!(result.stderr().is_empty());
    assert!(
        !transcript
            .current()
            .entries()
            .iter()
            .any(|row| row.path() == "delayed")
    );
    std::thread::sleep(Duration::from_millis(300));
    let count = transcript.records().len();
    let (command, sealed) = command
        .seal_root(&selected, count, &scope, &|| true)
        .unwrap();
    drop(command);
    drop(sealed);
    release(&bank, baseline);
    // A real127 is an accepted normal result with its writes; failedexec has no
    // immutable result and consumes the entire command after actual cleanup.
    let (_temporary, selected, bank, scope, runtime) = fixture(1911);
    let baseline = bank.snapshot().retained_bytes;
    let command = start(selected.clone(), bank.clone(), scope.clone(), &runtime);
    let command = run(
        command,
        &selected,
        execute(
            &selected,
            &["/bin/sh", "-c", "printf before127 > kept; exit 127"],
            "",
        ),
        &scope,
        &runtime,
    );
    let transcript = &command.epoch().transcripts().unwrap()[0];
    let RepositoryWorkspaceResult::Execute(result) = transcript.records().last().unwrap().result()
    else {
        panic!()
    };
    assert_eq!(result.return_code(), 127);
    assert!(
        transcript
            .current()
            .entries()
            .iter()
            .any(|row| row.path() == "kept")
    );
    drop(command);
    release(&bank, baseline);
    for (instance, argv) in [
        (1912, vec!["/definitely/not/a/slug-native-executable"]),
        (1913, vec!["/bin/sh", "-c", "head -c 1048577 /dev/zero"]),
        (1914, vec!["/bin/sh", "-c", "head -c 1048577 /dev/zero >&2"]),
    ] {
        let (_temporary, selected, bank, scope, runtime) = fixture(instance);
        let baseline = bank.snapshot().retained_bytes;
        let command = start(selected.clone(), bank.clone(), scope.clone(), &runtime);
        let attempt = launch(command, &selected, execute(&selected, &argv, ""), &scope);
        let error = match runtime.block_on(process_owner::run(attempt, platform(), &|| true)) {
            Ok(_) => panic!("failure boundary returned a command"),
            Err(error) => error,
        };
        let PhysicalRootError::Io(error) = error else {
            panic!("expected actual native IO failure")
        };
        if instance == 1912 {
            assert_eq!(error.to_string(), "repository helper stage 11 errno 2");
        } else {
            assert_eq!(error.kind(), std::io::ErrorKind::FileTooLarge, "{error}");
        }
        released_eventually(&bank, baseline);
    }
}
#[test]
#[ignore = "actual CLI future-drop/startup-cancel/deadline proof; finite supervisor required"]
fn native_general_runner_future_drop_and_deadline_keep_worker_ownership_until_namespace_death() {
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    async fn until(stage: &AtomicUsize, wanted: usize) {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
        while stage.load(Ordering::Acquire) < wanted {
            assert!(
                tokio::time::Instant::now() < deadline,
                "native stage did not arrive"
            );
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    }
    // Pause only the test driver before HELLO receipt. The actual helper cannot
    // create or receive a repository root without its explicit HELLO_ACK.
    let (_temporary, selected, bank, scope, runtime) = fixture(1920);
    let baseline = bank.snapshot().retained_bytes;
    let stage = Arc::new(AtomicUsize::new(0));
    let startup =
        BootstrapLaunch::new(selected.clone(), bank.clone(), scope.clone(), &|| true).unwrap();
    runtime.block_on(async{let mut future=Box::pin(process_owner::run(startup,platform().observe_for_test(stage.clone(),true),&||true));tokio::select!{result=&mut future=>panic!("held startup returned: {}",result.is_ok()),_=until(&stage,1)=>{}};assert!(bank.snapshot().backing_bytes>=ARCHIVE_BYTES);drop(future);});
    released_eventually(&bank, baseline);
    // Stage3 is set only after actual payload stdout, never from setup/status.
    let (_temporary, selected, bank, scope, runtime) = fixture(1921);
    let baseline = bank.snapshot().retained_bytes;
    let command = start(selected.clone(), bank.clone(), scope.clone(), &runtime);
    let attempt = launch(
        command,
        &selected,
        execute(
            &selected,
            &["/bin/sh", "-c", "printf running; sleep 30"],
            "",
        ),
        &scope,
    );
    let stage = Arc::new(AtomicUsize::new(0));
    runtime.block_on(async{let mut future=Box::pin(process_owner::run(attempt,platform().observe_for_test(stage.clone(),false),&||true));tokio::select!{result=&mut future=>panic!("live payload returned: {}",result.is_ok()),_=until(&stage,3)=>{}};drop(future);});
    released_eventually(&bank, baseline);
    let (_temporary, selected, bank, _unused_scope, runtime) = fixture(1922);
    let baseline = bank.snapshot().retained_bytes;
    let deadline = Instant::now() + Duration::from_secs(3);
    let scope = SourceServiceScope::new_for_test(
        selected.workspace().clone(),
        deadline,
        deadline + Duration::from_secs(1),
        crate::runtime::dice::source_service::SourceServiceLimits::default(),
    )
    .unwrap();
    let command = start(selected.clone(), bank.clone(), scope.clone(), &runtime);
    let attempt = launch(
        command,
        &selected,
        execute(
            &selected,
            &["/bin/sh", "-c", "printf running; sleep 30"],
            "",
        ),
        &scope,
    );
    let stage = Arc::new(AtomicUsize::new(0));
    runtime.block_on(async{let mut future=Box::pin(process_owner::run(attempt,platform().observe_for_test(stage.clone(),false),&||true));tokio::select!{result=&mut future=>panic!("before live marker: {}",result.is_ok()),_=until(&stage,3)=>{}};assert!(future.await.is_err());});
    released_eventually(&bank, baseline);
}
