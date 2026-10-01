use base64::Engine as _;
use sha2::Digest;
use slug_workspace_v2::PathObservationInstanceId;

use super::*;

struct GeneratedPathFixture {
    workspace: tempfile::TempDir,
    policy: BzlmodCommandPolicyKey,
    registries: Vec<String>,
    runtime: WorkspaceRuntime,
}

impl GeneratedPathFixture {
    fn new() -> Self {
        let workspace = tempfile::tempdir().unwrap();
        let policy = configured_test_command_policy(workspace.path());
        fs::write(workspace.path().join("MODULE.bazel"), configured_test_module(
            "module(name='generated_paths')\na=use_extension('//:ext.bzl','consumer')\nuse_repo(a,proof='proof')\nb=use_extension('//:ext.bzl','producer')\nuse_repo(b,tools='tools')\na.path(file='@tools//:marker')\n",
        )).unwrap();
        fs::write(
            workspace.path().join("BUILD.bazel"),
            "alias(name='proof', actual='@proof//:value')\n",
        )
        .unwrap();
        fs::write(
            workspace.path().join("ext.bzl"),
            r#"
def _tools(ctx):
    ctx.file('REPO.bazel', '')
    ctx.file('BUILD.bazel', "exports_files(['marker'])\n")
    ctx.file('marker', 'producer-a')
    ctx.file('bin/tool', 'tool', executable=True)
tools_repo = repository_rule(implementation=_tools)
def _alternate(ctx):
    ctx.file('REPO.bazel', '')
    ctx.file('BUILD.bazel', "exports_files(['marker'])\n")
    ctx.file('marker', 'producer-alternate')
    ctx.file('bin/tool', 'alternate-tool', executable=True)
alternate_repo = repository_rule(implementation=_alternate)
def _proof(ctx):
    ctx.file('BUILD.bazel', "exports_files(['value'])\n")
    ctx.file('value', ctx.attr.value)
proof_repo = repository_rule(implementation=_proof, attrs={'value':attr.string()})
path_tag = tag_class(attrs={'file':attr.label()})
def _producer(ctx):
    for module in ctx.modules:
        for tag in module.tags.path:
            ctx.path(tag.file)
    tools_repo(name='tools')
    alternate_repo(name='alternate')
def _consumer(ctx):
    for module in ctx.modules:
        for tag in module.tags.path:
            print('consumer-path-complete')
            path = ctx.path(tag.file)
            proof_repo(name='proof', value=str(path) + '|' + str(path.dirname.get_child('bin')))
producer = module_extension(implementation=_producer, tag_classes={'path':path_tag})
consumer = module_extension(implementation=_consumer, tag_classes={'path':path_tag})
"#,
        )
        .unwrap();
        let runtime = test_runtime(workspace.path()).unwrap();
        Self {
            workspace,
            policy,
            registries: Vec::new(),
            runtime,
        }
    }

    fn run(
        &self,
        expression: &str,
    ) -> AcceptedCommand<Arc<Result<CqueryCommandEvaluation, CqueryCommandError>>> {
        self.runtime
            .cquery_command_with_bzlmod_inputs(
                expression,
                true,
                true,
                self.policy.clone(),
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                LockfileMode::Update,
                &self.registries,
                root_setting_overlay(None),
            )
            .unwrap()
    }

    fn generated_roots(
        &self,
    ) -> Vec<(
        Arc<RepositoryMaterializationRequest>,
        PathBuf,
        PathObservationInstanceId,
    )> {
        let guard = NativeDemandAbortGuard::new(
            self.runtime
                .begin_native_demand_command()
                .unwrap()
                .into_command(),
        );
        let mut result = Vec::new();
        for request in guard.command().reusable_requests.values() {
            if !matches!(
                request.kind,
                slug_bzlmod_v2::RepositoryMaterializationKind::GeneratedFileEffects(_)
            ) {
                continue;
            }
            let value = self.runtime.repository_materializer.active_result_for_test(
                guard.command().repository_session,
                request.id.canonical_repo.as_str(),
            );
            let slug_bzlmod_v2::RepositoryMaterializationResult::Success(
                slug_bzlmod_v2::RepositoryMaterializationSuccess::Immutable {
                    generation_root,
                    observation_instance,
                    ..
                },
            ) = value
            else {
                panic!("generated path must use a real immutable result: {value:?}")
            };
            result.push((request.clone(), generation_root, observation_instance));
        }
        result
    }

