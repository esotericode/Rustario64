#!/usr/bin/env python3
"""Authored packaging checks: private files and the C oracle never ship."""
import tempfile
import unittest
from pathlib import Path
import zipfile
import struct
from package_desktop import NOTICES, RUNTIME_BINARIES, package, dependency_notices, windows_subsystem


def authored_pe(subsystem):
    data = bytearray(128 + 94)
    data[:2] = b"MZ"
    struct.pack_into("<I", data, 60, 128)
    data[128:132] = b"PE\0\0"
    struct.pack_into("<H", data, 132, 0x8664)
    struct.pack_into("<H", data, 152, 0x20B)
    struct.pack_into("<H", data, 220, subsystem)
    return data


class DesktopPackageTests(unittest.TestCase):
    def test_allowlist_and_native_names(self):
        for target, suffix in [("linux-x86_64", ""), ("windows-x86_64", ".exe")]:
            with self.subTest(target=target), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                release = root / "release"
                release.mkdir()
                for name in RUNTIME_BINARIES:
                    data = authored_pe(2 if name == "rustario64-desktop" else 3) if suffix else b"authored fake executable"
                    (release / (name + suffix)).write_bytes(data)
                for name in ["owner.z64", "decoded.rgba", "oracle.exe", "run.trace.json"]:
                    (release / name).write_bytes(b"must not ship")
                archive = package(target, release, root / "dist", "authored dependency notices", "authored build identifier")
                with zipfile.ZipFile(archive) as bundle:
                    names = {p.split("/", 1)[1] for p in bundle.namelist()}
                    expected = {name + suffix for name in RUNTIME_BINARIES} | {"THIRD_PARTY_NOTICES.txt", "BUILD_INFO.txt"}
                    expected.update("README.md" if p == "docs/PLAYTEST.md" else p for p in NOTICES)
                    self.assertEqual(names, expected)
                    if not suffix:
                        for name in RUNTIME_BINARIES:
                            self.assertEqual(bundle.getinfo(f"rustario64-{target}/{name}").external_attr >> 16 & 0o777, 0o755)
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

    def test_windows_desktop_must_be_gui_and_viewer_must_be_console(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            for name in RUNTIME_BINARIES:
                (root / (name + ".exe")).write_bytes(authored_pe(3))
            self.assertEqual(windows_subsystem(root / "rustario64-viewer.exe"), 3)
            with self.assertRaisesRegex(ValueError, "GUI/console"):
                package("windows-x86_64", root, root / "dist", "notices", "build")
            self.assertFalse((root / "dist").exists())
            (root / "rustario64-desktop.exe").write_bytes(b"not a PE file")
            with self.assertRaisesRegex(ValueError, "not a Windows executable"):
                windows_subsystem(root / "rustario64-desktop.exe")


if __name__ == "__main__":
    unittest.main()
