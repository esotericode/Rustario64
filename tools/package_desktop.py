#!/usr/bin/env python3
"""Package only the two Rust runtime binaries and required notices (MIT).

Never walks target/, private/, or the repository to discover bundle files.
The native C oracle and all ROM-derived content are excluded by construction.
"""
import argparse
import json
import subprocess
from pathlib import Path
import zipfile

ROOT = Path(__file__).resolve().parent.parent
TARGETS = {"linux-x86_64": "", "windows-x86_64": ".exe"}
TRIPLES = {"linux-x86_64": "x86_64-unknown-linux-gnu", "windows-x86_64": "x86_64-pc-windows-msvc"}
FALLBACK_LICENSES = {
    ("gl_generator", "0.14.0"): [ROOT / "LICENSES/dependencies/gl_generator-0.14.0-APACHE.txt"],
    ("khronos_api", "3.1.0"): [ROOT / "LICENSES/dependencies/khronos_api-3.1.0-APACHE.txt"],
    ("profiling", "1.0.18"): [ROOT / "LICENSES/dependencies/profiling-1.0.18-MIT.txt"],
    ("spirv", "0.4.0+sdk-1.4.341.0"): [ROOT / "LICENSES/dependencies/spirv-0.4.0-APACHE.txt"],
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


def package(target: str, release_dir: Path, output: Path, notices: str, build_info: str) -> Path:
    suffix = TARGETS[target]
    files = [(release_dir / (name + suffix), name + suffix)
             for name in ["rustario64", "rustario64-viewer"]]
    files += [(ROOT / name, "README.md" if name == "docs/PLAYTEST.md" else name)
              for name in NOTICES]
    for source, _ in files:
        if source.is_symlink() or not source.is_file():
            raise ValueError(f"missing regular bundle file: {source}")
    output.mkdir(parents=True, exist_ok=True)
    archive = output / f"rustario64-{target}.zip"
    # An existing artifact is an error so partial/old bundles cannot be reused.
    with zipfile.ZipFile(archive, "x", compression=zipfile.ZIP_DEFLATED) as bundle:
        bundle.writestr(f"rustario64-{target}/THIRD_PARTY_NOTICES.txt", notices)
        bundle.writestr(f"rustario64-{target}/BUILD_INFO.txt", build_info)
        for source, destination in files:
            bundle.write(source, f"rustario64-{target}/{destination}")
    return archive


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", choices=TARGETS, required=True)
    parser.add_argument("--release-dir", type=Path, default=ROOT / "target/release")
    parser.add_argument("--output", type=Path, default=ROOT / "dist")
    args = parser.parse_args()
    raw = subprocess.check_output(["cargo", "metadata", "--locked", "--format-version", "1",
                                   "--filter-platform", TRIPLES[args.target]], cwd=ROOT)
    notices = dependency_notices(json.loads(raw))
    commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    dirty = bool(subprocess.check_output(["git", "status", "--porcelain", "--untracked-files=no"],
                                        cwd=ROOT, text=True).strip())
    compiler = subprocess.check_output(["rustc", "--version"], cwd=ROOT, text=True).strip()
    build_info = f"Commit: {commit}\nTracked source modified: {dirty}\nTarget: {args.target}\nCompiler: {compiler}\n"
    print(package(args.target, args.release_dir, args.output, notices, build_info))


if __name__ == "__main__":
    main()
