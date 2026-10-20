# Bundled geographic assets

`ne_110m_admin_0_countries.geojson` and `ne_110m_populated_places.geojson` come
from [Natural Earth](https://www.naturalearthdata.com/) 1:110m cultural vectors,
retrieved through the upstream
[`natural-earth-vector`](https://github.com/nvkelso/natural-earth-vector)
GeoJSON distribution.

- License: public domain. Natural Earth data is free of copyright restrictions
  and may be modified and redistributed without attribution. Attribution is
  still shown in the app because it is good practice, not because it is
  required.
- Retrieved: 2026-10-20, from the `master` branch of `natural-earth-vector`.
- Size on disk: 0.80 MB countries, 0.62 MB places.

They are compiled into `earthcraft-data` with `include_str!` from
`boundaries/sources.rs`. Only the lowest detail level ships with the app;
1:50m and 1:10m stay on the upstream download path.

Do not regenerate or reformat these files casually: the app's startup output and
the `bundled_collections_decode` test depend on the exact feature counts (177
countries, 243 places).
