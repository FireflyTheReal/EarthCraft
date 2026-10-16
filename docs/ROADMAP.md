# v0.2.0 delivery plan

The next release focuses on desktop controls, preferences and display options.
Deliver each slice as a separate pull request. Bump the workspace to **0.2.0 only
after all slices below are implemented and reviewed**, including the celestial
background. This is a scoped delivery plan, not a claim that these features exist.

| Slice | Deliverable |
| --- | --- |
| Sidebar | Explore/Layers/Places horizontal selector, compact resizable panel, pinned/full-height Places, saved layout, shared ruler/placemark shortcuts |
| Settings | Categorized, resizable preferences; existing settings preserved; additional appearance/navigation/cache/display options with real behavior |
| Coordinates | DD, DMS, DDM, UTM/UPS and MGRS display/input, configurable precision and copy behavior; geographic values preserved |
| Native menus | macOS View/Add/Tools/Window/Help and expanded existing menus using shared app actions; equivalent discoverable controls elsewhere |
| Controllers | Gamepad/joystick detection, navigation, remapping/deadzones/inversion and controller settings using shared camera/tool actions |
| Celestial skybox v1 | Source-grounded Milky Way background, enabled/disabled without changing Earth, bounded async loading, CPU/GPU compatibility and credits |
| Release | Final regression/desktop checks, updated release notes and version bump after the preceding work |

Ruler controls belong in the sidebar and are revealed by the existing toolbar
shortcut. Do not add the floating Ruler popup proposed by the original planning
pack. Preserve the existing exact-coordinate editor until its sidebar replacement
is implemented and verified.

The first skybox does not include star catalogs, HiPS streaming or astronomy tools.
Studio, plugin infrastructure, terrain replacement, a general layer registry and
expanded KML geometry/fidelity are separate later milestones. Existing KML limits
remain documented rather than being described as solved by the sidebar work.
