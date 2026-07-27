/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory or the Apache License, Version 2.0
 * found in the LICENSE-APACHE file in the root directory of this source tree.
 * You may select, at your option, one of the above-listed licenses.
 */

#![allow(dead_code)] // Dormant until the later Host registry packet.

use std::fmt;
use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use dice::InjectedKey;
use dupe::Dupe;
use slug_workspace_v2::NormalizedAbsolutePath;
use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;

const DEFAULT_REGISTRY_URL: &str = "https://bcr.bazel.build/";

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(crate) struct HostRegistryUrlsInput {
    urls: Arc<SmallSet<CompactString>>,
}

impl HostRegistryUrlsInput {
    pub(crate) fn iter(&self) -> impl ExactSizeIterator<Item = &CompactString> {
        self.urls.iter()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct HostModuleMirrorOccurrence {
    registry: CompactString,
    mirrors: Arc<[CompactString]>,
}

impl HostModuleMirrorOccurrence {
    pub(crate) fn new(
        registry: impl Into<CompactString>,
        mirrors: impl Into<Arc<[CompactString]>>,
    ) -> Self {
        Self {
            registry: registry.into(),
            mirrors: mirrors.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(crate) struct HostModuleMirrorsInput {
    mirrors: Arc<SmallMap<CompactString, Arc<SmallSet<CompactString>>>>,
}

impl HostModuleMirrorsInput {
    pub(crate) fn get(&self, registry: &str) -> Option<&SmallSet<CompactString>> {
        self.mirrors.get(registry).map(Arc::as_ref)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(crate) enum HostRegistryInputsNormalizationError {
    UnknownModuleMirrorRegistries { registries: Arc<[CompactString]> },
}

impl fmt::Display for HostRegistryInputsNormalizationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownModuleMirrorRegistries { registries } => {
                write!(f, "unknown module-mirror registries: {registries:?}")
            }
        }
    }
}

impl std::error::Error for HostRegistryInputsNormalizationError {}

fn normalize_base_url(value: &str) -> CompactString {
    CompactString::new(value.trim_end_matches('/'))
}

pub(crate) fn normalize_host_registry_inputs<R, S, M>(
    registry_occurrences: R,
    mirror_occurrences: M,
) -> Result<(HostRegistryUrlsInput, HostModuleMirrorsInput), HostRegistryInputsNormalizationError>
where
    R: IntoIterator<Item = S>,
    S: AsRef<str>,
    M: IntoIterator<Item = HostModuleMirrorOccurrence>,
{
    let mut registry_urls = SmallSet::new();
    let mut has_registry_occurrence = false;
    for registry in registry_occurrences {
        has_registry_occurrence = true;
        registry_urls.insert(normalize_base_url(registry.as_ref()));
    }
    if !has_registry_occurrence {
        registry_urls.insert(DEFAULT_REGISTRY_URL.into());
    }

    let mut mirror_replacements = SmallMap::new();
    let mut has_mirror_occurrence = false;
    for occurrence in mirror_occurrences {
        has_mirror_occurrence = true;
        let mirrors = occurrence
            .mirrors
            .iter()
            .map(|mirror| normalize_base_url(mirror))
            .collect::<SmallSet<_>>();
        mirror_replacements.insert(normalize_base_url(&occurrence.registry), Arc::new(mirrors));
    }

    let unknown_registries = mirror_replacements
        .keys()
        .filter(|registry| !registry.is_empty() && !registry_urls.contains(*registry))
        .cloned()
        .collect::<Arc<[_]>>();
    if !unknown_registries.is_empty() {
        return Err(
            HostRegistryInputsNormalizationError::UnknownModuleMirrorRegistries {
                registries: unknown_registries,
            },
        );
    }

    let registry_urls = HostRegistryUrlsInput {
        urls: Arc::new(registry_urls),
    };
    if !has_mirror_occurrence {
        return Ok((
            registry_urls,
            HostModuleMirrorsInput {
                mirrors: Arc::new(SmallMap::new()),
            },
        ));
    }

    let default_mirrors = mirror_replacements
        .get("")
        .cloned()
        .unwrap_or_else(|| Arc::new(SmallSet::new()));
    let mirrors = registry_urls
        .iter()
        .map(|registry| {
            (
                registry.clone(),
                mirror_replacements
                    .get(registry)
                    .map(|mirrors| mirrors.dupe())
                    .unwrap_or_else(|| default_mirrors.dupe()),
            )
        })
        .collect::<SmallMap<_, _>>();
    Ok((
        registry_urls,
        HostModuleMirrorsInput {
            mirrors: Arc::new(mirrors),
        },
    ))
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative, Dupe)]
pub(crate) struct HostRegistryUrlsInputKey {
    workspace: NormalizedAbsolutePath,
}

impl HostRegistryUrlsInputKey {
    pub(crate) fn new(workspace: NormalizedAbsolutePath) -> Self {
        Self { workspace }
    }
}

impl fmt::Display for HostRegistryUrlsInputKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "host-registry-urls-input:{}", self.workspace)
    }
}

