use std::hash::Hash;
use std::hash::Hasher;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use dice::DetectCycles;
use dice::Dice;
use dice::DiceComputations;
use dice::DynKey;
use dice::InjectedKey;
use dice::Key;
use dice::UserComputationData;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use sha2::Digest;
use sha2::Sha256;
use slug_workspace_v2::ElfMalformedReason;
use slug_workspace_v2::ElfMetadataOutcome;
use slug_workspace_v2::ElfMetadataPolicy;
use slug_workspace_v2::ElfMetadataRejection;
use slug_workspace_v2::ElfSourceDigestBinding;
use slug_workspace_v2::FileContentDigest;
use slug_workspace_v2::ImageChargedPath;
use slug_workspace_v2::ImageDescriptorBudget;
use slug_workspace_v2::ImageDescriptorBudgetError;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::VerifiedElfMetadata;

use super::*;
use crate::HostSelectedExtensionOwner;
use crate::RepositoryEnvironmentSnapshot;
use crate::SelectedScratchOperandSpec;
use crate::SelectedScratchPayloadBudget;
use crate::SelectedScratchPolicy;
use crate::SelectedScratchPrefix;
use crate::SelectedScratchPrefixBuilder;
use crate::SelectedScratchProcessSpec;
use crate::SelectedScratchResources;
use crate::SourcePreparationNeeds;
use crate::SourcePreparationNeedsError;
use crate::SourcePreparationOutcome;

const GENERATOR: SelectedToolOperand = SelectedToolOperand {
    role: SelectedToolRole::Generator,
    argument_index: 0,
};
fn policy() -> SelectedToolSourcesPolicy {
    SelectedToolSourcesPolicy::new(&[GENERATOR], ElfMetadataPolicy::default()).unwrap()
}
fn resources() -> SelectedScratchResources {
    SelectedScratchResources::new(
        SelectedScratchPayloadBudget::new(256 << 20),
        ImageDescriptorBudget::new(64 << 20),
    )
}
fn builder(resources: &SelectedScratchResources) -> SelectedScratchPrefixBuilder {
    SelectedScratchPrefixBuilder::new(
        ImageChargedPath::pin_existing(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            resources.descriptor_budget(),
        )
        .unwrap(),
        Arc::new(HostSelectedExtensionOwner::testing("tool+")),
        SelectedScratchPolicy::plain(),
        resources,
    )
    .unwrap()
}
fn prefix(
    resources: &SelectedScratchResources,
    bytes: &[u8],
    prior: Option<&[u8]>,
) -> SelectedScratchPrefix {
    let mut b = builder(resources);
    let p = b.path("generator").unwrap();
    if let Some(prior) = prior {
        b.file(&p, prior, true).unwrap();
    }
    b.file(&p, bytes, true).unwrap();
    b.finish(
        SelectedScratchProcessSpec {
            arguments: &[SelectedScratchOperandSpec::Scratch(&p)],
            cwd: "/__slug_module",
            environment: &[],
            timeout_seconds: 30,
            quiet: true,
        },
        &RepositoryEnvironmentSnapshot::empty(),
    )
    .unwrap()
}
fn binding(bytes: &[u8]) -> ElfSourceDigestBinding {
    ElfSourceDigestBinding::from_digest(
        FileContentDigest::new(Sha256::digest(bytes).into(), bytes.len() as u64).unwrap(),
    )
}
fn request(
    prefix: &SelectedScratchPrefix,
    resources: &SelectedScratchResources,
) -> SelectedScratchElfMetadataRequest {
    let selection =
        SelectedToolSourceSelection::from_prefix(prefix, policy(), GENERATOR, resources).unwrap();
    let digest = binding(selection.scratch_entry().unwrap().2.bytes());
    SelectedScratchElfMetadataRequest::new(&selection, digest, resources).unwrap()
}
fn result(
    request: &SelectedScratchElfMetadataRequest,
    resources: &SelectedScratchResources,
) -> SelectedScratchElfMetadataResult {
    // Raw proof transport fixture, not a runnable certificate or native grant.
    let verified = VerifiedElfMetadata::new(
        request.expected_digest(),
        request.policy(),
        ElfMetadataOutcome::Rejected(ElfMetadataRejection::Malformed(ElfMalformedReason::Header)),
        resources.descriptor_budget().try_reserve(0).unwrap(),
    )
    .unwrap();
    SelectedScratchElfMetadataResult::new(request, verified, resources).unwrap()
}
fn data(
    resources: &SelectedScratchResources,
    transaction: Option<SelectedScratchElfMetadataTransaction>,
) -> UserComputationData {
    let mut data = UserComputationData::default();
    data.data.set(resources.dupe());
    if let Some(transaction) = transaction {
        data.data.set(transaction);
    }
    data
}
fn transaction(
    request: &SelectedScratchElfMetadataRequest,
    resources: &SelectedScratchResources,
    ready: Option<&SelectedScratchElfMetadataResult>,
) -> SelectedScratchElfMetadataTransaction {
    let allowed = SelectedScratchElfMetadataNeeds::singleton(request, resources).unwrap();
    SelectedScratchElfMetadataTransaction::new(
        request.selector(),
        resources,
        Some(&allowed),
        ready.map(std::slice::from_ref).unwrap_or(&[]),
    )
    .unwrap()
}

