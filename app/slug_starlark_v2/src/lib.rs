use std::sync::OnceLock;

use starlark::environment::Globals;
use starlark::environment::GlobalsBuilder;
use starlark::environment::GlobalsStatic;
use starlark::environment::LibraryExtension;

const BAZEL_STANDARD_NAMES: &[&str] = &[
    "False",
    "True",
    "None",
    "min",
    "max",
    "abs",
    "all",
    "any",
    "sorted",
    "reversed",
    "tuple",
    "list",
    "len",
    "str",
    "repr",
    "bool",
    "float",
    "int",
    "dict",
    "enumerate",
    "hash",
    "range",
    "hasattr",
    "getattr",
    "dir",
    "fail",
    "type",
    "zip",
];

/// Populate Bazel's process-stable universal globals before a context overlay.
pub fn populate_universe(out: &mut GlobalsBuilder) {
    static STANDARD: OnceLock<Globals> = OnceLock::new();
    static UNIVERSE: GlobalsStatic = GlobalsStatic::new();
    UNIVERSE.populate(
        |universe| {
            let standard = STANDARD.get_or_init(Globals::standard);
            for (name, value) in standard.iter() {
                if BAZEL_STANDARD_NAMES.contains(&name) {
                    universe.set(name, value);
                }
            }
            LibraryExtension::Print.add(universe);
            LibraryExtension::SetType.add(universe);
        },
        out,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bazel_universe_is_exact_and_process_stable() {
        let build = |builder: &mut GlobalsBuilder| populate_universe(builder);
        let first = GlobalsBuilder::new().with(build).build();
        let second = GlobalsBuilder::new().with(build).build();
        let mut names = first.names().map(|name| name.as_str()).collect::<Vec<_>>();
        names.sort_unstable();
        assert_eq!(
            names,
            [
                "False",
                "None",
                "True",
                "abs",
                "all",
                "any",
                "bool",
                "dict",
                "dir",
                "enumerate",
                "fail",
                "float",
                "getattr",
                "hasattr",
                "hash",
                "int",
                "len",
                "list",
                "max",
                "min",
                "print",
                "range",
                "repr",
                "reversed",
                "set",
                "sorted",
                "str",
                "tuple",
                "type",
                "zip",
            ]
        );
        assert!(first.iter().all(|(name, value)| {
            second
                .iter()
                .find(|(other, _)| *other == name)
                .unwrap()
                .1
                .to_value()
                .ptr_eq(value.to_value())
        }));
    }
}
