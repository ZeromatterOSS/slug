// Literal source controls are conversion evidence; native capture is tested separately.
const DATES: &[(i128, &str)] = &[
    (0, "1970 Jan 01 00 00 00 Thu"),
    (1, "1970 Jan 01 00 00 01 Thu"),
    (86_400, "1970 Jan 02 00 00 00 Fri"),
    (172_800, "1970 Jan 03 00 00 00 Sat"),
    (259_200, "1970 Jan 04 00 00 00 Sun"),
    (345_600, "1970 Jan 05 00 00 00 Mon"),
    (432_000, "1970 Jan 06 00 00 00 Tue"),
    (518_400, "1970 Jan 07 00 00 00 Wed"),
    (946_684_799, "1999 Dec 31 23 59 59 Fri"),
    (946_684_800, "2000 Jan 01 00 00 00 Sat"),
    (951_782_399, "2000 Feb 28 23 59 59 Mon"),
    (951_782_400, "2000 Feb 29 00 00 00 Tue"),
    (951_868_800, "2000 Mar 01 00 00 00 Wed"),
    (1_582_979_696, "2020 Feb 29 12 34 56 Sat"),
    (1_700_000_000, "2023 Nov 14 22 13 20 Tue"),
    (1_704_067_200, "2024 Jan 01 00 00 00 Mon"),
    (1_706_745_600, "2024 Feb 01 00 00 00 Thu"),
    (1_709_251_200, "2024 Mar 01 00 00 00 Fri"),
    (1_711_929_600, "2024 Apr 01 00 00 00 Mon"),
    (1_714_521_600, "2024 May 01 00 00 00 Wed"),
    (1_717_200_000, "2024 Jun 01 00 00 00 Sat"),
    (1_719_792_000, "2024 Jul 01 00 00 00 Mon"),
    (1_722_470_400, "2024 Aug 01 00 00 00 Thu"),
    (1_725_148_800, "2024 Sep 01 00 00 00 Sun"),
    (1_727_740_800, "2024 Oct 01 00 00 00 Tue"),
    (1_730_419_200, "2024 Nov 01 00 00 00 Fri"),
    (1_733_011_200, "2024 Dec 01 00 00 00 Sun"),
    (2_147_483_647, "2038 Jan 19 03 14 07 Tue"),
    (4_107_542_399, "2100 Feb 28 23 59 59 Sun"),
    (4_107_542_400, "2100 Mar 01 00 00 00 Mon"),
    (253_402_300_799, "9999 Dec 31 23 59 59 Fri"),
];

// Independent Gregorian arithmetic for actual native samples; production uses
// gmtime_r. Fixed literal controls above also check the calendar boundaries.
fn independent_date(seconds: i128) -> String {
    let days = seconds / 86_400;
    let within = seconds % 86_400;
    let shifted = days + 719_468;
    let era = shifted / 146_097;
    let doe = shifted - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += i128::from(month <= 2);
    let months = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let weekdays = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"];
    format!(
        "{year:04} {} {day:02} {:02} {:02} {:02} {}",
        months[(month - 1) as usize],
        within / 3600,
        within % 3600 / 60,
        within % 60,
        weekdays[(days % 7) as usize]
    )
}

fn stable_bytes(user: &str, host: &str) -> Vec<u8> {
    format!("BUILD_EMBED_LABEL \nBUILD_HOST {host}\nBUILD_USER {user}\n").into_bytes()
}
fn volatile_bytes(seconds: i128, date: &str) -> Vec<u8> {
    format!("BUILD_TIMESTAMP {seconds}\nFORMATTED_DATE {date}\n").into_bytes()
}
fn header_bytes(ordinal: usize, user: &str, host: &str, seconds: i128) -> Vec<u8> {
    if ordinal == 0 {
        format!("#define BUILD_SCM_REVISION \"0\"\n#define BUILD_SCM_STATUS \"\"\n#define BUILD_TIMESTAMP {seconds}\n").into_bytes()
    } else {
        format!("#define BUILD_EMBED_LABEL \"\"\n#define BUILD_HOST \"{host}\"\n#define BUILD_USER \"{user}\"\n").into_bytes()
    }
}
fn status_workspace() -> tempfile::TempDir {
    let parent = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/wp926/status-fixtures");
    std::fs::create_dir_all(&parent).unwrap();
    let workspace = tempfile::tempdir_in(parent.canonicalize().unwrap()).unwrap();
    write_template_workspace(workspace.path());
    workspace
}

