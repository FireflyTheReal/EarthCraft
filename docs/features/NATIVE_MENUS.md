# Desktop menus

[English](NATIVE_MENUS.md) | [Português (Brasil)](NATIVE_MENUS.pt-BR.md)

macOS uses the native menu bar. Windows and Linux expose the same command catalog
through the **EarthCraft** button in the top bar; native menu integration remains
macOS-only.

| Menu | Available commands |
| --- | --- |
| EarthCraft | About, Settings, Quit; macOS also retains Hide/Hide Others/Show All |
| File | Open KML/KMZ, Save, Save As, Save Screenshot |
| Edit | Undo and Redo, respecting focused text editors |
| View | Sidebar, Explore/Layers/Places panels, navigation controls, grid, atmosphere, sunlight, north-up, global view |
| Add | Placemark at camera center, Folder in the selected folder or My Places |
| Tools | Measure distances, Explorer |
| Window | Fullscreen, Minimize; macOS registers the system Window menu |
| Help | Controls/shortcuts, documentation, issue reporting; macOS registers the system Help menu |

View/tool checkmarks follow the current app state, including changes made in the
sidebar or Settings. Screenshot is disabled until a displayed image exists.
Undo/Redo enablement follows the ruler history or a focused text editor. The
ruler action toggles the existing measurement state and reveals its controls;
placemark/folder actions reveal the existing name editor.

Settings uses Cmd+, on macOS and Ctrl+, elsewhere. Add Placemark uses
Cmd/Ctrl+Shift+P, Add Folder uses Cmd/Ctrl+Shift+N, and Sidebar uses
Cmd/Ctrl+Alt+B. File/history shortcuts remain unchanged. Help lists the shortcut
catalog with platform-specific modifier names.

Callbacks enqueue typed actions and request repaint. The app performs state
changes, native dialogs and file submissions during update on the window thread.
Menu objects stay on that thread and are removed on shutdown. Both catalogs
localize captions, window labels and help. Links open only when clicked.

These menus expose working commands. Future project/tour/geometry editors and
other planned tools are not represented by inactive placeholders. Explore/Layers/Places open directly from View without clearing pin state.