#[test]
fn finite_policy_identity_and_bounds_are_structural() {
    let elf = ElfMetadataPolicy::default();
    let cargo = SelectedToolOperand {
        role: SelectedToolRole::Cargo,
        argument_index: 9,
    };
    let rustc = SelectedToolOperand {
        role: SelectedToolRole::Rustc,
        argument_index: 11,
    };
    let p = SelectedToolSourcesPolicy::new(&[GENERATOR, cargo, rustc], elf).unwrap();
    assert_eq!(p.operands(), &[GENERATOR, cargo, rustc]);
    for bad in [
        &[][..],
        &[cargo][..],
        &[GENERATOR, GENERATOR][..],
        &[
            GENERATOR,
            SelectedToolOperand {
                role: SelectedToolRole::Cargo,
                argument_index: 0,
            },
        ][..],
        &[GENERATOR, cargo, rustc, cargo][..],
    ] {
        assert_eq!(
            SelectedToolSourcesPolicy::new(bad, elf),
            Err(SelectedToolSourceError::InvalidPolicy)
        );
    }
    let r = resources();
    let prefix = prefix(&r, b"binary", None);
    let a = SelectedScratchElfMetadataSelector::from_prefix(&prefix, p, &r).unwrap();
    let b = SelectedScratchElfMetadataSelector::from_prefix(
        &prefix,
        SelectedToolSourcesPolicy::new(
            &[
                GENERATOR,
                SelectedToolOperand {
                    argument_index: 8,
                    ..cargo
                },
                rustc,
            ],
            elf,
        )
        .unwrap(),
        &r,
    )
    .unwrap();
    assert_ne!(a, b);
    fn hash(value: &impl Hash) -> u64 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        value.hash(&mut h);
        h.finish()
    }
    assert_eq!(hash(&a), hash(&a.dupe()));
    assert_ne!(hash(&a), hash(&b));
}

#[test]
fn typed_selection_uses_final_overwrite_and_refuses_literal_directory_missing_and_mode() {
    let r = resources();
    let overwrite = prefix(&r, b"final", Some(b"prior"));
    let selected = request(&overwrite, &r);
    assert_eq!(selected.blob().bytes(), b"final");
    assert_eq!(selected.mode(), 0o755);
    for (kind, expected) in [
        (0, SelectedToolSourceError::LiteralOperand),
        (1, SelectedToolSourceError::WrongKind),
        (2, SelectedToolSourceError::MissingEntry),
        (3, SelectedToolSourceError::NotExecutable),
    ] {
        let mut b = builder(&r);
        let p = b.path(if kind == 1 { "." } else { "generator" }).unwrap();
        if kind == 0 || kind == 3 {
            b.file(&p, b"final", false).unwrap();
        }
        let argument = if kind == 0 {
            SelectedScratchOperandSpec::Literal(p.guest_spelling())
        } else {
            SelectedScratchOperandSpec::Scratch(&p)
        };
        let prefix = b
            .finish(
                SelectedScratchProcessSpec {
                    arguments: &[argument],
                    cwd: "/__slug_module",
                    environment: &[],
                    timeout_seconds: 1,
                    quiet: true,
                },
                &RepositoryEnvironmentSnapshot::empty(),
            )
            .unwrap();
        assert_eq!(
            SelectedToolSourceSelection::from_prefix(&prefix, policy(), GENERATOR, &r),
            Err(expected)
        );
    }
    let profile = SelectedToolSourcesPolicy::new(
        &[
            GENERATOR,
            SelectedToolOperand {
                role: SelectedToolRole::Cargo,
                argument_index: 9,
            },
        ],
        ElfMetadataPolicy::default(),
    )
    .unwrap();
    assert_eq!(
        SelectedToolSourceSelection::from_prefix(&overwrite, profile, profile.operands()[1], &r),
        Err(SelectedToolSourceError::MissingArgument)
    );
    let foreign = resources();
    assert_eq!(
        SelectedToolSourceSelection::from_prefix(&overwrite, policy(), GENERATOR, &foreign),
        Err(SelectedToolSourceError::ForeignResources)
    );
}

