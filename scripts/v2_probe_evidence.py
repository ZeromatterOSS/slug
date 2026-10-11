"""Finite, read-only probe evidence authentication; no dispatch or source acquisition.

Raw operation maps retain their original spellings and historical source brackets.
Only the named Q/L source exceptions are replaced by current source identity.
"""
import hashlib
import json
from pathlib import Path
import re

METADATA_LIMIT = 2 << 20
TIME_POLICY = dict(qualified_work_seconds=26, qualified_cleanup_seconds=29,
                   native_outer_seconds=32, default_work_seconds=5,
                   default_cleanup_seconds=8, ordinary_outer_seconds=10)
CHANGED = ['app/slug_core_v2/src/runtime/tests/platforms_module_preparation_tests.rs',
           'app/slug_core_v2/src/runtime/tests/platforms_module_preparation_tests/public_probe_preparation_tests.rs']
MEMORY = 'app/slug_core_v2/src/runtime/source_service/regular_file.rs'
READER = 'app/slug_core_v2/src/runtime/source_service/regular_file_tests.rs'
MEMORY_BEFORE = '04413bdc878b65224556f24a8d8b56dc9fe289041bd3704f866035323394d48f'
READER_BEFORE = '6da02db74f164389b00cb5c296cfeb84a644843481ffc724004bcadb8b536a55'
CURRENT_CHANGED = CHANGED + [MEMORY, READER]
HISTORY = Path('target/wp1132')
COMPILE_FREEZE = HISTORY / 'source-freeze.json'
COMPILE_FREEZE_SHA = '915501cdb77e419ed592c03e7f26bd6221f8166bb3b3d9cadd84b315b8525510'
ROOTS = dict(Q='target/wp1119', L='target/wp1128', S='target/wp1130',
             H='target/wp1129', O='target/wp1124', E='target/wp1125', I='target/wp1131')


def digest(path):
    value = hashlib.sha256()
    with Path(path).open('rb') as stream:
        while block := stream.read(65536):
            value.update(block)
    return value.hexdigest()


def encoded(path):
    with Path(path).open('rb') as stream:
        value = stream.read(METADATA_LIMIT + 1)
    assert len(value) <= METADATA_LIMIT, str(path)
    return value


def read(path):
    return json.loads(encoded(path))


def sha_map(value):
    assert isinstance(value, dict)
    for path, expected in value.items():
        assert isinstance(path, str) and path and isinstance(expected, str)
        assert re.fullmatch('[0-9a-f]{64}', expected), path
    return value


def equal_aliases(pins):
    resolved = {}
    for path, expected in sha_map(pins).items():
        canonical = str(Path(path).resolve())
        assert canonical not in resolved or resolved[canonical] == expected, path
        resolved[canonical] = expected
    return resolved


