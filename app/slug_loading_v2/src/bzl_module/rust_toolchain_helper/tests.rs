use dice::DetectCycles;
use dice::Dice;
use starlark::environment::Globals;

use super::*;

fn identity(label: &str) -> BzlModuleIdentity {
    let label = CanonicalLabel::parse(label).unwrap();
    BzlModuleIdentity {
        workspace_path: PathBuf::from(format!(
            "/selected/rules_rust/{}/{}",
            label.package().package(),
            label.target()
        )),
        label,
        repository_mapping: Arc::from([(
            ApparentRepoName::parse("@rules_rust").unwrap(),
            CanonicalRepoName::parse("@@rules_rust+").unwrap(),
        )]),
    }
}

fn module(
    identity: BzlModuleIdentity,
    digest: [u8; 32],
    source: &str,
    children: &[FrozenBzlModule],
) -> FrozenBzlModule {
    let module = Module::new();
    if let Some(child) = children.first() {
        let function = child.module.get("leaf").unwrap();
        module.set("_child", function.owned_value(module.frozen_heap()));
    }
    Evaluator::new(&module)
        .eval_module(
            AstModule::parse(
                identity.workspace_path.to_str().unwrap(),
                source.to_owned(),
                &Dialect::Bazel,
            )
            .unwrap(),
            &Globals::standard(),
        )
        .unwrap();
    FrozenBzlModule {
        module: module.freeze().unwrap(),
        path: identity.workspace_path.clone(),
        loads: Vec::new(),
        manifest: BzlLoadManifest::new(identity, digest, children),
        bzl_load_visibility: BzlLoadVisibility::Public,
        retained_bzl_modules: children
            .iter()
            .map(|child| FrozenBzlLifetimeEntry {
                identity: child.manifest.root.clone(),
                module: child.module.dupe(),
            })
            .collect::<Vec<_>>()
            .into(),
    }
}

fn fixture() -> (BzlLoadManifest, FrozenBzlModule, RustToolchainHelperRequest) {
    // Synthetic code exercises the private metadata/lifetime seam only. The
    // public preparation path obtains digests from the actual DICE loader.
    let leaf = module(
        identity("@@rules_rust+//rust/private:leaf.bzl"),
        [7; 32],
        "_private = ['leaf']\ndef leaf(x):\n    return [x] + _private\n",
        &[],
    );
    let helper = module(
        identity(HELPER_LABEL),
        HELPER_SHA,
        "def make_libstd_and_allocator_ccinfo(x):\n    return _child(x)\n",
        &[leaf],
    );
    let caller = BzlLoadManifest::new(identity(CALLER_LABEL), CALLER_SHA, [&helper]);
    let request = RustToolchainHelperRequest::Producer(root_source(&caller).unwrap().clone());
    (caller, helper, request)
}

#[tokio::test]
async fn producer_rejects_wrong_canonical_source_and_digest_before_loading() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut ctx = dice.updater().commit().await;
    let (caller, _, _) = fixture();
    let source = root_source(&caller).unwrap();
    for mode in [
        RustToolchainHelperMode::Legacy,
        RustToolchainHelperMode::Observed,
    ] {
        for field in 0..2 {
            let mut wrong = source.clone();
            if field == 0 {
                wrong.source_digest[0] ^= 1;
            } else {
                wrong.identity.label =
                    CanonicalLabel::parse("@@other+//rust/private:toolchain.bzl").unwrap();
            }
            let outcome = prepare_rust_toolchain_helper(
                &mut ctx,
                NormalizedAbsolutePath::new("/workspace").unwrap(),
                RustToolchainHelperRequest::Producer(wrong),
                mode,
            )
            .await;
            assert!(
                matches!(outcome, SourcePreparationOutcome::Complete(Ok((Err(RustToolchainHelperError::Authentication(_)), epoch))) if epoch == PathObservationEpoch::empty())
            );
        }
    }
}

