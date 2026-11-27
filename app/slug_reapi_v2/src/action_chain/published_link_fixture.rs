use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

/// Check the literal and resolve its direct target without following another
/// link. This preserves discrimination of alias chains and backing ownership.
pub fn read_contained_target(workspace: &Path, link: &Path) -> PathBuf {
    let mut resolved = link.parent().unwrap().to_owned();
    let target = std::fs::read_link(link).unwrap();
    assert!(
        target.is_relative(),
        "{} points to {target:?}",
        link.display()
    );
    for component in target.components() {
        match component {
            Component::Normal(name) => resolved.push(name),
            Component::ParentDir => assert!(resolved.pop()),
            Component::CurDir => {}
            _ => unreachable!(),
        }
        assert!(
            resolved.starts_with(workspace),
            "{} escapes",
            link.display()
        );
    }
    resolved
}