def validate_roles(roles, root=None):
    """Validate declarations without opening any donor body, archive or executable."""
    assert set(roles) == set(('authority_schema', 'baseline', 'changed_allowlist', 'checkpoint_name', 'contract_sha256', 'copy_count_selectors', 'core_counts', 'current_authorities', 'current_before', 'current_private_before', 'direct_groups', 'direct_pins', 'evidence_path', 'existing_current_only_sources', 'fixture_counts', 'gates', 'historical_private_paths', 'historical_vector_pins', 'historical_vector_root', 'historical_pins', 'historical_compile_root', 'last_actual_native_root', 'immediate_pins', 'inherited_pins', 'known_validation_sources_before', 'loading_counts', 'loading_private_before', 'loading_root', 'mandatory_projection_selectors', 'mandatory_skylib_selector', 'mandatory_time_profile_selectors', 'native_time_policy', 'new_authorities', 'new_gate_sources', 'original_vector_root', 'owner_test_introduction_root', 'prior', 'prior_receipt', 'reader_test_selectors', 'private_core_before', 'protected_diagnostic_before', 'protected_diagnostic_paths', 'protected_loading_before', 'protected_loading_paths', 'protected_root', 'root', 'rust_source_evidence_root', 'rust_source_policy', 'source_counts', 'source_evidence_pins', 'source_evidence_root', 'source_owner_pins', 'source_vector_policy', 'supervisor_path'))
    root = Path(roles['root'] if root is None else root)
    assert str(root) == roles['root'] and re.fullmatch(r'target/wp[1-9][0-9]*', roles['root'])
    assert roles['authority_schema'] == 'finite-probe-evidence-v1'
    assert roles['prior'] == roles['last_actual_native_root'] and roles['prior'] != roles['root']
    assert re.fullmatch(r'target/wp[1-9][0-9]*', roles['prior'])
    assert isinstance(roles['prior_receipt'], str) and re.fullmatch(r'thoughts/shared/plans/slug-v2-subplans/' + Path(roles['prior']).name + r'-[a-z0-9-]+\.json', roles['prior_receipt'])
    assert roles['historical_compile_root'] == str(HISTORY)
    assert roles['protected_root'] == ROOTS['Q'] and roles['loading_root'] == ROOTS['L']
    assert roles['owner_test_introduction_root'] == ROOTS['I']
    assert roles['source_evidence_root'] == ROOTS['E']
    assert roles['original_vector_root'] == ROOTS['O']
    assert roles['historical_vector_root'] == ROOTS['H']
    assert roles['rust_source_evidence_root'] == ROOTS['S']
    assert roles['supervisor_path'] == 'scripts/v2_probe_supervisor.py'
    assert roles['evidence_path'] == 'scripts/v2_probe_evidence.py'
    assert roles['checkpoint_name'] == 'root-source-checkpoint.json'
    assert re.fullmatch('[0-9a-f]{40}', roles['baseline'])
    assert re.fullmatch('[0-9a-f]{64}', roles['contract_sha256'])
    assert roles['changed_allowlist'] == CURRENT_CHANGED
    assert roles['known_validation_sources_before'] == {READER: READER_BEFORE}
    assert set(sha_map(roles['new_gate_sources'])) == {READER}
    assert len(roles['reader_test_selectors']) == len(set(roles['reader_test_selectors'])) == 7
    assert all(isinstance(s, str) for s in roles['reader_test_selectors'])
    assert roles['source_counts'] == dict(current=55, unchanged=51, vector=361, callers=8, environment=18, backings=6)
    assert roles['core_counts'] == dict(donor=52, extra=7, current=59, protected=36, focused=95)
    assert roles['loading_counts'] == dict(current=29)
    assert roles['fixture_counts'] == dict(selected=31, passed=31, deselected=3, objects=35, bytes=32440662)
    assert roles['native_time_policy'] == TIME_POLICY
    for key in ('direct_pins', 'historical_pins', 'immediate_pins', 'current_before', 'current_authorities',
                'source_evidence_pins', 'source_owner_pins', 'historical_vector_pins',
                'private_core_before', 'current_private_before', 'protected_diagnostic_before',
                'protected_loading_before', 'loading_private_before', 'existing_current_only_sources'):
        sha_map(roles[key])
    assert len(roles['current_before']) == 55 and len(roles['current_authorities']) == 6
    assert list(roles['current_authorities']) == roles['new_authorities']
    assert len(roles['existing_current_only_sources']) == 3
    assert roles['existing_current_only_sources'][READER] == roles['current_before'][READER] == READER_BEFORE
    assert roles['current_before'][MEMORY] == MEMORY_BEFORE
    for paths, mapping, count in [('historical_private_paths', 'private_core_before', 3),
                                  ('protected_diagnostic_paths', 'protected_diagnostic_before', 2),
                                  ('protected_loading_paths', 'protected_loading_before', 2)]:
        assert len(roles[paths]) == len(set(roles[paths])) == count
        assert set(roles[paths]) == set(roles[mapping])
    exceptions = roles['historical_private_paths'] + roles['protected_diagnostic_paths'] + roles['protected_loading_paths']
    assert len(set(exceptions)) == 7
    assert set(roles['current_private_before']) == set(roles['historical_private_paths'])
    assert set(roles['loading_private_before']) == set(roles['historical_private_paths']) - {roles['historical_private_paths'][1]}
    assert set(roles['direct_groups']) == {'Q', 'L', 'S', 'H', 'O', 'E', 'I', 'S-dependencies', 'E-dependencies', 'owners', 'runtime-supervisors'}
    assert set(roles['direct_pins']) == {p for paths in roles['direct_groups'].values() for p in paths}
    assert roles['direct_pins'][ROOTS['I'] + '/source-freeze.json'] == 'bc7c2ada61f722a437fc1ddcefaf694c0b8d549cdee4f3a754750f36d8492816'
    owner_test = 'app/slug_core_v2/src/runtime/path_observation/scoped_directory/tests.rs'
    assert roles['existing_current_only_sources'][owner_test] == '8f922912cb3e433c2ceb5772b41cdffec9ef7813d73f903b1a99d1435ff1bb27'
    assert roles['inherited_pins'] == {ROOTS['O'] + '/source-vector.json': roles['direct_pins'][ROOTS['O'] + '/source-vector.json']}
    expected = {(ROOTS['Q'], s, n, k) for s, n, k in [
        ('compile-bzlmod', 0, 'bzlmod'), ('bzlmod-list', 0, 'bzlmod'),
        ('bzlmod-preflight', 0, 'bzlmod'), ('bzlmod-tests', 7, 'bzlmod'), ('fixture-tests', 31, 'fixture')]}
    expected |= {(ROOTS['L'], s, n, 'loading') for s, n in [
        ('compile-loading', 0), ('loading-list', 0), ('loading-preflight', 0), ('loading-tests', 29)]}
    expected |= {(ROOTS['S'], s, 0, 'source') for s in ('inspection', 'qualification')}
    assert {(g['root'], g['stem'], g['count'], g['kind']) for g in roles['gates']} == expected
    assert len(roles['gates']) == 11
    for gate in roles['gates']:
        assert gate['supervisor'] == (ROOTS['H'] if gate['kind'] == 'source' else gate['root']) + '/run_gate.py'
        for name in [gate['stem'] + '-config.json', *[gate['stem'] + '/' + s for s in ('operation.json', 'stdout', 'stderr')]]:
            assert str(Path(gate['root']) / name) in roles['direct_pins']
    p = Path(roles['prior'])
    immediate = {'source-freeze.json', 'caller-source-pins.json', 'core-selectors.json', 'source-vector.json',
                 'native-config.json', 'slug_core_v2-executable.json', 'artifact-binding.json',
                 'root-source-checkpoint.json', 'root-gate-results.json', 'qualification.json', 'final-evidence-review.md'}
    receipt = roles['prior_receipt']
    assert set(roles['immediate_pins']) == {str(p / name) for name in immediate} | {receipt}
    compile_names = {'slug_core_v2-executable.json'} | {s + '-config.json' for s in ('compile-core', 'production-check')}
    compile_names |= {s + '/' + n for s in ('compile-core', 'production-check') for n in ('operation.json', 'stdout', 'stderr')}
    native_names = {'native-config.json', 'root-native-facts.json', 'next-frontier.json', 'native/operation.json', 'native/stdout', 'native/stderr'}
    assert set(roles['historical_pins']) == {str(HISTORY / n) for n in compile_names} | {str(p / n) for n in native_names}
    equal_aliases({**roles['direct_pins'], **roles['immediate_pins'], **roles['historical_pins']})
    return root


