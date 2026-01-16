//! TerminalView - GPUI view container for multiple terminal tabs.

use gpui::{
    App, AsyncWindowContext, Context, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Render, Rgba, ScrollHandle,
    SharedString, StatefulInteractiveElement, Styled, WeakEntity, Window, div, prelude::*, px,
};
use terminal::{
    Event as TerminalEvent, Terminal, TerminalBuilder, mappings::colors::TerminalColors,
};

use crate::TerminalElement;

/// Events emitted by the TerminalView.
#[derive(Clone, Debug)]
pub enum Event {
    TitleChanged,
    Closed,
}

/// A single terminal tab.
struct TerminalTab {
    id: usize,
    title: String,
    terminal: Option<Entity<Terminal>>,
}

/// Tab bar colors (Pigs in Space theme).
mod colors {
    use gpui::Rgba;

    pub fn tab_bar_bg() -> Rgba {
        Rgba {
            r: 0.082,
            g: 0.098,
            b: 0.118,
            a: 1.0,
        } // #15191e - darker than panel for contrast
    }

    pub fn active_tab_bg() -> Rgba {
        Rgba {
            r: 0.129,
            g: 0.149,
            b: 0.173,
            a: 1.0,
        } // #21262c - terminal background
    }

    pub fn hover_bg() -> Rgba {
        Rgba {
            r: 0.243,
            g: 0.275,
            b: 0.302,
            a: 1.0,
        } // #3e464d
    }

    pub fn border() -> Rgba {
        Rgba {
            r: 0.176,
            g: 0.2,
            b: 0.227,
            a: 1.0,
        } // #2d333a
    }

    pub fn text() -> Rgba {
        Rgba {
            r: 0.761,
            g: 0.839,
            b: 0.918,
            a: 1.0,
        } // #c2d6ea
    }

    pub fn text_muted() -> Rgba {
        Rgba {
            r: 0.471,
            g: 0.522,
            b: 0.608,
            a: 1.0,
        } // #78859b
    }
}

/// GPUI view that contains and renders multiple terminal tabs.
pub struct TerminalView {
    tabs: Vec<TerminalTab>,
    active_tab_idx: usize,
    next_tab_id: usize,
    focus_handle: FocusHandle,
    /// Scroll handle for horizontal tab bar scrolling (like Zed).
    tab_bar_scroll_handle: ScrollHandle,
}

impl TerminalView {
    /// Create a new terminal view with a single terminal tab.
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();

        let mut view = TerminalView {
            tabs: Vec::new(),
            active_tab_idx: 0,
            next_tab_id: 0,
            focus_handle,
            tab_bar_scroll_handle: ScrollHandle::new(),
        };

