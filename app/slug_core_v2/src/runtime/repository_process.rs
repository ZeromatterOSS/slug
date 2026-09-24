//! Confined, source-derived GCC probes for selected repository rules.
//!
//! This is deliberately not a general `repository_ctx.execute` transport.
//! Loading produces a typed, replay-checked request; Core stages its preceding
//! file effects and launches only the finite rules_cc Linux GCC probe grammar.

use std::fmt;
#[cfg(not(all(target_os = "linux", target_env = "gnu")))]
use std::path::Path;

#[cfg(not(all(target_os = "linux", target_env = "gnu")))]
use slug_bzlmod_v2::RepositoryExecuteRequest;
#[cfg(not(all(target_os = "linux", target_env = "gnu")))]
use slug_bzlmod_v2::RepositoryExecuteResult;

#[derive(Debug)]
pub(super) enum RepositoryProcessError {
    Policy(&'static str),
    Launcher(&'static str),
    LauncherSpawn(Option<i32>),
    Prefix(&'static str),
    Sandbox(&'static str),
    OutputLimit,
    Timeout,
}

impl fmt::Display for RepositoryProcessError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Policy(reason) => write!(formatter, "repository execute policy: {reason}"),
            Self::Launcher(reason) => write!(formatter, "repository execute launcher: {reason}"),
            Self::LauncherSpawn(Some(errno)) => {
                write!(
                    formatter,
                    "repository execute launcher: sandbox launch failed (os error {errno})"
                )
            }
            Self::LauncherSpawn(None) => {
                formatter.write_str("repository execute launcher: sandbox launch failed")
            }
            Self::Prefix(reason) => write!(formatter, "repository execute prefix: {reason}"),
            Self::Sandbox(reason) => write!(formatter, "repository execute sandbox: {reason}"),
            Self::OutputLimit => formatter.write_str("repository execute output limit exceeded"),
            Self::Timeout => formatter.write_str("repository execute timed out"),
        }
    }
}

impl std::error::Error for RepositoryProcessError {}

#[cfg(not(all(target_os = "linux", target_env = "gnu")))]
pub(super) async fn run_repository_execute(
    _request: &RepositoryExecuteRequest,
    _workspace: &Path,
    _bwrap_path: &Path,
) -> Result<RepositoryExecuteResult, RepositoryProcessError> {
    Err(RepositoryProcessError::Policy(
        "confined GCC probes require Linux with glibc",
    ))
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
mod linux {
    use std::ffi::CStr;
    use std::ffi::CString;
    use std::ffi::OsString;
    use std::fs::File;
    use std::fs::Metadata;
    use std::io;
    use std::io::Read;
    use std::io::Seek;
    use std::mem::MaybeUninit;
    use std::os::fd::AsRawFd;
    use std::os::fd::FromRawFd;
    use std::os::fd::OwnedFd;
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::MetadataExt;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use std::ptr;
    use std::sync::Arc;
    use std::time::Duration;

    use slug_bzlmod_v2::GeneratedRepositoryFileEffectPlan;
    use slug_bzlmod_v2::GeneratedRepositoryOrderedEffectRef;
    use slug_bzlmod_v2::RepositoryEnvironmentSnapshot;
    use slug_bzlmod_v2::RepositoryExecuteArg;
    use slug_bzlmod_v2::RepositoryExecuteOptions;
    use slug_bzlmod_v2::RepositoryExecuteRequest;
    use slug_bzlmod_v2::RepositoryExecuteResult;
    use tokio::io::unix::AsyncFd;

    use super::RepositoryProcessError;

    const EXEC_ROOT: &str = "/exec";
    const EXEC_SOURCE: &str = "/exec/tools/cpp/empty.cc";
    const SOURCE_EFFECT: &str = "tools/cpp/empty.cc";
    const SOURCE_BYTES: &[u8] = b"int main() {}";
    const TEMPLATE_EFFECTS: [&str; 3] = [
        "validate_static_library.sh",
        "cc_wrapper.sh",
        "deps_scanner_wrapper.sh",
    ];
    const MAX_TEMPLATE_EFFECT_BYTES: usize = 64 * 1024;
    const MAX_TEMPLATE_PREFIX_BYTES: usize = 128 * 1024;
    const MAX_STREAM_BYTES: usize = 1024 * 1024;
    const MAX_STATUS_BYTES: u64 = 4096;
    const TMPFS_BYTES: &str = "67108864";
    const MAX_RUNTIME: Duration = Duration::from_secs(30);
    const REAP_TIMEOUT: Duration = Duration::from_secs(3);

    const COMPILER_OPTIONS: &[&str] = &[
        "-print-resource-dir",
        "-fno-canonical-system-headers",
        "-no-canonical-prefixes",
        "-Wthread-safety",
        "-Wself-assign",
        "-Wunused-but-set-parameter",
        "-Wno-free-nonheap-object",
        "-fcolor-diagnostics",
        "-pass-exit-codes",
    ];
    const LINKER_OPTIONS: &[&str] = &[
        "-Wl,-no-as-needed",
        "-Wl,--push-state",
        "-Wl,-z,relro,-z,now",
        "-Wl,--gc-sections",
    ];
    const DISALLOWED_COMPILER_ENV: &[&str] = &[
        "COMPILER_PATH",
        "LIBRARY_PATH",
        "CPATH",
        "C_INCLUDE_PATH",
        "OBJC_INCLUDE_PATH",
        "OBJCPLUS_INCLUDE_PATH",
        "GCC_EXEC_PREFIX",
        "GCC_SPECS",
        "GCC_COMPARE_DEBUG",
        "LD_PRELOAD",
        "LD_LIBRARY_PATH",
        "LD_AUDIT",
        "TMPDIR",
        "SOURCE_DATE_EPOCH",
    ];

    pub(crate) async fn run_repository_execute(
        request: &RepositoryExecuteRequest,
        workspace: &Path,
        bwrap_path: &Path,
    ) -> Result<RepositoryExecuteResult, RepositoryProcessError> {
        validate_launcher(workspace, bwrap_path)?;
        let argv = guarded_argv(request)?;
        let staged = super::super::repository_io::stage_generated_repository_file_effect_prefix(
            request.prefix(),
            workspace,
        )
        .map_err(|_| RepositoryProcessError::Prefix("could not stage ordered file effects"))?;
        let (return_code, stdout, stderr) =
            run_sandboxed(bwrap_path, staged.path(), &argv, MAX_RUNTIME).await?;
        Ok(RepositoryExecuteResult::new(
            return_code,
            Arc::from(stdout),
            Arc::from(stderr),
        ))
    }

    fn validate_launcher(
        workspace: &Path,
        bwrap_path: &Path,
    ) -> Result<(), RepositoryProcessError> {
        if !workspace.is_absolute() || !bwrap_path.is_absolute() {
            return Err(RepositoryProcessError::Launcher(
                "workspace and launcher must be absolute paths",
            ));
        }
        let resolved_workspace = workspace
            .canonicalize()
            .map_err(|_| RepositoryProcessError::Launcher("workspace cannot be resolved"))?;
        let resolved_launcher = bwrap_path
            .canonicalize()
            .map_err(|_| RepositoryProcessError::Launcher("launcher is unavailable"))?;
        if bwrap_path.starts_with(workspace) || resolved_launcher.starts_with(&resolved_workspace) {
            return Err(RepositoryProcessError::Launcher(
                "launcher is inside the workspace",
            ));
        }
        let metadata = resolved_launcher
            .metadata()
            .map_err(|_| RepositoryProcessError::Launcher("launcher metadata is unavailable"))?;
        if !metadata.is_file() || metadata.permissions().mode() & 0o111 == 0 {
            return Err(RepositoryProcessError::Launcher(
                "launcher is not an executable regular file",
            ));
        }
        Ok(())
    }

    fn guarded_argv(
        request: &RepositoryExecuteRequest,
    ) -> Result<Vec<OsString>, RepositoryProcessError> {
        if request.options() != RepositoryExecuteOptions::DefaultLocalRepositoryCwd {
            return Err(RepositoryProcessError::Policy(
                "nondefault execute options are unsupported",
            ));
        }
        validate_environment(request.environment())?;
        let generated_repository = request.generated_repository();
        if generated_repository.is_empty() || generated_repository.contains(['/', '\\', '\0']) {
            return Err(RepositoryProcessError::Policy(
                "invalid generated repository name",
            ));
        }
        let expanded_prefix = validate_prefix(request.prefix())?;
        validate_template_environment(request.environment(), expanded_prefix)?;
        let Some(RepositoryExecuteArg::Which(compiler)) = request.argv().first() else {
            return Err(RepositoryProcessError::Policy(
                "compiler is not a which-origin path",
            ));
        };
        if compiler.path_str() != "/usr/bin/gcc" {
            return Err(RepositoryProcessError::Policy(
                "selected compiler is outside the admitted system GCC path",
            ));
        }
        validate_system_executable(Path::new(compiler.path_str()))?;

        let mut raw_tail = Vec::with_capacity(request.argv().len().saturating_sub(1));
        for argument in &request.argv()[1..] {
            let RepositoryExecuteArg::String(value) = argument else {
                return Err(RepositoryProcessError::Policy(
                    "only the compiler may be a path-valued argument",
                ));
            };
            raw_tail.push(value.as_str());
        }
        let logical_source = format!("@@{generated_repository}//{SOURCE_EFFECT}");
        let form = match_form(&raw_tail, &logical_source)?;
        if form.uses_linker() {
            validate_system_executable(Path::new("/usr/bin/ld"))?;
            if form.requires_linker_variant("gold") {
                validate_system_executable(Path::new("/usr/bin/ld.gold"))?;
            }
            if form.requires_linker_variant("lld") {
                validate_system_executable(Path::new("/usr/bin/ld.lld"))?;
            }
            if let ProbeForm::LinkerDiscovery(variant) = form {
                let candidate = match variant {
                    "gold" => Path::new("/usr/bin/ld.gold"),
                    "lld" => Path::new("/usr/bin/ld.lld"),
                    _ => unreachable!("probe-form parser admits only gold or lld"),
                };
                if candidate.exists() {
                    validate_system_executable(candidate)?;
                }
            }
        }
        let argv = std::iter::once(OsString::from(compiler.path_str()))
            .chain(raw_tail.into_iter().map(|argument| {
                if argument == logical_source {
                    OsString::from(EXEC_SOURCE)
                } else {
                    OsString::from(argument)
                }
            }))
            .collect();
        Ok(argv)
    }

    fn validate_environment(
        environment: &RepositoryEnvironmentSnapshot,
    ) -> Result<(), RepositoryProcessError> {
        if environment.get("CC").is_some() || environment.get("CPLUS_INCLUDE_PATH").is_some() {
            return Err(RepositoryProcessError::Policy(
                "compiler or include-path override is unsupported",
            ));
        }
        if environment
            .get("BAZEL_CONLYOPTS")
            .is_some_and(|value| !value.is_empty())
            || environment
                .get("BAZEL_CXXOPTS")
                .is_some_and(|value| value.as_ref() != "-std=c++17")
            || DISALLOWED_COMPILER_ENV
                .iter()
                .any(|name| environment.get(name).is_some())
        {
            return Err(RepositoryProcessError::Policy(
                "dynamic compiler environment is unsupported",
            ));
        }
        Ok(())
    }

    fn validate_template_environment(
        environment: &RepositoryEnvironmentSnapshot,
        expanded_prefix: bool,
    ) -> Result<(), RepositoryProcessError> {
        if expanded_prefix && environment.get("HOMEBREW_RUBY_PATH").is_some() {
            return Err(RepositoryProcessError::Policy(
                "generated GCC template prefix with Homebrew environment is unsupported",
            ));
        }
        Ok(())
    }

    fn validate_prefix(
        prefix: &GeneratedRepositoryFileEffectPlan,
    ) -> Result<bool, RepositoryProcessError> {
        if prefix.has_archives() {
            return Err(RepositoryProcessError::Policy(
                "archive effects cannot precede GCC probes",
            ));
        }
        // The offline probe has only this source. unix_cc_configure.bzl first
        // authors precisely these two links, in order. Reject every other
        // effect before the writer can materialize it.
        let mut effects = prefix.ordered_effects();
        let (source_effect, has_source_links) = match effects.next() {
            Some(GeneratedRepositoryOrderedEffectRef::File(file)) => (file, false),
            Some(GeneratedRepositoryOrderedEffectRef::Symlink(first))
                if first.path() == "cc_toolchain_config.bzl" =>
            {
                let Some(GeneratedRepositoryOrderedEffectRef::Symlink(second)) = effects.next()
                else {
                    return Err(RepositoryProcessError::Policy(
                        "generated GCC probe prefix has an unexpected effect",
                    ));
                };
                if second.path() != "armeabi_cc_toolchain_config.bzl" {
                    return Err(RepositoryProcessError::Policy(
                        "generated GCC probe symlink destination is unsupported",
                    ));
                }
                let Some(GeneratedRepositoryOrderedEffectRef::File(file)) = effects.next() else {
                    return Err(RepositoryProcessError::Policy(
                        "missing generated GCC probe source",
                    ));
                };
                (file, true)
            }
            _ => {
                return Err(RepositoryProcessError::Policy(
                    "generated GCC probe prefix has an unexpected effect",
                ));
            }
        };
        if source_effect.path() != SOURCE_EFFECT {
            return Err(RepositoryProcessError::Policy(
                "generated GCC probe prefix has an unexpected effect",
            ));
        }
        if source_effect.content() != SOURCE_BYTES || !source_effect.executable() {
            return Err(RepositoryProcessError::Policy(
                "generated GCC probe source differs from the audited file",
            ));
        }
        let Some(first_template) = effects.next() else {
            return Ok(false);
        };
        if !has_source_links {
            return Err(RepositoryProcessError::Policy(
                "generated GCC probe prefix has an unexpected effect",
            ));
        }
        let mut template_effects = std::iter::once(first_template).chain(effects);
        let mut template_bytes = 0;
        for destination in TEMPLATE_EFFECTS {
            let Some(effect) = template_effects.next() else {
                return Err(RepositoryProcessError::Policy(
                    "generated GCC template prefix is incomplete",
                ));
            };
            let GeneratedRepositoryOrderedEffectRef::File(file) = effect else {
                return Err(RepositoryProcessError::Policy(
                    "generated GCC template prefix has an unexpected effect",
                ));
            };
            if file.path() != destination || !file.executable() {
                return Err(RepositoryProcessError::Policy(
                    "generated GCC template destination or mode is unsupported",
                ));
            }
            if file.content().len() > MAX_TEMPLATE_EFFECT_BYTES {
                return Err(RepositoryProcessError::Policy(
                    "generated GCC template effect exceeds the byte limit",
                ));
            }
            template_bytes += file.content().len();
        }
        if template_effects.next().is_some() || template_bytes > MAX_TEMPLATE_PREFIX_BYTES {
            return Err(RepositoryProcessError::Policy(
                "generated GCC template prefix has an unexpected effect or exceeds the byte limit",
            ));
        }
        Ok(true)
    }

    #[derive(Clone, Copy)]
    enum ProbeForm {
        Version,
        Compiler,
        Include,
        LinkerDiscovery(&'static str),
        LinkerOption(Option<&'static str>),
    }

    impl ProbeForm {
        fn uses_linker(self) -> bool {
            matches!(self, Self::LinkerDiscovery(_) | Self::LinkerOption(_))
        }

        fn requires_linker_variant(self, variant: &str) -> bool {
            match self {
                Self::LinkerOption(Some(selected)) => selected == variant,
                _ => false,
            }
        }
    }

    fn match_form(
        tail: &[&str],
        logical_source: &str,
    ) -> Result<ProbeForm, RepositoryProcessError> {
        if matches!(tail, ["-v"] | ["--version"]) {
            return Ok(ProbeForm::Version);
        }
        if let [option, "-o", "/dev/null", "-c", source] = tail
            && *source == logical_source
            && COMPILER_OPTIONS.contains(option)
        {
            return Ok(ProbeForm::Compiler);
        }
        if let [
            source,
            "-o",
            "/dev/null",
            "-Wl,--start-lib",
            "-Wl,--end-lib",
            linker,
            "-v",
        ] = tail
            && *source == logical_source
        {
            return match *linker {
                "-fuse-ld=lld" => Ok(ProbeForm::LinkerDiscovery("lld")),
                "-fuse-ld=gold" => Ok(ProbeForm::LinkerDiscovery("gold")),
                _ => Err(RepositoryProcessError::Policy(
                    "linker discovery uses an unsupported linker",
                )),
            };
        }
        if let Some(index) = tail.iter().position(|value| LINKER_OPTIONS.contains(value))
            && tail[index + 1..] == ["-o", "/dev/null", logical_source]
        {
            let prefix = &tail[..index];
            let variant = match prefix {
                ["-B/usr/bin"] => None,
                ["-fuse-ld=gold", "-B/usr/bin"] => Some("gold"),
                ["-fuse-ld=lld", "-B/usr/bin"] => Some("lld"),
                _ => {
                    return Err(RepositoryProcessError::Policy(
                        "linker force flags are outside the audited system paths",
                    ));
                }
            };
            return Ok(ProbeForm::LinkerOption(variant));
        }
        if let ["-E", language @ ("-xc" | "-xc++"), "-", "-v", flags @ ..] = tail
            && include_flags_match(language, flags)
        {
            return Ok(ProbeForm::Include);
        }
        if let ["-print-resource-dir", flags @ ..] = tail
            && resource_flags_match(flags)
        {
            return Ok(ProbeForm::Include);
        }
        Err(RepositoryProcessError::Policy(
            "argv is outside the audited rules_cc GCC probe grammar",
        ))
    }

    fn include_flags_match(language: &str, flags: &[&str]) -> bool {
        match language {
            "-xc" => matches!(
                flags,
                [] | ["-fno-canonical-system-headers"] | ["-no-canonical-prefixes"]
            ),
            "-xc++" => matches!(
                flags,
                ["-std=c++17"]
                    | ["-std=c++17", "-stdlib=libc++"]
                    | ["-std=c++17", "-fno-canonical-system-headers"]
                    | ["-std=c++17", "-no-canonical-prefixes"]
                    | [
                        "-std=c++17",
                        "-fno-canonical-system-headers",
                        "-stdlib=libc++"
                    ]
                    | ["-std=c++17", "-no-canonical-prefixes", "-stdlib=libc++"]
            ),
            _ => false,
        }
    }

    fn resource_flags_match(flags: &[&str]) -> bool {
        matches!(
            flags,
            [] | ["-fno-canonical-system-headers"] | ["-no-canonical-prefixes"]
        ) || include_flags_match("-xc++", flags)
    }

    fn validate_system_executable(path: &Path) -> Result<(), RepositoryProcessError> {
        if !path.starts_with("/usr/bin") {
            return Err(RepositoryProcessError::Policy(
                "tool is outside the trusted system bin directory",
            ));
        }
        let resolved = path
            .canonicalize()
            .map_err(|_| RepositoryProcessError::Policy("trusted system tool is unavailable"))?;
        if !resolved.starts_with("/usr") {
            return Err(RepositoryProcessError::Policy(
                "trusted system tool resolves outside /usr",
            ));
        }
        for ancestor in resolved
            .ancestors()
            .take_while(|ancestor| *ancestor != Path::new("/"))
        {
            trusted_metadata(&ancestor.metadata().map_err(|_| {
                RepositoryProcessError::Policy("trusted system tool metadata is unavailable")
            })?)?;
        }
        let metadata = resolved.metadata().map_err(|_| {
            RepositoryProcessError::Policy("trusted system tool metadata is unavailable")
        })?;
        if !metadata.is_file() || metadata.permissions().mode() & 0o111 == 0 {
            return Err(RepositoryProcessError::Policy(
                "trusted system tool is not an executable regular file",
            ));
        }
        let mut file = File::open(&resolved)
            .map_err(|_| RepositoryProcessError::Policy("trusted system tool cannot be opened"))?;
        let mut magic = [0_u8; 4];
        file.read_exact(&mut magic)
            .map_err(|_| RepositoryProcessError::Policy("trusted system tool cannot be read"))?;
        if magic != *b"\x7fELF" {
            return Err(RepositoryProcessError::Policy(
                "trusted system tool is not ELF",
            ));
        }
        Ok(())
    }

    fn trusted_metadata(metadata: &Metadata) -> Result<(), RepositoryProcessError> {
        if metadata.uid() != 0 || metadata.permissions().mode() & 0o022 != 0 {
            return Err(RepositoryProcessError::Policy(
                "trusted system tool chain is writable or non-root-owned",
            ));
        }
        Ok(())
    }

    async fn run_sandboxed(
        bwrap_path: &Path,
        prefix_root: &Path,
        argv: &[OsString],
        runtime_limit: Duration,
    ) -> Result<(i32, Vec<u8>, Vec<u8>), RepositoryProcessError> {
        let mut status_file = fixed_status_file()?;
        let (mut child, stdout, stderr) = spawn_bwrap(bwrap_path, prefix_root, argv, &status_file)?;
        let completed = tokio::time::timeout(runtime_limit, async {
            let (stdout, stderr) =
                tokio::try_join!(capture_bounded(stdout), capture_bounded(stderr))?;
            // waitpid(WNOHANG) is the only reaper. Until both pipes reach EOF,
            // the launcher PID remains reserved even if it has exited.
            let exit = child.wait_reap().await?;
            Ok((stdout, stderr, exit))
        })
        .await;
        let (stdout, stderr, exit) = match completed {
            Ok(Ok(value)) => value,
            Ok(Err(error)) => {
                child.kill_and_reap().await;
                return Err(error);
            }
            Err(_) => {
                child.kill_and_reap().await;
                return Err(RepositoryProcessError::Timeout);
            }
        };
        let status_code = sandbox_exit_code(&mut status_file)?;
        if Some(status_code) != exit {
            return Err(RepositoryProcessError::Sandbox(
                "sandbox status and child exit differ",
            ));
        }
        Ok((status_code, stdout, stderr))
    }

    fn bwrap_argv(
        bwrap_path: &Path,
        prefix_root: &Path,
        argv: &[OsString],
    ) -> Result<Vec<CString>, RepositoryProcessError> {
        let mut arguments: Vec<OsString> = std::iter::once(bwrap_path.as_os_str().to_owned())
            .chain(
                [
                    "--unshare-user",
                    "--unshare-pid",
                    "--unshare-net",
                    "--unshare-ipc",
                    "--unshare-uts",
                    "--die-with-parent",
                    "--clearenv",
                    "--ro-bind",
                    "/usr",
                    "/usr",
                    "--symlink",
                    "usr/bin",
                    "/bin",
                    "--symlink",
                    "usr/lib",
                    "/lib",
                    "--symlink",
                    "usr/lib64",
                    "/lib64",
                    "--dev",
                    "/dev",
                    "--proc",
                    "/proc",
                    "--size",
                    TMPFS_BYTES,
                    "--tmpfs",
                    "/tmp",
                    "--ro-bind",
                ]
                .into_iter()
                .map(OsString::from),
            )
            .collect();
        arguments.push(prefix_root.as_os_str().to_owned());
        arguments.push(OsString::from(EXEC_ROOT));
        arguments.extend(
            [
                "--chdir",
                EXEC_ROOT,
                "--setenv",
                "PATH",
                "/usr/bin:/bin",
                "--setenv",
                "LC_ALL",
                "C",
                "--setenv",
                "TMPDIR",
                "/tmp",
                "--json-status-fd",
                "3",
                "--",
            ]
            .into_iter()
            .map(OsString::from),
        );
        arguments.extend(argv.iter().cloned());
        arguments
            .into_iter()
            .map(|argument| {
                CString::new(argument.as_bytes()).map_err(|_| {
                    RepositoryProcessError::Policy("repository execute argv contains NUL")
                })
            })
            .collect()
    }

    type AddClosefrom =
        unsafe extern "C" fn(*mut libc::posix_spawn_file_actions_t, libc::c_int) -> libc::c_int;

    fn resolve_closefrom_with(
        resolve: impl FnOnce(&CStr) -> *mut libc::c_void,
    ) -> Result<AddClosefrom, RepositoryProcessError> {
        let name = CStr::from_bytes_with_nul(b"posix_spawn_file_actions_addclosefrom_np\0")
            .expect("static symbol name is terminated");
        let symbol = resolve(name);
        if symbol.is_null() {
            return Err(RepositoryProcessError::Launcher(
                "glibc close-from spawn action is unavailable",
            ));
        }
        Ok(unsafe { std::mem::transmute::<*mut libc::c_void, AddClosefrom>(symbol) })
    }

    fn resolve_closefrom() -> Result<AddClosefrom, RepositoryProcessError> {
        resolve_closefrom_with(|name| unsafe { libc::dlsym(libc::RTLD_DEFAULT, name.as_ptr()) })
    }

    fn spawn_code(code: libc::c_int) -> Result<(), RepositoryProcessError> {
        if code == 0 {
            Ok(())
        } else {
            Err(RepositoryProcessError::LauncherSpawn(Some(code)))
        }
    }

    struct SpawnActions(libc::posix_spawn_file_actions_t);

    impl SpawnActions {
        fn new() -> Result<Self, RepositoryProcessError> {
            let mut value = MaybeUninit::uninit();
            spawn_code(unsafe { libc::posix_spawn_file_actions_init(value.as_mut_ptr()) })?;
            Ok(Self(unsafe { value.assume_init() }))
        }

        fn ptr(&mut self) -> *mut libc::posix_spawn_file_actions_t {
            &mut self.0
        }
    }

    impl Drop for SpawnActions {
        fn drop(&mut self) {
            unsafe { libc::posix_spawn_file_actions_destroy(self.ptr()) };
        }
    }

    struct SpawnAttributes(libc::posix_spawnattr_t);

    impl SpawnAttributes {
        fn new() -> Result<Self, RepositoryProcessError> {
            let mut value = MaybeUninit::uninit();
            spawn_code(unsafe { libc::posix_spawnattr_init(value.as_mut_ptr()) })?;
            Ok(Self(unsafe { value.assume_init() }))
        }

        fn ptr(&mut self) -> *mut libc::posix_spawnattr_t {
            &mut self.0
        }
    }

    impl Drop for SpawnAttributes {
        fn drop(&mut self) {
            unsafe { libc::posix_spawnattr_destroy(self.ptr()) };
        }
    }

    fn pipe_pair() -> Result<(OwnedFd, OwnedFd), RepositoryProcessError> {
        let mut descriptors = [-1, -1];
        if unsafe { libc::pipe2(descriptors.as_mut_ptr(), libc::O_CLOEXEC) } == -1 {
            return Err(RepositoryProcessError::Sandbox(
                "capture pipe creation failed",
            ));
        }
        Ok(unsafe {
            (
                OwnedFd::from_raw_fd(descriptors[0]),
                OwnedFd::from_raw_fd(descriptors[1]),
            )
        })
    }

    fn duplicate_source(file: &impl AsRawFd) -> Result<OwnedFd, RepositoryProcessError> {
        let descriptor = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 4) };
        if descriptor < 4 {
            return Err(RepositoryProcessError::Sandbox(
                "spawn source descriptor normalization failed",
            ));
        }
        Ok(unsafe { OwnedFd::from_raw_fd(descriptor) })
    }

    fn nonblocking_pipe(file: OwnedFd) -> Result<AsyncFd<OwnedFd>, RepositoryProcessError> {
        let flags = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETFL) };
        if flags == -1
            || unsafe { libc::fcntl(file.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) }
                == -1
        {
            return Err(RepositoryProcessError::Sandbox("capture pipe setup failed"));
        }
        AsyncFd::new(file)
            .map_err(|_| RepositoryProcessError::Sandbox("capture pipe registration failed"))
    }