#[test]
fn same_digest_does_not_erase_prefix_provenance_and_result_binding() {
    let r = resources();
    let p1 = prefix(&r, b"final", None);
    let p2 = prefix(&r, b"final", Some(b"prior"));
    let a = request(&p1, &r);
    let b = request(&p2, &r);
    assert_eq!(a.expected_digest(), b.expected_digest());
    assert_eq!(
        SelectedScratchElfMetadataEpochKey::new(&a),
        SelectedScratchElfMetadataEpochKey::new(&b)
    );
    assert_ne!(a, b);
    let allowed = SelectedScratchElfMetadataNeeds::singleton(&b, &r).unwrap();
    let old = result(&a, &r);
    assert_eq!(
        SelectedScratchElfMetadataTransaction::new(b.selector(), &r, Some(&allowed), &[old]),
        Err(SelectedToolSourceError::UnauthorizedRequest)
    );
    let wrong = VerifiedElfMetadata::new(
        binding(b"other"),
        b.policy(),
        ElfMetadataOutcome::Rejected(ElfMetadataRejection::Malformed(ElfMalformedReason::Header)),
        r.descriptor_budget().try_reserve(0).unwrap(),
    )
    .unwrap();
    assert_eq!(
        SelectedScratchElfMetadataResult::new(&b, wrong, &r),
        Err(SelectedToolSourceError::ObservationMismatch)
    );
    assert_eq!(
        SelectedScratchElfMetadataRequest::new(b.selection(), binding(b"x"), &r),
        Err(SelectedToolSourceError::InvalidRequest)
    );
}

#[test]
fn all_label_transaction_authorizes_complete_profile_without_scratch_cells() {
    let r = resources();
    let mut b = builder(&r);
    let external = b.record_external_path("/actual/source/bin/tool").unwrap();
    let prefix = b
        .finish(
            SelectedScratchProcessSpec {
                arguments: &[SelectedScratchOperandSpec::ExternalPath(&external)],
                cwd: "/__slug_module",
                environment: &[],
                timeout_seconds: 1,
                quiet: true,
            },
            &RepositoryEnvironmentSnapshot::empty(),
        )
        .unwrap();
    let selected =
        SelectedToolSourceSelection::from_prefix(&prefix, policy(), GENERATOR, &r).unwrap();
    assert_eq!(
        selected.external_path().unwrap().event_index(),
        external.event_index()
    );
    assert_eq!(selected.spelling(), external.spelling());
    assert!(selected.scratch_entry().is_none());
    let transaction =
        SelectedScratchElfMetadataTransaction::new(selected.selector(), &r, None, &[]).unwrap();
    assert!(transaction.authorizes_selector(selected.selector()));
    let changed = SelectedScratchElfMetadataSelector::from_prefix(
        &prefix,
        SelectedToolSourcesPolicy::new(
            &[
                GENERATOR,
                SelectedToolOperand {
                    role: SelectedToolRole::Cargo,
                    argument_index: 1,
                },
            ],
            ElfMetadataPolicy::default(),
        )
        .unwrap(),
        &r,
    )
    .unwrap();
    assert!(!transaction.authorizes_selector(&changed));
    assert!(!transaction.same_resources(&resources()));
    assert_eq!(
        SelectedScratchElfMetadataRequest::new(&selected, binding(b""), &r),
        Err(SelectedToolSourceError::WrongKind)
    );
}

