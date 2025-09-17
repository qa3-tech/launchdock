use ::image::{ImageBuffer, Rgb, codecs::png::PngEncoder};
use iced::{
    Alignment, Background, Color, Element, Length, Padding, Size,
    daemon::Appearance,
    keyboard::{self},
    widget::{column, container, image, row, scrollable, text},
    window,
};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use std::process::Command;

use crate::apps::{self, AppInfo};

use crate::logs;

pub fn run_ui(all_apps: Vec<AppInfo>) -> Result<(), Box<dyn std::error::Error>> {
    iced::application("launchdock", update, view)
        .subscription(subscription)
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
        .run_with(move || (AppState::new(all_apps), iced::Task::none()))?;

    Ok(())
}

#[derive(Debug, Clone)]
pub enum Message {
    InputChanged(String),
    KeyPressed(keyboard::Key, keyboard::Modifiers),
    IgnoreEvent,
}

struct AppState {
    all_apps: Vec<AppInfo>,
    search_query: String,
    selected_index: usize,
    current_filtered_apps: Vec<AppInfo>,
}

impl AppState {
    fn new(all_apps: Vec<AppInfo>) -> Self {
        Self {
            all_apps,
            selected_index: 0,
            search_query: String::new(),
            current_filtered_apps: Vec::new(),
        }
    }

    pub fn filtered_apps(&self) -> Vec<&AppInfo> {
        if self.search_query.is_empty() {
            return Vec::new();
        }

        let query_lower = self.search_query.to_ascii_lowercase();
        let query_chars: Vec<char> = query_lower.chars().collect();

        let mut matches: Vec<(&AppInfo, f32)> = Vec::new();

        for app in &self.all_apps {
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
        Message::IgnoreEvent => iced::Task::none(),

        Message::InputChanged(value) => {
            state.search_query = value;
            state.selected_index = 0;
            state.current_filtered_apps.clear();
            state.current_filtered_apps = state.filtered_apps().into_iter().cloned().collect();
            iced::Task::none()
        }

        Message::KeyPressed(key, modifiers) => {
            match (key, modifiers) {
                (keyboard::Key::Named(keyboard::key::Named::Escape), _) => iced::exit(),

                (keyboard::Key::Named(keyboard::key::Named::Enter), _) => {
                    if let Some(app) = state.current_filtered_apps.get(state.selected_index) {
                        launch_app(app);
                    }
                    iced::exit()
                }

                (keyboard::Key::Named(keyboard::key::Named::ArrowDown), _) => {
                    if !state.current_filtered_apps.is_empty() {
                        let display_count = state.current_filtered_apps.len().min(DISPLAY_COUNT);
                        state.selected_index = (state.selected_index + 1) % display_count;
                    }
                    iced::Task::none()
                }

                (keyboard::Key::Named(keyboard::key::Named::ArrowUp), _) => {
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

                (keyboard::Key::Named(keyboard::key::Named::Backspace), _) => {
                    state.search_query.pop();
                    state.selected_index = 0;
                    state.current_filtered_apps.clear();
                    state.current_filtered_apps =
                        state.filtered_apps().into_iter().cloned().collect();
                    iced::Task::none()
                }

                (keyboard::Key::Character(ref c), modifiers) if modifiers.logo() => {
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
                    iced::Task::none()
                }

                // Regular character input (only when no modifiers)
                (keyboard::Key::Character(ref c), modifiers)
                    if !modifiers.logo() && !modifiers.control() && !modifiers.alt() =>
                {
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
    iced::event::listen().map(|event| match event {
        iced::Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) => {
            // logs for debug purposes
            // logs::log_info(&format!(
            //       "Global event - Key: {:?}, Modifiers: logo={}",
            //       key,
            //       modifiers.logo()
            //   ));
            Message::KeyPressed(key, modifiers)
        }
        _ => Message::IgnoreEvent,
    })
}

fn view(state: &AppState) -> Element<'_, Message> {
    let input = container(
        text(&state.search_query)
            .size(24)
            .color(Color::from_rgb(0.96, 0.96, 0.96)),
    )
    .padding(Padding::from(12))
    .width(Length::Fill)
    .style(|_| container::Style {
        background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.96))),
        border: iced::Border {
            color: Color::from_rgba(1.0, 1.0, 1.0, 0.2),
            width: 1.0,
            radius: 8.0.into(),
        },
        shadow: iced::Shadow::default(),
        text_color: None,
    });

    let max_results = state.current_filtered_apps.len().min(DISPLAY_COUNT);

    let app_items: Vec<Element<Message>> = state
        .current_filtered_apps
        .iter()
        .take(max_results)
        .enumerate()
        .map(|(index, app)| {
            let is_selected = index == state.selected_index;

            let icon = extract_app_icon(app);
            let icon_widget = image(icon).width(48).height(48);

            let app_name = text(&app.name)
                .size(24)
                .color(Color::from_rgb(0.96, 0.96, 0.96));

            let shortcut_symbol = if cfg!(target_os = "macos") {
                "⌘"
            } else {
                "Logo+"
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

/// Launch an application using platform-specific methods
pub fn launch_app(app: &AppInfo) {
    logs::log_info(&format!("Launching: {}", app.name));

    let result = {
        #[cfg(windows)]
        {
            Command::new("cmd")
                .args(["/c", "start", ""])
                .arg(&app.exe_path)
                .spawn()
        }

        #[cfg(target_os = "macos")]
        {
            Command::new("open").arg(&app.exe_path).spawn()
        }

        #[cfg(target_os = "linux")]
        {
            Command::new(&app.exe_path).spawn()
        }
    };

    if let Err(e) = result {
        logs::log_error(&format!("Failed to launch {}: {}", app.name, e));
    }
}

/// Extract icon and return iced handle
fn extract_app_icon(app: &AppInfo) -> iced::widget::image::Handle {
    if let Ok(Some(icon_data)) = apps::extract_icon(app) {
        return iced::widget::image::Handle::from_bytes(icon_data);
    }

    generate_fallback_icon(&app.name)
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

    fn create_test_app(name: &str, path: &str) -> AppInfo {
        AppInfo {
            name: name.to_string(),
            exe_path: PathBuf::from(path),
            icon_path: None,
        }
    }

    fn create_test_state(apps: Vec<AppInfo>, query: &str) -> AppState {
        let mut state = AppState::new(apps);
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
