use std::fmt;

use allocative::Allocative;
use dice::CancellationContext;
use dice::DiceComputations;
use slug_workspace_v2::PathObservationKey;

use super::*;
use crate::ModuleRegistrationExpansion;
use crate::ModuleRegistrationExpansionObservationError;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
enum Consumer {
    Module(ModuleRegistrationExpansionObservationKey),
    Command(CommandRegistrationExpansionObservationKey),
}

impl fmt::Display for Consumer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("registration-projection-consumer")
    }
}

#[async_trait::async_trait]
impl Key for Consumer {
    type Value = LoadingPreparationOutcome<
        Result<Arc<ModuleRegistrationExpansion>, ModuleRegistrationExpansionObservationError>,
    >;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match self {
            Self::Module(key) => key.compute_semantic(ctx).await.unwrap(),
            Self::Command(key) => key.compute_semantic(ctx).await.unwrap(),
        }
    }

    fn equality(left: &Self::Value, right: &Self::Value) -> bool {
        left.complete_eq(right)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

impl Consumer {
    fn base_name(&self) -> String {
        match self {
            Self::Module(key) => key.to_string(),
            Self::Command(key) => key.to_string(),
        }
    }

    async fn observed(&self, tx: &mut DiceTransaction) -> PathObservationEpoch {
        match self {
            Self::Module(key) => {
                let LoadingPreparationOutcome::Complete(Ok(observed)) =
                    tx.compute(key).await.unwrap()
                else {
                    panic!("MODULE expansion should complete")
                };
                observed.observations().dupe()
            }
            Self::Command(key) => {
                let LoadingPreparationOutcome::Complete(Ok(observed)) =
                    tx.compute(key).await.unwrap()
                else {
                    panic!("command expansion should complete")
                };
                observed.observations().dupe()
            }
        }
    }
}

fn complete(value: <Consumer as Key>::Value) -> Arc<ModuleRegistrationExpansion> {
    let LoadingPreparationOutcome::Complete(Ok(value)) = value else {
        panic!("expected a complete registration expansion, got {value:?}")
    };
    value
}

#[tokio::test]
async fn observed_registration_projections_reuse_on_refresh_and_recover_from_need() {
    const MODULE: &str =
        "module(name = 'bazel_tools')\nregister_toolchains('//pkg:all', '//pkg:*')\n";
    const BUILD: &str = concat!(
        "filegroup(name = 'impl')\n",
        "toolchain_type(name = 'type')\n",
        "toolchain(name = 'all', toolchain_type = ':type', toolchain = ':impl')\n",
        "toolchain(name = 'last', toolchain_type = ':type', toolchain = ':impl')\n",
    );
    let configuration = command_configuration([CommandConfigurationOccurrence::extra_toolchains(
        "//pkg:*,//pkg:all",
    )]);
    let build_lstat = EpochBuilder::demand(
        "/workspace/pkg/BUILD.bazel",
        PathObservationOperation::Lstat,
    );
    let package_lstat = EpochBuilder::demand("/workspace/pkg", PathObservationOperation::Lstat);
    for key in [
        Consumer::Module(ModuleRegistrationExpansionObservationKey::toolchains(
            workspace(),
        )),
        Consumer::Command(CommandRegistrationExpansionObservationKey::toolchains(
            workspace(),
            configuration.dupe(),
        )),
    ] {
        let tracker = Arc::new(ExpansionTracker::default());
        let old = EpochBuilder::root_package(MODULE, BUILD, 1);
        let mut tx = transaction_with_tracker(MODULE, old.dupe(), true, Some(tracker.dupe())).await;
        let first = complete(tx.compute(&key).await.unwrap());
        assert_eq!(labels(&first), ["@@//pkg:all", "@@//pkg:last"]);
        assert_eq!(first.ambiguities().len(), 1);
        tracker.take();

        let fresh = EpochBuilder::root_package(MODULE, BUILD, 2);
        let mut tx = update_module_epoch(tx, MODULE, fresh.dupe()).await;
        let refreshed = complete(tx.compute(&key).await.unwrap());
        assert!(Arc::ptr_eq(&first, &refreshed));
        let rows = tracker.take();
        let consumer = activation_for(&rows, &key);
        assert_eq!(consumer.kind, ActivationKind::Reused);
        let base = activation_for(&rows, &key.base_name());
        assert_eq!(base.kind, ActivationKind::Evaluated);
        assert!(
            matches!(base.batch.as_ref().unwrap().events(), [EvaluationEvent::Diagnostic { level: EvaluationDiagnosticLevel::Warning, text }] if text.contains("//pkg:all"))
        );
        let path = activation_for(&rows, &PathObservationKey::new(build_lstat.dupe()));
        let closure = tx.activation_closure([consumer.node]).await.unwrap();
        let position = |id| {
            closure
                .nodes()
                .iter()
                .position(|node| node.node() == id)
                .unwrap()
        };
        assert!(position(path.node) < position(base.node));
        assert!(position(base.node) < position(consumer.node));
        let observed = key.observed(&mut tx).await;
        assert_ne!(observed.get(&build_lstat), old.get(&build_lstat));
        assert!(Arc::ptr_eq(
            observed.get(&build_lstat).unwrap(),
            fresh.get(&build_lstat).unwrap()
        ));
        tracker.take();

        let missing = EpochBuilder::root_package_builder(MODULE, 2).build();
        let mut tx = update_module_epoch(tx, MODULE, missing).await;
        let pending = tx.compute(&key).await.unwrap();
        let LoadingPreparationOutcome::Need(ref need) = pending else {
            panic!("revoked package authority must request current observations")
        };
        assert_eq!(
            need,
            &slug_bzlmod_v2::SourcePreparationNeeds::path(
                slug_workspace_v2::NeedPathObservations::singleton(package_lstat.dupe()),
            )
        );
        assert!(!Consumer::validity(&pending));
        assert!(!Consumer::equality(&pending, &pending));
        let rows = tracker.take();
        let base = activation_for(&rows, &key.base_name());
        assert_eq!(base.kind, ActivationKind::Evaluated);
        assert!(base.batch.is_none());

        let mut tx = update_module_epoch(tx, MODULE, fresh).await;
        let restored = complete(tx.compute(&key).await.unwrap());
        assert!(Arc::ptr_eq(&first, &restored));
        assert_eq!(
            activation_for(&tracker.take(), &key).kind,
            ActivationKind::Reused
        );

        let edited = BUILD.replace("name = 'last'", "name = 'changed'");
        let mut tx =
            update_module_epoch(tx, MODULE, EpochBuilder::root_package(MODULE, &edited, 3)).await;
        let changed = complete(tx.compute(&key).await.unwrap());
        assert_eq!(labels(&changed), ["@@//pkg:all", "@@//pkg:changed"]);
        assert_eq!(changed.ambiguities(), first.ambiguities());
        assert_eq!(
            activation_for(&tracker.take(), &key).kind,
            ActivationKind::Evaluated
        );
    }
}
