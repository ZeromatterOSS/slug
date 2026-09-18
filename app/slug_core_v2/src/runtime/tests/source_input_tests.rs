use slug_build_api_v2::AnalysisArtifact;

use super::*;
use crate::runtime::source_input::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
struct SourceProbeKey(SourceArtifactInputObservationKey);

#[derive(Debug, Clone, Dupe, PartialEq, Eq, Allocative)]
struct SourceProbeTerminal {
    input: Arc<ObservedSourceArtifactInput>,
    certificate: SourceCertificate,
}

impl fmt::Display for SourceProbeKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "source-input-root:{}", self.0)
    }
}

#[async_trait]
impl Key for SourceProbeKey {
    type Value = ObservedBuildOutcome<SourceProbeTerminal>;
    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        ctx.store_evaluation_data(EventBatch::empty()).unwrap();
        ctx.compute(&self.0).await.unwrap().map(|result| {
            result.map(|input| SourceProbeTerminal {
                certificate: SourceCertificate::from_epoch(input.observations().dupe()).unwrap(),
                input,
            })
        })
    }
    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }
    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[derive(Clone)]
struct SourceProbeRoot {
    key: SourceProbeKey,
    mutation: Arc<Mutex<Option<PathBuf>>>,
    completions: Arc<AtomicUsize>,
    rejected: Arc<Mutex<Option<SourceArtifactInputError>>>,
}

#[async_trait]
impl NativeCommandRoot for SourceProbeRoot {
    type Terminal = SourceProbeTerminal;
    fn initializes_request_revision(&self) -> bool {
        true
    }
    fn source_certificate<'a>(
        &self,
        terminal: &'a Self::Terminal,
    ) -> Option<&'a SourceCertificate> {
        Some(&terminal.certificate)
    }
    fn observations<'a>(&self, terminal: &'a Self::Terminal) -> Option<&'a PathObservationEpoch> {
        Some(terminal.input.observations())
    }
    fn observed_selection_association(&self) -> ObservedSelectionAssociation {
        ObservedSelectionAssociation::SelectedDependencySuperset
    }
    fn event_reconciliation_policy(&self, _: &Self::Terminal) -> EventReconciliationPolicy {
        EventReconciliationPolicy::SourceCertifiedCurrentClosure
    }
    async fn compute(
        &self,
        tx: &mut dice::DiceTransaction,
    ) -> Result<PreparationOutcome<Self::Terminal>, NativeDemandSessionError> {
        let result = tx
            .compute(&self.key)
            .await
            .map_err(|error| NativeDemandSessionError::Computation(anyhow::anyhow!("{error}")))?;
        if let PreparationOutcome::Complete(Ok(terminal)) = &result {
            if let Err(error) = terminal.input.result() {
                *self.rejected.lock().unwrap() = Some(error.clone());
                return Err(NativeDemandSessionError::Computation(anyhow::anyhow!(
                    "source input rejected: {error:?}"
                )));
            }
            self.completions.fetch_add(1, Ordering::SeqCst);
            let mutation = self.mutation.lock().unwrap().take();
            if let Some(path) = mutation {
                fs::write(path, b"bbb").unwrap();
            }
        }
        project_native_observed_terminal(result)
    }
}

fn root(runtime: &WorkspaceRuntime, label: &str) -> SourceProbeRoot {
    SourceProbeRoot {
        key: SourceProbeKey(
            SourceArtifactInputObservationKey::new(
                NormalizedAbsolutePath::new(runtime.workspace.clone()).unwrap(),
                &AnalysisArtifact::Source(CanonicalLabel::parse(label).unwrap()),
            )
            .unwrap(),
        ),
        mutation: Arc::new(Mutex::new(None)),
        completions: Arc::new(AtomicUsize::new(0)),
        rejected: Arc::new(Mutex::new(None)),
    }
}

fn request(runtime: &WorkspaceRuntime) -> NativeDemandRequestInputBundle {
    let mut request = NativeDemandRequestInputBundle::normalized_initial();
    request.registry_urls = RegistryUrls::new([format!(
        "file://{}/empty-registry",
        runtime.workspace.display()
    )]);
    request
}