        // Create the first tab
        view.create_tab_internal(window, cx);
        view
    }

    /// Create a terminal view from an existing Terminal entity.
    pub fn from_terminal(terminal: Entity<Terminal>, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();

        let mut view = TerminalView {
            tabs: Vec::new(),
            active_tab_idx: 0,
            next_tab_id: 1,
            focus_handle,
            tab_bar_scroll_handle: ScrollHandle::new(),
        };

        let tab = TerminalTab {
            id: 0,
            title: "Terminal".to_string(),
            terminal: Some(terminal.clone()),
        };
        view.tabs.push(tab);
        view.subscribe_to_terminal(0, &terminal, cx);
        view
    }

    /// Add a new terminal tab and switch to it.
    fn add_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.create_tab_internal(window, cx);
        self.tab_bar_scroll_handle
            .scroll_to_item(self.active_tab_idx);
        cx.notify();
    }

    /// Internal method to create a new tab.
    fn create_tab_internal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let tab_id = self.next_tab_id;
        self.next_tab_id += 1;

        let tab = TerminalTab {
            id: tab_id,
            title: "Terminal".to_string(),
            terminal: None,
        };
        self.tabs.push(tab);
        self.active_tab_idx = self.tabs.len() - 1;

        // Spawn the terminal creation in the background
        let task = cx
            .background_executor()
            .spawn(async { TerminalBuilder::new(None, 0) });

        let tab_idx = self.tabs.len() - 1;
        cx.spawn_in(
            window,
            async move |this: WeakEntity<Self>, cx: &mut AsyncWindowContext| match task.await {
                Ok(builder) => {
                    this.update_in(cx, |this, _window, cx| {
                        let terminal = cx.new(|cx| builder.build(cx));
                        this.subscribe_to_terminal(tab_idx, &terminal, cx);
                        if let Some(tab) = this.tabs.get_mut(tab_idx) {
                            tab.terminal = Some(terminal);
                        }
                        cx.notify();
                    })
                    .ok();
                }
                Err(e) => {
                    eprintln!("Failed to create terminal: {}", e);
                }
            },
        )
        .detach();
    }

    /// Switch to a different tab.
    fn switch_tab(&mut self, idx: usize, cx: &mut Context<Self>) {
        if idx < self.tabs.len() && idx != self.active_tab_idx {
            self.active_tab_idx = idx;
            self.tab_bar_scroll_handle.scroll_to_item(idx);
            cx.notify();
        }
    }

    /// Switch to the next tab, wrapping around to the first tab.
    fn next_tab(&mut self, cx: &mut Context<Self>) {
        if self.tabs.len() > 1 {
            self.active_tab_idx = (self.active_tab_idx + 1) % self.tabs.len();
            self.tab_bar_scroll_handle
                .scroll_to_item(self.active_tab_idx);
            cx.notify();
        }
    }

    /// Switch to the previous tab, wrapping around to the last tab.
    fn prev_tab(&mut self, cx: &mut Context<Self>) {
        if self.tabs.len() > 1 {
            self.active_tab_idx = if self.active_tab_idx == 0 {
                self.tabs.len() - 1
            } else {
                self.active_tab_idx - 1
            };
            self.tab_bar_scroll_handle
                .scroll_to_item(self.active_tab_idx);
            cx.notify();
        }
    }

    /// Close a tab by index.
    fn close_tab(&mut self, idx: usize, cx: &mut Context<Self>) {
        // Don't close the last tab
        if self.tabs.len() <= 1 {
            return;
        }

        if idx < self.tabs.len() {
            self.tabs.remove(idx);

            // Adjust active index if needed
            if self.active_tab_idx >= self.tabs.len() {
                self.active_tab_idx = self.tabs.len().saturating_sub(1);
            } else if self.active_tab_idx > idx {
                self.active_tab_idx -= 1;
            }

            cx.notify();
        }
    }

    fn subscribe_to_terminal(
        &mut self,
        tab_idx: usize,
        terminal: &Entity<Terminal>,
        cx: &mut Context<Self>,
    ) {
        cx.subscribe(
            terminal,
            move |this, _terminal, event: &TerminalEvent, cx| {
                match event {
                    TerminalEvent::Wakeup => {
                        cx.notify();
                    }
                    TerminalEvent::Bell => {
                        // Could play a sound or flash the window
                    }
                    TerminalEvent::TitleChanged => {
                        // Update tab title from terminal
                        if let Some(tab) = this.tabs.get_mut(tab_idx) {
                            // For now just keep "Terminal" - could parse shell title later
                            tab.title = format!("Terminal {}", tab.id + 1);
                        }
                        cx.emit(Event::TitleChanged);
                        cx.notify();
                    }
                    TerminalEvent::CloseTerminal => {
                        this.close_tab(tab_idx, cx);
                        cx.emit(Event::Closed);
                    }
                }
            },
        )
        .detach();
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;

        // Handle Ctrl+Tab to cycle through tabs
        if keystroke.modifiers.control && keystroke.key.as_str() == "tab" {
            if keystroke.modifiers.shift {
                self.prev_tab(cx);
            } else {
                self.next_tab(cx);
            }
            return;
        }

        // Handle Cmd+T to create new terminal tab
        if keystroke.modifiers.platform && keystroke.key.as_str() == "t" {
            self.add_tab(window, cx);
            return;
        }

        if let Some(tab) = self.tabs.get(self.active_tab_idx) {
            if let Some(terminal) = &tab.terminal {
                terminal.update(cx, |terminal, _cx| {
                    terminal.try_keystroke(&event.keystroke);
                });
            }
        }
    }

    /// Render the active terminal content.
    fn render_terminal_content(&self, window: &mut Window) -> impl IntoElement {
        let focused = self.focus_handle.is_focused(window);
        let bg_color: gpui::Hsla = TerminalColors::background().into();

        if let Some(tab) = self.tabs.get(self.active_tab_idx) {
            if let Some(terminal) = &tab.terminal {
                div()
                    .size_full()
                    .bg(bg_color)
                    .child(TerminalElement::new(
                        terminal.clone(),
                        self.focus_handle.clone(),
                        focused,
                    ))
                    .into_any_element()
            } else {
                // Terminal still loading
                div()
                    .size_full()
                    .bg(bg_color)
                    .flex()
                    .items_center()
                    .justify_center()
                    .font_family("IBM Plex Sans")
                    .text_color(colors::text_muted())
                    .child("Starting terminal...")
                    .into_any_element()
            }
        } else {
            // No tabs (shouldn't happen)
            div()
                .size_full()
                .bg(bg_color)
                .flex()
                .items_center()
                .justify_center()
                .font_family("IBM Plex Sans")
                .text_color(colors::text_muted())
                .child("No terminal")
                .into_any_element()
        }
    }
}

impl EventEmitter<Event> for TerminalView {}

