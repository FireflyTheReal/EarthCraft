# Sidebar panels

[English](SIDEBAR.md) | [Português (Brasil)](SIDEBAR.pt-BR.md)

A compact horizontal icon selector inside the sidebar switches between **Globe**,
**Layers** and **Places**. Hover for panel names. Selecting the active item keeps
it open. The toolbar **☰** button collapses/reopens the entire sidebar, including
its selector. There is no separate rail visibility setting or header close button.
The default width is 240 points instead of a 270-point panel plus a 44-point rail;
manual resizing remains available and saved widths are preserved.

- **Globe** contains navigation and auto-rotate, search results, ruler controls,
  screenshot export and controls help. The toolbar ruler toggles the same
  measurement state and reveals these controls. Measurements/history survive
  panel switches.
- **Layers** groups controls into imagery, borders/labels/grid, roads/traffic,
  weather, aircraft/ships/satellites and sky/lighting. Each group can collapse;
  its existing status, credentials and attribution remain with its controls.
- **Places** contains the editable folder tree and file actions. Open/Save/Save As
  remain visible. **KML / KMZ files starts expanded** in a reserved bottom area;
  a slightly shorter, independently scrolling placemark viewer cannot push it
  off-screen. In short pinned panels the file area also scrolls. Double-click,
  context menus, visibility, name/description editing and drag-to-folder remain.

Use **Pin** in Places to retain its outline below Globe or Layers. Drag the
horizontal divider to resize that area. Selecting Places gives it the full panel
without duplicating the tree. Use the same Pin control to unpin. The toolbar
placemark shortcut reveals/focuses the existing name editor, including when pinned.

Panel choice, open state, pin state, width and pinned height are stored as
`sidebar` in the existing settings.json. Old files default to an open Globe
panel; invalid fields retain valid defaults/current values. The obsolete
rail_visible field is ignored on read and no longer written. The existing
coalesced writer and geographic/document formats are preserved.

This first v0.2.0 slice does not introduce future panels, a layer registry or a
floating ruler. The exact-coordinate ruler editor remains available. Places
search, tree virtualization and a separate object inspector are later work.
