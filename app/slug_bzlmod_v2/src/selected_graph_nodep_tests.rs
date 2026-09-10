// Pinned Bazel 8220c619 DiscoveryTest.testNodep_* regressions. These inline
// registry fixtures reuse the isolated bazel_tools root, not a CLI builtin.
struct NodepRegistrySpy {
    inner: RegistryIo,
    reads: std::sync::Mutex<Vec<String>>,
}

impl NodepRegistrySpy {
    fn new(modules: &[(&str, &str, &str)]) -> Arc<Self> {
        Arc::new(Self {
            inner: RegistryIo(
                modules
                    .iter()
                    .map(|(name, version, body)| {
                        (
                            format!(
                                "https://registry.invalid/modules/{name}/{version}/MODULE.bazel"
                            ),
                            Arc::from(body.as_bytes()),
                        )
                    })
                    .collect(),
            ),
            reads: Default::default(),
        })
    }

    fn read_module(&self, name: &str, version: &str) -> bool {
        let suffix = format!("/modules/{name}/{version}/MODULE.bazel");
        self.reads
            .lock()
            .unwrap()
            .iter()
            .any(|url| url.ends_with(&suffix))
    }
}

#[async_trait]
impl crate::RegistryIo for NodepRegistrySpy {
    async fn read_exact(
        &self,
        url: &crate::RegistryFileUrl,
    ) -> Result<crate::RegistryIoOutcome, crate::RegistryTransportError> {
        self.reads.lock().unwrap().push(url.as_str().to_owned());
        crate::RegistryIo::read_exact(&self.inner, url).await
    }
}

fn nodep_dice(io: Arc<NodepRegistrySpy>) -> Arc<Dice> {
    let mut builder = Dice::builder();
    crate::install_registry_io(&mut builder, io);
    builder.build(DetectCycles::Enabled)
}

fn nodep_graph(outcome: &GraphOutcome) -> &HostSelectedModuleGraph {
    let SourcePreparationOutcome::Complete(value) = outcome else {
        panic!("inline registry graph must complete: {outcome:?}");
    };
    value.as_ref().as_ref().unwrap()
}

#[tokio::test]
async fn nodep_unfulfilled_edges_are_pruned_after_fixed_point() {
    let io = NodepRegistrySpy::new(&[("optional", "1", "module(name='optional', version='1')\n")]);
    let dice = nodep_dice(io.clone());
    let outcome = compute_graph(
        &dice,
        "module(name='bazel_tools')\nbazel_dep(name='optional', version='1', repo_name=None)\n",
        1,
    )
    .await;
    assert!(
        !io.read_module("optional", "1"),
        "absent name must not fetch available metadata"
    );
    let graph = nodep_graph(&outcome);
    assert_eq!(graph.unpruned.len(), 1);
    assert_eq!(graph.resolved.len(), 1);
    assert!(graph.unpruned[0].nodep_dependencies.is_empty());
    assert!(graph.resolved[0].nodep_dependencies.is_empty());
}

