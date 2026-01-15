use std::ops::Range;

use gpui::{
    actions, div, prelude::*, px, App, Bounds, ClipboardItem, Context,
    CursorStyle, ElementId, EntityInputHandler, FocusHandle, Focusable, Hsla, KeyBinding,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, ScrollHandle,
    ScrollWheelEvent, UTF16Selection, Window,
};
use manifest_client::ManifestClient;
use uuid::Uuid;

use crate::editor_tab::{CursorPosition, FeatureEditorTab};
use crate::text_input::TextLayoutInfo;
use crate::scrollbar::ScrollbarState;

// Define editor actions
actions!(
    feature_editor,
    [
        Backspace,
        Delete,
        Left,
        Right,
        Up,
        Down,
        SelectLeft,
        SelectRight,
        SelectUp,
        SelectDown,
        SelectAll,
        Home,
        End,
        PageUp,
        PageDown,
        DocumentStart,
        DocumentEnd,
        Paste,
        Cut,
        Copy,
        Save,
        NewLine,
        CloseTab,
        NextTab,
        PrevTab,
    ]
);

/// Events emitted by the FeatureEditor.
#[derive(Clone, Debug)]
pub enum Event {
    /// Tab was saved successfully.
    FeatureSaved(Uuid),
    /// Save failed with error message.
    SaveFailed(Uuid, String),
    /// Request to close a dirty tab (needs confirmation).
    DirtyCloseRequested(usize),
}

/// Colors for the editor (Pigs in Space theme).
mod colors {
    use gpui::Hsla;

    pub fn background() -> Hsla {
        Hsla { h: 210.0 / 360.0, s: 0.13, l: 0.15, a: 1.0 }
    }

    pub fn text() -> Hsla {
        Hsla { h: 210.0 / 360.0, s: 0.45, l: 0.84, a: 1.0 }
    }

    pub fn text_muted() -> Hsla {
        Hsla { h: 215.0 / 360.0, s: 0.12, l: 0.45, a: 1.0 }
    }

    pub fn tab_active_bg() -> Hsla {
        Hsla { h: 210.0 / 360.0, s: 0.13, l: 0.15, a: 1.0 }
    }

    pub fn tab_inactive_bg() -> Hsla {
        Hsla { h: 212.0 / 360.0, s: 0.15, l: 0.10, a: 1.0 }
    }

    pub fn tab_bar_bg() -> Hsla {
        Hsla { h: 212.0 / 360.0, s: 0.15, l: 0.10, a: 1.0 }
    }

    pub fn border() -> Hsla {
        Hsla { h: 210.0 / 360.0, s: 0.10, l: 0.25, a: 1.0 }
    }

    pub fn dirty_indicator() -> Hsla {
        // Blue accent for dirty state
        Hsla { h: 220.0 / 360.0, s: 1.0, l: 0.75, a: 1.0 }
    }

    pub fn hover() -> Hsla {
        Hsla { h: 212.0 / 360.0, s: 0.12, l: 0.28, a: 1.0 }
    }
}

/// Multi-tab feature editor view.
pub struct FeatureEditor {
    /// Open tabs.
    tabs: Vec<FeatureEditorTab>,
    /// Currently active tab index.
    active_tab_idx: usize,
    /// Counter for generating unique tab IDs.
    next_tab_id: usize,
    /// Focus handle for keyboard input.
    focus_handle: FocusHandle,
    /// API client for saving.
    client: ManifestClient,
    /// Cached layout info for the active tab's text.
    last_layout: Option<TextLayoutInfo>,
    /// Whether user is currently selecting with mouse.
    is_selecting: bool,
    /// Visible height for scroll calculations (updated during render).
    visible_height: f32,
    /// Scrollbar interaction state.
    scrollbar_state: ScrollbarState,
    /// Scroll handle for horizontal tab bar scrolling (like Zed).
    tab_bar_scroll_handle: ScrollHandle,
}

