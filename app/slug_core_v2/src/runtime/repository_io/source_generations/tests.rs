use slug_bzlmod_v2::RepoRuleId;
use slug_identity_v2::CanonicalLabel;
use starlark_map::small_map::SmallMap;

use super::*;

struct Fixture {
    materializer: RepositoryMaterializer,
    token: RepositorySessionToken,
    request: Arc<RepositoryMaterializationRequest>,
    root: PathBuf,
    path: NormalizedAbsolutePath,
    instance: PathObservationInstanceId,
}

fn request(
    workspace: &NormalizedAbsolutePath,
    salt: &str,
) -> Arc<RepositoryMaterializationRequest> {
    Arc::new(RepositoryMaterializationRequest {
        id: RepositoryMaterializationRequestId {
            workspace: workspace.clone(),
            canonical_repo: CanonicalRepoName::new("dep").unwrap(),
        },
        repo_spec: RepoSpec {
            rule_id: RepoRuleId {
                bzl_file: CanonicalLabel::parse("@@bazel_tools//tools/build_defs/repo:git.bzl")
                    .unwrap(),
                rule_name: "git_repository".into(),
            },
            attributes: Arc::new(SmallMap::from_iter([(
                compact_str::CompactString::new("commit"),
                OverrideAttributeValue::String(salt.into()),
            )])),
        },
        kind: RepositoryMaterializationKind::Immutable,
    })
}

fn materialize(
    materializer: &RepositoryMaterializer,
    token: RepositorySessionToken,
    request: Arc<RepositoryMaterializationRequest>,
    bytes: &[u8],
) {
    materializer
        .materialize_with(
            token,
            request,
            RepositoryMaterializationGeneration(1),
            || {
                let root = tempfile::tempdir().unwrap();
                std::fs::write(root.path().join("input"), bytes).unwrap();
                RepositoryMaterializationAttempt::Immutable {
                    bytes: bytes.to_vec(),
                    root,
                }
            },
        )
        .unwrap();
}

impl Fixture {
    fn new() -> Self {
        let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
        let materializer = RepositoryMaterializer::new(workspace.clone());
        let token = materializer.begin().unwrap();
        materializer
            .validate(token, &mut |_, _| panic!("no prior roots"))
            .unwrap();
        let request = request(&workspace, "a");
        materialize(&materializer, token, request.clone(), b"a");
        let RepositoryMaterializationResult::Success(RepositoryMaterializationSuccess::Immutable {
            generation_root: root,
            observation_instance: instance,
            ..
        }) = materializer.active_result_for_test(token, "dep")
        else {
            panic!("immutable result")
        };
        let path = NormalizedAbsolutePath::new(root.join("input")).unwrap();
        Self {
            materializer,
            token,
            request,
            root,
            path,
            instance,
        }
    }

    fn source(&self) -> SourceGenerationInput<'_> {
        SourceGenerationInput {
            repo: &self.request.id.canonical_repo,
            namespace: PathObservationNamespace::Materialization(self.instance),
            requested_path: &self.path,
        }
    }

    fn epoch(&self) -> RepositoryMaterializationResultEpoch {
        self.materializer
            .selected_epoch(self.token, std::slice::from_ref(&self.request))
            .unwrap()
    }

    fn retain(
        &self,
        sources: &[SourceGenerationInput<'_>],
    ) -> Result<NativeSourceGenerations, Arc<str>> {
        self.materializer.retain_source_generation_inputs(
            self.token,
            std::slice::from_ref(&self.request),
            &self.epoch(),
            sources,
        )
    }
}

#[test]
fn selected_native_leases_deduplicate_and_outlive_acceptance_discard_and_owner() {
    for accept in [false, true] {
        let fixture = Fixture::new();
        let lease = fixture
            .retain(&[fixture.source(), fixture.source()])
            .unwrap();
        assert_eq!(lease._roots.len(), 1);
        let clone = lease.clone();
        assert!(Arc::ptr_eq(&lease._roots, &clone._roots));
        if accept {
            fixture
                .materializer
                .accept(
                    fixture.token,
                    std::slice::from_ref(&fixture.request),
                    Vec::new(),
                )
                .unwrap();
        } else {
            fixture.materializer.discard(fixture.token).unwrap();
        }
        let path = fixture.path.clone();
        let root = fixture.root.clone();
        drop(fixture);
        assert_eq!(std::fs::read(path.as_path()).unwrap(), b"a");
        drop(lease);
        assert!(root.exists());
        drop(clone);
        assert!(!root.exists());
    }
    let fixture = Fixture::new();
    let root = fixture.root.clone();
    let lease = fixture.retain(&[fixture.source()]).unwrap();
    drop(lease);
    fixture.materializer.discard(fixture.token).unwrap();
    assert!(
        !root.exists(),
        "a rejected attempt retains no generation lease"
    );
    assert!(NativeSourceGenerations::default()._roots.is_empty());
}

