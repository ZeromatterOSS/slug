use std::path::PathBuf;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use dice::DetectCycles;
use dice::Dice;
use dice::DiceComputations;
use dice::InjectedKey;
use dice::Key;
use dice::UserComputationData;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use sha2::Digest;
use sha2::Sha256;
use slug_workspace_v2::ImageChargedPath;
use slug_workspace_v2::ImageDescriptorBudget;
use slug_workspace_v2::ImageDescriptorBudgetError;
use slug_workspace_v2::NormalizedAbsolutePath;

use super::*;
use crate::HostSelectedExtensionOwner;
use crate::RepositoryEnvironmentEntry;
use crate::RepositoryEnvironmentSnapshot;
use crate::SelectedExtensionDownloadRequest;
use crate::SelectedExtensionDownloadVerified;
use crate::host_repository_relative_path;

fn resources(payload: usize) -> SelectedScratchResources {
    SelectedScratchResources::new(
        SelectedScratchPayloadBudget::new(payload),
        ImageDescriptorBudget::new(1 << 20),
    )
}
fn builder(
    r: &SelectedScratchResources,
    policy: SelectedScratchPolicy,
) -> SelectedScratchPrefixBuilder {
    SelectedScratchPrefixBuilder::new(
        ImageChargedPath::pin_existing(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            r.descriptor_budget(),
        )
        .unwrap(),
        Arc::new(HostSelectedExtensionOwner::testing("crate+")),
        policy,
        r,
    )
    .unwrap()
}
fn finish(b: SelectedScratchPrefixBuilder) -> Result<SelectedScratchPrefix, SelectedScratchError> {
    b.finish(
        SelectedScratchProcessSpec {
            arguments: &[SelectedScratchOperandSpec::Literal("tool")],
            cwd: "/__slug_module",
            environment: &[],
            timeout_seconds: 600,
            quiet: true,
        },
        &RepositoryEnvironmentSnapshot::empty(),
    )
}
fn verified(
    output: &str,
    bytes: &[u8],
    executable: bool,
) -> Arc<SelectedExtensionDownloadVerified> {
    let mut digest = [0; 32];
    digest.copy_from_slice(&Sha256::digest(bytes));
    let request = Arc::new(
        SelectedExtensionDownloadRequest::new(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            Arc::new(HostSelectedExtensionOwner::testing("crate+")),
            host_repository_relative_path(PathBuf::from(output)).unwrap(),
            "file:///generator",
            digest,
            executable,
        )
        .unwrap(),
    );
    Arc::new(SelectedExtensionDownloadVerified::new(request, Arc::from(bytes)).unwrap())
}
fn env(value: &str) -> RepositoryEnvironmentSnapshot {
    RepositoryEnvironmentSnapshot::from_canonical(vec![RepositoryEnvironmentEntry::new(
        "SOURCE", value,
    )])
    .unwrap()
}

