use std::collections::HashMap;

use super::*;

#[test]
fn container_identity_keys_retain_temporary_owners_and_distinguish_equal_storage() {
    let native = AnalysisValue::list(vec![AnalysisValue::string("same")]);
    let weak = Arc::downgrade(&native.0);
    let key = native.container_identity().unwrap();
    assert_eq!(key, native.dupe().container_identity().unwrap());
    let equal = AnalysisValue::list(vec![AnalysisValue::string("same")]);
    assert_eq!(native, equal);
    assert_ne!(key, equal.container_identity().unwrap());
    assert!(AnalysisValue::none().container_identity().is_none());
    let mut memo = HashMap::new();
    memo.insert(key, 7);
    drop(native);
    assert!(
        weak.upgrade().is_some(),
        "scratch key must retain the Arc owner"
    );
    assert_eq!(memo.get(&equal.container_identity().unwrap()), None);
    drop(memo);
    assert!(
        weak.upgrade().is_none(),
        "phase-local memo must release its owner"
    );
}