fn reject(runtime: &WorkspaceRuntime, root: SourceProbeRoot) -> SourceArtifactInputError {
    let before = runtime
        .native_demand_sessions
        .state
        .lock()
        .unwrap()
        .accepted
        .clone();
    *root.rejected.lock().unwrap() = None;
    assert!(matches!(
        runtime.drive_command(request(runtime), root.clone()),
        Err(NativeDemandSessionError::Computation(_))
    ));
    let after = runtime
        .native_demand_sessions
        .state
        .lock()
        .unwrap()
        .accepted
        .clone();
    assert_eq!(after.path_observations, before.path_observations);
    assert_eq!(after.repository_results, before.repository_results);
    assert_eq!(after.inputs, before.inputs);
    assert_eq!(after.selected, before.selected);
    assert_eq!(
        after.repository_environment_frontier,
        before.repository_environment_frontier
    );
    let error = root.rejected.lock().unwrap().take().unwrap();
    error
}

fn run(runtime: &WorkspaceRuntime, root: SourceProbeRoot) -> Arc<ObservedSourceArtifactInput> {
    let driven = runtime.drive_command(request(runtime), root).unwrap();
    let input = driven.accepted.terminal_for_test().input.dupe();
    let accepted = runtime
        .native_demand_sessions
        .state
        .lock()
        .unwrap()
        .accepted
        .clone();
    for (demand, value) in input.observations().observations() {
        assert!(
            Arc::ptr_eq(value, accepted.path_observations.get(demand).unwrap()),
            "{demand:?}"
        );
    }
    input
}

fn workspace() -> tempfile::TempDir {
    let parent =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/wp730/source-input-fixtures");
    fs::create_dir_all(&parent).unwrap();
    tempfile::tempdir_in(parent.canonicalize().unwrap()).unwrap()
}

fn module(repository: &str) -> String {
    let mut source = format!(
        "module(name='source_probe')\nbazel_dep(name='dep',version='1.0.0')\nlocal_path_override(module_name='dep',path='{repository}')\n"
    );
    for (name, version) in BUILTIN_DEPENDENCIES {
        source.push_str(&format!(
            "local_path_override(module_name='{name}',path='{name}')\n"
        ));
        if matches!(
            *name,
            "bazel_features" | "rules_apple" | "rules_swift" | "abseil-cpp"
        ) {
            source.push_str(&format!("bazel_dep(name='{name}',version='{version}')\n"));
        }
    }
    source
}

// Reuse the focused analysis fixture's declarations for unrelated dependencies
// of the unchanged built-in bazel_tools MODULE. No rule bodies are needed.
const BUILTIN_DEPENDENCIES: &[(&str, &str)] = &[
    ("rules_license", "1.0.0"),
    ("buildozer", "8.5.1"),
    ("platforms", "1.0.0"),
    ("zlib", "1.3.1.bcr.5"),
    ("bazel_features", "1.42.1"),
    ("protobuf", "33.4"),
    ("rules_java", "9.1.0"),
    ("rules_cc", "0.2.17"),
    ("rules_python", "1.7.0"),
    ("rules_shell", "0.6.1"),
    ("apple_support", "1.24.2"),
    ("rules_apple", "4.1.0"),
    ("rules_swift", "3.1.2"),
    ("abseil-cpp", "20250814.1"),
];

