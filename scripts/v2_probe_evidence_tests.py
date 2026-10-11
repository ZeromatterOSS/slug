"""Owned local controls for the fixed supervisor and direct evidence contract."""
import copy
import json
from pathlib import Path
import re
import sys
import tempfile
import time
import unittest
from unittest import mock

import v2_probe_supervisor as supervisor
import v2_probe_evidence as evidence


ROOT = None
SHARED = ('scripts/v2_probe_supervisor.py', 'scripts/v2_probe_evidence.py',
          'scripts/v2_probe_evidence_tests.py')


def dump(path, value):
    Path(path).write_text(json.dumps(value, indent=2) + '\n')


def control_root(args):
    assert len(args) == 1 and isinstance(args[0], str)
    root = Path(args[0])
    assert str(root) == args[0] and re.fullmatch(r'target/wp[1-9][0-9]*', args[0])
    roles = evidence.read(root / 'roles.json')
    assert roles['root'] == str(root)
    evidence.authenticate_current_checkpoint(roles, root)
    return root


class DirectEvidenceControls(unittest.TestCase):
    def setUp(self):
        self.roles = json.loads((ROOT / 'roles.json').read_text())

    def test_fixed_role_schema_counts_and_introduction(self):
        evidence.validate_roles(self.roles)
        for key, value in (('extra_owner', 'invented'), ('owner_test_introduction_root', 'target/wp1132'),
                           ('historical_compile_root', 'target/wp1134'), ('last_actual_native_root', 'target/wp1133'),
                           ('prior', 'target/wp1133'), ('prior_receipt', 'thoughts/shared/plans/slug-v2-subplans/wp1133-invented.json'),
                           ('core_counts', {**self.roles['core_counts'], 'current': 52}),
                           ('loading_counts', {'current': 29}), ('loading_test_selector', 'bzl_module::invented'),
                           ('source_counts', {**self.roles['source_counts'], 'current': 54}),
                           ('native_time_policy', {**self.roles['native_time_policy'], 'native_outer_seconds': 33})):
            changed = copy.deepcopy(self.roles)
            changed[key] = value
            with self.assertRaises((AssertionError, KeyError, ValueError)):
                evidence.validate_roles(changed)

    def test_current_root_seam_and_escape_refusal(self):
        changed = copy.deepcopy(self.roles)
        alternate = Path('target/wp9999999999999999999')
        assert alternate != ROOT and not alternate.exists()
        changed['root'] = str(alternate)
        self.assertEqual(evidence.validate_roles(changed, alternate), alternate)
        with self.assertRaises(AssertionError):
            evidence.validate_roles(changed, ROOT)
        changed['root'] = str(ROOT) + '/../wp9999999999999999999'
        with self.assertRaises(AssertionError):
            evidence.validate_roles(changed)
        self.assertEqual(control_root([str(ROOT)]), ROOT)
        for args in ([], [str(ROOT), str(ROOT)], ['./' + str(ROOT)], [str(ROOT) + '/../wp1']):
            with self.assertRaises(AssertionError):
                control_root(args)
        changed = copy.deepcopy(self.roles); changed['root'] = str(alternate)
        with self.replaced_record(ROOT / 'roles.json', changed), self.assertRaises(AssertionError):
            control_root([str(ROOT)])
        cp = evidence.read(ROOT / self.roles['checkpoint_name']); cp['verdict'] = 'REJECT'
        with self.replaced_record(ROOT / self.roles['checkpoint_name'], cp), self.assertRaises(AssertionError):
            control_root([str(ROOT)])

    def test_direct_pin_missing_and_wrong_alias_digest(self):
        changed = copy.deepcopy(self.roles)
        path = 'target/wp1131/source-freeze.json'
        del changed['direct_pins'][path]
        with self.assertRaises((AssertionError, KeyError, ValueError)):
            evidence.validate_roles(changed)
        changed = copy.deepcopy(self.roles)
        changed['direct_pins'][str(Path(path).absolute())] = '0' * 64
        with self.assertRaises((AssertionError, KeyError, ValueError)):
            evidence.validate_roles(changed)
        changed = copy.deepcopy(self.roles)
        changed['direct_pins'][path] = '0' * 64
        with self.assertRaises((AssertionError, KeyError, ValueError)):
            evidence.validate_roles(changed)

    def test_native_policy_count_order_and_environment_are_exact(self):
        config = json.loads((ROOT / 'native-policy-prototype.json').read_text())
        evidence.native_policy_equivalent(self.roles, config)
        for key, value in (('expected_count', 2), ('native_output_bytes', 64 << 20),
                           ('sources', list(reversed(config['sources']))),
                           ('command', config['command'] + ['--ignored']),
                           ('environment', {**config['environment'], 'RUST_BACKTRACE': '1'}),
                           ('native_time_policy', {**config['native_time_policy'], 'qualified_work_seconds': 27})):
            changed = copy.deepcopy(config)
            changed[key] = value
            with self.assertRaises((AssertionError, KeyError, ValueError)):
                evidence.native_policy_equivalent(self.roles, changed)
        for path in ('target/wp1131/source-freeze.json', str(evidence.COMPILE_FREEZE), str(evidence.READER_FREEZE), 'scripts/v2_probe_evidence.py',
                     str(ROOT / 'loading-artifact-binding.json'), str(ROOT / 'loading-selectors.json'),
                     str(ROOT / 'loading-tests/operation.json'),
                     str(ROOT / 'root-source-checkpoint.json')):
            changed = copy.deepcopy(config)
            changed['pins'].remove(path)
            with self.assertRaises((AssertionError, KeyError, ValueError)):
                evidence.native_policy_equivalent(self.roles, changed)


    def replaced_record(self, path, value):
        original = evidence.read
        return mock.patch.object(evidence, 'read', side_effect=lambda p: copy.deepcopy(value) if Path(p) == Path(path) else original(p))

    def test_literal_transition_rejects_extra_missing_and_false_after(self):
        freeze = evidence.read(ROOT / 'source-freeze.json')
        evidence.validate_transition(self.roles, freeze)
        third = next(p for p in freeze['before'] if p not in evidence.CURRENT_CHANGED)
        for field, key, value in [('after_format', third, '0' * 64), ('after_format', evidence.CHANGED[0], freeze['before'][evidence.CHANGED[0]])]:
            bad = copy.deepcopy(freeze); bad[field][key] = value
            with self.assertRaises(AssertionError):
                evidence.validate_transition(self.roles, bad)
        for key, value in [('changed', evidence.CURRENT_CHANGED[:1]), ('changed', list(reversed(evidence.CURRENT_CHANGED))), ('unchanged', 53), ('known_validation_sources_before', {third: '0' * 64}), ('new_gate_sources', {evidence.READER: evidence.READER_BEFORE}), ('new_gate_sources', {evidence.LOADING_TEST: evidence.LOADING_TEST_BEFORE})]:
            bad = copy.deepcopy(freeze); bad[key] = value
            with self.assertRaises(AssertionError):
                evidence.validate_transition(self.roles, bad)
        bad = copy.deepcopy(freeze); bad['after_format'][evidence.CHANGED[0]] = '0' * 64
        with self.replaced_record(ROOT / 'source-freeze.json', bad), self.assertRaises(AssertionError):
            evidence.authenticate_current_checkpoint(self.roles)
        original = evidence.digest
        with mock.patch.object(evidence, 'digest', side_effect=lambda p: freeze['before'][str(p)] if str(p) in evidence.CHANGED else original(p)), self.assertRaises(AssertionError):
            evidence.authenticate_current_checkpoint(self.roles)
        for path in evidence.CURRENT_CHANGED:
            bad = copy.deepcopy(freeze); bad['after_format'][path] = bad['before'][path]
            with self.assertRaises(AssertionError):
                evidence.validate_transition(self.roles, bad)
        for path, old in ((evidence.MEMORY, evidence.MEMORY_BEFORE), (evidence.READER, evidence.READER_BEFORE)):
            bad = copy.deepcopy(freeze); bad['after_format'][path] = old
            with self.assertRaises(AssertionError):
                evidence.validate_transition(self.roles, bad)
            bad_roles = copy.deepcopy(self.roles); bad_roles['current_before'][path] = old
            with self.assertRaises(AssertionError):
                evidence.validate_roles(bad_roles)
        for key in ('known_validation_sources_before', 'current_before', 'existing_current_only_sources'):
            bad_roles = copy.deepcopy(self.roles); bad_roles[key][evidence.LOADING_TEST] = '0' * 64
            with self.assertRaises(AssertionError):
                evidence.validate_roles(bad_roles)
        bad_roles = copy.deepcopy(self.roles); bad_roles['current_before'][evidence.LOADING] = '0' * 64
        with self.assertRaises(AssertionError):
            evidence.validate_roles(bad_roles)
        after = freeze['after_format']
        callers_path = ROOT / 'caller-source-pins.json'; callers = evidence._current_callers(self.roles, ROOT, after)
        for row, field, value in [('1', 'sha256', '0' * 64), ('2', 'path', callers['1']['path']),
                                  ('2', 'sha256', evidence.MEMORY_BEFORE), ('2', 'bytes', callers['2']['bytes'] + 1)]:
            bad = copy.deepcopy(callers); bad[row][field] = value
            with self.replaced_record(callers_path, bad), self.assertRaises(AssertionError):
                evidence._current_callers(self.roles, ROOT, after)
        selectors_path = ROOT / 'core-selectors.json'; evidence._current_selectors(self.roles, ROOT)
        for index in (0, 52):
            bad = evidence.read(selectors_path); bad['ordinary'][index] = 'invented::selector'
            with self.replaced_record(selectors_path, bad), self.assertRaises(AssertionError):
                evidence._current_selectors(self.roles, ROOT)
        loading_path = ROOT / 'loading-selectors.json'
        evidence._current_loading_selectors(self.roles, ROOT)
        for index in (0, 29):
            bad = evidence.read(loading_path); bad['ordinary'][index] = 'invented::selector'
            with self.replaced_record(loading_path, bad), self.assertRaises(AssertionError):
                evidence._current_loading_selectors(self.roles, ROOT)

    def test_fresh_compile_profile_features_and_old_artifact_refuse(self):
        evidence.authenticate_current_compile(self.roles)
        paths = [ROOT / 'slug_core_v2-executable.json', ROOT / 'artifact-binding.json']
        old = evidence.read(Path(self.roles['prior']) / 'slug_core_v2-executable.json')
        with self.replaced_record(paths[0], old), self.assertRaises(AssertionError):
            evidence.authenticate_current_compile(self.roles)
        binding = evidence.read(paths[1])
        for field, value in [('features', ['default']), ('profile', {**binding['cargo_artifact']['profile'], 'opt_level': '0'})]:
            bad = copy.deepcopy(binding); bad['cargo_artifact'][field] = value
            with self.replaced_record(paths[1], bad), self.assertRaises(AssertionError):
                evidence.authenticate_current_compile(self.roles)
        bad = copy.deepcopy(binding); bad['checkpoint_sha256'] = '0' * 64
        with self.replaced_record(paths[1], bad), self.assertRaises(AssertionError):
            evidence.authenticate_current_compile(self.roles)
        # Core artifact staging does not read the not-yet-published Loading binding.
        original_read = evidence.read
        def without_loading_binding(path):
            assert Path(path) != ROOT / 'loading-artifact-binding.json'
            return original_read(path)
        with mock.patch.object(evidence, 'read', side_effect=without_loading_binding):
            evidence.authenticate_current_compile(self.roles, include_production=False)
        evidence._authenticate_current_loading(self.roles, include_tests=False)
        loading_descriptor = ROOT / 'slug_loading_v2-executable.json'
        old_loading = evidence.read(Path(evidence.ROOTS['L']) / 'slug_loading_v2-executable.json')
        with self.replaced_record(loading_descriptor, old_loading), self.assertRaises(AssertionError):
            evidence._authenticate_current_loading(self.roles, include_tests=False)
        loading_binding = ROOT / 'loading-artifact-binding.json'
        for field, value in [('features', ['default']), ('profile', {**evidence.read(loading_binding)['cargo_artifact']['profile'], 'opt_level': '1'})]:
            bad = evidence.read(loading_binding); bad['cargo_artifact'][field] = value
            with self.replaced_record(loading_binding, bad), self.assertRaises(AssertionError):
                evidence._authenticate_current_loading(self.roles, include_tests=False)
        bad = evidence.read(loading_binding); bad['checkpoint_sha256'] = '0' * 64
        with self.replaced_record(loading_binding, bad), self.assertRaises(AssertionError):
            evidence._authenticate_current_loading(self.roles, include_tests=False)

    def test_historical_complete_map_and_protected_artifact_stay_strict(self):
        freeze = evidence.read(ROOT / 'source-freeze.json')
        gate = dict(root=str(evidence.HISTORY), stem='compile-core', count=0, kind='current-core', supervisor=str(evidence.HISTORY / 'run_gate.py'))
        path = evidence.HISTORY / 'compile-core/operation.json'; bad = evidence.read(path)
        missing = evidence.CHANGED[0]
        del bad['pins_before'][missing]; del bad['pins_after'][missing]
        with self.replaced_record(path, bad), self.assertRaises(AssertionError):
            evidence.authenticate_operation(self.roles, gate, freeze)
        bad = evidence.read(path)
        bad['pins_before'][evidence.LOADING] = bad['pins_after'][evidence.LOADING] = freeze['after_format'][evidence.LOADING]
        with self.replaced_record(path, bad), self.assertRaises(AssertionError):
            evidence.authenticate_operation(self.roles, gate, freeze)
        bad = evidence.read(path)
        for p in evidence.CHANGED + [evidence.MEMORY, evidence.LOADING]:
            bad['pins_before'][p] = bad['pins_after'][p] = self.roles['current_before'][p]
        with self.replaced_record(path, bad), self.assertRaises(AssertionError):
            evidence.authenticate_operation(self.roles, gate, freeze)
        original = evidence.digest
        with mock.patch.object(evidence, 'digest', side_effect=lambda p: '0' * 64 if Path(p) == evidence.COMPILE_FREEZE else original(p)), self.assertRaises(AssertionError):
            evidence.historical_compile_sources(self.roles)
        cp_path = ROOT / self.roles['checkpoint_name']
        for required in (evidence.COMPILE_FREEZE, evidence.READER_FREEZE):
            bad = evidence.read(cp_path); del bad['source_pins'][str(required)]
            with self.replaced_record(cp_path, bad), self.assertRaises((AssertionError, KeyError)):
                evidence.authenticate_current_checkpoint(self.roles)
        with mock.patch.object(evidence, 'digest', side_effect=lambda p: '0' * 64 if Path(p) == evidence.READER_FREEZE else original(p)), self.assertRaises(AssertionError):
            evidence.historical_compile_sources(self.roles)
        for field, mapping in [('known_validation_sources_before', {evidence.READER: evidence.READER_CURRENT}),
                               ('new_gate_sources', {evidence.READER: evidence.READER_BEFORE})]:
            bad = evidence.read(evidence.READER_FREEZE); bad[field] = mapping
            with self.replaced_record(evidence.READER_FREEZE, bad), self.assertRaises(AssertionError):
                evidence.authenticate_current_checkpoint(self.roles)
        bad = evidence.read(evidence.COMPILE_FREEZE); bad['after_format'][evidence.MEMORY] = evidence.MEMORY_CURRENT
        with self.replaced_record(evidence.COMPILE_FREEZE, bad), self.assertRaises(AssertionError):
            evidence.historical_compile_sources(self.roles)
        bad = evidence.read(evidence.COMPILE_FREEZE); bad['after_format'][evidence.LOADING] = freeze['after_format'][evidence.LOADING]
        with self.replaced_record(evidence.COMPILE_FREEZE, bad), self.assertRaises(AssertionError):
            evidence.historical_compile_sources(self.roles)
        prior = Path(self.roles['last_actual_native_root'])
        native_path = prior / 'native/operation.json'
        evidence.authenticate_last_native(self.roles, freeze['after_format'])
        for p in (evidence.CHANGED[0], str(Path(self.roles['supervisor_path']).resolve())):
            bad = evidence.read(native_path)
            del bad['pins_before'][p]; del bad['pins_after'][p]
            with self.replaced_record(native_path, bad), self.assertRaises(AssertionError):
                evidence.authenticate_last_native(self.roles, freeze['after_format'])
        for record, field, p in [(prior / self.roles['checkpoint_name'], 'source_pins', 'scripts/v2_probe_evidence.py'),
                                 (prior / 'source-freeze.json', 'helper_pins', 'scripts/v2_probe_evidence_tests.py')]:
            bad = evidence.read(record); bad[field][p] = '0' * 64
            with self.replaced_record(record, bad), self.assertRaises(AssertionError):
                evidence.authenticate_last_native(self.roles, freeze['after_format'])
        # No arbitrary third source/helper exception, even when the recorded maps agree.
        third = 'scripts/v2_test_preflight.py'; bad = evidence.read(native_path)
        bad['pins_before'][third] = bad['pins_after'][third] = '0' * 64
        cp = evidence.read(prior / self.roles['checkpoint_name']); cp['source_pins'][third] = '0' * 64
        original_read = evidence.read
        replacements = {native_path: bad, prior / self.roles['checkpoint_name']: cp}
        with mock.patch.object(evidence, 'read', side_effect=lambda p: copy.deepcopy(replacements[Path(p)]) if Path(p) in replacements else original_read(p)), self.assertRaises(AssertionError):
            evidence.authenticate_last_native(self.roles, freeze['after_format'])
        bad = evidence.read(prior / 'slug_core_v2-executable.json'); bad['sha256'] = '0' * 64
        with self.replaced_record(prior / 'slug_core_v2-executable.json', bad), self.assertRaises(AssertionError):
            evidence.authenticate_last_native(self.roles, freeze['after_format'])
        receipt_path = Path(self.roles['prior_receipt'])
        for field, value in [('verdict', 'REJECT'), ('baseline', self.roles['baseline']),
                             ('contract_sha256', self.roles['contract_sha256']), ('source_pins', {})]:
            bad = evidence.read(receipt_path); bad[field] = value
            with self.replaced_record(receipt_path, bad), self.assertRaises(AssertionError):
                evidence.authenticate_last_native(self.roles, freeze['after_format'])
        bad = evidence.read(receipt_path); bad['native']['operation_sha256'] = '0' * 64
        with self.replaced_record(receipt_path, bad), self.assertRaises(AssertionError):
            evidence.authenticate_last_native(self.roles, freeze['after_format'])
        bad_roles = copy.deepcopy(self.roles); bad_roles['immediate_pins'][str(receipt_path)] = '0' * 64
        with self.assertRaises(AssertionError):
            evidence.authenticate_last_native(bad_roles, freeze['after_format'])
        config_path = prior / 'native-config.json'; config = evidence.read(config_path)
        for extra in (config['pins'][0], str(config_path), str(Path(self.roles['supervisor_path']).resolve())):
            bad = copy.deepcopy(config); bad['pins'].append(extra)
            with self.replaced_record(config_path, bad), self.assertRaises(AssertionError):
                evidence.authenticate_last_native(self.roles, freeze['after_format'])
        gate = next(g for g in self.roles['gates'] if g['kind'] == 'loading' and g['stem'] == 'loading-tests')
        artifact = evidence.read(Path(evidence.ROOTS['L']) / 'slug_loading_v2-executable.json')
        original = evidence.digest
        # The old Loading executable is metadata only, even if that physical path is replaced.
        with mock.patch.object(evidence, 'digest', side_effect=lambda p: '0' * 64 if str(p) == artifact['path'] else original(p)):
            evidence.authenticate_operation(self.roles, gate, freeze)
            evidence.authenticate_last_native(self.roles, freeze['after_format'])
        fresh = evidence.read(ROOT / 'slug_loading_v2-executable.json')
        with mock.patch.object(evidence, 'digest', side_effect=lambda p: '0' * 64 if str(p) == fresh['path'] else original(p)), self.assertRaises(AssertionError):
            evidence._authenticate_current_loading(self.roles, include_tests=False)
        operation_path = Path(evidence.ROOTS['L']) / 'loading-tests/operation.json'
        bad = evidence.read(operation_path); bad['pins_before'][evidence.LOADING] = bad['pins_after'][evidence.LOADING] = freeze['after_format'][evidence.LOADING]
        with self.replaced_record(operation_path, bad), self.assertRaises(AssertionError):
            evidence.authenticate_operation(self.roles, gate, freeze)
        protected = evidence.read(Path(evidence.ROOTS['Q']) / 'slug_bzlmod_v2-executable.json')
        prototype = evidence.read(ROOT / 'native-policy-prototype.json')
        with mock.patch.object(evidence, 'digest', side_effect=lambda p: '0' * 64 if str(p) == protected['path'] else original(p)), self.assertRaises(AssertionError):
            evidence.native_policy_equivalent(self.roles, prototype)
        # Named physical-source replacements never expand to an unrelated source.
        third = next(p for p in self.roles['current_before'] if p not in evidence.CURRENT_CHANGED and p not in self.roles['loading_private_before'])
        with mock.patch.object(evidence, 'digest', side_effect=lambda p: '0' * 64 if str(p) == third else original(p)), self.assertRaises(AssertionError):
            evidence.authenticate_operation(self.roles, gate, freeze)
        bad_roles = copy.deepcopy(self.roles); bad_roles['historical_pins']['target/wp1133/native/operation.json'] = '0' * 64
        with self.assertRaises(AssertionError):
            evidence.validate_roles(bad_roles)