impl InjectedKey for HostRegistryUrlsInputKey {
    type Value = HostRegistryUrlsInput;

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative, Dupe)]
pub(crate) struct HostModuleMirrorsInputKey {
    workspace: NormalizedAbsolutePath,
}

impl HostModuleMirrorsInputKey {
    pub(crate) fn new(workspace: NormalizedAbsolutePath) -> Self {
        Self { workspace }
    }
}

impl fmt::Display for HostModuleMirrorsInputKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "host-module-mirrors-input:{}", self.workspace)
    }
}

impl InjectedKey for HostModuleMirrorsInputKey {
    type Value = HostModuleMirrorsInput;

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Allocative, Dupe)]
pub(crate) struct HostRegistryRefreshToken(u64);

impl HostRegistryRefreshToken {
    pub(crate) fn new(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative, Dupe)]
pub(crate) struct HostRegistryRefreshTokenKey {
    workspace: NormalizedAbsolutePath,
}

impl HostRegistryRefreshTokenKey {
    pub(crate) fn new(workspace: NormalizedAbsolutePath) -> Self {
        Self { workspace }
    }
}

impl fmt::Display for HostRegistryRefreshTokenKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "host-registry-refresh-token:{}", self.workspace)
    }
}

impl InjectedKey for HostRegistryRefreshTokenKey {
    type Value = HostRegistryRefreshToken;

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    use async_trait::async_trait;
    use dice::ActivationData;
    use dice::ActivationTracker;
    use dice::DetectCycles;
    use dice::Dice;
    use dice::DiceComputations;
    use dice::DiceTransaction;
    use dice::DynKey;
    use dice::Key;
    use dice::UserComputationData;
    use dice_futures::cancellation::CancellationContext;

    use super::*;
    use crate::RegistryRequestGeneration;
    use crate::RegistryRequestGenerationKey;
    use crate::RootPackagePolicyInputs;
    use crate::inject_root_package_policy_inputs;
    use crate::package_policy::RootVendorDirectoryProjectionKey;

    fn path(value: &str) -> NormalizedAbsolutePath {
        NormalizedAbsolutePath::new(value).unwrap()
    }

    fn mirror(registry: &str, mirrors: &[&str]) -> HostModuleMirrorOccurrence {
        HostModuleMirrorOccurrence::new(
            registry,
            mirrors
                .iter()
                .map(|mirror| CompactString::new(*mirror))
                .collect::<Arc<[_]>>(),
        )
    }

    fn normalized(
        registries: &[&str],
        mirrors: &[HostModuleMirrorOccurrence],
    ) -> Result<(HostRegistryUrlsInput, HostModuleMirrorsInput), HostRegistryInputsNormalizationError>
    {
        normalize_host_registry_inputs(registries.iter().copied(), mirrors.iter().cloned())
    }

