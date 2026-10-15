"""Offline checks of shipped sky provenance and transfer function."""
import hashlib
import json
from pathlib import Path
import unittest
import prepare_sky

ROOT = Path(__file__).resolve().parents[1]


class SkyPreparationTests(unittest.TestCase):
    def test_prepared_asset_matches_manifest(self):
        manifest = json.loads((ROOT / "assets/sky/manifest.json").read_text())
        prepared = manifest["prepared"]
        asset = (ROOT / "assets/sky" / prepared["filename"]).read_bytes()
        self.assertEqual(hashlib.sha256(asset).hexdigest(), prepared["sha256"])
        self.assertEqual(len(asset), prepared["encoded_bytes"])
        self.assertEqual(manifest["source"]["sha256"], prepare_sky.SOURCE_SHA256)
        self.assertEqual(prepared["decoded_rgb_bytes"], prepare_sky.WIDTH * prepare_sky.HEIGHT * 3)

    def test_transfer_is_bounded_monotonic_and_has_explicit_sdr_loss(self):
        self.assertEqual(prepare_sky.srgb_byte(float("nan")), 0)
        self.assertEqual(prepare_sky.srgb_byte(-1), 0)
        ramp = [prepare_sky.srgb_byte(value) for value in [0, 0.001, 0.01, 0.1, 1, 10, 100]]
        self.assertEqual(ramp, sorted(ramp))
        self.assertLess(prepare_sky.srgb_byte(1), 255)
        self.assertEqual(prepare_sky.srgb_byte(0), 0)


if __name__ == "__main__":
    unittest.main()