    fn spawn_bwrap(
        bwrap_path: &Path,
        prefix_root: &Path,
        argv: &[OsString],
        status_file: &File,
    ) -> Result<(ChildReaper, AsyncFd<OwnedFd>, AsyncFd<OwnedFd>), RepositoryProcessError> {
        let closefrom = resolve_closefrom()?;
        let arguments = bwrap_argv(bwrap_path, prefix_root, argv)?;
        let argv_pointers: Vec<*mut libc::c_char> = arguments
            .iter()
            .map(|argument| argument.as_ptr().cast_mut())
            .chain(std::iter::once(ptr::null_mut()))
            .collect();
        let environment = [ptr::null_mut()];
        let (stdout_read, stdout_write) = pipe_pair()?;
        let (stderr_read, stderr_write) = pipe_pair()?;
        let stdout_source = duplicate_source(&stdout_write)?;
        let stderr_source = duplicate_source(&stderr_write)?;
        let status_source = duplicate_source(status_file)?;
        // The child sees only normalized sources >=4. This is important when
        // daemon descriptors 0..=3 are closed or reassigned by its caller.
        drop(stdout_write);
        drop(stderr_write);
        let stdout_read = nonblocking_pipe(stdout_read)?;
        let stderr_read = nonblocking_pipe(stderr_read)?;

        let mut actions = SpawnActions::new()?;
        let dev_null =
            CStr::from_bytes_with_nul(b"/dev/null\0").expect("static path is terminated");
        spawn_code(unsafe {
            libc::posix_spawn_file_actions_addopen(
                actions.ptr(),
                0,
                dev_null.as_ptr(),
                libc::O_RDONLY,
                0,
            )
        })?;
        for (source, target) in [
            (&stdout_source, 1),
            (&stderr_source, 2),
            (&status_source, 3),
        ] {
            spawn_code(unsafe {
                libc::posix_spawn_file_actions_adddup2(actions.ptr(), source.as_raw_fd(), target)
            })?;
        }
        spawn_code(unsafe { closefrom(actions.ptr(), 4) })?;

        let mut attributes = SpawnAttributes::new()?;
        spawn_code(unsafe {
            libc::posix_spawnattr_setflags(attributes.ptr(), libc::POSIX_SPAWN_SETSID)
        })?;
        let mut pid = 0;
        let result = unsafe {
            libc::posix_spawn(
                &mut pid,
                arguments[0].as_ptr(),
                actions.ptr(),
                attributes.ptr(),
                argv_pointers.as_ptr(),
                environment.as_ptr(),
            )
        };
        spawn_code(result)?;
        if pid <= 0 {
            return Err(RepositoryProcessError::Sandbox(
                "spawn did not return a positive launcher PID",
            ));
        }
        // All writer/source copies close before capture begins. The reader
        // FDs are the only remaining parent endpoints for these pipes.
        drop(stdout_source);
        drop(stderr_source);
        drop(status_source);
        Ok((ChildReaper::new(pid), stdout_read, stderr_read))
    }

