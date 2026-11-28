use starlark::environment::Module;
use starlark::eval::ExecutionBudget;
use starlark::values::BoundedText;
use starlark::values::list::AllocList;

use super::*;

#[test]
fn collectors_use_bazel_label_str_and_repr() {
    let long = format!("@@{}//pkg:é雪", "long".repeat(4096));
    for (raw, repr_label) in [
        ("@@//:root", "//:root"),
        ("@@//pkg:é雪", "//pkg:é雪"),
        ("@@dep+//:target", "@@dep+//:target"),
        (long.as_str(), long.as_str()),
    ] {
        let label = StarlarkLabel::new(CanonicalLabel::parse(raw).unwrap());
        let mut plain = "existing: ".to_owned();
        let mut representation = "existing: ".to_owned();
        label.collect_str(&mut plain);
        label.collect_repr(&mut representation);
        drop(label);
        assert_eq!(plain, format!("existing: {raw}"));
        assert_eq!(representation, format!("existing: Label(\"{repr_label}\")"));
    }
}

#[test]
fn bounded_label_repr_uses_bazel_spelling_and_exact_size() {
    let module = Module::new();
    let heap = module.heap();
    for (raw, repr) in [
        ("@@//x:x", "Label(\"//x:x\")"),
        ("@@hello//x:x", "Label(\"@@hello//x:x\")"),
    ] {
        let label = heap.alloc_simple(StarlarkLabel::new(CanonicalLabel::parse(raw).unwrap()));
        for (value, mode, expected) in [
            (label, ValueTextMode::Str, raw.to_owned()),
            (label, ValueTextMode::Display, raw.to_owned()),
            (label, ValueTextMode::Repr, repr.to_owned()),
            (
                heap.alloc(AllocList([label])),
                ValueTextMode::Repr,
                format!("[{repr}]"),
            ),
        ] {
            let budget = ExecutionBudget::new(1024, 1024, 8);
            let mut exact = BoundedText::new(&budget, expected.len());
            exact.write_value(value, mode).unwrap();
            assert_eq!(exact.finish().unwrap(), expected);

            let budget = ExecutionBudget::new(1024, 1024, 8);
            let mut short = BoundedText::new(&budget, expected.len() - 1);
            assert_eq!(
                short.write_value(value, mode),
                Err(ExecutionLimit::TextBytes)
            );
            assert_eq!(short.finish(), Err(ExecutionLimit::TextBytes));
        }
    }
}

#[test]
fn bounded_label_text_preserves_repr_and_container_display() {
    let module = Module::new();
    let heap = module.heap();
    let raw = "@@dep+//pkg:é雪";
    let label = heap.alloc_simple(StarlarkLabel::new(CanonicalLabel::parse(raw).unwrap()));
    let list = heap.alloc(AllocList([label]));
    for (value, mode, expected) in [
        (label, ValueTextMode::Str, raw.to_owned()),
        (label, ValueTextMode::Repr, format!("Label(\"{raw}\")")),
        (list, ValueTextMode::Repr, format!("[Label(\"{raw}\")]")),
        (
            heap.alloc((list,)),
            ValueTextMode::Repr,
            format!("([{raw}],)"),
        ),
    ] {
        let budget = ExecutionBudget::new(1024, 1024, 8);
        let mut text = BoundedText::new(&budget, expected.len());
        text.write_value(value, mode).unwrap();
        assert_eq!(text.finish().unwrap(), expected);
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
