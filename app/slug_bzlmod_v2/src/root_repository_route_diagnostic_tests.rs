use super::*;

#[test]
fn route_diagnostic_distinguishes_original_safe_and_opaque_stages() {
    let apparent_repo = ApparentRepoName::new("external_repo").unwrap();
    let cases = [
        (
            RootRepositoryRouteErrorKind::Unknown {
                apparent_repo: apparent_repo.clone(),
            },
            "Unknown @external_repo",
            true,
        ),
        (
            RootRepositoryRouteErrorKind::Unsupported {
                apparent_repo,
                module_name: CompactString::new("original_module"),
            },
            "Unsupported @external_repo module=original_module",
            true,
        ),
        (
            RootRepositoryRouteErrorKind::Root(HostRootModuleFileError::CommandPolicy {
                message: CompactString::new("OPAQUE_POISON"),
            }),
            "Root [diagnostic incomplete: Root]",
            false,
        ),
        (
            RootRepositoryRouteErrorKind::SelectedInfrastructure(Arc::from("ORIGINAL_CAUSE")),
            "SelectedInfrastructure: ORIGINAL_CAUSE",
            false,
        ),
    ];
    for (kind, expected, fallback) in cases {
        let owner = Arc::new(RootRepositoryRouteError { kind });
        let weak = Arc::downgrade(&owner);
        let count = Arc::strong_count(&owner);
        let mut text = String::new();
        owner.write_route_diagnostic(&mut text).unwrap();
        assert_eq!(text, expected);
        assert!(!text.contains("OPAQUE_POISON"));
        assert_eq!(owner.is_generated_route_fallback(), fallback);
        assert_eq!(Arc::strong_count(&owner), count);
        drop(owner);
        assert!(weak.upgrade().is_none());
    }
}

#[test]
fn route_diagnostic_borrows_message_and_propagates_writer_stop() {
    let message: Arc<str> = Arc::from("original \"cause\"\n\u{e9}\t");
    let weak = Arc::downgrade(&message);
    let owner = RootRepositoryRouteError {
        kind: RootRepositoryRouteErrorKind::SelectedInfrastructure(message),
    };
    struct BorrowWitness {
        pointer: *const u8,
        length: usize,
        seen: bool,
    }
    impl fmt::Write for BorrowWitness {
        fn write_str(&mut self, text: &str) -> fmt::Result {
            if text.len() == self.length && text.as_ptr() == self.pointer {
                self.seen = true;
            }
            Ok(())
        }
    }
    let mut witness = BorrowWitness {
        pointer: weak.as_ptr() as *const u8,
        length: "original \"cause\"\n\u{e9}\t".len(),
        seen: false,
    };
    owner.write_route_diagnostic(&mut witness).unwrap();
    assert!(witness.seen);
    struct Reject(usize);
    impl fmt::Write for Reject {
        fn write_str(&mut self, _: &str) -> fmt::Result {
            self.0 += 1;
            Err(fmt::Error)
        }
    }
    let mut reject = Reject(0);
    assert!(owner.write_route_diagnostic(&mut reject).is_err());
    assert_eq!(reject.0, 1);
    let mut text = String::new();
    owner.write_route_diagnostic(&mut text).unwrap();
    assert_eq!(text, "SelectedInfrastructure: original \"cause\"\n\u{e9}\t");
    // Escaping and byte limits belong to the receiving Loading sink. The
    // natural owner streams the original message without copying its storage.
    assert!(weak.upgrade().is_some());
    drop(owner);
    assert!(weak.upgrade().is_none());
}
