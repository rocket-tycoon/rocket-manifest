//! Custom GPUI Element for rendering the terminal grid.

use gpui::{
    App, Bounds, Element, ElementId, FocusHandle, Font, FontStyle, FontWeight, GlobalElementId,
    Hitbox, HitboxBehavior, Hsla, InspectorElementId, IntoElement, LayoutId, Pixels, Point,
    ShapedLine, Size, StrikethroughStyle, TextAlign, TextRun, UnderlineStyle, Window, fill, point,
    px, size,
};
use itertools::Itertools;
use std::panic::Location;
use terminal::{
    Mode, Terminal, TerminalBounds, TerminalContent,
    mappings::colors::{TerminalColors, convert_color},
};

use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::vte::ansi::CursorShape;

/// Layout state computed during prepaint, used for painting.
pub struct LayoutState {
    #[allow(dead_code)] // Will be used for mouse interaction
    hitbox: Hitbox,
    dimensions: TerminalBounds,
    cursor: Option<CursorLayout>,
    background_rects: Vec<BackgroundRect>,
    text_runs: Vec<TextRunLayout>,
}

struct CursorLayout {
    point: Point<Pixels>,
    size: Size<Pixels>,
    shape: CursorShape,
    text: Option<ShapedLine>,
}

struct BackgroundRect {
    bounds: Bounds<Pixels>,
    color: Hsla,
}

struct TextRunLayout {
    position: Point<Pixels>,
    line: ShapedLine,
}

/// Custom element for rendering the terminal grid.
pub struct TerminalElement {
    terminal: gpui::Entity<Terminal>,
    #[allow(dead_code)] // Will be used for focus tracking
    focus: FocusHandle,
    focused: bool,
}

impl TerminalElement {
    pub fn new(terminal: gpui::Entity<Terminal>, focus: FocusHandle, focused: bool) -> Self {
        TerminalElement {
            terminal,
            focus,
            focused,
        }
    }

    fn layout_grid(
        &self,
        content: &TerminalContent,
        dimensions: &TerminalBounds,
        origin: Point<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) -> (Vec<BackgroundRect>, Vec<TextRunLayout>) {
        let mut background_rects = Vec::new();
        let mut text_runs = Vec::new();

        let line_height = dimensions.line_height();
        let cell_width = dimensions.cell_width();

        let fg_default = TerminalColors::foreground();
        let bg_default = TerminalColors::background();

        // Group cells by line
        for (line_idx, line_cells) in &content.cells.iter().chunk_by(|c| c.point.line.0) {
            let line_cells: Vec<_> = line_cells.collect();
            let y = origin.y + (line_idx as f32) * line_height;

            let mut current_text = String::new();
            let mut current_fg: Hsla = fg_default.into();
            let mut current_start_col = 0i32;
            let mut current_flags = Flags::empty();

            for cell in line_cells {
                let col = cell.point.column.0 as i32;
                let x = origin.x + (col as f32) * cell_width;

                // Get cell colors, respecting INVERSE flag for reverse video
                // TUI apps like Claude Code use reverse video to render their cursors
                let (fg_color, bg_color) = if cell.flags.contains(Flags::INVERSE) {
                    (convert_color(&cell.bg), convert_color(&cell.fg))
                } else {
                    (convert_color(&cell.fg), convert_color(&cell.bg))
                };

                // Draw background if not default
                let bg_rgba: gpui::Rgba = bg_color.into();
                let bg_default_rgba: gpui::Rgba = bg_default.into();
                if bg_rgba != bg_default_rgba {
                    background_rects.push(BackgroundRect {
                        bounds: Bounds {
                            origin: point(x, y),
                            size: size(cell_width, line_height),
                        },
                        color: bg_color,
                    });
                }

                // Check if we need to flush the current text run
                let fg_changed = {
                    let current_rgba: gpui::Rgba = current_fg.into();
                    let new_rgba: gpui::Rgba = fg_color.into();
                    current_rgba != new_rgba
                };

                if fg_changed && !current_text.is_empty() {
                    // Flush current text run
                    if let Some(text_run) = self.shape_text_run(
                        &current_text,
                        current_fg,
                        current_flags,
                        point(origin.x + (current_start_col as f32) * cell_width, y),
                        window,
                        cx,
                    ) {
                        text_runs.push(text_run);
                    }
                    current_text.clear();
                    current_start_col = col;
                }

                if current_text.is_empty() {
                    current_start_col = col;
                    current_fg = fg_color;
                    current_flags = cell.flags;
                }

                // Add character to current run
                let c = cell.c;
                if c != ' ' && c != '\0' {
                    current_text.push(c);
                } else if !current_text.is_empty() {
                    // Space - flush if we have accumulated text
                    if let Some(text_run) = self.shape_text_run(
                        &current_text,
                        current_fg,
                        current_flags,
                        point(origin.x + (current_start_col as f32) * cell_width, y),
                        window,
                        cx,
                    ) {
                        text_runs.push(text_run);
                    }
                    current_text.clear();
                }
            }

            // Flush remaining text
            if !current_text.is_empty() {
                if let Some(text_run) = self.shape_text_run(
                    &current_text,
                    current_fg,
                    current_flags,
                    point(origin.x + (current_start_col as f32) * cell_width, y),
                    window,
                    cx,
                ) {
                    text_runs.push(text_run);
                }
            }
        }

        (background_rects, text_runs)
    }