#[test]
fn batch_precharges_union_before_legacy_merge_and_retains_credit_until_last_drop() {
    let r = resources();
    let mut b = builder(&r);
    let a_path = b.path("a").unwrap();
    let c_path = b.path("cargo").unwrap();
    b.file(&a_path, b"a", true).unwrap();
    b.file(&c_path, b"c", true).unwrap();
    let prefix = b
        .finish(
            SelectedScratchProcessSpec {
                arguments: &[
                    SelectedScratchOperandSpec::Scratch(&a_path),
                    SelectedScratchOperandSpec::Scratch(&c_path),
                ],
                cwd: "/__slug_module",
                environment: &[],
                timeout_seconds: 1,
                quiet: true,
            },
            &RepositoryEnvironmentSnapshot::empty(),
        )
        .unwrap();
    let p = SelectedToolSourcesPolicy::new(
        &[
            GENERATOR,
            SelectedToolOperand {
                role: SelectedToolRole::Cargo,
                argument_index: 1,
            },
        ],
        ElfMetadataPolicy::default(),
    )
    .unwrap();
    let selection_a =
        SelectedToolSourceSelection::from_prefix(&prefix, p, p.operands()[0], &r).unwrap();
    let selection_c =
        SelectedToolSourceSelection::from_prefix(&prefix, p, p.operands()[1], &r).unwrap();
    let request_a =
        SelectedScratchElfMetadataRequest::new(&selection_a, binding(b"a"), &r).unwrap();
    let request_c =
        SelectedScratchElfMetadataRequest::new(&selection_c, binding(b"c"), &r).unwrap();
    let a = SourcePreparationNeeds::scratch_elf_metadata(
        SelectedScratchElfMetadataNeeds::singleton(&request_a, &r).unwrap(),
    );
    let c = SourcePreparationNeeds::scratch_elf_metadata(
        SelectedScratchElfMetadataNeeds::singleton(&request_c, &r).unwrap(),
    );
    let pressure = r
        .descriptor_budget()
        .try_reserve(r.descriptor_budget().limit_bytes() - r.descriptor_budget().used_bytes())
        .unwrap();
    let used = r.descriptor_budget().used_bytes();
    let error = a.try_union(&c).unwrap_err();
    assert_eq!(
        error,
        SourcePreparationNeedsError::ScratchElfMetadata(SelectedToolSourceError::Descriptor(
            ImageDescriptorBudgetError::ResourceLimit
        ))
    );
    assert!(error.is_transient());
    assert_eq!(r.descriptor_budget().used_bytes(), used);
    assert_eq!(
        a.try_union(&a)
            .unwrap()
            .scratch_elf_metadata_requests()
            .unwrap()
            .len(),
        1
    );
    drop(pressure);
    let merged = a.try_union(&c).unwrap();
    assert_eq!(merged.scratch_elf_metadata_requests().unwrap().len(), 2);
    let retained = merged.dupe();
    drop(merged);
    drop(a);
    drop(c);
    drop(prefix);
    drop(selection_a);
    drop(selection_c);
    drop(request_a);
    drop(request_c);
    drop(a_path);
    drop(c_path);
    assert_eq!(r.payload_budget().used_bytes(), 2);
    drop(retained);
    assert_eq!(r.payload_budget().used_bytes(), 0);
    assert_eq!(r.descriptor_budget().used_bytes(), 0);
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative)]
struct Revision;
impl std::fmt::Display for Revision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "tool-test-revision")
    }
}
impl InjectedKey for Revision {
    type Value = usize;
    fn equality(a: &usize, b: &usize) -> bool {
        a == b
    }
}
#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative)]
struct NormalPrefix;
impl std::fmt::Display for NormalPrefix {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "tool-test-normal-prefix")
    }
}
#[async_trait]
impl Key for NormalPrefix {
    type Value = SelectedScratchPrefix;
    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let revision = ctx.compute(&Revision).await.unwrap();
        let resources = ctx
            .per_transaction_data()
            .data
            .get::<SelectedScratchResources>()
            .unwrap();
        prefix(resources, &[revision as u8; 64], None)
    }
    fn equality(a: &Self::Value, b: &Self::Value) -> bool {
        a == b
    }
}
#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative)]
struct Consumer;
impl std::fmt::Display for Consumer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "tool-test-consumer")
    }
}
#[async_trait]
impl Key for Consumer {
    type Value =
        SourcePreparationOutcome<Result<SelectedScratchElfMetadataResult, SelectedToolSourceError>>;
    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let prefix = ctx.compute(&NormalPrefix).await.unwrap();
        let resources = ctx
            .per_transaction_data()
            .data
            .get::<SelectedScratchResources>()
            .unwrap()
            .dupe();
        let request = request(&prefix, &resources);
        lookup_selected_scratch_elf_metadata(ctx, &request)
            .await
            .unwrap()
    }
    fn equality(a: &Self::Value, b: &Self::Value) -> bool {
        Self::validity(a) && Self::validity(b) && a.complete_eq(b)
    }
    fn validity(value: &Self::Value) -> bool {
        matches!(value, SourcePreparationOutcome::Complete(result) if result.as_ref().err().is_none_or(|e| !e.is_transient()))
    }
}

