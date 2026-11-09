use std::collections::HashMap;
use std::hash::BuildHasherDefault;
use std::hash::Hash;
use std::hash::Hasher;

use super::*;

#[derive(Debug, Default, PartialEq, Eq)]
struct HashWork {
    writes: usize,
    bytes: usize,
}

impl Hasher for HashWork {
    fn finish(&self) -> u64 {
        0
    }
    fn write(&mut self, bytes: &[u8]) {
        self.writes += 1;
        self.bytes += bytes.len();
    }
}

fn configuration(suffix: &str) -> SlugConfiguration {
    command_configuration([
        CommandConfigurationOccurrence::extra_toolchains(format!("//pkg:t{suffix}")),
        CommandConfigurationOccurrence::extra_execution_platforms(format!("//pkg:p{suffix}")),
    ])
}

fn keys(
    workspace: &NormalizedAbsolutePath,
    configuration: &SlugConfiguration,
    family: ModuleRegistrationFamily,
) -> (
    CommandRegistrationExpansionKey,
    CommandRegistrationExpansionObservationKey,
) {
    match family {
        ModuleRegistrationFamily::Toolchains => (
            CommandRegistrationExpansionKey::toolchains(workspace.dupe(), configuration.dupe()),
            CommandRegistrationExpansionObservationKey::toolchains(
                workspace.dupe(),
                configuration.dupe(),
            ),
        ),
        ModuleRegistrationFamily::ExecutionPlatforms => (
            CommandRegistrationExpansionKey::execution_platforms(
                workspace.dupe(),
                configuration.dupe(),
            ),
            CommandRegistrationExpansionObservationKey::execution_platforms(
                workspace.dupe(),
                configuration.dupe(),
            ),
        ),
    }
}

fn fresh_key_work_is_bounded<K: Hash>(
    fresh: impl Fn() -> K,
    workspace: &NormalizedAbsolutePath,
    configuration: &SlugConfiguration,
    family: ModuleRegistrationFamily,
) -> bool {
    let mut expected = HashWork::default();
    let mut actual = HashWork::default();
    for _ in 0..64 {
        // Preserve the original outer field streams; only this operational
        // configuration slot uses the already owned 32-byte projection.
        workspace.hash(&mut expected);
        expected.write(configuration.projection().as_bytes());
        family.hash(&mut expected);
        fresh().hash(&mut actual);
    }
    println!(
        "COMMAND_CONFIG_HASH_WORK key={} family={family} constructions=64 actual_writes={} actual_bytes={} expected_writes={} expected_bytes={} canonical_config_bytes={} expected_config_bytes=32",
        std::any::type_name::<K>(),
        actual.writes,
        actual.bytes,
        expected.writes,
        expected.bytes,
        configuration.canonical_bytes().len(),
    );
    actual == expected
}