    fn shape_text_run(
        &self,
        text: &str,
        color: Hsla,
        flags: Flags,
        position: Point<Pixels>,
        window: &mut Window,
        _cx: &mut App,
    ) -> Option<TextRunLayout> {
        if text.is_empty() {
            return None;
        }

        let font_size = px(14.0);
        let font_weight = if flags.contains(Flags::BOLD) {
            FontWeight::BOLD
        } else {
            FontWeight::NORMAL
        };

        let underline = if flags.contains(Flags::UNDERLINE) {
            Some(UnderlineStyle {
                color: Some(color),
                thickness: px(1.0),
                wavy: false,
            })
        } else {
            None
        };

        let strikethrough = if flags.contains(Flags::STRIKEOUT) {
            Some(StrikethroughStyle {
                color: Some(color),
                thickness: px(1.0),
            })
        } else {
            None
        };

        let run = TextRun {
            len: text.len(),
            font: Font {
                family: "Bitstream Vera Sans Mono".into(),
                features: Default::default(),
                fallbacks: None,
                weight: font_weight,
                style: if flags.contains(Flags::ITALIC) {
                    FontStyle::Italic
                } else {
                    FontStyle::Normal
                },
            },
            color,
            background_color: None,
            underline,
            strikethrough,
        };

        let text_string: gpui::SharedString = text.to_string().into();
        let shaped = window.text_system().shape_line(
            text_string,
            font_size,
            &[run],
            None, // force_width
        );

        Some(TextRunLayout {
            position,
            line: shaped,
        })
    }

    fn layout_cursor(
        &self,
        content: &TerminalContent,
        dimensions: &TerminalBounds,
        origin: Point<Pixels>,
        focused: bool,
        window: &mut Window,
        _cx: &mut App,
    ) -> Option<CursorLayout> {
        let cursor = &content.cursor;

        // Respect cursor visibility from terminal mode (DECTCEM: ESC[?25h/l)
        // TUI apps like Claude Code send ESC[?25l to hide the terminal cursor
        // and render their own cursor as styled characters in the grid
        if !content.mode.contains(Mode::SHOW_CURSOR) {
            return None;
        }

        // Also respect Hidden cursor shape
        if cursor.shape == CursorShape::Hidden {
            return None;
        }

        // Convert from buffer coordinates to display coordinates by adding display_offset
        // (same as Zed's DisplayCursor::from pattern)
        let col = cursor.point.column.0 as f32;
        let line = (cursor.point.line.0 + content.display_offset as i32) as f32;

        let x = origin.x + col * dimensions.cell_width();
        let y = origin.y + line * dimensions.line_height();

        let shape = if !focused {
            CursorShape::HollowBlock
        } else {
            cursor.shape
        };

        // Shape the character under the cursor for block cursor
        let text = if shape == CursorShape::Block
            && content.cursor_char != ' '
            && content.cursor_char != '\0'
        {
            let cursor_char = content.cursor_char.to_string();
            let run = TextRun {
                len: cursor_char.len(),
                font: Font {
                    family: "Bitstream Vera Sans Mono".into(),
                    features: Default::default(),
                    fallbacks: None,
                    weight: FontWeight::NORMAL,
                    style: FontStyle::Normal,
                },
                color: TerminalColors::background().into(),
                background_color: None,
                underline: None,
                strikethrough: None,
            };

            Some(
                window
                    .text_system()
                    .shape_line(cursor_char.into(), px(14.0), &[run], None),
            )
        } else {
            None
        };

        Some(CursorLayout {
            point: point(x, y),
            size: size(dimensions.cell_width(), dimensions.line_height()),
            shape,
            text,
        })
    }
}

impl IntoElement for TerminalElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TerminalElement {
    type RequestLayoutState = ();
    type PrepaintState = LayoutState;

    fn id(&self) -> Option<ElementId> {
        Some("terminal-element".into())
    }

