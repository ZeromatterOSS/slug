use super::*;

#[derive(Clone, Copy)]
pub(super) enum CommandChoice {
    Modules,
    PublicProbe,
}
pub(super) enum ObservationOutcome {
    Modules(Result<DrivenCommand<ModuleTerminal>, NativeDemandSessionError>),
    Probe(ProbeOutcome),
}
pub(super) enum ProbeOutcome {
    PrepareRefusal(BuildCommandError),
    DispatchRefusal,
    Driven(
        Result<DrivenCommand<SingletonRootSingleBuildCommandTerminal>, NativeDemandSessionError>,
    ),
}
impl ObservationOutcome {
    pub(super) fn is_ok(&self) -> bool {
        matches!(
            self,
            Self::Modules(Ok(_)) | Self::Probe(ProbeOutcome::Driven(Ok(_)))
        )
    }
    pub(super) fn stage(&self) -> &'static str {
        match self {
            Self::Modules(Ok(_)) => "mapping_complete",
            Self::Modules(Err(_)) | Self::Probe(ProbeOutcome::Driven(Err(_))) => "native_refusal",
            Self::Probe(ProbeOutcome::PrepareRefusal(_) | ProbeOutcome::DispatchRefusal) => {
                "prepare_refusal"
            }
            Self::Probe(ProbeOutcome::Driven(Ok(driven))) => {
                if driven.accepted.terminal().result.is_ok() {
                    "build_complete"
                } else {
                    "build_refusal"
                }
            }
        }
    }
    pub(super) fn report_native(&self) {
        let result = match self {
            Self::Modules(Ok(driven)) => {
                assert!(root_bytes(&driven.accepted.terminal().source));
                (
                    "complete",
                    Some((driven.attempts, driven.terminal_root_count)),
                )
            }
            Self::Probe(ProbeOutcome::Driven(Ok(driven))) => (
                "complete",
                Some((driven.attempts, driven.terminal_root_count)),
            ),
            Self::Modules(Err(error)) | Self::Probe(ProbeOutcome::Driven(Err(error))) => {
                (native_kind(error), None)
            }
            _ => ("unavailable", None),
        };
        if let Some((attempts, roots)) = result.1 {
            println!(
                "WP942_NATIVE kind={} attempts={attempts} roots={roots}",
                result.0
            );
        } else {
            println!(
                "WP942_NATIVE kind={} attempts=unavailable roots=unavailable",
                result.0
            );
        }
    }
}
pub(super) struct ProbeReport {
    preparation: &'static str,
    dispatch: &'static str,
    policy_checked: bool,
}
struct Scalar(Option<usize>);
impl std::fmt::Display for Scalar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Some(value) => write!(f, "{value}"),
            None => f.write_str("unavailable"),
        }
    }
}
impl Default for ProbeReport {
    fn default() -> Self {
        Self {
            preparation: "not_started",
            dispatch: "not_started",
            policy_checked: false,
        }
    }
}
impl ProbeReport {
    pub(super) fn report(
        &self,
        stage: &str,
        early_kind: &str,
        outcome: Option<&ObservationOutcome>,
    ) {
        let mut certificate = "unavailable";
        let mut package_cause = None;
        let mut observations = None;
        let mut counts = None;
        let kind = match outcome {
            Some(ObservationOutcome::Probe(ProbeOutcome::PrepareRefusal(error))) => {
                if let BuildCommandErrorKind::Package(cause) = &error.kind {
                    package_cause = Some(cause);
                }
                build_kind(error)
            }
            Some(ObservationOutcome::Probe(ProbeOutcome::DispatchRefusal)) => "dispatch",
            Some(ObservationOutcome::Probe(ProbeOutcome::Driven(Err(error)))) => native_kind(error),
            Some(ObservationOutcome::Probe(ProbeOutcome::Driven(Ok(driven)))) => {
                let terminal = driven.accepted.terminal();
                certificate = if terminal.source_certificate().is_some() {
                    "true"
                } else {
                    "false"
                };
                observations = terminal
                    .observations
                    .as_ref()
                    .map(|epoch| epoch.observations().len());
                match terminal.result.as_ref() {
                    Err(error) => {
                        if let BuildCommandErrorKind::Package(cause) = &error.kind {
                            package_cause = Some(cause);
                        }
                        build_kind(error)
                    }
                    Ok(evaluation) => {
                        let mut analyses = 0;
                        let mut actions = 0;
                        let mut rustc = 0;
                        for analysis in evaluation.analyses() {
                            analyses += 1;
                            actions += analysis.actions().len();
                            rustc += analysis
                                .actions()
                                .iter()
                                .filter(|action| {
                                    action
                                        .spawn_spec()
                                        .is_some_and(|spawn| spawn.mnemonic() == "Rustc")
                                })
                                .count();
                        }
                        counts = Some((analyses, actions, rustc));
                        "complete"
                    }
                }
            }
            _ => early_kind,
        };
        println!(
            "WP958_RESULT stage={stage} kind={kind} preparation={} dispatch={} policy_checked={} certificate={certificate} observations={} analyses={} actions={} rustc={}",
            self.preparation,
            self.dispatch,
            self.policy_checked,
            Scalar(observations),
            Scalar(counts.map(|n| n.0)),
            Scalar(counts.map(|n| n.1)),
            Scalar(counts.map(|n| n.2))
        );
        if let Some(cause) = package_cause {
            println!(
                "WP966_PACKAGE stage={} detail={}",
                cause.diagnostic_stage(),
                cause.diagnostic()
            );
        } else {
            println!("WP966_PACKAGE stage=none detail=unavailable");
        }
    }
}

