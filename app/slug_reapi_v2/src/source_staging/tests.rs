use prost::Message;

use super::*;

#[test]
fn source_merkle_nodes_are_executable_and_paths_are_structural() {
    let digest = ReapiDigest::of_bytes(b"source");
    let entry = |path| ReapiInputTreeEntry::new(path, digest.clone(), InputTreeEntryKind::Source);
    let tree = ReapiInputTree::from_source_entries([
        entry("external/dep+/a"),
        entry("root"),
        entry("root"),
    ])
    .unwrap();
    assert_eq!(tree.entries().len(), 2);
    let mut files = 0;
    for blob in tree.directory_blobs() {
        let directory = crate::proto::Directory::decode(blob.data()).unwrap();
        for file in directory.files {
            files += 1;
            assert!(file.is_executable);
            assert_eq!(file.digest.as_ref().unwrap().hash, digest.hash());
        }
    }
    assert_eq!(files, 2);
    for (left, right) in [("a", "a/b"), ("a/b", "a")] {
        assert!(ReapiInputTree::from_source_entries([entry(left), entry(right)]).is_err());
    }
    assert!(ReapiInputTree::from_source_entries([entry("../escape")]).is_err());
    assert!(
        ReapiInputTree::from_source_entries([
            entry("same"),
            ReapiInputTreeEntry::new(
                "same",
                ReapiDigest::of_bytes(b"different"),
                InputTreeEntryKind::Source
            ),
        ])
        .is_err()
    );
}

#[path = "../../../slug_core_v2/src/runtime/source_staging/test_workspace.rs"]
mod fixture;