    fn fixed_status_file() -> Result<File, RepositoryProcessError> {
        let status_parent = Path::new("/tmp");
        let status_parent_metadata = std::fs::symlink_metadata(status_parent)
            .map_err(|_| RepositoryProcessError::Sandbox("status parent is unavailable"))?;
        if !status_parent_metadata.file_type().is_dir() {
            return Err(RepositoryProcessError::Sandbox(
                "status parent is not a real directory",
            ));
        }
        tempfile::tempfile_in(status_parent)
            .map_err(|_| RepositoryProcessError::Sandbox("status file creation failed"))
    }

    fn sandbox_exit_code(file: &mut File) -> Result<i32, RepositoryProcessError> {
        if file
            .metadata()
            .map_err(|_| RepositoryProcessError::Sandbox("sandbox status unavailable"))?
            .len()
            > MAX_STATUS_BYTES
        {
            return Err(RepositoryProcessError::Sandbox(
                "sandbox status exceeded its bound",
            ));
        }
        file.rewind()
            .map_err(|_| RepositoryProcessError::Sandbox("sandbox status rewind failed"))?;
        let mut status = String::new();
        file.read_to_string(&mut status)
            .map_err(|_| RepositoryProcessError::Sandbox("sandbox status is not UTF-8"))?;
        let mut lines = status.lines();
        if !lines
            .next()
            .is_some_and(|line| line.contains("\"child-pid\""))
        {
            return Err(RepositoryProcessError::Sandbox(
                "sandbox did not launch a child",
            ));
        }
        let code_line = lines.next().ok_or(RepositoryProcessError::Sandbox(
            "sandbox did not report a child exit",
        ))?;
        if lines.next().is_some() {
            return Err(RepositoryProcessError::Sandbox(
                "sandbox reported unexpected status",
            ));
        }
        let (_, code) =
            code_line
                .split_once("\"exit-code\"")
                .ok_or(RepositoryProcessError::Sandbox(
                    "sandbox did not report a child exit",
                ))?;
        code.trim()
            .strip_prefix(':')
            .and_then(|code| code.trim().strip_suffix('}'))
            .and_then(|code| code.trim().parse::<i32>().ok())
            .ok_or(RepositoryProcessError::Sandbox(
                "sandbox child exit status is invalid",
            ))
    }

