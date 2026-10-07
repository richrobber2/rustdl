import pathlib
import shutil
import subprocess
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
CHANGELOG = pathlib.Path('src/local/pages/changelog.rs')


class PushGuardTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.repo = pathlib.Path(self.temp.name) / 'repo'
        self.remote = pathlib.Path(self.temp.name) / 'remote.git'
        self.repo.mkdir()
        self.git('init', '-q')
        self.git('config', 'user.name', 'Synthetic test')
        self.git('config', 'user.email', 'synthetic@example.invalid')
        subprocess.run(['git', 'init', '--bare', '-q', str(self.remote)], check=True)
        for relative in ['scripts/check-changelog.py', '.githooks/pre-push']:
            target = self.repo / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(ROOT / relative, target)
        (self.repo / CHANGELOG).parent.mkdir(parents=True)
        self.notes('Created the synthetic test application.')
        (self.repo / 'code.txt').write_text('initial')
        self.commit()
        self.git('remote', 'add', 'origin', str(self.remote))
        self.git('push', '-q', '-u', 'origin', 'HEAD:main')
        self.base = self.git('rev-parse', 'HEAD').stdout.strip()
        self.git('config', 'core.hooksPath', '.githooks')

    def tearDown(self):
        self.temp.cleanup()

    def git(self, *args, check=True):
        return subprocess.run(['git', *args], cwd=self.repo, text=True, capture_output=True, check=check)

    def notes(self, *notes):
        text = '\n'.join(f'            "{note}",' for note in notes)
        (self.repo / CHANGELOG).write_text('const CHANGELOG: &[(&str, &[&str])] = &[\n    ("0.1.0", &[\n' + text + '\n    ]),\n];\n')

    def commit(self):
        self.git('add', '.')
        self.git('commit', '-q', '-m', 'Synthetic change')

    def push(self, ref='main'):
        return self.git('push', '-q', 'origin', 'HEAD:' + ref, check=False)

    def test_missing_notes_and_uncommitted_notes_block_real_push(self):
        (self.repo / 'code.txt').write_text('changed')
        self.commit()
        self.notes('Created the synthetic test application.', 'Added the synthetic changed behavior.')
        result = self.push()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('Changelog guard:', result.stderr)
        remote_head = subprocess.check_output(['git', '--git-dir', str(self.remote), 'rev-parse', 'main'], text=True).strip()
        self.assertEqual(remote_head, self.base)

    def test_committed_notes_allow_real_push(self):
        (self.repo / 'code.txt').write_text('changed')
        self.notes('Created the synthetic test application.', 'Added the synthetic changed behavior.')
        self.commit()
        self.assertEqual(self.push().returncode, 0)

    def test_changelog_code_change_without_release_notes_blocks_push(self):
        with (self.repo / CHANGELOG).open('a') as stream:
            stream.write('// Changed the rendering implementation.\n')
        self.commit()
        self.assertNotEqual(self.push().returncode, 0)

    def test_new_branch_with_missing_notes_is_blocked(self):
        (self.repo / 'code.txt').write_text('changed')
        self.commit()
        self.assertNotEqual(self.push('new-branch').returncode, 0)

    def test_existing_history_under_new_branch_is_allowed(self):
        self.assertEqual(self.push('existing-history').returncode, 0)

    def test_empty_or_placeholder_notes_do_not_satisfy_guard(self):
        (self.repo / 'code.txt').write_text('changed')
        self.notes('Created the synthetic test application.', '                  ', 'TODO fill out the release notes')
        self.commit()
        self.assertNotEqual(self.push().returncode, 0)

    def test_duplicating_an_existing_note_does_not_satisfy_guard(self):
        (self.repo / 'code.txt').write_text('changed')
        self.notes('Created the synthetic test application.', 'Created the synthetic test application.')
        self.commit()
        self.assertNotEqual(self.push().returncode, 0)

    def test_initial_ci_range_requires_release_notes(self):
        command = ['python3', 'scripts/check-changelog.py', '--range', '0' * 40, 'HEAD']
        result = subprocess.run(command, cwd=self.repo, capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.notes()
        self.commit()
        result = subprocess.run(command, cwd=self.repo, capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)


if __name__ == '__main__':
    unittest.main()