use crate::runtime::dice::build_info_native_inputs::BuildInfoNativeInputs;
use crate::runtime::dice::build_info_native_inputs::BuildInfoNativeInputsKey;
use crate::runtime::dice::build_info_native_inputs::validate_capture;
use crate::runtime::dice::workspace_status_content::WorkspaceStatusContent;
use crate::runtime::dice::workspace_status_content::WorkspaceStatusContentKey;
use crate::runtime::dice::workspace_status_content::formatted_date;
use crate::runtime::dice::workspace_status_content::produce;
use crate::runtime::workspace_status_inputs::*;

fn raw() -> RawWorkspaceStatusInputs {
    RawWorkspaceStatusInputs {
        user: ObservedText::Text("reported user".into()),
        hostname: ObservedText::Text("kernel.example".into()),
        uid: Ok((123, 456)),
        source_date_epoch: ObservedText::Absent,
        millis: Ok(1_700_000_000_999),
        policy: StatusPolicy::DefaultEnglishUtc,
        source: StatusSource::UnixReportedUserKernelHost,
    }
}
fn capture(
    workspace: &NormalizedAbsolutePath,
    request: u64,
    raw: RawWorkspaceStatusInputs,
) -> Arc<WorkspaceStatusCapture> {
    Arc::new(WorkspaceStatusCapture {
        workspace: workspace.dupe(),
        request,
        raw,
    })
}
fn supplied(raw: RawWorkspaceStatusInputs) -> Result<Arc<WorkspaceStatusContent>, StatusError> {
    let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
    produce(
        capture(&workspace, 1, raw),
        Arc::new(WorkspaceStatusArtifacts::new(workspace)),
    )
}
async fn raw_transaction(
    dice: &Arc<Dice>,
    workspace: &NormalizedAbsolutePath,
    value: Option<&Arc<WorkspaceStatusCapture>>,
    carrier: Option<&Arc<WorkspaceStatusCapture>>,
) -> dice::DiceTransaction {
    let mut data = UserComputationData::default();
    if let Some(carrier) = carrier {
        install(&mut data, carrier);
    }
    let mut updater = dice.updater_with_data(data);
    inject(&mut updater, workspace, value).unwrap();
    updater.commit().await
}