    fn source_location(&self) -> Option<&'static Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let layout_id = window.request_layout(
            gpui::Style {
                size: gpui::Size {
                    width: gpui::Length::Definite(gpui::DefiniteLength::Fraction(1.0)),
                    height: gpui::Length::Definite(gpui::DefiniteLength::Fraction(1.0)),
                },
                ..Default::default()
            },
            [],
            cx,
        );
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);

        // Calculate dimensions based on font metrics
        let font_size = px(14.0);
        let line_height = font_size * 1.2; // Standard line height for terminal rendering

        // Measure actual cell width by shaping a reference character
        let measure_run = TextRun {
            len: 1,
            font: Font {
                family: "Bitstream Vera Sans Mono".into(),
                features: Default::default(),
                fallbacks: None,
                weight: FontWeight::NORMAL,
                style: FontStyle::Normal,
            },
            color: gpui::black(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let measured = window
            .text_system()
            .shape_line("M".into(), font_size, &[measure_run], None);
        let cell_width = measured.width;

        // Interior padding
        let padding = px(5.0);
        let padding_left = px(10.0);
        let content_bounds = Bounds {
            origin: point(bounds.origin.x + padding_left, bounds.origin.y + padding),
            size: size(
                bounds.size.width - padding_left - padding,
                bounds.size.height - padding * 2.0,
            ),
        };

        let dimensions = TerminalBounds::new(line_height, cell_width, content_bounds);

        // Update terminal size
        self.terminal.update(cx, |terminal, _cx| {
            terminal.set_size(dimensions);
        });

        // Get terminal content
        let content = self.terminal.read(cx).last_content().clone();

        let origin = content_bounds.origin;

        // Layout grid
        let (background_rects, text_runs) =
            self.layout_grid(&content, &dimensions, origin, window, cx);

        // Layout cursor
        let cursor = self.layout_cursor(&content, &dimensions, origin, self.focused, window, cx);

        LayoutState {
            hitbox,
            dimensions,
            cursor,
            background_rects,
            text_runs,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        // Paint background
        window.paint_quad(fill(bounds, TerminalColors::background()));

        // Paint cell backgrounds
        for rect in &prepaint.background_rects {
            window.paint_quad(fill(rect.bounds, rect.color));
        }

        // Paint text
        for text_run in &prepaint.text_runs {
            text_run
                .line
                .paint(
                    text_run.position,
                    prepaint.dimensions.line_height(),
                    TextAlign::Left,
                    None,
                    window,
                    cx,
                )
                .ok();
        }

        // Paint cursor
        if let Some(cursor) = &prepaint.cursor {
            let cursor_color: Hsla = TerminalColors::cursor().into();

            match cursor.shape {
                CursorShape::Block => {
                    // Solid block
                    window.paint_quad(fill(
                        Bounds {
                            origin: cursor.point,
                            size: cursor.size,
                        },
                        cursor_color,
                    ));

                    // Paint character on top with inverted color
                    if let Some(text) = &cursor.text {
                        text.paint(
                            cursor.point,
                            prepaint.dimensions.line_height(),
                            TextAlign::Left,
                            None,
                            window,
                            cx,
                        )
                        .ok();
                    }
                }
                CursorShape::HollowBlock => {
                    // Outline only
                    let stroke_width = px(1.0);
                    let cursor_bounds = Bounds {
                        origin: cursor.point,
                        size: cursor.size,
                    };

                    // Top
                    window.paint_quad(fill(
                        Bounds {
                            origin: cursor_bounds.origin,
                            size: size(cursor_bounds.size.width, stroke_width),
                        },
                        cursor_color,
                    ));
                    // Bottom
                    window.paint_quad(fill(
                        Bounds {
                            origin: point(
                                cursor_bounds.origin.x,
                                cursor_bounds.origin.y + cursor_bounds.size.height - stroke_width,
                            ),
                            size: size(cursor_bounds.size.width, stroke_width),
                        },
                        cursor_color,
                    ));
                    // Left
                    window.paint_quad(fill(
                        Bounds {
                            origin: cursor_bounds.origin,
                            size: size(stroke_width, cursor_bounds.size.height),
                        },
                        cursor_color,
                    ));
                    // Right
                    window.paint_quad(fill(
                        Bounds {
                            origin: point(
                                cursor_bounds.origin.x + cursor_bounds.size.width - stroke_width,
                                cursor_bounds.origin.y,
                            ),
                            size: size(stroke_width, cursor_bounds.size.height),
                        },
                        cursor_color,
                    ));
                }
                CursorShape::Beam => {
                    // Vertical bar
                    window.paint_quad(fill(
                        Bounds {
                            origin: cursor.point,
                            size: size(px(2.0), cursor.size.height),
                        },
                        cursor_color,
                    ));
                }
                CursorShape::Underline => {
                    // Horizontal bar at bottom
                    window.paint_quad(fill(
                        Bounds {
                            origin: point(
                                cursor.point.x,
                                cursor.point.y + cursor.size.height - px(2.0),
                            ),
                            size: size(cursor.size.width, px(2.0)),
                        },
                        cursor_color,
                    ));
                }
                CursorShape::Hidden => {}
            }
        }
    }
}