#[tokio::test]
async fn command_registration_fresh_keys_hash_bounded_config_after_real_a_b_a() {
    let module = "module(name='bazel_tools')\n";
    let build = concat!(
        "filegroup(name='impl')\n",
        "toolchain_type(name='type')\n",
        "toolchain(name='ta',toolchain_type=':type',toolchain=':impl')\n",
        "toolchain(name='tb',toolchain_type=':type',toolchain=':impl')\n",
        "platform(name='pa')\nplatform(name='pb')\n",
    );
    let workspace = workspace();
    let config_a = configuration("a");
    let config_b = configuration("b");
    let tracker = Arc::new(ExpansionTracker::default());
    let mut tx = transaction_with_tracker(
        module,
        EpochBuilder::root_package(module, build, 55),
        true,
        Some(tracker.dupe()),
    )
    .await;
    // Establish actual DICE results, ordering and retained reuse before the
    // work assertion. Both base keys and observed wrappers are real consumers.
    for (family, expected_a, expected_b) in [
        (
            ModuleRegistrationFamily::Toolchains,
            "@@//pkg:ta",
            "@@//pkg:tb",
        ),
        (
            ModuleRegistrationFamily::ExecutionPlatforms,
            "@@//pkg:pa",
            "@@//pkg:pb",
        ),
    ] {
        let (base_a, observed_a) = keys(&workspace, &config_a, family);
        let (base_b, observed_b) = keys(&workspace, &config_b, family);
        let a = tx.compute(&base_a).await.unwrap();
        let b = tx.compute(&base_b).await.unwrap();
        assert!(!CommandRegistrationExpansionKey::equality(&a, &b));
        tracker.take();
        let restored = tx.compute(&base_a).await.unwrap();
        let (
            LoadingPreparationOutcome::Complete(a),
            LoadingPreparationOutcome::Complete(b),
            LoadingPreparationOutcome::Complete(restored),
        ) = (&a, &b, &restored)
        else {
            panic!("complete base-key A/B/A fixture returned Need");
        };
        assert_eq!(labels(a), [expected_a]);
        assert_eq!(labels(b), [expected_b]);
        assert!(Arc::ptr_eq(a, restored));
        assert_eq!(
            activation_for(&tracker.take(), &base_a).kind,
            ActivationKind::Reused
        );

        let a = tx.compute(&observed_a).await.unwrap();
        let b = tx.compute(&observed_b).await.unwrap();
        assert!(!CommandRegistrationExpansionObservationKey::equality(
            &a, &b
        ));
        tracker.take();
        let restored = tx.compute(&observed_a).await.unwrap();
        let (
            LoadingPreparationOutcome::Complete(Ok(a)),
            LoadingPreparationOutcome::Complete(Ok(b)),
            LoadingPreparationOutcome::Complete(Ok(restored)),
        ) = (&a, &b, &restored)
        else {
            panic!("complete observed A/B/A fixture returned Need/error");
        };
        assert_eq!(labels(a.result()), [expected_a]);
        assert_eq!(labels(b.result()), [expected_b]);
        assert!(Arc::ptr_eq(a.result(), restored.result()));
        assert_eq!(
            activation_for(&tracker.take(), &observed_a).kind,
            ActivationKind::Reused
        );
        assert_eq!(a.observations(), restored.observations());
    }
    let mut bounded = true;
    for family in [
        ModuleRegistrationFamily::Toolchains,
        ModuleRegistrationFamily::ExecutionPlatforms,
    ] {
        bounded &= fresh_key_work_is_bounded(
            || match family {
                ModuleRegistrationFamily::Toolchains => {
                    CommandRegistrationExpansionKey::toolchains(workspace.dupe(), config_a.dupe())
                }
                ModuleRegistrationFamily::ExecutionPlatforms => {
                    CommandRegistrationExpansionKey::execution_platforms(
                        workspace.dupe(),
                        config_a.dupe(),
                    )
                }
            },
            &workspace,
            &config_a,
            family,
        );
        bounded &= fresh_key_work_is_bounded(
            || match family {
                ModuleRegistrationFamily::Toolchains => {
                    CommandRegistrationExpansionObservationKey::toolchains(
                        workspace.dupe(),
                        config_a.dupe(),
                    )
                }
                ModuleRegistrationFamily::ExecutionPlatforms => {
                    CommandRegistrationExpansionObservationKey::execution_platforms(
                        workspace.dupe(),
                        config_a.dupe(),
                    )
                }
            },
            &workspace,
            &config_a,
            family,
        );
    }
    assert!(
        bounded,
        "every fresh-key hash must use the bounded configuration slot"
    );
}

#[derive(Default)]
struct ConstantHasher;
impl Hasher for ConstantHasher {
    fn finish(&self) -> u64 {
        0
    }
    fn write(&mut self, _bytes: &[u8]) {}
}

fn assert_collisions<K: Hash + Eq + Clone + std::fmt::Debug>(values: &[K], equal: K) {
    let mut table = HashMap::<K, usize, BuildHasherDefault<ConstantHasher>>::default();
    for (index, value) in values.iter().enumerate() {
        assert_eq!(table.insert(value.clone(), index), None);
        for previous in &values[..index] {
            assert_ne!(previous, value);
        }
    }
    assert_eq!(table.len(), values.len());
    for (index, value) in values.iter().enumerate() {
        assert_eq!(table.get(value), Some(&index));
    }
    assert_eq!(equal, values[0]);
    assert_eq!(table.get(&equal), Some(&0));
    assert_eq!(table.insert(equal, values.len()), Some(0));
    assert_eq!(table.len(), values.len());
}

#[test]
fn command_registration_projection_routing_keeps_full_structural_identity() {
    let workspace = workspace();
    let other_workspace = NormalizedAbsolutePath::new("/different-workspace").unwrap();
    let a = configuration("a");
    let independently_equal = configuration("a");
    assert_eq!(a, independently_equal);
    assert!(!std::ptr::eq(
        a.canonical_bytes(),
        independently_equal.canonical_bytes()
    ));
    let b = configuration("b");
    let unrelated =
        a.with_incompatible_auto_exec_groups(!a.incompatible_auto_exec_groups().unwrap());
    assert_ne!(a, unrelated);
    let toolchains = ModuleRegistrationFamily::Toolchains;
    let platforms = ModuleRegistrationFamily::ExecutionPlatforms;
    let variants = [
        keys(&workspace, &a, toolchains),
        keys(&workspace, &a, platforms),
        keys(&other_workspace, &a, toolchains),
        keys(&workspace, &b, toolchains),
        keys(&workspace, &unrelated, toolchains),
    ];
    let base = variants
        .iter()
        .map(|(base, _)| base.clone())
        .collect::<Vec<_>>();
    let observed = variants
        .iter()
        .map(|(_, observed)| observed.clone())
        .collect::<Vec<_>>();
    let equal = keys(&workspace, &independently_equal, toolchains);
    assert_collisions(&base, equal.0);
    assert_collisions(&observed, equal.1);
    // The optimization must not touch global full-byte configuration hashing.
    let mut global = HashWork::default();
    a.hash(&mut global);
    assert_eq!(
        global,
        HashWork {
            writes: 1,
            bytes: a.canonical_bytes().len()
        }
    );
}