    fn map_tools_to(&self, generated_name: &str) {
        fs::write(self.workspace.path().join("MODULE.bazel"), configured_test_module(&format!(
            "module(name='generated_paths')\na=use_extension('//:ext.bzl','consumer')\nuse_repo(a,proof='proof')\nb=use_extension('//:ext.bzl','producer')\nuse_repo(b,tools='{generated_name}')\na.path(file='@tools//:marker')\n",
        ))).unwrap();
    }

    fn completed_path(
        &self,
        target: &str,
        marker: &str,
        expected_published_completions: usize,
    ) -> GeneratedPathState {
        let outcome = self.run("//:proof");
        assert!(
            outcome.terminal_for_test().as_ref().is_ok(),
            "{:?}",
            outcome.terminal_for_test()
        );
        assert_eq!(
            consumer_completion_count(&outcome),
            expected_published_completions
        );
        let roots = self.generated_roots();
        assert_eq!(
            roots.len(),
            2,
            "only the selected producer and consumer materialize: {roots:?}"
        );
        let snapshot = accepted_native_snapshot(&self.runtime);
        // Reconciliation retains an unchanged batch on the same owner node but
        // publishes it only when it differs from the prior accepted batch.
        // Suppressed publication must still leave the current completion fact.
        assert_eq!(
            snapshot
                .events
                .entries
                .iter()
                .flat_map(|(_, batch)| batch.events())
                .filter(|event| is_consumer_completion(event))
                .count(),
            1
        );
        for (request, root, instance) in &roots {
            let slug_bzlmod_v2::RepositoryMaterializationKind::GeneratedFileEffects(plan) =
                &request.kind
            else {
                unreachable!()
            };
            for effect in plan.effects() {
                assert_eq!(
                    fs::read(root.join(effect.path())).unwrap(),
                    effect.content()
                );
            }
            assert!(
                snapshot
                    .path_observations
                    .observations()
                    .keys()
                    .any(|demand| demand.namespace()
                        == PathObservationNamespace::Materialization(*instance)
                        && demand.path().as_path().starts_with(root))
            );
        }
        let (producer_request, producer_root, producer_instance) = roots
            .iter()
            .find(|(_, root, _)| root.join("marker").is_file())
            .unwrap();
        let (_, consumer_root, _) = roots
            .iter()
            .find(|(_, root, _)| root.join("value").is_file())
            .unwrap();
        assert_eq!(
            fs::read_to_string(producer_root.join("marker")).unwrap(),
            marker
        );
        let consumer_value = fs::read_to_string(consumer_root.join("value")).unwrap();
        assert_eq!(
            consumer_value,
            format!(
                "{}|{}",
                producer_root.join(target).display(),
                producer_root.join("bin").display()
            )
        );
        GeneratedPathState {
            request: producer_request.clone(),
            root: producer_root.clone(),
            instance: *producer_instance,
            consumer_value,
        }
    }
}

struct GeneratedPathState {
    request: Arc<RepositoryMaterializationRequest>,
    root: PathBuf,
    instance: PathObservationInstanceId,
    consumer_value: String,
}

fn consumer_completion_count(
    outcome: &AcceptedCommand<Arc<Result<CqueryCommandEvaluation, CqueryCommandError>>>,
) -> usize {
    outcome
        .batches_for_test()
        .iter()
        .flat_map(slug_events_v2::EventBatch::events)
        .filter(|event| is_consumer_completion(event))
        .count()
}