#[tokio::test]
async fn direct_raw_lookup_mints_current_need_replays_and_rejects_foreign_authorization() {
    let r = resources();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut updater = dice.updater_with_data(data(&r, None));
    updater.changed_to(vec![(Revision, 0)]).unwrap();
    let mut tx = updater.commit().await;
    let prefix = tx.compute(&NormalPrefix).await.unwrap();
    let request = request(&prefix, &r);
    let cell = SelectedScratchElfMetadataEpochKey::new(&request);
    let mut updater = dice.updater_with_data(data(&r, Some(transaction(&request, &r, None))));
    updater
        .changed_to(vec![(
            cell.dupe(),
            SelectedScratchElfMetadataEpoch::AuthorizedEmpty,
        )])
        .unwrap();
    let mut tx = updater.commit().await;
    let need = tx.compute(&Consumer).await.unwrap();
    assert!(!Consumer::validity(&need));
    let SourcePreparationOutcome::Need(need) = need else {
        panic!("expected scratch Need")
    };
    assert_eq!(
        need.scratch_elf_metadata_requests()
            .unwrap()
            .requests()
            .next(),
        Some(&request)
    );
    let ready = result(&request, &r);
    let mut updater =
        dice.updater_with_data(data(&r, Some(transaction(&request, &r, Some(&ready)))));
    updater
        .changed_to(vec![(
            cell.dupe(),
            SelectedScratchElfMetadataEpoch::Ready(ready.dupe()),
        )])
        .unwrap();
    let mut tx = updater.commit().await;
    let complete = tx.compute(&Consumer).await.unwrap();
    assert!(
        matches!(complete, SourcePreparationOutcome::Complete(Ok(ref value)) if value == &ready)
    );
    // Raw helper checks current userdata even when its compact cell is warm.
    let foreign = resources();
    let mut tx = dice
        .updater_with_data(data(
            &foreign,
            Some(transaction(&request, &r, Some(&ready))),
        ))
        .commit()
        .await;
    assert!(matches!(
        lookup_selected_scratch_elf_metadata(&mut tx, &request)
            .await
            .unwrap(),
        SourcePreparationOutcome::Complete(Err(SelectedToolSourceError::ForeignResources))
    ));
    let mut tx = dice.updater_with_data(data(&r, None)).commit().await;
    assert!(matches!(
        lookup_selected_scratch_elf_metadata(&mut tx, &request)
            .await
            .unwrap(),
        SourcePreparationOutcome::Complete(Err(SelectedToolSourceError::MissingTransaction))
    ));
    // Raw absence must be valid input data so SAME-updater revocation works;
    // its source-owned refusal remains transient/invalid in the normal consumer.
    let mut updater = dice.updater_with_data(data(&r, Some(transaction(&request, &r, None))));
    updater
        .changed_to(vec![(
            cell.dupe(),
            SelectedScratchElfMetadataEpoch::Unauthorized,
        )])
        .unwrap();
    let mut tx = updater.commit().await;
    let revoked = tx.compute(&Consumer).await.unwrap();
    assert!(SelectedScratchElfMetadataEpochKey::validity(
        &SelectedScratchElfMetadataEpoch::Unauthorized
    ));
    assert!(matches!(
        revoked,
        SourcePreparationOutcome::Complete(Err(SelectedToolSourceError::UnauthorizedRequest))
    ));
    assert!(!Consumer::validity(&revoked));
    // Same compact slot and digest, changed ordered prefix: old Ready is not current.
    let changed_prefix = super::tests::prefix(&r, &[0; 64], Some(b"overwritten"));
    let changed = super::tests::request(&changed_prefix, &r);
    let mut updater = dice.updater_with_data(data(&r, Some(transaction(&changed, &r, None))));
    updater
        .changed_to(vec![(cell, SelectedScratchElfMetadataEpoch::Ready(ready))])
        .unwrap();
    let mut tx = updater.commit().await;
    let outcome = lookup_selected_scratch_elf_metadata(&mut tx, &changed)
        .await
        .unwrap();
    let SourcePreparationOutcome::Need(need) = outcome else {
        panic!("stale proof must Need")
    };
    assert_eq!(
        need.scratch_elf_metadata_requests()
            .unwrap()
            .requests()
            .next(),
        Some(&changed)
    );
}

