//! A source edit must remain visible independently of argv and native aliases.
use slug_build_api_v2::*;

use super::*;

#[derive(Default)]
struct Check {
    left_native: Vec<AnalysisValue>,
    right_native: Vec<AnalysisValue>,
    left_actions: Vec<ActionSpec>,
    right_actions: Vec<ActionSpec>,
    changed_authorities: usize,
    expected_before: String,
    expected_after: String,
}

pub(super) fn assert_cc_source_only_change(
    left: &ConfiguredNodeResult,
    right: &ConfiguredNodeResult,
    before: &str,
    after: &str,
) {
    macro_rules! same {
        ($($field:ident),+ $(,)?) => { $(
            assert!(left.$field() == right.$field(), stringify!($field));
        )+ };
    }
    same!(
        key,
        kind,
        actual_target,
        declared_outputs,
        edges,
        diagnostics,
        rule_capability,
        exec_groups,
        platform_semantic_fact,
        constraint_setting_default,
        runfiles_packages
    );
    let mut check = Check {
        expected_before: slug_bzlmod_v2::digest_module_file_content(before.as_bytes()),
        expected_after: slug_bzlmod_v2::digest_module_file_content(after.as_bytes()),
        ..Check::default()
    };
    assert_eq!(left.actions().len(), right.actions().len());
    for (left, right) in left.actions().iter().zip(right.actions()) {
        assert!(left.context() == right.context(), "action context");
        let left = left.spawn_spec().expect("fixture spawn");
        let right = right.spawn_spec().expect("fixture spawn");
        macro_rules! same_spawn {
            ($($field:ident),+ $(,)?) => { $(
                assert!(left.$field() == right.$field(), stringify!($field));
            )+ };
        }
        same_spawn!(
            invocation,
            outputs,
            unused_inputs_list,
            environment,
            execution_requirements,
            mnemonic,
            progress_message,
            resource_set
        );
        assert_eq!(left.render_argv().unwrap(), right.render_argv().unwrap());
        assert_eq!(
            left.expand_forced_param_files().unwrap(),
            right.expand_forced_param_files().unwrap()
        );
        check.left_actions.push(callback_free_action(left));
        check.right_actions.push(callback_free_action(right));
        let l = left.command_line().segments();
        let r = right.command_line().segments();
        assert_eq!(l.len(), r.len());
        for (l, r) in l.iter().zip(r) {
            match (l, r) {
                (
                    RetainedCommandLineSegment::LiteralRun(l),
                    RetainedCommandLineSegment::LiteralRun(r),
                ) => assert_eq!(l, r),
                (
                    RetainedCommandLineSegment::ArgsSnapshot(l),
                    RetainedCommandLineSegment::ArgsSnapshot(r),
                ) => {
                    assert_eq!(l.param_file(), r.param_file());
                    assert_eq!(l.recipe().calls().len(), r.recipe().calls().len());
                    for (l, r) in l.recipe().calls().iter().zip(r.recipe().calls()) {
                        match (l, r) {
                            (RetainedArgCall::Scalar(l), RetainedArgCall::Scalar(r)) => {
                                assert_eq!(l, r)
                            }
                            (RetainedArgCall::AddAll(l), RetainedArgCall::AddAll(r))
                            | (RetainedArgCall::AddJoined(l), RetainedArgCall::AddJoined(r)) => {
                                if matches!(l.source(), RetainedVectorSource::Mapped { .. }) {
                                    check.vector(l.source(), r.source());
                                } else {
                                    // The complete ordinary call is retained in the
                                    // callback-free action below, with the shared
                                    // providers/input/capture publication state.
                                    assert!(!matches!(
                                        r.source(),
                                        RetainedVectorSource::Mapped { .. }
                                    ));
                                }
                            }
                            _ => panic!("Args call kind changed"),
                        }
                    }
                }
                _ => panic!("command segment kind changed"),
            }
        }
    }
    assert!(
        check.changed_authorities > 0,
        "edited source must occur in actual callback authority"
    );
    // Clone only native ownership handles. One publication state compares ALL
    // original providers, native capture atoms, and mapped inputs jointly.
    // No graph/source is normalized or reconstructed for this comparison.
    fn native_roots(
        providers: &ProviderCollection,
        native: Vec<AnalysisValue>,
    ) -> ProviderCollection {
        let mut values = providers
            .iter()
            .map(|(_, value)| value.clone())
            .collect::<Vec<_>>();
        values.push(ProviderValue::Occurrence(ProviderOccurrence::new(
            ProviderIdentity::builtin("CallableSourceCheckRoots"),
            [("roots", AnalysisValue::tuple(native))],
        )));
        ProviderCollection::from_values(values, false).unwrap()
    }
    let l = native_roots(left.providers(), check.left_native);
    let r = native_roots(right.providers(), check.right_native);
    assert!(
        publication_eq_providers_and_actions(
            &l,
            &r,
            check.left_actions.iter().zip(check.right_actions.iter()),
        ),
        "providers / native input-capture alias partition changed"
    );
    eprintln!(
        "Cc source proof: {} callback authority occurrences changed only cc_info.bzl SHA and closure fingerprint; providers/native aliases, code/edges, other result fields, argv and forced-param bytes equal",
        check.changed_authorities
    );
}

