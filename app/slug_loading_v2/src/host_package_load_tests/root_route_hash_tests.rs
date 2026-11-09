//! Fixed operational routing work on an actual selected-registry DICE route.
use std::hash::Hash;
use std::hash::Hasher;

use super::*;

#[derive(Default, Debug, PartialEq, Eq)]
struct CountingHasher {
    writes: usize,
    bytes: usize,
}
impl Hasher for CountingHasher {
    fn finish(&self) -> u64 {
        0
    }
    fn write(&mut self, bytes: &[u8]) {
        self.writes += 1;
        self.bytes += bytes.len();
    }
}
fn work(value: &impl Hash, repeats: usize) -> CountingHasher {
    let mut state = CountingHasher::default();
    for _ in 0..repeats {
        value.hash(&mut state);
    }
    state
}
#[derive(Default, Debug, PartialEq, Eq)]
struct Stream(Vec<Vec<u8>>);
impl Hasher for Stream {
    fn finish(&self) -> u64 {
        0
    }
    fn write(&mut self, bytes: &[u8]) {
        self.0.push(bytes.to_vec());
    }
}
fn stream(value: &impl Hash) -> Stream {
    let mut result = Stream::default();
    value.hash(&mut result);
    result
}
fn fixed_work(name: &str, actual: CountingHasher, tail: &CountingHasher) -> bool {
    let expected = CountingHasher {
        writes: 64 * (1 + tail.writes),
        bytes: 64 * (8 + tail.bytes),
    };
    eprintln!(
        "root_route_operational_hash {name}: actual_writes={} actual_bytes={} expected_writes={} expected_bytes={}",
        actual.writes, actual.bytes, expected.writes, expected.bytes,
    );
    actual == expected
}

#[tokio::test]
async fn actual_selected_root_route_keys_hash_one_operational_word_and_keep_full_generic_stream() {
    let (epoch, instance) = selected_registry_root_package_epoch();
    let (mut tx, route) = selected_registry_visibility_transaction(epoch, instance, None).await;
    let mapping = route.bzl_repository_mapping();
    assert!(
        !mapping.is_empty(),
        "fixture must exercise selected mapping Hash work"
    );
    assert!(Arc::ptr_eq(
        &mapping,
        &route.clone().bzl_repository_mapping()
    ));
    let mapping_weak = Arc::downgrade(&mapping);
    let key = external_bzl_key(route.clone(), "", "nested.bzl");
    let observed_key = observed_external_bzl_key(route.clone(), "", "nested.bzl");
    let cycle = key.cycle_identity();
    let observed_cycle = observed_key.cycle_identity();
    assert_eq!(cycle, observed_cycle);

    // Establish that the measured keys belong to the actual admitted module
    // consumer, rather than substituting a test-only routing key.
    let observed = tx.compute(&observed_key).await.unwrap();
    let carrier = observed_external(&observed);
    let module = carrier.result().as_ref().as_ref().unwrap();
    assert!(!module.manifest.reachable.is_empty());
    assert!(ExternalBzlModuleObservationKey::validity(&observed));
    let legacy = tx.compute(&key).await.unwrap();
    let LoadingPreparationOutcome::Complete(legacy) = legacy else {
        panic!("complete selected-registry module returned Need")
    };
    assert!(legacy.as_ref().is_ok());

    // Generic RootRepositoryRoute Hash remains its original full five-field
    // stream. Only the operational source-route wrapper may use a weak word.
    let full_fields = (
        route.workspace(),
        route.apparent_repo(),
        route.module_name(),
        route.canonical_repo(),
        route.source(),
    );
    assert_eq!(stream(&route), stream(&full_fields));
    assert_eq!(stream(&route), stream(&route.clone()));
    let full = work(&route, 64);
    assert!(full.writes > 64 && full.bytes > 512);
    let one_full_writer = work(&route, 1);
    assert_eq!(full.bytes, 64 * one_full_writer.bytes);
    assert!(one_full_writer.bytes + 512 < full.bytes);
    eprintln!(
        "root_route_construction_writer_bytes={} original_64_writer_bytes={}",
        one_full_writer.bytes, full.bytes
    );

    let source = HostRepositorySourceRoute::root(route.clone());
    let label =
        resolve_external_load_label(&PackagePath::parse("").unwrap(), ":nested.bzl").unwrap();
    let tail = work(&(label, crate::bzl_module::BzlModuleContext::Build), 1);
    let empty = CountingHasher::default();
    // Collect all real wrapper metrics before asserting, so baseline evidence
    // covers the source route, both key kinds and both cycle identities.
    let mut bounded = fixed_work("source_route", work(&source, 64), &empty);
    bounded &= fixed_work("external_key", work(&key, 64), &tail);
    bounded &= fixed_work("observed_external_key", work(&observed_key, 64), &tail);
    bounded &= fixed_work("cycle_identity", work(&cycle, 64), &tail);
    bounded &= fixed_work("observed_cycle_identity", work(&observed_cycle, 64), &tail);
    assert!(Arc::ptr_eq(&mapping, &mapping_weak.upgrade().unwrap()));
    assert_eq!(source, HostRepositorySourceRoute::root(route.clone()));
    assert!(
        bounded,
        "operational root-route owners must hash one word, then their original remaining fields"
    );
}