#[test]
fn ordered_overwrite_and_modes_are_structural_a_b_a() {
    fn build(
        r: &SelectedScratchResources,
        first: &[u8],
        mode: bool,
        inherited: &str,
        arg: &str,
        cwd: &str,
        value: Option<&str>,
    ) -> SelectedScratchPrefix {
        let mut b = builder(r, SelectedScratchPolicy::plain());
        let p = b.path("pkg/config").unwrap();
        b.file(&p, first, mode).unwrap();
        b.file(&p, b"final", false).unwrap();
        b.finish(
            SelectedScratchProcessSpec {
                arguments: &[SelectedScratchOperandSpec::Literal(arg)],
                cwd,
                environment: &[("NAME", value)],
                timeout_seconds: 600,
                quiet: true,
            },
            &env(inherited),
        )
        .unwrap()
    }
    let r = resources(1024);
    let a = build(
        &r,
        b"initial",
        true,
        "A",
        "tool",
        "/__slug_module",
        Some(""),
    );
    let again = build(
        &r,
        b"initial",
        true,
        "A",
        "tool",
        "/__slug_module",
        Some(""),
    );
    assert_eq!(a, again);
    assert_eq!(a.occurrence_bytes(), 12);
    let file = a.entries().iter().find(|e| e.blob().is_some()).unwrap();
    assert_eq!(file.blob().unwrap().bytes(), b"final");
    assert_eq!(file.mode(), 0o644);
    assert_eq!(a.entries().len(), 3);
    for b in [
        build(
            &r,
            b"changed",
            true,
            "A",
            "tool",
            "/__slug_module",
            Some(""),
        ),
        build(
            &r,
            b"initial",
            false,
            "A",
            "tool",
            "/__slug_module",
            Some(""),
        ),
        build(
            &r,
            b"initial",
            true,
            "B",
            "tool",
            "/__slug_module",
            Some(""),
        ),
        build(
            &r,
            b"initial",
            true,
            "A",
            "other",
            "/__slug_module",
            Some(""),
        ),
        build(
            &r,
            b"initial",
            true,
            "A",
            "tool",
            "/__slug_module/pkg",
            Some(""),
        ),
        build(&r, b"initial", true, "A", "tool", "/__slug_module", None),
    ] {
        assert_ne!(a, b);
    }
    assert_eq!(a.process().environment()[0].value(), Some(""));
    let held = a.dupe();
    drop(a);
    assert_eq!(
        held.process()
            .inherited_environment()
            .get("SOURCE")
            .unwrap()
            .as_ref(),
        "A"
    );
    drop(held);
    drop(again);
    assert_eq!(r.payload_budget().used_bytes(), 0);
}

#[test]
fn shared_download_pin_counts_occurrences_but_payload_once() {
    let r = resources(8);
    let v = verified("generator", b"12345678", true);
    let mut b = builder(&r, SelectedScratchPolicy::plain());
    let p = b.path("generator").unwrap();
    b.download(&p, &v).unwrap();
    b.download(&p, &v).unwrap();
    assert_eq!(r.payload_budget().used_bytes(), 8);
    let prefix = finish(b).unwrap();
    assert_eq!(prefix.occurrence_bytes(), 16);
    let writes: Vec<_> = prefix
        .operations()
        .iter()
        .filter_map(|op| match op {
            SelectedScratchOperation::Download { blob, .. } => Some(blob),
            _ => None,
        })
        .collect();
    assert!(std::ptr::eq(writes[0].bytes().as_ptr(), v.bytes().as_ptr()));
    assert!(std::ptr::eq(
        writes[0].bytes().as_ptr(),
        writes[1].bytes().as_ptr()
    ));
    assert!(Arc::ptr_eq(writes[0].download().unwrap(), &v));
    assert_eq!(
        prefix
            .entries()
            .iter()
            .find(|e| e.blob().is_some())
            .unwrap()
            .mode(),
        0o755
    );
    drop(writes);
    drop(v);
    let held = prefix.dupe();
    drop(prefix);
    assert_eq!(r.payload_budget().used_bytes(), 8);
    drop(held);
    assert_eq!(r.payload_budget().used_bytes(), 0);

    let mut b = builder(&r, SelectedScratchPolicy::plain());
    let p = b.path("a").unwrap();
    assert_eq!(
        b.download(&p, &verified("b", b"x", true)),
        Err(SelectedScratchError::DownloadMismatch)
    );
    let mut b = builder(&r, SelectedScratchPolicy::plain());
    let p = b.path("a").unwrap();
    b.download(&p, &verified("a", b"x", false)).unwrap();
    let q = b.path("b").unwrap();
    assert_eq!(
        b.download(&q, &verified("b", b"x", false)),
        Err(SelectedScratchError::Unsupported(
            SelectedScratchUnsupported::DistinctDownloads
        ))
    );
}

