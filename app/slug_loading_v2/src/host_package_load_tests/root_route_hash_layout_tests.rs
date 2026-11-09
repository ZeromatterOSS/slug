//! Unique-owner cost and shared-cluster accounting through real Loading holders.
//! These Allocative totals are representation checks, not native-bank or RSS proof.
use allocative::Allocative;

use super::*;
use crate::bzl_module::BzlModuleContext;
use crate::bzl_module::ExternalBzlCycleIdentity;
use crate::bzl_module::RepositoryBzlLabel;

#[derive(Allocative)]
struct OldRootRoute {
    workspace: NormalizedAbsolutePath,
    apparent_repo: ApparentRepoName,
    module_name: compact_str::CompactString,
    canonical_repo: CanonicalRepoName,
    source: slug_bzlmod_v2::RootRepositorySource,
}
#[allow(dead_code)]
#[derive(Allocative)]
enum OldSourceRoute {
    Root(OldRootRoute),
    Canonical(slug_bzlmod_v2::HostCanonicalRepositorySourceInput),
}
#[derive(Allocative)]
struct OldExternalKey {
    route: OldSourceRoute,
    label: RepositoryBzlLabel,
    context: BzlModuleContext,
}
#[derive(Allocative)]
struct OldObservedKey(OldExternalKey);
#[derive(Allocative)]
struct OldCycleIdentity {
    route: OldSourceRoute,
    label: RepositoryBzlLabel,
    context: BzlModuleContext,
}
#[derive(Allocative)]
struct OldCluster {
    route: OldRootRoute,
    source: OldSourceRoute,
    key: OldExternalKey,
    observed: OldObservedKey,
    cycle: OldCycleIdentity,
    observed_cycle: OldCycleIdentity,
}
#[derive(Allocative)]
struct ActualCluster {
    route: RootRepositoryRoute,
    source: HostRepositorySourceRoute,
    key: ExternalBzlModuleEvalKey,
    observed: ExternalBzlModuleObservationKey,
    cycle: ExternalBzlCycleIdentity,
    observed_cycle: ExternalBzlCycleIdentity,
}
fn old_route(route: &RootRepositoryRoute) -> OldRootRoute {
    OldRootRoute {
        workspace: route.workspace().clone(),
        apparent_repo: route.apparent_repo().clone(),
        module_name: route.module_name().into(),
        canonical_repo: route.canonical_repo().clone(),
        source: route.source().clone(),
    }
}
fn old_key(route: &RootRepositoryRoute, label: &RepositoryBzlLabel) -> OldExternalKey {
    OldExternalKey {
        route: OldSourceRoute::Root(old_route(route)),
        label: label.clone(),
        context: BzlModuleContext::Build,
    }
}
fn old_cycle(route: &RootRepositoryRoute, label: &RepositoryBzlLabel) -> OldCycleIdentity {
    OldCycleIdentity {
        route: OldSourceRoute::Root(old_route(route)),
        label: label.clone(),
        context: BzlModuleContext::Build,
    }
}
fn retained(value: &dyn Allocative) -> usize {
    let mut visitor = allocative::FlameGraphBuilder::default();
    visitor.visit_root(value);
    visitor.finish().flamegraph().total_size()
}
fn report(name: &str, before: &impl Allocative, current: &impl Allocative) {
    eprintln!(
        "root_route_holder_layout {name}: inline={}->{} retained={}->{}",
        std::mem::size_of_val(before),
        std::mem::size_of_val(current),
        retained(before),
        retained(current),
    );
}
fn root(source: &HostRepositorySourceRoute) -> &RootRepositoryRoute {
    let HostRepositorySourceRoute::Root(route) = source else {
        panic!("fixture must retain the actual apparent root route")
    };
    route
}