#[test]
fn complete_authority_tracks_caller_import_order_mapping_and_source_restoration() {
    let (caller, helper, request) = fixture();
    let baseline = finish_binding(&request, &caller, helper.clone()).unwrap();
    let retained = RustToolchainHelperRequest::Retained(baseline.authority().dupe());
    assert_eq!(
        finish_binding(&retained, &caller, helper.clone())
            .unwrap()
            .authority(),
        baseline.authority()
    );
    for field in 0..6 {
        let mut changed = helper.clone();
        match field {
            0 => changed.manifest.fingerprint[0] ^= 1,
            1 => Arc::make_mut(&mut changed.manifest.reachable)[1].source_digest[0] ^= 1,
            2 => changed.manifest.direct_children = Arc::from([]),
            3 => {
                let mut children = changed.manifest.direct_children.to_vec();
                children.push(identity("@@rules_rust+//rust/private:extra.bzl"));
                children.reverse();
                changed.manifest.direct_children = children.into();
            }
            4 => {
                Arc::make_mut(&mut changed.manifest.reachable)[1]
                    .identity
                    .repository_mapping = Arc::from([])
            }
            5 => {
                Arc::make_mut(&mut changed.manifest.reachable)[1]
                    .identity
                    .workspace_path = "/different/leaf.bzl".into()
            }
            _ => unreachable!(),
        }
        assert!(matches!(
            finish_binding(&retained, &caller, changed),
            Err(RustToolchainHelperError::Authentication(_))
        ));
        assert_eq!(
            finish_binding(&retained, &caller, helper.clone())
                .unwrap()
                .authority(),
            baseline.authority()
        );
    }
    for field in 0..3 {
        let mut changed = caller.clone();
        match field {
            0 => Arc::make_mut(&mut changed.reachable)[0].source_digest[0] ^= 1,
            1 => {
                changed.root.workspace_path = "/forged/toolchain.bzl".into();
                Arc::make_mut(&mut changed.reachable)[0].identity = changed.root.clone();
            }
            2 => {
                changed.root.repository_mapping = Arc::from([]);
                Arc::make_mut(&mut changed.reachable)[0].identity = changed.root.clone();
            }
            _ => unreachable!(),
        }
        assert!(matches!(
            finish_binding(&request, &changed, helper.clone()),
            Err(RustToolchainHelperError::Authentication(_))
        ));
    }
    let mut changed = caller.clone();
    changed.direct_children = Arc::from([]);
    assert!(matches!(
        finish_binding(&request, &changed, helper),
        Err(RustToolchainHelperError::Authentication(_))
    ));
}

#[test]
fn source_provenance_deduplicates_equal_rows_and_refuses_collisions() {
    let (caller, helper, request) = fixture();
    let baseline = finish_binding(&request, &caller, helper.clone()).unwrap();
    let mut same = helper.clone();
    let mut reachable = same.manifest.reachable.to_vec();
    reachable.push(reachable[1].clone());
    same.manifest.reachable = reachable.into();
    assert_eq!(
        finish_binding(&request, &caller, same)
            .unwrap()
            .source_identities(),
        baseline.source_identities()
    );
    for collision in [0, 1] {
        let mut changed = helper.clone();
        let mut reachable = changed.manifest.reachable.to_vec();
        let mut duplicate = reachable[1].clone();
        if collision == 0 {
            duplicate.source_digest[0] ^= 1;
        } else {
            duplicate.identity.label =
                CanonicalLabel::parse("@@rules_rust+//rust/private:alias.bzl").unwrap();
        }
        reachable.push(duplicate);
        changed.manifest.reachable = reachable.into();
        assert!(matches!(
            finish_binding(&request, &caller, changed),
            Err(RustToolchainHelperError::Provenance(_))
        ));
    }
    assert_eq!(baseline.source_identities().len(), 3);
    assert_eq!(
        baseline.source_identities()[0].1,
        *root_source(&caller).unwrap()
    );
}

#[test]
fn installed_code_keeps_private_transitive_heap_after_binding_and_producer_disposal() {
    let (caller, helper, request) = fixture();
    let binding = finish_binding(&request, &caller, helper).unwrap();
    let consumer = Module::new();
    consumer.set("invoke", binding.install(consumer.frozen_heap()).to_value());
    drop(binding);
    drop(request);
    drop(caller);
    let frozen = consumer.freeze().unwrap();
    let function = frozen.get("invoke").unwrap();
    drop(frozen);
    let consumer = Module::new();
    let function = function.owned_value(consumer.frozen_heap());
    let mut eval = Evaluator::new(&consumer);
    assert_eq!(
        eval.eval_function(function, &[consumer.heap().alloc("consumer")], &[])
            .unwrap()
            .to_repr(),
        "[\"consumer\", \"leaf\"]"
    );
    assert!(eval.eval_function(function, &[], &[]).is_err());
}
