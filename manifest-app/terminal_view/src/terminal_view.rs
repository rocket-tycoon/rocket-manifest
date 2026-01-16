//! TerminalView - GPUI view container for multiple terminal tabs.

use gpui::{
    App, AsyncWindowContext, Context, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, KeyDownEvent, ModifiersChangedEvent, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Render, Styled, WeakEntity,
    Window, div, prelude::*, px,
};
use gpui_component::ActiveTheme;
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

/// GPUI view that contains and renders multiple terminal tabs.
pub struct TerminalView {
    tabs: Vec<TerminalTab>,
    active_tab_idx: usize,
    next_tab_id: usize,
    focus_handle: FocusHandle,
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
            cx.notify();
        }
    }

    /// Switch to the next tab, wrapping around to the first tab.
    fn next_tab(&mut self, cx: &mut Context<Self>) {
        if self.tabs.len() > 1 {
            self.active_tab_idx = (self.active_tab_idx + 1) % self.tabs.len();
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
                    TerminalEvent::OpenUrl(url) => {
                        // Open URL in default browser
                        if let Err(e) = open::that(url) {
                            eprintln!("Failed to open URL: {}", e);
                        }
                    }
                }
            },
        )
        .detach();
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(tab) = self.tabs.get(self.active_tab_idx) {
            if let Some(terminal) = &tab.terminal {
                // Get terminal bounds to calculate position relative to content area
                let bounds_origin = terminal
                    .read(cx)
                    .last_content()
                    .terminal_bounds
                    .bounds
                    .origin;

                // Convert window position to position relative to terminal content origin
                let content_position = gpui::point(
                    event.position.x - bounds_origin.x,
                    event.position.y - bounds_origin.y,
                );

                terminal.update(cx, |terminal, _cx| {
                    terminal.mouse_down(event.button, content_position, event.modifiers);
                });
            }
        }
    }

    fn on_mouse_up(&mut self, event: &MouseUpEvent, _window: &mut Window, cx: &mut Context<Self>) {
        if let Some(tab) = self.tabs.get(self.active_tab_idx) {
            if let Some(terminal) = &tab.terminal {
                // Get terminal bounds to calculate position relative to content area
                let bounds_origin = terminal
                    .read(cx)
                    .last_content()
                    .terminal_bounds
                    .bounds
                    .origin;

                // Convert window position to position relative to terminal content origin
                let content_position = gpui::point(
                    event.position.x - bounds_origin.x,
                    event.position.y - bounds_origin.y,
                );

                terminal.update(cx, |terminal, cx| {
                    terminal.mouse_up(event.button, content_position, event.modifiers, cx);
                });
            }
        }
    }

    fn on_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(tab) = self.tabs.get(self.active_tab_idx) {
            if let Some(terminal) = &tab.terminal {
                // Get terminal bounds to calculate position relative to content area
                let bounds_origin = terminal
                    .read(cx)
                    .last_content()
                    .terminal_bounds
                    .bounds
                    .origin;

                // Convert window position to position relative to terminal content origin
                let content_position = gpui::point(
                    event.position.x - bounds_origin.x,
                    event.position.y - bounds_origin.y,
                );

                terminal.update(cx, |terminal, _cx| {
                    terminal.mouse_move(content_position, event.modifiers);
                });
                cx.notify();
            }
        }
    }

    fn on_modifiers_changed(
        &mut self,
        event: &ModifiersChangedEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // When Cmd is released, clear the hyperlink hover state
        if !event.modifiers.platform {
            if let Some(tab) = self.tabs.get(self.active_tab_idx) {
                if let Some(terminal) = &tab.terminal {
                    terminal.update(cx, |terminal, _cx| {
                        terminal.clear_hovered_hyperlink();
                    });
                    cx.notify();
                }
            }
        }
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
    fn render_terminal_content(&self, window: &mut Window, cx: &App) -> impl IntoElement {
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
                    .text_color(cx.theme().muted_foreground)
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
                .text_color(cx.theme().muted_foreground)
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
        // Render terminal content
        let terminal_content = self.render_terminal_content(window, cx);
        let can_close = self.tabs.len() > 1;
        let border_color = cx.theme().border;
        let tab_bar_bg = cx.theme().tab_bar;
        let tab_active_bg = cx.theme().tab_active;
        let text_color = cx.theme().tab_foreground;
        let text_active_color = cx.theme().tab_active_foreground;
        let hover_bg = cx.theme().secondary_hover;

        // Build custom tab bar
        let mut tabs_container = div().flex().items_center().h(px(32.0));

        for (idx, tab) in self.tabs.iter().enumerate() {
            let close_idx = idx;
            let is_selected = idx == self.active_tab_idx;
            let tab_bg = if is_selected {
                tab_active_bg
            } else {
                tab_bar_bg
            };
            let tab_text = if is_selected {
                text_active_color
            } else {
                text_color
            };

            let tab_element = div()
                .id(idx)
                .h_full()
                .flex()
                .items_center()
                .px(px(4.0))
                .bg(tab_bg)
                .border_1()
                .border_color(border_color)
                .when(is_selected, |d| d.border_b_0())
                .text_color(tab_text)
                .font_family("IBM Plex Sans")
                .text_size(px(13.0))
                .cursor_pointer()
                .when(!is_selected, |d| d.hover(|s| s.bg(hover_bg)))
                .on_click(cx.listener(move |this, _, _window, cx| {
                    this.switch_tab(idx, cx);
                }))
                // Terminal icon prefix
                .child(
                    div()
                        .pl(px(8.0))
                        .pr(px(6.0))
                        .text_size(px(11.0))
                        .text_color(cx.theme().muted_foreground)
                        .child(">_"),
                )
                // Tab label
                .child(div().px(px(4.0)).child(tab.title.clone()))
                // Close button (if closeable)
                .when(can_close, |d| {
                    d.child(
                        div()
                            .id(format!("close-{}", idx))
                            .ml(px(4.0))
                            .mr(px(4.0))
                            .w(px(16.0))
                            .h(px(16.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(3.0))
                            .text_size(px(14.0))
                            .text_color(cx.theme().muted_foreground)
                            .hover(|s| {
                                s.bg(cx.theme().list_hover)
                                    .text_color(cx.theme().foreground)
                            })
                            .cursor_pointer()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.close_tab(close_idx, cx);
                            }))
                            .child("×"),
                    )
                });

            tabs_container = tabs_container.child(tab_element);
        }

        // Add "+" button
        let add_button = div()
            .id("add-terminal")
            .px(px(8.0))
            .h_full()
            .flex()
            .items_center()
            .font_family("IBM Plex Sans")
            .text_size(px(16.0))
            .text_color(cx.theme().muted_foreground)
            .hover(|s| s.text_color(cx.theme().foreground))
            .cursor_pointer()
            .on_click(cx.listener(|this, _, window, cx| {
                this.add_tab(window, cx);
            }))
            .child("+");

        let tab_bar = div()
            .id("terminal-tabs")
            .w_full()
            .h(px(32.0))
            .flex()
            .items_center()
            .bg(tab_bar_bg)
            .child(tabs_container)
            .child(add_button);

        div()
            .id("terminal-view")
            .size_full()
            .flex()
            .flex_col()
            .track_focus(&self.focus_handle)
            .key_context("Terminal")
            .on_key_down(cx.listener(Self::on_key_down))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_modifiers_changed(cx.listener(Self::on_modifiers_changed))
            // Tab bar
            .child(tab_bar)
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