def validate_transition(roles, freeze):
    assert digest(Path(roles['prior']) / 'source-freeze.json') == roles['immediate_pins'][str(Path(roles['prior']) / 'source-freeze.json')]
    before, after = sha_map(freeze['before']), sha_map(freeze['after_format'])
    accepted = read(Path(roles['prior']) / 'source-freeze.json')['after_format']
    assert before == roles['current_before'] == {**accepted, READER: READER_BEFORE}
    assert READER not in accepted and len(accepted) == 54
    assert len(before) == 55 and set(after) == set(before)
    assert roles['changed_allowlist'] == CURRENT_CHANGED
    assert [p for p in before if before[p] != after[p]] == freeze['changed'] == CURRENT_CHANGED
    assert freeze['unchanged'] == 51
    assert freeze['new_authorities'] == roles['current_authorities'] == read(Path(roles['prior']) / 'source-freeze.json')['new_authorities']
    assert freeze['known_validation_sources_before'] == roles['known_validation_sources_before'] == {READER: READER_BEFORE}
    assert freeze['new_gate_sources'] == roles['new_gate_sources'] == {READER: after[READER]}
    return after


def authenticate_current_checkpoint(roles, root=None, config_pins=None):
    root = validate_roles(roles, root)
    checkpoint = read(root / roles['checkpoint_name'])
    freeze = read(root / 'source-freeze.json')
    assert checkpoint['verdict'] == 'ACCEPT'
    assert checkpoint['baseline'] == roles['baseline'] and checkpoint['contract_sha256'] == roles['contract_sha256']
    mandatory = {str(root / 'roles.json'), roles['supervisor_path'], roles['evidence_path'], str(COMPILE_FREEZE)}
    assert mandatory <= checkpoint['source_pins'].keys() and mandatory <= freeze['helper_pins'].keys()
    for mapping in (checkpoint['source_pins'], freeze['helper_pins']):
        assert mapping[str(COMPILE_FREEZE)] == COMPILE_FREEZE_SHA
        equal_aliases(mapping)
        for path, expected in mapping.items():
            assert digest(path) == expected, path
            if config_pins is not None:
                assert config_pins[path] == expected, path
    assert digest(root / 'packet-contract.md') == roles['contract_sha256']
    design = read(root / 'design-binding.json')
    assert design['verdict'] == 'ACCEPT' and design['baseline'] == roles['baseline']
    assert design['contract_sha256'] == roles['contract_sha256']
    assert digest(root / 'design-review.md') == design['design_review_sha256']
    after = validate_transition(roles, freeze)
    assert all(checkpoint['source_pins'][path] == after[path] for path in CURRENT_CHANGED)
    return checkpoint


def historical_compile_sources(roles):
    assert roles['historical_compile_root'] == str(HISTORY)
    checkpoint, freeze = read(Path(roles['root']) / roles['checkpoint_name']), read(Path(roles['root']) / 'source-freeze.json')
    assert checkpoint['source_pins'][str(COMPILE_FREEZE)] == freeze['helper_pins'][str(COMPILE_FREEZE)] == COMPILE_FREEZE_SHA
    assert digest(COMPILE_FREEZE) == COMPILE_FREEZE_SHA
    original = read(COMPILE_FREEZE)
    before = sha_map(original['after_format'])
    accepted = {p: h for p, h in roles['current_before'].items() if p != READER}
    assert roles['current_before'][READER] == READER_BEFORE
    assert len(before) == 54 and set(before) == set(accepted)
    assert before[MEMORY] == accepted[MEMORY] == MEMORY_BEFORE
    assert original['new_authorities'] == roles['current_authorities']
    assert [p for p in before if before[p] != accepted[p]] == CHANGED
    return before


def authenticate_operation(roles, gate, current_freeze=None):
    """Preserve the complete saved map, including automatic config/self identities."""
    compile_gate = gate['stem'] in ('compile-core', 'production-check') and gate['kind'] == 'current-core'
    assert gate in roles['gates'] or (compile_gate and gate['root'] in (str(HISTORY), roles['root']))
    assert current_freeze == read(Path(roles['root']) / 'source-freeze.json')
    current = validate_transition(roles, current_freeze)
    root, stem = Path(gate['root']), gate['stem']
    config_path, operation_path = root / (stem + '-config.json'), root / stem / 'operation.json'
    declared = roles['historical_pins'] if root == HISTORY else roles['direct_pins']
    for path in (config_path, operation_path, root / stem / 'stdout', root / stem / 'stderr'):
        if str(root) != roles['root']:
            assert digest(path) == declared[str(path)], str(path)
    config, operation = read(config_path), read(operation_path)
    assert operation['exit_code'] == 0 and operation['stop_reason'] is None
    assert operation['joined'] and operation['owned_group_empty']
    assert operation['pins_before'] == operation['pins_after']
    assert operation['command'] == config['command'] and operation['environment'] == config['environment']
    expected_count = 0 if gate['kind'] == 'fixture' else gate['count']
    assert operation['selected_count'] == operation['pass_count'] == expected_count
    assert operation['observed_test_passes'] == expected_count
    mode = ('preparation' if stem.startswith('compile-') or stem == 'production-check' else
            'ordinary' if stem.endswith('-tests') and gate['kind'] != 'fixture' else
            'preflight' if stem.endswith('-preflight') or gate['kind'] == 'fixture' else 'list')
    assert config['mode'] == operation['mode'] == mode
    config_count = {'bzlmod': 7, 'loading': 29}[gate['kind']] if stem.endswith(('-preflight', '-tests')) and gate['kind'] in ('bzlmod', 'loading') else expected_count
    assert config.get('expected_count', 0) == config_count
    assert operation['source_bindings'] == []
    assert operation['file_size_bytes'] == (32 << 20 if gate['kind'] == 'fixture' else 16 << 20)
    if mode != 'preparation':
        assert operation['deadline_seconds'] == 10 and 0 <= operation['elapsed_seconds'] <= 10
        assert 0 <= operation['monitored_rss_kib'] <= 262144
        assert 0 <= operation['child_rusage_peak_kib'] <= 262144
    else:
        assert operation['deadline_seconds'] == config['preparation_seconds']
    if mode == 'ordinary':
        assert config['environment'] == {'RUST_BACKTRACE': '0'}
    if gate['kind'] == 'fixture':
        assert config['fixture_output_bytes'] == 32 << 20
    pins = operation['pins_before']
    self_path = str(Path(gate['supervisor']).resolve())
    assert set(pins) == set(config['pins']) | {str(config_path), self_path}
    equal_aliases(pins)
    assert pins[str(config_path)] == digest(config_path)
    assert pins[self_path] == digest(gate['supervisor'])
    assert operation['stdout_sha256'] == digest(root / stem / 'stdout')
    assert operation['stderr_sha256'] == digest(root / stem / 'stderr')
    exceptions = {}
    if gate['kind'] in ('bzlmod', 'fixture'):
        exceptions = {**roles['private_core_before'], **roles['protected_diagnostic_before'], **roles['protected_loading_before']}
    elif gate['kind'] == 'loading':
        exceptions = {**roles['loading_private_before'], MEMORY: MEMORY_BEFORE}
    elif root == HISTORY:
        historical = historical_compile_sources(roles)
        assert all(pins[p] == expected for p, expected in historical.items())
        assert all(pins[p] == expected for p, expected in roles['current_authorities'].items())
        exceptions = {p: historical[p] for p in CHANGED + [MEMORY]}
    if gate['kind'] in ('bzlmod', 'fixture', 'loading'):
        historical = read(root / 'source-freeze.json')['after_format']
        assert all(pins[path] == expected for path, expected in historical.items())
        obsolete = {read(Path(ROOTS['Q']) / 'native-config.json')['executable'],
                    read(Path(ROOTS['Q']) / 'slug_loading_v2-executable.json')['path'],
                    read(Path(roles['prior']) / 'slug_core_v2-executable.json')['path']}
        if gate['kind'] == 'loading':
            obsolete.discard(read(root / 'slug_loading_v2-executable.json')['path'])
        assert not obsolete & pins.keys()
    for path, expected in pins.items():
        if path in exceptions:
            assert expected == exceptions[path] == historical[path]
            assert digest(path) == current[path], path
        else:
            assert digest(path) == expected, path
    if gate['kind'] == 'source':
        assert config['environment'] == {} and config['mode'] == 'list'
        assert config['command'] == [read(root / 'inspection-source-checkpoint.json')['supervisor']['python_executable'], str(root / ('inspect_rust_build.py' if stem == 'inspection' else 'qualify_rust_build.py'))]
        assert config['stem'] == str((root / stem).resolve())
    if str(root) == roles['root']:
        checkpoint = read(root / roles['checkpoint_name'])
        required = {**current, **roles['current_authorities'], **checkpoint['source_pins']}
        required.update({str(root / name): digest(root / name) for name in ('source-freeze.json', roles['checkpoint_name'])})
        assert all(pins[path] == expected for path, expected in required.items())
    return operation


