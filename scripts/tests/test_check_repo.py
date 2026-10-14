"""Negative cases use disposable copies and synthetic Git history only."""
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

SOURCE = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('check_repo', SOURCE / 'scripts/check-repo.py')
checker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(checker)


class RepositoryChecks(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='radishroot-governance-test-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name) / 'repo'
        files, _, _ = checker.inventory(SOURCE)
        for name in files:
            source = SOURCE / name
            if source.is_file() and not source.is_symlink():
                target = self.root / name
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(source, target)

    def errors(self, base=None):
        return checker.check_repository(self.root, base)[0]

    def mutate_json(self, path, change):
        p = self.root / path
        data = json.loads(p.read_text())
        change(data)
        p.write_text(json.dumps(data, ensure_ascii=False, indent=2) + '\n')

    def git(self, *args):
        env = dict(os.environ, GIT_CONFIG_NOSYSTEM='1', GIT_CONFIG_GLOBAL=os.devnull,
                   GIT_AUTHOR_NAME='Synthetic Test', GIT_AUTHOR_EMAIL='test@example.invalid',
                   GIT_COMMITTER_NAME='Synthetic Test', GIT_COMMITTER_EMAIL='test@example.invalid')
        result = subprocess.run(['git', '-C', str(self.root), *args], env=env,
                                capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        return result.stdout.strip()

    def test_baseline_without_git(self):
        self.assertEqual(self.errors(), [])
        self.assertIn('files only', checker.check_repository(self.root)[1])

    def test_missing_required_file(self):
        (self.root / 'SECURITY.md').unlink()
        self.assertTrue(any('missing required file' in e for e in self.errors()))

    def test_root_entry_drift(self):
        with (self.root / 'CLAUDE.md').open('a') as f:
            f.write('\nDrift\n')
        self.assertIn('AGENTS.md / CLAUDE.md differ', self.errors())

    def test_untracked_text_and_links(self):
        (self.root / 'new.md').write_bytes(b'Bad \r\n[broken](missing.md)\r\n')
        errors = self.errors()
        for expected in ('BOM/CR', 'trailing whitespace', 'missing link'):
            self.assertTrue(any(expected in e for e in errors), errors)

    def test_reference_links_and_escape(self):
        (self.root / 'new.md').write_text('[ref]: absent.md\n[escape](../../outside.md)\n')
        errors = self.errors()
        self.assertTrue(any('missing link' in e for e in errors))
        self.assertTrue(any('leaves repository' in e for e in errors))

    def test_ignored_outputs_and_symlinks(self):
        (self.root / '.env').write_bytes(b'private local content\xff')
        (self.root / 'runtime').mkdir()
        (self.root / 'runtime/disk.img').write_bytes(b'not a product image')
        self.assertEqual(self.errors(), [])
        (self.root / 'external.md').symlink_to('/nonexistent-radishroot-test')
        self.assertTrue(any('symlinks' in e for e in self.errors()))

    def test_ruleset_scope_and_bypass(self):
        def change(data):
            data['conditions']['ref_name']['include'] = ['~ALL']
            data['bypass_actors'][0]['bypass_mode'] = 'always'
        self.mutate_json('.github/rulesets/master-protection.json', change)
        errors = self.errors()
        self.assertIn('ruleset branch scope drift', errors)
        self.assertIn('ruleset bypass drift', errors)

    def test_format_size_and_ignore_policy(self):
        (self.root / 'invalid.json').write_text('{bad json}\n')
        (self.root / 'large.png').write_bytes(b'0' * (10 * 1024 * 1024 + 1))
        with (self.root / '.gitignore').open('a') as f:
            f.write('Cargo.lock\n')
        errors = self.errors()
        for expected in ('expected JSON', 'size limit', 'source/lockfile incorrectly ignored'):
            self.assertTrue(any(expected in e for e in errors), errors)

    def test_workflow_failure_bypass(self):
        def change(data):
            data['on']['push'] = {}
            data['jobs']['repo-hygiene']['steps'][1]['continue-on-error'] = True
            data['jobs']['candidate-quality']['needs'] = []
        self.mutate_json('.github/workflows/pr-check.yml', change)
        errors = self.errors()
        for expected in ('workflow triggers drift', 'step can skip/ignore failure', 'aggregate dependencies drift'):
            self.assertIn(expected, errors)

    def test_actual_aggregate_command_rejects_non_success(self):
        workflow = json.loads((self.root / '.github/workflows/pr-check.yml').read_text())
        command = workflow['jobs']['candidate-quality']['steps'][0]['run']
        cases = [({'repo-hygiene': {'result': 'success'}}, 0)]
        cases += [({'repo-hygiene': {'result': state}}, 1)
                  for state in ('failure', 'cancelled', 'skipped', 'neutral', '')]
        cases += [({}, 1), ({'repo-hygiene': {}}, 1),
                  ({'repo-hygiene': {'result': 'success'}, 'future': {'result': 'failure'}}, 1)]
        for needs, expected in cases:
            with self.subTest(needs=needs):
                result = subprocess.run(['bash', '-c', command], env=dict(os.environ, NEEDS_JSON=json.dumps(needs)),
                                        capture_output=True, text=True)
                self.assertEqual(result.returncode, expected, result.stderr)

    def test_base_requires_git(self):
        self.assertIn('--base-ref requires a Git repository', self.errors('HEAD'))

    def test_git_ranges_untracked_and_forced_outputs(self):
        self.git('init', '-b', 'dev')
        self.git('add', '.')
        self.git('-c', 'commit.gpgsign=false', 'commit', '-m', 'chore: synthetic baseline')
        base = self.git('rev-parse', 'HEAD')
        self.git('-c', 'commit.gpgsign=false', 'commit', '--allow-empty', '-m', 'docs: synthetic change')
        self.assertEqual(self.errors(base), [])
        self.git('-c', 'commit.gpgsign=false', 'commit', '--allow-empty', '-m', 'Merge fake single-parent commit')
        self.assertTrue(any('not a Conventional Commit' in e for e in self.errors(base)))
        self.assertTrue(any('git rev-parse failed' in e for e in self.errors('missing-ref')))
        (self.root / 'new.md').write_text('bad \n')
        self.assertTrue(any('new.md: trailing whitespace' in e for e in self.errors()))
        (self.root / 'disk.img').write_text('synthetic image\n')
        self.git('add', '-f', 'disk.img')
        self.assertIn('generated/private file is tracked: disk.img', self.errors())

    def test_real_merge_title_is_allowed(self):
        self.git('init', '-b', 'dev')
        self.git('add', '.')
        self.git('-c', 'commit.gpgsign=false', 'commit', '-m', 'chore: synthetic baseline')
        base = self.git('rev-parse', 'HEAD')
        self.git('switch', '-c', 'docs/example')
        self.git('-c', 'commit.gpgsign=false', 'commit', '--allow-empty', '-m', 'docs: synthetic topic')
        self.git('switch', 'dev')
        self.git('-c', 'commit.gpgsign=false', 'merge', '--no-ff', 'docs/example', '-m', 'Merge synthetic topic')
        self.assertEqual(self.errors(base), [])


if __name__ == '__main__':
    unittest.main()