impl FeatureEditor {
    /// Create a new empty editor.
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            tabs: Vec::new(),
            active_tab_idx: 0,
            next_tab_id: 0,
            focus_handle: cx.focus_handle(),
            client: ManifestClient::localhost(),
            last_layout: None,
            is_selecting: false,
            visible_height: 300.0,
            scrollbar_state: ScrollbarState::default(),
            tab_bar_scroll_handle: ScrollHandle::new(),
        }
    }

    /// Open a feature in a new tab (or focus existing tab if already open).
    pub fn open_feature(
        &mut self,
        feature_id: Uuid,
        title: String,
        details: Option<String>,
        cx: &mut Context<Self>,
    ) {
        // Check if already open
        if let Some(idx) = self.tabs.iter().position(|t| t.feature_id == feature_id) {
            self.switch_tab(idx, cx);
            return;
        }

        // Create new tab
        let tab = FeatureEditorTab::new(self.next_tab_id, feature_id, title, details);
        self.next_tab_id += 1;
        self.tabs.push(tab);
        self.active_tab_idx = self.tabs.len() - 1;
        // Clear cached layout to force recalculation for new tab
        self.last_layout = None;
        self.scrollbar_state = ScrollbarState::default();
        // Scroll tab bar to show the new tab
        self.tab_bar_scroll_handle.scroll_to_item(self.active_tab_idx);
        cx.notify();
    }

    /// Check if a feature is already open.
    pub fn is_feature_open(&self, feature_id: &Uuid) -> bool {
        self.tabs.iter().any(|t| &t.feature_id == feature_id)
    }

    /// Get the active tab, if any.
    pub fn active_tab(&self) -> Option<&FeatureEditorTab> {
        self.tabs.get(self.active_tab_idx)
    }

    /// Get the active tab mutably, if any.
    pub fn active_tab_mut(&mut self) -> Option<&mut FeatureEditorTab> {
        self.tabs.get_mut(self.active_tab_idx)
    }

    /// Check if the editor has any open tabs.
    pub fn has_tabs(&self) -> bool {
        !self.tabs.is_empty()
    }

    /// Save the current tab's content to the server.
    pub fn save_current(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        // Extract data first to avoid borrow issues
        let (feature_id, content, is_dirty) = {
            let Some(tab) = self.active_tab() else { return };
            (tab.feature_id, tab.content(), tab.is_dirty)
        };

        if !is_dirty { return; }

        let client = self.client.clone();

        // Mark as saved optimistically (will revert on error)
        if let Some(tab) = self.active_tab_mut() {
            tab.mark_saved();
        }
        cx.notify();

        // Save in background
        cx.background_executor()
            .spawn(async move {
                client.update_feature(&feature_id, Some(content))
            })
            .detach_and_log_err(cx);

        cx.emit(Event::FeatureSaved(feature_id));
    }

    /// Close the active tab (with dirty check handled by caller).
    pub fn close_active_tab(&mut self, cx: &mut Context<Self>) {
        if self.tabs.is_empty() { return; }

        // Check for dirty state
        if let Some(tab) = self.active_tab() {
            if tab.is_dirty {
                cx.emit(Event::DirtyCloseRequested(self.active_tab_idx));
                return;
            }
        }

        self.force_close_tab(self.active_tab_idx, cx);
    }

    /// Close a tab by index without checking dirty state.
    pub fn force_close_tab(&mut self, idx: usize, cx: &mut Context<Self>) {
        if idx >= self.tabs.len() { return; }

        self.tabs.remove(idx);

        // Adjust active index
        if self.tabs.is_empty() {
            self.active_tab_idx = 0;
        } else if self.active_tab_idx >= self.tabs.len() {
            self.active_tab_idx = self.tabs.len() - 1;
        } else if idx < self.active_tab_idx {
            self.active_tab_idx -= 1;
        }

        cx.notify();
    }

    /// Switch to a specific tab.
    fn switch_tab(&mut self, idx: usize, cx: &mut Context<Self>) {
        if idx < self.tabs.len() && idx != self.active_tab_idx {
            self.active_tab_idx = idx;
            // Clear cached layout to force recalculation for new tab
            self.last_layout = None;
            self.scrollbar_state = ScrollbarState::default();
            // Scroll tab bar to show the selected tab
            self.tab_bar_scroll_handle.scroll_to_item(idx);
            cx.notify();
        }
    }

    /// Switch to the next tab.
    fn next_tab(&mut self, cx: &mut Context<Self>) {
        if self.tabs.len() > 1 {
            self.active_tab_idx = (self.active_tab_idx + 1) % self.tabs.len();
            self.last_layout = None;
            self.scrollbar_state = ScrollbarState::default();
            self.tab_bar_scroll_handle.scroll_to_item(self.active_tab_idx);
            cx.notify();
        }
    }

    /// Switch to the previous tab.
    fn prev_tab(&mut self, cx: &mut Context<Self>) {
        if self.tabs.len() > 1 {
            self.active_tab_idx = if self.active_tab_idx == 0 {
                self.tabs.len() - 1
            } else {
                self.active_tab_idx - 1
            };
            self.last_layout = None;
            self.scrollbar_state = ScrollbarState::default();
            self.tab_bar_scroll_handle.scroll_to_item(self.active_tab_idx);
            cx.notify();
        }
    }

    // --- Action handlers ---

    fn on_backspace(&mut self, _: &Backspace, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(tab) = self.active_tab_mut() {
            tab.backspace();
            cx.notify();
        }
    }

    fn on_delete(&mut self, _: &Delete, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(tab) = self.active_tab_mut() {
            tab.delete();
            cx.notify();
        }
    }

    fn on_left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(tab) = self.active_tab_mut() {
            tab.move_left();
            cx.notify();
        }
    }

    fn on_right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(tab) = self.active_tab_mut() {
            tab.move_right();
            cx.notify();
        }
    }

    fn on_up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(tab) = self.active_tab_mut() {
            tab.move_up();
            self.ensure_cursor_visible();
            cx.notify();
        }
    }

    fn on_down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(tab) = self.active_tab_mut() {
            tab.move_down();
            self.ensure_cursor_visible();
            cx.notify();
        }
    }

    fn on_select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(tab) = self.active_tab_mut() {
            tab.start_selection();
            tab.move_left();
            cx.notify();
        }
    }

    fn on_select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(tab) = self.active_tab_mut() {
            tab.start_selection();
            tab.move_right();
            cx.notify();
        }
    }

    fn on_select_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(tab) = self.active_tab_mut() {
            tab.start_selection();
            tab.move_up();
            self.ensure_cursor_visible();
            cx.notify();
        }
    }

    fn on_select_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(tab) = self.active_tab_mut() {
            tab.start_selection();
            tab.move_down();
            self.ensure_cursor_visible();
            cx.notify();
        }
    }

    fn on_select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(tab) = self.active_tab_mut() {
            tab.select_all();
            cx.notify();
        }
    }

    fn on_home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(tab) = self.active_tab_mut() {
            tab.move_to_line_start();
            cx.notify();
        }
    }

    fn on_end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(tab) = self.active_tab_mut() {
            tab.move_to_line_end();
            cx.notify();
        }
    }

    fn on_document_start(&mut self, _: &DocumentStart, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(tab) = self.active_tab_mut() {
            tab.move_to_start();
            self.ensure_cursor_visible();
            cx.notify();
        }
    }

    fn on_document_end(&mut self, _: &DocumentEnd, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(tab) = self.active_tab_mut() {
            tab.move_to_end();
            self.ensure_cursor_visible();
            cx.notify();
        }
    }

    fn on_paste(&mut self, _: &Paste, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            if let Some(tab) = self.active_tab_mut() {
                tab.insert_text(&text);
                self.ensure_cursor_visible();
                cx.notify();
            }
        }
    }

    fn on_copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(tab) = self.active_tab() {
            if let Some(range) = tab.selected_range() {
                let content = tab.content();
                if let Some(selected) = content.get(range) {
                    cx.write_to_clipboard(ClipboardItem::new_string(selected.to_string()));
                }
            }
        }
    }

    fn on_cut(&mut self, _: &Cut, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(tab) = self.active_tab() {
            if let Some(range) = tab.selected_range() {
                let content = tab.content();
                if let Some(selected) = content.get(range.clone()) {
                    cx.write_to_clipboard(ClipboardItem::new_string(selected.to_string()));
                }
            }
        }
        if let Some(tab) = self.active_tab_mut() {
            tab.delete_selection();
            cx.notify();
        }
    }

    fn on_save(&mut self, _: &Save, window: &mut Window, cx: &mut Context<Self>) {
        self.save_current(window, cx);
    }

    fn on_newline(&mut self, _: &NewLine, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(tab) = self.active_tab_mut() {
            tab.insert_newline();
            self.ensure_cursor_visible();
            cx.notify();
        }
    }

    fn on_close_tab(&mut self, _: &CloseTab, _: &mut Window, cx: &mut Context<Self>) {
        self.close_active_tab(cx);
    }

    fn on_next_tab(&mut self, _: &NextTab, _: &mut Window, cx: &mut Context<Self>) {
        self.next_tab(cx);
    }

    fn on_prev_tab(&mut self, _: &PrevTab, _: &mut Window, cx: &mut Context<Self>) {
        self.prev_tab(cx);
    }

    // --- Mouse handlers ---

    fn on_mouse_down(&mut self, event: &MouseDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        if event.button != MouseButton::Left { return; }

        self.is_selecting = true;
        let pos = self.position_for_point(event.position);

        if let Some(tab) = self.active_tab_mut() {
            if event.modifiers.shift {
                tab.start_selection();
            } else {
                tab.clear_selection();
            }
            tab.cursor = pos;
            cx.notify();
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if !self.is_selecting { return; }

        let pos = self.position_for_point(event.position);
        if let Some(tab) = self.active_tab_mut() {
            if tab.selection_anchor.is_none() {
                tab.selection_anchor = Some(tab.cursor);
            }
            tab.cursor = pos;
            cx.notify();
        }
    }

    fn on_scroll_wheel(&mut self, event: &ScrollWheelEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(layout) = self.last_layout else { return };
        let visible_height = self.visible_height;

        // Get line count before mutable borrow
        let line_count = self.active_tab().map(|t| t.line_count()).unwrap_or(0);

        let Some(tab) = self.active_tab_mut() else { return };

        // Get scroll delta (convert Pixels to f32)
        let delta_y = event.delta.pixel_delta(px(layout.line_height)).y / px(1.0);

        // Update scroll offset (negative delta = scroll down, show more content below)
        let new_offset = (tab.scroll_offset - delta_y).max(0.0);

        // Calculate max scroll (total content height - visible height)
        let total_height = line_count as f32 * layout.line_height;
        let max_scroll = (total_height - visible_height).max(0.0);

        tab.scroll_offset = new_offset.min(max_scroll);
        cx.notify();
    }

    /// Convert a screen point to a cursor position.
    fn position_for_point(&self, point: gpui::Point<Pixels>) -> CursorPosition {
        let Some(layout) = self.last_layout else {
            return CursorPosition::default();
        };
        let Some(tab) = self.active_tab() else {
            return CursorPosition::default();
        };

        // Convert Pixels to f32 using division by px(1.0)
        let point_x = point.x / px(1.0);
        let point_y = point.y / px(1.0);

        // Calculate line from y position (relative to content area)
        let relative_y = (point_y - layout.content_origin_y).max(0.0);
        let line_from_top = (relative_y / layout.line_height).floor() as usize;
        let line = layout.first_visible_line + line_from_top;
        let line = line.min(tab.line_count().saturating_sub(1));

        // Calculate column from x position
        let relative_x = (point_x - layout.content_origin_x).max(0.0);
        let column = (relative_x / layout.char_width).round() as usize;
        let line_len = tab.lines.get(line).map(|s| s.len()).unwrap_or(0);
        let column = column.min(line_len);

        CursorPosition { line, column }
    }

    /// Ensure the cursor is visible by adjusting scroll offset.
    fn ensure_cursor_visible(&mut self) {
        let Some(layout) = self.last_layout else { return };
        let visible_height = self.visible_height;
        let Some(tab) = self.active_tab_mut() else { return };

        let cursor_y = tab.cursor.line as f32 * layout.line_height;

        if cursor_y < tab.scroll_offset {
            tab.scroll_offset = cursor_y;
        } else if cursor_y + layout.line_height > tab.scroll_offset + visible_height {
            tab.scroll_offset = cursor_y + layout.line_height - visible_height;
        }
    }

    /// Store layout info for mouse calculations.
    pub fn set_layout_info(&mut self, info: TextLayoutInfo) {
        self.last_layout = Some(info);
    }

    /// Set the visible height for scroll calculations.
    pub fn set_visible_height(&mut self, height: f32) {
        self.visible_height = height;
    }
}