def native_policy_equivalent(roles, config):
    prior = Path(roles['prior']) / 'native-config.json'
    assert digest(prior) == roles['immediate_pins'][str(prior)]
    previous = read(prior)
    assert set(config) == set(previous)
    assert config['native_time_policy'] == roles['native_time_policy'] == TIME_POLICY
    assert config['command'][1:] == previous['command'][1:]
    assert config['command'][0] == config['executable']
    for key in previous.keys() - {'command', 'executable', 'executable_sha256', 'pins'}:
        assert config[key] == previous[key], key
    assert len(config['environment']) == 18 and len(config['sources']) == 361
    assert config['native_output_bytes'] == 32 << 20
    assert isinstance(config['pins'], list) and len(config['pins']) == len(set(config['pins']))
    root = validate_roles(roles)
    required = set(roles['historical_pins']) | (set(roles['direct_pins']) - set(roles['historical_vector_pins'])) | set(roles['immediate_pins'])
    required |= set(roles['source_owner_pins']) | set(roles['source_evidence_pins'])
    required |= set(roles['current_before']) | set(roles['current_authorities'])
    required |= {str(root / name) for name in ('roles.json', roles['checkpoint_name'], 'source-freeze.json',
                    'source-vector.json', 'core-selectors.json', 'caller-source-pins.json', 'slug_core_v2-executable.json', 'artifact-binding.json')}
    protected_artifacts = [read(Path(ROOTS[key]) / (family + '-executable.json')) for key, family in [('Q', 'slug_bzlmod_v2'), ('L', 'slug_loading_v2')]]
    required |= {a['path'] for a in protected_artifacts}
    required |= {roles['supervisor_path'], roles['evidence_path'], str(COMPILE_FREEZE), config['executable']}
    assert required <= set(config['pins']), sorted(required - set(config['pins']))
    assert not set(roles['historical_vector_pins']) & set(config['pins'])
    assert digest(config['executable']) == config['executable_sha256']
    pin_hashes = {path: digest(path) for path in config['pins']}
    after = validate_transition(roles, read(root / 'source-freeze.json'))
    declared = {**roles['direct_pins'], **roles['immediate_pins'], **roles['historical_pins'], **after, **roles['current_authorities'], str(COMPILE_FREEZE): COMPILE_FREEZE_SHA}
    for path in pin_hashes.keys() & declared.keys():
        assert pin_hashes[path] == declared[path], path
    assert all(pin_hashes[a['path']] == a['sha256'] for a in protected_artifacts)
    equal_aliases(pin_hashes)
    authenticate_current_checkpoint(roles, config_pins=pin_hashes)
    assert read(root / 'source-vector.json') == config['sources']
    artifact = authenticate_current_compile(roles, root)['artifact']
    assert (artifact['path'], artifact['sha256']) == (config['executable'], config['executable_sha256'])
    return previous


