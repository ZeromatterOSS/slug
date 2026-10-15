//! Explicit Linux/empty-environment inputs for convenience Analysis fixtures.
//! Low-level request owners retain their own platform and environment injection.
use std::path::Path;

use dice::DiceTransactionUpdater;
use dice::UserComputationData;
use slug_bzlmod_v2::RepositoryEnvironmentCell;
use slug_bzlmod_v2::RepositoryEnvironmentCellKey;
use slug_bzlmod_v2::RepositoryEnvironmentNameFrontier;
use slug_bzlmod_v2::RepositoryEnvironmentSnapshot;
use slug_bzlmod_v2::RepositoryEnvironmentSnapshotKey;
use slug_bzlmod_v2::RepositoryHostInputTransaction;
use slug_bzlmod_v2::RepositoryPlatform;
use slug_bzlmod_v2::RepositoryPlatformKey;
use slug_bzlmod_v2::SelectedExtensionDownloadTransaction;
use slug_workspace_v2::NormalizedAbsolutePath;

// Actual absent-name fixture input from the original MSVC_ENVVARS declaration
// imported by winsdk_configure, independently exercised by Loading's
// exact_builtin_winsdk_reaches_generic_environment_retry test. Knowing
// absence is distinct from an unknown name; this does not change repository
// evaluation or automatically repair any other Need family.
const ABSENT_ENVIRONMENT: &[&str] = &[
    "BAZEL_VC",
    "BAZEL_VC_FULL_VERSION",
    "BAZEL_VS",
    "BAZEL_WINSDK_FULL_VERSION",
    "TEMP",
    "TMP",
    "VS100COMNTOOLS",
    "VS110COMNTOOLS",
    "VS120COMNTOOLS",
    "VS140COMNTOOLS",
    "VS150COMNTOOLS",
    "VS160COMNTOOLS",
    "VS90COMNTOOLS",
];

pub(in super::super) struct RepositoryHostInputs {
    workspace: NormalizedAbsolutePath,
    snapshot: RepositoryEnvironmentSnapshot,
    frontier: RepositoryEnvironmentNameFrontier,
}

impl RepositoryHostInputs {
    pub(in super::super) fn prepare(workspace: &Path, data: &mut UserComputationData) -> Self {
        let workspace = NormalizedAbsolutePath::new(workspace.to_path_buf()).unwrap();
        if data.data.get::<RepositoryHostInputTransaction>().is_err() {
            data.data.set(RepositoryHostInputTransaction::new(
                RepositoryEnvironmentSnapshot::empty(),
                RepositoryEnvironmentNameFrontier::from_unsorted(
                    ABSENT_ENVIRONMENT.iter().map(|name| (*name).into()),
                ),
            ));
        }
        if data
            .data
            .get::<SelectedExtensionDownloadTransaction>()
            .is_err()
        {
            data.data.set(SelectedExtensionDownloadTransaction::empty(
                workspace.clone(),
            ));
        }
        let snapshot = data
            .data
            .get::<RepositoryHostInputTransaction>()
            .unwrap()
            .snapshot()
            .clone();
        let frontier = data
            .data
            .get::<RepositoryHostInputTransaction>()
            .unwrap()
            .frontier()
            .clone();
        Self {
            workspace,
            snapshot,
            frontier,
        }
    }

