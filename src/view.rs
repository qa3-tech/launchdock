use ::image::{ImageBuffer, Rgb, codecs::png::PngEncoder};
use iced::{
    Alignment, Background, Color, Element, Length, Padding, Size,
    daemon::Appearance,
    keyboard,
    widget::{column, container, image, row, scrollable, text, text_input},
    window,
};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

use crate::{
    logs,
    model::{App, AppModel, launch_app},
};

pub fn run_ui(model: AppModel) -> Result<AppModel, Box<dyn std::error::Error>> {
    iced::application("launchdock", update, view)
        .subscription(subscription)
        .transparent(true)
        .style(|_, _| Appearance {
            background_color: iced::Color::TRANSPARENT,
            text_color: iced::Color::WHITE,
        })
        .window(window::Settings {
            size: Size::new(600.0, 530.0),
            position: window::Position::Centered,
            resizable: false,
            decorations: false,
            transparent: true,
            level: window::Level::AlwaysOnTop,
            icon: None,
            exit_on_close_request: true,
            #[cfg(target_os = "macos")]
            platform_specific: window::settings::PlatformSpecific {
                title_hidden: true,
                titlebar_transparent: true,
                fullsize_content_view: false,
            },
            #[cfg(target_os = "linux")]
            platform_specific: window::settings::PlatformSpecific {
                application_id: "launchdock".to_string(),
                override_redirect: false,
            },
            ..Default::default()
        })
        .run_with(move || (AppState::new(model), iced::Task::none()))?;

    Ok(AppModel::default())
}

#[derive(Debug, Clone)]
pub enum Message {
    InputChanged(String),
    KeyPressed(keyboard::Key),
    ForceExit,
}

struct AppState {
    model: AppModel,
    search_query: String,
    selected_index: usize,
    current_filtered_apps: Vec<App>,
}

impl AppState {
    fn new(model: AppModel) -> Self {
        Self {
            model,
            selected_index: 0,
            search_query: String::new(),
            current_filtered_apps: Vec::new(),
        }
    }

    pub fn filtered_apps(&self) -> Vec<&App> {
        if self.search_query.is_empty() {
            return Vec::new();
        }

        let query_lower = self.search_query.to_ascii_lowercase();
        let query_chars: Vec<char> = query_lower.chars().collect();

        let mut matches: Vec<(&App, f32)> = Vec::new();

        for app in &self.model.all_apps {
            let app_name_lower = app.name.to_ascii_lowercase();
            let app_chars: Vec<char> = app_name_lower.chars().collect();

            // Find subsequence positions
            let mut positions = Vec::new();
            let mut app_idx = 0;
            let mut found_all = true;

            for &query_char in &query_chars {
                // Find next occurrence of query_char in app_chars starting from app_idx
                while app_idx < app_chars.len() && app_chars[app_idx] != query_char {
                    app_idx += 1;
                }

                if app_idx >= app_chars.len() {
                    found_all = false;
                    break;
                }

                positions.push(app_idx);
                app_idx += 1; // Move past this match for next search
            }

            if !found_all {
                continue; // Skip if not all query characters found in order
            }

            // Calculate score
            let mut score = 0.0f32;

            // Base score: prefer shorter names but give substantial base points
            score += 1000.0 / (app.name.len() as f32).max(1.0);

            // Character proximity bonus - closer characters get higher score (most important)
            if positions.len() > 1 {
                let total_span = positions.last().unwrap() - positions.first().unwrap() + 1;
                score += 1000.0 / (total_span as f32).max(1.0);
            }

            // Consecutive character bonus
            let mut consecutive_count = 0;
            for window in positions.windows(2) {
                if window[1] - window[0] == 1 {
                    consecutive_count += 1;
                }
            }
            score += consecutive_count as f32 * 200.0;

            // Early match bonus - matches earlier in the string get small bonus
            let first_match_pos = positions[0];
            score += 50.0 / (first_match_pos as f32 + 1.0);

            matches.push((app, score));
        }

        // Sort by score (highest first)
        matches.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        matches.into_iter().map(|(app, _)| app).collect()
    }
}

const DISPLAY_COUNT: usize = 7;