def qualified_sources(roles, root):
    """Authenticate saved source prerequisites; never reobserve a Host leaf."""
    O, H, E, S, P, R = [Path(p) for p in (ROOTS['O'], ROOTS['H'], ROOTS['E'], ROOTS['S'], roles['prior'], str(root))]
    ROLES = roles
    sha = digest
    policy = ROLES['source_vector_policy']
    original, sources = read(O / 'source-vector.json'), read(H / 'source-vector.json')
    assert policy['original_rows'] == len(original) == 359
    assert policy['current_rows'] == len(sources) == 360
    assert sha(O / 'source-vector.json') == policy['original_sha256'] == ROLES['inherited_pins'][str(O / 'source-vector.json')]
    assert sha(H / 'source-vector.json') == policy['current_sha256']
    assert sources == read(H / 'native-config.json')['sources']
    assert sha(H / 'source-vector.json') == policy['current_sha256'] == ROLES['direct_pins'][str(H / 'source-vector.json')]
    root = read(P / 'native-config.json')['environment']['SLUG_NATIVE_BAZEL_SKYLIB_ROOT']
    row = dict(path=str(Path(root) / 'lib/collections.bzl'), bytes=2176,
               sha256='89533929b5b22427531df5505aef56b9e6876bd91b4a56df56b94f0d9c33a1de')
    assert policy['relative'] == 'lib/collections.bzl' and policy['appended_row'] == row
    assert sources[:359] == original and sources[359:] == [row]
    assert not any(item['path'] == row['path'] for item in original)
    assert encoded(H / 'source-vector.json').startswith(encoded(O / 'source-vector.json').rstrip()[:-1])
    for path, expected in ROLES['source_evidence_pins'].items():
        assert sha(path) == expected, path
    evidence, host = read(E / 'skylib-closure-evidence.json'), read(E / 'host-source-qualification.json')
    assert evidence['seed'] == policy['relative'] and evidence['edges'] == 0
    assert len(evidence['existing_rows']) == len(set(evidence['existing_rows'])) == 14
    assert len(evidence['source_records']) == 1
    member = evidence['source_records'][0]
    assert {key: member[key] for key in ('path', 'bytes', 'sha256', 'loads')} == dict(
        path=policy['relative'], bytes=row['bytes'], sha256=row['sha256'], loads=[])
    assert {key: evidence['archive'][key] for key in ('bytes', 'sha256')} == dict(
        bytes=44667, sha256='6e78f0e57de26801f6f564fa7c4a48dc8b36873e416257a92bbb0937eeac8446')
    assert host['original_vector'] == str(O / 'source-vector.json') and host['original_sha256'] == policy['original_sha256']
    assert host['evidence_sha256'] == sha(E / 'skylib-closure-evidence.json') and host['root'] == root
    assert host['original_rows'] == 359 and host['rows'] == 360 and host['appended_rows'] == [row]
    assert host['original_encoded_prefix_preserved'] and not host['atomic_collection'] and not host['source_certificate']
    assert len(host['qualifications']) == 1
    proof = host['qualifications'][0]
    assert proof['relative'] == policy['relative'] and proof['row'] == row and proof['archive_member_equal']
    assert proof['lease']['status'] == 'observed' and proof['lease']['bytes'] == row['bytes']
    checkpoint = read(E / 'inspection-source-checkpoint.json')
    assert checkpoint['verdict'] == 'ACCEPT'
    assert all(sha(E / name) == expected for name, expected in checkpoint['source_pins'].items())
    assert encoded(E / 'cohort-review.md').decode().startswith('ACCEPT')
    design = read(R / 'design-binding.json')
    assert design['verdict'] == 'ACCEPT' and design['baseline'] == ROLES['baseline']
    assert design['contract_sha256'] == ROLES['contract_sha256']
    assert sha(R / 'design-review.md') == design['design_review_sha256']
    leaf_policy = ROLES['rust_source_policy']
    assert leaf_policy['binding_state'] == 'accepted_actual_cohort'
    assert leaf_policy['relative'] == 'rust/BUILD.bazel'
    assert leaf_policy['inherited_rows'] == len(sources) == 360 and leaf_policy['current_rows'] == 361
    assert leaf_policy['body_limit_bytes'] == 65536
    binding_path = S / 'rust-source-binding.json'
    assert re.fullmatch('[0-9a-f]{64}', leaf_policy['binding_sha256'])
    assert sha(binding_path) == leaf_policy['binding_sha256']
    binding = read(binding_path)
    assert binding['verdict'] == 'ACCEPT'
    expected_paths = {str(S / name) for name in leaf_policy['evidence_names']}
    assert set(binding['evidence_sha256']) == expected_paths
    for path, expected in binding['evidence_sha256'].items():
        assert re.fullmatch('[0-9a-f]{64}', expected) and sha(path) == expected
        assert Path(path).stat().st_size <= 2 << 20
    checkpoint = read(S / 'inspection-source-checkpoint.json')
    assert checkpoint['verdict'] == 'ACCEPT'
    assert all(sha(path) == expected for path, expected in checkpoint['source_pins'].items())
    assert encoded(S / 'cohort-review.md').decode().startswith('ACCEPT')
    capture = read(S / 'rust-build-capture.json')
    qualified = read(S / 'host-source-qualification.json')
    rust_root = read(P / 'native-config.json')['environment']['SLUG_NATIVE_RULES_RUST_ROOT']
    row = binding['row']
    assert set(row) == {'path', 'bytes', 'sha256'}
    assert row['path'] == str(Path(rust_root) / leaf_policy['relative'])
    assert type(row['bytes']) is int and 0 <= row['bytes'] <= 65536
    assert re.fullmatch('[0-9a-f]{64}', row['sha256'])
    assert (capture['root'], capture['relative'], capture['path']) == (rust_root, leaf_policy['relative'], row['path'])
    observed = capture['observation']
    assert observed['status'] == 'observed' and (observed['bytes'], observed['sha256']) == (row['bytes'], row['sha256'])
    assert capture['atomic_collection'] is capture['source_certificate'] is False
    assert capture['caps'] == dict(body=65536, aggregate=65536, scratch=65536, seconds=10,
        nodes=100000, edges=512, labels=256, expression=1024, report=2 << 20)
    assert capture['elapsed_seconds'] <= 10 and qualified['elapsed_seconds'] <= 10
    assert qualified['original_vector'] == str(H / 'source-vector.json')
    assert qualified['original_sha256'] == sha(H / 'source-vector.json')
    assert (qualified['original_rows'], qualified['rows'], qualified['appended_rows'], qualified['root']) == (360, 361, [row], rust_root)
    assert qualified['capture_sha256'] == sha(S / 'rust-build-capture.json')
    assert qualified['held_before'] == qualified['held_after'] == dict(bytes=row['bytes'], sha256=row['sha256'], eof=True)
    lease = qualified['lease']
    assert lease['status'] == 'observed' and lease['source_certificate'] is False
    assert lease['expected_bytes'] == lease['bytes'] == lease['shared_offset'] == row['bytes']
    assert lease['file_identity'] == observed['identity'] and lease['parent_chain'] == observed['ancestors']
    assert qualified['activated'] is qualified['atomic_collection'] is qualified['source_certificate'] is False
    assert qualified['original_encoded_prefix_preserved'] is True
    for report in (capture, qualified):
        assert report['pins_before'] == report['pins_after']
        for path, expected in report['pins_before'].items():
            assert sha(path) == expected
    current_raw = encoded(R / 'source-vector.json')
    prior_prefix = encoded(H / 'source-vector.json').rstrip()[:-1]
    assert len(current_raw) <= 2 << 20 and current_raw.startswith(prior_prefix)
    assert qualified['original_encoded_prefix_sha256'] == hashlib.sha256(prior_prefix).hexdigest()
    assert qualified['candidate_vector'] == str(S / 'candidate-source-vector.json')
    assert qualified['candidate_sha256'] == sha(S / 'candidate-source-vector.json') == sha(R / 'source-vector.json') == binding['source_vector_sha256']
    assert current_raw == encoded(P / 'source-vector.json') == encoded(S / 'source-vector.json')
    assert read(P / 'native-config.json')['sources'] == read(P / 'source-vector.json')
    current_sources = read(R / 'source-vector.json')
    assert current_sources == read(S / 'candidate-source-vector.json') == sources + [row]
    assert len(current_sources) == len({item['path'] for item in current_sources}) == 361
    return current_sources



