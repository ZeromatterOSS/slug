// Genuine Source original execution, borrowed current sink and shared grants.
use super::*;
const ORIGINAL: &str = r#"def make(bad):
    captured = ['captured']
    def callback(status):
        print('native-original-once')
        if bad:
            fail('native-original-body-failure')
        return {'{A}': captured[0], '{B}': 'tail'}
    return callback
def _impl(ctx):
    template = ctx.file.template
    if ctx.attr.derived:
        template = ctx.actions.declare_file('derived.template')
        ctx.actions.write(output=template, content='{A}|{B}')
    out = ctx.actions.transform_info_file(transform_func=make(ctx.attr.bad),template=template,output_file_name='custom.h')
    return [DefaultInfo(files=depset([out]))]
translator = rule(implementation=_impl,attrs={'bad':attr.bool(),'derived':attr.bool(),'template':attr.label(allow_single_file=True,default=Label('//:independent.template'))})
"#;
fn workspace() -> tempfile::TempDir {
    let root = status_workspace();
    fs::create_dir_all(root.path().join("test")).unwrap();
    fs::write(
        root.path().join("test/BUILD.bazel"),
        "exports_files(['custom.bzl'])\n",
    )
    .unwrap();
    fs::write(root.path().join("test/custom.bzl"), ORIGINAL).unwrap();
    fs::write(root.path().join("independent.template"), "{A}|{B}").unwrap();
    let build = root.path().join("BUILD.bazel");
    let old = fs::read_to_string(&build).unwrap();
    fs::write(build,format!("{old}\nload('//test:custom.bzl','translator')\nexports_files(['independent.template'])\ntranslator(name='custom')\ntranslator(name='bad',bad=True)\ntranslator(name='derived',derived=True)\n")).unwrap();
    root
}
fn key(runtime: &WorkspaceRuntime, name: &str) -> BuildInfoNativeInputsKey {
    let targets = [TargetPattern::parse(&format!("//:{name}")).unwrap()];
    let registries = [format!(
        "file://{}/empty-registry",
        runtime.workspace.display()
    )];
    let request = native_request(runtime);
    let accepted = runtime
        .build_command_with_repository_environment(
            &targets,
            request.command_policy,
            request.environment_policy,
            request.lockfile_mode,
            &registries,
            request.repository_environment,
            CommandConfigurationOverlay::default(),
        )
        .unwrap();
    let evaluation = accepted.terminal_for_test().as_ref().as_ref().unwrap();
    let owner = build_info_owner(evaluation);
    let target = owner.configured_target_key().unwrap().clone();
    let ordinal = owner
        .actions()
        .iter()
        .position(|action| action.build_info_transform_spec().is_some())
        .unwrap();
    let request = native_request(runtime);
    let (build, _) = runtime
        .prepare_build_request(
            &targets,
            request.command_policy,
            request.environment_policy,
            request.lockfile_mode,
            &registries,
            request.repository_environment,
            CommandConfigurationOverlay::default(),
        )
        .unwrap();
    BuildInfoNativeInputsKey(BuildInfoTemplatePreparationKey::new(build, target, ordinal))
}
#[derive(Clone)]
struct CloneProbe {
    root: NativeBuildInfoHeaderPublicationRoot,
    first_error: Option<Arc<Mutex<Option<String>>>>,
}
#[derive(Clone)]
struct CloneTerminal {
    actual: NativeBuildInfoHeaderPublicationTerminal,
    early: NativeBuildInfoHeaderPublicationTerminal,
}
#[async_trait]
impl NativeCommandRoot for CloneProbe {
    type Terminal = CloneTerminal;
    fn requires_workspace_status_inputs(&self) -> bool {
        true
    }
    fn initializes_request_revision(&self) -> bool {
        true
    }
    fn observations<'a>(&self, value: &'a Self::Terminal) -> Option<&'a PathObservationEpoch> {
        self.root.observations(&value.actual)
    }
    fn source_certificate<'a>(&self, value: &'a Self::Terminal) -> Option<&'a SourceCertificate> {
        self.root.source_certificate(&value.actual)
    }
    fn observed_selection_association(&self) -> ObservedSelectionAssociation {
        self.root.observed_selection_association()
    }
    fn event_reconciliation_policy(&self, value: &Self::Terminal) -> EventReconciliationPolicy {
        self.root.event_reconciliation_policy(&value.actual)
    }
    async fn compute(
        &self,
        tx: &mut dice::DiceTransaction,
    ) -> Result<PreparationOutcome<Self::Terminal>, NativeDemandSessionError> {
        Ok(self.root.compute(tx).await?.map(|actual| CloneTerminal {
            early: actual.clone(),
            actual,
        }))
    }
    fn complete<'a>(
        &'a self,
        value: &'a mut Self::Terminal,
        context: NativeCommandCompletionContext<'a, '_>,
    ) -> impl std::future::Future<Output = Result<(), NativeDemandSessionError>> + 'a {
        async move {
            let second = NativeCommandCompletionContext {
                command: context.command,
                transaction: context.transaction,
                selected_repository_requests: context.selected_repository_requests,
                selected_repository_results: context.selected_repository_results,
                callback_output: context.callback_output,
            };
            let first = self.root.complete(&mut value.actual, context).await;
            if let Some(observed) = &self.first_error {
                let error = first.expect_err("current cancellation must refuse actual completion");
                let message = error.to_string();
                assert!(
                    message.contains("build-info evaluation cancelled"),
                    "{message}"
                );
                *observed.lock().unwrap() = Some(message);
                assert!(value.actual.published.is_none());
                assert!(value.early.published.is_none());
                // Return the actual completion error before driver abort handling
                // can replace it with a poisoned source-service refusal.
                return Err(error);
            }
            first?;
            let error = self
                .root
                .complete(&mut value.early, second)
                .await
                .err()
                .expect("pre-complete clone must not mint another invocation grant");
            assert!(value.actual.published.is_none());
            assert!(value.early.published.is_none());
            Err(error)
        }
    }
}
#[test]
fn original_callback_metadata_zero_native_once_and_precomplete_clone_cannot_rearm() {
    let workspace = workspace();
    let runtime =
        WorkspaceRuntime::new(workspace.path(), crate::runtime::ProcessHostOwner::native())
            .unwrap();
    let key = key(&runtime, "custom");
    let calls = Mutex::new(Vec::new());
    let sink = |text: &str| {
        calls.lock().unwrap().push(text.to_owned());
        Ok(())
    };
    // Real build and native metadata must not execute the original body.
    runtime
        .drive_command(native_request(&runtime), key.clone())
        .unwrap();
    assert!(calls.lock().unwrap().is_empty());
    let root = NativeBuildInfoHeaderPublicationRoot::new(key.clone());
    let result = runtime
        .drive_command_with_callback_output(native_request(&runtime), root, &sink)
        .unwrap();
    let result = result
        .accepted
        .terminal_for_test()
        .published
        .as_ref()
        .unwrap();
    assert_eq!(result.consumed.header, b"captured|tail");
    let path = header_path(&result.consumed.inputs);
    assert_eq!(fs::read(&path).unwrap(), b"captured|tail");
    assert_eq!(*calls.lock().unwrap(), vec!["native-original-once"]);
    let held_source = result
        .consumed
        .inputs
        .template
        .source()
        .result()
        .as_ref()
        .unwrap();
    assert_eq!(held_source.bytes(), b"{A}|{B}");
    assert!(
        held_source.catalog_owner().is_none(),
        "ordinary Source is not a catalog fixture"
    );
    for (template, expected) in [
        (b"{B}::{A}".as_slice(), b"tail::captured".as_slice()),
        (b"{A}|{B}".as_slice(), b"captured|tail".as_slice()),
    ] {
        fs::write(workspace.path().join("independent.template"), template).unwrap();
        calls.lock().unwrap().clear();
        let current = runtime
            .drive_command_with_callback_output(
                native_request(&runtime),
                NativeBuildInfoHeaderPublicationRoot::new(key.clone()),
                &sink,
            )
            .unwrap();
        let current = current
            .accepted
            .terminal_for_test()
            .published
            .as_ref()
            .unwrap();
        assert_eq!(current.consumed.header, expected);
        assert_eq!(
            fs::read(header_path(&current.consumed.inputs)).unwrap(),
            expected
        );
        assert_eq!(
            current
                .consumed
                .inputs
                .template
                .source()
                .result()
                .as_ref()
                .unwrap()
                .bytes(),
            template
        );
        assert_eq!(*calls.lock().unwrap(), vec!["native-original-once"]);
        assert_eq!(result.consumed.header, b"captured|tail");
        assert_eq!(
            held_source.bytes(),
            b"{A}|{B}",
            "prior immutable Source owner survives A/B/A"
        );
    }
    calls.lock().unwrap().clear();
    let root = CloneProbe {
        root: NativeBuildInfoHeaderPublicationRoot::new(key),
        first_error: None,
    };
    let error = runtime
        .drive_command_with_callback_output(native_request(&runtime), root, &sink)
        .err()
        .unwrap();
    assert!(
        error.to_string().contains("already prepared") || error.to_string().contains("spent"),
        "{error}"
    );
    assert_eq!(*calls.lock().unwrap(), vec!["native-original-once"]);
    assert_eq!(fs::read(path).unwrap(), b"captured|tail");
}
#[test]
fn original_callback_failure_missing_sink_and_derived_refusal_do_not_publish() {
    let workspace = workspace();
    let runtime =
        WorkspaceRuntime::new(workspace.path(), crate::runtime::ProcessHostOwner::native())
            .unwrap();
    let calls = Mutex::new(Vec::new());
    let sink = |text: &str| {
        calls.lock().unwrap().push(text.to_owned());
        Ok(())
    };
    let good = key(&runtime, "custom");
    let missing = runtime
        .drive_command(
            native_request(&runtime),
            NativeBuildInfoHeaderPublicationRoot::new(good.clone()),
        )
        .err()
        .unwrap();
    assert!(
        missing
            .to_string()
            .contains("requires a current output sink"),
        "{missing}"
    );
    assert!(calls.lock().unwrap().is_empty());
    let bad = key(&runtime, "bad");
    // This original always fails when invoked: successful NativeInputs metadata
    // proves non-invocation independently of a newly installed print sink.
    runtime
        .drive_command(native_request(&runtime), bad.clone())
        .unwrap();
    let error = runtime
        .drive_command_with_callback_output(
            native_request(&runtime),
            NativeBuildInfoHeaderPublicationRoot::new(bad),
            &sink,
        )
        .err()
        .unwrap();
    assert!(
        error.to_string().contains("native-original-body-failure"),
        "{error}"
    );
    assert!(error.to_string().contains("TranslateBuildInfo"), "{error}");
    assert_eq!(*calls.lock().unwrap(), vec!["native-original-once"]);
    calls.lock().unwrap().clear();
    let failing = |text: &str| {
        calls.lock().unwrap().push(text.to_owned());
        Err(slug_build_api_v2::BuildInfoTransformError::Callback(
            "current-print-refusal".into(),
        ))
    };
    let error = runtime
        .drive_command_with_callback_output(
            native_request(&runtime),
            NativeBuildInfoHeaderPublicationRoot::new(good),
            &failing,
        )
        .err()
        .unwrap();
    assert!(
        error.to_string().contains("current-print-refusal"),
        "{error}"
    );
    assert_eq!(*calls.lock().unwrap(), vec!["native-original-once"]);
    calls.lock().unwrap().clear();
    let derived = key(&runtime, "derived");
    let error = runtime
        .drive_command_with_callback_output(
            native_request(&runtime),
            NativeBuildInfoHeaderPublicationRoot::new(derived),
            &sink,
        )
        .err()
        .unwrap();
    assert!(
        error.to_string().contains("verified producer output"),
        "{error}"
    );
    assert!(calls.lock().unwrap().is_empty());
    assert_eq!(
        final_bytes(workspace.path()),
        [None, None],
        "all failed attempts leave no published status bytes"
    );
}
#[test]
fn original_callback_physical_validation_failure_never_renders_again() {
    let workspace = workspace();
    let runtime =
        WorkspaceRuntime::new(workspace.path(), crate::runtime::ProcessHostOwner::native())
            .unwrap();
    let key = key(&runtime, "custom");
    let calls = Mutex::new(Vec::new());
    let sink = |text: &str| {
        calls.lock().unwrap().push(text.to_owned());
        Ok(())
    };
    let metadata = runtime
        .drive_command(native_request(&runtime), key.clone())
        .unwrap();
    let output = header_path(metadata.accepted.terminal_for_test());
    assert!(optional_header(&output).is_none());
    let probe = FileProbe::new(Arc::from([key]), 0, FileMode::Mutate(0), false);
    let error = runtime
        .drive_command_with_callback_output(native_request(&runtime), probe.clone(), &sink)
        .err()
        .unwrap();
    assert!(
        matches!(
            error,
            NativeDemandSessionError::Revision(RequestRevisionError::OutputPublication(_))
        ),
        "{error}"
    );
    assert_eq!(*calls.lock().unwrap(), vec!["native-original-once"]);
    assert!(optional_header(&output).is_none());
    assert_eq!(
        probe.issued.lock().unwrap().len(),
        1,
        "mutation follows actual status publication"
    );
}

