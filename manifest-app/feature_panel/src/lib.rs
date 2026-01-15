//! Feature Panel - A tree view for displaying Manifest features.

use std::collections::HashSet;

use gpui::{
    div, px, App, Context, EventEmitter, Focusable, FocusHandle,
    InteractiveElement, IntoElement, ParentElement, Render, SharedString,
    StatefulInteractiveElement, Styled, Window, rgba,
};
use manifest_client::{Feature, FeatureState};
use uuid::Uuid;

/// Colors for the feature panel (Pigs in Space theme).
mod colors {
    use gpui::Rgba;

    pub fn header_background() -> Rgba {
        Rgba { r: 0.082, g: 0.098, b: 0.118, a: 1.0 } // #15191e - darker for title bar
    }

    pub fn panel_background() -> Rgba {
        Rgba { r: 0.114, g: 0.133, b: 0.157, a: 1.0 } // #1d2228 - panel.background
    }

    pub fn hover_background() -> Rgba {
        Rgba { r: 0.243, g: 0.275, b: 0.302, a: 1.0 } // #3e464d - element.hover
    }

    pub fn selected_background() -> Rgba {
        Rgba { r: 0.216, g: 0.247, b: 0.278, a: 1.0 } // #373f47 - element.selected
    }

    pub fn text_primary() -> Rgba {
        Rgba { r: 0.761, g: 0.839, b: 0.918, a: 1.0 } // #c2d6ea - text
    }

    pub fn text_muted() -> Rgba {
        Rgba { r: 0.471, g: 0.522, b: 0.608, a: 1.0 } // #78859b - text.muted
    }

    // State icon colors (Pigs in Space theme-aligned)
    pub fn proposed_amber() -> Rgba {
        Rgba { r: 0.973, g: 0.745, b: 0.325, a: 1.0 } // #f8be53 - warning
    }

    pub fn specified_green() -> Rgba {
        Rgba { r: 0.765, g: 0.910, b: 0.553, a: 1.0 } // #c3e88d - success/green
    }

    pub fn implemented_blue() -> Rgba {
        Rgba { r: 0.510, g: 0.667, b: 1.0, a: 1.0 } // #82aaff - info/blue
    }

    pub fn implemented_check() -> Rgba {
        Rgba { r: 0.129, g: 0.149, b: 0.173, a: 1.0 } // #21262c - dark checkmark
    }

    pub fn deprecated_gray() -> Rgba {
        Rgba { r: 0.388, g: 0.431, b: 0.502, a: 1.0 } // #636e80 - text.muted
    }
}

/// Events emitted by the FeaturePanel.
#[derive(Clone, Debug)]
pub enum Event {
    FeatureSelected(Uuid),
}

/// A flattened feature entry for rendering.
#[derive(Clone, Debug)]
struct FlatFeature {
    id: Uuid,
    title: String,
    state: FeatureState,
    depth: usize,
    has_children: bool,
    is_expanded: bool,
}

/// State of data loading.
#[derive(Clone, Debug)]
pub enum LoadState {
    Loading,
    Loaded,
    Error(String),
}

/// GPUI Entity for displaying a feature tree.
pub struct FeaturePanel {
    features: Vec<Feature>,
    expanded_ids: HashSet<Uuid>,
    selected_id: Option<Uuid>,
    focus_handle: FocusHandle,
    load_state: LoadState,
}