def authenticate_current_compile(roles, root=None, include_production=True):
    assert type(include_production) is bool
    root = validate_roles(roles, root)
    authenticate_current_checkpoint(roles, root)
    freeze = read(root / 'source-freeze.json')
    stems = ('compile-core', 'production-check') if include_production else ('compile-core',)
    operations = {}
    for stem in stems:
        config, template = read(root / (stem + '-config.json')), read(HISTORY / (stem + '-config.json'))
        assert digest(HISTORY / (stem + '-config.json')) == roles['historical_pins'][str(HISTORY / (stem + '-config.json'))]
        assert set(config) == set(template) and all(config[k] == template[k] for k in config.keys() - {'pins'})
        gate = dict(root=str(root), stem=stem, count=0, kind='current-core', supervisor=roles['supervisor_path'])
        operations[stem] = authenticate_operation(roles, gate, freeze)
    artifact, binding = read(root / 'slug_core_v2-executable.json'), read(root / 'artifact-binding.json')
    paths = dict(source_freeze=root / 'source-freeze.json', checkpoint=root / roles['checkpoint_name'],
                 compile_config=root / 'compile-core-config.json', compile_operation=root / 'compile-core/operation.json',
                 compile_stdout=root / 'compile-core/stdout', compile_stderr=root / 'compile-core/stderr',
                 artifact_descriptor=root / 'slug_core_v2-executable.json')
    assert set(binding) == {k + '_sha256' for k in paths} | {'artifact_sha256', 'cargo_artifact', 'build_finished'}
    assert all(binding[k + '_sha256'] == digest(path) for k, path in paths.items())
    rows = [json.loads(line) for line in encoded(root / 'compile-core/stdout').splitlines() if line.startswith(b'{')]
    selected = [row for row in rows if row.get('reason') == 'compiler-artifact' and row.get('executable') == artifact['path']]
    assert len(selected) == 1 and selected[0] == binding['cargo_artifact']
    row = selected[0]
    assert row['target']['name'] == 'slug_core_v2' and row['target']['kind'] == ['lib']
    assert row['profile'] == dict(opt_level='1', debuginfo=0, debug_assertions=True, overflow_checks=True, test=True)
    assert row['features'] == artifact['features'] == ['default', 'native-probe-observer']
    assert [r for r in rows if r.get('reason') == 'build-finished'] == [binding['build_finished']] == [dict(reason='build-finished', success=True)]
    assert artifact['compile_stem'] == 'compile-core' and artifact['compile_operation_sha256'] == digest(paths['compile_operation'])
    assert binding['artifact_sha256'] == artifact['sha256'] == digest(artifact['path'])
    if include_production:
        rows = [json.loads(line) for line in encoded(root / 'production-check/stdout').splitlines() if line.startswith(b'{')]
        assert [r for r in rows if r.get('reason') == 'build-finished'] == [dict(reason='build-finished', success=True)]
        for family in ('slug_core_v2', 'slug'):
            selected = [r for r in rows if r.get('reason') == 'compiler-artifact' and r['target']['name'] == family]
            profile = dict(opt_level='1', debuginfo=0 if family == 'slug_core_v2' else 2, debug_assertions=True, overflow_checks=True, test=False)
            assert selected and all(r['features'] == ['default'] and r['profile'] == profile and r['executable'] is None for r in selected)
    return dict(artifact=artifact, operations=operations, artifact_binding_sha256=digest(root / 'artifact-binding.json'))


