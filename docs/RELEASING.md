# Releases

The [release workflow](../.github/workflows/release.yml) builds locked release
binaries with Rust 1.89 (FreeBSD uses its packaged Rust) and includes both READMEs
and contributor documentation. Its desktop target set matches the
[ArtCraft suite release matrix](https://github.com/storytold/craft-common-actions/blob/f0808a5ccbc32869248193ffab0de98d733de451/.github/workflows/release.yml).
These artifacts use `--no-default-features`: GPU rendering is excluded while
native cross-platform GPU failures remain unresolved.

| Platform | Artifact |
| --- | --- |
| Linux x86-64, ARM64 and RISC-V 64 | `.tar.gz` containing the executable |
| Windows x86-64, x86 and ARM64 | `.zip` containing `earthcraft.exe` |
| macOS Apple Silicon | `.zip` containing `EarthCraft.app` |
| macOS Intel | `.zip` containing `EarthCraft.app` |
| FreeBSD x86-64 | `.tar.gz` containing the executable |

Each archive has a SHA-256 checksum file. Native Linux builds run on Ubuntu 22.04;
RISC-V is cross-compiled on Ubuntu 24.04 with ArtCraft's pinned dependency setup
and EarthCraft's GTK 3 dependencies. FreeBSD builds run in a FreeBSD 15.1 VM.
GTK 3 and a working graphics driver are runtime requirements. macOS bundles
target macOS 11 or newer and are unsigned/unnotarized. Archives are portable
packages, not installers. Build/package checks do not verify interactive runtime
behavior: manually test downloads on the supported platforms before announcing them.

## Preview builds

Pull requests changing the release workflow, packaging script or this guide run
all nine desktop targets and upload preview artifacts without publishing a release.
After merge, the Actions page also offers a manual **Release** run with the same
preview-only behavior. Inspect and test these artifacts before creating a version tag.

## Publish a version

1. Set `workspace.package.version` in `Cargo.toml`, regenerate `Cargo.lock`,
   run the contributor checks, and merge those changes.
2. From the reviewed commit, create and push the matching tag, for example
   `git tag v0.1.0` followed by `git push upstream v0.1.0`.
3. The workflow requires the tag to equal `v` plus the workspace version.
   All nine target builds and archive checksums must succeed before publication.
4. Publication first creates a draft with generated notes, uploads the complete
   artifact set, then publishes it. Versions containing a prerelease suffix are
   marked as prereleases.

A failed upload leaves a draft that can be completed by rerunning the workflow.
A published release is not overwritten by a rerun; use a new version and tag.
Only the tag-triggered publication job has `contents: write`; build and preview
jobs have read-only repository access. No signing credentials or additional
personal access token are needed.

## Local packaging

Python 3.11+ is required for the packaging helper:

```sh
python3 scripts/package_release.py validate --tag v0.1.0
cargo build -p earthcraft --release --no-default-features --locked
python3 scripts/package_release.py package --version 0.1.0 --target aarch64-apple-darwin --binary target/release/earthcraft --output /tmp/earthcraft-release
```

Use the target and executable name for your native platform. For explicit-target
builds, the executable lives under `target/<target>/release/`.
The helper packages binaries; it does not cross-compile them or validate their architecture.
