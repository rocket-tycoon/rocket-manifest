# How Zed editor manages tabs: scrolling, preview, and ordering

Zed handles file and terminal tabs through its pane system implemented in `crates/workspace/src/pane.rs`, using GPUI's scroll primitives for overflow management and a preview tab system modeled after VS Code. **The specific mouse-movement-triggered horizontal scrolling behavior described does not exist** - Zed uses standard scroll wheel/trackpad events for tab bar scrolling, not cursor position tracking.

## Tab addition inserts after the active tab

New tabs in Zed are **inserted to the right of the currently active tab**, not at the end of the tab bar. This is controlled through the internal `items: Vec<Box<dyn ItemHandle>>` data structure that stores all tabs in a pane, with `active_item_index` tracking which tab has focus.

When opening a file, the `add_item` function determines placement based on the current active tab position and whether the item is a preview tab. This behavior mirrors VS Code's default but differs from browsers that append new tabs at the far right. GitHub Issue #16587 tracks a feature request for "tabs to open at the end of the tab bar," confirming the current default is next-to-active insertion.

Each pane maintains its own independent tab bar, so when working with split panes, tabs are scoped to their respective pane rather than shared across the workspace.

## Tab overflow uses wheel scrolling, not mouse position tracking

The tab bar implements horizontal scrolling through GPUI's `ScrollHandle` component when tabs exceed the available width. The key architectural elements in `pane.rs` include:

- **`tab_bar_scroll_handle: ScrollHandle`** — manages horizontal scroll state
- **`suppress_scroll: bool`** — prevents scroll conflicts during drag-drop operations
- **`overflow_x_scroll()`** — CSS-style overflow container on the tab bar div

**Scrolling is triggered exclusively by scroll wheel and trackpad gestures**, not mouse cursor position. Users scroll the tab bar by:

1. Using the mouse scroll wheel while hovering over the tab bar
2. Two-finger horizontal swipe gestures on trackpads
3. Programmatic auto-scroll when switching/opening tabs

The programmatic auto-scroll ensures newly opened or switched-to tabs are always visible. This was implemented in PR #3306 to fix Issue #4356, where the tab bar wasn't scrolling to show the active tab when many tabs were open. No evidence exists in the codebase for mouse-position-based scrolling handlers — searches for `mouse_move` combined with scroll operations in the tab bar context yielded no results.

## Preview tabs determine new vs. replaced tabs

Zed's preview tab system, implemented after Issue #4922, determines whether clicking a file opens a new permanent tab or replaces an existing temporary tab:

| Action | Result |
|--------|--------|
| **Single-click** file in Project Panel | Opens in preview mode (replaceable) |
| **Double-click** file | Opens as permanent tab |
| **Edit** a preview tab | Promotes to permanent |
| **Pin** a preview tab | Promotes to permanent |

Preview tabs display **italicized text** in the tab header as a visual indicator. When another file is single-clicked, the preview tab's contents are replaced rather than opening an additional tab, reducing clutter during file browsing.

### Configuration options

```json
"preview_tabs": {
  "enabled": true,
  "enable_preview_from_file_finder": false,
  "enable_preview_from_code_navigation": false
}
```

Setting `enabled` to `false` disables preview tabs entirely — every opened file becomes a permanent tab. The file finder (Cmd+P) and code navigation (Go to Definition) have separate toggles since users often want different behavior for intentional file opening versus quick browsing.

## Terminal tabs follow the same pane model

Terminal tabs in Zed's dock panel use the same underlying pane architecture as file tabs. Terminal-specific fixes include:

- **PR #9221** fixed double-clicking the terminal tab bar to properly open a new terminal instead of a buffer
- **PR #21238** added terminal panel splitting, matching the main editor's split mechanism
- **PR #22013** fixed drag-drop reordering of terminal tabs

The Tab Switcher (Ctrl+Tab) works identically in both contexts, cycling through tabs by most-recently-used order rather than visual position.

## Tab bar configuration settings

Zed exposes several tab bar customizations:

```json
"tab_bar": {
  "show": true,
  "show_nav_history_buttons": true,
  "show_tab_bar_buttons": true
},
"tabs": {
  "close_position": "right",
  "file_icons": false,
  "git_status": false,
  "activate_on_close": "history"
}
```

The `activate_on_close` setting controls which tab receives focus after closing the current one: `"history"` follows most-recently-used order, while `"neighbour"` selects the adjacent tab. Pinned tabs (supported since v0.154.0) remain fixed on the left side of the tab bar and persist across sessions.

## Conclusion

Zed's tab system prioritizes keyboard-driven workflows with features like MRU tab switching and preview tabs for reduced clutter. The tab bar scrolling mechanism relies on **standard scroll events** rather than mouse position tracking — if the mouse-movement scroll behavior was observed, it was likely trackpad gestures being inadvertently triggered. For users wanting end-of-bar tab insertion instead of next-to-active, this remains an open feature request. The preview tab system provides the cleanest answer to the new-versus-replace question: single-click for preview (replaceable), double-click or edit for permanent.