fn update(state: &mut AppState, message: Message) -> iced::Task<Message> {
    match message {
        Message::ForceExit => {
            iced::exit() // Only update can return the exit task
        }
        Message::InputChanged(value) => {
            state.search_query = value;
            state.selected_index = 0;
            state.current_filtered_apps.clear();
            state.current_filtered_apps = state.filtered_apps().into_iter().cloned().collect();
            iced::Task::none()
        }
        Message::KeyPressed(key) => {
            match key {
                keyboard::Key::Named(keyboard::key::Named::Escape) => iced::exit(),
                keyboard::Key::Named(keyboard::key::Named::Enter) => {
                    if let Some(app) = state.current_filtered_apps.get(state.selected_index) {
                        launch_app(app);
                    }
                    iced::exit()
                }
                keyboard::Key::Named(keyboard::key::Named::ArrowDown) => {
                    if !state.current_filtered_apps.is_empty() {
                        let display_count = state.current_filtered_apps.len().min(DISPLAY_COUNT);
                        state.selected_index = (state.selected_index + 1) % display_count;
                    }
                    iced::Task::none()
                }
                keyboard::Key::Named(keyboard::key::Named::ArrowUp) => {
                    if !state.current_filtered_apps.is_empty() {
                        let display_count = state.current_filtered_apps.len().min(DISPLAY_COUNT);
                        state.selected_index = if state.selected_index == 0 {
                            display_count - 1
                        } else {
                            state.selected_index - 1
                        };
                    }
                    iced::Task::none()
                }
                keyboard::Key::Character(ref c) => {
                    // Handle shortcuts first
                    if let Ok(num) = c.parse::<usize>()
                        && num >= 1
                        && num <= state.current_filtered_apps.len().min(DISPLAY_COUNT)
                    {
                        let index = num - 1;
                        if let Some(app) = state.current_filtered_apps.get(index) {
                            launch_app(app);
                            return iced::exit();
                        }
                    }
                    // For non-shortcut characters, treat as search input
                    let mut new_search = state.search_query.clone();
                    new_search.push_str(c);
                    update(state, Message::InputChanged(new_search))
                }
                _ => iced::Task::none(),
            }
        }
    }
}

fn subscription(_state: &AppState) -> iced::Subscription<Message> {
    iced::Subscription::batch([
        // Force capture Escape before any widget can handle it
        iced::event::listen_with(|event, _status, _window| match event {
            iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
                ..
            }) => Some(Message::ForceExit),
            _ => None,
        }),
        // Regular keyboard handling for other keys
        iced::keyboard::on_key_press(|key, _modifiers| {
            match key {
                iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape) => None, // Skip, handled above
                _ => Some(Message::KeyPressed(key)),
            }
        }),
    ])
}