#[test]
fn native_source_inputs_route_digest_restore_and_reject_missing_main_shadow() {
    let workspace = workspace();
    fs::write(workspace.path().join("MODULE.bazel"), module("dep")).unwrap();
    fs::write(workspace.path().join("input"), b"main").unwrap();
    for (name, version) in BUILTIN_DEPENDENCIES {
        fs::create_dir(workspace.path().join(name)).unwrap();
        fs::write(
            workspace.path().join(name).join("MODULE.bazel"),
            format!("module(name='{name}',version='{version}')\n"),
        )
        .unwrap();
    }
    for repo in ["dep", "dep2"] {
        fs::create_dir(workspace.path().join(repo)).unwrap();
        fs::write(
            workspace.path().join(repo).join("MODULE.bazel"),
            "module(name='dep',version='1.0.0')\n",
        )
        .unwrap();
        fs::write(workspace.path().join(repo).join("input"), b"aaa").unwrap();
    }
    let runtime = test_runtime(workspace.path()).unwrap();
    let external = root(&runtime, "@@dep+//:input");
    let first = run(&runtime, external.clone());
    let first_fact = first.result().as_ref().unwrap();
    assert_eq!(
        first_fact.real_path().as_path(),
        workspace.path().join("dep/input")
    );
    assert_eq!(first_fact.digest().size_bytes(), 3);
    assert_eq!(first_fact.namespace(), PathObservationNamespace::Host);
    assert!(
        first
            .observations()
            .observations()
            .keys()
            .any(
                |demand| demand.path().as_path() == workspace.path().join("MODULE.bazel")
                    && demand.operation() == PathObservationOperation::FileBytes
            )
    );
    let source = workspace.path().join("dep/input");
    assert!(
        first
            .observations()
            .observations()
            .keys()
            .any(|demand| demand.path().as_path() == source
                && demand.operation() == PathObservationOperation::FileDigest)
    );
    assert!(
        !first
            .observations()
            .observations()
            .keys()
            .any(|demand| demand.path().as_path() == source
                && demand.operation() == PathObservationOperation::FileBytes)
    );
    assert_eq!(
        first_fact,
        run(&runtime, external.clone()).result().as_ref().unwrap()
    );
    let main = run(&runtime, root(&runtime, "@@//:input"));
    assert_ne!(
        first_fact.digest(),
        main.result().as_ref().unwrap().digest()
    );
    for bytes in [b"bbb", b"aaa"] {
        fs::write(&source, bytes).unwrap();
        let changed = run(&runtime, external.clone());
        assert_eq!(
            first_fact == changed.result().as_ref().unwrap(),
            bytes == b"aaa"
        );
    }
    fs::remove_file(&source).unwrap();
    assert!(workspace.path().join("input").is_file());
    assert!(matches!(
        reject(&runtime, external.clone()),
        SourceArtifactInputError::Missing
    ));
    fs::create_dir(&source).unwrap();
    assert!(matches!(
        reject(&runtime, external.clone()),
        SourceArtifactInputError::Digest(_)
    ));
    fs::remove_dir(&source).unwrap();
    fs::write(&source, b"aaa").unwrap();
    assert_eq!(
        first_fact,
        run(&runtime, external.clone()).result().as_ref().unwrap()
    );
    fs::write(workspace.path().join("MODULE.bazel"), module("dep2")).unwrap();
    let relocated = run(&runtime, external.clone());
    let relocated = relocated.result().as_ref().unwrap();
    assert_eq!(first_fact.label(), relocated.label());
    assert_eq!(first_fact.digest(), relocated.digest());
    assert_ne!(first_fact.real_path(), relocated.real_path());
    fs::write(workspace.path().join("MODULE.bazel"), module("dep")).unwrap();
    assert_eq!(
        first_fact,
        run(&runtime, external).result().as_ref().unwrap()
    );
}

#[test]
fn native_source_digest_is_revalidated_before_acceptance() {
    let workspace = workspace();
    let source = workspace.path().join("input");
    fs::write(&source, b"aaa").unwrap();
    let runtime = test_runtime(workspace.path()).unwrap();
    let probe = root(&runtime, "@@//:input");
    let first = run(&runtime, probe.clone());
    *probe.mutation.lock().unwrap() = Some(source);
    probe.completions.store(0, Ordering::SeqCst);
    let accepted = run(&runtime, probe.clone());
    assert!(probe.completions.load(Ordering::SeqCst) >= 2);
    assert_ne!(
        first.result().as_ref().unwrap().digest(),
        accepted.result().as_ref().unwrap().digest()
    );
    assert_eq!(
        accepted.result().as_ref().unwrap(),
        run(&runtime, probe).result().as_ref().unwrap()
    );
}

#[cfg(unix)]
#[test]
fn native_source_symlink_retargets_even_when_content_matches() {
    let workspace = workspace();
    for name in ["a", "b"] {
        fs::write(workspace.path().join(name), b"aaa").unwrap();
    }
    let link = workspace.path().join("input");
    std::os::unix::fs::symlink("a", &link).unwrap();
    let runtime = test_runtime(workspace.path()).unwrap();
    let probe = root(&runtime, "@@//:input");
    let first = run(&runtime, probe.clone());
    for target in ["b", "a"] {
        fs::remove_file(&link).unwrap();
        std::os::unix::fs::symlink(target, &link).unwrap();
        let changed = run(&runtime, probe.clone());
        assert_eq!(
            first.result().as_ref().unwrap().digest(),
            changed.result().as_ref().unwrap().digest()
        );
        assert_eq!(first.result() == changed.result(), target == "a");
        assert!(
            changed
                .observations()
                .observations()
                .keys()
                .any(
                    |demand| demand.operation() == PathObservationOperation::ReadLink
                        && demand.path().as_path() == link
                )
        );
    }
}
