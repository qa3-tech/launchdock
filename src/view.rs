use ::image::{ImageBuffer, Rgb, codecs::png::PngEncoder};
use iced::{
    Alignment, Background, Color, Element, Length, Padding, Size, keyboard,
    widget::{column, container, image, row, text, text_input},
    window,
};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use std::collections::HashMap;

use crate::{
    logs,
    model::{AppModel, launch_app},
};

pub fn run_ui(model: AppModel) -> Result<(), Box<dyn std::error::Error>> {
    iced::application("launchdockui", update, view)
        .subscription(subscription)
        .window(window::Settings {
            size: Size::new(800.0, 600.0),
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
            platform_specific: window::settings::PlatformSpecific { skip_taskbar: true },
            #[cfg(target_os = "windows")]
            platform_specific: window::settings::PlatformSpecific {
                skip_taskbar: true,
                undecorated_shadow: false,
            },
            ..Default::default()
        })
        .run_with(move || (AppState::new(model), iced::Task::none()))?;

    Ok(())
}

#[derive(Debug, Clone)]
pub enum Message {
    InputChanged(String),
    KeyPressed(keyboard::Key),
}

struct AppState {
    model: AppModel,
    icon_cache: HashMap<String, iced::widget::image::Handle>,
}

impl AppState {
    fn new(model: AppModel) -> Self {
        let mut state = Self {
            model,
            icon_cache: HashMap::new(),
        };
        state.load_icons_for_filtered_apps();
        state
    }
}

fn update(state: &mut AppState, message: Message) -> iced::Task<Message> {
    match message {
        Message::InputChanged(value) => {
            state.model.search_query = value;
            state.model.selected_index = 0; // Reset to first item
            state.load_icons_for_filtered_apps();
            iced::Task::none()
        }
        Message::KeyPressed(key) => {
            match key {
                keyboard::Key::Named(keyboard::key::Named::Escape) => {
                    state.model.search_query.clear();
                    iced::exit()
                }
                keyboard::Key::Named(keyboard::key::Named::Enter) => {
                    let filtered = state.model.filtered_apps();
                    if let Some(app) = filtered.get(state.model.selected_index) {
                        launch_app(app);
                        return iced::exit();
                    }
                    iced::Task::none()
                }
                keyboard::Key::Named(keyboard::key::Named::ArrowDown) => {
                    let filtered = state.model.filtered_apps();
                    if !filtered.is_empty() {
                        state.model.selected_index =
                            (state.model.selected_index + 1).min(filtered.len() - 1);
                    }
                    iced::Task::none()
                }
                keyboard::Key::Named(keyboard::key::Named::ArrowUp) => {
                    if state.model.selected_index > 0 {
                        state.model.selected_index -= 1;
                    }
                    iced::Task::none()
                }
                keyboard::Key::Character(c) => {
                    // Handle Cmd/Win + number shortcuts
                    if let Ok(num) = c.parse::<usize>() {
                        if num >= 1 && num <= 8 {
                            let index = num - 1;
                            let filtered = state.model.filtered_apps();
                            if let Some(app) = filtered.get(index) {
                                launch_app(app);
                                return iced::exit();
                            }
                        }
                    }
                    iced::Task::none()
                }
                _ => iced::Task::none(),
            }
        }
    }
}

fn subscription(_state: &AppState) -> iced::Subscription<Message> {
    iced::keyboard::on_key_press(|key, _modifiers| Some(Message::KeyPressed(key)))
}

fn view(state: &AppState) -> Element<'_, Message> {
    let input = text_input("Type to search applications...", &state.model.search_query)
        .on_input(Message::InputChanged)
        .padding(Padding::from(12))
        .size(16)
        .style(|_, _| text_input::Style {
            background: Background::Color(Color::BLACK),
            border: iced::Border::default(),
            icon: Color::WHITE,
            placeholder: Color::from_rgb(0.7, 0.7, 0.7),
            value: Color::from_rgb(0.96, 0.96, 0.96), // whitesmoke
            selection: Color::from_rgb(0.3, 0.3, 0.8),
        })
        .width(Length::Fill);

    let filtered_apps = state.model.filtered_apps();
    let max_results = 8.min(filtered_apps.len());

    let mut app_list = column![].spacing(2);

    for (index, app) in filtered_apps.iter().take(max_results).enumerate() {
        let is_selected = index == state.model.selected_index;

        let icon = state.get_app_icon(&app.name);
        let icon_widget = image(icon).width(48).height(48);

        let app_name = text(&app.name)
            .size(14)
            .color(Color::from_rgb(0.96, 0.96, 0.96)); // whitesmoke

        let shortcut_symbol = if cfg!(target_os = "macos") {
            "⌘"
        } else {
            "Win+"
        };
        let shortcut = text(format!("{}{}", shortcut_symbol, index + 1))
            .size(12)
            .color(Color::from_rgb(0.7, 0.7, 0.7));

        let content = row![icon_widget, column![app_name, shortcut].spacing(2),]
            .spacing(12)
            .align_y(Alignment::Center);

        let item = container(content)
            .padding(Padding::from(8))
            .width(Length::Fill)
            .style(move |_| container::Style {
                background: Some(Background::Color(if is_selected {
                    Color::from_rgb(0.2, 0.2, 0.2) // Slightly lighter for selection
                } else {
                    Color::BLACK
                })),
                border: iced::Border::default(),
                shadow: iced::Shadow::default(),
                text_color: None,
            });

        app_list = app_list.push(item);
    }

    let content = column![
        input,
        container(app_list)
            .padding(Padding::from(8))
            .style(|_| container::Style {
                background: Some(Background::Color(Color::BLACK)),
                border: iced::Border::default(),
                shadow: iced::Shadow::default(),
                text_color: None,
            })
    ]
    .spacing(0);

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
    fn load_icons_for_filtered_apps(&mut self) {
        let filtered = self.model.filtered_apps();
        for app in filtered.iter().take(8) {
            if !self.icon_cache.contains_key(&app.name) {
                let handle = self.load_or_generate_icon(app);
                self.icon_cache.insert(app.name.clone(), handle);
            }
        }
    }

    fn get_app_icon(&self, app_name: &str) -> iced::widget::image::Handle {
        self.icon_cache
            .get(app_name)
            .cloned()
            .unwrap_or_else(|| self.generate_fallback_icon(app_name))
    }

    fn load_or_generate_icon(&self, app: &crate::model::App) -> iced::widget::image::Handle {
        // Try to load real icon first using iced's built-in loading
        if let Some(icon_path) = &app.icon {
            // Use iced's from_path method for direct file loading
            return iced::widget::image::Handle::from_path(icon_path);
        }

        // Fallback to generated icon
        self.generate_fallback_icon(&app.name)
    }

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
