#!/usr/bin/env python3
"""Authored packaging checks: private files and the C oracle never ship."""
import tempfile
import unittest
from pathlib import Path
import zipfile
from package_desktop import NOTICES, package, dependency_notices


class DesktopPackageTests(unittest.TestCase):
    def test_allowlist_and_native_names(self):
        for target, suffix in [("linux-x86_64", ""), ("windows-x86_64", ".exe")]:
            with self.subTest(target=target), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                release = root / "release"
                release.mkdir()
                for name in ["rustario64", "rustario64-viewer"]:
                    (release / (name + suffix)).write_bytes(b"authored fake executable")
                for name in ["owner.z64", "decoded.rgba", "oracle.exe", "run.trace.json"]:
                    (release / name).write_bytes(b"must not ship")
                archive = package(target, release, root / "dist", "authored dependency notices", "authored build identifier")
                with zipfile.ZipFile(archive) as bundle:
                    names = {p.split("/", 1)[1] for p in bundle.namelist()}
                    expected = {"rustario64" + suffix, "rustario64-viewer" + suffix, "THIRD_PARTY_NOTICES.txt", "BUILD_INFO.txt"}
                    expected.update("README.md" if p == "docs/PLAYTEST.md" else p for p in NOTICES)
                    self.assertEqual(names, expected)
                with self.assertRaises(FileExistsError):
                    package(target, release, root / "dist", "authored dependency notices", "authored build identifier")

    def test_runtime_graph_excludes_oracle_and_dev_dependencies(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            runtime = root / "runtime"
            runtime.mkdir()
            (runtime / "LICENSE").write_text("authored runtime license")
            packages = [
                {"id": "core", "name": "rustario64", "version": "1", "source": None},
                {"id": "viewer", "name": "rustario64-render", "version": "1", "source": None},
                {"id": "normal", "name": "normal", "version": "1", "source": "registry", "manifest_path": str(runtime / "Cargo.toml"), "license": "MIT"},
                {"id": "dev", "name": "private-dev", "version": "1", "source": "registry"},
                {"id": "oracle", "name": "rustario64-oracle", "version": "1", "source": None},
            ]
            nodes = [
                {"id": "core", "deps": [{"pkg": "dev", "dep_kinds": [{"kind": "dev"}]}]},
                {"id": "viewer", "deps": [{"pkg": "core", "dep_kinds": [{"kind": None}]}, {"pkg": "normal", "dep_kinds": [{"kind": None}]}]},
                {"id": "normal", "deps": []},
                {"id": "dev", "deps": []},
                {"id": "oracle", "deps": [{"pkg": "dev", "dep_kinds": [{"kind": None}]}]},
            ]
            notices = dependency_notices({"packages": packages, "resolve": {"nodes": nodes}})
            self.assertIn("authored runtime license", notices)
            self.assertNotIn("private-dev", notices)
            self.assertNotIn("rustario64-oracle", notices)

    def test_missing_binary_fails_before_creating_archive(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            with self.assertRaises(ValueError):
                package("linux-x86_64", root, root / "dist", "authored dependency notices", "authored build identifier")
            self.assertFalse((root / "dist").exists())


if __name__ == "__main__":
    unittest.main()