class SupervisorControls(unittest.TestCase):
    def setUp(self):
        self.scratch = tempfile.TemporaryDirectory(prefix='owned-control-', dir=ROOT)
        self.addCleanup(self.scratch.cleanup)
        self.root = Path(self.scratch.name)
        self.program = self.root / 'child.py'
        self.program.write_text("print('owned local child')\n")
        self.executable = str(Path(sys.executable).resolve())
        self.roles = dict(root=str(self.root), supervisor_path=SHARED[0],
                          baseline='owned fixture', contract_sha256='owned fixture')
        dump(self.root / 'roles.json', self.roles)
        self.checkpoint = dict(verdict='ACCEPT', baseline=self.roles['baseline'],
                               contract_sha256=self.roles['contract_sha256'], source_pins={})
        self.config = dict(mode='list', stem='child', command=[self.executable, str(self.program)],
                           environment={'RUST_BACKTRACE': '0'}, executable=self.executable,
                           executable_sha256=supervisor.digest(self.executable), pins=[])
        self.seal()

    def seal(self):
        dump(self.root / 'roles.json', self.roles)
        pins = {path: supervisor.digest(path) for path in SHARED}
        pins[str(self.root / 'roles.json')] = supervisor.digest(self.root / 'roles.json')
        pins[str(self.program)] = supervisor.digest(self.program)
        self.checkpoint['source_pins'] = pins
        dump(self.root / 'root-source-checkpoint.json', self.checkpoint)
        dump(self.root / 'source-freeze.json', dict(helper_pins=pins))
        self.config['pins'] = list(pins) + [str(self.root / 'root-source-checkpoint.json'),
                                          str(self.root / 'source-freeze.json'), self.executable]
        self.config_path = self.root / 'config.json'
        dump(self.config_path, self.config)

    def reject(self):
        dump(self.config_path, self.config)
        with self.assertRaises((AssertionError, KeyError, ValueError)):
            supervisor.validate_config(self.config_path)
        self.assertFalse((self.root / self.config['stem']).exists())

    def test_aliases_preserve_spelling_and_reject_conflicting_digest(self):
        alias = str(Path(SHARED[0]).absolute())
        self.checkpoint['source_pins'][alias] = supervisor.digest(alias)
        self.config['pins'].append(alias)
        dump(self.root / 'root-source-checkpoint.json', self.checkpoint)
        dump(self.config_path, self.config)
        actual = supervisor.validate_config(self.config_path)[3]
        self.assertEqual(actual[alias], actual[SHARED[0]])
        self.checkpoint['source_pins'][alias] = '0' * 64
        dump(self.root / 'root-source-checkpoint.json', self.checkpoint)
        self.reject()

    def test_required_source_and_checkpoint_pins(self):
        for path in (SHARED[1], str(self.root / 'root-source-checkpoint.json')):
            original = list(self.config['pins'])
            self.config['pins'].remove(path)
            self.reject()
            self.config['pins'] = original

    def test_policy_mode_environment_count_and_root_refuse_before_dispatch(self):
        original = copy.deepcopy(self.config)
        for changes in ({'mode': 'unknown'}, {'rss_bytes': 1 << 30},
                        {'preparation_seconds': 30}, {'expected_count': 0},
                        {'expected_count': True}, {'environment': {'RUST_BACKTRACE': '0',
                                                                   'SLUG_NATIVE_TEST': '1'}},
                        {'native_output_bytes': 64 << 20}):
            self.config = {**copy.deepcopy(original), **changes}
            self.reject()
        self.config = original
        self.roles['root'] = str(self.root / 'other')
        self.seal()
        self.reject()

    def test_real_join_and_current_before_after_pins(self):
        supervisor.main(self.config_path)
        operation = json.loads((self.root / 'child/operation.json').read_text())
        self.assertEqual(operation['exit_code'], 0)
        self.assertIsNone(operation['stop_reason'])
        self.assertTrue(operation['joined'] and operation['owned_group_empty'])
        self.assertEqual(operation['pins_before'], operation['pins_after'])
        self.assertEqual(operation['pass_count'], 0)  # This fixture grants no Rust credit.

    def held_source(self, mutate):
        source = self.root / 'source.txt'
        source.write_text('owned small source\n')
        self.roles['source_owner_pins'] = {path: supervisor.digest(path) for path in
                                          ('target/wp1117/fd_source_r7.py', 'target/wp933/read.py')}
        self.config['sources'] = [dict(path=str(source.absolute()), bytes=source.stat().st_size,
                                       sha256=supervisor.digest(source))]
        if mutate:
            self.program.write_text("from pathlib import Path\nPath(" + repr(str(source)) +
                                    ").write_text('owned changed data\\n')\n")
        self.seal()
        self.config['pins'] += list(self.roles['source_owner_pins'])
        dump(self.config_path, self.config)
        if mutate:
            with self.assertRaises(AssertionError):
                supervisor.main(self.config_path)
        else:
            supervisor.main(self.config_path)
        operation = json.loads((self.root / 'child/operation.json').read_text())
        self.assertTrue(operation['joined'] and operation['owned_group_empty'])
        self.assertEqual(operation['pass_count'], 0)
        self.assertFalse(operation['source_bindings'][0]['fd_inherited'])
        if mutate:
            self.assertIsNotNone(operation['stop_reason'])
            self.assertIsNone(operation['source_bindings'][0]['after'])
        else:
            self.assertIsNone(operation['stop_reason'])
            self.assertEqual(operation['source_bindings'][0]['before'],
                             operation['source_bindings'][0]['after'])

    def test_owned_held_fd_before_after(self):
        self.held_source(False)

    def test_owned_held_fd_change_refuses_after_join(self):
        self.held_source(True)


if __name__ == '__main__':
    ROOT = control_root(sys.argv[1:])
    started = time.monotonic()
    result = unittest.TextTestRunner(verbosity=2).run(unittest.defaultTestLoader.loadTestsFromModule(sys.modules[__name__]))
    record = dict(selected=result.testsRun, passed=result.testsRun - len(result.errors) - len(result.failures),
                  errors=len(result.errors), failures=len(result.failures), elapsed_seconds=time.monotonic() - started,
                  scope='owned local fixtures and metadata; no native/protected Rust credit')
    with (ROOT / 'helper-controls.json').open('x') as stream:
        stream.write(json.dumps(record, indent=2) + '\n')
    raise SystemExit(0 if result.wasSuccessful() else 1)
