use starlark::environment::Module;

use super::*;

#[test]
fn collectors_preserve_prefixed_root_external_unicode_and_long_text() {
    for raw in [
        "@@//:root".to_owned(),
        "@@//pkg:é雪".to_owned(),
        "@@dep+//:target".to_owned(),
        format!("@@{}//pkg:é雪", "long".repeat(4096)),
    ] {
        let label = StarlarkLabel::new(CanonicalLabel::parse(&raw).unwrap());
        let mut plain = "existing: ".to_owned();
        let mut representation = "existing: ".to_owned();
        label.collect_str(&mut plain);
        label.collect_repr(&mut representation);
        drop(label);
        assert_eq!(plain, format!("existing: {raw}"));
        assert_eq!(representation, format!("existing: Label(\"{raw}\")"));
    }
}

#[test]
fn workspace_root_strings_survive_label_drop_and_module_freeze() {
    for repository in ["".to_owned(), "dep+".to_owned(), "long".repeat(4096)] {
        let module = Module::new();
        let label =
            StarlarkLabel::new(CanonicalLabel::parse(&format!("@@{repository}//pkg:é雪")).unwrap());
        let value = label.get_attr("workspace_root", module.heap()).unwrap();
        let expected = if repository.is_empty() {
            String::new()
        } else {
            format!("external/{repository}")
        };
        drop(label);
        drop(repository);
        assert_eq!(value.unpack_str(), Some(expected.as_str()));
        module.set("ROOT", value);
        let frozen = module.freeze().unwrap();
        let retained = frozen.get("ROOT").unwrap();
        drop(frozen);
        assert_eq!(retained.unpack_str(), Some(expected.as_str()));
    }
}