#[test]
fn policy_limits_precede_payload_and_descriptor_growth() {
    let r = resources(0); // Semantic rejection must win over physical byte exhaustion.
    let mut limits = SelectedScratchLimits::default();
    limits.max_file_bytes = 1;
    let mut b = builder(&r, SelectedScratchPolicy::new(limits).unwrap());
    let p = b.path("a").unwrap();
    assert_eq!(
        b.file(&p, b"xx", false),
        Err(SelectedScratchError::LimitExceeded(
            SelectedScratchLimit::FileBytes
        ))
    );
    limits.max_file_bytes = 32 << 20;
    limits.max_occurrence_bytes = 1;
    let mut b = builder(&r, SelectedScratchPolicy::new(limits).unwrap());
    let p = b.path("a").unwrap();
    assert_eq!(
        b.file(&p, b"xx", false),
        Err(SelectedScratchError::LimitExceeded(
            SelectedScratchLimit::OccurrenceBytes
        ))
    );
    limits = SelectedScratchLimits::default();
    limits.max_entries = 2;
    let mut b = builder(&r, SelectedScratchPolicy::new(limits).unwrap());
    let p = b.path("a/b").unwrap();
    assert_eq!(
        b.file(&p, b"x", false),
        Err(SelectedScratchError::LimitExceeded(
            SelectedScratchLimit::Entries
        ))
    );
    limits = SelectedScratchLimits::default();
    limits.max_depth = 1;
    let mut b = builder(&r, SelectedScratchPolicy::new(limits).unwrap());
    let p = b.path("a").unwrap();
    let before = r.descriptor_budget().used_bytes();
    assert_eq!(
        b.child(&p, "b"),
        Err(SelectedScratchError::LimitExceeded(
            SelectedScratchLimit::Depth
        ))
    );
    assert_eq!(
        r.descriptor_budget().used_bytes(),
        before,
        "depth refuses before join allocation"
    );
    limits = SelectedScratchLimits::default();
    limits.max_path_bytes = 16;
    let mut b = builder(&r, SelectedScratchPolicy::new(limits).unwrap());
    b.path("a").unwrap();
    assert_eq!(
        b.path("aa"),
        Err(SelectedScratchError::LimitExceeded(
            SelectedScratchLimit::PathBytes
        ))
    );
    limits = SelectedScratchLimits::default();
    limits.max_operations = 1;
    let mut b = builder(&r, SelectedScratchPolicy::new(limits).unwrap());
    b.path("a").unwrap();
    assert_eq!(
        finish(b),
        Err(SelectedScratchError::LimitExceeded(
            SelectedScratchLimit::Operations
        ))
    );
    limits = SelectedScratchLimits::default();
    limits.max_descriptor_bytes = 14 + 4;
    let mut b = builder(&r, SelectedScratchPolicy::new(limits).unwrap());
    b.record_external_path("four").unwrap();
    let before = r.descriptor_budget().used_bytes();
    assert_eq!(
        b.record_external_path("x"),
        Err(SelectedScratchError::LimitExceeded(
            SelectedScratchLimit::DescriptorBytes
        ))
    );
    assert_eq!(r.descriptor_budget().used_bytes(), before);
    assert_eq!(r.payload_budget().used_bytes(), 0);
}

