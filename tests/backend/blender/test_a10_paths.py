"""Read-only path resolution cases for the A10 development fixture."""

from pathlib import Path
import tempfile
import unittest

import three_variant_proof as proof


class A10PathTests(unittest.TestCase):
    def test_normal_checkout_and_worktree_share_workspace_tools(self):
        workspace = Path('/home/example/archy')
        common = workspace / 'repository/.git'
        for root in (workspace / 'repository', workspace / '.worktrees/proof'):
            with self.subTest(root=root):
                paths = proof.resolve_fixture_paths(root, common, {},
                    is_dir=lambda path: path == workspace / '.tools')
                self.assertEqual(paths.blender, workspace / '.tools/blender-4.5.15-linux-x64/blender')
                self.assertEqual(paths.evidence, workspace / '.tools/targets/a10-three-variant-proof')
                self.assertEqual(paths.cli, root / 'target/debug/archy')

    def test_local_tools_win_and_default_without_existing_tools(self):
        root = Path('/home/example/archy')
        for exists in (lambda path: path == root / '.tools', lambda path: False):
            paths = proof.resolve_fixture_paths(root, root / '.git', {}, is_dir=exists)
            self.assertEqual(paths.blender, root / '.tools/blender-4.5.15-linux-x64/blender')

    def test_explicit_absolute_test_overrides(self):
        root = Path('/home/example/repo')
        env = {'ARCHY_A10_TOOLS_DIR': '/ssd/tools', 'ARCHY_A10_EVIDENCE_DIR': '/ssd/evidence',
               'ARCHY_A10_CLI': '/build/archy'}
        paths = proof.resolve_fixture_paths(root, root / '.git', env, is_dir=lambda _: False)
        self.assertEqual(paths.blender, Path('/ssd/tools/blender-4.5.15-linux-x64/blender'))
        self.assertEqual(paths.evidence, Path('/ssd/evidence'))
        self.assertEqual(paths.cli, Path('/build/archy'))

    def test_reject_relative_or_empty_override(self):
        for key in ('ARCHY_A10_TOOLS_DIR', 'ARCHY_A10_EVIDENCE_DIR', 'ARCHY_A10_CLI'):
            for value in ('relative/path', ''):
                with self.subTest(key=key, value=value), self.assertRaisesRegex(ValueError, key):
                    proof.resolve_fixture_paths(Path('/repo'), Path('/repo/.git'), {key: value}, is_dir=lambda _: False)

    def test_evidence_root_symlink_is_refused_without_touching_target(self):
        with tempfile.TemporaryDirectory(prefix='a10-path-test-') as tmp:
            root = Path(tmp)
            target = root / 'target'
            target.mkdir()
            sentinel = target / 'sentinel'
            sentinel.write_text('unchanged')
            link = root / 'evidence'
            link.symlink_to(target, target_is_directory=True)
            with self.assertRaisesRegex(ValueError, 'evidence root.*symlink'):
                proof.create_evidence_slot(link, 0)
            self.assertEqual(sentinel.read_text(), 'unchanged')
            self.assertEqual(sorted(p.name for p in target.iterdir()), ['sentinel'])
            dangling = root / 'dangling'
            dangling.symlink_to(root / 'missing', target_is_directory=True)
            with self.assertRaisesRegex(ValueError, 'evidence root.*symlink'):
                proof.create_evidence_slot(dangling, 0)
            self.assertFalse((root / 'missing').exists())

    def test_real_evidence_directory_and_occupied_slot(self):
        with tempfile.TemporaryDirectory(prefix='a10-path-test-') as tmp:
            evidence = Path(tmp) / 'evidence'
            evidence.mkdir()
            slot = proof.create_evidence_slot(evidence, 0)
            sentinel = slot / 'sentinel'
            sentinel.write_text('unchanged')
            with self.assertRaises(FileExistsError):
                proof.create_evidence_slot(evidence, 0)
            self.assertEqual(sentinel.read_text(), 'unchanged')
            (evidence / 'slot-1').symlink_to(slot, target_is_directory=True)
            with self.assertRaises(FileExistsError):
                proof.create_evidence_slot(evidence, 1)
            self.assertEqual(sentinel.read_text(), 'unchanged')
            self.assertTrue(proof.create_evidence_slot(Path(tmp) / 'new-evidence', 2).is_dir())


if __name__ == '__main__':
    unittest.main()
