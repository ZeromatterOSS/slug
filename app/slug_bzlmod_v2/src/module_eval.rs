use std::cell::RefCell;
use std::collections::VecDeque;
use std::fmt;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use allocative::Allocative;
use anyhow::Context;
use async_trait::async_trait;
use compact_str::CompactString;
use dice::DiceComputations;
use dice::DiceTransactionUpdater;
use dice::InjectedKey;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalRepoName;
use slug_identity_v2::RepositoryMapping;
use slug_identity_v2::RepositoryMappingId;
use slug_workspace_v2::WorkspaceFileKey;
use slug_workspace_v2::WorkspaceFileValue;
use starlark::any::ProvidesStaticType;
use starlark::environment::GlobalsBuilder;
use starlark::environment::Module;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;
use starlark::values::list::UnpackList;
use starlark::values::none::NoneOr;
use starlark::values::none::NoneType;
use starlark_map::small_set::SmallSet;

use crate::BzlmodCommandPolicyKey;
use crate::BzlmodEnvironmentPolicyKey;
use crate::LockfileMode;
use crate::VisibleLockfileRead;
use crate::lockfile::bad_visible_lockfile_message;
use crate::lockfile::parse_visible_lockfile_content_for_mode;

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct RootModuleHeader {
    pub name: CompactString,
    pub version: Option<CompactString>,
    pub repo_name: Option<CompactString>,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct RootModuleDependency {
    pub name: CompactString,
    pub version: CompactString,
    pub repo_name: Option<CompactString>,
    pub nodep: bool,
    pub dev_dependency: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct RootModuleLocalPathOverride {
    pub module_name: CompactString,
    pub path: CompactString,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct RootModuleCommandPolicy {
    yanked_versions_policy: Arc<str>,
    ignore_dev_dependency: bool,
}

impl From<BzlmodCommandPolicyKey> for RootModuleCommandPolicy {
    fn from(policy: BzlmodCommandPolicyKey) -> Self {
        Self {
            yanked_versions_policy: Arc::from(policy.stable_serialize()),
            ignore_dev_dependency: policy.ignore_dev_dependency(),
        }
    }
}

impl RootModuleCommandPolicy {
    pub fn ignore_dev_dependency(&self) -> bool {
        self.ignore_dev_dependency
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct RootModuleEnvironmentPolicy {
    yanked_versions_policy: Arc<str>,
}

impl From<BzlmodEnvironmentPolicyKey> for RootModuleEnvironmentPolicy {
    fn from(policy: BzlmodEnvironmentPolicyKey) -> Self {
        Self {
            yanked_versions_policy: Arc::from(policy.stable_serialize()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct RootModuleLockfileMode(Arc<str>);

impl From<LockfileMode> for RootModuleLockfileMode {
    fn from(mode: LockfileMode) -> Self {
        Self(Arc::from(mode.as_str()))
    }
}

impl RootModuleLockfileMode {
    fn semantic_mode(&self) -> LockfileMode {
        match self.0.as_ref() {
            "off" => LockfileMode::Off,
            "update" => LockfileMode::Update,
            "refresh" => LockfileMode::Refresh,
            "error" => LockfileMode::Error,
            mode => unreachable!("injected lockfile mode was not normalized: {mode}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct ModuleFileEvaluation {
    pub path: PathBuf,
    pub header: Option<RootModuleHeader>,
    pub includes: Arc<[CompactString]>,
    pub dependencies: Arc<[RootModuleDependency]>,
    pub local_path_overrides: Arc<[RootModuleLocalPathOverride]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct RootModuleGraph {
    pub root: ModuleFileEvaluation,
    pub includes: Arc<[ModuleFileEvaluation]>,
    pub visible_lockfile: VisibleLockfileRead,
    pub repository_mapping: RepositoryMapping,
    pub command_policy: RootModuleCommandPolicy,
    pub environment_policy: RootModuleEnvironmentPolicy,
    pub lockfile_mode: RootModuleLockfileMode,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct RootModuleCommandPolicyKey {
    pub workspace: PathBuf,
}

impl fmt::Display for RootModuleCommandPolicyKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "root-module-command-policy:{}", self.workspace.display())
    }
}

impl InjectedKey for RootModuleCommandPolicyKey {
    type Value = RootModuleCommandPolicy;
    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct RootModuleEnvironmentPolicyKey {
    pub workspace: PathBuf,
}
impl fmt::Display for RootModuleEnvironmentPolicyKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "root-module-environment-policy:{}",
            self.workspace.display()
        )
    }
}
impl InjectedKey for RootModuleEnvironmentPolicyKey {
    type Value = RootModuleEnvironmentPolicy;
    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct RootModuleLockfileModeKey {
    pub workspace: PathBuf,
}
impl fmt::Display for RootModuleLockfileModeKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "root-module-lockfile-mode:{}", self.workspace.display())
    }
}
impl InjectedKey for RootModuleLockfileModeKey {
    type Value = RootModuleLockfileMode;
    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct VisibleLockfileKey {
    pub workspace: PathBuf,
}

impl fmt::Display for VisibleLockfileKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "visible-lockfile:{}", self.workspace.display())
    }
}

#[async_trait]
impl Key for VisibleLockfileKey {
    type Value = Arc<Result<VisibleLockfileRead, CompactString>>;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let mode = match ctx
            .compute(&RootModuleLockfileModeKey {
                workspace: self.workspace.clone(),
            })
            .await
        {
            Ok(mode) => mode.semantic_mode(),
            Err(error) => {
                return Arc::new(Err(CompactString::new(format!(
                    "missing injected root module lockfile mode: {error}"
                ))));
            }
        };
        if matches!(mode, LockfileMode::Off) {
            return Arc::new(Ok(VisibleLockfileRead::Ignored));
        }

        let value = match ctx
            .compute(&WorkspaceFileKey {
                workspace: self.workspace.clone(),
                path: self.workspace.join("MODULE.bazel.lock"),
            })
            .await
        {
            Ok(value) => value,
            Err(error) => return Arc::new(Err(CompactString::new(error.to_string()))),
        };
        let parsed = match value {
            WorkspaceFileValue::Present(source) => {
                parse_visible_lockfile_content_for_mode(&mode, Some(source.as_str()))
            }
            WorkspaceFileValue::Absent => parse_visible_lockfile_content_for_mode(&mode, None),
            WorkspaceFileValue::ReadError(error) => {
                Err(bad_visible_lockfile_message(error.as_str()))
            }
        };
        Arc::new(parsed.map_err(CompactString::new))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

/// Inject one normalized bzlmod request into a caller-owned DICE updater.
///
/// This helper deliberately supplies no defaults and does not commit. The
/// runtime owns the request transaction and must install all three values
/// before its sole commit.
pub fn inject_root_module_request_inputs(
    updater: &mut DiceTransactionUpdater,
    workspace: &Path,
    command_policy: BzlmodCommandPolicyKey,
    environment_policy: BzlmodEnvironmentPolicyKey,
    lockfile_mode: LockfileMode,
) -> anyhow::Result<()> {
    updater.changed_to(vec![(
        RootModuleCommandPolicyKey {
            workspace: workspace.to_path_buf(),
        },
        RootModuleCommandPolicy::from(command_policy),
    )])?;
    updater.changed_to(vec![(
        RootModuleEnvironmentPolicyKey {
            workspace: workspace.to_path_buf(),
        },
        RootModuleEnvironmentPolicy::from(environment_policy),
    )])?;
    updater.changed_to(vec![(
        RootModuleLockfileModeKey {
            workspace: workspace.to_path_buf(),
        },
        RootModuleLockfileMode::from(lockfile_mode),
    )])?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct ModuleFileEvaluationKey {
    pub workspace: PathBuf,
    pub path: PathBuf,
}
impl fmt::Display for ModuleFileEvaluationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "module-file-evaluation:{}", self.path.display())
    }
}

#[async_trait]
impl Key for ModuleFileEvaluationKey {
    type Value = Arc<Result<ModuleFileEvaluation, CompactString>>;
    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let value = match ctx
            .compute(&WorkspaceFileKey {
                workspace: self.workspace.clone(),
                path: self.path.clone(),
            })
            .await
        {
            Ok(value) => value,
            Err(error) => return Arc::new(Err(CompactString::new(error.to_string()))),
        };
        Arc::new(match value {
            WorkspaceFileValue::Present(source) => evaluate_module_file(&self.path, &source),
            WorkspaceFileValue::Absent => Err(CompactString::new(format!(
                "workspace file is absent: {}",
                self.path.display()
            ))),
            WorkspaceFileValue::ReadError(error) => Err(CompactString::new(error.as_str())),
        })
    }
    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct RootModuleGraphKey {
    pub workspace: PathBuf,
}
impl fmt::Display for RootModuleGraphKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "root-module-graph:{}", self.workspace.display())
    }
}

#[async_trait]
impl Key for RootModuleGraphKey {
    type Value = Arc<Result<RootModuleGraph, CompactString>>;
    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let command_policy = match ctx
            .compute(&RootModuleCommandPolicyKey {
                workspace: self.workspace.clone(),
            })
            .await
        {
            Ok(policy) => policy,
            Err(error) => {
                return Arc::new(Err(CompactString::new(format!(
                    "missing injected root module command policy: {error}"
                ))));
            }
        };
        let environment_policy = match ctx
            .compute(&RootModuleEnvironmentPolicyKey {
                workspace: self.workspace.clone(),
            })
            .await
        {
            Ok(policy) => policy,
            Err(error) => {
                return Arc::new(Err(CompactString::new(format!(
                    "missing injected root module environment policy: {error}"
                ))));
            }
        };
        let lockfile_mode = match ctx
            .compute(&RootModuleLockfileModeKey {
                workspace: self.workspace.clone(),
            })
            .await
        {
            Ok(mode) => mode,
            Err(error) => {
                return Arc::new(Err(CompactString::new(format!(
                    "missing injected root module lockfile mode: {error}"
                ))));
            }
        };
        let root_path = self.workspace.join("MODULE.bazel");
        let root = match ctx
            .compute(&ModuleFileEvaluationKey {
                workspace: self.workspace.clone(),
                path: root_path,
            })
            .await
        {
            Ok(value) => match value.as_ref().clone() {
                Ok(value) => value,
                Err(error) => return Arc::new(Err(error)),
            },
            Err(error) => return Arc::new(Err(CompactString::new(error.to_string()))),
        };
        let mut seen = SmallSet::new();
        let mut horizon = VecDeque::from(root.includes.iter().cloned().collect::<Vec<_>>());
        let mut includes = Vec::new();
        while let Some(label) = horizon.pop_front() {
            if !seen.insert(label.clone()) {
                continue;
            }
            let path = match include_path(&self.workspace, label.as_str()) {
                Ok(path) => path,
                Err(error) => return Arc::new(Err(error)),
            };
            let value = match ctx
                .compute(&ModuleFileEvaluationKey {
                    workspace: self.workspace.clone(),
                    path,
                })
                .await
            {
                Ok(value) => match value.as_ref().clone() {
                    Ok(value) => value,
                    Err(error) => return Arc::new(Err(error)),
                },
                Err(error) => return Arc::new(Err(CompactString::new(error.to_string()))),
            };
            if value.header.is_some() {
                return Arc::new(Err(CompactString::new(
                    "if module() is called, it must be called before any other functions",
                )));
            }
            horizon.extend(value.includes.iter().cloned());
            includes.push(value);
        }
        let visible_lockfile = match ctx
            .compute(&VisibleLockfileKey {
                workspace: self.workspace.clone(),
            })
            .await
        {
            Ok(value) => match value.as_ref().clone() {
                Ok(value) => value,
                Err(error) => return Arc::new(Err(error)),
            },
            Err(error) => return Arc::new(Err(CompactString::new(error.to_string()))),
        };
        Arc::new(Ok(RootModuleGraph {
            repository_mapping: match root_mapping(&root, &includes, &command_policy) {
                Ok(mapping) => mapping,
                Err(error) => return Arc::new(Err(error)),
            },
            root,
            includes: includes.into(),
            visible_lockfile,
            command_policy,
            environment_policy,
            lockfile_mode,
        }))
    }
    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

fn include_path(workspace: &Path, label: &str) -> Result<PathBuf, CompactString> {
    let Some(repo_relative) = label.strip_prefix("//") else {
        return Err(CompactString::new(format!(
            "bad include label '{label}': include() must be called with repo-relative labels (starting with double slashes)"
        )));
    };
    let (package, target) = match repo_relative.split_once(':') {
        Some((package, "")) => (package, package.rsplit('/').next().unwrap_or_default()),
        Some((package, target)) => (package, target),
        None => (
            repo_relative,
            repo_relative.rsplit('/').next().unwrap_or_default(),
        ),
    };
    if package.contains(':')
        || target.contains(':')
        || repo_relative.contains('\\')
        || repo_relative.chars().any(char::is_control)
        || !(package.is_empty() || package.split('/').all(valid_package_segment))
        || !target.split('/').all(valid_target_segment)
        || !target.ends_with(".MODULE.bazel")
        || target
            .rsplit('/')
            .next()
            .is_some_and(|name| name.starts_with('.'))
    {
        return Err(CompactString::new(format!("bad include label '{label}'")));
    }
    Ok(if package.is_empty() {
        workspace.join(target)
    } else {
        workspace.join(package).join(target)
    })
}

fn valid_package_segment(segment: &str) -> bool {
    !segment.is_empty()
        && segment.bytes().all(|byte| {
            byte.is_ascii()
                && !byte.is_ascii_control()
                && byte != b'\x7f'
                && !matches!(byte, b':' | b'\\')
        })
        && !segment.bytes().all(|byte| byte == b'.')
}

fn valid_target_segment(segment: &str) -> bool {
    !matches!(segment, "" | "." | "..")
}

fn validate_module_name(name: &str) -> anyhow::Result<()> {
    let valid = name.bytes().enumerate().all(|(index, byte)| match byte {
        b'a'..=b'z' => true,
        b'0'..=b'9' => index != 0,
        b'.' | b'_' | b'-' => index != 0 && index + 1 != name.len(),
        _ => false,
    });
    if !valid {
        anyhow::bail!("invalid module name '{name}'");
    }
    Ok(())
}

fn validate_repo_name(name: &str) -> anyhow::Result<()> {
    if name.is_empty() {
        return Ok(());
    }
    let valid = name.bytes().enumerate().all(|(index, byte)| match byte {
        b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' => true,
        b'-' | b'.' | b'_' => index != 0,
        _ => false,
    });
    if !valid {
        anyhow::bail!("invalid repository name '{name}'");
    }
    Ok(())
}

fn validate_version(version: &str, directive: &str) -> anyhow::Result<()> {
    if !is_valid_version(version) {
        anyhow::bail!("Invalid version in {directive}()");
    }
    Ok(())
}

fn is_valid_version(version: &str) -> bool {
    if version.is_empty() {
        return true;
    }
    let mut build_split = version.split('+');
    let core = build_split.next().unwrap_or_default();
    let build = build_split.next();
    if build_split.next().is_some()
        || build.is_some_and(|build| build.is_empty() || !valid_version_chars(build, true))
    {
        return false;
    }
    let (release, prerelease) = match core.split_once('-') {
        Some((release, prerelease)) => (release, Some(prerelease)),
        None => (core, None),
    };
    valid_version_identifiers(release, false)
        && prerelease.is_none_or(|value| valid_version_identifiers(value, true))
}

fn valid_version_chars(value: &str, allow_hyphen: bool) -> bool {
    value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'.' || (allow_hyphen && byte == b'-'))
}