#[test]
fn status_raw_conversion_has_exact_utf8_sorted_buffers_dates_and_refusal_priority() {
    for &(seconds, date) in DATES {
        assert_eq!(independent_date(seconds), date);
        assert_eq!(formatted_date(seconds as u64).unwrap(), date);
        for remainder in [0, 999] {
            let mut input = raw();
            input.millis = Ok(seconds as u64 * 1000 + remainder);
            let value = supplied(input).unwrap();
            assert_eq!(value.stable, stable_bytes("reported user", "kernel"));
            assert_eq!(value.volatile, volatile_bytes(seconds, date));
            assert_ne!(value.stable.as_ptr(), value.volatile.as_ptr());
        }
    }
    let mut unicode = raw();
    unicode.user = ObservedText::Text("é𐐀".into());
    unicode.hostname = ObservedText::Text("höst.example".into());
    let value = supplied(unicode).unwrap();
    assert_eq!(value.stable, stable_bytes("é𐐀", "höst"));
    let parsed = WorkspaceStatusArtifacts::parse_supplied_values(&value.stable);
    assert_eq!(parsed.get("BUILD_USER"), Some("Ã©ð\u{90}\u{90}\u{80}"));
    let callback = BuildInfoCallbackRecipe::new(
        slug_build_api_v2::BuildInfoTransformKind::Info,
        CanonicalLabel::parse(BuildInfoCallbackRecipe::SOURCE_LABEL).unwrap(),
        BuildInfoCallbackRecipe::SOURCE_DIGEST,
    )
    .unwrap();
    assert_eq!(
        callback
            .render_supplied_bytes(&value.stable, TEMPLATE_CASES[1].2)
            .unwrap(),
        header_bytes(1, "é𐐀", "höst", 1_700_000_000)
    );
    let mut cases = Vec::new();
    for policy in [
        StatusPolicy::UnsupportedEmbed,
        StatusPolicy::UnsupportedCommand,
        StatusPolicy::UnsupportedLocale,
    ] {
        let mut input = raw();
        input.policy = policy;
        input.source_date_epoch = ObservedText::Text("".into());
        input.user = ObservedText::Absent;
        cases.push((input, StatusError::UnsupportedPolicy));
    }
    for epoch in [
        ObservedText::Text("".into()),
        ObservedText::Text("0".into()),
        ObservedText::NonUnicode,
        ObservedText::TooLong,
        ObservedText::ReadError(StatusError::HostRead),
    ] {
        let mut input = raw();
        input.source_date_epoch = epoch;
        input.user = ObservedText::Absent;
        cases.push((input, StatusError::SourceDateEpochPresent));
    }
    for (identity, error) in [
        (ObservedText::Absent, StatusError::MissingIdentity),
        (ObservedText::NonUnicode, StatusError::NonUnicodeIdentity),
        (ObservedText::TooLong, StatusError::OversizeIdentity),
        (
            ObservedText::Text("x".repeat(4097).into()),
            StatusError::OversizeIdentity,
        ),
        (ObservedText::Text("".into()), StatusError::InvalidIdentity),
        (
            ObservedText::Text("u\0v".into()),
            StatusError::InvalidIdentity,
        ),
        (
            ObservedText::Text("u\rv".into()),
            StatusError::InvalidIdentity,
        ),
        (
            ObservedText::Text("u\nv".into()),
            StatusError::InvalidIdentity,
        ),
        (
            ObservedText::ReadError(StatusError::HostRead),
            StatusError::HostRead,
        ),
    ] {
        for host in [false, true] {
            let mut input = raw();
            if host {
                input.hostname = identity.clone();
            } else {
                input.user = identity.clone();
            }
            input.millis = Err(StatusError::TimeRead);
            cases.push((input, error));
        }
    }
    let mut input = raw();
    input.hostname = ObservedText::Text(".example".into());
    cases.push((input, StatusError::InvalidIdentity));
    let mut input = raw();
    input.uid = Err(StatusError::HostRead);
    cases.push((input, StatusError::HostRead));
    let mut input = raw();
    input.source = StatusSource::Unsupported;
    cases.push((input, StatusError::UnsupportedPlatform));
    let mut input = raw();
    input.millis = Err(StatusError::TimeRead);
    cases.push((input, StatusError::TimeRead));
    let mut input = raw();
    input.millis = Err(StatusError::Calendar);
    cases.push((input, StatusError::Calendar));
    let mut input = raw();
    input.millis = Ok(MAX_MILLIS + 1);
    cases.push((input, StatusError::TimeRange));
    for (input, expected) in cases {
        assert_eq!(supplied(input).unwrap_err(), expected);
    }
    assert_eq!(formatted_date(u64::MAX), Err(StatusError::TimeRange));
    assert_eq!(
        crate::runtime::ProcessHostOwner::unsupported().workspace_status_inputs(),
        Err(StatusError::UnsupportedPlatform)
    );
    let mut input = raw();
    input.user = ObservedText::Text(" x ' ".into());
    assert_eq!(
        supplied(input).unwrap().stable,
        stable_bytes(" x ' ", "kernel")
    );
    assert_eq!(
        ObservedText::from_os(Some("x".repeat(4097).into())),
        ObservedText::TooLong
    );
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        assert_eq!(
            ObservedText::from_os(Some(std::ffi::OsString::from_vec(vec![0xff]))),
            ObservedText::NonUnicode
        );
    }
}

