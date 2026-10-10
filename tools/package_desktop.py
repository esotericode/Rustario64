#!/usr/bin/env python3
"""Package only the three Rust runtime binaries and required notices (MIT).

Never walks target/, private/, or the repository to discover bundle files.
The native C oracle and all ROM-derived content are excluded by construction.
"""
import argparse
import json
import subprocess
import shutil
import stat
import struct
from pathlib import Path
import zipfile
from project_version import check as project_version

ROOT = Path(__file__).resolve().parent.parent
TARGETS = {"linux-x86_64": "", "windows-x86_64": ".exe"}
TRIPLES = {"linux-x86_64": "x86_64-unknown-linux-gnu", "windows-x86_64": "x86_64-pc-windows-msvc"}
RUNTIME_BINARIES = ["rustario64", "rustario64-viewer", "rustario64-desktop"]
FALLBACK_LICENSES = {
    ("gilrs", "0.11.2"): [ROOT / "LICENSES/dependencies/gilrs-0.11.2-MIT.txt",
                           ROOT / "LICENSES/dependencies/gilrs-0.11.2-controller-db-ZLIB.txt"],
    ("gilrs-core", "0.6.8"): [ROOT / "LICENSES/dependencies/gilrs-0.11.2-MIT.txt"],
    ("gl_generator", "0.14.0"): [ROOT / "LICENSES/dependencies/gl_generator-0.14.0-APACHE.txt"],
    ("khronos_api", "3.1.0"): [ROOT / "LICENSES/dependencies/khronos_api-3.1.0-APACHE.txt"],
    ("profiling", "1.0.18"): [ROOT / "LICENSES/dependencies/profiling-1.0.18-MIT.txt"],
    ("spirv", "0.4.0+sdk-1.4.341.0"): [ROOT / "LICENSES/dependencies/spirv-0.4.0-APACHE.txt"],
    ("accesskit", "0.24.1"): [ROOT / "LICENSES/dependencies/accesskit-0.24.1-MIT.txt"],
    **{(name, "0.36.2"): [ROOT / "LICENSES/dependencies/egui-0.36.2-MIT.txt"]
       for name in ["ecolor", "egui", "egui-wgpu", "egui-winit", "emath", "epaint"]},
    ("epaint_default_fonts", "0.36.2"): [ROOT / "LICENSES/dependencies/egui-0.36.2-MIT.txt",
        *sorted((ROOT / "LICENSES/dependencies").glob("egui-fonts-0.36.2-*.txt"))],
}
NOTICES = ["LICENSE", "PROVENANCE.md", "docs/PLAYTEST.md",
           "LICENSES/sm64-CC0.txt", "LICENSES/sm64tools-MIT.txt"]


def dependency_notices(metadata: dict) -> str:
    packages = {p["id"]: p for p in metadata["packages"]}
    nodes = {n["id"]: n for n in metadata["resolve"]["nodes"]}
    pending = [p["id"] for p in metadata["packages"] if p["name"] in ["rustario64", "rustario64-render"]]
    seen = set()
    while pending:
        identity = pending.pop()
        if identity in seen:
            continue
        seen.add(identity)
        for dep in nodes[identity]["deps"]:
            if any(kind["kind"] != "dev" for kind in dep["dep_kinds"]):
                pending.append(dep["pkg"])
    out = ["Third-party runtime dependency notices\n"]
    for identity in sorted(seen, key=lambda key: (packages[key]["name"], packages[key]["version"])):
        p = packages[identity]
        if p["source"] is None:
            continue
        root = Path(p["manifest_path"]).parent
        candidates = [f for f in root.iterdir() if f.name.lower().startswith(("license", "licence", "copyright", "notice", "copying"))]
        if p.get("license_file"):
            candidates.append(root / p["license_file"])
        files = set()
        for candidate in candidates:
            if candidate.is_file():
                files.add(candidate)
            elif candidate.is_dir():
                files.update(f for f in candidate.rglob("*") if f.is_file())
        if not files:
            files.update(FALLBACK_LICENSES.get((p["name"], p["version"]), []))
        if not files:
            raise ValueError(f"no license/notice files for {p['name']} {p['version']}")
        out.append(f"\n{p['name']} {p['version']} ({p['license'] or 'see license file'})\n")
        for source in sorted(files):
            label = source.relative_to(root) if source.is_relative_to(root) else source.name
            out.append(f"\n--- {label} ---\n" + source.read_text(encoding="utf-8"))
    return "\n".join(out)


