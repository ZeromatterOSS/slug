use std::collections::hash_map::DefaultHasher;
use std::hash::Hash;
use std::hash::Hasher;

use slug_query_v2::QueryOrder;
use slug_query_v2::QueryOutputCompletion;
use slug_query_v2::QueryPolicy;
use slug_query_v2::RootQueryCommandKey;
use slug_query_v2::RootQueryCommandObservationKey;
use slug_workspace_v2::NormalizedAbsolutePath;

fn hash(value: &impl Hash) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

#[test]
fn observed_root_query_has_distinct_structural_identity_and_exact_admission() {
    let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
    let legacy = RootQueryCommandKey::new(
        workspace.clone(),
        "deps(//pkg:target)",
        QueryOrder::Auto,
        QueryPolicy::default(),
        QueryOutputCompletion::Standard,
    )
    .unwrap();
    let observed = RootQueryCommandObservationKey::new(
        workspace.clone(),
        "deps(//pkg:target)",
        QueryOrder::Auto,
        QueryPolicy::default(),
        QueryOutputCompletion::Standard,
    )
    .unwrap();
    let same = RootQueryCommandObservationKey::new(
        workspace.clone(),
        "deps(//pkg:target)",
        QueryOrder::Auto,
        QueryPolicy::default(),
        QueryOutputCompletion::Standard,
    )
    .unwrap();

    assert_eq!(observed, same);
    assert_eq!(hash(&observed), hash(&same));
    assert_eq!(legacy.to_string(), "root-query-command:deps(//pkg:target)");
    assert_eq!(
        observed.to_string(),
        "observed-root-query-command:deps(//pkg:target)"
    );
    assert!(
        RootQueryCommandKey::new(
            workspace.clone(),
            "deps(",
            QueryOrder::Auto,
            QueryPolicy::default(),
            QueryOutputCompletion::Standard,
        )
        .is_err()
    );
    assert!(
        RootQueryCommandObservationKey::new(
            workspace,
            "deps(",
            QueryOrder::Auto,
            QueryPolicy::default(),
            QueryOutputCompletion::Standard,
        )
        .is_err()
    );
}