#[tokio::test]
async fn status_capture_raw_aba_and_overlapping_versions_keep_structural_request_ownership() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
    let key = WorkspaceStatusContentKey(workspace.dupe());
    let a = capture(&workspace, 1, raw());
    let mut tx_a = raw_transaction(&dice, &workspace, Some(&a), Some(&a)).await;
    let first = tx_a.compute(&key).await.unwrap().unwrap();
    assert!(Arc::ptr_eq(&first.capture, &a));
    assert!(Arc::ptr_eq(
        &first,
        &tx_a.compute(&key).await.unwrap().unwrap()
    ));
    let mut b_raw = raw();
    b_raw.user = ObservedText::Text("B".into());
    b_raw.uid = Ok((456, 123));
    b_raw.millis = Ok(1000);
    let b = capture(&workspace, 2, b_raw);
    let mut tx_b = raw_transaction(&dice, &workspace, Some(&b), Some(&b)).await;
    let second = tx_b.compute(&key).await.unwrap().unwrap();
    assert_ne!(first, second);
    assert_eq!(first.stable, stable_bytes("reported user", "kernel"));
    assert_eq!(second.stable, stable_bytes("B", "kernel"));
    assert!(Arc::ptr_eq(
        &first,
        &tx_a.compute(&key).await.unwrap().unwrap()
    ));
    let mut tx_restored = raw_transaction(&dice, &workspace, Some(&a), Some(&a)).await;
    let restored = tx_restored.compute(&key).await.unwrap().unwrap();
    assert_eq!(first, restored);
    assert!(Arc::ptr_eq(&restored.capture, &a));
    assert!(Arc::ptr_eq(
        &second,
        &tx_b.compute(&key).await.unwrap().unwrap()
    ));
    let artifacts = tx_restored
        .compute(&slug_analysis_v2::WorkspaceStatusArtifactsKey(
            workspace.dupe(),
        ))
        .await
        .unwrap();
    assert!(Arc::ptr_eq(&restored.artifacts, &artifacts));
    for (revision, mutate) in [(3, 0), (4, 1), (5, 2)] {
        let mut input = raw();
        match mutate {
            0 => input.uid = Ok((999, 456)),
            1 => input.source = StatusSource::Unsupported,
            _ => input.policy = StatusPolicy::UnsupportedCommand,
        }
        let changed = capture(&workspace, revision, input);
        let mut tx = raw_transaction(&dice, &workspace, Some(&changed), Some(&changed)).await;
        let value = tx.compute(&key).await.unwrap();
        if mutate == 0 {
            let value = value.unwrap();
            assert_eq!(value.stable, first.stable);
            assert_ne!(value, first);
            assert!(!Arc::ptr_eq(&value, &first));
        } else {
            assert_eq!(
                value.unwrap_err(),
                if mutate == 1 {
                    StatusError::UnsupportedPlatform
                } else {
                    StatusError::UnsupportedPolicy
                }
            );
        }
        let mut tx = raw_transaction(&dice, &workspace, Some(&a), Some(&a)).await;
        assert_eq!(tx.compute(&key).await.unwrap().unwrap(), first);
    }
    let same_payload_new_request = capture(&workspace, 6, raw());
    let mut tx = raw_transaction(
        &dice,
        &workspace,
        Some(&same_payload_new_request),
        Some(&same_payload_new_request),
    )
    .await;
    let fresh = tx.compute(&key).await.unwrap().unwrap();
    assert_eq!(fresh.stable, first.stable);
    assert_eq!(fresh.volatile, first.volatile);
    assert_ne!(fresh, first);
    let foreign = Arc::new(same_payload_new_request.as_ref().clone());
    let foreign_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let mut wrong = raw_transaction(
        &foreign_dice,
        &workspace,
        Some(&same_payload_new_request),
        Some(&foreign),
    )
    .await;
    let error = wrong.compute(&key).await.unwrap();
    assert_eq!(error, Err(StatusError::ForeignCapture));
    assert!(!<WorkspaceStatusContentKey as Key>::validity(&error));
    let mut revoked = raw_transaction(&dice, &workspace, None, None).await;
    assert_eq!(
        revoked.compute(&key).await.unwrap(),
        Err(StatusError::Unauthorized)
    );
    let another = NormalizedAbsolutePath::new("/foreign").unwrap();
    let mut updater = dice.updater();
    assert!(inject(&mut updater, &another, Some(&a)).is_err());
    assert_eq!(
        produce(a.clone(), Arc::new(WorkspaceStatusArtifacts::new(another))),
        Err(StatusError::ForeignCapture)
    );
}

fn native_keys(runtime: &WorkspaceRuntime) -> Arc<[BuildInfoNativeInputsKey]> {
    let targets = [TargetPattern::parse("//:info").unwrap()];
    let mut keys = Vec::new();
    for changed in [false, true] {
        let overlay: CommandConfigurationOverlay = if changed {
            [
                slug_configuration_v2::NativeCommandOption::Copt,
                slug_configuration_v2::NativeCommandOption::HostCopt,
            ]
            .into_iter()
            .map(|option| {
                slug_configuration_v2::CommandConfigurationOccurrence::native(
                    option,
                    Some("-DSTATUS_B"),
                    false,
                )
            })
            .collect::<Vec<_>>()
            .into()
        } else {
            CommandConfigurationOverlay::default()
        };
        let bundle = native_request(runtime);
        let registries = [format!(
            "file://{}/empty-registry",
            runtime.workspace.display()
        )];
        let accepted = runtime
            .build_command_with_repository_environment(
                &targets,
                bundle.command_policy,
                bundle.environment_policy,
                bundle.lockfile_mode,
                &registries,
                bundle.repository_environment,
                overlay.clone(),
            )
            .unwrap();
        let owner = build_info_owner(accepted.terminal_for_test().as_ref().as_ref().unwrap())
            .configured_target_key()
            .unwrap()
            .clone();
        let bundle = native_request(runtime);
        let (build, _) = runtime
            .prepare_build_request(
                &targets,
                bundle.command_policy,
                bundle.environment_policy,
                bundle.lockfile_mode,
                &registries,
                bundle.repository_environment,
                overlay,
            )
            .unwrap();
        for ordinal in [0, 1] {
            keys.push(BuildInfoNativeInputsKey(
                BuildInfoTemplatePreparationKey::new(build.clone(), owner.clone(), ordinal),
            ));
        }
    }
    keys.into()
}