fn is_consumer_completion(event: &slug_events_v2::EvaluationEvent) -> bool {
    matches!(
        event,
        slug_events_v2::EvaluationEvent::StarlarkPrint { text, .. }
            if text == "consumer-path-complete"
    )
}

#[test]
fn generated_label_missing_target_is_lexical_but_missing_package_fails() {
    let fixture = GeneratedPathFixture::new();
    let source = fs::read_to_string(fixture.workspace.path().join("MODULE.bazel")).unwrap();
    fs::write(
        fixture.workspace.path().join("MODULE.bazel"),
        source.replace("@tools//:marker", "@tools//:missing"),
    )
    .unwrap();
    let lexical = fixture.completed_path("missing", "producer-a", 1);
    assert!(!lexical.root.join("missing").exists());

    fs::write(
        fixture.workspace.path().join("MODULE.bazel"),
        source.replace("@tools//:marker", "@tools//absent:missing"),
    )
    .unwrap();
    let absent = fixture.run("//:proof");
    let error = absent.terminal_for_test().as_ref().as_ref().unwrap_err();
    assert!(format!("{error:?}").contains("PackageAbsent"), "{error:?}");
    assert_eq!(consumer_completion_count(&absent), 0);
    assert!(
        fixture
            .generated_roots()
            .iter()
            .all(|(_, root, _)| !root.join("value").exists())
    );

    fs::write(fixture.workspace.path().join("MODULE.bazel"), &source).unwrap();
    fixture.completed_path("marker", "producer-a", 1);
}

#[test]
fn generated_label_mapping_a_b_a_updates_selected_owner_and_immutable_path() {
    let fixture = GeneratedPathFixture::new();
    let first = fixture.completed_path("marker", "producer-a", 1);
    fixture.map_tools_to("alternate");
    // The consumer owner and its print location/text stay unchanged across the
    // mapping edits, so the retained batch is not published again.
    let alternate = fixture.completed_path("marker", "producer-alternate", 0);
    assert_ne!(
        alternate.request.id.canonical_repo,
        first.request.id.canonical_repo
    );
    assert_ne!(alternate.request.kind, first.request.kind);
    assert_ne!(alternate.root, first.root);
    assert_ne!(alternate.instance, first.instance);
    assert_ne!(alternate.consumer_value, first.consumer_value);

    fixture.map_tools_to("tools");
    let restored = fixture.completed_path("marker", "producer-a", 0);
    assert_eq!(restored.request.as_ref(), first.request.as_ref());
    assert_ne!(restored.root, alternate.root);
    assert_ne!(restored.instance, alternate.instance);
    assert_ne!(restored.consumer_value, alternate.consumer_value);
}

#[test]
fn generated_label_producer_a_b_a_restores_ordered_plan_without_stale_path() {
    let fixture = GeneratedPathFixture::new();
    let source = fs::read_to_string(fixture.workspace.path().join("ext.bzl")).unwrap();
    let first = fixture.completed_path("marker", "producer-a", 1);
    fs::write(
        fixture.workspace.path().join("ext.bzl"),
        source.replace("'producer-a'", "'producer-b'"),
    )
    .unwrap();
    // Changing producer bytes preserves the consumer key and print event; the
    // immutable output and observed namespaces below must still change.
    let changed = fixture.completed_path("marker", "producer-b", 0);
    assert_eq!(changed.request.id, first.request.id);
    assert_ne!(changed.request.kind, first.request.kind);
    assert_ne!(changed.root, first.root);
    assert_ne!(changed.instance, first.instance);
    assert_ne!(changed.consumer_value, first.consumer_value);

    fs::write(fixture.workspace.path().join("ext.bzl"), &source).unwrap();
    let restored = fixture.completed_path("marker", "producer-a", 0);
    assert_eq!(restored.request.as_ref(), first.request.as_ref());
    assert_ne!(restored.root, changed.root);
    assert_ne!(restored.instance, changed.instance);
    assert_ne!(restored.consumer_value, changed.consumer_value);
}