#[test]
fn nodep_pruning_is_exact_transformed_stable_and_idempotent() {
    let root = HostGraphModuleKey::Root;
    let empty = HostGraphModuleKey::module("local".into(), BazelModuleVersion::empty());
    let mut transformed = dependency("override", key("dep", "1"));
    transformed.transformed = key("dep", "2");
    let mut requested_only = dependency("requested-only", key("dep", "2"));
    requested_only.transformed = key("dep", "3");
    let mut root_dep = dependency("self", key("self", "9"));
    root_dep.transformed = root.clone();
    let mut local_dep = dependency("local", key("local", "9"));
    local_dep.transformed = empty.clone();
    // Same-name/wrong-version cannot be a successful converged producer state:
    // eligible versions must be fetched. Here it discriminates helper membership.
    let nodeps = vec![
        dependency("absent", key("absent", "1")),
        transformed,
        dependency("wrong-version", key("dep", "1")),
        root_dep,
        requested_only,
        local_dep,
        dependency("last", key("dep", "2")),
    ];
    let ordinary = dependency("ordinary-missing", key("missing", "1"));
    let mut entries = vec![
        module(root.clone(), vec![ordinary.clone()], nodeps.clone()),
        module(key("dep", "2"), vec![], nodeps),
        module(empty.clone(), vec![], vec![]),
    ];
    entries[1].source = HostGraphModuleSource::Discovered(discovered("dep", "2"));
    let before = entries.clone();
    let keys = entries.iter().map(|entry| entry.key.clone()).collect();
    for _ in 0..2 {
        prune_unfulfilled_nodep_edges(&mut entries, &keys);
        for index in 0..2 {
            assert_eq!(entries[index].key, before[index].key);
            assert_eq!(entries[index].source, before[index].source);
            assert_eq!(
                entries[index]
                    .nodep_dependencies
                    .iter()
                    .map(|dep| {
                        (
                            dep.apparent_name.as_deref(),
                            dep.requested.clone(),
                            dep.transformed.clone(),
                        )
                    })
                    .collect::<Vec<_>>(),
                vec![
                    (Some("override"), key("dep", "1"), key("dep", "2")),
                    (Some("self"), key("self", "9"), root.clone()),
                    (Some("local"), key("local", "9"), empty.clone()),
                    (Some("last"), key("dep", "2"), key("dep", "2")),
                ],
            );
        }
        assert!(entries[2].nodep_dependencies.is_empty());
        for deps in [&entries[0].dependencies, &entries[0].original_dependencies] {
            assert_eq!(deps.len(), 1);
            assert_eq!(deps[0].apparent_name, ordinary.apparent_name);
            assert_eq!(deps[0].requested, ordinary.requested);
            assert_eq!(deps[0].transformed, ordinary.transformed);
        }
        match (&entries[0].source, &before[0].source) {
            (HostGraphModuleSource::Root(a), HostGraphModuleSource::Root(b)) => {
                assert!(Arc::ptr_eq(a, b))
            }
            _ => unreachable!(),
        }
        match (&entries[1].source, &before[1].source) {
            (HostGraphModuleSource::Discovered(a), HostGraphModuleSource::Discovered(b)) => {
                assert!(Arc::ptr_eq(a, b))
            }
            _ => unreachable!(),
        }
    }
}

#[tokio::test]
async fn nodep_two_and_three_round_fulfillment_is_not_pruned_early() {
    // DiscoveryTest.testNodep_fulfilled[_manyRounds]: c2 introduces the name d
    // only after c1 makes c eligible. d2 must then be fetched in a later round.
    for many_rounds in [false, true] {
        let io = NodepRegistrySpy::new(&[
            (
                "b",
                "1",
                "module(name='b',version='1')\nbazel_dep(name='c',version='1')\n",
            ),
            ("c", "1", "module(name='c',version='1')\n"),
            (
                "c",
                "2",
                if many_rounds {
                    "module(name='c',version='2')\nbazel_dep(name='d',version='1')\n"
                } else {
                    "module(name='c',version='2')\n"
                },
            ),
            ("d", "1", "module(name='d',version='1')\n"),
            ("d", "2", "module(name='d',version='2')\n"),
        ]);
        let dice = nodep_dice(io.clone());
        let root = "module(name='bazel_tools')\nbazel_dep(name='b',version='1')\nbazel_dep(name='c',version='2',repo_name=None)\nbazel_dep(name='d',version='2',repo_name=None)\n";
        let outcome = compute_graph(&dice, root, 1).await;
        let graph = nodep_graph(&outcome);
        let mut expected = vec![
            HostGraphModuleKey::Root,
            key("b", "1"),
            key("c", "2"),
            key("c", "1"),
        ];
        let mut nodeps = vec![key("c", "2")];
        if many_rounds {
            // Final BFS admits d2 directly from root, then d1 through c2.
            expected.insert(3, key("d", "2"));
            expected.push(key("d", "1"));
            nodeps.push(key("d", "2"));
        }
        assert_eq!(
            graph
                .unpruned
                .iter()
                .map(|entry| entry.key.clone())
                .collect::<Vec<_>>(),
            expected
        );
        assert_eq!(
            graph.unpruned[0]
                .nodep_dependencies
                .iter()
                .map(|dep| dep.key.clone())
                .collect::<Vec<_>>(),
            nodeps
        );
        assert!(io.read_module("c", "1") && io.read_module("c", "2"));
        assert_eq!(io.read_module("d", "1"), many_rounds);
        assert_eq!(io.read_module("d", "2"), many_rounds);
        assert_eq!(graph.resolved[1].dependencies[0].key, key("c", "2"));
    }
}