#[tokio::test]
async fn compact_key_and_demand_catalogue_do_not_pin_blob_payload() {
    let r = resources();
    let prefix = prefix(&r, b"executable", None);
    let request = request(&prefix, &r);
    let cell = SelectedScratchElfMetadataEpochKey::new(&request);
    let metadata = DynKey::from_key(cell.dupe())
        .request_value::<SelectedScratchElfMetadataEpochKey>()
        .unwrap();
    assert_eq!(metadata, cell);
    let dice = Dice::builder().build(DetectCycles::Enabled);
    assert!(SelectedScratchElfMetadataEpochKey::validity(
        &SelectedScratchElfMetadataEpoch::Unauthorized
    ));
    let mut updater = dice.updater();
    updater
        .changed_to(vec![(
            cell.dupe(),
            SelectedScratchElfMetadataEpoch::Unauthorized,
        )])
        .unwrap();
    let mut tx = updater.commit().await;
    assert!(matches!(
        tx.compute(&cell).await.unwrap(),
        SelectedScratchElfMetadataEpoch::Unauthorized
    ));
    drop(prefix);
    drop(request);
    assert_eq!(
        r.payload_budget().used_bytes(),
        0,
        "interned compact key and provided catalogue metadata must not retain prefix/blob"
    );
    assert!(
        r.descriptor_budget().used_bytes() > 0,
        "only charged selector/workspace remain"
    );
    drop(tx);
    dice.wait_for_idle().await;
    drop(dice);
    drop(cell);
    drop(metadata);
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while r.descriptor_budget().used_bytes() != 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn raw_value_and_normal_history_keep_payload_credit_until_actual_engine_retirement() {
    let r = resources();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut previous_tx = None;
    for revision in 0..2 {
        let mut updater = dice.updater_with_data(data(&r, None));
        updater.changed_to(vec![(Revision, revision)]).unwrap();
        let mut tx = updater.commit().await;
        let prefix = tx.compute(&NormalPrefix).await.unwrap();
        let request = request(&prefix, &r);
        let ready = result(&request, &r);
        let mut updater =
            dice.updater_with_data(data(&r, Some(transaction(&request, &r, Some(&ready)))));
        updater
            .changed_to(vec![(
                SelectedScratchElfMetadataEpochKey::new(&request),
                SelectedScratchElfMetadataEpoch::Ready(ready),
            )])
            .unwrap();
        let mut tx = updater.commit().await;
        assert!(matches!(
            tx.compute(&Consumer).await.unwrap(),
            SourcePreparationOutcome::Complete(Ok(_))
        ));
        if revision == 1 {
            assert_eq!(r.payload_budget().used_bytes(), 128);
        }
        previous_tx = Some(tx);
    }
    drop(previous_tx);
    dice.wait_for_idle().await;
    assert!(
        r.payload_budget().used_bytes() > 0,
        "history release is not inferred from tx retirement"
    );
    drop(dice);
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while r.payload_budget().used_bytes() != 0 || r.descriptor_budget().used_bytes() != 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("actual historical values must retire with engine shutdown");
    assert_eq!(r.payload_budget().used_bytes(), 0);
    assert_eq!(r.descriptor_budget().used_bytes(), 0);
}
