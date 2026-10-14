"""Offline archive, metadata and release-version regression checks."""

import hashlib
from pathlib import Path
import tarfile
import tempfile
import unittest
import zipfile

import package_release as release


class ReleasePackagingTests(unittest.TestCase):
    def test_archive_contents_permissions_and_checksums(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            binary = root / "binary"
            binary.write_bytes(b"archive fixture, not a native executable")
            changelog = root / "notes.md"
            changelog.write_text("# Changelog\n\n- Fixture release\n", encoding="utf-8")
            version = release.workspace_version()
            for target in release.TARGETS:
                if "apple" in target:
                    continue  # Universal Mach-O/signing is verified on macOS separately.
                with self.subTest(target=target):
                    output = root / target
                    release.package(version, target, binary, output, changelog)
                    archive = next(p for p in output.iterdir() if p.suffix != ".sha256")
                    prefix = f"earthcraft-{version}-{release.TARGETS[target]}/"
                    self.assertEqual(
                        archive.name,
                        prefix.rstrip("/") + (".tar.gz" if "linux" in target else ".zip"),
                    )
                    if "linux" in target:
                        with tarfile.open(archive) as contents:
                            executable = contents.getmember(prefix + "earthcraft")
                            self.assertEqual(executable.mode & 0o777, 0o755)
                            self.assertEqual(contents.extractfile(executable).read(), binary.read_bytes())
                            self.assertEqual(contents.extractfile(prefix + "CHANGELOG.md").read(), changelog.read_bytes())
                    else:
                        with zipfile.ZipFile(archive) as contents:
                            self.assertEqual(contents.read(prefix + "earthcraft.exe"), binary.read_bytes())
                            self.assertEqual(contents.read(prefix + "CHANGELOG.md"), changelog.read_bytes())
                    expected = hashlib.sha256(archive.read_bytes()).hexdigest()
                    self.assertEqual(
                        archive.with_name(archive.name + ".sha256").read_text(),
                        f"{expected}  {archive.name}\n",
                    )

    def test_version_and_changelog_validation(self):
        with self.assertRaises(ValueError):
            release.validate("v9999.0.0")
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            binary = root / "binary"
            binary.write_bytes(b"fixture")
            changelog = root / "missing.md"
            for text in (None, " \n"):
                if text is not None:
                    changelog.write_text(text, encoding="utf-8")
                with self.assertRaisesRegex(ValueError, "changelog is missing or empty"):
                    release.package(release.workspace_version(), next(iter(release.TARGETS)), binary, root / "out", changelog)


if __name__ == "__main__":
    unittest.main()