fn view(state: &AppState) -> Element<'_, Message> {
    let input = text_input("Type to search applications...", &state.search_query)
        .on_input(Message::InputChanged)
        .padding(Padding::from(12))
        .size(24)
        .style(|_, _| text_input::Style {
            background: Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.96)),
            border: iced::Border {
                color: Color::from_rgba(1.0, 1.0, 1.0, 0.2),
                width: 1.0,
                radius: 8.0.into(),
            },
            icon: Color::WHITE,
            placeholder: Color::from_rgb(0.8, 0.8, 0.8),
            value: Color::from_rgb(0.96, 0.96, 0.96),
            selection: Color::from_rgb(0.3, 0.3, 0.8),
        })
        .width(Length::Fill);

    let max_results = state.current_filtered_apps.len().min(DISPLAY_COUNT);

    let app_items: Vec<Element<Message>> = state
        .current_filtered_apps
        .iter()
        .take(max_results)
        .enumerate()
        .map(|(index, app)| {
            let is_selected = index == state.selected_index;

            let icon = load_app_icon(app);
            let icon_widget = image(icon).width(48).height(48);

            let app_name = text(&app.name)
                .size(24)
                .color(Color::from_rgb(0.96, 0.96, 0.96));

            let shortcut_symbol = if cfg!(target_os = "macos") {
                "⌘"
            } else {
                "Win+"
            };
            let shortcut = text(format!("{}{}", shortcut_symbol, index + 1))
                .size(18)
                .color(Color::from_rgb(0.8, 0.8, 0.8));

            let content = row![
                icon_widget,
                app_name,
                iced::widget::horizontal_space(),
                shortcut
            ]
            .padding(Padding {
                top: 0.0,
                right: 12.0,
                bottom: 0.0,
                left: 0.0,
            })
            .spacing(12)
            .align_y(Alignment::Center);

            container(content)
                .padding(Padding::from(8))
                .width(Length::Fill)
                .style(move |_| container::Style {
                    background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.84))),
                    border: iced::Border {
                        color: if is_selected {
                            Color::from_rgb(0.149, 0.498, 0.749)
                        } else {
                            Color::from_rgba(1.0, 1.0, 1.0, 0.8)
                        },
                        width: 2.0,
                        radius: 6.0.into(),
                    },
                    shadow: iced::Shadow::default(),
                    text_color: None,
                })
                .into()
        })
        .collect();

    let app_list = column(app_items).spacing(2);

    let content = column![
        input,
        scrollable(app_list)
            .id(scrollable::Id::new("app_list"))
            .style(|_theme, _status| scrollable::Style {
                container: container::Style {
                    background: Some(Background::Color(Color::TRANSPARENT)),
                    border: iced::Border::default(),
                    shadow: iced::Shadow::default(),
                    text_color: None,
                },
                vertical_rail: scrollable::Rail {
                    background: Some(Background::Color(Color::TRANSPARENT)), // Hide track
                    border: iced::Border::default(),
                    scroller: scrollable::Scroller {
                        color: Color::TRANSPARENT, // Hide thumb
                        border: iced::Border::default(),
                    },
                },
                horizontal_rail: scrollable::Rail {
                    background: Some(Background::Color(Color::TRANSPARENT)),
                    border: iced::Border::default(),
                    scroller: scrollable::Scroller {
                        color: Color::TRANSPARENT,
                        border: iced::Border::default(),
                    },
                },
                gap: Some(Background::Color(Color::TRANSPARENT)),
            })
    ]
    .spacing(8);

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_| container::Style {
            background: Some(Background::Color(Color::TRANSPARENT)),
            border: iced::Border::default(),
            shadow: iced::Shadow::default(),
            text_color: None,
        })
        .into()
}

/// Simplified icon loading - uses the icon path resolved by the model or generates fallback
fn load_app_icon(app: &App) -> iced::widget::image::Handle {
    // Try to load the icon path that was resolved by the model
    if let Some(icon_path) = &app.icon
        && let Ok(handle) = load_icon_from_path(icon_path)
    {
        return handle;
    }

    // Fallback to generated icon if no icon path or loading failed
    generate_fallback_icon(&app.name)
}

/// Load icon from the path resolved by the model
#[cfg(target_os = "macos")]
fn load_icon_from_path(
    icon_path: &str,
) -> Result<iced::widget::image::Handle, Box<dyn std::error::Error>> {
    use icns::{IconFamily, IconType};
    use std::fs::File;

    let file = File::open(icon_path)?;
    let icon_family = IconFamily::read(file)?;

    // Try multiple sizes in preference order
    let icon_types = [
        IconType::RGBA32_64x64,
        IconType::RGBA32_32x32,
        IconType::RGBA32_128x128,
        IconType::RGBA32_16x16,
    ];

    for &icon_type in &icon_types {
        if let Ok(image) = icon_family.get_icon_with_type(icon_type) {
            let rgba_data = image.data();
            let (width, height) = match icon_type {
                IconType::RGBA32_16x16 => (16, 16),
                IconType::RGBA32_32x32 => (32, 32),
                IconType::RGBA32_64x64 => (64, 64),
                IconType::RGBA32_128x128 => (128, 128),
                _ => continue,
            };

            if let Ok(png_bytes) = rgba_to_png(rgba_data, width, height) {
                return Ok(iced::widget::image::Handle::from_bytes(png_bytes));
            }
        }
    }

    Err("No suitable icon size found".into())
}

#[cfg(target_os = "linux")]
fn load_icon_from_path(
    icon_path: &str,
) -> Result<iced::widget::image::Handle, Box<dyn std::error::Error>> {
    use std::fs;

    let icon_data = fs::read(icon_path)?;
    Ok(iced::widget::image::Handle::from_bytes(icon_data))
}

