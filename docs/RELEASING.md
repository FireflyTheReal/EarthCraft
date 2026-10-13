# Releases

The [release workflow](../.github/workflows/release.yml) builds locked release
binaries with Rust 1.89 and includes both READMEs and contributor documentation.
These artifacts use `--no-default-features`: GPU rendering is excluded while
native cross-platform GPU failures remain unresolved.

| Platform | Artifact |
| --- | --- |
| Linux x86-64 and ARM64 | `.tar.gz` containing the executable |
| Windows x86-64 and ARM64 | `.zip` containing `earthcraft.exe` |
| macOS Intel and Apple Silicon | One universal `.zip` containing `EarthCraft.app` |

Each of the five release archives has a SHA-256 checksum file. Linux builds run
on Ubuntu 22.04; GTK 3 and a working graphics driver are runtime requirements.
macOS bundles
target macOS 11 or newer and are unsigned/unnotarized. Intel and Apple Silicon
executables build separately, then `lipo` combines them into the universal bundle.
Packaging verifies that both architectures are present. The separate macOS build
artifacts are intermediate files and are not published as release downloads.
Archives are portable packages, not installers. Build/package checks do not verify
interactive runtime behavior: manually test downloads on the supported platforms
before announcing them.

## Preview builds

Pull requests changing the release workflow, packaging script or this guide run
six native architecture builds and upload preview artifacts without publishing
a release.
After merge, the Actions page also offers a manual **Release** run with the same
preview-only behavior. Inspect and test these artifacts before creating a version tag.

## Publish a version

1. Set `workspace.package.version` in `Cargo.toml`, regenerate `Cargo.lock`,
   run the contributor checks, and merge those changes.
2. From the reviewed commit, create and push the matching tag, for example
   `git tag v0.1.0` followed by `git push upstream v0.1.0`.
3. The workflow requires the tag to equal `v` plus the workspace version.
   All six native builds, universal macOS packaging and archive checksums must
   succeed before publication.
4. Publication first creates a draft with generated notes, uploads the complete
   artifact set, then publishes it. Versions containing a prerelease suffix are
   marked as prereleases.

A failed upload leaves a draft that can be completed by rerunning the workflow.
A published release is not overwritten by a rerun; use a new version and tag.
Only the tag-triggered publication job has `contents: write`; build and preview
jobs have read-only repository access. No signing credentials or additional
personal access token are needed.

## Local packaging

Python 3.11+ is required for the packaging helper. For a native Linux x86-64 build:

```sh
python3 scripts/package_release.py validate --tag v0.1.0
cargo build -p earthcraft --release --no-default-features --locked
python3 scripts/package_release.py package --version 0.1.0 --target x86_64-unknown-linux-gnu --binary target/release/earthcraft --output /tmp/earthcraft-release
```

Use the target and executable name for your native platform. For explicit-target
builds, the executable lives under `target/<target>/release/`.
The helper packages binaries; it does not cross-compile them. The universal macOS
packaging target additionally requires `lipo` and verifies both architecture slices.

To reproduce the universal macOS artifact:

```sh
rustup target add aarch64-apple-darwin x86_64-apple-darwin
MACOSX_DEPLOYMENT_TARGET=11.0 cargo build -p earthcraft --release --no-default-features --locked --target aarch64-apple-darwin
MACOSX_DEPLOYMENT_TARGET=11.0 cargo build -p earthcraft --release --no-default-features --locked --target x86_64-apple-darwin
lipo -create target/aarch64-apple-darwin/release/earthcraft target/x86_64-apple-darwin/release/earthcraft -output /tmp/earthcraft-universal
python3 scripts/package_release.py package --version 0.1.0 --target universal-apple-darwin --binary /tmp/earthcraft-universal --output /tmp/earthcraft-release
```
