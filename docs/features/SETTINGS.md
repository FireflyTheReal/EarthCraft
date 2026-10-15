# Settings

[English](SETTINGS.md) | [Português (Brasil)](SETTINGS.pt-BR.md)

Open Settings from the toolbar or the application menu (Ctrl/Cmd+,). The resizable
window groups working controls into General, Appearance, Navigation, Globe,
Graphics, Data & Cache and Files. Search matches localized category names,
labels and help text; matching categories remain editable. Reset and cache
clearing ask for confirmation inside the window.

Appearance includes System/Dark/Light themes, independent installed UI/map font
families, distance units, coordinate display/precision and text scales. The
existing dark theme remains the default. A bounded frontend worker discovers
font metadata and reads selected faces; bundled fonts remain fallback glyphs.
Missing fonts report an English diagnostic and fall back without changing the
saved family name. Font changes invalidate cached map text.

Navigation adjusts drag pan, heading rotation and wheel zoom sensitivity, and
can invert wheel zoom. Defaults preserve previous controls. Save the current
camera as the startup view, or restore the original startup camera. This changes
subsequent startup; it does not move the current view. Existing keyboard and
pinch controls retain their behavior.

The decoded imagery target defaults to 256 MiB and managed disk imagery to
128 MiB. Both budget changes require restarting EarthCraft. Decoded residency is
a best-effort Moka weighted target with temporary eviction overshoot, not a hard
process RAM cap. References held by renders and other datasets are separate.
Disk limits include managed SQLite/file bytes. Clearing retains the existing
asynchronous, epoch-safe implementation and preserves unrelated files.

Preferences remain in the same settings.json and language file, written through
the coalescing persistence worker. Old/malformed nested fields retain defaults
or the current valid selection; numeric input is bounded. Changing appearance
does not modify source coordinates, KML or existing documents.

Automated coverage checks localized search, fallback font-family separation,
round trips, independent malformed fields, startup limits and actual cache
constructor budgets. Visual layout and installed-font quality need desktop review.