#[derive(Clone)]
struct GuardedSingleton {
    inner: SingletonRootSingleBuildCommandKey,
    bodies: Arc<Bodies>,
    milestones: Arc<Mutex<Milestones>>,
}
impl GuardedSingleton {
    fn check_need(&self, needs: &SourcePreparationNeeds) -> Result<(), NativeDemandSessionError> {
        self.bodies.admit(needs).map_err(|denied| {
            self.milestones.lock().unwrap().denied = Some(denied);
            refusal()
        })
    }
}
#[async_trait]
impl NativeCommandRoot for GuardedSingleton {
    type Terminal = SingletonRootSingleBuildCommandTerminal;
    fn complete<'a>(
        &'a self,
        terminal: &'a mut Self::Terminal,
        context: NativeCommandCompletionContext<'a, '_>,
    ) -> impl std::future::Future<Output = Result<(), NativeDemandSessionError>> + 'a {
        self.inner.complete(terminal, context)
    }
    fn publish(&self, terminal: &mut Self::Terminal) -> std::io::Result<()> {
        self.inner.publish(terminal)
    }
    fn publication_completed(&self, terminal: &Self::Terminal) -> bool {
        self.inner.publication_completed(terminal)
    }
    fn initializes_request_revision(&self) -> bool {
        self.inner.initializes_request_revision()
    }
    fn requires_workspace_status_inputs(&self) -> bool {
        self.inner.requires_workspace_status_inputs()
    }
    fn source_certificate<'a>(
        &self,
        terminal: &'a Self::Terminal,
    ) -> Option<&'a SourceCertificate> {
        self.inner.source_certificate(terminal)
    }
    fn observations<'a>(&self, terminal: &'a Self::Terminal) -> Option<&'a PathObservationEpoch> {
        self.inner.observations(terminal)
    }
    fn observed_selection_association(&self) -> ObservedSelectionAssociation {
        self.inner.observed_selection_association()
    }
    fn terminal_observed_selection_association(
        &self,
        terminal: &Self::Terminal,
    ) -> ObservedSelectionAssociation {
        self.inner.terminal_observed_selection_association(terminal)
    }
    fn terminal_demand_association(&self, terminal: &Self::Terminal) -> TerminalDemandAssociation {
        self.inner.terminal_demand_association(terminal)
    }
    fn event_reconciliation_policy(&self, terminal: &Self::Terminal) -> EventReconciliationPolicy {
        self.inner.event_reconciliation_policy(terminal)
    }
    fn allows_empty_terminal(&self) -> bool {
        self.inner.allows_empty_terminal()
    }
    fn allows_unavailable_terminal_roots(&self, terminal: &Self::Terminal) -> bool {
        self.inner.allows_unavailable_terminal_roots(terminal)
    }
    async fn compute(
        &self,
        tx: &mut dice::DiceTransaction,
    ) -> Result<SourcePreparationOutcome<Self::Terminal>, NativeDemandSessionError> {
        let outcome = NativeCommandRoot::compute(&self.inner, tx).await?;
        if let SourcePreparationOutcome::Need(needs) = &outcome {
            self.check_need(needs)?;
        }
        Ok(outcome)
    }
}
fn nightly_overlay() -> CommandConfigurationOverlay {
    CommandConfigurationOverlay::from(vec![
        slug_configuration_v2::CommandConfigurationOccurrence::starlark(
            "@rules_rust//rust/toolchain/channel",
            Some("nightly"),
            false,
        ),
    ])
}
fn projection_matches(
    inner: &SingletonRootSingleBuildCommandKey,
    request: &NativeDemandRequestInputBundle,
    bodies: &Bodies,
    registry: &RegistryUrls,
    overlay: &CommandConfigurationOverlay,
) -> bool {
    request.registry_urls == *registry
        && request.lockfile_mode == LockfileMode::Off
        && inner.0.workspace == bodies.roots[0]
        && matches!(inner.0.targets.as_ref(), [pattern] if pattern.as_ref() == "//pkg:probe")
        && bodies.visible_lockfile_metadata
        && bodies.repo_marker_metadata
        && bodies.ignore_file_bytes
        && bodies.rust_package_metadata
        && bodies.nightly_package_metadata
        && bodies.nightly_build_source
        && bodies.nightly_channel_source
        && bodies.probe_package_source
        && bodies.rules_rust_defs_source
        && bodies.rules_rust_defs_import_sources
        && inner.0.configuration_overlay == *overlay
        && request.command_policy.module_overrides().count() == bodies.bindings().count()
        && bodies.bindings().all(|(name, _, root)| {
            request
                .command_policy
                .module_overrides()
                .any(|(actual, path)| actual == name && path == root.as_path())
        })
}
pub(super) fn prepare_and_drive(
    runtime: &WorkspaceRuntime,
    bodies: Arc<Bodies>,
    policy: BzlmodCommandPolicyKey,
    registry: &RegistryUrls,
    milestones: Arc<Mutex<Milestones>>,
    report: &mut ProbeReport,
) -> ProbeOutcome {
    let target = [TargetPattern::parse("//pkg:probe").expect("fixed public singleton")];
    let environment = BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None)
        .expect("absent yanked versions");
    let urls = [registry.as_slice()[0].as_str().to_owned()];
    let overlay = nightly_overlay();
    let prepared = runtime.prepare_build_request(
        &target,
        policy,
        environment,
        LockfileMode::Off,
        &urls,
        slug_bzlmod_v2::RepositoryEnvironmentSnapshot::empty(),
        overlay.clone(),
    );
    let (key, request) = match prepared {
        Err(error) => {
            report.preparation = "refused";
            return ProbeOutcome::PrepareRefusal(error);
        }
        Ok(prepared) => {
            report.preparation = "complete";
            prepared
        }
    };
    if BuildCommandRootObservationKey::new(key.clone()).is_some() {
        report.dispatch = "refused";
        return ProbeOutcome::DispatchRefusal;
    }
    let Some(inner) = SingletonRootSingleBuildCommandKey::new(key) else {
        report.dispatch = "refused";
        return ProbeOutcome::DispatchRefusal;
    };
    report.dispatch = "singleton";
    report.policy_checked = projection_matches(&inner, &request, &bodies, registry, &overlay);
    if !report.policy_checked {
        report.dispatch = "refused";
        return ProbeOutcome::DispatchRefusal;
    }
    milestones.lock().unwrap().registry_policy_checked = true;
    ProbeOutcome::Driven(runtime.drive_command(
        request,
        GuardedSingleton {
            inner,
            bodies,
            milestones,
        },
    ))
}

