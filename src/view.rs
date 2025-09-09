use iced::{
    Alignment, Background, Border, Color, Element, Length, Shadow, Size, Subscription, Task, Theme,
    application, exit, keyboard,
    widget::{Column, Space, column, container, image, row, text, text_input},
    window,
};

use crate::model::{AppModel, launch_app};

#[derive(Debug, Clone)]
pub enum Message {
    InputChanged(String),
    KeyPressed(keyboard::Key, keyboard::Modifiers),
}

pub fn run_ui(initial_model: AppModel) -> iced::Result {
    // Calculate initial window height based on whether there are results
    let initial_height = if initial_model.search_query.is_empty() {
        80.0
    } else {
        let results = initial_model.filtered_apps().len().min(8);
        80.0 + (results as f32 * 60.0)
    };

    application("Launchdock", update, view)
        .subscription(subscription)
        .theme(|_| Theme::Dark)
        .window(window::Settings {
            size: Size::new(700.0, initial_height),
            position: window::Position::Centered,
            visible: true,
            resizable: false,
            decorations: false,
            transparent: true,
            level: window::Level::AlwaysOnTop,
            icon: None,
            #[cfg(target_os = "macos")]
            platform_specific: window::settings::PlatformSpecific {
                title_hidden: true,
                titlebar_transparent: true,
                fullsize_content_view: false,
            },
            #[cfg(not(target_os = "macos"))]
            platform_specific: Default::default(),
            exit_on_close_request: true,
            ..Default::default()
        })
        .run_with(|| (initial_model, Task::none()))
}

fn update(model: &mut AppModel, message: Message) -> Task<Message> {
    match message {
        Message::InputChanged(value) => {
            model.search_query = value;
            model.selected_index = 0;
        }
        Message::KeyPressed(key, modifiers) => match key {
            keyboard::Key::Named(keyboard::key::Named::ArrowDown) => {
                let results = model.filtered_apps();
                if !results.is_empty() && model.selected_index < results.len() - 1 {
                    model.selected_index += 1;
                }
            }
            keyboard::Key::Named(keyboard::key::Named::ArrowUp) => {
                if model.selected_index > 0 {
                    model.selected_index -= 1;
                }
            }
            keyboard::Key::Named(keyboard::key::Named::Enter) => {
                let results = model.filtered_apps();
                if model.selected_index < results.len() {
                    if let Some(app) = results.get(model.selected_index) {
                        launch_app(app);
                        model.search_query.clear();
                        model.selected_index = 0;
                        return exit();
                    }
                }
            }
            keyboard::Key::Named(keyboard::key::Named::Escape) => {
                // Clear and exit
                model.search_query.clear();
                model.selected_index = 0;
                return exit();
            }
            keyboard::Key::Character(c) if modifiers.command() => {
                if let Ok(num) = c.parse::<usize>() {
                    if num < 8 {
                        let results = model.filtered_apps();
                        if num < results.len() {
                            if let Some(app) = results.get(num) {
                                launch_app(app);
                                model.search_query.clear();
                                model.selected_index = 0;
                                return exit();
                            }
                        }
                    }
                }
            }
            _ => {}
        },
    }
    Task::none()
}

