#!/usr/bin/env python3
"""Version rollovers, lockfile consistency and forgotten documentation bumps."""
from pathlib import Path
import subprocess
import tempfile
import unittest
from project_version import PACKAGES, bump, check, check_history, next_version, parse_version


def workspace(root, version="0.0.1"):
    for name, path in PACKAGES.items():
        file = root / path
        file.parent.mkdir(parents=True, exist_ok=True)
        shared = f'[workspace.package]\nversion = "{version}"\n\n' if path == "Cargo.toml" else ""
        file.write_text(shared + f'[package]\nname = "{name}"\nversion.workspace = true\n')
    (root / "Cargo.lock").write_text("\n".join(
        f'[[package]]\nname = "{name}"\nversion = "{version}"\n' for name in PACKAGES
    ) + '\n[[package]]\nname = "external-fixture"\nversion = "4.5.6"\nsource = "registry"\n')
    (root / "PROJECT_PLAN.md").write_text(f"Current project version: **{version}**\n")


def git(root, *args):
    return subprocess.check_output(["git", "-c", "user.name=Version Tests",
                                   "-c", "user.email=version-tests@example.invalid", *args],
                                  cwd=root, text=True, stderr=subprocess.DEVNULL)


class ProjectVersionTests(unittest.TestCase):
    def test_base_100_rollovers(self):
        for before, after in [("0.0.1", "0.0.2"), ("0.0.98", "0.0.99"),
                              ("0.0.99", "0.1.0"), ("0.1.99", "0.2.0"),
                              ("0.99.99", "1.0.0"), ("9.99.99", "10.0.0")]:
            with self.subTest(before=before):
                self.assertEqual(next_version(before), after)

    def test_invalid_counters_are_rejected(self):
        for value in [None, "-1.0.0", "0.0.100", "0.100.0", "00.0.1", "0.1", "0.0.1+git"]:
            with self.subTest(value=value), self.assertRaises(ValueError):
                parse_version(value)

    def test_bump_updates_all_local_lock_entries_and_preserves_dependencies(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            workspace(root, "0.0.99")
            self.assertEqual(bump(root), "0.1.0")
            self.assertEqual(check(root), "0.1.0")
            self.assertIn("**0.1.0**", (root / "PROJECT_PLAN.md").read_text())
            self.assertIn('version = "4.5.6"', (root / "Cargo.lock").read_text())
            lock = root / "Cargo.lock"
            lock.write_text(lock.read_text().replace('version = "0.1.0"', 'version = "0.0.99"', 1))
            with self.assertRaisesRegex(ValueError, "Cargo.lock"):
                check(root)

    def test_history_rejects_a_documentation_change_without_a_bump(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            git(root, "init", "-q")
            (root / "README.md").write_text("Before versioned development\n")
            git(root, "add", ".")
            git(root, "commit", "-qm", "Old documentation-only history")
            workspace(root)
            git(root, "add", ".")
            git(root, "commit", "-qm", "Bootstrap 0.0.1")
            self.assertEqual(check_history(root), 1)
            (root / "README.md").write_text("Documentation changed\n")
            git(root, "add", ".")
            git(root, "commit", "-qm", "Forgot the version")
            with self.assertRaisesRegex(ValueError, "requires version 0.0.2"):
                check_history(root)
            bump(root)
            git(root, "add", ".")
            git(root, "commit", "--amend", "--no-edit", "-q")
            self.assertEqual(check_history(root), 2)


if __name__ == "__main__":
    unittest.main()