#[test]
fn independent_generated_label_path_uses_real_immutable_roots_and_replays_warm() {
    let fixture = GeneratedPathFixture::new();
    let first = fixture.run("//:proof");
    assert!(
        first.terminal_for_test().as_ref().is_ok(),
        "{:?}",
        first.terminal_for_test()
    );
    assert_eq!(
        first
            .batches_for_test()
            .iter()
            .flat_map(slug_events_v2::EventBatch::events)
            .filter(|event| format!("{event:?}").contains("consumer-path-complete"))
            .count(),
        1
    );
    let roots = fixture.generated_roots();
    assert_eq!(roots.len(), 2, "{roots:?}");
    for (request, root, instance) in &roots {
        let slug_bzlmod_v2::RepositoryMaterializationKind::GeneratedFileEffects(plan) =
            &request.kind
        else {
            unreachable!()
        };
        assert!(root.is_dir());
        for effect in plan.effects() {
            assert_eq!(
                fs::read(root.join(effect.path())).unwrap(),
                effect.content()
            );
        }
        let snapshot = accepted_native_snapshot(&fixture.runtime);
        assert!(
            snapshot
                .path_observations
                .observations()
                .keys()
                .any(|demand| demand.namespace()
                    == PathObservationNamespace::Materialization(*instance)
                    && demand.path().as_path().starts_with(root))
        );
    }
    let producer = roots
        .iter()
        .find(|(_, root, _)| root.join("marker").is_file())
        .unwrap();
    let consumer = roots
        .iter()
        .find(|(_, root, _)| root.join("value").is_file())
        .unwrap();
    assert_eq!(
        fs::read_to_string(consumer.1.join("value")).unwrap(),
        format!(
            "{}|{}",
            producer.1.join("marker").display(),
            producer.1.join("bin").display()
        )
    );
    let warm = fixture.run("//:proof");
    assert!(
        warm.terminal_for_test().as_ref().is_ok(),
        "{:?}",
        warm.terminal_for_test()
    );
    assert_eq!(consumer_completion_count(&warm), 0);
    let warm_roots = fixture.generated_roots();
    for (request, root, instance) in roots {
        let (_, warm_root, warm_instance) = warm_roots
            .iter()
            .find(|(candidate, _, _)| candidate == &request)
            .unwrap();
        assert_eq!(warm_root, &root);
        assert_eq!(warm_instance, &instance);
    }
    assert!(fixture.workspace.path().join("ext.bzl").is_file());
}