#[test]
fn original_callback_uses_current_scope_cancellation_and_fresh_scope_recovers() {
    use crate::runtime::dice::source_service::SourceServiceLimits;
    use crate::runtime::dice::source_service::SourceServiceScope;
    use crate::runtime::dice::source_service::WorkspaceSourceServices;

    let workspace = workspace();
    // Keys describe immutable requests, not issuance, grants or prepared state.
    // Prepare this pure key/path before either scoped consumer exists so each
    // finite SourceAttempts ledger covers exactly ONE actual native command.
    let preparation =
        WorkspaceRuntime::new(workspace.path(), crate::runtime::ProcessHostOwner::native())
            .unwrap();
    let original = key(&preparation, "custom");
    let metadata = preparation
        .drive_command(native_request(&preparation), original.clone())
        .unwrap();
    let output = header_path(metadata.accepted.terminal_for_test());
    assert!(optional_header(&output).is_none());
    drop(metadata);
    drop(preparation);
    let scoped = || {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(26);
        let scope = SourceServiceScope::new_for_test(
            slug_workspace_v2::NormalizedAbsolutePath::new(
                fs::canonicalize(workspace.path()).unwrap(),
            )
            .unwrap(),
            deadline,
            deadline + std::time::Duration::from_secs(3),
            SourceServiceLimits::default(),
        )
        .unwrap();
        let runtime = WorkspaceRuntime::new_with_source_services(
            workspace.path(),
            crate::runtime::ProcessHostOwner::native(),
            WorkspaceSourceServices::Diagnostic(scope.clone()),
        )
        .unwrap();
        assert!(Arc::ptr_eq(runtime.source_scope.as_ref().unwrap(), &scope));
        (runtime, scope)
    };
    let (runtime, scope) = scoped();
    assert!(optional_header(&output).is_none());
    assert!(!scope.is_cancelled());
    let calls = Mutex::new(Vec::new());
    let observed = Arc::new(Mutex::new(None));
    let sink = |text: &str| {
        assert!(!scope.is_cancelled());
        calls.lock().unwrap().push(text.to_owned());
        scope.cancel();
        Ok(())
    };
    let root = CloneProbe {
        root: NativeBuildInfoHeaderPublicationRoot::new(original.clone()),
        first_error: Some(observed.clone()),
    };
    let _abort_error = runtime
        .drive_command_with_callback_output(native_request(&runtime), root, &sink)
        .err()
        .expect("cancelled original must not publish");
    assert!(
        observed.lock().unwrap().is_some(),
        "actual completion was not intercepted: driver={_abort_error:?}; calls={:?}; scope_cancelled={}",
        *calls.lock().unwrap(),
        scope.is_cancelled(),
    );
    assert!(
        observed
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .contains("build-info evaluation cancelled"),
        "the actual original completion, before abort handling, observes current scope"
    );
    assert!(scope.is_cancelled());
    assert_eq!(*calls.lock().unwrap(), vec!["native-original-once"]);
    assert!(optional_header(&output).is_none());
    drop(runtime);

    // The retained callback must borrow this attempt's healthy scope and sink;
    // cancellation from a previous runtime is not retained semantic state.
    let (healthy, fresh_scope) = scoped();
    assert!(!Arc::ptr_eq(&scope, &fresh_scope));
    assert!(!fresh_scope.is_cancelled());
    calls.lock().unwrap().clear();
    let sink = |text: &str| {
        assert!(!fresh_scope.is_cancelled());
        calls.lock().unwrap().push(text.to_owned());
        Ok(())
    };
    let result = healthy
        .drive_command_with_callback_output(
            native_request(&healthy),
            NativeBuildInfoHeaderPublicationRoot::new(original),
            &sink,
        )
        .unwrap();
    let published = result
        .accepted
        .terminal_for_test()
        .published
        .as_ref()
        .unwrap();
    assert_eq!(published.consumed.header, b"captured|tail");
    assert_eq!(
        fs::read(header_path(&published.consumed.inputs)).unwrap(),
        b"captured|tail"
    );
    assert_eq!(*calls.lock().unwrap(), vec!["native-original-once"]);
    assert!(!fresh_scope.is_cancelled());
}
