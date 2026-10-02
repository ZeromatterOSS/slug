use std::sync::Condvar;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use dice::UserCycleDetector;
use dice::UserCycleDetectorGuard;
use slug_bzlmod_v2::HostRootRepositoryMappingObservationKey;
use slug_bzlmod_v2::HostSelectedExtensionDemandObservationKey;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_loading_v2::HostSelectedExtensionOwnerCertificateObservationKey;

fn bounded_outcome_debug(value: &impl std::fmt::Debug) -> String {
    struct Output {
        text: String,
        remaining: usize,
        truncated: bool,
    }
    impl std::fmt::Write for Output {
        fn write_str(&mut self, text: &str) -> std::fmt::Result {
            if let Some((end, _)) = text.char_indices().nth(self.remaining) {
                self.text.push_str(&text[..end]);
                self.truncated = true;
                Err(std::fmt::Error)
            } else {
                self.text.push_str(text);
                self.remaining -= text.chars().count();
                Ok(())
            }
        }
    }
    let mut output = Output {
        text: String::new(),
        remaining: 1536,
        truncated: false,
    };
    let _ignored = std::fmt::write(&mut output, format_args!("{value:?}"));
    if output.truncated {
        output.text.push_str("...");
    }
    output.text
}

async fn prepare_shared_certificate_keys(
    transaction: &mut dice::DiceTransaction,
    workspace: &NormalizedAbsolutePath,
) -> SourcePreparationOutcome<Vec<HostSelectedExtensionOwnerCertificateObservationKey>> {
    let mapping = match transaction
        .compute(&HostRootRepositoryMappingObservationKey::new(
            workspace.dupe(),
        ))
        .await
        .unwrap()
    {
        SourcePreparationOutcome::Need(need) => return SourcePreparationOutcome::Need(need),
        SourcePreparationOutcome::Complete(mapping) => mapping,
    };
    let mapping = mapping
        .as_ref()
        .unwrap()
        .result()
        .as_ref()
        .as_ref()
        .unwrap()
        .view()
        .unwrap();
    let mut keys = Vec::new();
    for apparent in ["proof", "tools"] {
        let canonical = mapping
            .mapping()
            .find(|(name, _)| name.as_str() == apparent)
            .unwrap()
            .1
            .clone();
        let demand = match transaction
            .compute(&HostSelectedExtensionDemandObservationKey::new(
                workspace.dupe(),
                canonical,
            ))
            .await
            .unwrap()
        {
            SourcePreparationOutcome::Need(need) => return SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(demand) => demand,
        };
        keys.push(HostSelectedExtensionOwnerCertificateObservationKey::new(
            workspace.dupe(),
            demand
                .as_ref()
                .unwrap()
                .result()
                .as_ref()
                .as_ref()
                .unwrap()
                .owner()
                .clone(),
        ));
    }
    for (index, key) in keys.iter().enumerate() {
        match transaction.compute(key).await.unwrap() {
            SourcePreparationOutcome::Need(need) => return SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Ok(certificate)) => {
                let error = certificate.result().as_ref().as_ref().unwrap_err();
                assert!(
                    error
                        .to_string()
                        .contains("cycle detected in generated repository dependencies"),
                    "preparation root {index}: {error:?}"
                );
            }
            outcome => panic!(
                "preparation root {index} must return a complete typed cycle: {}",
                bounded_outcome_debug(&outcome)
            ),
        }
    }
    SourcePreparationOutcome::Complete(keys)
}

struct RootGate {
    entered: tokio::sync::Semaphore,
    released: Mutex<bool>,
    wake: Condvar,
}

impl RootGate {
    fn release(&self) {
        *self.released.lock().unwrap() = true;
        self.wake.notify_all();
    }
}

struct GatedDetector {
    delegate: Arc<dyn UserCycleDetector>,
    target: HostSelectedExtensionOwnerCertificateObservationKey,
    gate: Arc<RootGate>,
    started: AtomicBool,
}

impl UserCycleDetector for GatedDetector {
    fn start_computing_key(&self, key: &DynKey) -> Option<Arc<dyn UserCycleDetectorGuard>> {
        let guard = self.delegate.start_computing_key(key);
        if key.downcast_ref::<HostSelectedExtensionOwnerCertificateObservationKey>()
            == Some(&self.target)
            && !self.started.swap(true, Ordering::SeqCst)
        {
            assert!(
                guard.is_some(),
                "the genuine certificate must install its graph guard"
            );
            // Both roots must belong to different requests before either root can
            // discover its dependency. The delegated graph lock is already released.
            self.gate.entered.add_permits(1);
            tokio::task::block_in_place(|| {
                let (released, timeout) = self
                    .gate
                    .wake
                    .wait_timeout_while(
                        self.gate.released.lock().unwrap(),
                        Duration::from_secs(2),
                        |released| !*released,
                    )
                    .unwrap();
                let was_released = *released;
                drop(released);
                assert!(
                    was_released && !timeout.timed_out(),
                    "root barrier was not released"
                );
            });
        }
        guard
    }