fn view(model: &AppModel) -> Element<'_, Message> {
    let input = text_input("Search...", &model.search_query)
        .on_input(Message::InputChanged)
        .padding(16)
        .size(18)
        .width(Length::Fill)
        .style(|_theme: &Theme, _status| text_input::Style {
            background: Background::Color(Color::BLACK),
            border: Border {
                width: 0.0,
                color: Color::TRANSPARENT,
                radius: 8.0.into(),
            },
            icon: Color::from_rgb(0.7, 0.7, 0.7),
            placeholder: Color::from_rgb(0.5, 0.5, 0.5),
            value: Color::WHITE,
            selection: Color::from_rgb(0.3, 0.5, 0.8),
        });

    let results = model.filtered_apps();

    // Calculate the needed height
    let content_height = if results.is_empty() {
        Length::Shrink
    } else {
        Length::Fixed(60.0 + (results.len().min(8) as f32 * 60.0))
    };

    let mut main_column = Column::new()
        .push(
            container(input).width(Length::Fill).padding([0, 20]), // Horizontal padding for centering
        )
        .spacing(0);

    if !results.is_empty() {
        let result_elements = results
            .iter()
            .enumerate()
            .map(|(index, app)| {
                // Try to use actual icon, fallback to emoji
                let icon_element: Element<Message> = if let Some(icon_path) = &app.icon {
                    if std::path::Path::new(icon_path).exists() {
                        container(image(icon_path).width(24).height(24))
                            .width(32)
                            .height(32)
                            .align_x(Alignment::Center)
                            .align_y(Alignment::Center)
                            .into()
                    } else {
                        container(text(get_app_icon(&app.name)).size(24))
                            .width(32)
                            .height(32)
                            .align_x(Alignment::Center)
                            .align_y(Alignment::Center)
                            .into()
                    }
                } else {
                    container(text(get_app_icon(&app.name)).size(24))
                        .width(32)
                        .height(32)
                        .align_x(Alignment::Center)
                        .align_y(Alignment::Center)
                        .into()
                };

                let app_info = column![
                    text(&app.name).size(16).color(Color::WHITE),
                    text(app.description.as_deref().unwrap_or(""))
                        .size(12)
                        .color(Color::from_rgb(0.6, 0.6, 0.6))
                ]
                .spacing(2);

                let shortcut = text(format!("⌘{}", index))
                    .size(14)
                    .color(Color::from_rgb(0.5, 0.5, 0.5));

                let row_content = row![
                    icon_element,
                    Space::with_width(12),
                    app_info,
                    Space::with_width(Length::Fill),
                    shortcut
                ]
                .align_y(Alignment::Center)
                .padding(12);

                container(row_content)
                    .width(Length::Fill)
                    .style(move |_theme: &Theme| container::Style {
                        background: if index == model.selected_index {
                            Some(Background::Color(Color::from_rgb(0.15, 0.15, 0.15)))
                        } else {
                            Some(Background::Color(Color::BLACK))
                        },
                        border: Border::default(),
                        shadow: Shadow::default(),
                        text_color: Some(Color::WHITE),
                    })
                    .into()
            })
            .collect::<Vec<Element<Message>>>();

        let results_container = container(Column::with_children(result_elements).spacing(0))
            .width(Length::Fill)
            .style(|_theme: &Theme| container::Style {
                background: Some(Background::Color(Color::BLACK)),
                border: Border {
                    width: 1.0,
                    color: Color::from_rgb(0.2, 0.2, 0.2),
                    radius: 0.0.into(),
                },
                shadow: Shadow::default(),
                text_color: Some(Color::WHITE),
            });

        main_column = main_column.push(results_container);
    }

    container(main_column)
        .padding(0)
        .width(600)
        .height(content_height)
        .style(|_theme: &Theme| container::Style {
            background: Some(Background::Color(Color::from_rgb(0.2, 0.2, 0.2))),
            border: Border {
                width: 0.0,
                color: Color::TRANSPARENT,
                radius: 12.0.into(),
            },
            shadow: Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.5),
                offset: iced::Vector::new(0.0, 4.0),
                blur_radius: 20.0,
            },
            text_color: Some(Color::WHITE),
        })
        .into()
}

fn subscription(_model: &AppModel) -> Subscription<Message> {
    keyboard::on_key_press(|key, modifiers| Some(Message::KeyPressed(key, modifiers)))
}

fn get_app_icon(name: &str) -> &'static str {
    let lower = name.to_lowercase();

    // System apps
    if lower.contains("finder") {
        return "📁";
    }
    if lower.contains("system preferences") || lower.contains("system settings") {
        return "⚙️";
    }
    if lower.contains("terminal") || lower.contains("iterm") {
        return "💻";
    }
    if lower.contains("activity monitor") {
        return "📊";
    }
    if lower.contains("app store") {
        return "🛍️";
    }

    // Browsers
    if lower.contains("chrome") {
        return "🌐";
    }
    if lower.contains("firefox") {
        return "🦊";
    }
    if lower.contains("safari") {
        return "🧭";
    }
    if lower.contains("edge") {
        return "🌊";
    }
    if lower.contains("brave") {
        return "🦁";
    }

    // Development
    if lower.contains("vscode") || lower.contains("visual studio code") || lower.contains("code") {
        return "📝";
    }
    if lower.contains("xcode") {
        return "🔨";
    }
    if lower.contains("sublime") {
        return "✨";
    }
    if lower.contains("docker") {
        return "🐳";
    }
    if lower.contains("github") {
        return "🐙";
    }

    // Communication
    if lower.contains("slack") {
        return "💬";
    }
    if lower.contains("discord") {
        return "🎮";
    }
    if lower.contains("zoom") {
        return "📹";
    }
    if lower.contains("teams") {
        return "👥";
    }
    if lower.contains("mail") || lower.contains("outlook") {
        return "📧";
    }
    if lower.contains("messages") {
        return "💬";
    }
    if lower.contains("whatsapp") {
        return "📞";
    }

    // Media
    if lower.contains("spotify") || lower.contains("music") {
        return "🎵";
    }
    if lower.contains("photos") {
        return "📷";
    }
    if lower.contains("vlc") {
        return "🎞️";
    }

    // Productivity
    if lower.contains("notes") {
        return "📝";
    }
    if lower.contains("calendar") {
        return "📅";
    }
    if lower.contains("reminders") {
        return "☑️";
    }
    if lower.contains("notion") {
        return "📓";
    }

    // Creative
    if lower.contains("figma") {
        return "🎨";
    }
    if lower.contains("sketch") {
        return "✏️";
    }
    if lower.contains("photoshop") {
        return "🖼️";
    }

    // Utilities
    if lower.contains("calculator") {
        return "🧮";
    }
    if lower.contains("1password") || lower.contains("bitwarden") {
        return "🔑";
    }

    "📦" // Default
}