#[test]
fn process_limits_include_inherited_environment_and_precede_allocation() {
    let r = resources(0);
    let limits = SelectedScratchLimits {
        max_argv_entries: 2,
        max_argv_bytes: 4,
        max_environment_entries: 3,
        max_environment_bytes: 4,
        ..SelectedScratchLimits::default()
    };
    let policy = SelectedScratchPolicy::new(limits).unwrap();
    let inherited =
        RepositoryEnvironmentSnapshot::from_canonical(vec![RepositoryEnvironmentEntry::new(
            "A", "1",
        )])
        .unwrap();
    let arguments = [
        SelectedScratchOperandSpec::Literal("tool"),
        SelectedScratchOperandSpec::Literal(""),
    ];
    let environment = [("B", Some("")), ("C", None)];
    fn call<'a>(
        arguments: &'a [SelectedScratchOperandSpec<'a>],
        environment: &'a [(&'a str, Option<&'a str>)],
    ) -> SelectedScratchProcessSpec<'a> {
        SelectedScratchProcessSpec {
            arguments,
            cwd: "/__slug_module",
            environment,
            timeout_seconds: 1,
            quiet: false,
        }
    }
    let prefix = builder(&r, policy)
        .finish(call(&arguments, &environment), &inherited)
        .unwrap();
    assert_eq!(prefix.process().arguments().len(), 2);
    assert_eq!(prefix.process().arguments()[1].spelling(), "");
    assert_eq!(prefix.process().environment()[0].value(), Some(""));
    assert_eq!(prefix.process().environment()[1].value(), None);
    assert_eq!(prefix.process().inherited_environment(), &inherited);
    drop(prefix);
    assert_eq!(r.descriptor_budget().used_bytes(), 0);
    let excess_arguments = [
        SelectedScratchOperandSpec::Literal("tool"),
        SelectedScratchOperandSpec::Literal(""),
        SelectedScratchOperandSpec::Literal(""),
    ];
    let excess_argument_bytes = [
        SelectedScratchOperandSpec::Literal("tools"),
        SelectedScratchOperandSpec::Literal(""),
    ];
    let excess_environment = [("B", Some("")), ("C", None), ("D", None)];
    let excess_environment_bytes = [("B", Some("x")), ("C", None)];
    for (arguments, environment, limit) in [
        (
            &excess_arguments[..],
            &environment[..],
            SelectedScratchLimit::ArgvEntries,
        ),
        (
            &excess_argument_bytes[..],
            &environment[..],
            SelectedScratchLimit::ArgvBytes,
        ),
        (
            &arguments[..],
            &excess_environment[..],
            SelectedScratchLimit::EnvironmentEntries,
        ),
        (
            &arguments[..],
            &excess_environment_bytes[..],
            SelectedScratchLimit::EnvironmentBytes,
        ),
    ] {
        let builder = builder(&r, policy);
        let budget = r.descriptor_budget();
        let pressure = budget
            .try_reserve(budget.limit_bytes() - budget.used_bytes())
            .unwrap();
        let pressure_bytes = pressure.charged_bytes();
        let error = builder
            .finish(call(arguments, environment), &inherited)
            .unwrap_err();
        assert_eq!(error, SelectedScratchError::LimitExceeded(limit));
        assert!(!error.is_transient());
        assert_eq!(
            budget.used_bytes(),
            pressure_bytes,
            "keyed process refusal must precede new allocations and release the builder"
        );
        assert_eq!(r.payload_budget().used_bytes(), 0);
        drop(pressure);
        assert_eq!(budget.used_bytes(), 0);
    }
}

#[test]
fn external_tokens_root_spelling_and_literal_operands_preserve_identity() {
    let r = resources(32);
    let mut b = builder(&r, SelectedScratchPolicy::plain());
    let root = b.path("").unwrap();
    let dot = b.path(".").unwrap();
    assert_eq!(root.guest_spelling(), "/__slug_module");
    assert_eq!(root, dot);
    assert_eq!(b.parent(&root).unwrap(), root);
    let child = b.child(&root, "bin/cargo").unwrap();
    assert_eq!(child.relative(), "bin/cargo");
    let label = b.record_external_path("/selected/bin/cargo").unwrap();
    let index = label.event_index();
    let prefix = b
        .finish(
            SelectedScratchProcessSpec {
                arguments: &[
                    SelectedScratchOperandSpec::ExternalPath(&label),
                    SelectedScratchOperandSpec::Literal(label.spelling()),
                    SelectedScratchOperandSpec::Scratch(&child),
                ],
                cwd: "/__slug_module",
                environment: &[],
                timeout_seconds: 600,
                quiet: true,
            },
            &RepositoryEnvironmentSnapshot::empty(),
        )
        .unwrap();
    assert!(
        matches!(&prefix.operations()[index], SelectedScratchOperation::ExternalPath {path} if path == &label)
    );
    let arguments = prefix.process().arguments();
    assert_eq!(arguments[0].spelling(), arguments[1].spelling());
    assert_ne!(arguments[0], arguments[1]);
    assert_eq!(arguments[0].external_path().unwrap().event_index(), index);
    assert!(arguments[1].external_path().is_none());
    let mut other = builder(&r, SelectedScratchPolicy::plain());
    assert_eq!(
        other.child(&root, "x"),
        Err(SelectedScratchError::ForeignOrigin)
    );
    assert_eq!(
        finish(other),
        Err(SelectedScratchError::ForeignOrigin),
        "caught error stays sticky"
    );
    let other = builder(&r, SelectedScratchPolicy::plain());
    assert_eq!(
        other.finish(
            SelectedScratchProcessSpec {
                arguments: &[SelectedScratchOperandSpec::ExternalPath(&label)],
                cwd: "/__slug_module",
                environment: &[],
                timeout_seconds: 1,
                quiet: true,
            },
            &RepositoryEnvironmentSnapshot::empty()
        ),
        Err(SelectedScratchError::ForeignOrigin)
    );
}