def authenticate_last_native(roles, after):
    """Saved actual native maps are historical metadata, never old helper execution."""
    prior = Path(roles['last_actual_native_root'])
    validate_roles(roles)
    assert prior == Path(roles['prior'])
    for name in ('native-config.json', 'native/operation.json', 'native/stdout', 'native/stderr', 'root-native-facts.json', 'next-frontier.json'):
        assert digest(prior / name) == roles['historical_pins'][str(prior / name)]
    for name in ('source-freeze.json', roles['checkpoint_name'], 'slug_core_v2-executable.json', 'artifact-binding.json'):
        assert digest(prior / name) == roles['immediate_pins'][str(prior / name)]
    freeze, checkpoint = read(prior / 'source-freeze.json'), read(prior / roles['checkpoint_name'])
    original, native = freeze['after_format'], read(prior / 'native-config.json')
    assert checkpoint['verdict'] == 'ACCEPT' and original == {p: h for p, h in roles['current_before'].items() if p != READER}
    assert READER not in original and len(original) == 54 and original[MEMORY] == MEMORY_BEFORE
    receipt_path = Path(roles['prior_receipt'])
    assert digest(receipt_path) == roles['immediate_pins'][str(receipt_path)]
    receipt = read(receipt_path)
    assert receipt['verdict'] == 'ACCEPT'
    assert receipt['baseline'] == checkpoint['baseline'] == freeze['baseline']
    assert receipt['contract_sha256'] == checkpoint['contract_sha256'] == digest(prior / 'packet-contract.md')
    assert receipt['source_pins'] == checkpoint['source_pins']
    assert all(receipt['source_pins'][p] == h for mapping in (original, freeze['new_authorities'], freeze['helper_pins']) for p, h in mapping.items())
    facts = read(prior / 'root-native-facts.json')
    assert receipt['native'] == facts
    for field, name in [('operation_sha256', 'native/operation.json'), ('config_sha256', 'native-config.json'), ('stdout_sha256', 'native/stdout')]:
        assert facts[field] == digest(prior / name) == receipt['evidence_sha256'][str(prior / name)]
    artifact, binding = read(prior / 'slug_core_v2-executable.json'), read(prior / 'artifact-binding.json')
    assert binding['source_freeze_sha256'] == digest(prior / 'source-freeze.json')
    assert binding['checkpoint_sha256'] == digest(prior / roles['checkpoint_name'])
    assert binding['artifact_descriptor_sha256'] == digest(prior / 'slug_core_v2-executable.json')
    assert binding['artifact_sha256'] == artifact['sha256'] and binding['compile_operation_sha256'] == artifact['compile_operation_sha256']
    assert artifact['features'] == ['default', 'native-probe-observer'] and artifact['compile_stem'] == 'compile-core'
    assert native['executable'] == artifact['path'] and native['executable_sha256'] == artifact['sha256']
    operation = read(prior / 'native/operation.json')
    assert operation['exit_code'] == 0 and operation['stop_reason'] is None and operation['joined'] and operation['owned_group_empty']
    assert operation['pins_before'] == operation['pins_after'] and operation['selected_count'] == operation['pass_count'] == 1
    assert operation['command'] == native['command'] and operation['environment'] == native['environment']
    pins = operation['pins_before']
    self_path = str(Path(roles['supervisor_path']).resolve())
    automatic = {str(prior / 'native-config.json'), self_path}
    assert isinstance(native['pins'], list) and len(native['pins']) == len(set(native['pins']))
    assert len(automatic) == 2 and not automatic & set(native['pins'])
    assert set(pins) == set(native['pins']) | automatic and len(pins) == len(native['pins']) + 2
    equal_aliases(pins)
    assert all(pins[p] == expected for p, expected in original.items())
    assert all(pins[p] == expected for p, expected in roles['current_authorities'].items())
    current_checkpoint = read(Path(roles['root']) / roles['checkpoint_name'])
    historical_helpers = ('scripts/v2_probe_evidence.py', 'scripts/v2_probe_evidence_tests.py')
    for p in historical_helpers:
        assert pins[p] == freeze['helper_pins'][p] == checkpoint['source_pins'][p]
        assert digest(p) == current_checkpoint['source_pins'][p]
    assert all(pins[p] == expected for p, expected in checkpoint['source_pins'].items())
    for p, expected in pins.items():
        if p in CHANGED + [MEMORY]:
            assert expected == original[p] and digest(p) == after[p]
        elif p == artifact['path']:
            assert expected == artifact['sha256']  # Superseded Core executable is saved metadata only.
        elif p not in historical_helpers:
            assert digest(p) == expected, p
    assert len(operation['source_bindings']) == len(native['sources']) == 361
    assert all(r['before'] == r['after'] == {k: row[k] for k in ('bytes', 'sha256')} and r['path'] == row['path'] for r, row in zip(operation['source_bindings'], native['sources']))
    assert operation['stdout_sha256'] == digest(prior / 'native/stdout') and operation['stderr_sha256'] == digest(prior / 'native/stderr')
    assert native['native_time_policy'] == TIME_POLICY and len(native['environment']) == 18
    return native


def _current_callers(roles, root, after):
    prior = Path(roles['prior']) / 'caller-source-pins.json'
    assert digest(prior) == roles['immediate_pins'][str(prior)]
    old, callers = read(prior), read(root / 'caller-source-pins.json')
    assert set(callers) == set(old) == {str(i) for i in range(1, 9)}
    assert old['2'] == dict(path=MEMORY, sha256=MEMORY_BEFORE, bytes=15294)
    assert roles['current_before'][MEMORY] == old['2']['sha256']
    assert all(callers[i] == old[i] for i in old if i != '2')
    assert callers['2']['path'] == old['2']['path'] == MEMORY
    for row in callers.values():
        assert set(row) == {'path', 'sha256', 'bytes'}
        assert row['sha256'] == after[row['path']] == digest(row['path'])
        assert row['bytes'] == Path(row['path']).stat().st_size
    return callers


def _current_selectors(roles, root):
    prior = Path(roles['prior']) / 'core-selectors.json'
    assert digest(prior) == roles['immediate_pins'][str(prior)]
    donor, current = read(prior), read(root / 'core-selectors.json')
    assert set(current) == set(donor)
    assert all(current[k] == donor[k] for k in donor if k != 'ordinary')
    assert len(donor['ordinary']) == len(set(donor['ordinary'])) == 52
    ordinary = current['ordinary']
    assert ordinary == donor['ordinary'] + roles['reader_test_selectors']
    assert len(ordinary) == len(set(ordinary)) == 59
    return ordinary


