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
                skip_taskbar: true,
                titlebar_transparent: true,
                fullsize_content_view: false,
            },
            #[cfg(target_os = "windows")]
            platform_specific: window::settings::PlatformSpecific {
                skip_taskbar: true,
                undecorated_shadow: false,
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
        let state = Self {
            model,
            selected_index: 0,
            search_query: String::new(),
            current_filtered_apps: Vec::new(),
        };
        state
    }

    pub fn filtered_apps(&self) -> Vec<&App> {
        if self.search_query.is_empty() {
            return Vec::new();
        }

        let query_chars: Vec<char> = self.search_query.to_lowercase().chars().collect();

        let results: Vec<_> = self
            .model
            .all_apps
            .iter()
            .filter(|app| {
                let app_name = app.name.to_lowercase();
                let app_chars: Vec<char> = app_name.chars().collect();

                // Check if query characters appear in order
                let mut query_index = 0;

                for app_char in app_chars {
                    if query_index < query_chars.len() && app_char == query_chars[query_index] {
                        query_index += 1;
                    }
                }

                // All query characters must be found in order
                let matches = query_index == query_chars.len();

                // if matches {
                //     logs::log_info(&format!(
                //         "Fuzzy match: '{}' matches '{}'",
                //         self.search_query, app.name
                //     ));
                // }

                matches
            })
            .collect();

        // logs::log_info(&format!(
        //     "Query '{}' found {} apps",
        //     self.search_query,
        //     results.len()
        // ));
        results
    }
}

const DISPLAY_COUNT: usize = 7;

fn update(state: &mut AppState, message: Message) -> iced::Task<Message> {
    match message {
        Message::ForceExit => {
            return iced::exit(); // Only update can return the exit task
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
                keyboard::Key::Named(keyboard::key::Named::Escape) => return iced::exit(),
                keyboard::Key::Named(keyboard::key::Named::Enter) => {
                    if let Some(app) = state.current_filtered_apps.get(state.selected_index) {
                        launch_app(app);
                    }
                    return iced::exit();
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
                    if let Ok(num) = c.parse::<usize>() {
                        if num >= 1 && num <= state.current_filtered_apps.len().min(DISPLAY_COUNT) {
                            let index = num - 1;
                            if let Some(app) = state.current_filtered_apps.get(index) {
                                launch_app(app);
                                return iced::exit();
                            }
                        }
                    }
                    // For non-shortcut characters, treat as search input
                    let mut new_search = state.search_query.clone();
                    new_search.push_str(c);
                    return update(state, Message::InputChanged(new_search));
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

            let icon = if let Some(icon_path) = &app.icon {
                iced::widget::image::Handle::from_path(icon_path)
            } else {
                state.generate_fallback_icon(&app.name)
            };

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

impl AppState {
    // fn load_or_generate_icon(&self, app: &crate::model::App) -> iced::widget::image::Handle {
    //     // Try to load real icon first using iced's built-in loading
    //     if let Some(icon_path) = &app.icon {
    //         // Use iced's from_path method for direct file loading
    //         return iced::widget::image::Handle::from_path(icon_path);
    //     }

    //     // Fallback to generated icon
    //     self.generate_fallback_icon(&app.name)
    // }

    fn generate_fallback_icon(&self, app_name: &str) -> iced::widget::image::Handle {
        // Create a deterministic but random-looking 48x48 icon based on app name
        let base_seed = app_name.chars().map(|c| c as u64).sum::<u64>();
        let mut img = ImageBuffer::new(48, 48);

        // Generate a simple pixelated pattern
        for y in 0..48 {
            for x in 0..48 {
                // Create blocks of 6x6 pixels for pixelated effect
                let block_x = x / 6;
                let block_y = y / 6;
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
}
