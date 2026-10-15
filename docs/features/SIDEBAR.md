# Sidebar panels

[English](SIDEBAR.md) | [Português (Brasil)](SIDEBAR.pt-BR.md)

The left navigation rail switches between **Explore**, **Layers** and **Places**.
Clicking the active rail button collapses its panel; the rail stays available.
The toolbar **☰** button also closes/reopens the panel. The panel-header **☰**
button hides the rail independently; text tabs keep panel switching available.

- **Explore** contains navigation, search results, ruler controls, screenshot
  export and controls help. The toolbar ruler toggles the same measurement state
  and reveals these controls. Measurements and undo/redo survive panel switches.
- **Layers** contains base imagery/Wayback and the existing geographic/live
  overlays, with their status, credentials and attribution.
- **Places** contains the existing editable folder tree and its file actions.
  The outline grows with the panel instead of retaining a 128-point viewport.
  Open, Save and Save As stay visible; path/drop-zone options are under KML / KMZ
  files. Double-click, context menus, visibility and drag-to-folder keep working.

Use **Pin** in Places to retain its outline below Explore or Layers. Drag the
horizontal divider to resize that area. Selecting Places gives it the full panel
without duplicating the tree. Unpin from its header to reclaim the space.
The toolbar placemark shortcut reveals the existing name editor in Places,
including when pinned. Sidebar and pinned Places each scroll independently.

Panel choice, open state, rail visibility, pin state, width and pinned height
are stored as `sidebar` in the existing `settings.json`. Old files default to
Explore with an open sidebar and visible rail; invalid/unknown layout fields keep
valid defaults or current values. Writes use the existing coalesced worker.

This is the first sidebar slice for v0.2.0. It does not add future rail entries,
change KML fidelity, implement a new layer registry, or introduce a floating ruler.
The existing exact-coordinate ruler editor remains available from the sidebar.
Places search, tree virtualization and a separate object inspector are later work.
