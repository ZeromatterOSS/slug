use super::*;

type Outcome = <ConfiguredNodeAnalysisObservationKey as Key>::Value;

const DEFS: &str = r#"Observed = provider(fields={'value':''})
def _tool(ctx): return [platform_common.ToolchainInfo(marker=ctx.attr.marker)]
tool = rule(implementation=_tool, attrs={'marker':attr.string()})
def _consumer(ctx): return [Observed(value=ctx.toolchains['//:type'].marker)]
consumer = rule(implementation=_consumer, toolchains=['//:type'])
"#;
const BUILD: &str = r#"load(':defs.bzl','tool','consumer')
constraint_setting(name='cpu')
constraint_value(name='x86', constraint_setting=':cpu')
constraint_value(name='ppc', constraint_setting=':cpu')
platform(name='platform', constraint_values=[':x86'])
toolchain_type(name='type')
tool(name='good_impl', marker='good')
tool(name='alternate', marker='alternate')
filegroup(name='plain')
config_setting(name='on', values={'compilation_mode':'fastbuild'})
toolchain(name='good', toolchain_type=':type', toolchain=':good_impl', exec_compatible_with=[':x86'])
alias(name='declaration', actual=':good')
toolchain(name='rejected', toolchain_type=':type', toolchain='@poison//:impl', exec_compatible_with=[':ppc'], target_settings=[':on'])
consumer(name='consumer')
"#;
const POISON: &str = "def _repo(ctx): fail('IMPLEMENTATION_FETCHED')\nrepo = repository_rule(implementation=_repo)\n";

struct ScratchCleanup(PathBuf);