fn valid_version_identifiers(value: &str, allow_hyphen: bool) -> bool {
    !value.is_empty()
        && valid_version_chars(value, allow_hyphen)
        && value.split('.').all(|identifier| {
            !identifier.is_empty()
                && (!identifier.bytes().all(|byte| byte.is_ascii_digit())
                    || identifier.parse::<u64>().is_ok())
        })
}

fn validate_bazel_compatibility(value: &str) -> anyhow::Result<()> {
    let (operator, version) = if let Some(version) = value
        .strip_prefix("<=")
        .or_else(|| value.strip_prefix(">="))
    {
        (true, version)
    } else if let Some(version) = value.strip_prefix(['<', '>', '-']) {
        (true, version)
    } else {
        (false, "")
    };
    if !operator
        || version.split('.').count() != 3
        || version
            .split('.')
            .any(|segment| segment.is_empty() || !segment.bytes().all(|byte| byte.is_ascii_digit()))
    {
        anyhow::bail!("invalid bazel_compatibility value '{value}'");
    }
    Ok(())
}

fn root_mapping(
    root: &ModuleFileEvaluation,
    includes: &[ModuleFileEvaluation],
    policy: &RootModuleCommandPolicy,
) -> Result<RepositoryMapping, CompactString> {
    let mut mapping = RepositoryMapping::new(
        RepositoryMappingId::new("root-module").map_err(CompactString::new)?,
    );
    for dep in std::iter::once(root)
        .chain(includes.iter())
        .flat_map(|module| module.dependencies.iter())
        .filter(|dep| !dep.dev_dependency || !policy.ignore_dev_dependency())
        .filter(|dep| !dep.nodep)
    {
        let apparent = dep.repo_name.as_deref().unwrap_or(dep.name.as_str());
        mapping.insert(
            ApparentRepoName::new(apparent).map_err(CompactString::new)?,
            CanonicalRepoName::new(format!("{}+", dep.name)).map_err(CompactString::new)?,
        );
    }
    Ok(mapping)
}

