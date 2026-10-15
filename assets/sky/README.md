# NASA diffuse Milky Way derivative

This directory contains the **celestial**, Milky Way-only NASA SVS 4851 map.
`manifest.json` records the exact source URL/hash, preparation, coordinate convention,
credit and usage review. Dataset rights are separate from the MIT application.
SVS permits reuse/redistribution unless otherwise stated; no separate restriction
was identified for this diffuse map. Keep NASA/Gaia credits and avoid endorsement.

The 4096×2048 JPEG is a fixed-exposure, per-channel Reinhard/sRGB **SDR derivative**,
not the original HDR product. Bright Hipparcos/Tycho foreground stars are absent;
we do not add fictional procedural stars over it. The original remains upstream,
not in Git. Regenerate offline with Python and FFmpeg 8.0.1:

```sh
python3 scripts/prepare_sky.py /path/to/milkyway_2020_4k.exr assets/sky/milkyway-2020-4k-sdr.jpg
```

The script verifies the pinned EXR hash before decoding. JPEG bytes can vary with
FFmpeg versions; review a changed derivative and update its manifest explicitly.
Runtime loading performs no network request and only accepts this 4K size under
encoded/decoded limits. RGB residency is 24 MiB, shared across pending frames.

NASA source: https://svs.gsfc.nasa.gov/4851/
Usage: https://svs.gsfc.nasa.gov/help/ and
https://www.nasa.gov/nasa-brand-center/images-and-media/

Celestial transforms use the Rust SOFA port sofars 0.6.1. Its MIT and upstream SOFA
notices are retained in `sofars-LICENSE.txt` and embedded in Settings → Globe.
No claim of IAU/NASA/ESA endorsement is made.
