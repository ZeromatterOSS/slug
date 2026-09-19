//! CAS terminal provenance cannot be replaced by equal source bytes through aliases.
use slug_core_v2::runtime::PlannedActionOutputStaging;

use super::*;

#[derive(Clone, Copy)]
enum Mutation {
    Remove,
    Corrupt,
}
struct FaultTransport {
    inner: ActionChainReapiTransport,
    mutation: Mutation,
    producers: AtomicUsize,
    consumers: AtomicUsize,
    generated_aliases: AtomicUsize,
    inputs: std::sync::Mutex<Option<Arc<PreparedActionChainInputs>>>,
}
impl ActionChainTransport for FaultTransport {
    type Session = ActionChainReapiSession;
    type Staged = StagedChainAction;
    type Output = ActionChainRemoteResult;
    type Error = RemoteExecutionError;
    async fn start(
        &self,
        inputs: Arc<PreparedActionChainInputs>,
    ) -> Result<Self::Session, Self::Error> {
        *self.inputs.lock().unwrap() = Some(inputs.clone());
        self.inner.start(inputs).await
    }
    async fn stage(
        &self,
        session: &mut Self::Session,
        index: usize,
    ) -> Result<Self::Staged, Self::Error> {
        self.inner.stage(session, index).await
    }
    async fn execute(
        &self,
        session: &mut Self::Session,
        index: usize,
        staged: Self::Staged,
    ) -> Result<(), Self::Error> {
        if session.inputs.plan().unwrap().actions()[index]
            .action()
            .outputs()
            .iter()
            .any(|out| out.path().ends_with("_consumer.out"))
        {
            self.consumers.fetch_add(1, Ordering::SeqCst);
        }
        self.inner.execute(session, index, staged).await?;
        if matches!(&session.results[index], ActionChainStepResult::ArtifactSymlink(alias) if ["generated.first", "generated.alias"].contains(&alias.output().path()))
        {
            self.generated_aliases.fetch_add(1, Ordering::SeqCst);
        }
        let Some(file) = session.results[index].remote().and_then(|remote| {
            remote
                .result
                .output_files()
                .iter()
                .find(|file| file.path() == "generated")
        }) else {
            return Ok(());
        };
        self.producers.fetch_add(1, Ordering::SeqCst);
        let digest = file.digest();
        assert!(
            session.inputs.sources().any(|source| {
                source.digest().size_bytes() == digest.size_bytes()
                    && source
                        .digest()
                        .sha256()
                        .iter()
                        .map(|byte| format!("{byte:02x}"))
                        .collect::<String>()
                        == digest.hash()
            }),
            "fixture must retain equal-content certified sources"
        );
        let root = PathBuf::from(std::env::var("SLUG_V2_NATIVELINK_TEST_ROOT").unwrap());
        for store in ["fast-content", "slow-content"] {
            let path = root.join("cas").join(store).join("d").join(format!(
                "{}-{}",
                digest.hash(),
                digest.size_bytes()
            ));
            assert!(path.is_file());
            match self.mutation {
                Mutation::Remove => fs::remove_file(path).unwrap(),
                Mutation::Corrupt => {
                    fs::set_permissions(
                        &path,
                        fs::Permissions::from_mode(
                            path.metadata().unwrap().permissions().mode() | 0o200,
                        ),
                    )
                    .unwrap();
                    fs::write(path, vec![b'!'; digest.size_bytes() as usize]).unwrap();
                }
            }
        }
        let requested = [digest.clone()].into_iter().collect();
        match self.mutation {
            Mutation::Remove => {
                // Clear both supervised store indexes, as in the accepted chain proof.
                for _ in 0..2 {
                    session
                        .cache
                        .read_blob_verified(digest, |_| Ok(()))
                        .await
                        .expect_err("removed generated blob must fail verification");
                }
                assert_eq!(
                    session.cache.find_missing(&requested).await.unwrap(),
                    requested
                );
            }
            Mutation::Corrupt => assert!(
                session
                    .cache
                    .find_missing(&requested)
                    .await
                    .unwrap()
                    .is_empty()
            ),
        }
        Ok(())
    }
    async fn finish(&self, session: Self::Session) -> Result<Self::Output, Self::Error> {
        self.inner.finish(session).await
    }
}
impl ActionChainOutputTransport for FaultTransport {
    async fn stage_outputs(
        &self,
        session: &mut Self::Session,
        stages: &[PlannedActionOutputStaging],
    ) -> Result<(), Self::Error> {
        self.inner.stage_outputs(session, stages).await
    }
}

#[test]
#[ignore = "requires supervised fresh verifying NativeLink and fixture CAS paths"]
fn nativelink_artifact_alias_cas_failure_blocks_alias_completion_and_consumer() {
    for (mutation, bytes, expected) in [
        (Mutation::Remove, "evicted", "missing"),
        (Mutation::Corrupt, "corrupt", "digest mismatch"),
    ] {
        let fixture = Fixture::new();
        fixture.content(bytes);
        let transport = FaultTransport {
            inner: remote_transport(),
            mutation,
            producers: AtomicUsize::new(0),
            consumers: AtomicUsize::new(0),
            generated_aliases: AtomicUsize::new(0),
            inputs: std::sync::Mutex::new(None),
        };
        let error = fixture
            .run(&["//:source_consumer"], &transport, false)
            .unwrap_err()
            .to_string();
        assert!(error.contains(expected), "{error}");
        assert_eq!(transport.producers.load(Ordering::SeqCst), 1);
        assert_eq!(
            transport.generated_aliases.load(Ordering::SeqCst),
            0,
            "first generated alias must not complete"
        );
        assert_eq!(transport.consumers.load(Ordering::SeqCst), 0);
        fixture.assert_absent(transport.inputs.lock().unwrap().as_ref().unwrap(), false);
    }
}