#[test]
fn physical_limits_foreign_services_empty_headers_and_inherited_pins() {
    let r = resources(3);
    let mut b = builder(&r, SelectedScratchPolicy::plain());
    let p = b.path("a").unwrap();
    let error = b.file(&p, b"four", false).unwrap_err();
    assert!(error.is_transient());
    assert_eq!(finish(b), Err(error));
    assert_eq!(r.payload_budget().used_bytes(), 0);
    let mut b = builder(&r, SelectedScratchPolicy::plain());
    let p = b.path("empty").unwrap();
    let before = r.descriptor_budget().used_bytes();
    b.file(&p, b"", false).unwrap();
    assert!(
        r.descriptor_budget().used_bytes() > before,
        "empty blob/record has a real descriptor owner"
    );
    let inherited = env(&"e".repeat(4096));
    let before = r.descriptor_budget().used_bytes();
    let prefix = b
        .finish(
            SelectedScratchProcessSpec {
                arguments: &[SelectedScratchOperandSpec::Literal("tool")],
                cwd: "/__slug_module",
                environment: &[],
                timeout_seconds: 1,
                quiet: false,
            },
            &inherited,
        )
        .unwrap();
    assert!(
        r.descriptor_budget().used_bytes() >= before + 4096,
        "raw inherited snapshot conservatively pinned"
    );
    assert!(prefix.same_resources(&r));
    assert!(!prefix.same_resources(&SelectedScratchResources::new(
        SelectedScratchPayloadBudget::new(3),
        r.descriptor_budget().dupe()
    )));
    assert!(!prefix.same_resources(&SelectedScratchResources::new(
        r.payload_budget().dupe(),
        ImageDescriptorBudget::new(1 << 20)
    )));
    let budget = r.descriptor_budget();
    let pressure = budget
        .try_reserve(budget.limit_bytes() - budget.used_bytes())
        .unwrap();
    let error = match SelectedScratchPrefixBuilder::new(
        prefix.workspace().dupe(),
        prefix.owner().dupe(),
        SelectedScratchPolicy::plain(),
        &r,
    ) {
        Ok(_) => panic!("physical descriptor pressure must reject"),
        Err(error) => error,
    };
    assert_eq!(
        error,
        SelectedScratchError::Descriptor(ImageDescriptorBudgetError::ResourceLimit)
    );
    assert!(error.is_transient());
    drop(pressure);
    drop(prefix);
    drop(p);
    drop(inherited);
    assert_eq!(r.payload_budget().used_bytes(), 0);
}

#[test]
fn tree_collisions_and_occurrence_overwrites_cannot_reset_limits() {
    let r = resources(32);
    let mut limits = SelectedScratchLimits::default();
    limits.max_occurrence_bytes = 4;
    let mut b = builder(&r, SelectedScratchPolicy::new(limits).unwrap());
    let p = b.path("a").unwrap();
    b.file(&p, b"xx", false).unwrap();
    b.file(&p, b"xx", true).unwrap();
    assert_eq!(r.payload_budget().used_bytes(), 4);
    assert_eq!(
        b.file(&p, b"x", false),
        Err(SelectedScratchError::LimitExceeded(
            SelectedScratchLimit::OccurrenceBytes
        ))
    );
    drop(b);
    drop(p);
    assert_eq!(r.payload_budget().used_bytes(), 0);
    let mut b = builder(&r, SelectedScratchPolicy::plain());
    let p = b.path("a/b").unwrap();
    b.file(&p, b"x", false).unwrap();
    let dir = b.path("a").unwrap();
    assert_eq!(
        b.file(&dir, b"x", false),
        Err(SelectedScratchError::WrongKind)
    );
    let mut b = builder(&r, SelectedScratchPolicy::plain());
    let p = b.path("a").unwrap();
    b.file(&p, b"x", false).unwrap();
    let child = b.child(&p, "b").unwrap();
    assert_eq!(
        b.file(&child, b"x", false),
        Err(SelectedScratchError::WrongKind)
    );
    let mut b = builder(&r, SelectedScratchPolicy::plain());
    let root = b.path(".").unwrap();
    assert_eq!(
        b.file(&root, b"", false),
        Err(SelectedScratchError::WrongKind)
    );
}