    pub(in super::super) fn inject(self, updater: &mut DiceTransactionUpdater) {
        let cells = self
            .frontier
            .iter()
            .map(|name| {
                (
                    RepositoryEnvironmentCellKey::new(self.workspace.clone(), name.clone()),
                    RepositoryEnvironmentCell::observed(self.snapshot.get(name).cloned()),
                )
            })
            .collect::<Vec<_>>();
        updater.changed_to(cells).unwrap();
        updater
            .changed_to(vec![(
                RepositoryPlatformKey::new(self.workspace.clone()),
                RepositoryPlatform::new("linux", "x86_64"),
            )])
            .unwrap();
        updater
            .changed_to(vec![(
                RepositoryEnvironmentSnapshotKey::new(self.workspace),
                self.snapshot,
            )])
            .unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn convenience_bundle_preserves_userdata_and_injects_matching_host_inputs() {
        let root = super::super::super::scratch();
        let workspace = NormalizedAbsolutePath::new(root.clone()).unwrap();
        let mut data = UserComputationData::default();
        data.data.set(23usize);
        let inputs = RepositoryHostInputs::prepare(&root, &mut data);
        assert_eq!(*data.data.get::<usize>().unwrap(), 23);
        assert!(
            data.data
                .get::<RepositoryHostInputTransaction>()
                .unwrap()
                .snapshot()
                .is_empty()
        );
        let download = data
            .data
            .get::<SelectedExtensionDownloadTransaction>()
            .unwrap();
        assert_eq!(download.workspace(), &workspace);
        assert!(download.authorized().is_empty());
        assert!(download.entries().is_empty());
        assert_eq!(
            data.data
                .get::<RepositoryHostInputTransaction>()
                .unwrap()
                .frontier()
                .iter()
                .map(|name| name.as_str())
                .collect::<Vec<_>>(),
            ABSENT_ENVIRONMENT
        );

        let dice = dice::Dice::builder().build(dice::DetectCycles::Enabled);
        let mut updater = dice.updater_with_data(data);
        inputs.inject(&mut updater);
        let mut tx = updater.commit().await;
        let platform = tx
            .compute(&RepositoryPlatformKey::new(workspace.clone()))
            .await
            .unwrap();
        assert_eq!((platform.os_name(), platform.arch()), ("linux", "x86_64"));
        for name in ABSENT_ENVIRONMENT {
            let cell = tx
                .compute(&RepositoryEnvironmentCellKey::new(workspace.clone(), *name))
                .await
                .unwrap();
            assert_eq!(cell.value(), Some(&None));
        }

        assert!(
            tx.compute(&RepositoryEnvironmentSnapshotKey::new(workspace.clone()))
                .await
                .unwrap()
                .is_empty()
        );

        let snapshot = RepositoryEnvironmentSnapshot::from_canonical(vec![
            slug_bzlmod_v2::RepositoryEnvironmentEntry::new("EXPLICIT", "value"),
        ])
        .unwrap();
        let mut data = UserComputationData::default();
        data.data.set(RepositoryHostInputTransaction::new(
            snapshot.clone(),
            slug_bzlmod_v2::RepositoryEnvironmentNameFrontier::from_unsorted(["EXPLICIT".into()]),
        ));
        let foreign = NormalizedAbsolutePath::new(root.join("caller-download-owner")).unwrap();
        data.data
            .set(SelectedExtensionDownloadTransaction::empty(foreign.clone()));
        let inputs = RepositoryHostInputs::prepare(&root, &mut data);
        assert_eq!(inputs.snapshot, snapshot);
        assert_eq!(
            data.data
                .get::<RepositoryHostInputTransaction>()
                .unwrap()
                .frontier()
                .iter()
                .map(|name| name.as_str())
                .collect::<Vec<_>>(),
            ["EXPLICIT"]
        );

        assert_eq!(
            data.data
                .get::<RepositoryHostInputTransaction>()
                .unwrap()
                .snapshot(),
            &snapshot
        );
        assert_eq!(
            data.data
                .get::<SelectedExtensionDownloadTransaction>()
                .unwrap()
                .workspace(),
            &foreign
        );
        let mut updater = dice.updater_with_data(data);
        inputs.inject(&mut updater);
        // DICE admits each injected key once per updater. An explicit caller
        // platform in the next transaction replaces convenience defaults.
        let mut tx = updater.commit().await;
        assert_eq!(
            tx.compute(&RepositoryEnvironmentSnapshotKey::new(workspace.clone()))
                .await
                .unwrap(),
            snapshot
        );
        let cell = tx
            .compute(&RepositoryEnvironmentCellKey::new(
                workspace.clone(),
                "EXPLICIT",
            ))
            .await
            .unwrap();
        assert_eq!(cell.value().unwrap().as_deref(), Some("value"));
        let mut updater = tx.into_updater();
        updater
            .changed_to(vec![(
                RepositoryPlatformKey::new(workspace.clone()),
                RepositoryPlatform::new("mac os x", "aarch64"),
            )])
            .unwrap();
        let mut tx = updater.commit().await;
        assert_eq!(
            tx.compute(&RepositoryEnvironmentSnapshotKey::new(workspace.clone()))
                .await
                .unwrap(),
            snapshot
        );
        let platform = tx
            .compute(&RepositoryPlatformKey::new(workspace))
            .await
            .unwrap();
        assert_eq!(
            (platform.os_name(), platform.arch()),
            ("mac os x", "aarch64")
        );
    }

    #[test]
    fn current_local_inventory_keeps_selected_registry_and_authored_sources_distinct() {
        let root = super::super::super::scratch();
        let source = "module(name='root')\nlocal_path_override(module_name='bazel_features_globals',path='custom_globals')\nlocal_path_override(module_name='bazel_features_version',path='custom_version')\nlocal_path_override(module_name='platforms',path='custom_platforms')\nbazel_dep(name='rules_cc',version='0.2.17')\narchive_override(module_name='protobuf',urls=['https://invalid.example/proto'])\n";
        std::fs::write(root.join("MODULE.bazel"), source).unwrap();
        assert_eq!(
            super::super::local_repositories(&root),
            vec![
                (
                    "bazel_features_globals+".to_owned(),
                    "custom_globals".to_owned()
                ),
                (
                    "bazel_features_version+".to_owned(),
                    "custom_version".to_owned()
                ),
                ("platforms".to_owned(), "custom_platforms".to_owned()),
                ("platforms+".to_owned(), "custom_platforms".to_owned()),
            ]
        );
        assert_eq!(
            std::fs::read_to_string(root.join("MODULE.bazel")).unwrap(),
            source
        );
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 1);
    }
}
