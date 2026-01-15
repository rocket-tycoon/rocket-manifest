//! Manifest Terminal Application
//!
//! A GPUI application with a feature explorer panel and terminal.

use gpui::{
    actions, App, Application, Bounds, Context, Entity, Focusable,
    KeyBinding, Menu, MenuItem, ParentElement, Render, Styled,
    TitlebarOptions, Window, WindowBounds, WindowOptions, div, point,
    prelude::*, px, size,
};
use feature_panel::FeaturePanel;
use manifest_client::ManifestClient;
use terminal::mappings::colors::TerminalColors;
use terminal_view::TerminalView;

actions!(app, [Quit, Open, OpenRecent]);

/// Set up the application menus.
fn set_menus(cx: &mut App) {
    cx.set_menus(vec![
        Menu {
            name: "Manifest".into(),
            items: vec![
                MenuItem::action("Quit Manifest", Quit),
            ],
        },
        Menu {
            name: "File".into(),
            items: vec![
                MenuItem::action("Open...", Open),
                MenuItem::action("Open Recent", OpenRecent),
            ],
        },
    ]);
}

/// Root application view with feature panel and terminal.
struct ManifestApp {
    feature_panel: Entity<FeaturePanel>,
    terminal_view: Entity<TerminalView>,
}

impl ManifestApp {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let feature_panel = cx.new(|cx| FeaturePanel::new(cx));
        let terminal_view = cx.new(|cx| TerminalView::new(window, cx));

        // Focus the terminal on startup
        let focus_handle = terminal_view.focus_handle(cx);
        focus_handle.focus(window, cx);

        // Fetch features in background, then update panel
        let feature_panel_clone = feature_panel.clone();
        let background_executor = cx.background_executor().clone();
        cx.spawn(async move |_this, cx| {
            // Run blocking HTTP calls on background thread
            let result = background_executor.spawn(async move {
                Self::fetch_features()
            }).await;

            // Update panel on main thread using AsyncApp::update_entity
            match result {
                Ok(features) => {
                    eprintln!("Loaded {} features", features.len());
                    cx.update_entity(&feature_panel_clone, |panel, cx| {
                        panel.set_features(features, cx);
                    });
                }
                Err(e) => {
                    eprintln!("Failed to load features: {}", e);
                    cx.update_entity(&feature_panel_clone, |panel, cx| {
                        panel.set_error(e, cx);
                    });
                }
            }
        }).detach();

        ManifestApp {
            feature_panel,
            terminal_view,
        }
    }

    /// Fetch features from the Manifest API (blocking, runs on background thread).
    fn fetch_features() -> Result<Vec<manifest_client::Feature>, String> {
        let client = ManifestClient::localhost();

        // Try to get project by directory path first
        let project_path = "/Users/alastair/Documents/work/rocket-tycoon/RocketManifest";

        if let Ok(Some(project)) = client.get_project_by_directory(project_path) {
            eprintln!("Found project '{}' for directory", project.name);
            match client.get_feature_tree(&project.id) {
                Ok(features) => {
                    eprintln!("Loaded {} features from '{}'", features.len(), project.name);
                    return Ok(features);
                }
                Err(e) => {
                    eprintln!("Error fetching features: {}", e);
                }
            }
        }

        // Fallback: find first project with features
        let projects = client.get_projects()
            .map_err(|e| format!("Failed to fetch projects: {}", e))?;

        for project in &projects {
            match client.get_feature_tree(&project.id) {
                Ok(features) if !features.is_empty() => {
                    eprintln!("Found {} features in project '{}'", features.len(), project.name);
                    return Ok(features);
                }
                Ok(_) => continue,
                Err(e) => {
                    eprintln!("Error fetching features for '{}': {}", project.name, e);
                }
            }
        }

        Err("No projects with features found".into())
    }
}

impl Render for ManifestApp {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let bg_color: gpui::Hsla = TerminalColors::background().into();

        div()
            .id("manifest-app")
            .size_full()
            .bg(bg_color)
            .flex()
            .flex_row()
            .child(self.feature_panel.clone())
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .child(self.terminal_view.clone())
            )
    }
}

fn main() {
    Application::new().run(|cx: &mut App| {
        // Register global actions
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.on_action(|_: &Open, _cx| {
            eprintln!("Open action triggered");
            // TODO: Implement file open dialog
        });
        cx.on_action(|_: &OpenRecent, _cx| {
            eprintln!("Open Recent action triggered");
            // TODO: Implement recent files submenu
        });

        // Set up application menus
        set_menus(cx);

        // Bind keys
        cx.bind_keys([
            KeyBinding::new("cmd-q", Quit, None),
            KeyBinding::new("cmd-o", Open, None),
        ]);

        // Open the main window
        let window_options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: point(px(100.0), px(100.0)),
                size: size(px(1200.0), px(800.0)),
            })),
            titlebar: Some(TitlebarOptions {
                title: Some("Manifest".into()),
                appears_transparent: false,
                ..Default::default()
            }),
            focus: true,
            show: true,
            kind: gpui::WindowKind::Normal,
            is_movable: true,
            app_id: Some("com.manifest.app".into()),
            ..Default::default()
        };

        cx.open_window(window_options, |window, cx| {
            cx.new(|cx| ManifestApp::new(window, cx))
        })
        .ok();

        cx.activate(true);
    });
}