#[test]
fn guarded_singleton_preserves_dispatch_policy_and_whole_need() {
    let path = |s: &str| NormalizedAbsolutePath::new(s).unwrap();
    let mut bodies = Arc::new(
        Bodies::new([path("/fixture"), path("/rules"), path("/platforms")])
            .with_visible_lockfile_metadata(RegistryChoice::PreparedFile)
            .with_repo_marker_metadata(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
            .with_ignore_file_bytes(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
            .with_rust_package_metadata(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
            .with_nightly_package_metadata(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
            .with_nightly_build_source(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
            .with_nightly_channel_source(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
            .with_probe_package_source(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
            .with_rules_rust_defs_source(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
            .with_rules_rust_defs_import_sources(
                RegistryChoice::PreparedFile,
                CommandChoice::PublicProbe,
            ),
    );
    let root = BuildCommandRootKey::new_with_overlay(
        path("/fixture"),
        &[TargetPattern::parse("//pkg:probe").unwrap()],
        build_test_configuration("probe-dispatch"),
        nightly_overlay(),
    )
    .unwrap();
    assert!(BuildCommandRootObservationKey::new(root.clone()).is_none());
    let inner = SingletonRootSingleBuildCommandKey::new(root).unwrap();
    let registry = RegistryUrls::new(["file:///task/registry"]);
    let mut request = NativeDemandRequestInputBundle::normalized_initial();
    request.command_policy = BzlmodCommandPolicyKey::from_flags_with_module_overrides(
        None,
        false,
        Path::new("/fixture"),
        ["rules_rust=/rules", "platforms=/platforms"],
    )
    .unwrap();
    request.lockfile_mode = LockfileMode::Off;
    request.registry_urls = registry.dupe();
    assert!(projection_matches(
        &inner,
        &request,
        &bodies,
        &registry,
        &nightly_overlay()
    ));
    let closed = Bodies::new([path("/fixture"), path("/rules"), path("/platforms")])
        .with_visible_lockfile_metadata(RegistryChoice::PreparedFile);
    assert!(!projection_matches(
        &inner,
        &request,
        &closed,
        &registry,
        &nightly_overlay()
    ));
    let no_ignore = Bodies::new([path("/fixture"), path("/rules"), path("/platforms")])
        .with_visible_lockfile_metadata(RegistryChoice::PreparedFile)
        .with_repo_marker_metadata(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_rust_package_metadata(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_nightly_package_metadata(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_nightly_build_source(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_nightly_channel_source(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_probe_package_source(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_rules_rust_defs_source(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_rules_rust_defs_import_sources(
            RegistryChoice::PreparedFile,
            CommandChoice::PublicProbe,
        );
    assert!(!projection_matches(
        &inner,
        &request,
        &no_ignore,
        &registry,
        &nightly_overlay()
    ));
    let no_rust_metadata = Bodies::new([path("/fixture"), path("/rules"), path("/platforms")])
        .with_visible_lockfile_metadata(RegistryChoice::PreparedFile)
        .with_repo_marker_metadata(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_ignore_file_bytes(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_nightly_package_metadata(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_nightly_build_source(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_nightly_channel_source(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_probe_package_source(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_rules_rust_defs_source(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_rules_rust_defs_import_sources(
            RegistryChoice::PreparedFile,
            CommandChoice::PublicProbe,
        );
    assert!(!projection_matches(
        &inner,
        &request,
        &no_rust_metadata,
        &registry,
        &nightly_overlay()
    ));
    let no_nightly_metadata = Bodies::new([path("/fixture"), path("/rules"), path("/platforms")])
        .with_visible_lockfile_metadata(RegistryChoice::PreparedFile)
        .with_repo_marker_metadata(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_ignore_file_bytes(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_rust_package_metadata(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_nightly_build_source(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_nightly_channel_source(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_probe_package_source(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_rules_rust_defs_source(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_rules_rust_defs_import_sources(
            RegistryChoice::PreparedFile,
            CommandChoice::PublicProbe,
        );
    assert!(!projection_matches(
        &inner,
        &request,
        &no_nightly_metadata,
        &registry,
        &nightly_overlay()
    ));
    let no_build_source = Bodies::new([path("/fixture"), path("/rules"), path("/platforms")])
        .with_visible_lockfile_metadata(RegistryChoice::PreparedFile)
        .with_repo_marker_metadata(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_ignore_file_bytes(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_rust_package_metadata(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_nightly_package_metadata(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_nightly_channel_source(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_probe_package_source(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_rules_rust_defs_source(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_rules_rust_defs_import_sources(
            RegistryChoice::PreparedFile,
            CommandChoice::PublicProbe,
        );
    assert!(!projection_matches(
        &inner,
        &request,
        &no_build_source,
        &registry,
        &nightly_overlay()
    ));
    let no_channel_source = Bodies::new([path("/fixture"), path("/rules"), path("/platforms")])
        .with_visible_lockfile_metadata(RegistryChoice::PreparedFile)
        .with_repo_marker_metadata(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_ignore_file_bytes(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_rust_package_metadata(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_nightly_package_metadata(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_nightly_build_source(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_probe_package_source(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_rules_rust_defs_source(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_rules_rust_defs_import_sources(
            RegistryChoice::PreparedFile,
            CommandChoice::PublicProbe,
        );
    assert!(!projection_matches(
        &inner,
        &request,
        &no_channel_source,
        &registry,
        &nightly_overlay()
    ));
    let no_package_source = Bodies::new([path("/fixture"), path("/rules"), path("/platforms")])
        .with_visible_lockfile_metadata(RegistryChoice::PreparedFile)
        .with_repo_marker_metadata(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_ignore_file_bytes(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_rust_package_metadata(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_nightly_package_metadata(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_nightly_build_source(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_nightly_channel_source(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_rules_rust_defs_source(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_rules_rust_defs_import_sources(
            RegistryChoice::PreparedFile,
            CommandChoice::PublicProbe,
        );
    assert!(!projection_matches(
        &inner,
        &request,
        &no_package_source,
        &registry,
        &nightly_overlay()
    ));
    Arc::get_mut(&mut bodies).unwrap().rules_rust_defs_source = false;
    assert!(!projection_matches(
        &inner,
        &request,
        &bodies,
        &registry,
        &nightly_overlay()
    ));
    Arc::get_mut(&mut bodies).unwrap().rules_rust_defs_source = true;
    Arc::get_mut(&mut bodies)
        .unwrap()
        .rules_rust_defs_import_sources = false;
    assert!(!projection_matches(
        &inner,
        &request,
        &bodies,
        &registry,
        &nightly_overlay()
    ));
    Arc::get_mut(&mut bodies)
        .unwrap()
        .rules_rust_defs_import_sources = true;
    request.lockfile_mode = LockfileMode::Update;
    assert!(!projection_matches(
        &inner,
        &request,
        &bodies,
        &registry,
        &nightly_overlay()
    ));
    request.lockfile_mode = LockfileMode::Off;
    request.registry_urls = RegistryUrls::new(["https://bcr.bazel.build"]);
    assert!(!projection_matches(
        &inner,
        &request,
        &bodies,
        &registry,
        &nightly_overlay()
    ));
    let milestones = Arc::new(Mutex::new(Milestones::default()));
    let guarded = GuardedSingleton {
        inner: inner.clone(),
        bodies,
        milestones: milestones.clone(),
    };
    assert!(guarded.initializes_request_revision());
    assert_eq!(
        guarded.requires_workspace_status_inputs(),
        inner.requires_workspace_status_inputs()
    );
    assert_eq!(
        std::mem::discriminant(&guarded.observed_selection_association()),
        std::mem::discriminant(&inner.observed_selection_association())
    );
    assert!(!guarded.allows_empty_terminal());
    // Only a synthetic metadata error is supplied for delegation; no build producer is claimed.
    let terminal = SingletonRootSingleBuildCommandTerminal {
        result: Arc::new(Err(BuildCommandError::infrastructure("unit delegation"))),
        observations: None,
    };
    assert!(guarded.source_certificate(&terminal).is_none());
    assert!(guarded.observations(&terminal).is_none());
    assert_eq!(
        std::mem::discriminant(&guarded.terminal_observed_selection_association(&terminal)),
        std::mem::discriminant(&inner.terminal_observed_selection_association(&terminal))
    );
    assert_eq!(
        std::mem::discriminant(&guarded.terminal_demand_association(&terminal)),
        std::mem::discriminant(&inner.terminal_demand_association(&terminal))
    );
    assert_eq!(
        std::mem::discriminant(&guarded.event_reconciliation_policy(&terminal)),
        std::mem::discriminant(&inner.event_reconciliation_policy(&terminal))
    );
    assert_eq!(
        guarded.allows_unavailable_terminal_roots(&terminal),
        inner.allows_unavailable_terminal_roots(&terminal)
    );
    assert_eq!(
        guarded.publication_completed(&terminal),
        inner.publication_completed(&terminal)
    );
    assert!(Arc::ptr_eq(&terminal.result, &terminal.clone().result));
    let demand =
        |p: &str, operation| {
            SourcePreparationNeeds::path(NeedPathObservations::singleton(
                PathObservationDemand::new(PathObservationNamespace::Host, path(p), operation),
            ))
        };
    let allowed = demand(
        "/fixture/MODULE.bazel.lock",
        PathObservationOperation::Lstat,
    );
    assert!(guarded.check_need(&allowed).is_ok());
    let mut progress = 0;
    for (excluded, operation) in [
        (
            demand("/fixture/pkg/main.rs", PathObservationOperation::FileBytes),
            "file_bytes",
        ),
        (
            SourcePreparationNeeds::root_module_bootstrap(
                slug_bzlmod_v2::RootModuleBootstrapRequest {
                    workspace: path("/foreign"),
                },
            ),
            "none",
        ),
    ] {
        let mixed = allowed.try_union(&excluded).unwrap();
        if guarded.check_need(&mixed).is_ok() {
            progress += 1;
        }
        assert!(guarded.check_need(&mixed).is_err());
        assert_eq!(
            milestones
                .lock()
                .unwrap()
                .denied
                .as_ref()
                .unwrap()
                .operation,
            operation
        );
    }
    assert_eq!(progress, 0);
    assert_eq!(
        milestones
            .lock()
            .unwrap()
            .denied
            .as_ref()
            .unwrap()
            .operation,
        "none"
    );
}
#[test]
fn repo_marker_metadata_need_is_opt_in_root_lstat_only() {
    assert_exact_root_file_role(
        "REPO.bazel",
        &[PathObservationOperation::Lstat],
        Bodies::with_repo_marker_metadata,
        |b| b.repo_marker_metadata,
        &[
            "/other/REPO.bazel",
            "/fixture/sub/REPO.bazel",
            "/fixture/REPO.bazel/child",
            "/fixture/REPO.bazel.other",
            "/fixture/repo.bazel",
            "/fixture/REPO.BAZEL",
        ],
        "/rules/REPO.bazel",
        None,
    );
}
#[test]
fn ignore_file_bytes_need_is_opt_in_exact_root_only() {
    assert_exact_root_file_role(
        ".bazelignore",
        &[PathObservationOperation::FileBytes],
        Bodies::with_ignore_file_bytes,
        |b| b.ignore_file_bytes,
        &[
            "/other/.bazelignore",
            "/fixture/sub/.bazelignore",
            "/fixture/.bazelignore/child",
            "/fixture/.bazelignore.other",
            "/fixture/.BAZELIGNORE",
            "/fixture/bazelignore",
        ],
        "/rules/.bazelignore",
        None,
    );
}
#[test]
fn rust_package_metadata_need_is_opt_in_exact_rules_root_only() {
    assert_exact_root_file_role(
        "rust",
        &[PathObservationOperation::Lstat],
        Bodies::with_rust_package_metadata,
        |b| b.rust_package_metadata,
        &[
            "/other/rust",
            "/rules/sub/rust",
            "/rules/rust/child",
            "/rules/rust.extra",
            "/rules/RUST",
            "/rules/Rust",
            "/rules-other/rust",
        ],
        "/rules/rust",
        Some("/rules"),
    );
}
#[test]
fn nightly_package_metadata_need_is_opt_in_exact_chain_only() {
    for (relative, selected, spellings) in [
        (
            "rust/toolchain",
            "/rules/rust/toolchain",
            &[
                "/other/rust/toolchain",
                "/rules/rust/other/toolchain",
                "/rules/rust/toolchain/child",
                "/rules/rust/toolchain.extra",
                "/rules/rust/TOOLCHAIN",
                "/rules/RUST/toolchain",
                "/rules-other/rust/toolchain",
            ][..],
        ),
        (
            "rust/toolchain/channel",
            "/rules/rust/toolchain/channel",
            &[
                "/other/rust/toolchain/channel",
                "/rules/rust/other/channel",
                "/rules/rust/toolchain/channel/child",
                "/rules/rust/toolchain/channel.extra",
                "/rules/rust/toolchain/CHANNEL",
                "/rules/rust/TOOLCHAIN/channel",
                "/rules-other/rust/toolchain/channel",
            ][..],
        ),
    ] {
        assert_exact_root_file_role(
            relative,
            &[PathObservationOperation::Lstat],
            Bodies::with_nightly_package_metadata,
            |b| b.nightly_package_metadata,
            spellings,
            selected,
            Some("/rules"),
        );
    }
    let path = |p: &str| NormalizedAbsolutePath::new(p).unwrap();
    let bodies = Bodies::new([path("/fixture"), path("/rules"), path("/platforms")])
        .with_nightly_package_metadata(RegistryChoice::PreparedFile, CommandChoice::PublicProbe);
    let need =
        |p: &str, operation| {
            SourcePreparationNeeds::path(NeedPathObservations::singleton(
                PathObservationDemand::new(PathObservationNamespace::Host, path(p), operation),
            ))
        };
    let both = need("/rules/rust/toolchain", PathObservationOperation::Lstat)
        .try_union(&need(
            "/rules/rust/toolchain/channel",
            PathObservationOperation::Lstat,
        ))
        .unwrap();
    assert!(bodies.admit(&both).is_ok());
    for (excluded, operation) in [
        (
            "/rules/rust/toolchain/channel/child",
            PathObservationOperation::Lstat,
        ),
        (
            "/rules/rust/toolchain/channel",
            PathObservationOperation::FileBytes,
        ),
    ] {
        let mixed = both.try_union(&need(excluded, operation.dupe())).unwrap();
        let mut progress = 0;
        if bodies.admit(&mixed).is_ok() {
            progress += 1;
        }
        assert_eq!(progress, 0);
        let denied = bodies.admit(&mixed).unwrap_err();
        assert_eq!(denied.family, "path");
        assert_eq!(denied.operation, path_operation_code(&operation));
        assert_eq!(denied.path.unwrap().as_path(), Path::new(excluded));
    }
}
#[test]
fn nightly_build_source_need_is_opt_in_exact_package_only() {
    for (relative, selected, spellings) in [
        (
            "rust/toolchain/channel/BUILD.bazel",
            "/rules/rust/toolchain/channel/BUILD.bazel",
            &[
                "/other/rust/toolchain/channel/BUILD.bazel",
                "/rules/rust/toolchain/other/BUILD.bazel",
                "/rules/rust/toolchain/channel/BUILD.bazel/child",
                "/rules/rust/toolchain/channel/BUILD.bazel.extra",
                "/rules/rust/toolchain/channel/build.bazel",
                "/rules/rust/TOOLCHAIN/channel/BUILD.bazel",
                "/rules-other/rust/toolchain/channel/BUILD.bazel",
            ][..],
        ),
        (
            "rust/toolchain/channel/BUILD",
            "/rules/rust/toolchain/channel/BUILD",
            &[
                "/other/rust/toolchain/channel/BUILD",
                "/rules/rust/toolchain/other/BUILD",
                "/rules/rust/toolchain/channel/BUILD/child",
                "/rules/rust/toolchain/channel/BUILD.extra",
                "/rules/rust/toolchain/channel/build",
                "/rules/rust/TOOLCHAIN/channel/BUILD",
                "/rules-other/rust/toolchain/channel/BUILD",
            ][..],
        ),
    ] {
        assert_exact_root_file_role(
            relative,
            &[
                PathObservationOperation::Lstat,
                PathObservationOperation::FileBytes,
            ],
            Bodies::with_nightly_build_source,
            |b| b.nightly_build_source,
            spellings,
            selected,
            Some("/rules"),
        );
    }
    let path = |p: &str| NormalizedAbsolutePath::new(p).unwrap();
    let bodies = Bodies::new([path("/fixture"), path("/rules"), path("/platforms")])
        .with_nightly_build_source(RegistryChoice::PreparedFile, CommandChoice::PublicProbe);
    let need =
        |p: &str, operation| {
            SourcePreparationNeeds::path(NeedPathObservations::singleton(
                PathObservationDemand::new(PathObservationNamespace::Host, path(p), operation),
            ))
        };
    let mut all = need(
        "/rules/rust/toolchain/channel/BUILD.bazel",
        PathObservationOperation::Lstat,
    );
    for (marker, operation) in [
        (
            "/rules/rust/toolchain/channel/BUILD.bazel",
            PathObservationOperation::FileBytes,
        ),
        (
            "/rules/rust/toolchain/channel/BUILD",
            PathObservationOperation::Lstat,
        ),
        (
            "/rules/rust/toolchain/channel/BUILD",
            PathObservationOperation::FileBytes,
        ),
    ] {
        all = all.try_union(&need(marker, operation)).unwrap();
    }
    assert!(bodies.admit(&all).is_ok());
    for (excluded, operation) in [
        (
            "/rules/rust/toolchain/channel/BUILD.bazel/child",
            PathObservationOperation::Lstat,
        ),
        (
            "/rules/rust/toolchain/channel/BUILD",
            PathObservationOperation::FileDigest,
        ),
    ] {
        let mixed = all.try_union(&need(excluded, operation.dupe())).unwrap();
        let mut progress = 0;
        if bodies.admit(&mixed).is_ok() {
            progress += 1;
        }
        assert_eq!(progress, 0);
        let denied = bodies.admit(&mixed).unwrap_err();
        assert_eq!(denied.family, "path");
        assert_eq!(denied.operation, path_operation_code(&operation));
        assert_eq!(denied.path.unwrap().as_path(), Path::new(excluded));
    }
}
#[test]
fn nightly_channel_source_need_is_opt_in_exact_import_only() {
    assert_exact_root_file_role(
        "rust/toolchain/channel/channel.bzl",
        &[
            PathObservationOperation::Lstat,
            PathObservationOperation::FileBytes,
        ],
        Bodies::with_nightly_channel_source,
        |b| b.nightly_channel_source,
        &[
            "/other/rust/toolchain/channel/channel.bzl",
            "/rules/rust/toolchain/other/channel.bzl",
            "/rules/rust/toolchain/channel/channel.bzl/child",
            "/rules/rust/toolchain/channel/channel.bzl.extra",
            "/rules/rust/toolchain/channel/CHANNEL.bzl",
            "/rules/rust/TOOLCHAIN/channel/channel.bzl",
            "/rules-other/rust/toolchain/channel/channel.bzl",
        ],
        "/rules/rust/toolchain/channel/channel.bzl",
        Some("/rules"),
    );
    let path = |p: &str| NormalizedAbsolutePath::new(p).unwrap();
    let bodies = Bodies::new([path("/fixture"), path("/rules"), path("/platforms")])
        .with_nightly_channel_source(RegistryChoice::PreparedFile, CommandChoice::PublicProbe);
    let need =
        |p: &str, operation| {
            SourcePreparationNeeds::path(NeedPathObservations::singleton(
                PathObservationDemand::new(PathObservationNamespace::Host, path(p), operation),
            ))
        };
    let both = need(
        "/rules/rust/toolchain/channel/channel.bzl",
        PathObservationOperation::Lstat,
    )
    .try_union(&need(
        "/rules/rust/toolchain/channel/channel.bzl",
        PathObservationOperation::FileBytes,
    ))
    .unwrap();
    assert!(bodies.admit(&both).is_ok());
    for (excluded, operation) in [
        (
            "/rules/rust/toolchain/channel/channel.bzl/child",
            PathObservationOperation::Lstat,
        ),
        (
            "/rules/rust/toolchain/channel/channel.bzl",
            PathObservationOperation::FileDigest,
        ),
    ] {
        let mixed = both.try_union(&need(excluded, operation.dupe())).unwrap();
        let mut progress = 0;
        if bodies.admit(&mixed).is_ok() {
            progress += 1;
        }
        assert_eq!(progress, 0);
        let denied = bodies.admit(&mixed).unwrap_err();
        assert_eq!(denied.family, "path");
        assert_eq!(denied.operation, path_operation_code(&operation));
        assert_eq!(denied.path.unwrap().as_path(), Path::new(excluded));
    }
}
#[test]
fn rules_rust_defs_source_need_is_opt_in_exact_import_only() {
    assert_exact_root_file_role(
        "rust/defs.bzl",
        &[
            PathObservationOperation::Lstat,
            PathObservationOperation::FileBytes,
        ],
        Bodies::with_rules_rust_defs_source,
        |b| b.rules_rust_defs_source,
        &[
            "/other/rust/defs.bzl",
            "/rules/rust/private/defs.bzl",
            "/rules/rust/defs.bzl/child",
            "/rules/rust/defs.bzl.extra",
            "/rules/rust/DEFS.bzl",
            "/rules/RUST/defs.bzl",
            "/rules-other/rust/defs.bzl",
        ],
        "/rules/rust/defs.bzl",
        Some("/rules"),
    );
    let path = |p: &str| NormalizedAbsolutePath::new(p).unwrap();
    let bodies = Bodies::new([path("/fixture"), path("/rules"), path("/platforms")])
        .with_rules_rust_defs_source(RegistryChoice::PreparedFile, CommandChoice::PublicProbe);
    let need =
        |p: &str, operation| {
            SourcePreparationNeeds::path(NeedPathObservations::singleton(
                PathObservationDemand::new(PathObservationNamespace::Host, path(p), operation),
            ))
        };
    let both = need("/rules/rust/defs.bzl", PathObservationOperation::Lstat)
        .try_union(&need(
            "/rules/rust/defs.bzl",
            PathObservationOperation::FileBytes,
        ))
        .unwrap();
    assert!(bodies.admit(&both).is_ok());
    for (excluded, operation) in [
        (
            "/rules/rust/defs.bzl/child",
            PathObservationOperation::Lstat,
        ),
        ("/rules/rust/defs.bzl", PathObservationOperation::FileDigest),
    ] {
        let mixed = both.try_union(&need(excluded, operation.dupe())).unwrap();
        let mut progress = 0;
        if bodies.admit(&mixed).is_ok() {
            progress += 1;
        }
        assert_eq!(progress, 0);
        let denied = bodies.admit(&mixed).unwrap_err();
        assert_eq!(denied.family, "path");
        assert_eq!(denied.operation, path_operation_code(&operation));
        assert_eq!(denied.path.unwrap().as_path(), Path::new(excluded));
    }
}
#[test]
fn rules_rust_defs_import_sources_need_is_opt_in_exact_table_only() {
    let operations = [
        PathObservationOperation::Lstat,
        PathObservationOperation::FileBytes,
    ];
    for relative in RULES_RUST_DEFS_IMPORTS {
        let selected = Path::new("/rules").join(relative);
        assert_exact_root_file_role(
            relative,
            &operations,
            Bodies::with_rules_rust_defs_import_sources,
            |b| b.rules_rust_defs_import_sources,
            &[],
            selected.to_str().unwrap(),
            Some("/rules"),
        );
    }
    for (relative, selected, spellings) in [
        (
            "rust/private",
            "/rules/rust/private",
            [
                "/rules/rust/private/child",
                "/rules/RUST/private",
                "/rules/rust/private-extra",
                "/rules-other/rust/private",
            ],
        ),
        (
            "rust/platform",
            "/rules/rust/platform",
            [
                "/rules/rust/platform/child",
                "/rules/RUST/platform",
                "/rules/rust/platform-extra",
                "/rules-other/rust/platform",
            ],
        ),
        (
            "rust/settings",
            "/rules/rust/settings",
            [
                "/rules/rust/settings/child",
                "/rules/RUST/settings",
                "/rules/rust/settings-extra",
                "/rules-other/rust/settings",
            ],
        ),
    ] {
        assert_exact_root_file_role(
            relative,
            &[PathObservationOperation::Lstat],
            Bodies::with_rules_rust_defs_import_sources,
            |b| b.rules_rust_defs_import_sources,
            &spellings,
            selected,
            Some("/rules"),
        );
    }
    let path = |p: &str| NormalizedAbsolutePath::new(p).unwrap();
    let bodies = Bodies::new([path("/fixture"), path("/rules"), path("/platforms")])
        .with_rules_rust_defs_import_sources(
            RegistryChoice::PreparedFile,
            CommandChoice::PublicProbe,
        );
    let need = |p: &Path, operation| {
        SourcePreparationNeeds::path(NeedPathObservations::singleton(PathObservationDemand::new(
            PathObservationNamespace::Host,
            NormalizedAbsolutePath::new(p).unwrap(),
            operation,
        )))
    };
    let mut all = need(
        Path::new("/rules/rust/private"),
        PathObservationOperation::Lstat,
    );
    for relative in &RULES_RUST_IMPORT_METADATA[1..] {
        let selected = Path::new("/rules").join(relative);
        all = all
            .try_union(&need(&selected, PathObservationOperation::Lstat))
            .unwrap();
    }
    for relative in RULES_RUST_DEFS_IMPORTS {
        let selected = Path::new("/rules").join(relative);
        for operation in &operations {
            all = all.try_union(&need(&selected, operation.dupe())).unwrap();
        }
    }
    assert_eq!(all.path_observations().unwrap().demands().len(), 59);
    assert!(bodies.admit(&all).is_ok());
    for (excluded, operation) in [
        (
            "/rules/rust/private/unknown.bzl",
            PathObservationOperation::Lstat,
        ),
        (
            "/rules/rust/private/rust.bzl.extra",
            PathObservationOperation::FileBytes,
        ),
        (
            "/rules/rust/private/RUST.bzl",
            PathObservationOperation::FileBytes,
        ),
        (
            "/rules/RUST/private/rust.bzl",
            PathObservationOperation::FileBytes,
        ),
        (
            "/rules/rust/private/rust.bzl/child",
            PathObservationOperation::Lstat,
        ),
        (
            "/rules/rust/private/rust.bzl",
            PathObservationOperation::FileDigest,
        ),
        ("/rules/rust/private", PathObservationOperation::FileBytes),
        ("/rules/rust/platform", PathObservationOperation::FileBytes),
        ("/rules/rust/settings", PathObservationOperation::FileBytes),
        (
            "/rules/rust/platform/child",
            PathObservationOperation::Lstat,
        ),
        (
            "/rules/rust/settings/child",
            PathObservationOperation::Lstat,
        ),
        (
            "/rules/rust/private/BUILD.bazel",
            PathObservationOperation::Lstat,
        ),
    ] {
        let mixed = all
            .try_union(&need(Path::new(excluded), operation.dupe()))
            .unwrap();
        let mut progress = 0;
        if bodies.admit(&mixed).is_ok() {
            progress += 1;
        }
        assert_eq!(progress, 0);
        let denied = bodies.admit(&mixed).unwrap_err();
        assert_eq!(denied.family, "path");
        assert_eq!(denied.operation, path_operation_code(&operation));
        assert_eq!(denied.path.unwrap().as_path(), Path::new(excluded));
    }
}
#[test]
fn rules_rust_defs_import_source_limits_preserve_byte_budgets() {
    use crate::runtime::dice::source_service::SourceCounter;
    let path = |p: &str| NormalizedAbsolutePath::new(p).unwrap();
    let make = || Bodies::new([path("/fixture"), path("/rules"), path("/platforms")]);
    let baseline = SourceServiceLimits {
        allow_network: false,
        registry_object_bytes: 1 << 20,
        registry_attempts: 128,
        registry_admitted_bytes: 16 << 20,
        source_attempts: 16,
        transferred_bytes: 16 << 20,
        decoded_bytes: 16 << 20,
        written_bytes: 16 << 20,
        live_buffer_bytes: 16 << 20,
        blocking_threads: Some(2),
    };
    assert_eq!(make().source_limits(RegistryChoice::PreparedFile), baseline);
    assert_eq!(SourceServiceLimits::default().source_attempts, 64);
    for (registry, command, attempts) in [
        (RegistryChoice::DefaultBcr, CommandChoice::Modules, 16),
        (RegistryChoice::DefaultBcr, CommandChoice::PublicProbe, 16),
        (RegistryChoice::PreparedFile, CommandChoice::Modules, 16),
        (RegistryChoice::PreparedFile, CommandChoice::PublicProbe, 64),
    ] {
        let bodies = make().with_rules_rust_defs_import_sources(registry, command);
        let limits = bodies.source_limits(registry);
        assert_eq!(
            limits,
            SourceServiceLimits {
                registry_attempts: registry.registry_attempts(),
                source_attempts: attempts,
                ..baseline
            }
        );
        for (counter, maximum) in [
            (SourceCounter::SourceAttempts, attempts),
            (SourceCounter::DecodedBytes, 16 << 20),
        ] {
            let start = Instant::now();
            let scope = SourceServiceScope::new_for_test(
                path("/fixture"),
                start + Duration::from_secs(2),
                start + Duration::from_secs(3),
                limits,
            )
            .unwrap();
            assert_eq!(scope.limits(), limits);
            scope.charge(counter, maximum).unwrap();
            assert_eq!(
                scope.charge(counter, 1),
                Err(SourceServiceError::Limit(counter))
            );
            let snapshot = scope.snapshot();
            assert_eq!(
                snapshot.source_attempts,
                if counter == SourceCounter::SourceAttempts {
                    attempts
                } else {
                    0
                }
            );
            assert_eq!(
                snapshot.decoded_bytes,
                if counter == SourceCounter::DecodedBytes {
                    16 << 20
                } else {
                    0
                }
            );
        }
    }
}
#[test]
fn probe_package_source_need_is_opt_in_exact_fixture_only() {
    for (relative, selected, operations, spellings) in [
        (
            "pkg",
            "/fixture/pkg",
            &[PathObservationOperation::Lstat][..],
            &[
                "/other/pkg",
                "/fixture/other/pkg",
                "/fixture/pkg/child",
                "/fixture/pkg.extra",
                "/fixture/PKG",
                "/fixture/Pkg",
                "/fixture-other/pkg",
            ][..],
        ),
        (
            "pkg/BUILD.bazel",
            "/fixture/pkg/BUILD.bazel",
            &[
                PathObservationOperation::Lstat,
                PathObservationOperation::FileBytes,
            ][..],
            &[
                "/other/pkg/BUILD.bazel",
                "/fixture/other/BUILD.bazel",
                "/fixture/pkg/BUILD.bazel/child",
                "/fixture/pkg/BUILD.bazel.extra",
                "/fixture/pkg/build.bazel",
                "/fixture/PKG/BUILD.bazel",
                "/fixture-other/pkg/BUILD.bazel",
            ][..],
        ),
        (
            "pkg/BUILD",
            "/fixture/pkg/BUILD",
            &[
                PathObservationOperation::Lstat,
                PathObservationOperation::FileBytes,
            ][..],
            &[
                "/other/pkg/BUILD",
                "/fixture/other/BUILD",
                "/fixture/pkg/BUILD/child",
                "/fixture/pkg/BUILD.extra",
                "/fixture/pkg/build",
                "/fixture/PKG/BUILD",
                "/fixture-other/pkg/BUILD",
            ][..],
        ),
    ] {
        assert_exact_root_file_role(
            relative,
            operations,
            Bodies::with_probe_package_source,
            |b| b.probe_package_source,
            spellings,
            selected,
            Some("/fixture"),
        );
    }
    let path = |p: &str| NormalizedAbsolutePath::new(p).unwrap();
    let bodies = Bodies::new([path("/fixture"), path("/rules"), path("/platforms")])
        .with_probe_package_source(RegistryChoice::PreparedFile, CommandChoice::PublicProbe);
    let need =
        |p: &str, operation| {
            SourcePreparationNeeds::path(NeedPathObservations::singleton(
                PathObservationDemand::new(PathObservationNamespace::Host, path(p), operation),
            ))
        };
    let mut all = need("/fixture/pkg", PathObservationOperation::Lstat);
    for (marker, operation) in [
        ("/fixture/pkg/BUILD.bazel", PathObservationOperation::Lstat),
        (
            "/fixture/pkg/BUILD.bazel",
            PathObservationOperation::FileBytes,
        ),
        ("/fixture/pkg/BUILD", PathObservationOperation::Lstat),
        ("/fixture/pkg/BUILD", PathObservationOperation::FileBytes),
    ] {
        all = all.try_union(&need(marker, operation)).unwrap();
    }
    assert!(bodies.admit(&all).is_ok());
    for (excluded, operation) in [
        ("/fixture/pkg", PathObservationOperation::FileBytes),
        ("/fixture/pkg/child", PathObservationOperation::Lstat),
        ("/fixture/pkg/main.rs", PathObservationOperation::FileBytes),
    ] {
        let mixed = all.try_union(&need(excluded, operation.dupe())).unwrap();
        let mut progress = 0;
        if bodies.admit(&mixed).is_ok() {
            progress += 1;
        }
        assert_eq!(progress, 0);
        let denied = bodies.admit(&mixed).unwrap_err();
        assert_eq!(denied.family, "path");
        assert_eq!(denied.operation, path_operation_code(&operation));
        assert_eq!(denied.path.unwrap().as_path(), Path::new(excluded));
    }
}
fn assert_exact_root_file_role(
    name: &str,
    admitted_operations: &[PathObservationOperation],
    select: fn(Bodies, RegistryChoice, CommandChoice) -> Bodies,
    is_enabled: fn(&Bodies) -> bool,
    spellings: &[&str],
    selected_path: &str,
    selected_root: Option<&str>,
) {
    let path = |s: &str| NormalizedAbsolutePath::new(s).unwrap();
    let make = || {
        Bodies::new([path("/fixture"), path("/rules"), path("/platforms")])
            .with_fourth(path("/features"))
            .with_fifth(path("/skylib"))
            .with_sixth(path("/cc"))
            .with_seventh(path("/license"))
            .with_eighth(path("/shell"))
            .with_ninth(path("/apple"))
            .with_tenth(path("/metadata"))
            .with_eleventh(path("/buildozer"))
    };
    let budget = slug_workspace_v2::ImageDescriptorBudget::new(4096);
    let need = |namespace, leaf: NormalizedAbsolutePath, operation| {
        let demand = match operation {
            PathObservationOperation::WindowsLongPath => {
                assert_eq!(namespace, PathObservationNamespace::Host);
                PathObservationDemand::windows_long_path(leaf, Arc::from([b'x' as u16]))
            }
            PathObservationOperation::WindowsOptionPathLongName => {
                assert_eq!(namespace, PathObservationNamespace::Host);
                PathObservationDemand::windows_option_path_long_name(leaf, Arc::from([b'x' as u16]))
            }
            PathObservationOperation::InputImage(request) => PathObservationDemand::input_image(
                namespace,
                slug_workspace_v2::ImageChargedPath::pin_existing(leaf, &budget).unwrap(),
                request,
            )
            .unwrap(),
            _ => PathObservationDemand::new(namespace, leaf, operation),
        };
        SourcePreparationNeeds::path(NeedPathObservations::singleton(demand))
    };
    let host = PathObservationNamespace::Host;
    assert!(!is_enabled(&make()));
    for (registry, command, enabled) in [
        (RegistryChoice::DefaultBcr, CommandChoice::Modules, false),
        (
            RegistryChoice::DefaultBcr,
            CommandChoice::PublicProbe,
            false,
        ),
        (RegistryChoice::PreparedFile, CommandChoice::Modules, false),
        (
            RegistryChoice::PreparedFile,
            CommandChoice::PublicProbe,
            true,
        ),
    ] {
        let bodies = select(make(), registry, command);
        assert_eq!(is_enabled(&bodies), enabled);
        assert_eq!(bodies.roots().count(), 11);
        for root in bodies.roots() {
            let marker = NormalizedAbsolutePath::new(root.as_path().join(name)).unwrap();
            for admitted_operation in admitted_operations {
                let demand = need(host, marker.dupe(), admitted_operation.dupe());
                if enabled
                    && selected_root.is_none_or(|selected| root.as_path() == Path::new(selected))
                {
                    assert!(bodies.admit(&demand).is_ok());
                } else {
                    let denied = bodies.admit(&demand).unwrap_err();
                    assert_eq!(
                        (denied.family, denied.operation),
                        ("path", path_operation_code(&admitted_operation))
                    );
                    assert_eq!(denied.path.as_ref(), Some(&marker));
                }
            }
        }
    }
    let admitted = select(
        make(),
        RegistryChoice::PreparedFile,
        CommandChoice::PublicProbe,
    );
    let foreign = PathObservationNamespace::Materialization(
        slug_workspace_v2::PathObservationInstanceId::new(1),
    );
    for root in admitted.roots() {
        let marker = NormalizedAbsolutePath::new(root.as_path().join(name)).unwrap();
        let anchor =
            slug_workspace_v2::ImageChargedPath::pin_existing(root.dupe(), &budget).unwrap();
        let image = slug_workspace_v2::ImageObservationRequest::new(
            anchor,
            slug_workspace_v2::ImageObservationOperation::Lstat,
        );
        for operation in [
            PathObservationOperation::Lstat,
            PathObservationOperation::FileBytes,
            PathObservationOperation::FileDigest,
            PathObservationOperation::ReadLink,
            PathObservationOperation::DirectoryEntries,
            PathObservationOperation::InputImage(image),
            PathObservationOperation::WindowsLongPath,
            PathObservationOperation::WindowsOptionPathLongName,
        ] {
            if (admitted_operations.contains(&operation)
                && selected_root.is_none_or(|selected| root.as_path() == Path::new(selected)))
                || (name == ".bazelignore" && operation == PathObservationOperation::Lstat)
            {
                assert!(
                    admitted
                        .admit(&need(host, marker.dupe(), operation))
                        .is_ok()
                );
                continue;
            }
            let code = path_operation_code(&operation);
            let denied = admitted
                .admit(&need(host, marker.dupe(), operation))
                .unwrap_err();
            assert_eq!(denied.operation, code);
            assert_eq!(denied.path.as_ref(), Some(&marker));
        }
        for admitted_operation in admitted_operations {
            let denied = admitted
                .admit(&need(foreign, marker.dupe(), admitted_operation.dupe()))
                .unwrap_err();
            assert_eq!(
                (denied.family, denied.operation),
                ("namespace", path_operation_code(&admitted_operation))
            );
            assert_eq!(denied.path.as_ref(), Some(&marker));
        }
    }
    assert_eq!(budget.used_bytes(), 0);
    for admitted_operation in admitted_operations {
        for &spelling in spellings {
            let denied = admitted
                .admit(&need(host, path(spelling), admitted_operation.dupe()))
                .unwrap_err();
            assert_eq!(
                (denied.family, denied.operation),
                ("path", path_operation_code(&admitted_operation))
            );
            assert_eq!(denied.path.unwrap().as_path(), Path::new(spelling));
        }
        let allowed = need(host, path(selected_path), admitted_operation.dupe());
        let mut progress_calls = 0;
        for (excluded, family, operation) in [
            (
                need(
                    host,
                    path(selected_path),
                    PathObservationOperation::FileDigest,
                ),
                "path",
                "file_digest",
            ),
            (
                need(foreign, path(selected_path), admitted_operation.dupe()),
                "namespace",
                path_operation_code(&admitted_operation),
            ),
            (
                SourcePreparationNeeds::root_module_bootstrap(
                    slug_bzlmod_v2::RootModuleBootstrapRequest {
                        workspace: path("/foreign"),
                    },
                ),
                "bootstrap",
                "none",
            ),
        ] {
            let mixed = allowed.try_union(&excluded).unwrap();
            if admitted.admit(&mixed).is_ok() {
                progress_calls += 1;
            }
            let denied = admitted.admit(&mixed).unwrap_err();
            assert_eq!((denied.family, denied.operation), (family, operation));
        }
        assert_eq!(progress_calls, 0);
    }
}

#[test]
#[ignore = "bounded normal public producer observation; frozen supervisor inputs required"]
fn native_eleven_root_prepared_file_public_probe_prerequisite() {
    observe_native_command(
        ModuleMode::Eleven,
        RegistryChoice::PreparedFile,
        CommandChoice::PublicProbe,
    );
}
