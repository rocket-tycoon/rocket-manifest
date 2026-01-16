use gpui::{App, AppContext, Entity, SharedString, Window};
use gpui_component::input::InputState;
use uuid::Uuid;

/// State for a single feature editor tab.
pub struct FeatureEditorTab {
    /// Unique ID for this tab (for GPUI element IDs).
    pub id: usize,
    /// The feature being edited.
    pub feature_id: Uuid,
    /// Feature title (displayed in tab).
    pub title: String,
    /// Original content from server (for dirty detection).
    /// Stored as SharedString to enable zero-allocation comparison with input value.
    pub original_content: SharedString,
    /// True if content has been modified since last save.
    pub is_dirty: bool,
    /// Input state entity for the text editor.
    pub input_state: Entity<InputState>,
}

impl FeatureEditorTab {
    /// Create a new tab for editing a feature.
    pub fn new(
        id: usize,
        feature_id: Uuid,
        title: String,
        details: Option<String>,
        window: &mut Window,
        cx: &mut App,
    ) -> Self {
        let content = details.unwrap_or_default();

        // Simple multi-line textarea (like GitHub issue editor)
        let input_state = cx.new(|cx| {
            InputState::new(window, cx)
                .multi_line(true)
                .default_value(content.clone())
        });

        Self {
            id,
            feature_id,
            title,
            original_content: content.into(),
            is_dirty: false,
            input_state,
        }
    }

    /// Get the full content as a string.
    pub fn content(&self, cx: &App) -> String {
        self.input_state.read(cx).value().to_string()
    }

    /// Get the content as a SharedString for rendering.
    pub fn content_shared(&self, cx: &App) -> SharedString {
        self.input_state.read(cx).value()
    }

    /// Update the dirty flag based on content comparison.
    /// Uses SharedString comparison to avoid allocations.
    pub fn update_dirty(&mut self, cx: &App) {
        let current = self.input_state.read(cx).value();
        self.is_dirty = current != self.original_content;
    }

    /// Mark content as saved (reset dirty state).
    pub fn mark_saved(&mut self, cx: &App) {
        self.original_content = self.input_state.read(cx).value();
        self.is_dirty = false;
    }

    /// Check if content has changed from original.
    pub fn check_dirty(&self, cx: &App) -> bool {
        self.input_state.read(cx).value() != self.original_content
    }
}
