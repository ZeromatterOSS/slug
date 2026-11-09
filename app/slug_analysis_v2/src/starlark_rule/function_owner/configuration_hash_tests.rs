//! Real immutable owner hash/equality bridge, not a configured-evaluation result.
use std::collections::HashMap;
use std::hash::BuildHasherDefault;

use slug_configuration_v2::native::host::AutoCpuToken;
use slug_configuration_v2::native::host::HostConversionInputs;
use slug_configuration_v2::native::host::HostPathFlavor;

use super::*;

#[derive(Default)]
struct Stream(Vec<u8>);
impl Hasher for Stream {
    fn finish(&self) -> u64 {
        0
    }
    fn write(&mut self, bytes: &[u8]) {
        self.0.extend_from_slice(bytes);
    }
}
#[derive(Default)]
struct Collision;
impl Hasher for Collision {
    fn finish(&self) -> u64 {
        0
    }
    fn write(&mut self, _: &[u8]) {}
}
fn target(changed: bool) -> ConfiguredTargetKey {
    let host = HostConversionInputs::new(
        Some(AutoCpuToken::K8),
        Some(HostPathFlavor::Unix),
        None,
        Arc::from([]),
        Arc::from([]),
    )
    .unwrap();
    let config = slug_configuration_v2::SlugConfiguration::default_target(&host).unwrap();
    let config = if changed {
        config.with_incompatible_auto_exec_groups(!config.incompatible_auto_exec_groups().unwrap())
    } else {
        config
    };
    ConfiguredTargetKey::new(
        slug_identity_v2::CanonicalLabel::parse("@@//pkg:owned").unwrap(),
        crate::ConfigurationKey::from_slug(config),
    )
}
fn stable(key: ConfiguredTargetKey) -> StableKey {
    let mut audit = Audit::default();
    // The production prepare prefix, including this key's actual inline size.
    audit.row::<StableKey>().unwrap();
    audit.key(&key).unwrap();
    StableKey {
        key,
        shape: audit.native.intrinsic_shape(),
    }
}
fn owner(key: ConfiguredTargetKey) -> FunctionEvaluationOwner {
    FunctionEvaluationOwner::new(
        FunctionOwnerDomain::ConfiguredRule,
        Arc::new(stable(key)),
        Arc::new(TestPart { visits: 1 }),
    )
    .unwrap()
}

#[test]
fn configuration_bucket_flows_through_structural_and_native_function_owners() {
    let first_key = target(false);
    let first = stable(first_key.clone());
    let equal = stable(target(false));
    let changed = stable(target(true));
    assert!(first.eq_part(&equal));
    assert!(!first.eq_part(&changed));
    let mut direct = Stream::default();
    first_key.hash(&mut direct);
    let mut part = Stream::default();
    first.hash_part(&mut part);
    assert_eq!(part.0, direct.0);

    let first = owner(first_key.clone());
    let equal = owner(target(false));
    let changed = owner(target(true));
    assert_eq!(first, equal);
    assert_ne!(first, changed);
    let shape = |owner: &FunctionEvaluationOwner| {
        let shape = owner.retained_shape();
        (
            shape.nodes,
            shape.edges,
            shape.bytes,
            shape.visits,
            shape.depth,
        )
    };
    assert_eq!(shape(&first), shape(&equal));
    let mut expected = Stream::default();
    FunctionOwnerDomain::ConfiguredRule.hash(&mut expected);
    first_key.hash(&mut expected);
    1u8.hash(&mut expected); // identical TestPart input-validity witness
    let mut actual = Stream::default();
    first.hash(&mut actual);
    assert_eq!(actual.0, expected.0);

    // This constructs the actual NativeFunctionOwner adapter; no evaluator,
    // native root, publication result or producer authority is manufactured.
    let first_session = slug_loading_v2::function_owner::function_symbol_session(first.clone());
    let equal_session = slug_loading_v2::function_owner::function_symbol_session(equal.clone());
    let changed_session = slug_loading_v2::function_owner::function_symbol_session(changed.clone());
    assert!(first_session.owner().equals(&**equal_session.owner()));
    assert!(!first_session.owner().equals(&**changed_session.owner()));
    let mut adapter = Stream::default();
    first_session.owner().hash_owner(&mut adapter);
    assert_eq!(adapter.0, expected.0);

    let mut map =
        HashMap::<FunctionEvaluationOwner, usize, BuildHasherDefault<Collision>>::default();
    assert!(map.insert(first, 1).is_none());
    assert_eq!(map.insert(equal, 2), Some(1));
    assert!(map.insert(changed, 3).is_none());
    assert_eq!(
        map.len(),
        2,
        "colliding owner buckets retain complete structural configuration equality"
    );
}
