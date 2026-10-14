use super::*;

fn identity(name: &str) -> BzlModuleIdentity {
    BzlModuleIdentity {
        label: CanonicalLabel::parse(&format!("@@rules_cc+//cc/private:{name}.bzl")).unwrap(),
        workspace_path: format!("/fixtures/{name}.bzl").into(),
        repository_mapping: Arc::from([]),
    }
}

fn evaluated_child(source: &str) -> FrozenBzlModule {
    let identity = identity("child");
    let ast = AstModule::parse(
        "same-diagnostic-name.bzl",
        source.to_owned(),
        &Dialect::Bazel,
    )
    .unwrap();
    let module = Module::new();
    let prepared = prepare(&module, &ast, identity.clone(), &[]).unwrap();
    let mut evaluator = Evaluator::new(&module);
    evaluator.extra = Some(prepared.context());
    evaluator.eval_module(ast, &loading_globals()).unwrap();
    drop(evaluator);
    FrozenBzlModule {
        semantic_inputs: None,
        module: module.freeze().unwrap(),
        path: identity.workspace_path,
        loads: vec![],
        manifest: prepared.manifest().clone(),
        bzl_load_visibility: BzlLoadVisibility::default(),
        retained_bzl_modules: Arc::from([]),
    }
}

#[test]
fn actual_ast_bytes_and_explicit_identity_form_the_test_certificate() {
    let first = evaluated_child("VALUE = 1\n");
    let changed = evaluated_child("VALUE = 2\n");
    let a = child_manifest(&first.module).unwrap();
    let b = child_manifest(&changed.module).unwrap();
    assert_eq!(a.root, b.root);
    assert_eq!(a.reachable[0].source_digest, digest("VALUE = 1\n"));
    assert_eq!(b.reachable[0].source_digest, digest("VALUE = 2\n"));
    assert_ne!(a, b, "same filename cannot certify different source bytes");
    let unprepared = Module::new().freeze().unwrap();
    assert!(child_manifest(&unprepared).is_err());
}

#[test]
fn actual_child_union_matches_production_and_conflicts_do_not_attach() {
    let child = evaluated_child("VALUE = 1\n");
    let ast = AstModule::parse(
        "parent.bzl",
        "load(':child.bzl', 'VALUE')\n".to_owned(),
        &Dialect::Bazel,
    )
    .unwrap();
    let module = Module::new();
    let child_manifest = child_manifest(&child.module).unwrap();
    let prepared = prepare(
        &module,
        &ast,
        identity("parent"),
        &[child_manifest, child_manifest],
    )
    .unwrap();
    let expected = BzlLoadManifest::new(
        identity("parent"),
        digest("load(':child.bzl', 'VALUE')\n"),
        [&child, &child],
    );
    assert_eq!(prepared.manifest(), &expected);
    assert_eq!(prepared.sources().len(), 2);
    assert!(prepare(&module, &ast, identity("other"), &[]).is_err());

    let changed = evaluated_child("VALUE = 2\n");
    let parent = Module::new();
    assert!(
        prepare(
            &parent,
            &ast,
            identity("parent"),
            &[
                child_manifest,
                super::child_manifest(&changed.module).unwrap()
            ],
        )
        .is_err()
    );
    assert!(parent.client_data().is_none());
}