#[tokio::test]
async fn nodep_eligible_missing_or_invalid_module_and_ordinary_missing_stay_errors() {
    for invalid in [false, true] {
        let mut modules = vec![("dep", "1", "module(name='dep',version='1')\n")];
        if invalid {
            modules.push(("dep", "2", "fail('eligible nodep failure')\n"));
        }
        let io = NodepRegistrySpy::new(&modules);
        let dice = nodep_dice(io.clone());
        let outcome = compute_graph(&dice,
            "module(name='bazel_tools')\nbazel_dep(name='dep',version='1')\nbazel_dep(name='dep',version='2',repo_name=None)\n", 1).await;
        assert!(io.read_module("dep", "2"));
        assert!(matches!(outcome, SourcePreparationOutcome::Complete(value)
            if matches!(value.as_ref(), Err(HostSelectedModuleGraphError::DiscoveryLeaf { module, .. })
                if module == &key("dep", "2"))));
    }
    let io = NodepRegistrySpy::new(&[]);
    let dice = nodep_dice(io.clone());
    let missing = compute_graph(
        &dice,
        "module(name='bazel_tools')\nbazel_dep(name='missing',version='1')\n",
        1,
    )
    .await;
    assert!(io.read_module("missing", "1"));
    assert!(matches!(missing, SourcePreparationOutcome::Complete(value)
        if matches!(value.as_ref(), Err(HostSelectedModuleGraphError::DiscoveryLeaf { module, .. })
            if module == &key("missing", "1"))));
}

#[tokio::test]
async fn nodep_override_and_root_transform_precede_membership() {
    let io = NodepRegistrySpy::new(&[
        ("b", "1", "module(name='b',version='1')\n"),
        ("b", "2", "module(name='b',version='2')\n"),
        (
            "c",
            "1",
            "module(name='c',version='1')\nbazel_dep(name='b',version='1',repo_name=None)\nbazel_dep(name='bazel_tools',version='9',repo_name=None)\n",
        ),
    ]);
    let dice = nodep_dice(io.clone());
    let outcome = compute_graph(&dice,
        "module(name='bazel_tools')\nbazel_dep(name='b',version='1')\nbazel_dep(name='c',version='1')\nsingle_version_override(module_name='b',version='2')\n", 1).await;
    let graph = nodep_graph(&outcome);
    assert!(!io.read_module("b", "1"));
    assert!(!io.read_module("bazel_tools", "9"));
    assert!(io.read_module("b", "2"));
    assert_eq!(graph.unpruned[0].dependencies[0].key, key("b", "2"));
    assert_eq!(
        graph.unpruned[0].original_dependencies[0].key,
        key("b", "1")
    );
    assert_eq!(
        graph.unpruned[2]
            .nodep_dependencies
            .iter()
            .map(|dep| dep.key.clone())
            .collect::<Vec<_>>(),
        vec![key("b", "2"), HostGraphModuleKey::Root]
    );
}

