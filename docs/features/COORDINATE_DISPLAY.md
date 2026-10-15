# Coordinate display and input

[English](COORDINATE_DISPLAY.md) | [Português (Brasil)](COORDINATE_DISPLAY.pt-BR.md)

Settings → Appearance → Coordinates selects **DD**, **DMS**, **DDM**, **UTM/UPS** or **MGRS**.
The default is DMS with two decimal places in seconds. Precision applies to the
selected format; changing formats selects its normal default precision. Decimal
places affect display and copied text, never stored geographic values or KML.
MGRS digits per axis describe the grid-cell size, not the accuracy of observations.

Cursor, camera-center and globe right-click coordinates use the selection. Click
a bottom-bar readout to copy its coordinates without the localized label. The
cursor slot retains the configured precision and fits its font when necessary;
hover shows the complete value and copy hint. Angular coordinate columns retain
stable comma positions, including when the cursor leaves the globe.

Search recognizes these formats independently of the display preference:

| Format | Example |
| --- | --- |
| DD | `37.7749, -122.4194` |
| DMS | `37°46′29.64″N, 122°25′09.84″W` |
| DDM | `37°46.494′N, 122°25.164′W` |
| UTM | `18 N 585664.121 4511315.422` or `18N 585664.121 4511315.422` |
| UPS | `UPS N 2000000 2000000` |
| MGRS | `18TWL8566411315` |

UTM input uses an explicit **N/S hemisphere**, not a latitude-band letter. Angular
input is latitude first and accepts ASCII quotes as well as prime symbols.
Minutes and seconds must be below 60. Reject conflicting signs/hemispheres,
non-finite values and out-of-range coordinates locally with an English error.
Recognized coordinate queries bypass remote place suggestions/geocoding; ordinary
place names and addresses keep their previous search behavior.

Core owns formats, rounding/carry, parsing and validation. Data tolerantly persists
`coordinate_display` in the existing `settings.json`; old files use the new display
default. The app owns controls, labels, copy actions and fitting. Exact coordinate
entry in the existing ruler editor retains its unrounded decimal values.

UTM/UPS/MGRS conversion uses [mgrs-convert](https://docs.rs/mgrs-convert/0.1.0/mgrs_convert/).
The planning pack suggested geoconvert 1.0.2; local fixed-reference and boundary
tests exposed its rejection of +180° and a panic when formatting valid equatorial
MGRS coordinates. The replacement avoids maintaining a private projection fork.
Tests retain fixed Empire State Building reference values and cover poles,
dateline endpoints, Norway/Svalbard zones, southern/northern hemispheres, DMS
carry, malformed input and persistence compatibility.
