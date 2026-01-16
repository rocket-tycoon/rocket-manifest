use gpui::{
    App, Context, FocusHandle, Focusable, KeyBinding, Window, actions, div, prelude::*, px,
};
use gpui_component::{
    ActiveTheme,
    input::{Input, InputEvent},
    tab::{Tab, TabBar},
};
use manifest_client::ManifestClient;
use uuid::Uuid;

use crate::editor_tab::FeatureEditorTab;

// Define editor actions (only tab/file operations - Input handles text editing)
actions!(feature_editor, [Save, CloseTab, NextTab, PrevTab]);

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
        Hsla {
            h: 210.0 / 360.0,
            s: 0.13,
            l: 0.15,
            a: 1.0,
        }
    }

    pub fn dirty_indicator() -> Hsla {
        // Yellow/amber for dirty state (matches Pigs in Space warning color)
        Hsla {
            h: 45.0 / 360.0,
            s: 0.95,
            l: 0.60,
            a: 1.0,
        }
    }
}

/// A feature waiting to be opened (set from async context, opened in render).
#[derive(Clone)]
struct PendingFeature {
    id: Uuid,
    title: String,
    details: Option<String>,
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
    /// Feature pending to be opened (set from async, opened in render with window access).
    pending_feature: Option<PendingFeature>,
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
            pending_feature: None,
        }
    }

    /// Queue a feature to be opened (can be called from async context without window).
    /// The feature will be opened in the next render when window is available.
    pub fn load_feature(
        &mut self,
        feature_id: Uuid,
        title: String,
        details: Option<String>,
        cx: &mut Context<Self>,
    ) {
        // Check if already open - just switch to it
        if let Some(idx) = self.tabs.iter().position(|t| t.feature_id == feature_id) {
            self.switch_tab(idx, cx);
            return;
        }

        // Queue for opening in render (when we have window access)
        self.pending_feature = Some(PendingFeature {
            id: feature_id,
            title,
            details,
        });
        cx.notify();
    }

    /// Open a feature in a new tab (or focus existing tab if already open).
    pub fn open_feature(
        &mut self,
        feature_id: Uuid,
        title: String,
        details: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Check if already open
        if let Some(idx) = self.tabs.iter().position(|t| t.feature_id == feature_id) {
            self.switch_tab(idx, cx);
            return;
        }

        // Check if we should replace the current tab (if it exists and is not dirty)
        let replace_current = if let Some(current_tab) = self.active_tab() {
            !current_tab.is_dirty
        } else {
            false
        };

        // Create new tab
        let tab = FeatureEditorTab::new(self.next_tab_id, feature_id, title, details, window, cx);
        self.next_tab_id += 1;

        // Subscribe to input changes to track dirty state
        // We capture the input entity to find the correct tab even if indices change
        let input_entity = tab.input_state.clone();
        cx.subscribe_in(
            &tab.input_state,
            window,
            move |this, _state, event: &InputEvent, _window, cx| {
                if matches!(event, InputEvent::Change) {
                    if let Some(tab) = this.tabs.iter_mut().find(|t| t.input_state == input_entity)
                    {
                        let was_dirty = tab.is_dirty;
                        tab.update_dirty(cx);
                        if tab.is_dirty != was_dirty {
                            cx.notify();
                        }
                    }
                }
            },
        )
        .detach();

        if replace_current {
            self.tabs[self.active_tab_idx] = tab;
        } else {
            self.tabs.push(tab);
            self.active_tab_idx = self.tabs.len() - 1;
        }

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
            (tab.feature_id, tab.content(cx), tab.is_dirty)
        };

        if !is_dirty {
            return;
        }

        let client = self.client.clone();

        // Mark as saved optimistically (will revert on error)
        if let Some(tab) = self.active_tab_mut() {
            tab.mark_saved(cx);
        }
        cx.notify();

        // Save in background
        cx.background_executor()
            .spawn(async move { client.update_feature(&feature_id, Some(content)) })
            .detach_and_log_err(cx);

        cx.emit(Event::FeatureSaved(feature_id));
    }

    /// Close the active tab (with dirty check handled by caller).
    pub fn close_active_tab(&mut self, cx: &mut Context<Self>) {
        if self.tabs.is_empty() {
            return;
        }

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
        if idx >= self.tabs.len() {
            return;
        }

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
            cx.notify();
        }
    }

    /// Switch to the next tab.
    fn next_tab(&mut self, cx: &mut Context<Self>) {
        if self.tabs.len() > 1 {
            self.active_tab_idx = (self.active_tab_idx + 1) % self.tabs.len();
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
            cx.notify();
        }
    }

    // --- Action handlers ---

    fn on_save(&mut self, _: &Save, window: &mut Window, cx: &mut Context<Self>) {
        self.save_current(window, cx);
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
}