impl Focusable for FeatureEditor {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl gpui::EventEmitter<Event> for FeatureEditor {}

impl EntityInputHandler for FeatureEditor {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let tab = self.active_tab()?;
        let content = tab.content();

        // Convert UTF-16 range to UTF-8
        let range = utf16_to_utf8_range(&content, &range_utf16);
        actual_range.replace(utf8_to_utf16_range(&content, &range));
        content.get(range).map(|s| s.to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let tab = self.active_tab()?;
        let content = tab.content();

        let cursor_offset = tab.position_to_offset(tab.cursor);
        let range = if let Some(anchor) = tab.selection_anchor {
            let anchor_offset = tab.position_to_offset(anchor);
            let start = cursor_offset.min(anchor_offset);
            let end = cursor_offset.max(anchor_offset);
            start..end
        } else {
            cursor_offset..cursor_offset
        };

        Some(UTF16Selection {
            range: utf8_to_utf16_range(&content, &range),
            reversed: tab.selection_anchor.map(|a| a > tab.cursor).unwrap_or(false),
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        let tab = self.active_tab()?;
        let content = tab.content();
        tab.marked_range.as_ref().map(|r| utf8_to_utf16_range(&content, r))
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        if let Some(tab) = self.active_tab_mut() {
            tab.marked_range = None;
        }
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(tab) = self.active_tab_mut() else { return };
        let content = tab.content();

        let range = range_utf16
            .as_ref()
            .map(|r| utf16_to_utf8_range(&content, r))
            .or(tab.marked_range.clone())
            .unwrap_or_else(|| tab.selected_range().unwrap_or(tab.position_to_offset(tab.cursor)..tab.position_to_offset(tab.cursor)));

        // Replace text
        let mut new_content = String::with_capacity(content.len() + new_text.len());
        new_content.push_str(&content[..range.start]);
        new_content.push_str(new_text);
        new_content.push_str(&content[range.end..]);

        let new_cursor_offset = range.start + new_text.len();
        tab.set_content(&new_content);
        tab.cursor = tab.offset_to_position(new_cursor_offset);
        tab.clear_selection();
        tab.marked_range = None;

        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(tab) = self.active_tab_mut() else { return };
        let content = tab.content();

        let range = range_utf16
            .as_ref()
            .map(|r| utf16_to_utf8_range(&content, r))
            .or(tab.marked_range.clone())
            .unwrap_or_else(|| tab.selected_range().unwrap_or(tab.position_to_offset(tab.cursor)..tab.position_to_offset(tab.cursor)));

        // Replace text
        let mut new_content = String::with_capacity(content.len() + new_text.len());
        new_content.push_str(&content[..range.start]);
        new_content.push_str(new_text);
        new_content.push_str(&content[range.end..]);

        tab.set_content(&new_content);

        // Set marked range
        if !new_text.is_empty() {
            tab.marked_range = Some(range.start..range.start + new_text.len());
        } else {
            tab.marked_range = None;
        }

        // Set selection/cursor
        if let Some(sel_range) = new_selected_range_utf16 {
            let sel = utf16_to_utf8_range(&new_content, &sel_range);
            let adjusted_start = range.start + sel.start;
            let adjusted_end = range.start + sel.end;
            tab.cursor = tab.offset_to_position(adjusted_end);
            if adjusted_start != adjusted_end {
                tab.selection_anchor = Some(tab.offset_to_position(adjusted_start));
            }
        } else {
            tab.cursor = tab.offset_to_position(range.start + new_text.len());
            tab.clear_selection();
        }

        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let tab = self.active_tab()?;
        let layout = self.last_layout?;
        let content = tab.content();

        let range = utf16_to_utf8_range(&content, &range_utf16);
        let start_pos = tab.offset_to_position(range.start);
        let end_pos = tab.offset_to_position(range.end);

        // Use the bounds origin plus layout offsets
        let start_x = bounds.origin.x + px(layout.content_origin_x + start_pos.column as f32 * layout.char_width);
        let end_x = bounds.origin.x + px(layout.content_origin_x + end_pos.column as f32 * layout.char_width);
        let y = bounds.origin.y + px(layout.content_origin_y + (start_pos.line - layout.first_visible_line) as f32 * layout.line_height);

        Some(Bounds::from_corners(
            gpui::point(start_x, y),
            gpui::point(end_x, y + px(layout.line_height)),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: gpui::Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let tab = self.active_tab()?;
        let pos = self.position_for_point(point);
        let offset = tab.position_to_offset(pos);
        let content = tab.content();
        Some(utf8_to_utf16_offset(&content, offset))
    }
}

impl Render for FeatureEditor {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Collect tab data to avoid borrow issues
        let tab_data: Vec<_> = self.tabs
            .iter()
            .enumerate()
            .map(|(idx, tab)| (idx, tab.id, tab.title.clone(), tab.is_dirty, idx == self.active_tab_idx))
            .collect();

        let has_tabs = !self.tabs.is_empty();

        div()
            .id("feature-editor")
            .size_full()
            .flex()
            .flex_col()
            .bg(colors::background())
            .track_focus(&self.focus_handle)
            .key_context("FeatureEditor")
            .on_action(cx.listener(Self::on_backspace))
            .on_action(cx.listener(Self::on_delete))
            .on_action(cx.listener(Self::on_left))
            .on_action(cx.listener(Self::on_right))
            .on_action(cx.listener(Self::on_up))
            .on_action(cx.listener(Self::on_down))
            .on_action(cx.listener(Self::on_select_left))
            .on_action(cx.listener(Self::on_select_right))
            .on_action(cx.listener(Self::on_select_up))
            .on_action(cx.listener(Self::on_select_down))
            .on_action(cx.listener(Self::on_select_all))
            .on_action(cx.listener(Self::on_home))
            .on_action(cx.listener(Self::on_end))
            .on_action(cx.listener(Self::on_document_start))
            .on_action(cx.listener(Self::on_document_end))
            .on_action(cx.listener(Self::on_paste))
            .on_action(cx.listener(Self::on_copy))
            .on_action(cx.listener(Self::on_cut))
            .on_action(cx.listener(Self::on_save))
            .on_action(cx.listener(Self::on_newline))
            .on_action(cx.listener(Self::on_close_tab))
            .on_action(cx.listener(Self::on_next_tab))
            .on_action(cx.listener(Self::on_prev_tab))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .cursor(CursorStyle::IBeam)
            // Tab bar
            .child(self.render_tab_bar(tab_data, cx))
            // Content area (scroll handler here so tab bar can scroll horizontally)
            .child(
                div()
                    .id("editor-content-area")
                    .flex_1()
                    .w_full()
                    .overflow_hidden()
                    .on_scroll_wheel(cx.listener(Self::on_scroll_wheel))
                    .child(if has_tabs {
                        div()
                            .id("editor-content-wrapper")
                            .size_full()
                            .child(self.render_editor_content(cx))
                    } else {
                        div()
                            .id("editor-empty-state")
                            .size_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .text_color(colors::text_muted())
                                    .child("No features open")
                            )
                    })
            )
    }
}

impl FeatureEditor {
    fn render_tab_bar(&self, tab_data: Vec<(usize, usize, String, bool, bool)>, cx: &mut Context<Self>) -> impl IntoElement {
        let can_close = self.tabs.len() > 0;

        // Tab bar container - fixed height, full width
        div()
            .id("tab-bar")
            .h(px(32.0))
            .w_full()
            .flex_shrink_0()
            .overflow_hidden()  // Constrain children to tab bar width
            .flex()
            .flex_row()
            .bg(colors::tab_bar_bg())
            .border_b_1()
            .border_color(colors::border())
            // Wrapper: flex item that takes remaining space (constrained by parent)
            .child(
                div()
                    .id("tab-scroll-wrapper")
                    .flex_1()
                    .flex_shrink()
                    .flex_basis(px(0.0))  // Start at 0, grow to fill - don't size from content
                    .min_w(px(0.0))       // Override content-based minimum width
                    .h_full()
                    .overflow_hidden()
                    .border_1()
                    .border_color(Hsla { h: 0.0, s: 1.0, l: 0.5, a: 1.0 })  // Red debug border
                    // Inner scroll container: fills wrapper, scrolls content
                    .child(
                        div()
                            .id("tab-scroll-container")
                            .size_full()
                            .overflow_x_scroll()
                            .track_scroll(&self.tab_bar_scroll_handle)
                            // Tabs row: can be wider than container, will scroll
                            .child(
                                div()
                                    .id("tabs-row")
                                    .h_full()
                                    .flex()
                                    .flex_row()
                                    .children(tab_data.into_iter().map(|(idx, _id, title, is_dirty, is_active)| {
                        let bg_color = if is_active { colors::tab_active_bg() } else { colors::tab_inactive_bg() };
                        let text_color = if is_active { colors::text() } else { colors::text_muted() };

                        div()
                            .id(ElementId::Name(format!("tab-{}", idx).into()))
                            .h_full()
                            .flex_shrink_0()  // Don't shrink tabs - scroll instead
                            .px(px(12.0))
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(6.0))
                            .bg(bg_color)
                            .border_r_1()
                            .border_color(colors::border())
                            .when(!is_active, |el| el.border_b_1())
                            .hover(|s| s.bg(colors::hover()))
                            .cursor(CursorStyle::PointingHand)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.switch_tab(idx, cx);
                            }))
                            // Dirty indicator (blue dot)
                            .when(is_dirty, |el| {
                                el.child(
                                    div()
                                        .w(px(6.0))
                                        .h(px(6.0))
                                        .rounded_full()
                                        .bg(colors::dirty_indicator())
                                )
                            })
                            // Tab title
                            .child(
                                div()
                                    .text_color(text_color)
                                    .text_sm()
                                    .text_ellipsis()
                                    .child(title)
                            )
                            // Close button
                            .when(can_close, |el| {
                                el.child(
                                    div()
                                        .id(ElementId::Name(format!("close-tab-{}", idx).into()))
                                        .w(px(16.0))
                                        .h(px(16.0))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .rounded(px(3.0))
                                        .text_color(colors::text_muted())
                                        .hover(|s| s.bg(colors::hover()).text_color(colors::text()))
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            // Check dirty state before closing
                                            if let Some(tab) = this.tabs.get(idx) {
                                                if tab.is_dirty {
                                                    cx.emit(Event::DirtyCloseRequested(idx));
                                                } else {
                                                    this.force_close_tab(idx, cx);
                                                }
                                            }
                                        }))
                                        .child("×")
                                )
                            })
                                    }))  // closes tabs-row .children()
                            )  // closes tabs-row div
                    )  // closes scroll-container div
            )  // closes wrapper div
            // Spacer: fixed width to keep scroll area 100px from right edge
            .child(
                div()
                    .w(px(100.0))
                    .h_full()
            )
    }

    fn render_editor_content(&self, cx: &mut Context<Self>) -> impl IntoElement {
        use crate::text_input::MultiLineTextElement;

        MultiLineTextElement {
            editor: cx.entity().clone(),
        }
    }
}