/// Helper function to convert RGBA data to PNG bytes
#[cfg(target_os = "macos")]
fn rgba_to_png(
    rgba_data: &[u8],
    width: u32,
    height: u32,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    use ::image::{ImageBuffer, Rgba, codecs::png::PngEncoder};

    // Create image buffer from RGBA data
    let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_raw(width, height, rgba_data.to_vec())
            .ok_or("Failed to create image buffer from RGBA data")?;

    // Encode as PNG
    let mut png_bytes = Vec::new();
    let encoder = PngEncoder::new(&mut png_bytes);
    img.write_with_encoder(encoder)?;

    Ok(png_bytes)
}

/// Generate a deterministic fallback icon for apps without icons
fn generate_fallback_icon(app_name: &str) -> iced::widget::image::Handle {
    // Create a deterministic but random-looking 48x48 icon based on app name
    let base_seed = app_name.chars().map(|c| c as u64).sum::<u64>();
    let mut img = ImageBuffer::new(64, 64);

    // Generate a simple pixelated pattern
    for y in 0..64 {
        for x in 0..64 {
            // Create blocks of 8x8 pixels for pixelated effect
            let block_x = x / 8;
            let block_y = y / 8;
            let block_seed = block_x * 8 + block_y;

            let mut block_rng = ChaCha8Rng::seed_from_u64(base_seed + block_seed as u64);

            let intensity = if block_rng.r#gen::<f32>() > 0.5 {
                200u8
            } else {
                50u8
            };
            let color = [intensity, intensity, intensity];

            img.put_pixel(x, y, Rgb(color));
        }
    }

    // Convert to bytes
    let mut bytes = Vec::new();
    let encoder = PngEncoder::new(&mut bytes);
    img.write_with_encoder(encoder).unwrap_or_else(|_| {
        logs::log_error("Failed to encode generated icon");
    });

    iced::widget::image::Handle::from_bytes(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn create_test_app(name: &str, path: &str) -> App {
        App {
            name: name.to_string(),
            path: PathBuf::from(path),
            description: None,
            icon: None,
        }
    }

    fn create_test_state(apps: Vec<App>, query: &str) -> AppState {
        let model = AppModel {
            all_apps: apps,
            ui_visible: false,
        };
        let mut state = AppState::new(model);
        state.search_query = query.to_string();
        state
    }

    #[test]
    fn test_basic_matching() {
        let apps = vec![
            create_test_app("firefox", "/usr/bin/firefox"),
            create_test_app("photogravure", "/usr/bin/photogravure"),
            create_test_app("gimp", "/usr/bin/gimp"),
            create_test_app("gnome-video", "/usr/bin/gnome-video"),
        ];

        let state = create_test_state(apps, "gv");
        let results = state.filtered_apps();

        // Should have exactly 2 results: photogravure first, then gnome-video
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].name, "photogravure");
        assert_eq!(results[1].name, "gnome-video");
    }

    #[test]
    fn test_case_insensitive() {
        let apps = vec![
            create_test_app("Firefox", "/usr/bin/Firefox"),
            create_test_app("GIMP", "/usr/bin/GIMP"),
        ];

        let state = create_test_state(apps.clone(), "fox");
        let results = state.filtered_apps();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Firefox");

        let state = create_test_state(apps, "GIM");
        let results = state.filtered_apps();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "GIMP");
    }

    #[test]
    fn test_empty_query() {
        let apps = vec![
            create_test_app("firefox", "/usr/bin/firefox"),
            create_test_app("gimp", "/usr/bin/gimp"),
        ];

        let state = create_test_state(apps, "");
        let results = state.filtered_apps();
        assert_eq!(results.len(), 0); // Empty query returns no results
    }

    #[test]
    fn test_no_matches() {
        let apps = vec![
            create_test_app("firefox", "/usr/bin/firefox"),
            create_test_app("photogravure", "/usr/bin/photogravure"),
            create_test_app("gimp", "/usr/bin/gimp"),
            create_test_app("gnome-video", "/usr/bin/gnome-video"),
        ];

        let state = create_test_state(apps, "xyz");
        let results = state.filtered_apps();

        // Should have no results as no app contains x, y, z in sequence
        assert_eq!(results.len(), 0);
    }
}
