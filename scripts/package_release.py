#!/usr/bin/env python3
"""Validate version tags and package native, CPU-only EarthCraft releases."""

import argparse
import hashlib
import os
from pathlib import Path
import plistlib
import re
import shutil
import subprocess
import tarfile
import tempfile
import tomllib
import zipfile

ROOT = Path(__file__).resolve().parents[1]
TARGETS = {
    "x86_64-unknown-linux-gnu": "linux-x64",
    "aarch64-unknown-linux-gnu": "linux-arm64",
    "x86_64-pc-windows-msvc": "windows-x64-portable",
    "aarch64-pc-windows-msvc": "windows-arm64-portable",
    "universal-apple-darwin": "macos-universal",
}
VERSION = re.compile(
    r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)"
    r"(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?"
    r"(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?"
)


def workspace_version():
    with (ROOT / "Cargo.toml").open("rb") as manifest:
        version = tomllib.load(manifest)["workspace"]["package"]["version"]
    if not VERSION.fullmatch(version):
        raise ValueError(f"Unsupported workspace version: {version}")
    return version


def validate(tag):
    version = workspace_version()
    if tag and tag != f"v{version}":
        raise ValueError(f"Tag {tag!r} must match workspace version v{version}")
    if output := os.environ.get("GITHUB_OUTPUT"):
        with Path(output).open("a", encoding="utf-8") as stream:
            stream.write(f"version={version}\n")
    print(version)


def package(version, target, binary, output, changelog):
    if version != workspace_version():
        raise ValueError("Package version must match the workspace manifest")
    if target not in TARGETS:
        raise ValueError(f"Unsupported target: {target}")
    if not binary.is_file():
        raise ValueError(f"Release executable does not exist: {binary}")
    if not changelog.is_file() or not changelog.read_text(encoding="utf-8").strip():
        raise ValueError(f"Generated changelog is missing or empty: {changelog}")
    if target == "universal-apple-darwin":
        subprocess.run(
            ["lipo", str(binary), "-verify_arch", "arm64", "x86_64"], check=True
        )
    output.mkdir(parents=True, exist_ok=True)
    name = f"earthcraft-{version}-{TARGETS[target]}"
    extension = ".tar.gz" if "linux" in target else ".zip"
    archive = output / f"{name}{extension}"
    with tempfile.TemporaryDirectory(prefix="earthcraft-package-") as temporary:
        payload = Path(temporary) / name
        payload.mkdir()
        for filename in ("README.md", "README.pt-BR.md", "AGENTS.md"):
            shutil.copy2(ROOT / filename, payload / filename)
        for filename in ("LICENSE", "LICENSE.md", "LICENSE.txt"):
            if (ROOT / filename).is_file():
                shutil.copy2(ROOT / filename, payload / filename)
        shutil.copytree(ROOT / "docs", payload / "docs")
        shutil.copy2(changelog, payload / "CHANGELOG.md")
        if "apple" in target:
            contents = payload / "EarthCraft.app" / "Contents"
            executable = contents / "MacOS" / "earthcraft"
            executable.parent.mkdir(parents=True)
            resources = contents / "Resources"
            resources.mkdir()
            shutil.copy2(ROOT / "assets" / "icon.icns", resources / "EarthCraft.icns")
            with (contents / "Info.plist").open("wb") as stream:
                plistlib.dump(
                    {
                        "CFBundleIdentifier": "com.theri.earthcraft",
                        "CFBundleName": "EarthCraft",
                        "CFBundleDisplayName": "EarthCraft",
                        "CFBundleExecutable": "earthcraft",
                        "CFBundleIconFile": "EarthCraft.icns",
                        "CFBundlePackageType": "APPL",
                        "CFBundleVersion": version.split("-")[0].split("+")[0],
                        "CFBundleShortVersionString": version.split("-")[0].split("+")[0],
                        "LSMinimumSystemVersion": "11.0",
                        "NSHighResolutionCapable": True,
                    },
                    stream,
                )
        else:
            executable = payload / ("earthcraft.exe" if "windows" in target else "earthcraft")
        shutil.copy2(binary, executable)
        executable.chmod(0o755)
        if "apple" in target:
            app = payload / "EarthCraft.app"
            subprocess.run(["codesign", "--force", "--sign", "-", str(app)], check=True)
            subprocess.run(
                ["codesign", "--verify", "--deep", "--strict", str(app)], check=True
            )
        (payload / "RELEASE.txt").write_text(
            f"EarthCraft {version} — {target}\n\n"
            "This archive contains a CPU-only release build. GPU rendering is not enabled.\n"
            "Linux requires a graphics driver and GTK 3 runtime libraries.\n"
            "macOS bundles are ad-hoc signed, without Developer ID or notarization.\n"
            "Extract the entire archive before launching. See docs/RELEASING.md.\n",
            encoding="utf-8",
        )
        if extension == ".tar.gz":
            with tarfile.open(archive, "w:gz") as stream:
                stream.add(payload, arcname=name)
        else:
            with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as stream:
                for path in sorted(payload.rglob("*")):
                    if path.is_file():
                        stream.write(path, path.relative_to(payload.parent))
    with archive.open("rb") as stream:
        digest = hashlib.file_digest(stream, "sha256").hexdigest()
    archive.with_name(archive.name + ".sha256").write_text(
        f"{digest}  {archive.name}\n", encoding="utf-8"
    )
    print(archive)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    version = commands.add_parser("validate")
    version.add_argument("--tag", default="")
    archive = commands.add_parser("package")
    archive.add_argument("--version", required=True)
    archive.add_argument("--target", choices=TARGETS, required=True)
    archive.add_argument("--binary", type=Path, required=True)
    archive.add_argument("--output", type=Path, required=True)
    archive.add_argument("--changelog", type=Path, required=True)
    args = parser.parse_args()
    try:
        if args.command == "validate":
            validate(args.tag)
        else:
            package(args.version, args.target, args.binary, args.output, args.changelog)
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"Release packaging failed: {error}\n")


if __name__ == "__main__":
    main()