// --- UTF-8/UTF-16 conversion helpers ---

fn utf8_to_utf16_offset(text: &str, utf8_offset: usize) -> usize {
    text[..utf8_offset.min(text.len())]
        .chars()
        .map(|c| c.len_utf16())
        .sum()
}

fn utf16_to_utf8_offset(text: &str, utf16_offset: usize) -> usize {
    let mut utf8_offset = 0;
    let mut utf16_count = 0;

    for ch in text.chars() {
        if utf16_count >= utf16_offset {
            break;
        }
        utf16_count += ch.len_utf16();
        utf8_offset += ch.len_utf8();
    }

    utf8_offset
}

fn utf8_to_utf16_range(text: &str, range: &Range<usize>) -> Range<usize> {
    utf8_to_utf16_offset(text, range.start)..utf8_to_utf16_offset(text, range.end)
}

fn utf16_to_utf8_range(text: &str, range: &Range<usize>) -> Range<usize> {
    utf16_to_utf8_offset(text, range.start)..utf16_to_utf8_offset(text, range.end)
}

/// Register key bindings for the feature editor.
pub fn register_bindings(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, Some("FeatureEditor")),
        KeyBinding::new("delete", Delete, Some("FeatureEditor")),
        KeyBinding::new("left", Left, Some("FeatureEditor")),
        KeyBinding::new("right", Right, Some("FeatureEditor")),
        KeyBinding::new("up", Up, Some("FeatureEditor")),
        KeyBinding::new("down", Down, Some("FeatureEditor")),
        KeyBinding::new("shift-left", SelectLeft, Some("FeatureEditor")),
        KeyBinding::new("shift-right", SelectRight, Some("FeatureEditor")),
        KeyBinding::new("shift-up", SelectUp, Some("FeatureEditor")),
        KeyBinding::new("shift-down", SelectDown, Some("FeatureEditor")),
        KeyBinding::new("cmd-a", SelectAll, Some("FeatureEditor")),
        KeyBinding::new("home", Home, Some("FeatureEditor")),
        KeyBinding::new("end", End, Some("FeatureEditor")),
        KeyBinding::new("cmd-up", DocumentStart, Some("FeatureEditor")),
        KeyBinding::new("cmd-down", DocumentEnd, Some("FeatureEditor")),
        KeyBinding::new("cmd-v", Paste, Some("FeatureEditor")),
        KeyBinding::new("cmd-c", Copy, Some("FeatureEditor")),
        KeyBinding::new("cmd-x", Cut, Some("FeatureEditor")),
        KeyBinding::new("cmd-s", Save, Some("FeatureEditor")),
        KeyBinding::new("enter", NewLine, Some("FeatureEditor")),
        KeyBinding::new("cmd-w", CloseTab, Some("FeatureEditor")),
        KeyBinding::new("ctrl-tab", NextTab, Some("FeatureEditor")),
        KeyBinding::new("ctrl-shift-tab", PrevTab, Some("FeatureEditor")),
    ]);
}