#[test]
fn geometric_descriptor_growth_fits_tight_service_and_retires_all_blocks() {
    // Per-entry full-vector replacement charges would exceed this service.
    let r = SelectedScratchResources::new(
        SelectedScratchPayloadBudget::new(0),
        ImageDescriptorBudget::new(64 << 10),
    );
    let mut b = builder(&r, SelectedScratchPolicy::plain());
    for _ in 0..128 {
        b.record_external_path("/tool/bin").unwrap();
    }
    let prefix = finish(b).unwrap();
    assert_eq!(prefix.operations().len(), 128);
    assert!(r.descriptor_budget().used_bytes() < 64 << 10);
    assert_eq!(prefix.process().arguments()[0].spelling(), "tool");
    drop(prefix);
    assert_eq!(r.descriptor_budget().used_bytes(), 0);
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Allocative)]
struct Revision;
impl std::fmt::Display for Revision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "scratch-test-revision")
    }
}
impl InjectedKey for Revision {
    type Value = usize;
    fn equality(a: &usize, b: &usize) -> bool {
        a == b
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Hash, Allocative)]
struct History;
impl std::fmt::Display for History {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "scratch-test-history")
    }
}
#[async_trait]
impl Key for History {
    type Value = Result<SelectedScratchPrefix, SelectedScratchError>;
    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let revision = ctx.compute(&Revision).await.unwrap();
        let r = ctx
            .per_transaction_data()
            .data
            .get::<SelectedScratchResources>()
            .unwrap();
        let mut b = builder(r, SelectedScratchPolicy::plain());
        let p = b.path("data")?;
        b.file(&p, &[revision as u8; 64], false)?;
        finish(b)
    }
    fn equality(a: &Self::Value, b: &Self::Value) -> bool {
        matches!((a,b), (Ok(a), Ok(b)) if a == b)
    }
    fn validity(v: &Self::Value) -> bool {
        v.as_ref().err().is_none_or(|e| !e.is_transient())
    }
}

#[tokio::test]
async fn historical_prefix_payload_retains_credit_until_engine_last_drop() {
    let r = resources(128);
    let mut data = UserComputationData::default();
    data.data.set(r.dupe());
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut update = dice.updater_with_data(data);
    update.changed_to(vec![(Revision, 0)]).unwrap();
    let mut tx1 = update.commit().await;
    let old = tx1.compute(&History).await.unwrap().unwrap();
    assert_eq!(r.payload_budget().used_bytes(), 64);
    let mut update = tx1.dupe().into_updater();
    update.changed_to(vec![(Revision, 1)]).unwrap();
    let mut tx2 = update.commit().await;
    let current = tx2.compute(&History).await.unwrap().unwrap();
    assert_ne!(old, current);
    drop(old);
    drop(current);
    assert_eq!(
        r.payload_budget().used_bytes(),
        128,
        "normal retained values keep their actual allocations credited"
    );
    let mut failed = builder(&r, SelectedScratchPolicy::plain());
    let p = failed.path("extra").unwrap();
    assert_eq!(
        failed.file(&p, b"x", false),
        Err(SelectedScratchError::PayloadResourceLimit)
    );
    drop(failed);
    drop(p);
    drop(tx1);
    drop(tx2);
    dice.wait_for_idle().await;
    assert!(
        r.payload_budget().used_bytes() > 0,
        "history retirement must not be inferred from transaction retirement"
    );
    drop(dice);
    // wait_for_idle drains cancellation, while the dedicated DICE actor retires
    // its cached values after noticing that its last sender has been dropped.
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while r.payload_budget().used_bytes() != 0 || r.descriptor_budget().used_bytes() != 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("actual cached prefix allocations must retire after engine shutdown");
    assert_eq!(r.payload_budget().used_bytes(), 0);
    assert_eq!(r.descriptor_budget().used_bytes(), 0);
}