impl Focusable for TerminalView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TerminalView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Collect tab data first to avoid borrow issues
        let can_close = self.tabs.len() > 1;
        let tab_data: Vec<(usize, usize, String, bool)> = self
            .tabs
            .iter()
            .enumerate()
            .map(|(idx, tab)| (idx, tab.id, tab.title.clone(), idx == self.active_tab_idx))
            .collect();

        // Render tabs from collected data
        let mut tab_elements = Vec::new();
        for (idx, tab_id, title, is_active) in tab_data {
            let title: SharedString = title.into();

            // Selected tab: terminal bg, no bottom border, bright text
            // Non-selected: panel bg, has bottom border, dimmed text
            let tab_bg = if is_active {
                colors::active_tab_bg()
            } else {
                colors::tab_bar_bg()
            };
            let text_color = if is_active {
                colors::text()
            } else {
                colors::text_muted()
            };
            let icon_color = if is_active {
                colors::text_muted()
            } else {
                colors::text_muted()
            };

            let el = div()
                .id(SharedString::from(format!("tab-{}", tab_id)))
                .h_full()
                .flex_shrink_0() // Don't compress tabs - scroll instead
                .px(px(12.0))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .bg(tab_bg)
                .border_r_1()
                .border_color(colors::border())
                // Non-selected tabs have bottom border to separate from terminal
                .when(!is_active, |el| el.border_b_1())
                // Only show hover state on non-selected tabs
                .when(!is_active, |el| el.hover(|s| s.bg(colors::hover_bg())))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.switch_tab(idx, cx);
                }))
                // Terminal icon
                .child(div().font_family("IBM Plex Sans").text_size(px(12.0)).text_color(icon_color).child(">_"))
                // Title
                .child(
                    div()
                        .font_family("IBM Plex Sans")
                        .text_size(px(13.0))
                        .text_color(text_color)
                        .child(title),
                )
                // Close button
                .when(can_close, |el| {
                    el.child(
                        div()
                            .id(SharedString::from(format!("close-{}", tab_id)))
                            .ml(px(4.0))
                            .w(px(16.0))
                            .h(px(16.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .font_family("IBM Plex Sans")
                            .text_size(px(14.0))
                            .text_color(colors::text_muted())
                            .rounded(px(3.0))
                            .hover(|s| s.bg(colors::hover_bg()).text_color(colors::text()))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.close_tab(idx, cx);
                            }))
                            .child("×"),
                    )
                });
            tab_elements.push(el);
        }

        // Add button (also has bottom border like non-selected tabs)
        let add_button = div()
            .id("add-tab")
            .h_full()
            .px(px(12.0))
            .flex()
            .items_center()
            .border_b_1()
            .border_color(colors::border())
            .font_family("IBM Plex Sans")
            .text_size(px(16.0))
            .text_color(colors::text_muted())
            .hover(|s| s.text_color(colors::text()))
            .on_click(cx.listener(|this, _, window, cx| {
                this.add_tab(window, cx);
            }))
            .child("+");

        // Render terminal content
        let terminal_content = self.render_terminal_content(window);

        div()
            .id("terminal-view")
            .size_full()
            .flex()
            .flex_col()
            .track_focus(&self.focus_handle)
            .key_context("Terminal")
            .on_key_down(cx.listener(Self::on_key_down))
            // Tab bar (no global bottom border - each element manages its own)
            .child(
                div()
                    .id("tab-bar")
                    .h(px(32.0))
                    .w_full()
                    .flex_shrink_0()
                    .overflow_hidden() // Constrain children to tab bar width
                    .flex()
                    .flex_row()
                    .bg(colors::tab_bar_bg())
                    // Wrapper: flex item that takes remaining space (constrained by parent)
                    .child(
                        div()
                            .id("tab-scroll-wrapper")
                            .flex_1()
                            .flex_shrink()
                            .flex_basis(px(0.0)) // Start at 0, grow to fill - don't size from content
                            .min_w(px(0.0)) // Override content-based minimum width
                            .h_full()
                            .overflow_hidden()
                            // Inner scroll container: fills wrapper, scrolls content
                            .child(
                                div()
                                    .id("tab-scroll-container")
                                    .w_full()
                                    .h_full()
                                    .overflow_x_scroll()
                                    .track_scroll(&self.tab_bar_scroll_handle)
                                    .flex()
                                    .flex_row()
                                    .children(tab_elements),
                            ),
                    )
                    .child(add_button)
                    // Right toolbar area - will contain buttons in future
                    .child(
                        div()
                            .w(px(10.0))
                            .h_full()
                            .border_l_1()
                            .border_color(colors::border()),
                    ),
            )
            // Terminal content area
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .overflow_hidden()
                    .child(terminal_content),
            )
    }
}