#[tokio::test]
async fn nodep_same_dice_absent_present_absent_preserves_source_and_warm_cutoff() {
    let io = NodepRegistrySpy::new(&[("optional", "1", "module(name='optional',version='1')\n")]);
    let dice = nodep_dice(io.clone());
    let root =
        "module(name='bazel_tools')\nbazel_dep(name='optional',version='1',repo_name=None)\n";
    let a = compute_graph(&dice, root, 1).await;
    assert!(nodep_graph(&a).unpruned[0].nodep_dependencies.is_empty());
    assert!(!io.read_module("optional", "1"));
    let warm = compute_graph(&dice, root, 1).await;
    let comment = compute_graph(&dice, &format!("{root}# no semantic change\n"), 1).await;
    assert!(HostSelectedModuleGraphKey::equality(&a, &warm));
    assert!(HostSelectedModuleGraphKey::equality(&a, &comment));
    if let (SourcePreparationOutcome::Complete(a), SourcePreparationOutcome::Complete(warm)) =
        (&a, &warm)
    {
        assert!(Arc::ptr_eq(a, warm));
    }
    let b = compute_graph(
        &dice,
        &format!("{root}bazel_dep(name='optional',version='1',repo_name='available')\n"),
        2,
    )
    .await;
    assert_eq!(
        nodep_graph(&b).unpruned[0].nodep_dependencies[0].key,
        key("optional", "1")
    );
    assert!(io.read_module("optional", "1"));
    assert!(!HostSelectedModuleGraphKey::equality(&a, &b));
    let restored = compute_graph(&dice, root, 3).await;
    assert!(HostSelectedModuleGraphKey::equality(&a, &restored));
    assert_eq!(nodep_graph(&a).unpruned.len(), 1);
    assert_eq!(nodep_graph(&b).unpruned.len(), 2);
    // Dropping the parsed optional declaration leaves identical pruned edges,
    // but must NOT erase the original facts participating in source equality.
    let no_declaration = compute_graph(&dice, "module(name='bazel_tools')\n", 4).await;
    assert!(
        nodep_graph(&no_declaration).unpruned[0]
            .nodep_dependencies
            .is_empty()
    );
    assert!(!HostSelectedModuleGraphKey::equality(&a, &no_declaration));
    let HostGraphModuleSource::Root(source) = &nodep_graph(&a).unpruned[0].source else {
        unreachable!()
    };
    assert_eq!(source.dependencies.len(), 1);
    assert!(source.dependencies[0].nodep);
}

#[tokio::test]
async fn nodep_nonregistry_transform_uses_empty_key_before_pruning() {
    // Exercise the existing transform with a resolved nonregistry override;
    // this unit control does not claim nonregistry source materialization.
    let workspace = NormalizedAbsolutePath::new("/selected-graph-test").unwrap();
    let mut cache = SmallMap::from_iter([(
        CompactString::new("local"),
        HostEffectiveModuleOverride::Root {
            override_: RootModuleOverride::NonRegistry(crate::module_eval::RepoSpec {
                rule_id: crate::module_eval::RepoRuleId {
                    bzl_file: slug_identity_v2::CanonicalLabel::parse(
                        "@@bazel_tools//tools/build_defs/repo:local.bzl",
                    )
                    .unwrap(),
                    rule_name: "local_repository".into(),
                },
                attributes: Arc::new(SmallMap::from_iter([(
                    CompactString::new("path"),
                    crate::module_eval::OverrideAttributeValue::String("local".into()),
                )])),
            }),
        },
    )]);
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut transaction = dice.updater().commit().await;
    let mut observations = PathObservationEpoch::empty();
    let transformed = transform_request(
        &mut transaction,
        &workspace,
        HostSelectedModuleGraphMode::Legacy,
        Some("bazel_tools"),
        &mut cache,
        &mut observations,
        "local".into(),
        version("9"),
    )
    .await
    .unwrap();
    let empty = HostGraphModuleKey::module("local".into(), BazelModuleVersion::empty());
    assert_eq!(transformed, empty);
    let mut dep = dependency("local", key("local", "9"));
    dep.transformed = transformed;
    let mut entries = vec![
        module(HostGraphModuleKey::Root, vec![], vec![dep]),
        module(empty.clone(), vec![], vec![]),
    ];
    let keys = entries.iter().map(|entry| entry.key.clone()).collect();
    prune_unfulfilled_nodep_edges(&mut entries, &keys);
    assert_eq!(entries[0].nodep_dependencies.len(), 1);
    assert_eq!(entries[0].nodep_dependencies[0].transformed, empty);
    assert!(observations.observations().is_empty());
}
