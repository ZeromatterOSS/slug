//! Native phase-local rows reuse the same registry without incomplete ActionSpecs.
use slug_build_api_v2::ActionError;
use slug_build_api_v2::ActionKind;
use slug_build_api_v2::ActionOutput;
use slug_build_api_v2::ActionRegistry;
use slug_build_api_v2::ActionSpec;
use slug_build_api_v2::AnalysisArtifact;
use slug_build_api_v2::CtxActions;
use slug_build_api_v2::DerivedArtifactRoot;
use slug_build_api_v2::RegisterableAction;
use slug_build_api_v2::SymlinkSpec;
use slug_build_api_v2::SymlinkTarget;
use slug_identity_v2::CanonicalLabel;

mod root_support;

// No PartialEq: pending rows are not semantic published values.
#[derive(Debug, Clone)]
enum PendingRow {
    Ready(ActionSpec),
    Write {
        outputs: Vec<ActionOutput>,
        root_index: usize,
    },
}

impl From<ActionSpec> for PendingRow {
    fn from(action: ActionSpec) -> Self {
        Self::Ready(action)
    }
}

impl RegisterableAction for PendingRow {
    fn outputs(&self) -> &[ActionOutput] {
        match self {
            Self::Ready(action) => action.outputs(),
            Self::Write { outputs, .. } => outputs,
        }
    }
    fn mnemonic(&self) -> &str {
        match self {
            Self::Ready(action) => action.mnemonic(),
            Self::Write { .. } => "FileWrite",
        }
    }
    fn validate(&self) -> Result<(), ActionError> {
        match self {
            Self::Ready(action) => action.validate(),
            Self::Write { outputs, .. } if outputs.is_empty() => Err(ActionError::MissingOutputs {
                mnemonic: self.mnemonic().to_owned(),
            }),
            Self::Write { .. } => Ok(()),
        }
    }
    fn as_action_spec(&self) -> Option<&ActionSpec> {
        match self {
            Self::Ready(action) => Some(action),
            Self::Write { .. } => None,
        }
    }
}

fn complete(row: &PendingRow) -> Result<ActionSpec, ActionError> {
    Ok(match row {
        PendingRow::Ready(action) => action.clone(),
        PendingRow::Write {
            outputs,
            root_index,
        } => ActionSpec::new(
            ActionKind::Write {
                content: format!("root-{root_index}"),
                is_executable: false,
            },
            "FileWrite",
            outputs.clone(),
        ),
    })
}

#[test]
fn pending_ready_completion_preserves_output_owners_ordinals_and_symlink_dedup() {
    let mut actions = CtxActions::<PendingRow>::new_typed(root_support::root());
    let ready = actions.declare_file("pkg/ready").unwrap();
    assert_eq!(actions.write(ready, "ready", false).unwrap(), 0);
    let pending = actions.declare_file("pkg/pending").unwrap();
    assert_eq!(
        actions
            .register_pending(PendingRow::Write {
                outputs: vec![pending.clone()],
                root_index: 7
            })
            .unwrap(),
        1
    );
    assert!(actions.registry().actions()[0].as_action_spec().is_some());
    assert!(actions.registry().actions()[1].as_action_spec().is_none());
    assert_eq!(
        actions.registry().structural_output_owner(&pending),
        Some(1)
    );
    assert!(matches!(
        actions.register_pending(PendingRow::Write {
            outputs: vec![pending.clone()],
            root_index: 8
        }),
        Err(ActionError::ConflictingOutput { .. })
    ));
    let link = actions.declare_file("pkg/link").unwrap();
    let target = SymlinkTarget::Artifact {
        input: AnalysisArtifact::Source(CanonicalLabel::parse("@@//pkg:source").unwrap()),
        require_executable: false,
        use_exec_root_for_source: false,
    };
    assert_eq!(
        actions
            .register_symlink(SymlinkSpec::new(
                link.clone(),
                target.clone(),
                Some("first")
            ))
            .unwrap(),
        2
    );
    assert_eq!(
        actions
            .register_symlink(SymlinkSpec::new(
                link,
                target,
                Some("ignored duplicate progress")
            ))
            .unwrap(),
        2
    );
    let completed: CtxActions = actions.try_map(complete).unwrap();
    assert_eq!(completed.registry().actions().len(), 3);
    assert_eq!(completed.registry().output_owner(pending.path()), Some(1));
    assert_eq!(
        completed.registry().structural_output_owner(&pending),
        Some(1)
    );
    assert!(
        matches!(completed.registry().actions()[1].kind(), ActionKind::Write { content, .. } if content == "root-7")
    );
    assert_eq!(
        completed.registry().actions()[2]
            .symlink_spec()
            .unwrap()
            .progress_message(),
        Some("first")
    );
}