#[derive(Debug, Default)]
struct PendingStatusGate {
    entered: std::sync::atomic::AtomicBool,
    notification: tokio::sync::Notify,
    worker: Mutex<Option<std::sync::Weak<()>>>,
    input: Mutex<Option<std::sync::Weak<BuildInfoNativeInputs>>>,
}

#[derive(Debug, Clone, Allocative)]
struct PendingStatusKey {
    original: BuildInfoNativeInputsKey,
    #[allocative(skip)]
    gate: Arc<PendingStatusGate>,
}
impl PartialEq for PendingStatusKey {
    fn eq(&self, other: &Self) -> bool {
        self.original == other.original && Arc::ptr_eq(&self.gate, &other.gate)
    }
}
impl Eq for PendingStatusKey {}
impl std::hash::Hash for PendingStatusKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.original.hash(state);
        Arc::as_ptr(&self.gate).hash(state);
    }
}
impl fmt::Display for PendingStatusKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "pending-status:{}:{:p}",
            self.original,
            Arc::as_ptr(&self.gate)
        )
    }
}
#[async_trait]
impl Key for PendingStatusKey {
    type Value = <BuildInfoNativeInputsKey as Key>::Value;
    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        ctx.store_evaluation_data(EventBatch::empty()).unwrap();
        let outcome = ctx.compute(&self.original).await.unwrap();
        let PreparationOutcome::Complete(Ok(input)) = &outcome else {
            return outcome;
        };
        let worker_owner = Arc::new(());
        *self.gate.worker.lock().unwrap() = Some(Arc::downgrade(&worker_owner));
        *self.gate.input.lock().unwrap() = Some(Arc::downgrade(input));
        self.gate
            .entered
            .store(true, std::sync::atomic::Ordering::SeqCst);
        self.gate.notification.notify_one();
        std::future::pending::<()>().await;
        std::hint::black_box(&worker_owner);
        outcome
    }
    fn equality(_: &Self::Value, _: &Self::Value) -> bool {
        false
    }
    fn validity(_: &Self::Value) -> bool {
        false
    }
}