    fn finished_computing_key(&self, key: &DynKey) {
        self.delegate.finished_computing_key(key);
    }
}

struct GateCleanup {
    gate: Arc<RootGate>,
    requests: Vec<tokio::task::AbortHandle>,
    attempts: Vec<Arc<AttemptEffectTracker>>,
}

struct RuntimePumpStop(Option<tokio::sync::oneshot::Sender<()>>);

impl Drop for RuntimePumpStop {
    fn drop(&mut self) {
        if let Some(stop) = self.0.take() {
            let _ignored = stop.send(());
        }
    }
}

impl Drop for GateCleanup {
    fn drop(&mut self) {
        self.gate.release();
        for request in &self.requests {
            request.abort();
        }
        for attempt in &self.attempts {
            let _ignored = attempt.finish_suppressed();
        }
    }
}

#[test]
fn reciprocal_owner_cycle_spans_two_independently_bound_request_proxies() {
    let fixture = GeneratedPathFixture::new();
    let source = fs::read_to_string(fixture.workspace.path().join("MODULE.bazel")).unwrap();
    fs::write(
        fixture.workspace.path().join("MODULE.bazel"),
        format!("{source}b.path(file='@proof//:value')\n"),
    )
    .unwrap();
    // The ordinary driver owns source observations/preparation. Its failed
    // invocation publishes neither consumer effects nor a completion event.
    let prepared = fixture.run("//:proof");
    let error = prepared.terminal_for_test().as_ref().as_ref().unwrap_err();
    assert!(
        error
            .to_string()
            .contains("cycle detected in generated repository dependencies"),
        "{error:?}"
    );
    assert_eq!(consumer_completion_count(&prepared), 0);
    assert!(fixture.generated_roots().is_empty());
    let prior_events = accepted_native_snapshot(&fixture.runtime).events;

    let request = NativeDemandRequestInputBundle {
        command_policy: fixture.policy.clone(),
        environment_policy: BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None)
            .unwrap(),
        lockfile_mode: LockfileMode::Update,
        registry_urls: RegistryUrls::from_request(&fixture.runtime.workspace, &fixture.registries)
            .unwrap(),
        repository_environment: slug_bzlmod_v2::RepositoryEnvironmentSnapshot::empty(),
        runtime_sources: None,
    };
    let mut command = NativeDemandAbortGuard::new(
        fixture
            .runtime
            .begin_native_demand_command_with_inputs(request)
            .unwrap()
            .into_command(),
    );
    // Retain preparation in this native command's owned snapshot instead of
    // assuming a prior failed command published its defining-source closure.
    let workspace = NormalizedAbsolutePath::new(fixture.runtime.workspace.clone()).unwrap();
    let preparation_started = std::time::Instant::now();
    let mut prepared_keys = None;
    for round in 0..16 {
        command.begin_attempt().unwrap();
        let remaining = Duration::from_secs(5)
            .checked_sub(preparation_started.elapsed())
            .expect("source-owned preparation exceeded five seconds");
        let outcome = fixture.runtime.runtime.block_on(async {
            tokio::time::timeout(remaining, async {
                let mut updater = fixture
                    .runtime
                    .dice
                    .updater_with_data(command.attempt_user_computation_data().unwrap());
                command.inject_attempt(&mut updater).unwrap();
                let mut transaction = fixture.runtime.request_revision.commit(updater).await;
                let outcome = prepare_shared_certificate_keys(&mut transaction, &workspace).await;
                match &outcome {
                    SourcePreparationOutcome::Need(_) => command.seal_retry().unwrap(),
                    SourcePreparationOutcome::Complete(_) => command.suppress_attempt().unwrap(),
                }
                drop(transaction);
                fixture.runtime.dice.wait_for_idle().await;
                outcome
            })
            .await
            .unwrap_or_else(|_| {
                panic!("source-owned preparation round {round} exceeded five seconds")
            })
        });
        match outcome {
            SourcePreparationOutcome::Complete(keys) => {
                prepared_keys = Some(keys);
                break;
            }
            SourcePreparationOutcome::Need(needs) => {
                assert!(needs.root_module_bootstrap_request().is_none(), "{needs:?}");
                assert!(needs.repository_environment().is_none(), "{needs:?}");
                assert!(needs.repository_materializations().is_empty(), "{needs:?}");
                assert!(needs.selected_extension_downloads().is_empty(), "{needs:?}");
                assert!(needs.repository_executes().is_empty(), "{needs:?}");
                assert!(needs.path_observations().is_some(), "{needs:?}");
                command.progress(&needs).unwrap(); // Normal owned observation, outside block_on.
            }
        }
    }
    let keys =
        prepared_keys.expect("source-owned path preparation must finish within sixteen rounds");
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    // The workspace retains a current-thread runtime. Its canceled predecessors
    // and legacy detector actors still require polling while the independent
    // request roots run on this separate multithread runtime.
    std::thread::scope(|scope| {
        let (stop, stopped) = tokio::sync::oneshot::channel();
        let retained_runtime = &fixture.runtime.runtime;
        scope.spawn(move || {
            retained_runtime.block_on(async {
                let _ignored = stopped.await;
            });
        });
        // Drop signals before the scoped thread joins, including on failure.
        let _stop = RuntimePumpStop(Some(stop));
        runtime.block_on(async {
        let stage = std::cell::Cell::new("committing independently bound root transactions");
        tokio::time::timeout(Duration::from_secs(5), async {
            let gate = Arc::new(RootGate {
                entered: tokio::sync::Semaphore::new(0),
                released: Mutex::new(false),
                wake: Condvar::new(),
            });
            let mut cleanup = GateCleanup {
                gate: gate.clone(),
                requests: Vec::new(),
                attempts: Vec::new(),
            };
            let mut transactions = Vec::new();
            let mut proxies = Vec::new();
            let mut wrappers = Vec::new();
            for key in keys {
                stage.set("committing independently bound root transaction");
                // Each request installs its own effect tracker exactly once.
                // The native command still owns the sole shared input snapshot.
                let attempt = CommandEffectOwner::new().begin_attempt().unwrap();
                let native = command.command();
                assert!(native.download_results.is_empty());
                assert!(native.execute_transcript.records().is_empty());
                let mut data = fixture.runtime.user_computation_data_with_repository_host_inputs(
                    Some(attempt.clone()),
                    native.inputs.request.repository_environment.clone(),
                    native.repository_environment_frontier.clone(),
                ).unwrap();
                install_selected_extension_download_transaction(
                    &mut data,
                    &fixture.runtime.workspace,
                    &native.download_results,
                ).unwrap();
                data.data.set(RepositoryExecuteEpoch::active(native.lease.0, native.execute_transcript.dupe()));
                assert!(data.data.get::<CaptureEvaluationEvents>().is_ok());
                cleanup.attempts.push(attempt);
                let proxy = data.cycle_detector.as_ref().unwrap().clone();
                proxies.push(proxy.clone());
                let wrapper = Arc::new(GatedDetector {
                    delegate: proxy,
                    target: key.clone(),
                    gate: gate.clone(),
                    started: AtomicBool::new(false),
                });
                data.cycle_detector = Some(wrapper.clone());
                wrappers.push(wrapper);
                let mut updater = fixture.runtime.dice.updater_with_data(data);
                command.inject_attempt(&mut updater).unwrap();
                transactions.push((fixture.runtime.request_revision.commit(updater).await, key));
            }
            assert_eq!(
                transactions[0].0.live_context_id(),
                transactions[1].0.live_context_id()
            );
            assert!(
                !Arc::ptr_eq(&proxies[0], &proxies[1]),
                "each request owns a fresh proxy"
            );
            assert!(!Arc::ptr_eq(&cleanup.attempts[0], &cleanup.attempts[1]));
            stage.set("starting and completing the two genuine roots");
            tokio::time::timeout(Duration::from_secs(2), async {
            let mut tasks = Vec::new();
            for (mut transaction, key) in transactions {
                let task = tokio::spawn(async move { transaction.compute(&key).await });
                cleanup.requests.push(task.abort_handle());
                tasks.push(task);
            }
            for _ in 0..2 {
                gate.entered.acquire().await.unwrap().forget();
            }
            assert!(
                wrappers
                    .iter()
                    .all(|wrapper| wrapper.started.load(Ordering::SeqCst))
            );
            gate.release();
            stage.set("awaiting reciprocal certificate cycle terminals");
            for (index, task) in tasks.into_iter().enumerate() {
                let certificate = match task.await.unwrap().unwrap() {
                    SourcePreparationOutcome::Complete(Ok(certificate)) => certificate,
                    outcome => panic!(
                        "source-owned certificate root {index} must return a complete typed cycle terminal: {}",
                        bounded_outcome_debug(&outcome),
                    ),
                };
                let error = certificate.result().as_ref().as_ref().unwrap_err();
                assert!(
                    error
                        .to_string()
                        .contains("cycle detected in generated repository dependencies"),
                    "{error:?}"
                );
            }
            stage.set("retiring genuine-root workers");
            fixture.runtime.dice.wait_for_idle().await;
            for attempt in cleanup.attempts.drain(..) {
                attempt.finish_suppressed().unwrap();
            }
            })
            .await
            .unwrap_or_else(|_| {
                panic!(
                    "two request-owned roots must close their shared cycle within two seconds; stage={}, root starts={:?}",
                    stage.get(),
                    wrappers.iter().map(|wrapper| wrapper.started.load(Ordering::SeqCst)).collect::<Vec<_>>()
                )
            });
        })
        .await
        .unwrap_or_else(|_| panic!("shared-root preparation/cleanup exceeded five seconds; stage={}", stage.get()));
        });
    });
    drop(runtime);
    // AbortGuard restores/closes the single native input lease after all DICE
    // workers retire; each transaction above had separate userdata and proxy.
    drop(command);
    assert_eq!(
        accepted_native_snapshot(&fixture.runtime).events,
        prior_events
    );
    assert!(fixture.generated_roots().is_empty());
}