    async fn capture_bounded(stream: AsyncFd<OwnedFd>) -> Result<Vec<u8>, RepositoryProcessError> {
        capture_bounded_with(stream, |fd, buffer| {
            let count = unsafe { libc::read(fd, buffer.as_mut_ptr().cast(), buffer.len()) };
            if count < 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(count as usize)
            }
        })
        .await
    }

    async fn capture_bounded_with(
        stream: AsyncFd<OwnedFd>,
        mut read: impl FnMut(libc::c_int, &mut [u8]) -> io::Result<usize>,
    ) -> Result<Vec<u8>, RepositoryProcessError> {
        let mut bytes = Vec::new();
        let mut buffer = [0_u8; 8192];
        loop {
            let mut ready = stream
                .readable()
                .await
                .map_err(|_| RepositoryProcessError::Sandbox("sandbox output read failed"))?;
            let count = match ready.try_io(|inner| read(inner.get_ref().as_raw_fd(), &mut buffer)) {
                Ok(Ok(count)) => count,
                Ok(Err(error)) if error.kind() == io::ErrorKind::Interrupted => continue,
                Ok(Err(_)) => {
                    return Err(RepositoryProcessError::Sandbox(
                        "sandbox output read failed",
                    ));
                }
                Err(_) => continue,
            };
            if count == 0 {
                return Ok(bytes);
            }
            if count > MAX_STREAM_BYTES - bytes.len() {
                return Err(RepositoryProcessError::OutputLimit);
            }
            bytes.extend_from_slice(&buffer[..count]);
        }
    }

    struct ChildReaper {
        pid: libc::pid_t,
        owned: bool,
    }

    impl ChildReaper {
        fn new(pid: libc::pid_t) -> Self {
            Self { pid, owned: true }
        }

        fn try_reap(&mut self) -> Result<Option<Option<i32>>, RepositoryProcessError> {
            let mut status = 0;
            let result = unsafe { libc::waitpid(self.pid, &mut status, libc::WNOHANG) };
            if result == 0 {
                return Ok(None);
            }
            if result == self.pid {
                self.owned = false;
                return Ok(Some(if libc::WIFEXITED(status) {
                    Some(libc::WEXITSTATUS(status))
                } else {
                    None
                }));
            }
            let errno = io::Error::last_os_error().raw_os_error();
            if errno == Some(libc::EINTR) {
                return Ok(None);
            }
            // ECHILD means the PID might be reusable. Never signal that group
            // after ownership has been lost, even on an infrastructure error.
            self.owned = false;
            Err(RepositoryProcessError::Sandbox(
                "launcher child ownership was lost",
            ))
        }

        async fn wait_reap(&mut self) -> Result<Option<i32>, RepositoryProcessError> {
            loop {
                if let Some(exit) = self.try_reap()? {
                    return Ok(exit);
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }

        fn kill_group(&mut self) {
            if !self.owned {
                return;
            }
            // WNOWAIT proves the direct launcher remains our unreaped child;
            // it does not release the PID before the process-group signal.
            let mut info = MaybeUninit::<libc::siginfo_t>::zeroed();
            loop {
                let result = unsafe {
                    libc::waitid(
                        libc::P_PID,
                        self.pid as libc::id_t,
                        info.as_mut_ptr(),
                        libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
                    )
                };
                if result == 0 {
                    let observed = unsafe { info.assume_init().si_pid() };
                    if observed != 0 && observed != self.pid {
                        self.owned = false;
                        return;
                    }
                    unsafe { libc::kill(-self.pid, libc::SIGKILL) };
                    return;
                }
                if io::Error::last_os_error().raw_os_error() != Some(libc::EINTR) {
                    self.owned = false;
                    return;
                }
            }
        }

        async fn kill_and_reap(&mut self) {
            self.kill_group();
            if self.owned {
                let _ = tokio::time::timeout(REAP_TIMEOUT, self.wait_reap()).await;
            }
        }
    }

    impl Drop for ChildReaper {
        fn drop(&mut self) {
            self.kill_group();
            if self.owned {
                let pid = self.pid;
                self.owned = false;
                // A detached OS waiter outlives cancellation of the caller's
                // Tokio task and cannot recursively create another owner.
                if std::thread::Builder::new()
                    .name("slug-repository-reap".into())
                    .stack_size(64 * 1024)
                    .spawn(move || {
                        reap_blocking(pid);
                    })
                    .is_err()
                {
                    // Under severe memory pressure even a small waiter thread
                    // can fail; retain the only owner until SIGKILL is reaped.
                    reap_blocking(pid);
                }
            }
        }
    }

    fn reap_blocking(pid: libc::pid_t) {
        if let Err(errno) = reap_blocking_with(pid, || {
            let mut status = 0;
            let result = unsafe { libc::waitpid(pid, &mut status, 0) };
            let errno = (result == -1)
                .then(|| io::Error::last_os_error().raw_os_error())
                .flatten();
            (result, errno)
        }) {
            // For a positive owned PID and options=0, only EINTR and ECHILD
            // are expected. Do not strand a waiter on a permanent host error.
            eprintln!("repository launcher reap failed (os error {errno:?})");
        }
    }

    fn reap_blocking_with(
        pid: libc::pid_t,
        mut wait: impl FnMut() -> (libc::pid_t, Option<i32>),
    ) -> Result<(), Option<i32>> {
        loop {
            let (result, errno) = wait();
            if result == pid || errno == Some(libc::ECHILD) {
                return Ok(());
            }
            if errno != Some(libc::EINTR) {
                return Err(errno);
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use std::ffi::OsString;
        use std::os::fd::AsRawFd;
        use std::path::Path;
        use std::sync::Arc;
        use std::time::Duration;

        use slug_bzlmod_v2::GeneratedRepositoryFileEffectPlan;
        use slug_bzlmod_v2::HostRepositoryLabelPathValue;
        use slug_bzlmod_v2::RepositoryEnvironmentEntry;
        use slug_bzlmod_v2::RepositoryEnvironmentSnapshot;
        use slug_workspace_v2::NormalizedAbsolutePath;
        use slug_workspace_v2::PathObservationNamespace;

        use super::ProbeForm;
        use super::RepositoryProcessError;
        use super::match_form;
        use super::nonblocking_pipe;
        use super::pipe_pair;
        use super::reap_blocking_with;
        use super::resolve_closefrom_with;
        use super::run_sandboxed;
        use super::spawn_bwrap;
        use super::validate_environment;
        use super::validate_launcher;
        use super::validate_prefix;
        use super::validate_template_environment;

        fn bwrap() -> &'static Path {
            if Path::new("/usr/bin/bwrap").is_file() {
                Path::new("/usr/bin/bwrap")
            } else {
                Path::new("/home/linuxbrew/.linuxbrew/bin/bwrap")
            }
        }

        fn source_link() -> HostRepositoryLabelPathValue {
            HostRepositoryLabelPathValue::new(
                NormalizedAbsolutePath::new("/usr/share/rules_cc/test.bzl").unwrap(),
                PathObservationNamespace::Host,
            )
            .unwrap()
        }

        fn probe_prefix(source: &[u8], executable: bool) -> GeneratedRepositoryFileEffectPlan {
            let mut builder = GeneratedRepositoryFileEffectPlan::builder();
            for destination in ["cc_toolchain_config.bzl", "armeabi_cc_toolchain_config.bzl"] {
                builder
                    .push_symlink(destination.into(), source_link(), Arc::from(&b"source"[..]))
                    .unwrap();
            }
            builder
                .push("tools/cpp/empty.cc".into(), Arc::from(source), executable)
                .unwrap();
            builder.finish()
        }

        fn template_prefix(files: &[(&str, &[u8], bool)]) -> GeneratedRepositoryFileEffectPlan {
            let mut builder = GeneratedRepositoryFileEffectPlan::builder();
            for destination in ["cc_toolchain_config.bzl", "armeabi_cc_toolchain_config.bzl"] {
                builder
                    .push_symlink(destination.into(), source_link(), Arc::from(&b"source"[..]))
                    .unwrap();
            }
            builder
                .push(
                    "tools/cpp/empty.cc".into(),
                    Arc::from(&b"int main() {}"[..]),
                    true,
                )
                .unwrap();
            for (path, content, executable) in files {
                builder
                    .push((*path).into(), Arc::from(*content), *executable)
                    .unwrap();
            }
            builder.finish()
        }

        #[test]
        fn launcher_rejects_missing_and_workspace_paths() {
            let workspace = tempfile::tempdir().unwrap();
            let missing = workspace.path().join("missing-bwrap");
            assert!(matches!(
                validate_launcher(workspace.path(), &missing),
                Err(RepositoryProcessError::Launcher(_))
            ));
            let inside = workspace.path().join("launcher");
            std::fs::copy(bwrap(), &inside).unwrap();
            assert!(matches!(
                validate_launcher(workspace.path(), &inside),
                Err(RepositoryProcessError::Launcher(_))
            ));
        }

        #[test]
        fn missing_runtime_closefrom_symbol_fails_only_this_runner() {
            assert!(matches!(
                resolve_closefrom_with(|_| std::ptr::null_mut()),
                Err(RepositoryProcessError::Launcher(_))
            ));
        }

        #[test]
        fn blocking_reaper_retries_eintr_until_owned_pid_or_echild() {
            let pid = 4242;
            let mut outcomes = [
                (-1, Some(libc::EINTR)),
                (-1, Some(libc::EINTR)),
                (pid, None),
            ]
            .into_iter();
            assert_eq!(
                reap_blocking_with(pid, || outcomes.next().expect("one result per retry")),
                Ok(())
            );
            assert!(outcomes.next().is_none());
            let mut outcomes = [(-1, Some(libc::EINTR)), (-1, Some(libc::ECHILD))].into_iter();
            assert_eq!(
                reap_blocking_with(pid, || outcomes.next().expect("one result per retry")),
                Ok(())
            );
            assert!(outcomes.next().is_none());
        }

        #[test]
        fn blocking_reaper_stops_on_unexpected_wait_error() {
            let mut calls = 0;
            let result = reap_blocking_with(4242, || {
                calls += 1;
                (-1, Some(libc::EINVAL))
            });
            assert_eq!(result, Err(Some(libc::EINVAL)));
            assert_eq!(calls, 1);
        }

        #[tokio::test]
        async fn bounded_capture_retries_eintr_without_losing_readiness() {
            let (read, write) = pipe_pair().unwrap();
            let stream = nonblocking_pipe(read).unwrap();
            assert_eq!(
                unsafe { libc::write(write.as_raw_fd(), b"ok".as_ptr().cast(), 2) },
                2
            );
            drop(write);
            let mut interrupted = false;
            let bytes = super::capture_bounded_with(stream, |fd, buffer| {
                if !interrupted {
                    interrupted = true;
                    return Err(std::io::Error::from_raw_os_error(libc::EINTR));
                }
                let count = unsafe { libc::read(fd, buffer.as_mut_ptr().cast(), buffer.len()) };
                if count < 0 {
                    Err(std::io::Error::last_os_error())
                } else {
                    Ok(count as usize)
                }
            })
            .await
            .unwrap();
            assert!(interrupted);
            assert_eq!(bytes, b"ok");
        }

        #[test]
        fn guarded_forms_match_only_source_derived_gcc_probes() {
            let source = "@@fixture//tools/cpp/empty.cc";
            assert!(matches!(
                match_form(&["-v"], source),
                Ok(ProbeForm::Version)
            ));
            assert!(matches!(
                match_form(&["--version"], source),
                Ok(ProbeForm::Version)
            ));
            assert!(matches!(
                match_form(
                    &[
                        "-fno-canonical-system-headers",
                        "-o",
                        "/dev/null",
                        "-c",
                        source
                    ],
                    source,
                ),
                Ok(ProbeForm::Compiler)
            ));
            assert!(matches!(
                match_form(
                    &[
                        source,
                        "-o",
                        "/dev/null",
                        "-Wl,--start-lib",
                        "-Wl,--end-lib",
                        "-fuse-ld=gold",
                        "-v",
                    ],
                    source,
                ),
                Ok(ProbeForm::LinkerDiscovery("gold"))
            ));
            assert!(matches!(
                match_form(
                    &[
                        "-fuse-ld=gold",
                        "-B/usr/bin",
                        "-Wl,--gc-sections",
                        "-o",
                        "/dev/null",
                        source,
                    ],
                    source,
                ),
                Ok(ProbeForm::LinkerOption(Some("gold")))
            ));
            assert!(matches!(
                match_form(&["-E", "-xc++", "-", "-v", "-std=c++17"], source),
                Ok(ProbeForm::Include)
            ));
            assert!(matches!(
                match_form(&["-print-resource-dir", "-std=c++17"], source),
                Ok(ProbeForm::Include)
            ));
            for bad in [
                vec!["-E", "-P", "-x", "c", source],
                vec!["-fplugin=/tmp/plugin.so", "-o", "/dev/null", "-c", source],
                vec!["-fno-canonical-system-headers", "-o", "out.o", "-c", source],
                vec![
                    "-fno-canonical-system-headers",
                    "-o",
                    "/dev/null",
                    "-c",
                    "-I@@fixture//tools/cpp/empty.cc",
                ],
                vec!["-B/tmp", "-Wl,--gc-sections", "-o", "/dev/null", source],
            ] {
                assert!(matches!(
                    match_form(&bad, source),
                    Err(RepositoryProcessError::Policy(_))
                ));
            }
        }

        #[test]
        fn compiler_environment_and_generated_source_are_fail_closed() {
            let default = RepositoryEnvironmentSnapshot::empty();
            assert!(validate_environment(&default).is_ok());
            for name in [
                "CC",
                "CPLUS_INCLUDE_PATH",
                "CPATH",
                "GCC_EXEC_PREFIX",
                "LD_PRELOAD",
                "TMPDIR",
            ] {
                let snapshot = RepositoryEnvironmentSnapshot::from_canonical(Arc::from([
                    RepositoryEnvironmentEntry::new(name, "injected"),
                ]))
                .unwrap();
                assert!(matches!(
                    validate_environment(&snapshot),
                    Err(RepositoryProcessError::Policy(_))
                ));
            }
            for (name, value) in [
                ("BAZEL_CONLYOPTS", "-fplugin=/tmp/plugin.so"),
                ("BAZEL_CXXOPTS", "-std=c++20"),
            ] {
                let snapshot = RepositoryEnvironmentSnapshot::from_canonical(Arc::from([
                    RepositoryEnvironmentEntry::new(name, value),
                ]))
                .unwrap();
                assert!(matches!(
                    validate_environment(&snapshot),
                    Err(RepositoryProcessError::Policy(_))
                ));
            }
            let source = probe_prefix(b"int main() {}", true);
            assert!(validate_prefix(&source).is_ok());
            let fixture_source = GeneratedRepositoryFileEffectPlan::build([(
                "tools/cpp/empty.cc".into(),
                Arc::<[u8]>::from(&b"int main() {}"[..]),
                true,
            )])
            .unwrap();
            assert!(validate_prefix(&fixture_source).is_ok());
            let wrong_mode = probe_prefix(b"int main() {}", false);
            assert!(matches!(
                validate_prefix(&wrong_mode),
                Err(RepositoryProcessError::Policy(_))
            ));
            let wrong_bytes = probe_prefix(b"int main() {return 1;}", true);
            assert!(matches!(
                validate_prefix(&wrong_bytes),
                Err(RepositoryProcessError::Policy(_))
            ));
            let mut extra_file = GeneratedRepositoryFileEffectPlan::builder();
            for destination in ["cc_toolchain_config.bzl", "armeabi_cc_toolchain_config.bzl"] {
                extra_file
                    .push_symlink(destination.into(), source_link(), Arc::from(&b"source"[..]))
                    .unwrap();
            }
            extra_file
                .push("OTHER".into(), Arc::from(&b"extra"[..]), false)
                .unwrap();
            extra_file
                .push(
                    "tools/cpp/empty.cc".into(),
                    Arc::from(&b"int main() {}"[..]),
                    true,
                )
                .unwrap();
            assert!(matches!(
                validate_prefix(&extra_file.finish()),
                Err(RepositoryProcessError::Policy(_))
            ));
            let mut wrong_link = GeneratedRepositoryFileEffectPlan::builder();
            wrong_link
                .push_symlink("other.bzl".into(), source_link(), Arc::from(&b"source"[..]))
                .unwrap();
            wrong_link
                .push_symlink(
                    "armeabi_cc_toolchain_config.bzl".into(),
                    source_link(),
                    Arc::from(&b"source"[..]),
                )
                .unwrap();
            wrong_link
                .push(
                    "tools/cpp/empty.cc".into(),
                    Arc::from(&b"int main() {}"[..]),
                    true,
                )
                .unwrap();
            assert!(matches!(
                validate_prefix(&wrong_link.finish()),
                Err(RepositoryProcessError::Policy(_))
            ));
        }

        #[test]
        fn generated_gcc_template_prefix_is_exact_and_bounded() {
            const FILES: [(&str, &[u8], bool); 3] = [
                ("validate_static_library.sh", b"inert one", true),
                ("cc_wrapper.sh", b"inert two", true),
                ("deps_scanner_wrapper.sh", b"inert three", true),
            ];
            assert_eq!(
                validate_prefix(&probe_prefix(b"int main() {}", true)).unwrap(),
                false
            );
            assert_eq!(validate_prefix(&template_prefix(&FILES)).unwrap(), true);
            assert!(matches!(
                validate_prefix(&template_prefix(&FILES[..2])),
                Err(RepositoryProcessError::Policy(_))
            ));
            let mut swapped = FILES;
            swapped.swap(0, 1);
            assert!(matches!(
                validate_prefix(&template_prefix(&swapped)),
                Err(RepositoryProcessError::Policy(_))
            ));
            let mut wrong_mode = FILES;
            wrong_mode[1].2 = false;
            assert!(matches!(
                validate_prefix(&template_prefix(&wrong_mode)),
                Err(RepositoryProcessError::Policy(_))
            ));
            let extra = [
                FILES[0],
                FILES[1],
                FILES[2],
                ("unexpected.sh", &b"extra"[..], true),
            ];
            assert!(matches!(
                validate_prefix(&template_prefix(&extra)),
                Err(RepositoryProcessError::Policy(_))
            ));
            let oversized = vec![b'x'; super::MAX_TEMPLATE_EFFECT_BYTES + 1];
            let one_large = [FILES[0], (FILES[1].0, oversized.as_slice(), true), FILES[2]];
            assert!(matches!(
                validate_prefix(&template_prefix(&one_large)),
                Err(RepositoryProcessError::Policy(_))
            ));
            let maximum = vec![b'x'; super::MAX_TEMPLATE_EFFECT_BYTES];
            let aggregate_large = [
                (FILES[0].0, maximum.as_slice(), true),
                (FILES[1].0, maximum.as_slice(), true),
                (FILES[2].0, &b"x"[..], true),
            ];
            assert!(matches!(
                validate_prefix(&template_prefix(&aggregate_large)),
                Err(RepositoryProcessError::Policy(_))
            ));
            let mut wrong_kind = GeneratedRepositoryFileEffectPlan::builder();
            for destination in ["cc_toolchain_config.bzl", "armeabi_cc_toolchain_config.bzl"] {
                wrong_kind
                    .push_symlink(destination.into(), source_link(), Arc::from(&b"source"[..]))
                    .unwrap();
            }
            wrong_kind
                .push(
                    "tools/cpp/empty.cc".into(),
                    Arc::from(&b"int main() {}"[..]),
                    true,
                )
                .unwrap();
            wrong_kind
                .push_symlink(FILES[0].0.into(), source_link(), Arc::from(&b"source"[..]))
                .unwrap();
            assert!(matches!(
                validate_prefix(&wrong_kind.finish()),
                Err(RepositoryProcessError::Policy(_))
            ));
            let mut archive = GeneratedRepositoryFileEffectPlan::builder();
            archive
                .push_archive(
                    Arc::from(["https://example.invalid/archive.tar.xz".into()]),
                    [0; 32],
                    "archive".into(),
                )
                .unwrap();
            assert!(matches!(
                validate_prefix(&archive.finish()),
                Err(RepositoryProcessError::Policy(_))
            ));
            let homebrew = RepositoryEnvironmentSnapshot::from_canonical(Arc::from([
                RepositoryEnvironmentEntry::new("HOMEBREW_RUBY_PATH", "private-sentinel"),
            ]))
            .unwrap();
            assert!(validate_template_environment(&homebrew, false).is_ok());
            let rejection = validate_template_environment(&homebrew, true).unwrap_err();
            assert!(matches!(rejection, RepositoryProcessError::Policy(_)));
            assert!(!rejection.to_string().contains("private-sentinel"));
        }

        #[tokio::test]
        async fn generated_gcc_templates_remain_inert_for_guarded_compiler_argv() {
            assert!(
                bwrap().is_file(),
                "Linux sandbox prerequisite is unavailable"
            );
            let workspace = tempfile::tempdir_in("/tmp").unwrap();
            let marker = workspace.path().join("template-executed");
            let adversarial = format!(
                "#!/bin/sh\nprintf wrapper-executed >&2\nprintf wrapper-executed > '{}'\n",
                marker.display()
            );
            let files = [
                ("validate_static_library.sh", adversarial.as_bytes(), true),
                ("cc_wrapper.sh", adversarial.as_bytes(), true),
                ("deps_scanner_wrapper.sh", adversarial.as_bytes(), true),
            ];
            let prefix = template_prefix(&files);
            assert_eq!(validate_prefix(&prefix).unwrap(), true);
            let staged =
                super::super::super::repository_io::stage_generated_repository_file_effect_prefix(
                    &prefix,
                    workspace.path(),
                )
                .unwrap();
            let logical_source = "@@selected//tools/cpp/empty.cc";
            assert!(matches!(
                match_form(
                    &[
                        logical_source,
                        "-o",
                        "/dev/null",
                        "-Wl,--start-lib",
                        "-Wl,--end-lib",
                        "-fuse-ld=lld",
                        "-v",
                    ],
                    logical_source,
                ),
                Ok(ProbeForm::LinkerDiscovery("lld"))
            ));
            let argv = [
                OsString::from("/usr/bin/gcc"),
                OsString::from("/exec/tools/cpp/empty.cc"),
                OsString::from("-o"),
                OsString::from("/dev/null"),
                OsString::from("-Wl,--start-lib"),
                OsString::from("-Wl,--end-lib"),
                OsString::from("-fuse-ld=lld"),
                OsString::from("-v"),
            ];
            let (_status, stdout, stderr) =
                run_sandboxed(bwrap(), staged.path(), &argv, Duration::from_secs(10))
                    .await
                    .unwrap();
            assert!(!marker.exists());
            assert!(
                !stdout
                    .windows(b"wrapper-executed".len())
                    .any(|bytes| bytes == b"wrapper-executed")
            );
            assert!(
                !stderr
                    .windows(b"wrapper-executed".len())
                    .any(|bytes| bytes == b"wrapper-executed")
            );
        }

        #[tokio::test]
        async fn sandbox_has_private_host_view_and_read_only_repository() {
            assert!(
                bwrap().is_file(),
                "Linux sandbox prerequisite is unavailable"
            );
            let prefix = tempfile::tempdir().unwrap();
            let host_marker = prefix
                .path()
                .parent()
                .unwrap()
                .join(format!("slug-wp839-host-marker-{}", std::process::id()));
            let command = vec![
                "/bin/sh".into(),
                "-c".into(),
                format!(
                    "test ! -e /home && test ! -e /run && test ! -e /etc || exit 20; touch /exec/denied 2>/dev/null || true; test ! -e /exec/denied || exit 21; touch '{}'",
                    host_marker.display()
                )
                .into(),
            ];
            let result = run_sandboxed(bwrap(), prefix.path(), &command, Duration::from_secs(3))
                .await
                .unwrap();
            assert_eq!(result.0, 0);
            assert!(!prefix.path().join("denied").exists());
            assert!(!host_marker.exists());
        }

        #[tokio::test]
        async fn sandbox_rejects_create_then_delete_without_host_write() {
            assert!(
                bwrap().is_file(),
                "Linux sandbox prerequisite is unavailable"
            );
            let prefix = tempfile::tempdir_in("/tmp").unwrap();
            let staged_target = prefix.path().join("create-delete-probe");
            let host_marker = prefix.path().with_extension("host-write-marker");
            assert!(!staged_target.exists() && !host_marker.exists());
            let command = vec![
                "/bin/sh".into(),
                "-c".into(),
                format!(
                    "touch /exec/create-delete-probe 2>/dev/null; create=$?; rm /exec/create-delete-probe 2>/dev/null; delete=$?; touch '{}' || exit 30; test -e '{}' || exit 31; printf '%s %s\\n' \"$create\" \"$delete\"",
                    host_marker.display(),
                    host_marker.display(),
                )
                .into(),
            ];
            let (code, stdout, _) =
                run_sandboxed(bwrap(), prefix.path(), &command, Duration::from_secs(3))
                    .await
                    .unwrap();
            assert_eq!(code, 0);
            let statuses = String::from_utf8(stdout).unwrap();
            let mut statuses = statuses.split_whitespace().map(str::parse::<i32>);
            assert!(
                statuses.next().unwrap().unwrap() != 0,
                "staged create succeeded"
            );
            assert!(
                statuses.next().unwrap().unwrap() != 0,
                "staged delete succeeded"
            );
            assert!(statuses.next().is_none());
            assert!(!staged_target.exists());
            assert!(!host_marker.exists());
        }

        #[tokio::test]
        async fn sandbox_caps_output_and_timeout() {
            assert!(
                bwrap().is_file(),
                "Linux sandbox prerequisite is unavailable"
            );
            let prefix = tempfile::tempdir().unwrap();
            let flood = vec!["/bin/sh".into(), "-c".into(), "/usr/bin/yes flood".into()];
            let result =
                run_sandboxed(bwrap(), prefix.path(), &flood, Duration::from_secs(3)).await;
            assert!(matches!(result, Err(RepositoryProcessError::OutputLimit)));
            let sleeper = vec!["/bin/sh".into(), "-c".into(), "sleep 5".into()];
            let result =
                run_sandboxed(bwrap(), prefix.path(), &sleeper, Duration::from_millis(30)).await;
            assert!(matches!(result, Err(RepositoryProcessError::Timeout)));
        }

        #[tokio::test]
        async fn sandbox_distinguishes_child_failure_from_setup_failure() {
            assert!(
                bwrap().is_file(),
                "Linux sandbox prerequisite is unavailable"
            );
            let prefix = tempfile::tempdir().unwrap();
            let false_command = vec!["/usr/bin/false".into()];
            let (code, stdout, stderr) = run_sandboxed(
                bwrap(),
                prefix.path(),
                &false_command,
                Duration::from_secs(3),
            )
            .await
            .unwrap();
            assert_eq!(code, 1);
            assert!(stdout.is_empty() && stderr.is_empty());
            let missing_prefix = prefix.path().join("missing");
            let result = run_sandboxed(
                bwrap(),
                &missing_prefix,
                &false_command,
                Duration::from_secs(3),
            )
            .await;
            assert!(matches!(result, Err(RepositoryProcessError::Sandbox(_))));
            let denied_launcher = prefix.path().join("not-executable");
            std::fs::write(&denied_launcher, b"not an executable").unwrap();
            let result = run_sandboxed(
                &denied_launcher,
                prefix.path(),
                &false_command,
                Duration::from_secs(3),
            )
            .await;
            assert!(matches!(
                result,
                Err(RepositoryProcessError::LauncherSpawn(Some(libc::EACCES)))
            ));
        }

        #[test]
        fn sandbox_status_file_ignores_ambient_tmpdir() {
            assert!(
                bwrap().is_file(),
                "Linux sandbox prerequisite is unavailable"
            );
            if std::env::var_os("SLUG_WP839_TMPDIR_CHILD").is_some() {
                let status_file = super::fixed_status_file().unwrap();
                let target =
                    std::fs::read_link(format!("/proc/self/fd/{}", status_file.as_raw_fd()))
                        .unwrap();
                assert!(target.starts_with("/tmp"));
                assert!(!target.starts_with(std::path::PathBuf::from(
                    std::env::var_os("TMPDIR").unwrap()
                )));
                let prefix = tempfile::tempdir_in("/tmp").unwrap();
                let command = vec!["/usr/bin/true".into()];
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap();
                let (code, _, _) = runtime
                    .block_on(run_sandboxed(
                        bwrap(),
                        prefix.path(),
                        &command,
                        Duration::from_secs(3),
                    ))
                    .unwrap();
                assert_eq!(code, 0);
                return;
            }
            let workspace = tempfile::tempdir_in("/tmp").unwrap();
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .arg("sandbox_status_file_ignores_ambient_tmpdir")
                .arg("--nocapture")
                .env("TMPDIR", workspace.path())
                .env("SLUG_WP839_TMPDIR_CHILD", "1")
                .output()
                .unwrap();
            assert!(
                output.status.success()
                    && String::from_utf8_lossy(&output.stdout).contains("1 passed"),
                "isolated status-file test failed"
            );
            assert_eq!(std::fs::read_dir(workspace.path()).unwrap().count(), 0);
        }

        #[tokio::test]
        async fn sandbox_child_environment_is_fixed_and_network_is_unshared() {
            assert!(
                bwrap().is_file(),
                "Linux sandbox prerequisite is unavailable"
            );
            let prefix = tempfile::tempdir_in("/tmp").unwrap();
            let (code, stdout, _) = run_sandboxed(
                bwrap(),
                prefix.path(),
                &["/usr/bin/env".into()],
                Duration::from_secs(3),
            )
            .await
            .unwrap();
            assert_eq!(code, 0);
            let environment = String::from_utf8(stdout).unwrap();
            let mut entries: Vec<_> = environment.lines().collect();
            entries.sort_unstable();
            assert_eq!(
                entries,
                ["LC_ALL=C", "PATH=/usr/bin:/bin", "PWD=/exec", "TMPDIR=/tmp"]
            );

            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let port = listener.local_addr().unwrap().port();
            let command = vec![
                "/bin/bash".into(),
                "-c".into(),
                format!("echo probe >/dev/tcp/127.0.0.1/{port}").into(),
            ];
            let (code, _, _) =
                run_sandboxed(bwrap(), prefix.path(), &command, Duration::from_secs(3))
                    .await
                    .unwrap();
            assert_ne!(code, 0);
        }

        #[test]
        fn sandbox_closes_inherited_host_descriptors_and_status_fd() {
            assert!(
                bwrap().is_file(),
                "Linux sandbox prerequisite is unavailable"
            );
            if std::env::var_os("SLUG_WP839_FD_CHILD").is_some() {
                let prefix = tempfile::tempdir_in("/tmp").unwrap();
                let host_file = prefix.path().join("host-fd");
                let file = std::fs::OpenOptions::new()
                    .create_new(true)
                    .read(true)
                    .write(true)
                    .open(host_file)
                    .unwrap();
                unsafe {
                    assert_eq!(libc::dup2(file.as_raw_fd(), 9), 9);
                    let flags = libc::fcntl(9, libc::F_GETFD);
                    assert!(flags >= 0);
                    assert_eq!(libc::fcntl(9, libc::F_SETFD, flags & !libc::FD_CLOEXEC), 0);
                }
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap();
                let command = vec![
                    "/bin/sh".into(),
                    "-c".into(),
                    "test ! -e /proc/self/fd/3 && test ! -e /proc/self/fd/9".into(),
                ];
                let (code, _, _) = runtime
                    .block_on(run_sandboxed(
                        bwrap(),
                        prefix.path(),
                        &command,
                        Duration::from_secs(3),
                    ))
                    .unwrap();
                assert_eq!(code, 0, "payload retained a host or status descriptor");
                return;
            }
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .arg("sandbox_closes_inherited_host_descriptors_and_status_fd")
                .arg("--nocapture")
                .env("SLUG_WP839_FD_CHILD", "1")
                .output()
                .unwrap();
            assert!(
                output.status.success()
                    && String::from_utf8_lossy(&output.stdout).contains("1 passed"),
                "isolated inherited-FD test failed"
            );
        }

        #[test]
        fn sandbox_reassigns_closed_parent_stdio_and_fd3_without_leak() {
            assert!(
                bwrap().is_file(),
                "Linux sandbox prerequisite is unavailable"
            );
            if let Some(marker) = std::env::var_os("SLUG_WP839_STDIO_CHILD_MARKER") {
                let prefix = tempfile::tempdir_in("/tmp").unwrap();
                let reserved = std::fs::File::open("/dev/null").unwrap();
                unsafe { assert_eq!(libc::dup2(reserved.as_raw_fd(), 3), 3) };
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap();
                for descriptor in 0..=3 {
                    unsafe { libc::close(descriptor) };
                }
                let command = vec![
                    "/bin/sh".into(),
                    "-c".into(),
                    "printf out; printf err >&2; test ! -e /proc/self/fd/3".into(),
                ];
                let (code, stdout, stderr) = runtime
                    .block_on(run_sandboxed(
                        bwrap(),
                        prefix.path(),
                        &command,
                        Duration::from_secs(3),
                    ))
                    .unwrap();
                assert_eq!(
                    (code, stdout.as_slice(), stderr.as_slice()),
                    (0, b"out".as_slice(), b"err".as_slice())
                );
                let host_fd = std::fs::OpenOptions::new()
                    .create_new(true)
                    .read(true)
                    .write(true)
                    .open(prefix.path().join("fd3-host"))
                    .unwrap();
                unsafe {
                    assert_eq!(libc::dup2(host_fd.as_raw_fd(), 3), 3);
                    let flags = libc::fcntl(3, libc::F_GETFD);
                    assert!(flags >= 0);
                    assert_eq!(libc::fcntl(3, libc::F_SETFD, flags & !libc::FD_CLOEXEC), 0);
                }
                let (code, stdout, stderr) = runtime
                    .block_on(run_sandboxed(
                        bwrap(),
                        prefix.path(),
                        &command,
                        Duration::from_secs(3),
                    ))
                    .unwrap();
                assert_eq!(
                    (code, stdout.as_slice(), stderr.as_slice()),
                    (0, b"out".as_slice(), b"err".as_slice())
                );
                assert_eq!(
                    std::fs::metadata(prefix.path().join("fd3-host"))
                        .unwrap()
                        .len(),
                    0
                );
                std::fs::write(marker, b"ran").unwrap();
                return;
            }
            let parent = tempfile::tempdir_in("/tmp").unwrap();
            let marker = parent.path().join("ran");
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .arg("sandbox_reassigns_closed_parent_stdio_and_fd3_without_leak")
                .env("SLUG_WP839_STDIO_CHILD_MARKER", &marker)
                .output()
                .unwrap();
            assert!(output.status.success(), "isolated closed-stdio test failed");
            assert_eq!(std::fs::read(marker).unwrap(), b"ran");
        }

        #[tokio::test]
        async fn dropped_runner_child_is_killed_and_reaped() {
            assert!(
                bwrap().is_file(),
                "Linux sandbox prerequisite is unavailable"
            );
            let prefix = tempfile::tempdir_in("/tmp").unwrap();
            let status = super::fixed_status_file().unwrap();
            let (reaper, stdout, stderr) = spawn_bwrap(
                bwrap(),
                prefix.path(),
                &["/bin/sh".into(), "-c".into(), "sleep 30".into()],
                &status,
            )
            .unwrap();
            let pid = reaper.pid;
            drop(reaper);
            drop(stdout);
            drop(stderr);
            let gone = tokio::time::timeout(Duration::from_secs(3), async {
                loop {
                    if unsafe { libc::kill(pid, 0) } == -1 {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await;
            assert!(gone.is_ok(), "dropped child was not reaped");
        }

        #[tokio::test]
        async fn cancelled_sandbox_future_kills_namespace_tree() {
            assert!(
                bwrap().is_file(),
                "Linux sandbox prerequisite is unavailable"
            );
            let prefix = tempfile::tempdir_in("/tmp").unwrap();
            let wrapper = prefix.path().join("bwrap-wrapper");
            let pid_file = prefix.path().join("launcher.pid");
            std::fs::write(
                &wrapper,
                format!(
                    "#!/bin/sh\necho $$ > {}\nexec {} \"$@\"\n",
                    pid_file.display(),
                    bwrap().display()
                ),
            )
            .unwrap();
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o700)).unwrap();
            let argv = vec!["/bin/sh".into(), "-c".into(), "sleep 30".into()];
            let root = prefix.path().to_owned();
            let task = tokio::spawn(async move {
                run_sandboxed(&wrapper, &root, &argv, Duration::from_secs(30)).await
            });
            let launcher_pid = tokio::time::timeout(Duration::from_secs(3), async {
                loop {
                    if let Ok(value) = std::fs::read_to_string(&pid_file)
                        && let Ok(pid) = value.trim().parse::<libc::pid_t>()
                    {
                        break pid;
                    }
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await
            .expect("sandbox launcher did not start");
            let child_pids = tokio::time::timeout(Duration::from_secs(3), async {
                loop {
                    let children_path =
                        format!("/proc/{launcher_pid}/task/{launcher_pid}/children");
                    if let Ok(value) = std::fs::read_to_string(children_path) {
                        let pids: Vec<libc::pid_t> = value
                            .split_whitespace()
                            .filter_map(|value| value.parse().ok())
                            .collect();
                        if !pids.is_empty() {
                            break pids;
                        }
                    }
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await
            .expect("sandbox namespace child did not start");
            task.abort();
            assert!(task.await.unwrap_err().is_cancelled());
            let gone = tokio::time::timeout(Duration::from_secs(3), async {
                loop {
                    let launcher_reaped = unsafe { libc::kill(launcher_pid, 0) } == -1;
                    let descendants_stopped = child_pids.iter().all(|pid| {
                        let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else {
                            return true;
                        };
                        stat.rsplit_once(") ").is_some_and(|(_, fields)| {
                            matches!(fields.chars().next(), Some('Z' | 'X'))
                        })
                    });
                    if launcher_reaped && descendants_stopped {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await;
            assert!(gone.is_ok(), "cancelled sandbox tree remained live");
        }

        #[tokio::test]
        async fn exited_launcher_remains_unreaped_while_descendant_holds_output_pipe() {
            assert!(
                bwrap().is_file(),
                "Linux sandbox prerequisite is unavailable"
            );
            let prefix = tempfile::tempdir_in("/tmp").unwrap();
            let wrapper = prefix.path().join("bwrap-wrapper");
            let launcher_pid_file = prefix.path().join("launcher.pid");
            let holder_pid_file = prefix.path().join("holder.pid");
            std::fs::write(
                &wrapper,
                format!(
                    "#!/bin/sh\n/usr/bin/sleep 30 &\necho $! > {}\necho $$ > {}\nexec {} \"$@\"\n",
                    holder_pid_file.display(),
                    launcher_pid_file.display(),
                    bwrap().display(),
                ),
            )
            .unwrap();
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o700)).unwrap();
            let root = prefix.path().to_owned();
            let task = tokio::spawn(async move {
                run_sandboxed(
                    &wrapper,
                    &root,
                    &["/usr/bin/true".into()],
                    Duration::from_secs(30),
                )
                .await
            });
            let observed = tokio::time::timeout(Duration::from_secs(3), async {
                loop {
                    if let (Ok(launcher), Ok(holder)) = (
                        std::fs::read_to_string(&launcher_pid_file),
                        std::fs::read_to_string(&holder_pid_file),
                    ) && let (Ok(launcher), Ok(holder)) = (
                        launcher.trim().parse::<libc::pid_t>(),
                        holder.trim().parse::<libc::pid_t>(),
                    ) && let Ok(stat) = std::fs::read_to_string(format!("/proc/{launcher}/stat"))
                        && stat
                            .rsplit_once(") ")
                            .is_some_and(|(_, fields)| fields.starts_with('Z'))
                        && unsafe { libc::kill(holder, 0) } == 0
                    {
                        break (launcher, holder);
                    }
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await;
            task.abort();
            assert!(task.await.unwrap_err().is_cancelled());
            let (launcher, holder) = observed
                .expect("launcher was reaped before a descendant holding its output pipe exited");
            let stopped = tokio::time::timeout(Duration::from_secs(3), async {
                loop {
                    let launcher_gone = unsafe { libc::kill(launcher, 0) } == -1;
                    let holder_stopped = std::fs::read_to_string(format!("/proc/{holder}/stat"))
                        .map_or(true, |stat| {
                            stat.rsplit_once(") ").is_some_and(|(_, fields)| {
                                matches!(fields.chars().next(), Some('Z' | 'X'))
                            })
                        });
                    if launcher_gone && holder_stopped {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await;
            assert!(stopped.is_ok(), "cancelled output holder remained live");
        }
    }
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
pub(super) use linux::run_repository_execute;