#[derive(Default, ProvidesStaticType)]
struct Recorder {
    state: RefCell<RecordedModuleFile>,
}

#[derive(Default)]
struct RecordedModuleFile {
    header: Option<RootModuleHeader>,
    non_module_called: bool,
    includes: Vec<CompactString>,
    dependencies: Vec<RootModuleDependency>,
    overrides: Vec<RootModuleLocalPathOverride>,
}

fn recorder<'a>(eval: &'a Evaluator<'_, '_, '_>) -> anyhow::Result<&'a Recorder> {
    eval.extra
        .and_then(|value| value.downcast_ref())
        .context("MODULE.bazel global invoked without module recorder")
}

#[starlark_module]
fn module_globals(builder: &mut GlobalsBuilder) {
    fn module(
        #[starlark(require = named, default = "")] name: &str,
        #[starlark(require = named, default = "")] version: &str,
        #[starlark(require = named, default = -1)] compatibility_level: i32,
        #[starlark(require = named, default = "")] repo_name: &str,
        #[starlark(require = named, default = UnpackList::default())]
        bazel_compatibility: UnpackList<&str>,
        eval: &mut Evaluator,
    ) -> anyhow::Result<NoneType> {
        let mut state = recorder(eval)?.state.borrow_mut();
        if state.header.is_some() {
            anyhow::bail!("module() can only be called once");
        }
        if state.non_module_called {
            anyhow::bail!("if module() is called, it must be called before any other functions");
        }
        let _compatibility_level = compatibility_level;
        if !name.is_empty() {
            validate_module_name(name)?;
        }
        validate_version(version, "module")?;
        validate_repo_name(repo_name)?;
        for value in bazel_compatibility.items {
            validate_bazel_compatibility(value)?;
        }
        state.header = Some(RootModuleHeader {
            name: name.into(),
            version: (!version.is_empty()).then(|| version.into()),
            repo_name: (!repo_name.is_empty()).then(|| repo_name.into()),
        });
        Ok(NoneType)
    }
    fn include(label: &str, eval: &mut Evaluator) -> anyhow::Result<NoneType> {
        let mut state = recorder(eval)?.state.borrow_mut();
        state.non_module_called = true;
        state.includes.push(label.into());
        Ok(NoneType)
    }
    fn bazel_dep(
        #[starlark(require = named)] name: &str,
        #[starlark(require = named, default = "")] version: &str,
        #[starlark(require = named, default = -1)] max_compatibility_level: i32,
        #[starlark(require = named)] repo_name: Option<NoneOr<&str>>,
        #[starlark(require = named, default = false)] dev_dependency: bool,
        eval: &mut Evaluator,
    ) -> anyhow::Result<NoneType> {
        let _max_compatibility_level = max_compatibility_level;
        validate_module_name(name)?;
        validate_version(version, "bazel_dep")?;
        if let Some(NoneOr::Other(repo_name)) = repo_name {
            validate_repo_name(repo_name)?;
        }
        let (repo_name, nodep) = match repo_name {
            Some(NoneOr::None) => (None, true),
            Some(NoneOr::Other("")) | None => (Some(name.into()), false),
            Some(NoneOr::Other(repo_name)) => (Some(repo_name.into()), false),
        };
        let mut state = recorder(eval)?.state.borrow_mut();
        state.non_module_called = true;
        state.dependencies.push(RootModuleDependency {
            name: name.into(),
            version: version.into(),
            repo_name,
            nodep,
            dev_dependency,
        });
        Ok(NoneType)
    }
    fn local_path_override(
        #[starlark(require = named)] module_name: &str,
        #[starlark(require = named)] path: &str,
        eval: &mut Evaluator,
    ) -> anyhow::Result<NoneType> {
        validate_module_name(module_name)?;
        let mut state = recorder(eval)?.state.borrow_mut();
        state.non_module_called = true;
        state.overrides.push(RootModuleLocalPathOverride {
            module_name: module_name.into(),
            path: path.into(),
        });
        Ok(NoneType)
    }
}

fn evaluate_module_file(path: &Path, source: &str) -> Result<ModuleFileEvaluation, CompactString> {
    let ast = AstModule::parse(
        &path.display().to_string(),
        source.to_owned(),
        &Dialect::Standard,
    )
    .map_err(|e| CompactString::new(e.to_string()))?;
    let module = Module::new();
    let recorder = Recorder::default();
    let globals = GlobalsBuilder::standard().with(module_globals).build();
    {
        let mut evaluator = Evaluator::new(&module);
        evaluator.extra = Some(&recorder);
        evaluator
            .eval_module(ast, &globals)
            .map_err(|e| CompactString::new(e.to_string()))?;
    }
    let recorder = recorder.state.into_inner();
    Ok(ModuleFileEvaluation {
        path: path.to_path_buf(),
        header: recorder.header,
        includes: recorder.includes.into(),
        dependencies: recorder.dependencies.into(),
        local_path_overrides: recorder.overrides.into(),
    })
}
