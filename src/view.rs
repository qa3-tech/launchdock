use iced::{
    Alignment, Element, Length, Subscription, Task, keyboard,
    widget::{button, column, container, text, text_input},
};

use crate::model::{AppModel, launch_app};

pub struct LauncherApp {
    model: AppModel,
}

#[derive(Debug, Clone)]
pub enum Message {
    SearchChanged(String),
    LaunchApp(usize),
    LaunchSelected,
    Navigate(i32),
    Close,
}

impl LauncherApp {
    fn new(model: AppModel) -> Self {
        Self { model }
    }
}

fn update(app: &mut LauncherApp, message: Message) -> Task<Message> {
    match message {
        Message::SearchChanged(query) => {
            app.model.search_query = query;
            app.model.selected_index = 0;
        }
        Message::LaunchApp(index) => {
            let filtered = app.model.filtered_apps();
            if let Some(app_item) = filtered.get(index) {
                launch_app(app_item);
            }
        }
        Message::LaunchSelected => {
            let filtered = app.model.filtered_apps();
            if let Some(app_item) = filtered.get(app.model.selected_index) {
                launch_app(app_item);
            }
        }
        Message::Navigate(delta) => {
            let filtered = app.model.filtered_apps();
            if !filtered.is_empty() {
                let current = app.model.selected_index as i32;
                let new_index = (current + delta).max(0) as usize;
                app.model.selected_index = new_index.min(filtered.len() - 1);
            }
        }
        Message::Close => {
            std::process::exit(0);
        }
    }
    Task::none()
}

fn view(app: &LauncherApp) -> Element<'_, Message> {
    // Platform-aware search placeholder
    let search_placeholder = if std::env::consts::OS == "linux" {
        "Search applications and descriptions..."
    } else {
        "Search applications..."
    };

    let search_input = text_input(search_placeholder, &app.model.search_query)
        .on_input(Message::SearchChanged)
        .padding(12)
        .size(16)
        .width(Length::Fill);

    let filtered_apps = app.model.filtered_apps();

    if filtered_apps.is_empty() {
        // Empty state handling
        let content = if app.model.search_query.is_empty() {
            column![
                text("LaunchDock").size(24),
                text("Type to search applications").size(14),
            ]
            .spacing(10)
            .align_x(Alignment::Center)
        } else {
            column![
                text("No applications found").size(16),
                text(format!("No matches for \"{}\"", app.model.search_query)).size(12),
            ]
            .spacing(8)
            .align_x(Alignment::Center)
        };

        container(
            column![search_input, content]
                .spacing(20)
                .align_x(Alignment::Center),
        )
        .padding(20)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .into()
    } else {
        // Build app list
        let mut app_list = column![];

        for (index, app_item) in filtered_apps.iter().enumerate().take(8) {
            let is_selected = index == app.model.selected_index;

            // Create app info column with name and optional description
            let mut app_content = column![text(&app_item.name).size(14)];

            // Add description if available and different from name
            if let Some(ref description) = app_item.description {
                if !description.is_empty() && description != &app_item.name {
                    app_content = app_content.push(text(description).size(11));
                }
            }

            // Create button with iced 0.13.1 styling
            let app_button = button(app_content)
                .on_press(Message::LaunchApp(index))
                .width(Length::Fill)
                .padding(12);

            // Apply styling using the correct 0.13.1 API
            let styled_button = if is_selected {
                app_button.style(button::primary)
            } else {
                app_button
            };

            app_list = app_list.push(styled_button);
        }

        app_list = app_list.spacing(4);

        // Create status bar with result count and navigation hints
        let status_text = if filtered_apps.len() > 8 {
            format!(
                "Showing 8 of {} results • ↑↓ navigate • Enter launch • Esc close",
                filtered_apps.len()
            )
        } else {
            let result_word = if filtered_apps.len() == 1 {
                "result"
            } else {
                "results"
            };
            format!(
                "{} {} • ↑↓ navigate • Enter launch • Esc close",
                filtered_apps.len(),
                result_word
            )
        };

        let status_bar = text(status_text).size(10);

        // Combine all elements
        container(column![search_input, app_list, status_bar].spacing(12))
            .padding(16)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}

fn subscription(_app: &LauncherApp) -> Subscription<Message> {
    keyboard::on_key_press(|key, _modifiers| match key {
        keyboard::Key::Named(keyboard::key::Named::ArrowUp) => Some(Message::Navigate(-1)),
        keyboard::Key::Named(keyboard::key::Named::ArrowDown) => Some(Message::Navigate(1)),
        keyboard::Key::Named(keyboard::key::Named::Enter) => Some(Message::LaunchSelected),
        keyboard::Key::Named(keyboard::key::Named::Escape) => Some(Message::Close),
        _ => None,
    })
}

pub fn run_ui(model: AppModel) -> iced::Result {
    iced::application("LaunchDock", update, view)
        .subscription(subscription)
        .window(iced::window::Settings {
            size: iced::Size::new(500.0, 400.0),
            position: iced::window::Position::Centered,
            decorations: false,
            level: iced::window::Level::AlwaysOnTop,
            resizable: false,
            ..Default::default()
        })
        .run_with(|| (LauncherApp::new(model), Task::none()))
}
