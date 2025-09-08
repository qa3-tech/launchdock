use iced::{
    widget::{column, container, row, text, text_input, Column, Space},
    keyboard, application, Alignment, Background, Border, Color, Element,
    Length, Shadow, Subscription, Task, Theme,
};

use crate::model::{AppModel, launch_app};

#[derive(Debug, Clone)]
pub enum Message {
    InputChanged(String),
    KeyPressed(keyboard::Key, keyboard::Modifiers),
}

pub fn run_ui(initial_model: AppModel) -> iced::Result {
    application("Launchdock", update, view)
        .subscription(subscription)
        .theme(|_| Theme::Dark)
        .run_with(|| (initial_model, Task::none()))
}

fn update(model: &mut AppModel, message: Message) -> Task<Message> {
    match message {
        Message::InputChanged(value) => {
            model.search_query = value;
            model.selected_index = 0;
        }
        Message::KeyPressed(key, modifiers) => {
            match key {
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
                        }
                    }
                }
                keyboard::Key::Named(keyboard::key::Named::Escape) => {
                    model.search_query.clear();
                    model.selected_index = 0;
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
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
    Task::none()
}

fn view(model: &AppModel) -> Element<Message> {
    let input = text_input("Search...", &model.search_query)
        .on_input(Message::InputChanged)
        .padding(16)
        .size(18)
        .style(|_theme: &Theme, _status| {
            text_input::Style {
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
            }
        });

    let mut content = Column::new()
        .push(input)
        .spacing(0);

    let results = model.filtered_apps();
    
    if !results.is_empty() {
        let result_elements = results
            .iter()
            .enumerate()
            .map(|(index, app)| {
                let icon = text("📦").size(24);
                
                let app_info = column![
                    text(&app.name)
                        .size(16)
                        .color(Color::WHITE),
                    text(app.description.as_deref().unwrap_or(""))
                        .size(12)
                        .color(Color::from_rgb(0.6, 0.6, 0.6))
                ]
                .spacing(2);
                
                let shortcut = text(format!("⌘{}", index))
                    .size(14)
                    .color(Color::from_rgb(0.5, 0.5, 0.5));
                
                let row_content = row![
                    icon,
                    Space::with_width(12),
                    app_info,
                    Space::with_width(Length::Fill),
                    shortcut
                ]
                .align_y(Alignment::Center)
                .padding(12);
                
                container(row_content)
                    .width(Length::Fill)
                    .style(move |_theme: &Theme| {
                        container::Style {
                            background: if index == model.selected_index {
                                Some(Background::Color(Color::from_rgb(0.15, 0.15, 0.15)))
                            } else {
                                Some(Background::Color(Color::BLACK))
                            },
                            border: Border::default(),
                            shadow: Shadow::default(),
                            text_color: Some(Color::WHITE),
                        }
                    })
                    .into()
            })
            .collect::<Vec<Element<Message>>>();
        
        let results_container = container(
            Column::with_children(result_elements)
                .spacing(0)
        )
        .style(|_theme: &Theme| {
            container::Style {
                background: Some(Background::Color(Color::BLACK)),
                border: Border {
                    width: 1.0,
                    color: Color::from_rgb(0.2, 0.2, 0.2),
                    radius: 0.0.into(),
                },
                shadow: Shadow::default(),
                text_color: Some(Color::WHITE),
            }
        });
        
        content = content.push(results_container);
    }

    let inner_container = container(content)
        .padding(0)
        .width(600)
        .style(|_theme: &Theme| {
            container::Style {
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
            }
        });

    container(inner_container)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .into()
}

fn subscription(_model: &AppModel) -> Subscription<Message> {
    keyboard::on_key_press(|key, modifiers| {
        Some(Message::KeyPressed(key, modifiers))
    })
}