#[test]
fn generated_defining_bzl_load_cycle_terminates_and_removal_recovers() {
    // Exercise both a cold cycle and insertion into an accepted acyclic graph.
    for warm in [false, true] {
        let mut fixture = GeneratedPathFixture::new();
        let dep = fixture.workspace.path().join("dep");
        fs::create_dir(&dep).unwrap();
        let dep = dep.canonicalize().unwrap();
        let uses = "a=use_extension('//:a.bzl','consumer')\nuse_repo(a,proof='proof')\nb=use_extension('//:b.bzl','producer')\nuse_repo(b,tools='tools')\n";
        let module = format!("module(name='dep',version='1')\n{uses}");
        fs::write(dep.join("MODULE.bazel"), &module).unwrap();
        fs::write(dep.join("BUILD.bazel"), "").unwrap();
        fs::write(dep.join("REPO.bazel"), "").unwrap();
        let safe = fs::read_to_string(fixture.workspace.path().join("ext.bzl")).unwrap();
        fs::write(dep.join("a.bzl"), &safe).unwrap();
        fs::write(dep.join("b.bzl"), &safe).unwrap();

        // Archive capture goes through the production materializer's admitted
        // registry route. Keep its binary input outside the workspace source walk.
        let archives = tempfile::Builder::new()
            .prefix("generated-owner-cycle-archives-")
            .tempdir()
            .unwrap();
        let registry = fixture.workspace.path().join("registry");
        let metadata = registry.join("modules/dep/1");
        fs::create_dir_all(&metadata).unwrap();
        fs::write(registry.join("bazel_registry.json"), "{}").unwrap();
        fs::write(metadata.join("MODULE.bazel"), &module).unwrap();
        let safe_integrity = generated_owner_archive_source(&dep, &metadata, archives.path());
        fixture.registries = vec![
            url::Url::from_directory_path(&registry)
                .unwrap()
                .to_string(),
        ];
        let root_uses = uses
            .replace("'//:a.bzl'", "'@dep//:a.bzl'")
            .replace("'//:b.bzl'", "'@dep//:b.bzl'");
        fs::write(
            fixture.workspace.path().join("MODULE.bazel"),
            configured_test_module(&format!(
                // Match the existing portable registry fixtures: the root
                // supplies bazel_tools rather than fetching its implicit deps.
                "module(name='bazel_tools')\nbazel_dep(name='dep',version='1')\n{root_uses}a.path(file='@tools//:marker')\n"
            )),
        )
        .unwrap();
        if warm {
            fixture.completed_path("marker", "producer-a", 1);
        }
        fs::write(
            dep.join("b.bzl"),
            format!("load('@proof//:helper.bzl','helper')\n{safe}"),
        )
        .unwrap();
        let cycle_integrity = generated_owner_archive_source(&dep, &metadata, archives.path());
        assert_ne!(cycle_integrity, safe_integrity);
        let cycle = fixture.run("//:proof");
        let error = cycle.terminal_for_test().as_ref().as_ref().unwrap_err();
        assert!(
            error
                .to_string()
                .contains("cycle detected in generated repository dependencies"),
            "warm={warm}: {error:?}"
        );
        assert_eq!(consumer_completion_count(&cycle), 0, "warm={warm}");
        assert!(fixture.generated_roots().is_empty(), "warm={warm}");
        fs::write(dep.join("b.bzl"), &safe).unwrap();
        assert_eq!(
            generated_owner_archive_source(&dep, &metadata, archives.path()),
            safe_integrity
        );
        fixture.completed_path("marker", "producer-a", 1);
    }
}

fn generated_owner_archive_source(dep: &Path, metadata: &Path, archives: &Path) -> String {
    // Closed sorted members and fixed headers make A/B/A bytes deterministic.
    // Repacking after each source edit changes the immutable source identity;
    // editing the staging directory alone would not invalidate an old archive.
    let compressed = flate2::GzBuilder::new()
        .mtime(0)
        .write(Vec::new(), flate2::Compression::default());
    let mut archive = tar::Builder::new(compressed);
    for member in [
        "BUILD.bazel",
        "MODULE.bazel",
        "REPO.bazel",
        "a.bzl",
        "b.bzl",
    ] {
        let bytes = fs::read(dep.join(member)).unwrap();
        let mut header = tar::Header::new_gnu();
        header.set_size(bytes.len() as u64);
        header.set_mode(0o644);
        header.set_uid(0);
        header.set_gid(0);
        header.set_mtime(0);
        header.set_cksum();
        archive
            .append_data(&mut header, member, bytes.as_slice())
            .unwrap();
    }
    let bytes = archive.into_inner().unwrap().finish().unwrap();
    let digest = sha2::Sha256::digest(&bytes);
    let archive_path = archives.join(format!("dep-{}.tar.gz", hex::encode(digest)));
    fs::write(&archive_path, &bytes).unwrap();
    let url = url::Url::from_file_path(archive_path).unwrap();
    let integrity = format!(
        "sha256-{}",
        base64::engine::general_purpose::STANDARD.encode(digest)
    );
    fs::write(
        metadata.join("source.json"),
        format!(
            "{{\"url\":\"{url}\",\"integrity\":\"{integrity}\",\"archive_type\":\"tar.gz\",\"strip_prefix\":\"\",\"patches\":{{}},\"patch_strip\":0}}"
        ),
    )
    .unwrap();
    integrity
}

mod shared_context_tests {
    use super::*;
    include!("generated_owner_shared_tests.rs");
}