#[derive(Clone)]
struct StatusRoot {
    keys: Arc<[BuildInfoNativeInputsKey]>,
    attempts: Arc<Mutex<Vec<Arc<WorkspaceStatusContent>>>>,
    mutation: Arc<Mutex<Option<(PathBuf, Vec<u8>)>>>,
    stale_capture: Option<Arc<WorkspaceStatusCapture>>,
    copied_capture: bool,
    cancel: bool,
    pending: Arc<PendingStatusGate>,
}
impl StatusRoot {
    fn new(keys: Arc<[BuildInfoNativeInputsKey]>) -> Self {
        Self {
            keys,
            attempts: Arc::new(Mutex::new(Vec::new())),
            mutation: Arc::new(Mutex::new(None)),
            stale_capture: None,
            copied_capture: false,
            cancel: false,
            pending: Arc::new(PendingStatusGate::default()),
        }
    }
}
#[async_trait]
impl NativeCommandRoot for StatusRoot {
    type Terminal = Vec<Arc<BuildInfoNativeInputs>>;
    fn requires_workspace_status_inputs(&self) -> bool {
        true
    }
    fn initializes_request_revision(&self) -> bool {
        true
    }
    fn observations<'a>(&self, value: &'a Self::Terminal) -> Option<&'a PathObservationEpoch> {
        Some(value[0].template.observations())
    }
    fn source_certificate<'a>(&self, value: &'a Self::Terminal) -> Option<&'a SourceCertificate> {
        Some(value[0].template.certificate())
    }
    fn observed_selection_association(&self) -> ObservedSelectionAssociation {
        ObservedSelectionAssociation::SelectedDependencySuperset
    }
    fn event_reconciliation_policy(&self, _: &Self::Terminal) -> EventReconciliationPolicy {
        EventReconciliationPolicy::SourceCertifiedCurrentClosure
    }
    fn complete<'a>(
        &'a self,
        value: &'a mut Self::Terminal,
        context: NativeCommandCompletionContext<'a, '_>,
    ) -> impl std::future::Future<Output = Result<(), NativeDemandSessionError>> + 'a {
        async move {
            assert!(matches!(
                context
                    .command
                    .runtime
                    .begin_native_demand_command_with_inputs(
                        NativeDemandRequestInputBundle::normalized_initial()
                    ),
                Err(NativeDemandSessionError::Busy)
            ));
            for input in value.iter() {
                validate_capture(&input.status.capture, &context).await?;
            }
            if self.copied_capture {
                validate_capture(
                    &Arc::new(value[0].status.capture.as_ref().clone()),
                    &context,
                )
                .await?;
            }
            if let Some(stale) = &self.stale_capture {
                validate_capture(stale, &context).await?;
            }
            NativeCommandRoot::complete(&self.keys[0], &mut value[0], context).await
        }
    }
    async fn compute(
        &self,
        tx: &mut dice::DiceTransaction,
    ) -> Result<PreparationOutcome<Self::Terminal>, NativeDemandSessionError> {
        let status = tx
            .compute(&WorkspaceStatusContentKey(
                self.keys[0].0.workspace().dupe(),
            ))
            .await
            .map_err(|error| NativeDemandSessionError::Computation(anyhow::anyhow!(error)))?
            .map_err(|error| NativeDemandSessionError::Computation(anyhow::anyhow!(error)))?;
        self.attempts.lock().unwrap().push(status);
        let mut values = Vec::new();
        for key in self.keys.iter() {
            match NativeCommandRoot::compute(key, tx).await? {
                PreparationOutcome::Need(need) => return Ok(PreparationOutcome::Need(need)),
                PreparationOutcome::Complete(value) => values.push(value),
            }
        }
        if self.cancel {
            let key = PendingStatusKey {
                original: self.keys[0].clone(),
                gate: self.pending.clone(),
            };
            let mut waiter = Box::pin(tx.compute(&key));
            tokio::time::timeout(std::time::Duration::from_secs(2), async {
                tokio::select! {
                    result = &mut waiter => panic!("pending actual status worker unexpectedly completed: {result:?}"),
                    _ = self.pending.notification.notified() => {}
                }
            }).await.expect("actual DICE status worker reaches its pending gate");
            assert!(
                self.pending
                    .entered
                    .load(std::sync::atomic::Ordering::SeqCst)
            );
            drop(waiter);
            return Err(NativeDemandSessionError::Computation(anyhow::anyhow!(
                "actual status root cancelled after content acquisition"
            )));
        }
        let mutation = self.mutation.lock().unwrap().take();
        if let Some((path, bytes)) = mutation {
            std::fs::write(path, bytes).unwrap();
        }
        Ok(PreparationOutcome::Complete(values))
    }
}
fn revoked(runtime: &WorkspaceRuntime) {
    runtime.runtime.block_on(async {
        let mut tx = runtime
            .dice
            .updater_with_data(runtime.user_computation_data(None).unwrap())
            .existing_state()
            .await;
        runtime.request_revision.bind_cycle_detector(&tx);
        let workspace = NormalizedAbsolutePath::new(runtime.workspace.clone()).unwrap();
        assert_eq!(
            tx.compute(&WorkspaceStatusCaptureKey(workspace.dupe()))
                .await
                .unwrap(),
            WorkspaceStatusCaptureValue::Unauthorized
        );
        assert_eq!(
            tx.compute(&WorkspaceStatusContentKey(workspace))
                .await
                .unwrap(),
            Err(StatusError::Unauthorized)
        );
    });
}
fn assert_snapshot_same(runtime: &WorkspaceRuntime, before: &AcceptedNativeDemandSnapshot) {
    let after = runtime
        .native_demand_sessions
        .state
        .lock()
        .unwrap()
        .accepted
        .clone();
    assert_eq!(before.inputs, after.inputs);
    assert_eq!(before.path_observations, after.path_observations);
    assert_eq!(before.repository_results, after.repository_results);
    assert_eq!(
        before.repository_environment_frontier,
        after.repository_environment_frontier
    );
    assert_eq!(before.download_results, after.download_results);
    assert_eq!(before.selected, after.selected);
}
fn assert_attempt_sharing(root: &StatusRoot) {
    let attempts = root.attempts.lock().unwrap();
    assert!(
        attempts.len() > 1,
        "the finite cold or modified frontier must actually retry"
    );
    for attempt in attempts.iter() {
        assert!(Arc::ptr_eq(attempt, &attempts[0]));
        assert!(Arc::ptr_eq(&attempt.capture, &attempts[0].capture));
    }
}