// Only callback-bearing calls are excluded from this native alias audit;
// their inputs/atoms are compared above and their descriptor is checked separately.
// Ordinary vector calls retain their original options and depset owners. The
// diagnostic recipe format is fixed on both sides; actual output formats are
// exercised separately by the complete forced-param byte comparison.
fn callback_free_action(spawn: &SpawnSpec) -> ActionSpec {
    let segments = spawn
        .command_line()
        .segments()
        .iter()
        .map(|segment| match segment {
            RetainedCommandLineSegment::LiteralRun(values) => {
                RetainedCommandLineSegment::LiteralRun(values.clone())
            }
            RetainedCommandLineSegment::ArgsSnapshot(snapshot) => {
                let calls = snapshot
                    .recipe()
                    .calls()
                    .iter()
                    .filter(|call| match call {
                        RetainedArgCall::Scalar(_) => true,
                        RetainedArgCall::AddAll(vector) | RetainedArgCall::AddJoined(vector) => {
                            !matches!(vector.source(), RetainedVectorSource::Mapped { .. })
                        }
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                RetainedCommandLineSegment::ArgsSnapshot(RetainedSpawnArgsSnapshot::new(
                    RetainedArgsRecipe::new(calls, RetainedParamFileFormat::Multiline),
                    snapshot.param_file().cloned(),
                ))
            }
        })
        .collect::<Vec<_>>();
    let mut result = SpawnSpec::new(
        spawn.invocation().clone(),
        RetainedCommandLine::new(segments),
        spawn.inputs().clone(),
        spawn.tools().clone(),
        spawn.outputs().to_vec(),
        spawn.unused_inputs_list().cloned(),
        spawn.environment().clone(),
        spawn.execution_requirements().clone(),
        spawn.mnemonic(),
        spawn.progress_message(),
    );
    if let Some(resource) = spawn.resource_set() {
        result = result.with_resource_set(resource);
    }
    ActionSpec::spawn(result)
}

impl Check {
    fn vector(&mut self, left: &RetainedVectorSource, right: &RetainedVectorSource) {
        match (left, right) {
            (
                RetainedVectorSource::Mapped {
                    inputs: l,
                    mapper: lm,
                    registration_location: ll,
                },
                RetainedVectorSource::Mapped {
                    inputs: r,
                    mapper: rm,
                    registration_location: rl,
                },
            ) => {
                assert_eq!(ll, rl);
                match (l, r) {
                    (RetainedArgsInputs::Sequence(l), RetainedArgsInputs::Sequence(r)) => {
                        assert_eq!(l.len(), r.len());
                        self.left_native.extend(l.iter().cloned());
                        self.right_native.extend(r.iter().cloned());
                    }
                    (RetainedArgsInputs::Depset(l), RetainedArgsInputs::Depset(r)) => {
                        self.left_native.push(AnalysisValue::depset(l.clone()));
                        self.right_native.push(AnalysisValue::depset(r.clone()));
                    }
                    _ => panic!("mapped input kind changed"),
                }
                assert_eq!(lm.root(), rm.root());
                self.graph(lm.graph(), rm.graph());
            }
            (RetainedVectorSource::Sequence(l), RetainedVectorSource::Sequence(r)) => {
                assert_eq!(l, r)
            }
            (RetainedVectorSource::Depset(_), RetainedVectorSource::Depset(_)) => {
                unreachable!("ordinary calls are compared in the joint native publication path")
            }
            _ => panic!("vector source kind changed"),
        }
    }

    fn graph(&mut self, left: &CallableGraph, right: &CallableGraph) {
        for index in 0..callable_graph::CALLABLE_GRAPH_NODE_LIMIT {
            let id = CallableNodeId::new(index).unwrap();
            let (l, r) = (left.node(id), right.node(id));
            let (Ok(l), Ok(r)) = (l, r) else {
                assert_eq!(
                    left.node(id).is_ok(),
                    right.node(id).is_ok(),
                    "graph node count"
                );
                return;
            };
            match (l, r) {
                (CallableNode::Native(l), CallableNode::Native(r)) => {
                    self.left_native.push(l.clone());
                    self.right_native.push(r.clone());
                }
                (CallableNode::Function(l), CallableNode::Function(r)) => {
                    self.code(&l.code, &r.code, left, right);
                    parameters(&l.parameters, &r.parameters);
                    assert_eq!(l.return_annotation, r.return_annotation);
                    assert_eq!(l.capture_names, r.capture_names);
                    assert_eq!(
                        left.edges(l.captures).unwrap(),
                        right.edges(r.captures).unwrap()
                    );
                }
                (CallableNode::Code(l), CallableNode::Code(r)) => self.code(l, r, left, right),
                (CallableNode::Intrinsic(l), CallableNode::Intrinsic(r)) => {
                    assert_eq!(l.api, r.api);
                    parameters(&l.parameters, &r.parameters);
                    assert_eq!(l.return_annotation, r.return_annotation);
                    assert_eq!(l.receiver, r.receiver);
                }
                (CallableNode::Cell { contents: l }, CallableNode::Cell { contents: r }) => {
                    assert_eq!(l, r)
                }
                (CallableNode::List(l), CallableNode::List(r))
                | (CallableNode::Tuple(l), CallableNode::Tuple(r))
                | (CallableNode::Dict(l), CallableNode::Dict(r)) => {
                    assert_eq!(left.edges(*l).unwrap(), right.edges(*r).unwrap())
                }
                (
                    CallableNode::Struct {
                        names: ln,
                        values: l,
                    },
                    CallableNode::Struct {
                        names: rn,
                        values: r,
                    },
                ) => {
                    assert_eq!(ln, rn);
                    assert_eq!(left.edges(*l).unwrap(), right.edges(*r).unwrap());
                }
                (
                    CallableNode::Provider {
                        identity: li,
                        names: ln,
                        values: l,
                        has_schema: ls,
                    },
                    CallableNode::Provider {
                        identity: ri,
                        names: rn,
                        values: r,
                        has_schema: rs,
                    },
                ) => {
                    assert_eq!(li, ri);
                    assert_eq!(ln, rn);
                    assert_eq!(ls, rs);
                    assert_eq!(left.edges(*l).unwrap(), right.edges(*r).unwrap());
                }
                _ => panic!("graph node kind changed at {index}"),
            }
        }
    }

    fn code(&mut self, l: &CallableCode, r: &CallableCode, lg: &CallableGraph, rg: &CallableGraph) {
        assert_eq!(l.name, r.name);
        assert_eq!(l.definition, r.definition);
        assert_eq!(
            lg.edges(l.dependencies).unwrap(),
            rg.edges(r.dependencies).unwrap()
        );
        assert_eq!(l.globals.len(), r.globals.len());
        for (l, r) in l.globals.iter().zip(r.globals.iter()) {
            assert_eq!(l.name, r.name);
            assert_eq!(l.access, r.access);
            assert_eq!(l.value, r.value);
        }
        let (l, r) = (l.source.data(), r.source.data());
        assert_eq!(l.root, r.root);
        assert_eq!(l.direct_children, r.direct_children);
        assert_eq!(l.reachable.len(), r.reachable.len());
        let mut changes = 0;
        for (l, r) in l.reachable.iter().zip(r.reachable.iter()) {
            assert_eq!(l.identity, r.identity);
            if l.digest != r.digest {
                assert!(
                    l.identity.label.to_string() == "@@rules_cc+//cc/private:cc_info.bzl",
                    "unexpected changed source: {}",
                    l.identity.workspace_path.display()
                );
                let hex = |digest: &[u8]| {
                    digest
                        .iter()
                        .map(|byte| format!("{byte:02x}"))
                        .collect::<String>()
                };
                assert_eq!(hex(&l.digest), self.expected_before);
                assert_eq!(hex(&r.digest), self.expected_after);
                changes += 1;
            }
        }
        if changes == 0 {
            assert_eq!(l.fingerprint, r.fingerprint);
        } else {
            assert_eq!(changes, 1);
            assert_ne!(l.fingerprint, r.fingerprint);
            self.changed_authorities += 1;
        }
    }
}

fn parameters(left: &[CallableParameter], right: &[CallableParameter]) {
    assert_eq!(left.len(), right.len());
    for (l, r) in left.iter().zip(right) {
        assert_eq!(l.name, r.name);
        assert_eq!(l.mode, r.mode);
        assert_eq!(l.requirement, r.requirement);
        assert_eq!(l.annotation, r.annotation);
    }
}
