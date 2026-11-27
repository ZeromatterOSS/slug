//! Result and invalidation proof for the pinned Rust toolchain owner fixture.

use std::fs;

use slug_analysis_v2::ConfiguredRunfilesSupportActionView;
use slug_analysis_v2::ConfiguredSpawnActionView;
use slug_build_api_v2::AnalysisValueKind;
use slug_build_api_v2::ProviderId;
use slug_build_api_v2::ProviderIdentity;
use slug_build_api_v2::ProviderOccurrence;

use super::*;

const PROVIDERS: &str = "@@rules_rust+//rust/private:providers.bzl";

fn provider_field<'a>(provider: &'a ProviderOccurrence, name: &str) -> AnalysisValueKind<'a> {
    provider
        .field(name)
        .unwrap_or_else(|| panic!("missing pinned provider field {name}"))
        .kind()
}

fn check_crate_info(analysis: &ConfiguredNodeResult, edition: &str) {
    let crate_id = ProviderId::new(PROVIDERS, "CrateInfo").unwrap();
    let crate_info = analysis
        .providers()
        .user(&crate_id)
        .expect("the pinned rules_rust CrateInfo");
    let AnalysisValueKind::Label(owner) = provider_field(crate_info, "owner") else {
        panic!("CrateInfo owner must be a label");
    };
    assert!(owner.bazel_natural_cmp(analysis.key().label()).is_eq());
    for (name, expected) in [("name", "probe"), ("type", "bin"), ("edition", edition)] {
        assert!(
            matches!(provider_field(crate_info, name), AnalysisValueKind::String(actual) if actual == expected),
            "CrateInfo {name} differs from the pinned fixture"
        );
    }
    for (name, expected) in [("root", "pkg/main.rs"), ("output", "pkg/probe")] {
        assert!(
            matches!(provider_field(crate_info, name), AnalysisValueKind::Artifact(actual) if actual.short_path() == expected),
            "CrateInfo {name} differs from the pinned fixture"
        );
    }

    let AnalysisValueKind::Depset(deps) = provider_field(crate_info, "deps") else {
        panic!("CrateInfo deps must be a depset");
    };
    let dep_variant = ProviderIdentity::user(ProviderId::new(PROVIDERS, "DepVariantInfo").unwrap());
    let crate_identity = ProviderIdentity::user(crate_id);
    let mut owners = Vec::new();
    for dep in deps.to_list() {
        let AnalysisValueKind::Provider(dep) = dep.kind() else {
            panic!("CrateInfo dependency must be a provider");
        };
        assert!(dep.identity() == &dep_variant);
        let dependency = match provider_field(dep, "crate_info") {
            AnalysisValueKind::None => continue,
            AnalysisValueKind::Provider(dependency) => dependency,
            _ => panic!("DepVariantInfo crate_info must be a provider or None"),
        };
        assert!(dependency.identity() == &crate_identity);
        let AnalysisValueKind::Label(owner) = provider_field(dependency, "owner") else {
            panic!("dependency CrateInfo owner must be a label");
        };
        owners.push(owner.to_string());
    }
    owners.sort_unstable();
    assert!(owners == ["@@//pkg:support"], "CrateInfo dependency owners");

    let AnalysisValueKind::Depset(data) = provider_field(crate_info, "data") else {
        panic!("CrateInfo data must be a depset");
    };
    let data = data.to_list();
    assert!(
        matches!(data.as_slice(), [file] if matches!(file.kind(), AnalysisValueKind::Artifact(file) if file.short_path() == "pkg/data.txt")),
        "CrateInfo data short paths"
    );
}

fn check_action_outputs([rustc, symlink_tree, runfiles_tree]: [&ConfiguredAction; 3]) {
    let rustc = ConfiguredSpawnActionView::from_configured_action(rustc).unwrap();
    assert!(
        rustc
            .spec()
            .outputs()
            .iter()
            .map(|output| output.root_relative_path())
            .eq(["pkg/probe"]),
        "Rustc output"
    );
    for (action, expected) in [
        (symlink_tree, "pkg/probe.runfiles/MANIFEST"),
        (runfiles_tree, "pkg/probe.runfiles"),
    ] {
        let action = ConfiguredRunfilesSupportActionView::from_configured_action(action).unwrap();
        assert!(
            action.output().root_relative_path() == expected,
            "runfiles output"
        );
    }
}