// Public Core preparation and public CAS staging, with no Action or Execute call.
#[test]
#[ignore = "requires a supervised local NativeLink endpoint"]
fn nativelink_closure_sources_merkle_and_verified_upload() {
    use slug_configuration_v2::CommandConfigurationOverlay;
    use slug_core_v2::runtime::BzlmodCommandPolicyKey;
    use slug_core_v2::runtime::BzlmodEnvironmentPolicyKey;
    use slug_core_v2::runtime::LockfileMode;
    use slug_core_v2::runtime::ProcessHostOwner;
    use slug_core_v2::runtime::TargetPattern;
    use slug_core_v2::runtime::TerminalOutput;
    use slug_core_v2::runtime::WorkspaceRuntime;
    use slug_reapi_cache_v2::TransferPolicy;
    struct Root(std::path::PathBuf);
    impl Drop for Root {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let root = Root(std::env::temp_dir().join(format!(
            "slug-source-staging-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )));
    fixture::write(&root.0);
    let workspace = WorkspaceRuntime::new(&root.0, ProcessHostOwner::native()).unwrap();
    let command = BzlmodCommandPolicyKey::from_flags(None, false).unwrap();
    let environment = BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap();
    let targets = [TargetPattern::parse("//:one").unwrap()];
    let registry = [format!("file://{}/empty-registry", root.0.display())];
    let accepted = workspace
        .build_command_with_repository_environment(
            &targets,
            command.clone(),
            environment.clone(),
            LockfileMode::Update,
            &registry,
            Default::default(),
            CommandConfigurationOverlay::default(),
        )
        .unwrap();
    let mut owner = None;
    drop(accepted.project(|result| {
        owner = Some(
            result
                .as_ref()
                .as_ref()
                .unwrap()
                .analyses()
                .find_map(|value| {
                    value
                        .configured_target_key()
                        .filter(|key| key.label().target().as_str() == "one")
                        .cloned()
                })
                .unwrap(),
        );
        TerminalOutput::new(0, String::new(), String::new())
    }));
    let owner = owner.unwrap();
    let prepare = || {
        workspace.prepare_source_action_inputs_with_repository_environment(
            &targets,
            owner.clone(),
            0,
            command.clone(),
            environment.clone(),
            LockfileMode::Update,
            &registry,
            Default::default(),
            CommandConfigurationOverlay::default(),
        )
    };
    let mut prepared = None;
    drop(prepare().unwrap().project(|value| {
        prepared = Some(value.clone());
        TerminalOutput::new(0, String::new(), String::new())
    }));
    let plan = SourceInputReapiPlan::from_prepared(prepared.unwrap()).unwrap();
    assert_eq!(plan.prepared.sources().len(), 2);
    assert_eq!(plan.tree.inline_blobs().len(), 1);
    let command = plan.prepared.spawn().expand_forced_param_files().unwrap();
    let collision = ReapiInputTree::from_source_entries([ReapiInputTreeEntry::new(
        command.param_files()[0].path(),
        ReapiDigest::of_bytes(b"source"),
        InputTreeEntryKind::Source,
    )])
    .unwrap();
    assert!(collision.with_spawn_param_files(&command).is_err());

    let expected_sources = [
        ("input", b"aaa".as_slice()),
        ("tool", b"stage tool bytes".as_slice()),
    ];
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let endpoint = std::env::var("SLUG_V2_NATIVELINK_ENDPOINT").unwrap();
    let endpoint = endpoint.replace("grpc://", "http://");
    let cache = runtime.block_on(async {
        let channel = tonic::transport::Endpoint::from_shared(endpoint)
            .unwrap()
            .connect()
            .await
            .unwrap();
        CacheClient::new(
            channel,
            String::new(),
            TransferPolicy {
                max_batch_bytes: 1,
                chunk_bytes: 3,
            },
        )
        .unwrap()
    });
    runtime.block_on(async {
        let uploaded = plan.upload_missing(&cache).await.unwrap();
        let expected = plan
            .source_digests
            .iter()
            .chain(
                plan.tree
                    .directory_blobs()
                    .iter()
                    .chain(plan.tree.inline_blobs())
                    .map(|blob| blob.digest()),
            )
            .cloned()
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            uploaded
                .into_iter()
                .collect::<std::collections::BTreeSet<_>>(),
            expected
        );
        for (path, bytes) in expected_sources {
            let entry = plan
                .tree
                .entries()
                .iter()
                .find(|entry| entry.path() == path)
                .unwrap();
            let mut actual = Vec::new();
            cache
                .read_blob_verified(entry.digest(), |chunk| {
                    actual.extend_from_slice(chunk);
                    Ok(())
                })
                .await
                .unwrap();
            assert_eq!(actual, bytes);
        }
        let mut root_bytes = Vec::new();
        cache
            .read_blob_verified(plan.tree.root_digest(), |chunk| {
                root_bytes.extend_from_slice(chunk);
                Ok(())
            })
            .await
            .unwrap();
        let root = crate::proto::Directory::decode(root_bytes.as_slice()).unwrap();
        for name in ["input", "tool"] {
            assert!(
                root.files
                    .iter()
                    .find(|file| file.name == name)
                    .unwrap()
                    .is_executable
            );
        }
        let param = plan.tree.inline_blobs().first().unwrap();
        let mut bytes = Vec::new();
        cache
            .read_blob_verified(param.digest(), |chunk| {
                bytes.extend_from_slice(chunk);
                Ok(())
            })
            .await
            .unwrap();
        assert_eq!(bytes, b"parameter\n");
        std::fs::remove_file(root_path(&plan, "input")).unwrap();
        assert!(
            plan.upload_missing(&cache).await.unwrap().is_empty(),
            "an existing CAS digest does not reopen an old prepared source"
        );
    });
    assert!(
        prepare().is_err(),
        "fresh native preparation must reject the missing source"
    );
    // A new digest not yet in CAS must be verified against bytes opened later.
    std::fs::write(root.0.join("input"), b"not yet uploaded").unwrap();
    let mut changed = None;
    drop(prepare().unwrap().project(|value| {
        changed = Some(value.clone());
        TerminalOutput::new(0, String::new(), String::new())
    }));
    let stale = SourceInputReapiPlan::from_prepared(changed.unwrap()).unwrap();
    std::fs::write(root.0.join("input"), b"different bytes!").unwrap();
    runtime.block_on(async {
        let digest = stale.tree.entries().iter().find(|entry| entry.path() == "input").unwrap().digest().clone();
        assert_eq!(cache.find_missing(&[digest.clone()].into_iter().collect()).await.unwrap(), [digest.clone()].into_iter().collect());
        let error = stale.upload_missing(&cache).await.unwrap_err();
        assert!(matches!(&error, CacheError::Protocol(message) if message.starts_with("upload digest mismatch:")), "{error}");
        // A backend may expose interrupted bytes. Neither missing nor corrupt
        // remote data can become a successfully verified source result.
        assert!(cache.read_blob_verified(&digest, |_| Ok(())).await.is_err());

    });
}

fn root_path(plan: &SourceInputReapiPlan, name: &str) -> std::path::PathBuf {
    plan.prepared
        .sources()
        .find(|source| source.label().target().as_str() == name)
        .unwrap()
        .real_path()
        .as_path()
        .to_path_buf()
}