#[test]
fn genuine_native_status_shares_capture_across_configurations_renders_complete_headers_and_releases()
 {
    let workspace = status_workspace();
    let runtime =
        WorkspaceRuntime::new(workspace.path(), crate::runtime::ProcessHostOwner::native())
            .unwrap();
    let keys = native_keys(&runtime);
    revoked(&runtime); // Ordinary preparation did not acquire status authority.
    let root = StatusRoot::new(keys);
    let user = std::env::var("USER").unwrap();
    let kernel = nix::unistd::gethostname().unwrap().into_string().unwrap();
    let host = kernel.split('.').next().unwrap();
    let before = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let driven = runtime
        .drive_command(native_request(&runtime), root.clone())
        .unwrap();
    let after = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let held = driven.accepted.terminal_for_test().clone();
    let status = &held[0].status;
    assert_eq!(
        status.capture.raw.source,
        StatusSource::UnixReportedUserKernelHost
    );
    assert_eq!(status.capture.raw.policy, StatusPolicy::DefaultEnglishUtc);
    assert_eq!(
        status.capture.raw.user,
        ObservedText::Text(user.clone().into())
    );
    assert_eq!(
        status.capture.raw.hostname,
        ObservedText::Text(kernel.clone().into())
    );
    assert_eq!(
        status.capture.raw.uid,
        Ok((
            nix::unistd::getuid().as_raw(),
            nix::unistd::geteuid().as_raw()
        ))
    );
    assert_eq!(
        status.capture.raw.source_date_epoch,
        ObservedText::from_os(std::env::var_os("SOURCE_DATE_EPOCH"))
    );
    assert_eq!(status.capture.raw.source_date_epoch, ObservedText::Absent);
    let millis = status.capture.raw.millis.unwrap();
    assert!(before <= u128::from(millis) && u128::from(millis) <= after);
    let seconds = i128::from(millis / 1000);
    let expected_stable = stable_bytes(&user, host);
    let expected_volatile = volatile_bytes(seconds, &independent_date(seconds));
    assert_eq!(status.stable, expected_stable);
    assert_eq!(status.volatile, expected_volatile);
    assert_ne!(status.stable.as_ptr(), status.volatile.as_ptr());
    let snapshot = runtime
        .native_demand_sessions
        .state
        .lock()
        .unwrap()
        .accepted
        .clone();
    for (index, input) in held.iter().enumerate() {
        assert!(Arc::ptr_eq(status, &input.status));
        assert_eq!(input.header, header_bytes(index % 2, &user, host, seconds));
        assert_ne!(input.header.as_ptr(), input.status.stable.as_ptr());
        assert_ne!(input.header.as_ptr(), input.status.volatile.as_ptr());
        for (demand, result) in input.template.observations().observations() {
            assert!(Arc::ptr_eq(
                result,
                snapshot.path_observations.get(demand).unwrap()
            ));
            assert!(Arc::ptr_eq(
                result,
                input
                    .template
                    .certificate()
                    .observations()
                    .get(demand)
                    .unwrap()
            ));
        }
    }
    assert_ne!(
        held[0].template.action().context().owner(),
        held[2].template.action().context().owner()
    );
    revoked(&runtime);
    let status_weak = Arc::downgrade(status);
    let capture_weak = Arc::downgrade(&status.capture);
    let input_weak = Arc::downgrade(&held[0]);
    drop(snapshot);
    drop(driven);
    drop(root);
    drop(runtime);
    assert!(status_weak.upgrade().is_some());
    assert!(capture_weak.upgrade().is_some());
    assert_eq!(held[0].status.stable, expected_stable);
    assert_eq!(held[0].status.volatile, expected_volatile);
    for (index, input) in held.iter().enumerate() {
        assert_eq!(input.header, header_bytes(index % 2, &user, host, seconds));
    }
    drop(held);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while (status_weak.upgrade().is_some()
        || capture_weak.upgrade().is_some()
        || input_weak.upgrade().is_some())
        && std::time::Instant::now() < deadline
    {
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(status_weak.upgrade().is_none());
    assert!(capture_weak.upgrade().is_none());
    assert!(input_weak.upgrade().is_none());
}

#[test]
fn native_status_retry_new_request_cancellation_and_foreign_capture_refuse_without_publication() {
    let workspace = status_workspace();
    let runtime =
        WorkspaceRuntime::new(workspace.path(), crate::runtime::ProcessHostOwner::native())
            .unwrap();
    let keys = native_keys(&runtime);
    let root = StatusRoot::new(keys.clone());
    let build_path = workspace.path().join("BUILD.bazel");
    let mut changed = std::fs::read(&build_path).unwrap();
    changed.extend_from_slice(b"# status certified retry\n");
    *root.mutation.lock().unwrap() = Some((build_path, changed));
    let first = runtime
        .drive_command(native_request(&runtime), root.clone())
        .unwrap();
    assert_attempt_sharing(&root);
    let held = first.accepted.terminal_for_test()[0].clone();
    let previous_stable = held.status.stable.clone();
    let previous_volatile = held.status.volatile.clone();
    let previous_header = held.header.clone();
    assert!(held.template.observations().observations().values().any(|result| matches!(result.as_ref(), PathObservationResult::FileBytes(PathOperationResult::Present(bytes)) if bytes.ends_with(b"# status certified retry\n"))));
    revoked(&runtime);
    let next_root = StatusRoot::new(keys.clone());
    let next = runtime
        .drive_command(native_request(&runtime), next_root)
        .unwrap();
    let fresh = &next.accepted.terminal_for_test()[0];
    assert!(!Arc::ptr_eq(&held.status.capture, &fresh.status.capture));
    assert_ne!(held.status.capture.request, fresh.status.capture.request);
    assert_ne!(held.status, fresh.status);
    assert_eq!(held.status.stable, previous_stable);
    assert_eq!(held.status.volatile, previous_volatile);
    assert_eq!(held.header, previous_header);
    let before = runtime
        .native_demand_sessions
        .state
        .lock()
        .unwrap()
        .accepted
        .clone();
    for copied in [false, true] {
        let mut rejected = StatusRoot::new(keys.clone());
        if copied {
            rejected.copied_capture = true;
        } else {
            rejected.stale_capture = Some(held.status.capture.clone());
        }
        let error = runtime
            .drive_command(native_request(&runtime), rejected)
            .err()
            .unwrap();
        assert!(error.to_string().contains("ForeignCapture"), "{error}");
        assert_snapshot_same(&runtime, &before);
        revoked(&runtime);
    }
    let mut cancelled = StatusRoot::new(keys.clone());
    cancelled.cancel = true;
    assert!(matches!(
        runtime.drive_command(native_request(&runtime), cancelled.clone()),
        Err(NativeDemandSessionError::Computation(_))
    ));
    assert_snapshot_same(&runtime, &before);
    revoked(&runtime);
    let pending_status = Arc::downgrade(cancelled.attempts.lock().unwrap().last().unwrap());
    let pending_gate = cancelled.pending.clone();
    assert!(
        pending_gate
            .entered
            .load(std::sync::atomic::Ordering::SeqCst)
    );
    assert!(
        pending_gate
            .worker
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .upgrade()
            .is_none(),
        "the driver drains the cancelled DICE worker before abort returns"
    );
    let pending_input = pending_gate.input.lock().unwrap().as_ref().unwrap().clone();
    drop(cancelled);
    let recovered = runtime
        .drive_command(native_request(&runtime), StatusRoot::new(keys))
        .unwrap();
    assert_ne!(
        held.status.capture.request,
        recovered.accepted.terminal_for_test()[0]
            .status
            .capture
            .request
    );
    revoked(&runtime);
    drop(recovered);
    drop(next);
    drop(first);
    drop(root);
    drop(before);
    drop(held);
    drop(runtime);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while (pending_status.upgrade().is_some() || pending_input.upgrade().is_some())
        && std::time::Instant::now() < deadline
    {
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(pending_status.upgrade().is_none());
    assert!(pending_input.upgrade().is_none());
}

// Run this exact selector in a separate process with SOURCE_DATE_EPOCH=''.
// The test itself never mutates global environment state.
#[test]
fn genuine_native_present_empty_source_date_epoch_refuses_and_revokes_capture() {
    assert_eq!(
        std::env::var_os("SOURCE_DATE_EPOCH"),
        Some(std::ffi::OsString::new())
    );
    let workspace = status_workspace();
    let runtime =
        WorkspaceRuntime::new(workspace.path(), crate::runtime::ProcessHostOwner::native())
            .unwrap();
    let keys = native_keys(&runtime);
    let before = runtime
        .native_demand_sessions
        .state
        .lock()
        .unwrap()
        .accepted
        .clone();
    let error = runtime
        .drive_command(native_request(&runtime), keys[0].clone())
        .err()
        .unwrap();
    assert!(
        error.to_string().contains("SourceDateEpochPresent"),
        "{error}"
    );
    assert_snapshot_same(&runtime, &before);
    revoked(&runtime);
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
mod publication {
    use super::*;
    include!("workspace_status_publication_tests.rs");
}