impl Drop for ScratchCleanup {
    fn drop(&mut self) {
        // Success removes the directory explicitly; retain panic-path cleanup
        // without replacing an existing failure with a destructor panic.
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn fixture() -> (PathBuf, ConfigurationKey, ScratchCleanup) {
    let root = scratch();
    let cleanup = ScratchCleanup(root.clone());
    fs::write(root.join("MODULE.bazel"), "module(name='root')\np=use_repo_rule('//:repo.bzl','repo')\np(name='poison')\nregister_toolchains('//:declaration','//:rejected')\n").unwrap();
    fs::write(root.join("defs.bzl"), DEFS).unwrap();
    fs::write(root.join("repo.bzl"), POISON).unwrap();
    fs::write(root.join("BUILD.bazel"), BUILD).unwrap();
    let config = ConfigurationKey::from_slug(
        test_configuration()
            .slug_configuration()
            .unwrap()
            .with_host_platform_label(&CanonicalLabel::parse("@@//:platform").unwrap()),
    );
    (root, config, cleanup)
}

#[derive(Default)]
struct Tracker {
    effects: AtomicUsize,
    roots: Mutex<Vec<(dice::RootActivation, bool)>>,
    packages: Mutex<Vec<String>>,
}
impl ActivationTracker for Tracker {
    fn key_activated(&self, key: &DynKey, _: &mut dyn Iterator<Item = &DynKey>, _: ActivationData) {
        if let Some(key) = key.downcast_ref::<HostPackageInventoryKey>() {
            self.packages.lock().unwrap().push(key.to_string());
        }
        if let Some(key) = key.downcast_ref::<HostPackageInventoryObservationKey>() {
            self.packages.lock().unwrap().push(key.to_string());
        }
        if key
            .downcast_ref::<slug_loading_v2::HostSelectedRepositoryFileEffectKey>()
            .is_some()
            || key
                .downcast_ref::<slug_loading_v2::HostSelectedRepositoryFileEffectObservationKey>()
                .is_some()
        {
            self.effects.fetch_add(1, Ordering::SeqCst);
        }
    }
    fn root_completed(&self, key: &DynKey, root: dice::RootActivation, transient: bool) {
        if key
            .downcast_ref::<ConfiguredNodeAnalysisObservationKey>()
            .is_some()
        {
            self.roots.lock().unwrap().push((root, transient));
        }
    }
}

async fn request(
    dice: &Arc<Dice>,
    root: &PathBuf,
    config: &ConfigurationKey,
    tracker: Arc<Tracker>,
) -> Outcome {
    let workspace = NormalizedAbsolutePath::new(root.clone()).unwrap();
    // Prepare only the registration source prefix before the consumer tracker
    // exists. The zero-effects assertions below still cover every consumer.
    let request_data = |activation_tracker: Option<Arc<dyn ActivationTracker>>| {
        let mut data = UserComputationData {
            activation_tracker,
            cycle_detector: Some(analysis_cycle_detector()),
            ..Default::default()
        };
        data.data.set(CaptureEvaluationEvents);
        let host = builtin_fixture::RepositoryHostInputs::prepare(root, &mut data);
        (data, host)
    };
    let (data, host) = request_data(None);
    let mut updater = dice.updater_with_data(data);
    host.inject(&mut updater);
    let entries = inject_root_target_inputs_with_entries(&mut updater, root, root_epoch(root), &[]);
    let transaction = updater.commit().await;
    let transaction = builtin_fixture::prepare_generated_registrations(transaction, root, entries)
        .await
        .unwrap();
    let (data, _) = request_data(Some(tracker.clone()));
    let mut transaction = transaction.into_updater().commit_with_data(data).await;
    let key = ConfiguredNodeAnalysisObservationKey::new(
        workspace,
        ConfiguredTargetKey::new(
            CanonicalLabel::parse("@@//:consumer").unwrap(),
            config.clone(),
        ),
    )
    .unwrap();
    let outcome = transaction.compute(&key).await.unwrap();
    let activation = tracker.roots.lock().unwrap().last().copied();
    if let Some((activation, false)) = activation {
        let closure = transaction
            .activation_closure([activation.node()])
            .await
            .unwrap();
        assert_eq!(closure.version(), transaction.version());
        assert_eq!(closure.roots(), &[activation.node()]);
        assert!(
            closure
                .nodes()
                .iter()
                .any(|node| node.node() == activation.node())
        );
    }
    outcome
}

fn result(outcome: Outcome) -> Result<Arc<ConfiguredNodeResult>, String> {
    let AnalysisPreparationOutcome::Complete(Ok(value)) = outcome else {
        panic!("analysis did not complete: {outcome:?}")
    };
    value
        .as_ref()
        .as_ref()
        .cloned()
        .map_err(ToString::to_string)
}

fn observed(result: &ConfiguredNodeResult) -> String {
    provider_value(result, &ProviderId::new("//:defs.bzl", "Observed").unwrap())
}

#[tokio::test]
async fn rejected_generated_implementation_is_lazy_and_selected_callback_failure_need_restore() {
    let (root, config, _cleanup) = fixture();
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(Tracker::default());
    let first = result(request(&dice, &root, &config, tracker.clone()).await).unwrap();
    assert_eq!(observed(&first), "good");
    assert_eq!(
        tracker.effects.load(Ordering::SeqCst),
        0,
        "rejected repository callback was evaluated"
    );
    let selected = BUILD.replace("constraint_values=[':x86']", "constraint_values=[':ppc']");
    fs::write(root.join("BUILD.bazel"), &selected).unwrap();
    let error = result(request(&dice, &root, &config, tracker.clone()).await).unwrap_err();
    assert!(error.contains("IMPLEMENTATION_FETCHED"), "{error}");
    assert!(tracker.effects.load(Ordering::SeqCst) > 0);
    fs::write(root.join("BUILD.bazel"), BUILD).unwrap();
    assert_eq!(
        result(request(&dice, &root, &config, tracker.clone()).await).unwrap(),
        first
    );

    fs::write(root.join("repo.bzl"), "def _repo(ctx):\n    ctx.file('BUILD.bazel', \"load(':defs.bzl','tool')\\ntool(name='impl')\\n\")\n    ctx.file('defs.bzl', \"def _tool(ctx): return [platform_common.ToolchainInfo(marker='generated')]\\ntool=rule(implementation=_tool)\\n\")\nrepo=repository_rule(implementation=_repo)\n").unwrap();
    fs::write(root.join("BUILD.bazel"), &selected).unwrap();
    let outcome = request(&dice, &root, &config, tracker.clone()).await;
    let AnalysisPreparationOutcome::Need(needs) = outcome else {
        panic!("selected generated files did not demand real materialization: {outcome:?}")
    };
    assert_eq!(needs.repository_materializations().len(), 1);
    let materialization = needs.repository_materializations().values().next().unwrap();
    let slug_bzlmod_v2::RepositoryMaterializationKind::GeneratedFileEffects(plan) =
        &materialization.kind
    else {
        panic!(
            "callback did not produce ordinary file effects: {:?}",
            materialization.kind
        )
    };
    assert!(plan.effects().iter().any(|file| file.path() == "defs.bzl"
        && String::from_utf8_lossy(file.content()).contains("ToolchainInfo(marker='generated')")));
    assert!(
        tracker.roots.lock().unwrap().last().unwrap().1,
        "Need must not publish a configured consumer"
    );
    fs::write(root.join("repo.bzl"), POISON).unwrap();
    fs::write(root.join("BUILD.bazel"), BUILD).unwrap();
    assert_eq!(
        result(request(&dice, &root, &config, tracker).await).unwrap(),
        first
    );
    dice.wait_for_idle().await;
    drop(dice);
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn rejected_implementation_alias_cycles_conditions_and_declaration_errors_restore() {
    let (root, config, _cleanup) = fixture();
    fs::create_dir(root.join("cycle")).unwrap();
    fs::write(
        root.join("cycle/BUILD.bazel"),
        "alias(name='a',actual=':b')\nalias(name='b',actual=':a')\n",
    )
    .unwrap();
    let original = BUILD.replace("toolchain='@poison//:impl'", "toolchain='//cycle:a'");
    fs::write(root.join("BUILD.bazel"), &original).unwrap();
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(Tracker::default());
    let first = result(request(&dice, &root, &config, tracker.clone()).await).unwrap();
    assert_eq!(observed(&first), "good");
    let packages = tracker.packages.lock().unwrap().clone();
    assert!(
        packages.iter().all(|package| !package.contains("//cycle")),
        "rejected cycle implementation package was demanded: {packages:?}"
    );
    let selected = original.replace("constraint_values=[':x86']", "constraint_values=[':ppc']");
    fs::write(
        root.join("BUILD.bazel"),
        selected.replace("'compilation_mode':'fastbuild'", "'compilation_mode':'opt'"),
    )
    .unwrap();
    let nonmatching = result(request(&dice, &root, &config, tracker.clone()).await).unwrap_err();
    assert!(
        nonmatching.contains("no compatible toolchain") && !nonmatching.contains("cycle"),
        "{nonmatching}"
    );
    fs::write(root.join("BUILD.bazel"), &selected).unwrap();
    let cycle = result(request(&dice, &root, &config, tracker.clone()).await).unwrap_err();
    assert!(cycle.contains("cycle"), "{cycle}");
    fs::write(
        root.join("BUILD.bazel"),
        selected.replace("toolchain='//cycle:a'", "toolchain=':alternate'"),
    )
    .unwrap();
    let alternate = result(request(&dice, &root, &config, tracker.clone()).await).unwrap();
    assert_eq!(observed(&alternate), "alternate");
    fs::write(
        root.join("defs.bzl"),
        DEFS.replace(
            "[platform_common.ToolchainInfo(marker=ctx.attr.marker)]",
            "[]",
        ),
    )
    .unwrap();
    let missing_provider =
        result(request(&dice, &root, &config, tracker.clone()).await).unwrap_err();
    assert!(
        missing_provider.contains("does not provide ToolchainInfo"),
        "{missing_provider}"
    );
    fs::write(root.join("defs.bzl"), DEFS).unwrap();
    assert_eq!(
        result(request(&dice, &root, &config, tracker.clone()).await).unwrap(),
        alternate
    );
    fs::write(root.join("BUILD.bazel"), &selected).unwrap();
    assert_eq!(
        result(request(&dice, &root, &config, tracker.clone()).await).unwrap_err(),
        cycle
    );
    fs::write(root.join("BUILD.bazel"), &original).unwrap();
    assert_eq!(
        result(request(&dice, &root, &config, tracker.clone()).await).unwrap(),
        first
    );
    for (from, to, expected) in [
        ("actual=':good'", "actual=':declaration'", "cycle"),
        (
            "actual=':good'",
            "actual=':plain'",
            "registered toolchain is not toolchain",
        ),
        (
            "exec_compatible_with=[':x86']",
            "exec_compatible_with=[':plain']",
            "expected configured constraint value",
        ),
    ] {
        fs::write(root.join("BUILD.bazel"), original.replace(from, to)).unwrap();
        let error = result(request(&dice, &root, &config, tracker.clone()).await).unwrap_err();
        assert!(error.contains(expected), "{expected}: {error}");
        fs::write(root.join("BUILD.bazel"), &original).unwrap();
        assert_eq!(
            result(request(&dice, &root, &config, tracker.clone()).await).unwrap(),
            first
        );
    }
    dice.wait_for_idle().await;
    drop(dice);
    fs::remove_dir_all(root).unwrap();
}