impl Focusable for FeatureEditor {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl gpui::EventEmitter<Event> for FeatureEditor {}

impl Render for FeatureEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Process any pending feature that was queued from async context
        if let Some(pending) = self.pending_feature.take() {
            self.open_feature(pending.id, pending.title, pending.details, window, cx);
        }

        let has_tabs = !self.tabs.is_empty();

        div()
            .id("feature-editor")
            .size_full()
            .flex()
            .flex_col()
            .bg(colors::background())
            .track_focus(&self.focus_handle)
            .key_context("FeatureEditor")
            .on_action(cx.listener(Self::on_save))
            .on_action(cx.listener(Self::on_close_tab))
            .on_action(cx.listener(Self::on_next_tab))
            .on_action(cx.listener(Self::on_prev_tab))
            // Tab bar
            .child(self.render_tab_bar(cx))
            // Content area
            .child(
                div()
                    .id("editor-content-area")
                    .flex_1()
                    .w_full()
                    .overflow_hidden()
                    .child(if has_tabs {
                        self.render_editor_content(cx).into_any_element()
                    } else {
                        div()
                            .id("editor-empty-state")
                            .size_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .font_family("IBM Plex Sans")
                                    .text_color(cx.theme().muted_foreground)
                                    .child("No features open"),
                            )
                            .into_any_element()
                    }),
            )
    }
}

impl FeatureEditor {
    fn render_tab_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let can_close = !self.tabs.is_empty();

        // Build tab bar with gpui-component
        let mut tab_bar = TabBar::new("editor-tabs")
            .w_full()
            .selected_index(self.active_tab_idx)
            .on_click(cx.listener(|this, idx: &usize, _window, cx| {
                this.switch_tab(*idx, cx);
            }));

        // Add tabs with borders
        let border_color = cx.theme().border;
        for (idx, tab) in self.tabs.iter().enumerate() {
            let close_idx = idx;
            let is_dirty = tab.is_dirty;
            let tab_element = Tab::new()
                .outline()
                .label(&tab.title)
                // Dirty indicator (yellow dot)
                .when(is_dirty, |t| {
                    t.prefix(
                        div()
                            .h_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .ml(px(12.0))
                            .w(px(24.0))
                            .child(
                                div()
                                    .w(px(8.0))
                                    .h(px(8.0))
                                    .rounded_full()
                                    .bg(colors::dirty_indicator()),
                            ),
                    )
                })
                // Close button
                .when(can_close, |t| {
                    t.suffix(
                        div()
                            .id(format!("close-{}", idx))
                            .ml(px(4.0))
                            .mr(px(6.0))
                            .w(px(16.0))
                            .h(px(16.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(3.0))
                            .font_family("IBM Plex Sans")
                            .text_size(px(14.0))
                            .text_color(cx.theme().muted_foreground)
                            .hover(|s| {
                                s.bg(cx.theme().list_hover)
                                    .text_color(cx.theme().foreground)
                            })
                            .cursor_pointer()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if let Some(tab) = this.tabs.get(close_idx) {
                                    if tab.is_dirty {
                                        cx.emit(Event::DirtyCloseRequested(close_idx));
                                    } else {
                                        this.force_close_tab(close_idx, cx);
                                    }
                                }
                            }))
                            .child("×"),
                    )
                })
                // Add border to all tabs
                .border_1()
                .border_color(border_color);
            tab_bar = tab_bar.child(tab_element);
        }

        tab_bar
    }

    fn render_editor_content(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        let Some(tab) = self.active_tab() else {
            return div().into_any_element();
        };

        // Use gpui-component Input for text editing
        div()
            .id("editor-content-wrapper")
            .size_full()
            .p(px(16.0))
            .font_family("Bitstream Vera Sans Mono")
            .child(
                Input::new(&tab.input_state)
                    .appearance(false) // No borders
                    .w_full()
                    .h_full(),
            )
            .into_any_element()
    }
}

/// Register key bindings for the feature editor.
pub fn register_bindings(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("cmd-s", Save, Some("FeatureEditor")),
        KeyBinding::new("cmd-w", CloseTab, Some("FeatureEditor")),
        KeyBinding::new("ctrl-tab", NextTab, Some("FeatureEditor")),
        KeyBinding::new("ctrl-shift-tab", PrevTab, Some("FeatureEditor")),
    ]);
}