def windows_subsystem(path: Path) -> int:
    """Read the bounded PE headers; verify the packaged Windows x64 entry point."""
    with path.open("rb") as source:
        dos = source.read(64)
        if len(dos) != 64 or dos[:2] != b"MZ":
            raise ValueError(f"not a Windows executable: {path}")
        offset = struct.unpack_from("<I", dos, 60)[0]
        if not 64 <= offset <= 4096:
            raise ValueError(f"invalid PE header offset: {path}")
        source.seek(offset)
        header = source.read(94)
        if len(header) != 94 or header[:4] != b"PE\0\0" or struct.unpack_from("<H", header, 4)[0] != 0x8664 or struct.unpack_from("<H", header, 24)[0] != 0x20B:
            raise ValueError(f"not a Windows x64 PE executable: {path}")
        return struct.unpack_from("<H", header, 92)[0]


def package(target: str, release_dir: Path, output: Path, notices: str, build_info: str, version: str) -> Path:
    suffix = TARGETS[target]
    files = [(release_dir / (name + suffix), name + suffix)
             for name in RUNTIME_BINARIES]
    files += [(ROOT / name, "README.md" if name == "docs/PLAYTEST.md" else name)
              for name in NOTICES]
    for source, _ in files:
        if source.is_symlink() or not source.is_file():
            raise ValueError(f"missing regular bundle file: {source}")
    if target == "windows-x86_64":
        for name in RUNTIME_BINARIES:
            expected = 2 if name == "rustario64-desktop" else 3
            if windows_subsystem(release_dir / (name + suffix)) != expected:
                raise ValueError(f"incorrect Windows GUI/console subsystem: {name}")
    output.mkdir(parents=True, exist_ok=True)
    folder = f"rustario64-{version}-{target}"
    archive = output / f"{folder}.zip"
    # An existing artifact is an error so partial/old bundles cannot be reused.
    with zipfile.ZipFile(archive, "x", compression=zipfile.ZIP_DEFLATED) as bundle:
        bundle.writestr(f"{folder}/THIRD_PARTY_NOTICES.txt", notices)
        bundle.writestr(f"{folder}/BUILD_INFO.txt", build_info)
        for source, destination in files:
            name = f"{folder}/{destination}"
            if target == "linux-x86_64" and destination in RUNTIME_BINARIES:
                info = zipfile.ZipInfo.from_file(source, name)
                info.create_system = 3
                info.external_attr = (stat.S_IFREG | 0o755) << 16
                info.compress_type = zipfile.ZIP_DEFLATED
                with source.open("rb") as data, bundle.open(info, "w") as entry:
                    shutil.copyfileobj(data, entry)
            else:
                bundle.write(source, name)
    return archive


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", choices=TARGETS, required=True)
    parser.add_argument("--release-dir", type=Path, default=ROOT / "target/release")
    parser.add_argument("--output", type=Path, default=ROOT / "dist")
    args = parser.parse_args()
    version = project_version(ROOT)
    for name in RUNTIME_BINARIES:
        binary = (args.release_dir / (name + TARGETS[args.target])).resolve()
        actual = subprocess.check_output([str(binary), "--version"], text=True).strip()
        if actual != f"Rustario64 v{version}":
            raise ValueError(f"{binary.name} reports {actual!r}; rebuild version {version} before packaging")
    raw = subprocess.check_output(["cargo", "metadata", "--locked", "--format-version", "1",
                                   "--filter-platform", TRIPLES[args.target]], cwd=ROOT)
    notices = dependency_notices(json.loads(raw))
    commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    dirty = bool(subprocess.check_output(["git", "status", "--porcelain", "--untracked-files=no"],
                                        cwd=ROOT, text=True).strip())
    compiler = subprocess.check_output(["rustc", "--version"], cwd=ROOT, text=True).strip()
    build_info = f"Version: {version}\nCommit: {commit}\nTracked source modified: {dirty}\nTarget: {args.target}\nCompiler: {compiler}\n"
    print(package(args.target, args.release_dir, args.output, notices, build_info, version))


if __name__ == "__main__":
    main()
