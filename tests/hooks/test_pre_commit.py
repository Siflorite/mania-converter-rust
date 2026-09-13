"""Run with: python -m unittest discover -s tests/hooks -v.

Requires Git and a POSIX sh. Cargo is mocked; Git/index behavior is real.
"""

import os
from pathlib import Path
import subprocess
import tempfile
import unittest


HOOK = Path(__file__).resolve().parents[2] / ".cargo-husky/hooks/pre-commit"


class PreCommitTests(unittest.TestCase):
    def run_case(self, *, dirty=False, fix_step=0, fail_step=0,
                 snapshot_failure=False):
        with tempfile.TemporaryDirectory(prefix="hook-test-") as directory:
            root = Path(directory)
            env = os.environ.copy()
            # Keep a caller's Git hook environment out of the temporary repo.
            for key in list(env):
                if key.startswith("GIT_"):
                    del env[key]
            env.update(FIX_STEP=str(fix_step), FAIL_STEP=str(fail_step))

            def git(*args):
                return subprocess.run(
                    ["git", *args], cwd=root, env=env, check=True,
                    stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                ).stdout

            git("init", "-q")
            source = root / "source file.rs"
            source.write_text("staged content\n", encoding="utf-8")
            git("add", "--", source.name)
            if dirty:
                source.write_text("existing unstaged content\n", encoding="utf-8")
            # Includes a partially staged file when dirty=True.
            original_index = (root / ".git/index").read_bytes()

            script = r"""
step=0
cargo() {
    step=$((step + 1))
    printf '%s\n' "$step" >> calls
    if [ "$step" -eq "$FIX_STEP" ]; then
        printf 'automatic fix\n' >> 'source file.rs'
    fi
    [ "$step" -ne "$FAIL_STEP" ]
}
"""
            if snapshot_failure:
                script += '\ngit() { return 1; }\n'
            script += '\n. "$1"\n'
            result = subprocess.run(
                ["sh", "-c", script, "hook-test", HOOK.as_posix()],
                cwd=root, env=env, capture_output=True, text=True,
            )
            should_fail = bool(fix_step or fail_step or snapshot_failure)
            self.assertEqual(result.returncode, int(should_fail), result.stdout + result.stderr)
            self.assertEqual((root / ".git/index").read_bytes(), original_index)
            if not snapshot_failure:
                self.assertEqual((root / "calls").read_text().splitlines(), ["1", "2", "3", "4"])
            if fix_step:
                self.assertIn("automatic fix", source.read_text())
                self.assertIn("review and stage fixes", result.stdout)

    def test_unchanged_staged_files_pass(self):
        self.run_case()

    def test_preexisting_unstaged_changes_pass(self):
        self.run_case(dirty=True)

    def test_each_fixer_aborts_without_staging(self):
        for step in (1, 2, 3):
            with self.subTest(step=step):
                self.run_case(fix_step=step)

    def test_fixing_already_dirty_file_aborts(self):
        self.run_case(dirty=True, fix_step=2)

    def test_cargo_failures_abort(self):
        for step in (1, 2, 3, 4):
            with self.subTest(step=step):
                self.run_case(fail_step=step)

    def test_snapshot_failure_aborts(self):
        self.run_case(snapshot_failure=True)


if __name__ == "__main__":
    unittest.main()
