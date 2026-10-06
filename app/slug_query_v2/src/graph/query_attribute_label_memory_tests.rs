use std::collections::hash_map::DefaultHasher;

use super::*;

fn original(owner: &QueryLabel, label: &CanonicalLabel) -> CompactString {
    let same_owner_repo = label.package().repo() == owner.canonical.package().repo();
    QueryLabel {
        canonical: Arc::new(label.clone()),
        apparent_repo: same_owner_repo
            .then(|| owner.apparent_repo.clone())
            .flatten(),
    }
    .output_label()
}

fn hash(label: &QueryLabel) -> u64 {
    let mut state = DefaultHasher::new();
    label.hash(&mut state);
    state.finish()
}

fn query(canonical: &str, apparent: Option<&str>) -> QueryLabel {
    QueryLabel {
        canonical: Arc::new(CanonicalLabel::parse(canonical).unwrap()),
        apparent_repo: apparent.map(|name| Arc::new(ApparentRepoName::new(name).unwrap())),
    }
}

#[test]
fn attribute_label_rendering_preserves_spelling_capacity_and_source_identity() {
    let owners = [
        query("@@//:root", None),
        query("@@dep+//pkg:owner", None),
        query("@@dep+//pkg:owner", Some("alias")),
        query("@@dep+//pkg:owner", Some("other_alias")),
    ];
    let labels = [
        "@@//:root",
        "@@//p:é世界",
        "@@dep+//:empty_package",
        "@@dep+//nested/p:target",
        "@@other+//p:foreign",
    ];
    let mut count = 0;
    for owner in owners {
        let owner_clone = owner.clone();
        let before_hash = hash(&owner);
        let before_text = owner.to_string();
        let before_canonical = Arc::as_ptr(&owner.canonical);
        let before_apparent = owner.apparent_repo.as_ref().map(Arc::as_ptr);
        let displayed = owner.output_label();
        let expected_display = if let Some(repo) = &owner.apparent_repo {
            format!("@{}//{}:{}", repo.as_str(), owner.package(), owner.target())
        } else {
            before_text.clone()
        };
        assert_eq!(displayed, expected_display);
        assert_eq!(
            displayed.capacity(),
            CompactString::new(&expected_display).capacity()
        );
        for spelling in labels {
            let label = CanonicalLabel::parse(spelling).unwrap();
            let clone = label.clone();
            let output = owner.output_attribute_label(&label);
            let expected = original(&owner, &label);
            let same = label.package().repo() == owner.canonical.package().repo();
            let expected_text = if same && owner.apparent_repo.is_some() {
                format!(
                    "@{}//{}:{}",
                    owner.apparent_repo.as_ref().unwrap().as_str(),
                    label.package().package(),
                    label.target()
                )
            } else if label.package().repo().is_root() {
                spelling.strip_prefix("@@").unwrap().to_owned()
            } else {
                spelling.to_owned()
            };
            assert_eq!(output, expected_text);
            assert_eq!(output, expected);
            assert_eq!(output.capacity(), expected.capacity());
            assert_eq!(label, clone);
            count += 1;
        }
        assert_eq!(owner, owner_clone);
        assert_eq!(owner.cmp(&owner_clone), Ordering::Equal);
        assert_eq!(hash(&owner), before_hash);
        assert_eq!(owner.to_string(), before_text);
        assert_eq!(Arc::as_ptr(&owner.canonical), before_canonical);
        assert_eq!(
            owner.apparent_repo.as_ref().map(Arc::as_ptr),
            before_apparent
        );
    }
    assert_eq!(count, 20);
    println!("ATTRIBUTE_LABEL_CASES={count}");
}

#[test]
fn attribute_label_outputs_remain_owned_after_source_and_owner_drop() {
    let package = "package".repeat(512);
    let target = "é世界".repeat(1024);
    let canonical = format!("@@dep+//{package}:{target}");
    let apparent = "alias".repeat(1024);
    let owner = query("@@dep+//p:owner", Some(&apparent));
    let owner_clone = owner.clone();
    let same = CanonicalLabel::parse(&canonical).unwrap();
    let foreign = CanonicalLabel::parse(&format!("@@foreign+//{package}:{target}")).unwrap();
    let expected_same = format!("@{apparent}//{package}:{target}");
    let expected_foreign = foreign.to_string();
    let expected_owner = format!("@{apparent}//p:owner");
    let outputs = [
        owner.output_attribute_label(&same),
        owner.output_attribute_label(&foreign),
        owner.output_label(),
    ];
    for (output, expected) in
        outputs
            .iter()
            .zip([&expected_same, &expected_foreign, &expected_owner])
    {
        assert_eq!(output, expected);
        assert_eq!(output.capacity(), CompactString::new(expected).capacity());
    }
    drop(same);
    drop(foreign);
    drop(owner);
    drop(owner_clone);
    drop(canonical);
    drop(apparent);
    drop(package);
    drop(target);
    assert_eq!(outputs[0], expected_same);
    assert_eq!(outputs[1], expected_foreign);
    assert_eq!(outputs[2], expected_owner);
    println!(
        "ATTRIBUTE_LABEL_RETAINED={:?}",
        outputs.iter().map(|s| s.len()).collect::<Vec<_>>()
    );
}
