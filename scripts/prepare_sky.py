"""Prepare the pinned NASA equatorial map offline. Requires FFmpeg; no network I/O."""
import argparse
from array import array
import hashlib
import math
from pathlib import Path
import subprocess
import sys

SOURCE_SHA256 = "2eb802d6e68d170b410f766c7fec07f7518619f6b6708fdc81e9302d93e74fdb"
WIDTH = 4096
HEIGHT = 2048
EXPOSURE = 1.0
MAX_SOURCE_BYTES = 40 * 1024 * 1024


def srgb_byte(linear):
    """Reinhard at fixed exposure, followed by the IEC sRGB transfer function."""
    linear = max(0.0, linear) if math.isfinite(linear) else 0.0
    mapped = linear * EXPOSURE / (1.0 + linear * EXPOSURE)
    encoded = 12.92 * mapped if mapped <= 0.0031308 else 1.055 * mapped ** (1.0 / 2.4) - 0.055
    return round(255 * min(1.0, max(0.0, encoded)))


def prepare(source, output):
    if source.stat().st_size > MAX_SOURCE_BYTES:
        raise ValueError("Source exceeds the preparation limit")
    if hashlib.sha256(source.read_bytes()).hexdigest() != SOURCE_SHA256:
        raise ValueError("Source does not match the pinned NASA 4K celestial map")
    decoded = subprocess.run(["ffmpeg", "-v", "error", "-i", str(source), "-frames:v", "1",
                              "-pix_fmt", "gbrpf32le", "-f", "rawvideo", "pipe:1"],
                             check=True, stdout=subprocess.PIPE).stdout
    if len(decoded) != WIDTH * HEIGHT * 3 * 4:
        raise ValueError("Unexpected decoded dimensions/layout")
    planes = array("f")
    planes.frombytes(decoded)
    del decoded
    if sys.byteorder != "little":
        planes.byteswap()
    pixels = WIDTH * HEIGHT
    rgb = bytearray(pixels * 3)
    for pixel in range(pixels):
        for channel, plane in enumerate((2, 0, 1)):
            rgb[3 * pixel + channel] = srgb_byte(planes[plane * pixels + pixel])
    output.parent.mkdir(parents=True, exist_ok=True)
    subprocess.run(["ffmpeg", "-v", "error", "-y", "-f", "rawvideo", "-pixel_format", "rgb24",
                    "-video_size", f"{WIDTH}x{HEIGHT}", "-i", "pipe:0", "-frames:v", "1",
                    "-pix_fmt", "yuvj444p", "-q:v", "2", "-threads", "1", str(output)],
                   input=rgb, check=True)
    print("Prepared SHA-256:", hashlib.sha256(output.read_bytes()).hexdigest())


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("output", type=Path)
    arguments = parser.parse_args()
    prepare(arguments.source, arguments.output)
