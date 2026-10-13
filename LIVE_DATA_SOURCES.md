# EarthCraft — live imagery credits and use

**Satellite clouds:** Matt Eason, Live Cloud Maps (CC0-1.0). Latest source:
https://clouds.matteason.co.uk/images/8192x4096/clouds-alpha.png
Repository: https://github.com/matteason/live-cloud-maps
**Required source attribution:** Contains modified EUMETSAT data

The provider publishes eight updates per day (one every three hours), depending on upstream satellite data availability.

**Precipitation radar:** RainViewer public Weather Maps API, https://www.rainviewer.com/
API docs: https://www.rainviewer.com/api/weather-maps-api.html
The free API, as of 2026, offers radar tiles (up to zoom 7) in color scheme 2, Universal Blue. EarthCraft remaps colors to a green/yellow/red rain-intensity visualization. It is **not** satellite infrared temperature. Radar coverage is not global and may be unavailable in some regions. Last published observations can be delayed.

API free use is limited to personal, educational, and small-community uses. For commercial distribution arrange licensing with the provider.

**Flight markers:** simulated routes, not real live ADS-B.

Implementation notes: HTTP calls happen asynchronously; the world imagery remains rendered even if sources are offline; app displays status and image credits; clouds refresh every three hours and radar every ten minutes.