#[tokio::test]
async fn actual_selected_loading_keys_and_cycles_account_for_shared_root_route_data() {
    let (epoch, instance) = selected_registry_root_package_epoch();
    let (_tx, route) = selected_registry_visibility_transaction(epoch, instance, None).await;
    let mapping = route.bzl_repository_mapping();
    assert!(!mapping.is_empty());
    let capability = route.source_capability();
    let attributes = capability.repo_spec().unwrap().attributes.clone();
    let before = old_route(&route);
    let old_inline = std::mem::size_of::<OldRootRoute>();
    let new_inline = std::mem::size_of::<RootRepositoryRoute>();
    assert_eq!(old_inline, 256, "preserved old five-field layout");
    assert_eq!(new_inline, std::mem::size_of::<Arc<()>>());
    let old_unique = retained(&before);
    let new_unique = retained(&route);
    let unique_extra = new_unique
        .checked_sub(old_unique)
        .expect("a unique owner must account for its added allocation");
    assert!(unique_extra > 0, "unique owner cost must not be hidden");
    let data_and_header = old_inline + unique_extra - new_inline;
    assert!(data_and_header >= old_inline + std::mem::size_of::<u64>());
    eprintln!(
        "root_route_unique_owner old_inline={old_inline} outer={new_inline} data_and_arc_header={data_and_header} retained={old_unique}->{new_unique} positive_extra={unique_extra}",
    );

    // Use one actual label owner for all keys, as production clones do. The
    // whole-cluster visitor must count that label and the private route Data
    // once each, rather than summing independent sixfold visits.
    let label =
        resolve_external_load_label(&PackagePath::parse("").unwrap(), ":nested.bzl").unwrap();
    let key = ExternalBzlModuleEvalKey::new(route.clone(), label.clone());
    let observed = ExternalBzlModuleObservationKey::new(route.clone(), label.clone());
    let cycle = key.cycle_identity();
    let observed_cycle = observed.cycle_identity();
    assert_eq!(cycle, observed_cycle);
    let current = ActualCluster {
        route: route.clone(),
        source: HostRepositorySourceRoute::root(route.clone()),
        key,
        observed,
        cycle,
        observed_cycle,
    };
    let old = OldCluster {
        route: old_route(&route),
        source: OldSourceRoute::Root(old_route(&route)),
        key: old_key(&route, &label),
        observed: OldObservedKey(old_key(&route, &label)),
        cycle: old_cycle(&route, &label),
        observed_cycle: old_cycle(&route, &label),
    };
    // These assertions preserve real nested payload sharing, rather than
    // giving the new cluster an advantage through omitted mapping/spec data.
    for held in [
        &current.route,
        root(&current.source),
        root(&current.key.route),
        root(&current.observed.0.route),
        root(&current.cycle.route),
        root(&current.observed_cycle.route),
    ] {
        assert_eq!(held, &route);
        assert!(Arc::ptr_eq(&mapping, &held.bzl_repository_mapping()));
        assert!(Arc::ptr_eq(
            &attributes,
            &held.source_capability().repo_spec().unwrap().attributes
        ));
    }
    report("root", &old.route, &current.route);
    report("source", &old.source, &current.source);
    report("eval_key", &old.key, &current.key);
    report("observed_key", &old.observed, &current.observed);
    report("cycle", &old.cycle, &current.cycle);
    report(
        "observed_cycle",
        &old.observed_cycle,
        &current.observed_cycle,
    );
    let old_cluster = retained(&old);
    let new_cluster = retained(&current);
    let label_heap = retained(&label) - std::mem::size_of_val(&label);
    assert_eq!(
        new_cluster,
        std::mem::size_of_val(&current) + new_unique - new_inline + label_heap,
        "six holders must visit exactly one shared route Data allocation and one shared label payload",
    );
    eprintln!(
        "root_route_cluster holders=6 inline={}->{} retained={old_cluster}->{new_cluster}",
        std::mem::size_of_val(&old),
        std::mem::size_of_val(&current),
    );
    assert!(
        old_cluster > new_cluster,
        "the actual shared cluster must repay its positive unique-owner overhead"
    );
    // Existing compactness gates remain unchanged in their owning tests.
    assert!(std::mem::size_of::<HostRepositorySourceRoute>() <= 256);
    assert!(std::mem::size_of::<ExternalBzlModuleEvalKey>() <= 320);
    assert!(std::mem::size_of::<ExternalBzlModuleObservationKey>() <= 320);
    assert!(std::mem::size_of::<ExternalBzlCycleIdentity>() <= 320);
}