    fn strings<'a>(values: impl IntoIterator<Item = &'a CompactString>) -> Vec<&'a str> {
        values.into_iter().map(CompactString::as_str).collect()
    }

    #[test]
    fn normalizes_registry_and_mirror_semantics() {
        let (default_urls, absent_mirrors) = normalized(&[], &[]).unwrap();
        assert_eq!(
            strings(default_urls.iter()),
            vec!["https://bcr.bazel.build/"]
        );
        assert!(absent_mirrors.mirrors.is_empty());

        let (urls, mirrors) = normalized(
            &[
                "https://a.example///",
                "https://a.example",
                "%workspace%/registry///",
                "",
                "not a uri///",
                "ftp://unsupported/",
            ],
            &[
                mirror(
                    "",
                    &[
                        "https://default-1///",
                        "https://default-1",
                        "%workspace%/relative/",
                    ],
                ),
                mirror("https://a.example/", &["https://stale/"]),
                mirror(
                    "https://a.example",
                    &[
                        "https://specific-2/",
                        "https://specific-1//",
                        "https://specific-2",
                    ],
                ),
                mirror("%workspace%/registry/", &[]),
            ],
        )
        .unwrap();
        assert_eq!(
            strings(urls.iter()),
            vec![
                "https://a.example",
                "%workspace%/registry",
                "",
                "not a uri",
                "ftp://unsupported",
            ]
        );
        assert_eq!(
            strings(mirrors.get("https://a.example").unwrap()),
            vec!["https://specific-2", "https://specific-1"]
        );
        assert!(mirrors.get("https://a.example/").is_none());
        assert!(mirrors.get("%workspace%/registry").unwrap().is_empty());
        assert_eq!(
            strings(mirrors.get("not a uri").unwrap()),
            vec!["https://default-1", "%workspace%/relative"]
        );
        assert_eq!(
            normalized(
                &[],
                &[mirror("https://bcr.bazel.build", &["https://mirror/"])]
            ),
            Err(
                HostRegistryInputsNormalizationError::UnknownModuleMirrorRegistries {
                    registries: Arc::from([CompactString::new("https://bcr.bazel.build")]),
                }
            )
        );
        assert_eq!(
            normalized(
                &["https://known"],
                &[
                    mirror("https://unknown-2/", &[]),
                    mirror("https://unknown-1///", &[]),
                    mirror("https://unknown-2", &["replacement"]),
                ],
            ),
            Err(
                HostRegistryInputsNormalizationError::UnknownModuleMirrorRegistries {
                    registries: Arc::from([
                        CompactString::new("https://unknown-2"),
                        CompactString::new("https://unknown-1"),
                    ]),
                }
            )
        );
        let (_, absent) = normalized(&["https://a", "https://b"], &[]).unwrap();
        let (_, explicit_empty) =
            normalized(&["https://a", "https://b"], &[mirror("", &[])]).unwrap();
        assert_ne!(absent, explicit_empty);
        assert!(absent.mirrors.is_empty());
        assert_eq!(
            explicit_empty
                .mirrors
                .iter()
                .map(|(registry, mirrors)| (registry.as_str(), mirrors.is_empty()))
                .collect::<Vec<_>>(),
            vec![("https://a", true), ("https://b", true)]
        );
    }

    #[test]
    fn fresh_reorders_iterate_differently_but_compare_equal() {
        let (urls_ab, mirrors_ab) = normalized(
            &["https://a", "https://b"],
            &[
                mirror("https://a", &["https://one", "https://two"]),
                mirror("https://b", &["https://three"]),
            ],
        )
        .unwrap();
        let (urls_ba, mirrors_ba) = normalized(
            &["https://b", "https://a"],
            &[
                mirror("https://b", &["https://three"]),
                mirror("https://a", &["https://two", "https://one"]),
            ],
        )
        .unwrap();
        assert_eq!(urls_ab, urls_ba);
        assert_eq!(mirrors_ab, mirrors_ba);
        assert_eq!(strings(urls_ab.iter()), vec!["https://a", "https://b"]);
        assert_eq!(strings(urls_ba.iter()), vec!["https://b", "https://a"]);
        assert_eq!(
            strings(mirrors_ab.get("https://a").unwrap()),
            vec!["https://one", "https://two"]
        );
        assert_eq!(
            strings(mirrors_ba.get("https://a").unwrap()),
            vec!["https://two", "https://one"]
        );
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Allocative, Dupe)]
    enum InputKind {
        Urls,
        Mirrors,
        Vendor,
        Refresh,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
    enum InputSnapshot {
        Urls(HostRegistryUrlsInput),
        Mirrors(HostModuleMirrorsInput),
        Vendor(Option<NormalizedAbsolutePath>),
        Refresh(HostRegistryRefreshToken),
    }

    static EVALUATIONS: [AtomicUsize; 4] = [
        AtomicUsize::new(0),
        AtomicUsize::new(0),
        AtomicUsize::new(0),
        AtomicUsize::new(0),
    ];

    #[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative, Dupe)]
    struct InputConsumerKey {
        workspace: NormalizedAbsolutePath,
        kind: InputKind,
    }

    impl fmt::Display for InputConsumerKey {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(
                f,
                "test-host-registry-input:{:?}:{}",
                self.kind, self.workspace
            )
        }
    }

    #[async_trait]
    impl Key for InputConsumerKey {
        type Value = Result<InputSnapshot, InputKind>;

        async fn compute(
            &self,
            ctx: &mut DiceComputations,
            _cancellations: &CancellationContext,
        ) -> Self::Value {
            let value = match self.kind {
                InputKind::Urls => ctx
                    .compute(&HostRegistryUrlsInputKey::new(self.workspace.dupe()))
                    .await
                    .map(InputSnapshot::Urls)
                    .map_err(|_| self.kind),
                InputKind::Mirrors => ctx
                    .compute(&HostModuleMirrorsInputKey::new(self.workspace.dupe()))
                    .await
                    .map(InputSnapshot::Mirrors)
                    .map_err(|_| self.kind),
                InputKind::Vendor => ctx
                    .compute(&RootVendorDirectoryProjectionKey::new(
                        self.workspace.dupe(),
                    ))
                    .await
                    .map_err(|_| self.kind)?
                    .map(InputSnapshot::Vendor)
                    .map_err(|_| self.kind),
                InputKind::Refresh => ctx
                    .compute(&HostRegistryRefreshTokenKey::new(self.workspace.dupe()))
                    .await
                    .map(InputSnapshot::Refresh)
                    .map_err(|_| self.kind),
            }?;
            EVALUATIONS[self.kind as usize].fetch_add(1, Ordering::SeqCst);
            Ok(value)
        }

        fn equality(x: &Self::Value, y: &Self::Value) -> bool {
            x == y
        }
    }

    #[derive(Default)]
    struct InputTracker(Mutex<SmallSet<InputKind>>);

    fn is_key<T: Key>(key: &DynKey) -> bool {
        key.downcast_ref::<T>().is_some()
    }
    impl ActivationTracker for InputTracker {
        fn key_activated(
            &self,
            key: &DynKey,
            dependencies: &mut dyn Iterator<Item = &DynKey>,
            _activation: ActivationData,
        ) {
            let Some(key) = key.downcast_ref::<InputConsumerKey>() else {
                return;
            };
            let Some(dependency) = dependencies.next() else {
                return;
            };
            assert!(dependencies.next().is_none());
            assert!(match key.kind {
                InputKind::Urls => is_key::<HostRegistryUrlsInputKey>(dependency),
                InputKind::Mirrors => is_key::<HostModuleMirrorsInputKey>(dependency),
                InputKind::Vendor => is_key::<RootVendorDirectoryProjectionKey>(dependency),
                InputKind::Refresh => is_key::<HostRegistryRefreshTokenKey>(dependency),
            });
            self.0.lock().unwrap().insert(key.kind);
        }
    }
    fn inject_inputs(
        updater: &mut dice::DiceTransactionUpdater,
        workspace: &NormalizedAbsolutePath,
        inputs: &(HostRegistryUrlsInput, HostModuleMirrorsInput),
    ) {
        updater
            .changed_to(vec![(
                HostRegistryUrlsInputKey::new(workspace.dupe()),
                inputs.0.dupe(),
            )])
            .unwrap();
        updater
            .changed_to(vec![(
                HostModuleMirrorsInputKey::new(workspace.dupe()),
                inputs.1.dupe(),
            )])
            .unwrap();
    }
    async fn replace<T: InjectedKey>(
        transaction: DiceTransaction,
        key: T,
        value: T::Value,
    ) -> DiceTransaction {
        let mut updater = transaction.into_updater();
        updater.changed_to(vec![(key, value)]).unwrap();
        updater.commit().await
    }

    fn key(workspace: &NormalizedAbsolutePath, kind: InputKind) -> InputConsumerKey {
        InputConsumerKey {
            workspace: workspace.dupe(),
            kind,
        }
    }

    #[tokio::test]
    async fn retained_inputs_are_scoped_independent_and_prune_by_bazel_equality() {
        for counter in &EVALUATIONS[..2] {
            counter.store(0, Ordering::SeqCst);
        }
        let workspace = path("/work/a");
        let other_workspace = path("/work/b");
        let urls_key = key(&workspace, InputKind::Urls);
        let mirrors_key = key(&workspace, InputKind::Mirrors);
        let tracker = Arc::new(InputTracker::default());
        let user_data = UserComputationData {
            activation_tracker: Some(tracker.dupe() as Arc<dyn ActivationTracker>),
            ..Default::default()
        };
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let mut transaction = dice.updater_with_data(user_data).commit().await;
        let inputs_a = normalized(
            &["https://a", "https://b"],
            &[
                mirror("https://a", &["https://one", "https://two"]),
                mirror("https://b", &[]),
            ],
        )
        .unwrap();
        let mut updater = transaction.into_updater();
        inject_inputs(&mut updater, &workspace, &inputs_a);
        transaction = updater.commit().await;
        let InputSnapshot::Urls(stored_urls) =
            transaction.compute(&urls_key).await.unwrap().unwrap()
        else {
            unreachable!()
        };
        let InputSnapshot::Mirrors(stored_mirrors) =
            transaction.compute(&mirrors_key).await.unwrap().unwrap()
        else {
            unreachable!()
        };
        assert_eq!(strings(stored_urls.iter()), vec!["https://a", "https://b"]);
        assert_eq!(
            strings(stored_mirrors.get("https://a").unwrap()),
            vec!["https://one", "https://two"]
        );
        assert_eq!(
            EVALUATIONS[..2]
                .iter()
                .map(|counter| counter.load(Ordering::SeqCst))
                .collect::<Vec<_>>(),
            vec![1, 1]
        );
        assert_eq!(tracker.0.lock().unwrap().len(), 2);
        assert_eq!(
            transaction
                .compute(&key(&other_workspace, InputKind::Urls))
                .await
                .unwrap(),
            Err(InputKind::Urls)
        );
        assert_eq!(
            transaction
                .compute(&key(&other_workspace, InputKind::Mirrors))
                .await
                .unwrap(),
            Err(InputKind::Mirrors)
        );
        let reordered = normalized(
            &["https://b", "https://a"],
            &[
                mirror("https://b", &[]),
                mirror("https://a", &["https://two", "https://one"]),
            ],
        )
        .unwrap();
        let mut updater = transaction.into_updater();
        inject_inputs(&mut updater, &workspace, &reordered);
        transaction = updater.commit().await;
        let InputSnapshot::Urls(stored_urls) =
            transaction.compute(&urls_key).await.unwrap().unwrap()
        else {
            unreachable!()
        };
        assert_eq!(strings(stored_urls.iter()), vec!["https://a", "https://b"]);
        let InputSnapshot::Mirrors(stored_mirrors) =
            transaction.compute(&mirrors_key).await.unwrap().unwrap()
        else {
            unreachable!()
        };
        assert_eq!(
            strings(stored_mirrors.get("https://a").unwrap()),
            vec!["https://one", "https://two"]
        );
        assert_eq!(EVALUATIONS[0].load(Ordering::SeqCst), 1);
        assert_eq!(EVALUATIONS[1].load(Ordering::SeqCst), 1);
        let inputs_b = normalized(
            &["https://a", "https://c"],
            &[
                mirror("https://a", &["https://changed-2", "https://changed-1"]),
                mirror("https://c", &[]),
            ],
        )
        .unwrap();
        transaction = replace(
            transaction,
            HostRegistryUrlsInputKey::new(workspace.dupe()),
            inputs_b.0,
        )
        .await;
        let InputSnapshot::Urls(changed_urls) =
            transaction.compute(&urls_key).await.unwrap().unwrap()
        else {
            unreachable!()
        };
        assert_eq!(strings(changed_urls.iter()), vec!["https://a", "https://c"]);
        assert!(transaction.compute(&mirrors_key).await.unwrap().is_ok());
        assert_eq!(EVALUATIONS[0].load(Ordering::SeqCst), 2);
        assert_eq!(EVALUATIONS[1].load(Ordering::SeqCst), 1);
        transaction = replace(
            transaction,
            HostRegistryUrlsInputKey::new(workspace.dupe()),
            inputs_a.0.dupe(),
        )
        .await;
        assert!(transaction.compute(&urls_key).await.unwrap().is_ok());
        assert_eq!(EVALUATIONS[0].load(Ordering::SeqCst), 3);
        transaction = replace(
            transaction,
            HostModuleMirrorsInputKey::new(workspace.dupe()),
            inputs_b.1,
        )
        .await;
        assert!(transaction.compute(&urls_key).await.unwrap().is_ok());
        assert!(matches!(
            transaction.compute(&mirrors_key).await.unwrap(),
            Ok(InputSnapshot::Mirrors(ref mirrors))
                if strings(mirrors.get("https://a").unwrap())
                    == ["https://changed-2", "https://changed-1"]
        ));
        assert_eq!(EVALUATIONS[0].load(Ordering::SeqCst), 3);
        assert_eq!(EVALUATIONS[1].load(Ordering::SeqCst), 2);
        transaction = replace(
            transaction,
            HostModuleMirrorsInputKey::new(workspace),
            inputs_a.1,
        )
        .await;
        assert!(transaction.compute(&mirrors_key).await.unwrap().is_ok());
        assert_eq!(EVALUATIONS[1].load(Ordering::SeqCst), 3);
    }

    fn policy(
        workspace: &NormalizedAbsolutePath,
        root: &str,
        deleted: &str,
        vendor: Option<&str>,
        utf8: &str,
    ) -> RootPackagePolicyInputs {
        RootPackagePolicyInputs::new(
            workspace.dupe(),
            Arc::from([path(root)]),
            [deleted],
            vendor.map(path),
            Some(utf8),
        )
        .unwrap()
    }

    async fn replace_policy(
        transaction: DiceTransaction,
        inputs: RootPackagePolicyInputs,
    ) -> DiceTransaction {
        let mut updater = transaction.into_updater();
        inject_root_package_policy_inputs(&mut updater, inputs).unwrap();
        updater.commit().await
    }

    #[tokio::test]
    async fn vendor_projection_and_refresh_token_are_private_independent_inputs() {
        for counter in &EVALUATIONS[2..] {
            counter.store(0, Ordering::SeqCst);
        }
        let workspace = path("/work/inputs");
        let other = path("/work/missing");
        let vendor_key = key(&workspace, InputKind::Vendor);
        let refresh_key = key(&workspace, InputKind::Refresh);
        let tracker = Arc::new(InputTracker::default());
        let user_data = UserComputationData {
            activation_tracker: Some(tracker.dupe() as Arc<dyn ActivationTracker>),
            ..Default::default()
        };
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let mut updater = dice.updater_with_data(user_data);
        inject_root_package_policy_inputs(
            &mut updater,
            policy(&workspace, "/root/a", "pkg/a", Some("/vendor/a"), "warning"),
        )
        .unwrap();
        updater
            .changed_to(vec![(
                HostRegistryRefreshTokenKey::new(workspace.dupe()),
                HostRegistryRefreshToken(1),
            )])
            .unwrap();
        let mut transaction = updater.commit().await;
        assert!(matches!(
            transaction.compute(&vendor_key).await.unwrap(),
            Ok(InputSnapshot::Vendor(Some(ref vendor))) if vendor == &path("/vendor/a")
        ));
        assert_eq!(
            transaction.compute(&refresh_key).await.unwrap(),
            Ok(InputSnapshot::Refresh(HostRegistryRefreshToken(1)))
        );
        assert_eq!(tracker.0.lock().unwrap().len(), 2);
        assert_eq!(
            transaction
                .compute(&RootVendorDirectoryProjectionKey::new(other.dupe()))
                .await
                .unwrap(),
            Err(crate::RootPackagePolicyProjectionError::MissingInput {
                workspace: other.dupe(),
            })
        );
        assert_eq!(
            transaction
                .compute(&key(&other, InputKind::Refresh))
                .await
                .unwrap(),
            Err(InputKind::Refresh)
        );
        for inputs in [
            policy(&workspace, "/root/a", "pkg/a", Some("/vendor/a"), "warning"),
            policy(&workspace, "/root/b", "pkg/a", Some("/vendor/a"), "warning"),
            policy(&workspace, "/root/b", "pkg/b", Some("/vendor/a"), "warning"),
            policy(&workspace, "/root/b", "pkg/b", Some("/vendor/a"), "error"),
        ] {
            transaction = replace_policy(transaction, inputs).await;
            assert!(transaction.compute(&vendor_key).await.unwrap().is_ok());
            assert_eq!(EVALUATIONS[2].load(Ordering::SeqCst), 1);
        }
        for (vendor, expected) in [(Some("/vendor/b"), 2), (None, 3), (Some("/vendor/a"), 4)] {
            transaction = replace_policy(
                transaction,
                policy(&workspace, "/root/b", "pkg/b", vendor, "error"),
            )
            .await;
            assert!(transaction.compute(&vendor_key).await.unwrap().is_ok());
            assert_eq!(EVALUATIONS[2].load(Ordering::SeqCst), expected);
        }
        for (key, value, expected) in [
            (
                HostRegistryRefreshTokenKey::new(workspace.dupe()),
                HostRegistryRefreshToken(1),
                1,
            ),
            (
                HostRegistryRefreshTokenKey::new(workspace.dupe()),
                HostRegistryRefreshToken(2),
                2,
            ),
            (
                HostRegistryRefreshTokenKey::new(workspace.dupe()),
                HostRegistryRefreshToken(1),
                3,
            ),
        ] {
            transaction = replace(transaction, key, value).await;
            assert!(transaction.compute(&refresh_key).await.unwrap().is_ok());
            assert_eq!(EVALUATIONS[3].load(Ordering::SeqCst), expected);
        }
        transaction = replace(
            transaction,
            RegistryRequestGenerationKey {
                workspace: workspace.as_path().to_path_buf(),
            },
            RegistryRequestGeneration(9),
        )
        .await;
        assert!(transaction.compute(&refresh_key).await.unwrap().is_ok());
        assert_eq!(EVALUATIONS[3].load(Ordering::SeqCst), 3);
    }
}