#[test]
fn source_leases_reject_unselected_forged_stale_and_escaping_provenance() {
    let fixture = Fixture::new();
    let sources = [fixture.source()];
    let selected = std::slice::from_ref(&fixture.request);
    let epoch = fixture.epoch();
    let retain = |token,
                  selected: &[Arc<RepositoryMaterializationRequest>],
                  epoch: &RepositoryMaterializationResultEpoch,
                  sources: &[SourceGenerationInput<'_>]| {
        fixture
            .materializer
            .retain_source_generation_inputs(token, selected, epoch, sources)
            .unwrap_err()
    };
    assert!(
        retain(
            RepositorySessionToken(fixture.token.0 + 1),
            selected,
            &epoch,
            &sources
        )
        .contains("StaleToken")
    );
    let empty = complete_epoch(&fixture.materializer.workspace, &[]).unwrap();
    assert!(retain(fixture.token, &[], &empty, &sources).contains("not selected"));
    assert!(
        retain(
            fixture.token,
            &[fixture.request.clone(), fixture.request.clone()],
            &epoch,
            &sources
        )
        .contains("duplicate selected")
    );
    let wrong = request(&fixture.materializer.workspace, "wrong");
    assert!(retain(fixture.token, &[wrong], &epoch, &sources).contains("selected request differs"));
    let mut forged = fixture
        .materializer
        .state
        .lock()
        .unwrap()
        .active
        .as_ref()
        .unwrap()
        .entries[0]
        .clone();
    let RepositoryMaterializationResult::Success(RepositoryMaterializationSuccess::Immutable {
        source_identity,
        ..
    }) = &mut forged.result
    else {
        unreachable!()
    };
    *source_identity = "forged".into();
    let forged_epoch = complete_epoch(&fixture.materializer.workspace, &[forged]).unwrap();
    assert!(
        retain(fixture.token, selected, &forged_epoch, &sources).contains("result epoch differs")
    );
    let wrong_repo = CanonicalRepoName::new("other").unwrap();
    let wrong_source = SourceGenerationInput {
        repo: &wrong_repo,
        ..fixture.source()
    };
    assert!(
        fixture
            .retain(&[wrong_source])
            .unwrap_err()
            .contains("not selected")
    );
    let wrong_instance = SourceGenerationInput {
        namespace: PathObservationNamespace::Materialization(PathObservationInstanceId::new(
            fixture.instance.value() + 1,
        )),
        ..fixture.source()
    };
    assert!(
        fixture
            .retain(&[wrong_instance])
            .unwrap_err()
            .contains("instance differs")
    );
    let sibling = NormalizedAbsolutePath::new(PathBuf::from(format!(
        "{}-sibling/input",
        fixture.root.display()
    )))
    .unwrap();
    let outside = SourceGenerationInput {
        requested_path: &sibling,
        ..fixture.source()
    };
    assert!(
        fixture
            .retain(&[fixture.source(), outside])
            .unwrap_err()
            .contains("outside its native generation"),
        "deduplication must not bypass later path checks"
    );
    let host = SourceGenerationInput {
        namespace: PathObservationNamespace::Host,
        requested_path: &sibling,
        ..fixture.source()
    };
    assert!(fixture.retain(&[host]).unwrap()._roots.is_empty());
}

#[test]
fn historical_and_virtual_generations_do_not_supply_current_native_authority() {
    let fixture = Fixture::new();
    fixture
        .materializer
        .accept(
            fixture.token,
            std::slice::from_ref(&fixture.request),
            Vec::new(),
        )
        .unwrap();
    let token = fixture.materializer.begin().unwrap();
    fixture
        .materializer
        .validate(token, &mut |_, _| panic!("empty validation"))
        .unwrap();
    let changed = request(&fixture.materializer.workspace, "b");
    materialize(&fixture.materializer, token, changed.clone(), b"b");
    let selected = std::slice::from_ref(&changed);
    let epoch = fixture
        .materializer
        .selected_epoch(token, selected)
        .unwrap();
    assert!(fixture.root.exists(), "historical root remains owned");
    assert!(
        fixture
            .materializer
            .retain_source_generation_inputs(token, selected, &epoch, &[fixture.source()])
            .unwrap_err()
            .contains("instance differs")
    );
    fixture.materializer.discard(token).unwrap();

    let fixture = Fixture::new();
    {
        let mut state = fixture.materializer.state.lock().unwrap();
        let RepositoryMaterializationResult::Success(RepositoryMaterializationSuccess::Immutable {
            generation_root,
            ..
        }) = &mut state.active.as_mut().unwrap().entries[0].result
        else {
            unreachable!()
        };
        *generation_root = fixture.root.join("wrong-root");
    }
    assert!(
        fixture
            .retain(&[fixture.source()])
            .unwrap_err()
            .contains("root differs from native owner")
    );
    let fixture = Fixture::new();
    let roots = {
        let mut state = fixture.materializer.state.lock().unwrap();
        std::mem::take(&mut state.active.as_mut().unwrap().provisional_roots)
    };
    assert!(
        fixture
            .retain(&[fixture.source()])
            .unwrap_err()
            .contains("no native generation owner")
    );
    drop(roots);
    assert!(!fixture.root.exists());
}

#[cfg(unix)]
#[test]
fn requested_generation_path_can_retain_an_observed_symlink_to_an_external_target() {
    let fixture = Fixture::new();
    let outside = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(outside.path(), b"outside").unwrap();
    let link = NormalizedAbsolutePath::new(fixture.root.join("link")).unwrap();
    std::os::unix::fs::symlink(outside.path(), link.as_path()).unwrap();
    let source = SourceGenerationInput {
        requested_path: &link,
        ..fixture.source()
    };
    let lease = fixture.retain(&[source]).unwrap();
    assert_eq!(lease._roots.len(), 1);
    assert_eq!(std::fs::read(link.as_path()).unwrap(), b"outside");
    assert!(
        !std::fs::canonicalize(link.as_path())
            .unwrap()
            .starts_with(&fixture.root)
    );
}