impl FeaturePanel {
    /// Create a new empty feature panel.
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            features: Vec::new(),
            expanded_ids: HashSet::new(),
            selected_id: None,
            focus_handle: cx.focus_handle(),
            load_state: LoadState::Loading,
        }
    }

    /// Set the features to display.
    pub fn set_features(&mut self, features: Vec<Feature>, cx: &mut Context<Self>) {
        self.features = features;
        self.load_state = LoadState::Loaded;
        cx.notify();
    }

    /// Set an error state.
    pub fn set_error(&mut self, error: String, cx: &mut Context<Self>) {
        self.load_state = LoadState::Error(error);
        cx.notify();
    }

    /// Toggle expansion of a feature.
    fn toggle_expanded(&mut self, id: Uuid, cx: &mut Context<Self>) {
        if self.expanded_ids.contains(&id) {
            self.expanded_ids.remove(&id);
        } else {
            self.expanded_ids.insert(id);
        }
        cx.notify();
    }

    /// Select a feature.
    fn select_feature(&mut self, id: Uuid, cx: &mut Context<Self>) {
        self.selected_id = Some(id);
        cx.emit(Event::FeatureSelected(id));
        cx.notify();
    }

    /// Flatten the feature tree for rendering.
    fn flatten_features(&self) -> Vec<FlatFeature> {
        let mut result = Vec::new();
        self.flatten_recursive(&self.features, 0, &mut result);
        result
    }

    fn flatten_recursive(&self, features: &[Feature], depth: usize, result: &mut Vec<FlatFeature>) {
        for feature in features {
            let is_expanded = self.expanded_ids.contains(&feature.id);
            let has_children = !feature.children.is_empty();

            result.push(FlatFeature {
                id: feature.id,
                title: feature.title.clone(),
                state: feature.state,
                depth,
                has_children,
                is_expanded,
            });

            // Only include children if expanded
            if is_expanded && has_children {
                self.flatten_recursive(&feature.children, depth + 1, result);
            }
        }
    }

    /// Render a folder icon (Zed-style geometric shape).
    fn render_folder_icon(&self, is_expanded: bool) -> impl IntoElement {
        let folder_color = colors::text_muted();

        if is_expanded {
            // Open folder - 3/4 view with solid front flap
            div()
                .w(px(16.0))
                .h(px(16.0))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .relative()
                        .w(px(14.0))
                        .h(px(12.0))
                        // Tab at top-left (outline)
                        .child(
                            div()
                                .absolute()
                                .top(px(0.0))
                                .left(px(1.0))
                                .w(px(5.0))
                                .h(px(3.0))
                                .rounded_tl(px(1.0))
                                .rounded_tr(px(1.0))
                                .border_1()
                                .border_b_0()
                                .border_color(folder_color)
                        )
                        // Back of folder (outline rectangle)
                        .child(
                            div()
                                .absolute()
                                .top(px(2.0))
                                .left(px(0.0))
                                .w(px(14.0))
                                .h(px(8.0))
                                .rounded(px(1.0))
                                .border_1()
                                .border_color(folder_color)
                        )
                        // Front flap (solid, creates open look)
                        .child(
                            div()
                                .absolute()
                                .top(px(5.0))
                                .left(px(0.0))
                                .w(px(14.0))
                                .h(px(7.0))
                                .rounded(px(1.0))
                                .bg(folder_color)
                        )
                )
        } else {
            // Closed folder - complete outlined folder shape with tab
            div()
                .w(px(16.0))
                .h(px(16.0))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .relative()
                        .w(px(14.0))
                        .h(px(11.0))
                        // Tab at top-left (outline, sticks up)
                        .child(
                            div()
                                .absolute()
                                .top(px(0.0))
                                .left(px(1.0))
                                .w(px(5.0))
                                .h(px(3.0))
                                .rounded_tl(px(1.0))
                                .rounded_tr(px(1.0))
                                .border_1()
                                .border_b_0()
                                .border_color(folder_color)
                        )
                        // Main folder body (outline rectangle)
                        .child(
                            div()
                                .absolute()
                                .top(px(2.0))
                                .left(px(0.0))
                                .w(px(14.0))
                                .h(px(9.0))
                                .rounded(px(1.0))
                                .border_1()
                                .border_color(folder_color)
                        )
                )
        }
    }

    /// Render a proposed state icon: small amber solid circle.
    /// Size: 8px - this becomes the "inner dot" reference for the progression.
    fn render_proposed_icon(&self) -> impl IntoElement {
        div()
            .w(px(16.0))
            .h(px(16.0))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .w(px(8.0))
                    .h(px(8.0))
                    .rounded(px(4.0))
                    .bg(colors::proposed_amber())
            )
    }

    /// Render a specified state icon: green donut/ring.
    /// Outer: 14px (same as implemented), hole: 8px (same as proposed dot).
    fn render_specified_icon(&self) -> impl IntoElement {
        // Border of 3px creates: outer 14px, inner hole 8px (14 - 3*2 = 8)
        div()
            .w(px(16.0))
            .h(px(16.0))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .w(px(14.0))
                    .h(px(14.0))
                    .rounded(px(7.0))
                    .border_3()
                    .border_color(colors::specified_green())
            )
    }

    /// Render an implemented state icon: blue filled circle with checkmark.
    /// Size: 14px (same outer as specified).
    fn render_implemented_icon(&self) -> impl IntoElement {
        div()
            .w(px(16.0))
            .h(px(16.0))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .w(px(14.0))
                    .h(px(14.0))
                    .rounded(px(7.0))
                    .bg(colors::implemented_blue())
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_color(colors::implemented_check())
                            .text_size(px(9.0))
                            .font_weight(gpui::FontWeight::BOLD)
                            .child("✓")
                    )
            )
    }

    /// Render a deprecated state icon: gray archive box.
    fn render_deprecated_icon(&self) -> impl IntoElement {
        div()
            .w(px(16.0))
            .h(px(16.0))
            .flex()
            .items_center()
            .justify_center()
            .child(
                // Simple archive box shape: rectangle with a line near top
                div()
                    .w(px(12.0))
                    .h(px(10.0))
                    .rounded(px(1.0))
                    .border_2()
                    .border_color(colors::deprecated_gray())
            )
    }

    /// Render a single feature row.
    fn render_feature_row(&self, feature: &FlatFeature, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let id = feature.id;
        let is_selected = self.selected_id == Some(id);
        let depth = feature.depth;
        let has_children = feature.has_children;
        let is_expanded = feature.is_expanded;
        let state = feature.state;
        let title: SharedString = feature.title.clone().into();

        // Zed-style indentation: base padding + depth indent
        let indent = px(16.0 * depth as f32);

        let bg_color = if is_selected {
            colors::selected_background()
        } else {
            colors::panel_background()
        };

        // Disclosure triangle for expandable items (Zed-style)
        let disclosure = if has_children {
            if is_expanded { "▾" } else { "▸" }
        } else {
            " " // Space placeholder for alignment
        };

        // Build the icon element based on type
        let icon_element = if has_children {
            self.render_folder_icon(is_expanded).into_any_element()
        } else {
            match state {
                FeatureState::Proposed => self.render_proposed_icon().into_any_element(),
                FeatureState::Specified => self.render_specified_icon().into_any_element(),
                FeatureState::Implemented => self.render_implemented_icon().into_any_element(),
                FeatureState::Deprecated => self.render_deprecated_icon().into_any_element(),
            }
        };

        div()
            .id(SharedString::from(format!("feature-{}", id)))
            .h(px(22.0))
            .pl(indent + px(4.0))
            .pr(px(8.0))
            .flex()
            .items_center()
            .gap(px(2.0))
            .bg(bg_color)
            .hover(|s| s.bg(colors::hover_background()))
            .on_click(cx.listener(move |this, _event, _window, cx| {
                if has_children {
                    this.toggle_expanded(id, cx);
                }
                this.select_feature(id, cx);
            }))
            // Disclosure triangle
            .child(
                div()
                    .w(px(12.0))
                    .text_color(colors::text_muted())
                    .text_size(px(10.0))
                    .child(disclosure)
            )
            // Icon (folder or state)
            .child(icon_element)
            // Title
            .child(
                div()
                    .pl(px(4.0))
                    .text_color(colors::text_primary())
                    .text_size(px(13.0))
                    .overflow_hidden()
                    .child(title)
            )
    }
}