#[test]
fn pending_batch_and_replacement_validate_atomically_in_the_existing_owner() {
    let mut actions = CtxActions::<PendingRow>::new_typed(root_support::root());
    let output = actions.declare_file("pkg/original").unwrap();
    actions.write(output.clone(), "original", false).unwrap();
    let first = actions.declare_file("pkg/new").unwrap();
    let rows = [
        PendingRow::Write {
            outputs: vec![first.clone()],
            root_index: 1,
        },
        PendingRow::Write {
            outputs: vec![first],
            root_index: 2,
        },
    ];
    assert!(actions.replace_actions(rows.to_vec()).is_err());
    assert_eq!(actions.registry().actions().len(), 1);
    assert_eq!(actions.registry().structural_output_owner(&output), Some(0));
    let mut registry = ActionRegistry::<PendingRow>::new_typed();
    assert!(registry.register_batch(rows).is_err());
    assert!(registry.actions().is_empty());
    assert!(
        registry
            .register(PendingRow::Write {
                outputs: vec![],
                root_index: 0
            })
            .is_err()
    );
    assert!(registry.actions().is_empty());
}

#[test]
fn pending_completion_rejects_failure_invalid_payload_and_changed_headers() {
    let build = || {
        let mut actions = CtxActions::<PendingRow>::new_typed(root_support::root());
        let first = actions.declare_file("pkg/first").unwrap();
        actions.write(first, "first", false).unwrap();
        let second = actions.declare_file("pkg/second").unwrap();
        actions
            .register_pending(PendingRow::Write {
                outputs: vec![second],
                root_index: 2,
            })
            .unwrap();
        actions
    };
    let mut visits = 0;
    let result = build().try_map(|row| {
        visits += 1;
        if visits == 2 {
            Err(ActionError::MissingArgv {
                mnemonic: "deliberate completion failure".to_owned(),
            })
        } else {
            complete(row)
        }
    });
    assert!(matches!(result, Err(ActionError::MissingArgv { .. })));
    assert_eq!(visits, 2);
    let result = build().try_map(|row| {
        Ok::<_, ActionError>(ActionSpec::new(
            ActionKind::CppCompile,
            row.mnemonic(),
            row.outputs().to_vec(),
        ))
    });
    assert!(matches!(
        result,
        Err(ActionError::InvalidActionPayload { .. })
    ));
    let result = build().try_map(|row| {
        Ok::<_, ActionError>(ActionSpec::new(
            ActionKind::Write {
                content: "complete".to_owned(),
                is_executable: false,
            },
            "ChangedMnemonic",
            row.outputs().to_vec(),
        ))
    });
    assert!(matches!(
        result,
        Err(ActionError::ChangedRegistration { index: 0 })
    ));
    let other = CtxActions::new(root_support::root())
        .declare_file("pkg/changed-output")
        .unwrap();
    let result = build().try_map(|row| {
        Ok::<_, ActionError>(ActionSpec::new(
            ActionKind::Write {
                content: "complete".to_owned(),
                is_executable: false,
            },
            row.mnemonic(),
            vec![other.clone()],
        ))
    });
    assert!(matches!(
        result,
        Err(ActionError::ChangedRegistration { index: 0 })
    ));
}