def qualify_direct_evidence(roles, root=None, current_freeze=None):
    """Authenticate fixed donors and current identity; do not execute or publish."""
    root = validate_roles(roles, root)
    for mapping in (roles['direct_pins'], roles['immediate_pins'], roles['historical_pins']):
        for path, expected in mapping.items():
            assert digest(path) == expected, path
    P, Q, L, I = [Path(p) for p in (roles['prior'], ROOTS['Q'], ROOTS['L'], ROOTS['I'])]
    previous = read(P / 'source-freeze.json')
    current, historical = roles['current_before'], read(Q / 'source-freeze.json')['after_format']
    assert {**previous['after_format'], READER: READER_BEFORE} == current and READER not in previous['after_format'] and len(historical) == 52
    authenticate_current_checkpoint(roles, root)
    freeze = read(root / 'source-freeze.json')
    assert current_freeze is None or current_freeze == freeze
    after = validate_transition(roles, freeze)
    compiled = authenticate_current_compile(roles, root)
    assert previous['new_authorities'] == roles['current_authorities']
    assert read(Q / 'source-freeze.json')['new_authorities'] == previous['new_authorities']
    assert set(current) == set(historical) | set(roles['existing_current_only_sources'])
    assert not set(historical) & set(roles['existing_current_only_sources'])
    assert roles['existing_current_only_sources'] == {p: current[p] for p in roles['existing_current_only_sources']}
    owner_test = 'app/slug_core_v2/src/runtime/path_observation/scoped_directory/tests.rs'
    intro = read(I / 'source-freeze.json')
    assert intro['known_validation_sources_before'] == {owner_test: '8f922912cb3e433c2ceb5772b41cdffec9ef7813d73f903b1a99d1435ff1bb27'}
    assert intro['after_format'][owner_test] == current[owner_test] == intro['known_validation_sources_before'][owner_test]
    assert previous['known_validation_sources_before'] == {}
    exceptions = {**roles['private_core_before'], **roles['protected_diagnostic_before'], **roles['protected_loading_before']}
    assert exceptions == {p: historical[p] for p in exceptions}
    assert roles['current_private_before'] == {p: current[p] for p in roles['historical_private_paths']}
    assert roles['loading_private_before'] == {p: read(L / 'source-freeze.json')['after_format'][p] for p in roles['loading_private_before']}
    assert all(current[p] == expected for p, expected in historical.items() if p not in exceptions)
    for mapping in (after, roles['current_authorities']):
        for path, expected in mapping.items():
            assert digest(path) == expected, path
    _current_callers(roles, root, after)
    ordinary = _current_selectors(roles, root)
    required = roles['mandatory_projection_selectors'] + roles['mandatory_time_profile_selectors'] + roles['copy_count_selectors'] + [roles['mandatory_skylib_selector']]
    assert len(required) == len(set(required)) == 7 and all(ordinary.count(s) == 1 for s in required)
    assert read(L / 'loading-selectors.json')['ordinary'][:26] == read(Q / 'loading-selectors.json')['ordinary']
    assert len(read(L / 'loading-selectors.json')['ordinary']) == 29
    operations = {g['root'] + '/' + g['stem']: authenticate_operation(roles, g, freeze) for g in roles['gates']}
    for stem in ('compile-core', 'production-check'):
        gate = dict(root=str(HISTORY), stem=stem, count=0, kind='current-core', supervisor=str(HISTORY / 'run_gate.py'))
        operation = authenticate_operation(roles, gate, freeze)
        assert all(operation['pins_before'][p] == expected for p, expected in historical_compile_sources(roles).items())
        assert all(operation['pins_before'][p] == expected for p, expected in roles['current_authorities'].items())
        operations[str(HISTORY / stem)] = operation
    artifacts = [('slug_loading_v2', L, 'compile-loading', []), ('slug_bzlmod_v2', Q, 'compile-bzlmod', [])]
    for family, donor_root, stem, features in artifacts:
        artifact = read(donor_root / (family + '-executable.json'))
        assert artifact['features'] == features and artifact['compile_stem'] == stem
        assert artifact['compile_operation_sha256'] == digest(donor_root / stem / 'operation.json')
        assert digest(artifact['path']) == artifact['sha256']
    native = authenticate_last_native(roles, after)
    sources = qualified_sources(roles, root)
    assert sources == native['sources'] and len(sources) == 361
    fixture = read(Q / 'fixture-results.json')
    assert (fixture['selected'], fixture['passed'], fixture['deselected']) == (31, 31, 3)
    assert (fixture['object_count'], fixture['source_bytes']) == (35, 32440662)
    assert fixture['operation_sha256'] == digest(Q / 'fixture-tests/operation.json')
    assert fixture['stdout_sha256'] == digest(Q / 'fixture-tests/stdout')
    assert fixture['inventory_sha256'] == '6895d4ffee1075659d9db5709bf4a1418b0cc772ac7d95cbeaf472c14c669538'
    return dict(schema=roles['authority_schema'], direct_pins=dict(roles['direct_pins']),
                immediate_pins=dict(roles['immediate_pins']), source_paths=55, source_rows=361,
                compile_evidence='fresh-current-after55; historical-P1132-metadata-only',
                historical_pins=dict(roles['historical_pins']), artifact=compiled['artifact'], last_actual_native_root=roles['last_actual_native_root'],
                native_template_root=str(P),
                protected_tests=36, fixture_tests=31, portable_unproved=3,
                operation_sha256={path: digest(Path(path) / 'operation.json') for path in operations},
                private_core_before=roles['private_core_before'], private_core_after={p: after[p] for p in roles['historical_private_paths']},
                protected_diagnostic_before=roles['protected_diagnostic_before'],
                protected_diagnostic_after={p: after[p] for p in roles['protected_diagnostic_paths']},
                protected_loading_before=roles['protected_loading_before'],
                protected_loading_after={p: current[p] for p in roles['protected_loading_paths']},
                old_core_and_q_loading='metadata-only-no-physical-runtime-credit',
                atomic_collection=False, source_certificate=False)
