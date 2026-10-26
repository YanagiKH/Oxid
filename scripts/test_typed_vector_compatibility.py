"""Independent mutation controls for the typed-vector source boundary.

These Python controls authenticate source admission and replay wiring. They do
not execute the retained Rust assertions or qualify vector language semantics.
"""
import ast
import builtins
import hashlib
import inspect
import io
import json
import os
from pathlib import Path
import symtable
import tempfile
from contextlib import redirect_stderr
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import verify_typed_vector_compatibility as gate

ROOT = Path(__file__).resolve().parents[1]
DISPATCH = 'tests/typed_project_dispatch.rs'
DISPATCH_PIN = (5888, 'b24162962f6149422ebe38f58f6fe05d16de71ea06d81aa809df6b979035666d')


def baseline_names():
    body = (ROOT / gate.BINDING / 'current-source.json').read_bytes()
    if gate.sha(body) != gate.BASELINE_SOURCE_SHA:
        raise AssertionError('historical compiler membership changed')
    return {row['path'] for row in json.loads(body)['files']}


class TypedVectorAdmission(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.baseline = {name: b'// frozen baseline input\n' for name in baseline_names()}
        self.parent = {**self.baseline,
                       **{name: b'// retained predecessor addition\n' for name in gate.PREDECESSOR_ADDED}}
        self.parent[DISPATCH] = (ROOT / DISPATCH).read_bytes()
        self.current = dict(self.parent)
        self.current.update({name: b'// reviewed typed vector input\n'
                             for name in gate.TYPED_VECTOR_CHANGED | gate.TYPED_VECTOR_ADDED})
        for name, body in self.current.items():
            self.put(name, body)
        inputs = {name: (len(self.current[name]), gate.sha(self.current[name]))
                  for name in gate.TYPED_VECTOR_CHANGED | gate.TYPED_VECTOR_ADDED}
        directives = {name: gate.source_directives(name, self.current[name]) for name in inputs}
        for name, value in [('TYPED_VECTOR_INPUTS', inputs), ('REVIEWED_DIRECTIVES', directives)]:
            mocked = patch.object(gate, name, value)
            mocked.start()
            self.addCleanup(mocked.stop)

    def put(self, name, body):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(body)

    def capture(self):
        return gate.capture_sources(self.root, self.baseline, self.parent)

    def test_exact_successor_admits_all_473_parent_inputs_and_only_listed_additions(self):
        self.assertEqual(len(self.parent), 473)
        captured = self.capture()
        self.assertEqual(set(captured), set(self.parent) | gate.TYPED_VECTOR_ADDED)
        self.assertEqual(len(captured), 473 + len(gate.TYPED_VECTOR_ADDED))
        for name in set(self.parent) - gate.TYPED_VECTOR_CHANGED:
            self.assertEqual(captured[name], self.parent[name])
        manifest = gate.current_manifest(captured, self.baseline, {'source_kind': 'worktree'}, self.parent)
        self.assertEqual(manifest['predecessor_compiler_input_members'], 473)
        self.assertEqual(manifest['source_policy_input_members'], 48)
        self.assertEqual({row['path'] for row in manifest['files']}, set(captured))
        self.assertEqual({row['path'] for row in manifest['typed_vector_changes']}, gate.TYPED_VECTOR_CHANGED)
        for key, value in manifest.items():
            if key.endswith('qualified_by_this_gate'):
                self.assertIs(value, False, key)
        self.assertIs(manifest['typed_vectors_semantics_qualified_by_this_gate'], False)
        self.assertIs(manifest['typed_scalar_ranges_semantics_qualified_by_this_gate'], False)
        self.assertIs(manifest['macos_native_semantics_qualified_by_this_gate'], False)

    def test_every_unchanged_parent_input_remains_closed(self):
        for name in sorted(set(self.parent) - gate.TYPED_VECTOR_CHANGED):
            with self.subTest(path=name):
                self.put(name, self.parent[name] + b'// unreviewed\n')
                with self.assertRaisesRegex(ValueError, 'outside typed-vector boundary|protected retained input'):
                    self.capture()
                self.put(name, self.parent[name])

    def test_actual_unchanged_dispatch_bytes_are_pinned_independently(self):
        self.assertEqual(gate.UNCHANGED_INPUTS[DISPATCH], DISPATCH_PIN)
        actual = (ROOT / DISPATCH).read_bytes()
        self.assertEqual((len(actual), gate.sha(actual)), DISPATCH_PIN)
        self.assertNotIn(DISPATCH, gate.TYPED_VECTOR_CHANGED | gate.TYPED_VECTOR_ADDED)
        self.put(DISPATCH, actual + b'// changed retained assertion\n')
        with self.assertRaisesRegex(ValueError, 'protected retained input'):
            self.capture()
        self.parent[DISPATCH] = actual + b'// coherently forged parent\n'
        self.put(DISPATCH, self.parent[DISPATCH])
        with self.assertRaisesRegex(ValueError, 'protected retained input'):
            self.capture()
        with patch.object(gate, 'TYPED_VECTOR_CHANGED', gate.TYPED_VECTOR_CHANGED | {DISPATCH}):
            with self.assertRaisesRegex(ValueError, 'protected input cannot enter'):
                self.capture()

    def test_reviewed_length_hash_and_roster_are_required(self):
        name = sorted(gate.TYPED_VECTOR_CHANGED)[0]
        original = gate.TYPED_VECTOR_INPUTS[name]
        for pin in [(original[0] + 1, original[1]), (original[0], '0' * 64)]:
            gate.TYPED_VECTOR_INPUTS[name] = pin
            with self.assertRaisesRegex(ValueError, 'reviewed input'):
                self.capture()
        gate.TYPED_VECTOR_INPUTS[name] = original
        del gate.TYPED_VECTOR_INPUTS[name]
        with self.assertRaisesRegex(ValueError, 'reviewed typed-vector input membership'):
            self.capture()
        gate.TYPED_VECTOR_INPUTS[name] = original
        gate.TYPED_VECTOR_INPUTS['src/unlisted.rs'] = original
        with self.assertRaisesRegex(ValueError, 'reviewed typed-vector input membership'):
            self.capture()

    def test_extra_missing_and_empty_directory_members_are_refused(self):
        self.put('native/unlisted.h', b'/* unreviewed */\n')
        with self.assertRaisesRegex(ValueError, 'membership'):
            self.capture()
        (self.root / 'native/unlisted.h').unlink()
        name = sorted(gate.TYPED_VECTOR_ADDED)[0]
        (self.root / name).unlink()
        with self.assertRaisesRegex(ValueError, 'membership'):
            self.capture()
        self.put(name, self.current[name])
        (self.root / 'src/unlisted-empty').mkdir()
        with self.assertRaisesRegex(ValueError, 'directory'):
            self.capture()

    def test_parent_membership_cannot_shrink_and_reviewed_changes_cannot_revert(self):
        name = sorted(gate.PREDECESSOR_ADDED)[0]
        del self.parent[name]
        with self.assertRaisesRegex(ValueError, 'predecessor compiler input membership'):
            self.capture()
        self.parent[name] = self.current[name]
        name = sorted(gate.TYPED_VECTOR_CHANGED)[0]
        self.put(name, self.parent[name])
        with self.assertRaisesRegex(ValueError, 'reviewed input'):
            self.capture()

    def test_coherent_hash_changes_cannot_add_any_selector_or_asset(self):
        name = 'src/frontend/oir/owned/plan.rs'
        mutations = [b'const X:&str=include_str!("/outside");\n',
                     b'#[cfg(unreviewed)]\nfn conditional() {}\n',
                     b'#\n [cfg_attr(\n feature="unreviewed", path="elsewhere.rs"\n)]\nmod alternate;\n',
                     b'const X:&str=env!("UNREVIEWED_ASSET");\n',
                     b'#include "/outside.h"\n',
                     b'#define HIDDEN \\\n include_file\n']
        for change in mutations:
            with self.subTest(change=change):
                body = self.current[name] + change
                self.put(name, body)
                gate.TYPED_VECTOR_INPUTS[name] = (len(body), gate.sha(body))
                with self.assertRaisesRegex(ValueError, 'compile-time input directive'):
                    self.capture()
        self.put(name, self.current[name])

    def test_directive_roster_cannot_omit_or_add_reviewed_members(self):
        name = sorted(gate.REVIEWED_DIRECTIVES)[0]
        value = gate.REVIEWED_DIRECTIVES.pop(name)
        with self.assertRaisesRegex(ValueError, 'directive membership'):
            self.capture()
        gate.REVIEWED_DIRECTIVES[name] = value
        gate.REVIEWED_DIRECTIVES['src/unlisted.rs'] = value
        with self.assertRaisesRegex(ValueError, 'directive membership'):
            self.capture()

    @unittest.skipIf(os.name == 'nt', 'Unix source link and executable mode checks')
    def test_links_executables_and_oversized_sources_are_rejected(self):
        name = 'src/frontend/oir/owned/plan.rs'
        path = self.root / name
        outside = self.root / 'outside'
        outside.write_bytes(self.current[name])
        path.unlink()
        path.symlink_to(outside)
        with self.assertRaisesRegex(ValueError, 'symlink'):
            self.capture()
        path.unlink()
        os.link(outside, path)
        with self.assertRaisesRegex(ValueError, 'nonregular'):
            self.capture()
        path.unlink()
        self.put(name, self.current[name])
        path.chmod(0o755)
        with self.assertRaisesRegex(ValueError, 'nonregular'):
            self.capture()
        path.chmod(0o644)
        self.put(name, b'x' * (2 * 1024 * 1024 + 1))
        with self.assertRaisesRegex(ValueError, 'oversized'):
            self.capture()


class TypedVectorAuthority(unittest.TestCase):
    def test_predecessor_executes_only_after_its_actual_hash_and_length_are_checked(self):
        expected = (ROOT / gate.PREDECESSOR_POLICY).read_bytes()
        self.assertEqual((len(expected), gate.sha(expected)),
                         (35467, 'f23fa37e76036e633ff56a9902bf01efd4682750f2d71f2dded88a757d06ba75'))
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path = root / gate.PREDECESSOR_POLICY
            path.parent.mkdir(parents=True)
            for body in [expected + b'\n', b'raise RuntimeError("untrusted policy executed")\n']:
                path.write_bytes(body)
                with self.assertRaisesRegex(ValueError, 'changed pinned macOS native'):
                    gate.load_predecessor(root)

    def test_all_48_predecessor_policy_files_are_unchanged_and_authenticated(self):
        self.assertEqual(len(gate.PREDECESSOR_POLICIES), 48)
        self.assertEqual(gate.PREDECESSOR_POLICIES,
                         {**gate.library.PREDECESSOR_POLICIES,
                          gate.PREDECESSOR_POLICY: gate.PREDECESSOR_POLICY_SHA})
        with tempfile.TemporaryDirectory() as directory:
            roots = [Path(directory) / 'repo', Path(directory) / 'parent']
            for name, digest in gate.PREDECESSOR_POLICIES.items():
                body = (ROOT / name).read_bytes()
                self.assertEqual(gate.sha(body), digest)
                for root in roots:
                    path = root / name
                    path.parent.mkdir(parents=True, exist_ok=True)
                    path.write_bytes(body)
            gate.authenticate_policies(*roots)
            for name in sorted(gate.PREDECESSOR_POLICIES):
                body = (ROOT / name).read_bytes()
                for index, message in [(0, 'source policy changed'), (1, 'wrong pinned predecessor')]:
                    with self.subTest(path=name, copy=index):
                        (roots[index] / name).write_bytes(body + b'\n# altered\n')
                        with self.assertRaisesRegex(ValueError, message):
                            gate.authenticate_policies(*roots)
                        (roots[index] / name).write_bytes(body)

    def test_replay_helpers_selectors_and_observer_controls_use_the_actual_parent_chain(self):
        for name in ['extract_baseline', 'checkout_identity',
                     'verify_materialized', 'verify_current_observer',
                     'run_byte_buffer_observer_controls', 'run_file_observer_controls',
                     'run_i64_observer_controls', 'run_integer_width_observer_controls',
                     'run_integer128_observer_controls', 'run_float_observer_controls',
                     'run_character_observer_controls']:
            self.assertIs(getattr(gate, name), getattr(gate.library, name), name)
        for name in ['BASELINE', 'BASELINE_SOURCE_SHA', 'RETAINED_RUNNER_SHA', 'BINDING', 'RETAINED']:
            self.assertEqual(getattr(gate, name), getattr(gate.library, name), name)
        self.assertEqual(ast.dump(ast.parse(inspect.getsource(gate.regular))),
                         ast.dump(ast.parse(inspect.getsource(gate.library.regular))))
        self.assertIs(gate.directives, gate.library.directives)
        self.assertEqual(gate.PREDECESSOR_ADDED, gate.library.PARENT_ADDED)
        self.assertEqual(gate.PARENT_ADDED, gate.PREDECESSOR_ADDED | gate.TYPED_VECTOR_ADDED)
        self.assertEqual(gate.OBSERVER_ADAPTER, 'scripts/typed_vector_compatibility_observer.py')
        # Remove only the exact new closed-vector control call. Every prior
        # selector, statement, order and final recheck must still match Mac.
        def replay_block(function):
            tree = ast.parse(inspect.getsource(function))
            return [node for node in ast.walk(tree) if isinstance(node, ast.If)
                    and ast.dump(node.test) == ast.dump(ast.parse('not args.prepare_only', mode='eval').body)]
        current = replay_block(gate.run)
        previous = replay_block(gate.library.run)
        self.assertEqual((len(current), len(previous)), (1, 1))
        added = ast.parse('verified.update(run_vector_observer_controls(repo, output, selected, seam, original, observer_adapter, verified["test_function_executions"]))').body[0]
        indexes = [i for i, node in enumerate(current[0].body) if ast.dump(node) == ast.dump(added)]
        self.assertEqual(len(indexes), 1)
        index = indexes[0]
        preceding = ast.parse('verified.update(run_character_observer_controls(repo, output, selected, seam, original, observer_adapter))').body[0]
        self.assertGreater(index, 0)
        self.assertEqual(ast.dump(current[0].body[index - 1]), ast.dump(preceding))
        current[0].body.pop(index)
        self.assertEqual(ast.dump(current[0]), ast.dump(previous[0]))

    def test_parent_capture_requires_the_pinned_git_object_mode_length_and_bytes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path = root / 'src/a.rs'
            path.parent.mkdir()
            body = b'// immutable parent\n'
            path.write_bytes(body)
            digest = hashlib.sha1(b'blob '+str(len(body)).encode()+b'\0'+body,
                                  usedforsecurity=False).hexdigest().encode()
            row = b'100644 blob '+digest+b' '+str(len(body)).encode()+b'\tsrc/a.rs\0'
            with patch.object(gate, 'git', return_value=row) as read:
                self.assertEqual(gate.capture_parent(root, root, {'src/a.rs'}), {'src/a.rs': body})
                self.assertEqual(read.call_args.args[-1], gate.PREDECESSOR)
                path.write_bytes(body + b'// drift\n')
                with self.assertRaisesRegex(ValueError, 'pinned Git blob'):
                    gate.capture_parent(root, root, {'src/a.rs'})
                path.write_bytes(body)
            for bad in [row.replace(b'100644', b'100755'), row.replace(digest,b'0'*40),
                        row.replace(str(len(body)).encode()+b'\t',b'0\t')]:
                with patch.object(gate, 'git', return_value=bad):
                    with self.assertRaisesRegex(ValueError, 'pinned Git blob'):
                        gate.capture_parent(root, root, {'src/a.rs'})

    def test_complete_multiline_directives_and_build_script_are_retained(self):
        attribute = b'#[cfg_attr(\n feature="x", path="a]b.rs"\n)]'
        include = b'include_bytes!(concat!(env!("OUT_DIR"), "/asset"))'
        body = attribute+b'\nconst X:&[u8]='+include+b';\n'
        captured = gate.source_directives('src/a.rs', body)
        self.assertEqual(captured['attributes'], (attribute,))
        self.assertIn(include, captured['macros'])
        self.assertIn(b'env!("OUT_DIR")', captured['macros'])
        for truncated in [b'#[cfg_attr(test,', b'include_str!("asset"']:
            with self.assertRaisesRegex(ValueError, 'incomplete compile-time'):
                gate.source_directives('src/a.rs', truncated)
        c = b'#include "asset.h"\n#define BLOCK \\\n continued\n'
        self.assertEqual(gate.source_directives('native/a.c', c)['c_preprocessor'],
                         (b'#include "asset.h"\n', b'#define BLOCK \\\n continued\n'))
        build = b'fn main(){println!("cargo:rustc-link-lib=x");}\n'
        self.assertEqual(gate.source_directives('build.rs', build)['build_script'], build)

    def test_provisional_pins_cannot_be_misreported_as_a_qualified_compiler(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / 'proof'
            args = SimpleNamespace(repo=ROOT, output=output, prepare_only=False)
            with patch.object(gate, 'PINS_PROVISIONAL', True), \
                 patch.object(gate, 'checkout_identity') as checkout, \
                 patch.object(gate.subprocess, 'run') as execution, redirect_stderr(io.StringIO()):
                self.assertEqual(gate.run(args), 1)
            checkout.assert_not_called()
            execution.assert_not_called()
            receipt = json.loads((output / 'failure.json').read_text())
            self.assertIs(receipt['semantic_pass'], False)
            self.assertEqual(receipt['compiler_executions'], 0)
            self.assertIn('provisional', receipt['error'])
            self.assertFalse((output / 'result.json').exists())

    def test_prepare_only_enters_the_selected_controller_without_running_tools(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / 'proof'
            args = SimpleNamespace(repo=ROOT, output=output, prepare_only=True,
                                   expected_head='not-read', allow_dirty=False)
            with patch.object(gate, 'checkout_identity', side_effect=ValueError('checkout sentinel')) as checkout, \
                 patch.object(gate.subprocess, 'run') as execution, redirect_stderr(io.StringIO()):
                self.assertEqual(gate.run(args), 1)
            checkout.assert_called_once_with(ROOT, 'not-read', False)
            execution.assert_not_called()
            receipt = json.loads((output / 'failure.json').read_text())
            self.assertEqual(receipt['error'], 'ValueError: checkout sentinel')
            self.assertEqual(receipt['controller']['path'], 'scripts/verify_typed_vector_compatibility.py')

    def test_other_checkout_cannot_use_this_controller(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            output = root / 'proof'
            with self.assertRaisesRegex(ValueError, 'selected checkout'):
                gate.run(SimpleNamespace(repo=root, output=output))
            self.assertFalse(output.exists())

    def test_all_function_globals_are_bound(self):
        table = symtable.symtable(Path(gate.__file__).read_text(), gate.__file__, 'exec')
        available = set(vars(gate)) | set(vars(builtins))
        for child in table.get_children():
            if child.get_type() == 'function':
                self.assertFalse(set(child.get_globals()) - available, child.get_name())


if __name__ == '__main__':
    unittest.main()
