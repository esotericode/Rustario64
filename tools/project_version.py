#!/usr/bin/env python3
"""Maintain the shared project change counter and validate committed bumps."""
import argparse
from pathlib import Path
import re
import subprocess
import tomllib

ROOT = Path(__file__).resolve().parent.parent
PACKAGES = {"rustario64": "Cargo.toml", "rustario64-render": "render/Cargo.toml",
            "rustario64-oracle": "oracle/Cargo.toml"}


def parse_version(value):
    if not isinstance(value, str) or not re.fullmatch(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)", value):
        raise ValueError(f"invalid project version: {value!r}")
    parts = tuple(map(int, value.split(".")))
    if parts[1] >= 100 or parts[2] >= 100:
        raise ValueError("minor and patch must be in 0..99")
    return parts


def next_version(value):
    major, minor, patch = parse_version(value)
    patch += 1
    if patch == 100:
        minor, patch = minor + 1, 0
    if minor == 100:
        major, minor = major + 1, 0
    return f"{major}.{minor}.{patch}"


def manifest_version(text):
    return tomllib.loads(text).get("workspace", {}).get("package", {}).get("version")


def check(root=ROOT):
    version = manifest_version((root / "Cargo.toml").read_text(encoding="utf-8"))
    parse_version(version)
    for name, path in PACKAGES.items():
        package = tomllib.loads((root / path).read_text(encoding="utf-8"))["package"]
        if package["name"] != name or package.get("version") != {"workspace": True}:
            raise ValueError(f"{path} must inherit the workspace project version")
    locked = tomllib.loads((root / "Cargo.lock").read_text(encoding="utf-8"))["package"]
    for name in PACKAGES:
        versions = [p["version"] for p in locked if p["name"] == name and "source" not in p]
        if versions != [version]:
            raise ValueError(f"Cargo.lock version for {name} must be {version}")
    plan = root / "PROJECT_PLAN.md"
    if plan.exists():
        match = re.search(r"^Current project version: \*\*([^*]+)\*\*", plan.read_text(encoding="utf-8"), re.MULTILINE)
        if match is None or match[1] != version:
            raise ValueError(f"PROJECT_PLAN.md current project version must be {version}")
    return version


def bump(root=ROOT):
    version = next_version(check(root))
    manifest = root / "Cargo.toml"
    # The canonical block puts version first; keep surrounding text unchanged.
    text, count = re.subn(r'(\[workspace\.package\]\s*\nversion = ")[^"]+(")',
                         lambda m: m[1] + version + m[2], manifest.read_text(encoding="utf-8"))
    if count != 1:
        raise ValueError("expected one workspace.package version entry")
    lock = root / "Cargo.lock"
    locked = lock.read_text(encoding="utf-8")
    for name in PACKAGES:
        locked, count = re.subn(r'(^name = "' + re.escape(name) + r'"\nversion = ")[^"]+(")',
                               lambda m: m[1] + version + m[2], locked, flags=re.MULTILINE)
        if count != 1:
            raise ValueError(f"expected one lockfile package for {name}")
    plan = root / "PROJECT_PLAN.md"
    plan_text = None
    if plan.exists():
        plan_text = re.sub(r"^(Current project version: \*\*)[^*]+(\*\*)",
                           lambda m: m[1] + version + m[2], plan.read_text(encoding="utf-8"), flags=re.MULTILINE)
    manifest.write_text(text, encoding="utf-8")
    lock.write_text(locked, encoding="utf-8")
    if plan_text is not None:
        plan.write_text(plan_text, encoding="utf-8")
    return check(root)


def git(root, *args):
    return subprocess.check_output(["git", *args], cwd=root, text=True, encoding="utf-8").strip()


def version_at(root, ref):
    result = subprocess.run(["git", "show", f"{ref}:Cargo.toml"], cwd=root,
                            text=True, encoding="utf-8", capture_output=True)
    if result.returncode:
        # Very early project history may contain documentation only. Reject an
        # invalid revision, but allow a valid commit predating Cargo.toml.
        git(root, "rev-parse", "--verify", f"{ref}^{{commit}}")
        return None
    return manifest_version(result.stdout)


def require_bump(before, after):
    expected = next_version(before) if before is not None else "0.0.1"
    if after != expected:
        raise ValueError(f"project change requires version {expected}, found {after}")


def check_history(root=ROOT, ref="HEAD"):
    count = 0
    for commit in git(root, "rev-list", "--reverse", "--topo-order", ref).splitlines():
        after = version_at(root, commit)
        if after is None:  # History before the 0.0.1 versioning bootstrap.
            continue
        parse_version(after)
        parents = git(root, "show", "-s", "--format=%P", commit).split()
        before = [version_at(root, parent) for parent in parents]
        try:
            if len(parents) > 1:
                # Integrating already-versioned changes preserves the newest
                # counter; additional merge edits can advance it once more.
                versions = [v for v in before if v is not None]
                newest = max(versions, key=parse_version) if versions else None
                allowed = {newest, next_version(newest)} if newest else {"0.0.1"}
                if after not in allowed:
                    raise ValueError(f"merge must preserve/advance the newest version {newest}")
            elif parents and git(root, "rev-parse", f"{commit}^{{tree}}") == git(root, "rev-parse", f"{parents[0]}^{{tree}}"):
                if after != before[0]:
                    raise ValueError("an empty checkpoint must preserve its version")
            else:
                require_bump(before[0] if before else None, after)
        except ValueError as error:
            raise ValueError(f"{commit[:12]}: {error}") from error
        count += 1
    return count


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["show", "bump", "check"])
    parser.add_argument("--base", help="require the working version to advance this Git revision")
    parser.add_argument("--history", action="store_true", help="check all versioned checkpoints")
    args = parser.parse_args()
    try:
        version = bump() if args.command == "bump" else check()
        if args.base:
            require_bump(version_at(ROOT, args.base), version)
        if args.history:
            print(f"Checked {check_history()} versioned checkpoints")
        print(version)
    except (ValueError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"version error: {error}\n")


if __name__ == "__main__":
    main()