impl EventEmitter<Event> for FeaturePanel {}

impl Focusable for FeaturePanel {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for FeaturePanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let panel_bg = colors::panel_background();

        div()
            .id("feature-panel")
            .w(px(250.0))
            .h_full()
            .bg(panel_bg)
            .border_r_1()
            .border_color(rgba(0x2d333aff)) // border from pigs-in-space
            .flex()
            .flex_col()
            .overflow_hidden()
            .child(
                // Header
                div()
                    .h(px(32.0))
                    .px(px(12.0))
                    .flex()
                    .items_center()
                    .bg(colors::header_background())
                    .border_b_1()
                    .border_color(rgba(0x2d333aff)) // border from pigs-in-space
                    .child(
                        div()
                            .text_color(colors::text_primary())
                            .text_size(px(12.0))
                            .font_weight(gpui::FontWeight::BOLD)
                            .child("MANIFEST")
                    )
            )
            .child(
                // Content (scrollable)
                div()
                    .id("feature-list")
                    .flex_1()
                    .overflow_y_scroll()
                    .child(match &self.load_state {
                        LoadState::Loading => {
                            div()
                                .p(px(12.0))
                                .text_color(colors::text_muted())
                                .text_size(px(13.0))
                                .child("Loading features...")
                                .into_any_element()
                        }
                        LoadState::Error(err) => {
                            div()
                                .p(px(12.0))
                                .text_color(rgba(0xf14c4cff))
                                .text_size(px(13.0))
                                .child(format!("Error: {}", err))
                                .into_any_element()
                        }
                        LoadState::Loaded => {
                            let flat_features = self.flatten_features();
                            let mut rows = Vec::new();
                            for f in &flat_features {
                                rows.push(self.render_feature_row(f, cx));
                            }
                            div()
                                .flex()
                                .flex_col()
                                .children(rows)
                                .into_any_element()
                        }
                    })
            )
    }
}
