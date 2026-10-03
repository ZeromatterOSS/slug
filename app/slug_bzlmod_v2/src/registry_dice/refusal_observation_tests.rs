// These controls use the ordinary producer and the existing owned input fixture.
struct TypedRefusalIo {
    cause: Mutex<Option<RegistryRefusalCause>>,
    calls: AtomicUsize,
}
#[async_trait]
impl RegistryIo for TypedRefusalIo {
    async fn read_exact(
        &self,
        _: &RegistryFileUrl,
    ) -> Result<RegistryIoOutcome, RegistryTransportError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err(RegistryTransportError {
            message: "same transport message".into(),
            cause: *self.cause.lock().unwrap(),
        })
    }
}
fn typed_refusal_cause(site: RegistryRefusalSite) -> RegistryRefusalCause {
    RegistryRefusalCause {
        site,
        detail: RegistryRefusalDetail::Opaque,
    }
}

#[tokio::test]
async fn normal_registry_file_retains_current_typed_cause_and_equal_errors_cut_off() {
    let a = typed_refusal_cause(RegistryRefusalSite::Dns);
    let b = typed_refusal_cause(RegistryRefusalSite::ConnectExhausted);
    let io = Arc::new(TypedRefusalIo {
        cause: Mutex::new(Some(a)),
        calls: AtomicUsize::new(0),
    });
    let mut builder = Dice::builder();
    install_registry_io(&mut builder, io.clone());
    let dice = builder.build(DetectCycles::Enabled);
    let key = RegistryFileKey {
        workspace: PathBuf::from(WORKSPACE),
        url: RegistryFileUrl::new("https://registry/file"),
    };
    let mut retained = Vec::new();
    for (generation, cause) in [
        (1, Some(a)),
        (2, Some(a)),
        (3, Some(b)),
        (4, None),
        (5, Some(a)),
    ] {
        *io.cause.lock().unwrap() = cause;
        let mut updater = dice.updater();
        inject(
            &mut updater,
            MODULE,
            LOCKFILE_A,
            LockfileMode::Off,
            REGISTRY_A,
            1,
            true,
            false,
        );
        updater
            .changed_to([(
                RegistryRequestGenerationKey {
                    workspace: PathBuf::from(WORKSPACE),
                },
                RegistryRequestGeneration(generation),
            )])
            .unwrap();
        let mut tx = updater.commit().await;
        let value = tx.compute(&key).await.unwrap();
        assert!(
            matches!(value.as_ref(), Err(RegistryFileError::Transport { cause: actual, message, .. }) if *actual == cause && message == "same transport message")
        );
        assert!(RegistryFileKey::validity(&value));
        if let Some(previous) = retained.last() {
            assert_eq!(RegistryFileKey::equality(previous, &value), generation == 2);
            if generation == 2 {
                assert!(Arc::ptr_eq(previous, &value));
            }
        }
        retained.push(value);
    }
    assert_eq!(io.calls.load(Ordering::SeqCst), 5);
    assert_eq!(retained[0], retained[4]);
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative)]
struct TypedGenerationRefusalKey(bool);
impl fmt::Display for TypedGenerationRefusalKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "typed-generation-refusal:{}", self.0)
    }
}
#[async_trait]
impl Key for TypedGenerationRefusalKey {
    type Value = Arc<Result<RegistryFileValue, RegistryFileError>>;
    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        Arc::new(if self.0 {
            read_local_registry_file(
                ctx,
                Path::new(WORKSPACE),
                &RegistryFileUrl::new("file:///display"),
                Path::new("/registry/file"),
            )
            .await
            .map_err(Into::into)
        } else {
            fetch_remote_registry_file(
                ctx,
                Path::new(WORKSPACE),
                &RegistryFileUrl::new("https://registry/file"),
                Some([0; 32]),
                false,
            )
            .await
            .map_err(Into::into)
        })
    }
    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[tokio::test]
async fn generation_refusal_supersedes_native_cause_for_remote_and_local() {
    for local in [false, true] {
        let cause = typed_refusal_cause(RegistryRefusalSite::LocalRead);
        let io = Arc::new(TypedRefusalIo {
            cause: Mutex::new(Some(cause)),
            calls: AtomicUsize::new(0),
        });
        let mut builder = Dice::builder();
        install_registry_io(&mut builder, io.clone());
        let dice = builder.build(DetectCycles::Enabled);
        let mut tx = dice.updater().commit().await;
        let result = tx.compute(&TypedGenerationRefusalKey(local)).await.unwrap();
        assert!(matches!(
            result.as_ref(),
            Err(RegistryFileError::MissingRequestGeneration(_))
        ));
        assert_eq!(io.calls.load(Ordering::SeqCst), 1);
        // A real generation input repairs the supersession and exposes ONLY
        // the cause from the newly selected native read.
        let mut update = dice.updater();
        update
            .changed_to([(
                RegistryRequestGenerationKey {
                    workspace: PathBuf::from(WORKSPACE),
                },
                RegistryRequestGeneration(1),
            )])
            .unwrap();
        update.changed([TypedGenerationRefusalKey(local)]).unwrap();
        let mut next = update.commit().await;
        let result = next
            .compute(&TypedGenerationRefusalKey(local))
            .await
            .unwrap();
        assert!(
            matches!(result.as_ref(), Err(RegistryFileError::Transport { cause: Some(actual), .. } | RegistryFileError::LocalRead { cause: Some(actual), .. }) if *actual == cause)
        );
        assert_eq!(io.calls.load(Ordering::SeqCst), 2);
    }
}