#[test]
#[ignore = "four actual sequential Rust cqueries, each under native policy; staged sources and finite supervisor required"]
fn native_original_rust_toolchain_action_edition_comparison() {
    let config_path = std::env::var_os("SLUG_NATIVE_RUST_INPUTS").expect("staged Rust inputs");
    let config = read_config(Path::new(&config_path)).unwrap();
    let staged = Path::new(config["workspace"].as_str().unwrap());
    verify_inputs(&config, staged);

    // Mutate only the six-file test copy. Registry and distdir stay pinned.
    let owned = tempfile::tempdir().unwrap();
    let workspace = owned.path();
    for relative in OWNER_FILES {
        let destination = workspace.join(relative);
        fs::create_dir_all(destination.parent().unwrap()).unwrap();
        fs::copy(staged.join(relative), destination).unwrap();
    }
    verify_inputs(&config, workspace);
    let module = workspace.join("MODULE.bazel");
    let edition_a = fs::read_to_string(&module).unwrap();
    let token = "edition = \"2024\"";
    assert_eq!(edition_a.matches(token).count(), 1);
    let edition_b = edition_a.replacen(token, "edition = \"2021\"", 1);

    let resources = bank();
    let baseline = resources.snapshot().retained_bytes;
    let mut runtime =
        WorkspaceRuntime::new(workspace, crate::runtime::ProcessHostOwner::native()).unwrap();
    #[cfg(feature = "native-probe-observer")]
    let profile = phase_profile::InstalledProfile::attach(&mut runtime).unwrap();
    runtime
        .enable_general_repository_workspaces_for_test(resources.clone(), limits())
        .unwrap();
    let (root, inputs) = rust_cquery_request(&runtime, &config);
    let mut initial: Option<[ConfiguredAction; 3]> = None;
    for (case, edition, replacement) in [
        ("initial", "2024", None),
        ("warm", "2024", None),
        ("changed", "2021", Some(edition_b.as_str())),
        ("restored", "2024", Some(edition_a.as_str())),
    ] {
        if let Some(replacement) = replacement {
            fs::write(&module, replacement).unwrap();
        }
        let trace = Arc::new(Mutex::new(Trace::default()));
        let before = resources.snapshot();
        let before_claims = runtime
            .repository_materializer
            .claim_inventory_for_test()
            .unwrap_or_default();
        let driven = match runtime.drive_command(
            inputs.clone(),
            ObservedCquery {
                inner: root.clone(),
                trace: trace.clone(),
            },
        ) {
            Ok(driven) => driven,
            Err(error) => {
                let stage = runtime.native_demand_sessions.workspace_stage();
                let after = resources.snapshot();
                let after_claims = runtime
                    .repository_materializer
                    .claim_inventory_for_test()
                    .unwrap_or_default();
                let (computes, attempt, operations) = {
                    let trace = trace.lock().unwrap();
                    let event = trace
                        .events
                        .iter()
                        .rev()
                        .find(|event| event["event"] == "progress_refused");
                    let operations = event
                        .and_then(|event| event["needs"]["operations"].as_array())
                        .into_iter()
                        .flatten()
                        .filter_map(|row| row["kind"].as_str())
                        .map(str::to_owned)
                        .collect::<Vec<_>>();
                    (
                        trace.computes,
                        event.and_then(|event| event["attempt"].as_u64()),
                        operations,
                    )
                };
                // Retirement and rollback are part of the gate even on refusal.
                drop(initial);
                drop(runtime);
                let cleanup = resources.snapshot();
                println!(
                    "WP425_RUST_EDITION {case} {edition} refused {stage} computes={computes} attempt={attempt:?} operations={operations:?} error={} before={before:?} after={after:?} cleanup={cleanup:?}",
                    native_error(&error)
                );
                #[cfg(feature = "native-probe-observer")]
                {
                    let (_, progress) = profile.finish();
                    println!(
                        "WP425_RUST_BANK_REFUSAL {}",
                        json!({
                            "retained": progress["last_retained_admission_refusal"],
                            "backing": progress["last_backing_admission_refusal"],
                            "source_inventory": progress["last_source_bootstrap_backing_inventory"],
                            "closed_quotas": progress["closed_archive_quotas"],
                        })
                    );
                }
                assert_eq!(cleanup.native_fds, 0);
                assert_eq!(cleanup.backing_bytes, 0);
                assert_eq!(cleanup.retained_bytes, baseline);
                assert_eq!(after.native_fds, before.native_fds);
                assert_eq!(after.backing_bytes, before.backing_bytes);
                println!(
                    "WP425_RUST_CLAIM_ROLLBACK before={before_claims:?} after={after_claims:?}"
                );
                let mut before_controls = before_claims.domain_bytes;
                let mut after_controls = after_claims.domain_bytes;
                for (before, after) in [
                    (before_claims.general, after_claims.general),
                    (before_claims.source, after_claims.source),
                ] {
                    assert_eq!((before.dead_rows, after.dead_rows), (0, 0));
                    assert_eq!(before.live_rows, after.live_rows);
                    assert_eq!(before.live_row_bytes, after.live_row_bytes);
                    before_controls += before.capacity_bytes;
                    after_controls += after.capacity_bytes;
                }
                assert_eq!(
                    after.retained_bytes.checked_sub(after_controls).unwrap(),
                    before.retained_bytes.checked_sub(before_controls).unwrap(),
                );
                panic!("{case} command failed");
            }
        };
        let evaluation = driven
            .accepted
            .terminal_for_test()
            .result
            .as_ref()
            .as_ref()
            .unwrap_or_else(|error| panic!("{case} terminal failed: {}", terminal_leaf(error)));
        assert_eq!(
            rust_probe_projection(evaluation, edition),
            Some(expected_rust_probe_projection()),
            "{case} action projection"
        );
        let analysis = evaluation.analyses().next().unwrap();
        check_crate_info(analysis, edition);
        let selected = rust_probe_actions(analysis).collect::<Vec<_>>();
        let actions: [&ConfiguredAction; 3] = selected.try_into().expect("three selected actions");
        check_action_outputs(actions);
        if let Some(initial) = &initial {
            for (index, (action, original)) in actions.into_iter().zip(initial).enumerate() {
                if edition == "2021" {
                    // The wider configured context also holds ToolchainInfo.
                    // Compare actual payloads across the changed edition.
                    assert!(
                        (**action == **original) == (index != 0),
                        "{case} {} payload identity",
                        action.mnemonic()
                    );
                } else {
                    assert!(action == original, "{case} {} identity", action.mnemonic());
                }
            }
        } else {
            initial = Some(actions.map(Clone::clone));
        }
        let attempts = driven.attempts;
        drop(driven);
        let expected_module = if edition == "2021" {
            &edition_b
        } else {
            &edition_a
        };
        assert!(fs::read(&module).unwrap() == expected_module.as_bytes());
        for relative in OWNER_FILES
            .into_iter()
            .filter(|path| *path != "MODULE.bazel")
        {
            assert!(
                fs::read(workspace.join(relative)).unwrap()
                    == fs::read(staged.join(relative)).unwrap()
            );
        }
        println!(
            "WP425_RUST_EDITION {case} {edition} accepted attempts={attempts} bank={:?}",
            resources.snapshot()
        );
    }

    verify_inputs(&config, workspace);
    verify_inputs(&config, staged);
    drop(initial);
    drop(runtime);
    let snapshot = resources.snapshot();
    assert_eq!(snapshot.native_fds, 0);
    assert_eq!(snapshot.backing_bytes, 0);
    assert_eq!(snapshot.retained_bytes, baseline);
    #[cfg(feature = "native-probe-observer")]
    {
        let (_, progress) = profile.finish();
        println!(
            "WP425_RUST_SEALED_QUOTAS {}",
            progress["closed_archive_quotas"]
        );
    }
}
