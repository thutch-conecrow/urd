use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::crypto;

use super::app::{
    AddEnvStep, AddStep, App, DeleteTarget, MetadataField, Mode, Row, SENSITIVITY_OPTIONS,
};

pub fn draw(frame: &mut Frame, app: &mut App) {
    let chunks = Layout::vertical([
        Constraint::Min(5),       // item list
        Constraint::Length(7),    // detail panel
        Constraint::Length(1),    // status bar
    ])
    .split(frame.area());

    draw_list(frame, app, chunks[0]);
    draw_detail(frame, app, chunks[1]);
    draw_status_bar(frame, app, chunks[2]);
}

fn sensitivity_badge(item: &crate::store::types::Item) -> Span<'static> {
    if let Some(ref sens) = item.sensitivity {
        return match sens {
            crate::store::types::Sensitivity::Plaintext => {
                Span::styled(" [plaintext]", Style::default().fg(Color::DarkGray))
            }
            crate::store::types::Sensitivity::Sensitive => {
                Span::styled(" [sensitive]", Style::default().fg(Color::Yellow))
            }
            crate::store::types::Sensitivity::Secret => {
                Span::styled(" [secret]", Style::default().fg(Color::Red))
            }
        };
    }

    let level = item
        .values
        .values()
        .find_map(|v| crypto::parse_sensitivity(v));

    match level {
        Some(crypto::SensitivityLevel::Sensitive) => {
            Span::styled(" [sensitive]", Style::default().fg(Color::Yellow))
        }
        Some(crypto::SensitivityLevel::Secret) => {
            Span::styled(" [secret]", Style::default().fg(Color::Red))
        }
        None => Span::styled(" [plaintext]", Style::default().fg(Color::DarkGray)),
    }
}

/// Compute inline validation warnings for an item.
fn item_warnings(item: &crate::store::types::Item) -> Vec<&'static str> {
    let mut warnings = Vec::new();

    // Has values but no description
    if !item.values.is_empty() && item.description.is_none() {
        warnings.push("undocumented");
    }

    // Declared sensitive/secret but has unencrypted values
    if matches!(
        item.sensitivity,
        Some(crate::store::types::Sensitivity::Sensitive | crate::store::types::Sensitivity::Secret)
    ) {
        let has_unencrypted = item.values.values().any(|v| {
            !v.starts_with("ENC[age:")
                && !v.starts_with("ENC[age,")
        });
        if has_unencrypted {
            warnings.push("unencrypted");
        }
    }

    // Has declared envs with missing values
    let has_missing = item
        .environments
        .iter()
        .any(|env| !item.values.contains_key(env));
    if has_missing {
        warnings.push("missing values");
    }

    warnings
}

#[allow(clippy::too_many_lines)]
fn draw_list(frame: &mut Frame, app: &mut App, area: ratatui::layout::Rect) {
    let block = Block::default().title(" urd ").borders(Borders::ALL);
    let inner = block.inner(area);

    let mut lines: Vec<Line> = Vec::new();

    // Search bar takes one line from the viewport
    let search_height = usize::from(app.filter.is_some());
    if let Some(ref input) = app.filter {
        lines.push(Line::from(render_input_line("/ ", input)));
    }

    let viewport_height = inner.height as usize - search_height;
    app.adjust_scroll(viewport_height);

    let visible_range = app.scroll_offset
        ..app
            .visible_rows
            .len()
            .min(app.scroll_offset + viewport_height);

    for i in visible_range {
        let row = &app.visible_rows[i];
        let is_selected = i == app.selected;
        let style = if is_selected {
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        let line = match row {
            Row::ItemHeader(id) => {
                let item = &app.store[id];
                let prefix = if app.expanded.contains(id) {
                    "  "
                } else {
                    "+ "
                };
                let badge = sensitivity_badge(item);
                let warnings = item_warnings(item);
                let mut spans = vec![
                    Span::styled(prefix, style),
                    Span::styled(id.clone(), style),
                    badge,
                ];
                if !warnings.is_empty() {
                    spans.push(Span::styled(
                        format!(" ({})", warnings.join(", ")),
                        Style::default().fg(Color::Red),
                    ));
                }
                Line::from(spans)
            }
            Row::EnvValue(id, env) => {
                let item = &app.store[id];
                let is_last = app
                    .visible_rows
                    .get(i + 1)
                    .is_none_or(|next| {
                        !matches!(next, Row::EnvValue(nid, _) | Row::MissingEnv(nid, _) if nid == id)
                    });

                let connector = if is_last { "  \u{2514} " } else { "  \u{251c} " };

                // Check if this row is being edited
                let is_editing = matches!(
                    &app.mode,
                    Mode::EditValue { item_id, env: edit_env, .. }
                    if item_id == id && edit_env == env
                );

                if is_editing {
                    let Mode::EditValue { ref input, .. } = app.mode else {
                        unreachable!()
                    };
                    let before = &input.buffer[..input.cursor];
                    let (cursor_ch, after) = if input.cursor < input.buffer.len() {
                        let c = input.buffer[input.cursor..].chars().next().unwrap_or(' ');
                        (c, &input.buffer[input.cursor + c.len_utf8()..])
                    } else {
                        (' ', "")
                    };

                    Line::from(vec![
                        Span::styled(format!("{connector}{env}: "), style),
                        Span::raw(before.to_string()),
                        Span::styled(
                            cursor_ch.to_string(),
                            Style::default().bg(Color::White).fg(Color::Black),
                        ),
                        Span::raw(after.to_string()),
                    ])
                } else {
                    let value = item.values.get(env).map_or("", String::as_str);
                    let display_value = if crypto::parse_sensitivity(value).is_some() {
                        if app.is_revealed(id) {
                            crypto::decrypt_value(value)
                                .unwrap_or_else(|_| "(decrypt error)".into())
                        } else {
                            "\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}".to_string()
                        }
                    } else {
                        value.to_string()
                    };

                    Line::from(vec![Span::styled(
                        format!("{connector}{env}: {display_value}"),
                        style,
                    )])
                }
            }
            Row::MissingEnv(id, env) => {
                let is_last = app
                    .visible_rows
                    .get(i + 1)
                    .is_none_or(|next| {
                        !matches!(next, Row::EnvValue(nid, _) | Row::MissingEnv(nid, _) if nid == id)
                    });
                let connector = if is_last { "  \u{2514} " } else { "  \u{251c} " };
                Line::from(vec![Span::styled(
                    format!("{connector}{env}: (missing)"),
                    Style::default().fg(Color::Red).add_modifier(Modifier::DIM),
                )])
            }
        };

        lines.push(line);
    }

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "  Store is empty. Use `urd set` to add items.",
            Style::default().fg(Color::DarkGray),
        )));
    }

    let list = Paragraph::new(lines).block(block);
    frame.render_widget(list, area);
}

fn draw_detail(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let block = Block::default().borders(Borders::TOP);

    let content = if let Mode::EditMetadata {
        ref fields,
        active_field,
        ..
    } = app.mode
    {
        draw_metadata_form(fields, active_field)
    } else if let Mode::Clone {
        ref item_id,
        ref source_env,
        ref input,
    } = app.mode
    {
        vec![
            Line::from(vec![
                Span::styled("Clone ", Style::default().fg(Color::Cyan)),
                Span::raw(format!("{item_id} ({source_env})")),
            ]),
            Line::from(render_input_line("To environment: ", input)),
        ]
    } else if let Mode::Add { ref step, .. } = app.mode {
        draw_add_step(step)
    } else if let Mode::AddEnv {
        ref item_id,
        ref step,
        ..
    } = app.mode
    {
        draw_add_env_step(item_id, step)
    } else if let Some((_, item)) = app.selected_item() {
        let mut lines = Vec::new();

        if let Some(ref desc) = item.description {
            lines.push(Line::from(vec![
                Span::styled("description: ", Style::default().fg(Color::Cyan)),
                Span::raw(desc),
            ]));
        }
        if let Some(ref origin) = item.origin {
            lines.push(Line::from(vec![
                Span::styled("origin: ", Style::default().fg(Color::Cyan)),
                Span::raw(origin),
            ]));
        }
        if !item.tags.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("tags: ", Style::default().fg(Color::Cyan)),
                Span::raw(item.tags.join(", ")),
            ]));
        }
        if !item.environments.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("environments: ", Style::default().fg(Color::Cyan)),
                Span::raw(item.environments.join(", ")),
            ]));
        }

        let on_header = matches!(
            app.visible_rows.get(app.selected),
            Some(Row::ItemHeader(_))
        );

        if lines.is_empty() {
            if on_header {
                lines.push(Line::from(Span::styled(
                    "No metadata — press e to edit",
                    Style::default().fg(Color::DarkGray),
                )));
            } else {
                lines.push(Line::from(Span::styled(
                    "No metadata",
                    Style::default().fg(Color::DarkGray),
                )));
            }
        } else if on_header {
            lines.push(Line::from(Span::styled(
                "press e to edit",
                Style::default().fg(Color::DarkGray),
            )));
        }

        lines
    } else {
        vec![Line::from(Span::styled(
            "No item selected",
            Style::default().fg(Color::DarkGray),
        ))]
    };

    let detail = Paragraph::new(content).block(block);
    frame.render_widget(detail, area);
}

fn draw_metadata_form(
    fields: &super::app::MetadataFields,
    active: MetadataField,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    for field in MetadataField::ALL {
        let is_active = field == active;
        let label_style = if is_active {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Cyan)
        };

        let label = format!("{}: ", field.label());

        if field == MetadataField::Sensitivity {
            let mut spans = vec![Span::styled(label, label_style)];
            for (i, option) in SENSITIVITY_OPTIONS.iter().enumerate() {
                if i == fields.sensitivity {
                    spans.push(Span::styled(
                        format!("[{option}]"),
                        Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                    ));
                } else {
                    spans.push(Span::styled(
                        format!(" {option} "),
                        Style::default().fg(Color::DarkGray),
                    ));
                }
            }
            if is_active {
                spans.push(Span::styled(
                    " \u{2190}\u{2192}",
                    Style::default().fg(Color::DarkGray),
                ));
            }
            lines.push(Line::from(spans));
        } else {
            let input = match field {
                MetadataField::Description => &fields.description,
                MetadataField::Origin => &fields.origin,
                MetadataField::Tags => &fields.tags,
                MetadataField::Environments => &fields.environments,
                MetadataField::Sensitivity => unreachable!(),
            };

            if is_active {
                let before = &input.buffer[..input.cursor];
                let (cursor_ch, after) = if input.cursor < input.buffer.len() {
                    let c = input.buffer[input.cursor..].chars().next().unwrap_or(' ');
                    (c, &input.buffer[input.cursor + c.len_utf8()..])
                } else {
                    (' ', "")
                };
                lines.push(Line::from(vec![
                    Span::styled(label, label_style),
                    Span::raw(before.to_string()),
                    Span::styled(
                        cursor_ch.to_string(),
                        Style::default().bg(Color::White).fg(Color::Black),
                    ),
                    Span::raw(after.to_string()),
                ]));
            } else {
                lines.push(Line::from(vec![
                    Span::styled(label, label_style),
                    Span::raw(input.buffer.clone()),
                ]));
            }
        }
    }

    lines
}

fn draw_add_step(step: &AddStep) -> Vec<Line<'static>> {
    let label = step.label().to_string();
    let label_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);

    match step {
        AddStep::Sensitivity(idx) => {
            let mut spans = vec![Span::styled(format!("{label}: "), label_style)];
            for (i, option) in SENSITIVITY_OPTIONS.iter().enumerate() {
                if i == *idx {
                    spans.push(Span::styled(
                        format!("[{option}]"),
                        Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                    ));
                } else {
                    spans.push(Span::styled(
                        format!(" {option} "),
                        Style::default().fg(Color::DarkGray),
                    ));
                }
            }
            spans.push(Span::styled(
                " \u{2190}\u{2192}",
                Style::default().fg(Color::DarkGray),
            ));
            vec![Line::from(spans)]
        }
        AddStep::Id(input)
        | AddStep::Env(input)
        | AddStep::Value(input)
        | AddStep::Description(input)
        | AddStep::Origin(input)
        | AddStep::Tags(input) => {
            vec![Line::from(render_input_line(&format!("{label}: "), input))]
        }
    }
}

fn draw_add_env_step(item_id: &str, step: &AddEnvStep) -> Vec<Line<'static>> {
    let mut lines = vec![Line::from(vec![
        Span::styled("Adding to: ", Style::default().fg(Color::Cyan)),
        Span::raw(item_id.to_string()),
    ])];
    match step {
        AddEnvStep::Env(input) => {
            lines.push(Line::from(render_input_line("Environment: ", input)));
        }
        AddEnvStep::Value(input) => {
            lines.push(Line::from(render_input_line("Value: ", input)));
        }
    }
    lines
}

fn render_input_line<'a>(label: &str, input: &super::app::InputState) -> Vec<Span<'a>> {
    let label_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    let before = &input.buffer[..input.cursor];
    let (cursor_ch, after) = if input.cursor < input.buffer.len() {
        let c = input.buffer[input.cursor..].chars().next().unwrap_or(' ');
        (c, &input.buffer[input.cursor + c.len_utf8()..])
    } else {
        (' ', "")
    };
    vec![
        Span::styled(label.to_string(), label_style),
        Span::raw(before.to_string()),
        Span::styled(
            cursor_ch.to_string(),
            Style::default().bg(Color::White).fg(Color::Black),
        ),
        Span::raw(after.to_string()),
    ]
}

fn draw_status_bar(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    if let Some(ref msg) = app.status_message {
        let bar = Paragraph::new(Line::from(Span::styled(
            format!(" {msg}"),
            Style::default().fg(Color::Green),
        )));
        frame.render_widget(bar, area);
        return;
    }

    let enter_next = || {
        Line::from(vec![
            Span::styled(" Enter", Style::default().fg(Color::Yellow)),
            Span::raw(" next  "),
            Span::styled("Esc", Style::default().fg(Color::Yellow)),
            Span::raw(" cancel"),
        ])
    };

    let line = match &app.mode {
        Mode::ConfirmDelete(target) => {
            let prompt = match target {
                DeleteTarget::Item(id) => format!("Delete {id} and all its values? (y/n)"),
                DeleteTarget::EnvValue(id, env) => format!("Remove {env} from {id}? (y/n)"),
            };
            Line::from(Span::styled(
                format!(" {prompt}"),
                Style::default().fg(Color::Red),
            ))
        }
        Mode::EditValue { .. } => Line::from(vec![
            Span::styled(" Enter", Style::default().fg(Color::Yellow)),
            Span::raw(" save  "),
            Span::styled("Esc", Style::default().fg(Color::Yellow)),
            Span::raw(" cancel"),
        ]),
        Mode::EditMetadata { .. } => Line::from(vec![
            Span::styled(" Tab", Style::default().fg(Color::Yellow)),
            Span::raw(" next field  "),
            Span::styled("Shift+Tab", Style::default().fg(Color::Yellow)),
            Span::raw(" prev  "),
            Span::styled("Enter", Style::default().fg(Color::Yellow)),
            Span::raw(" save  "),
            Span::styled("Esc", Style::default().fg(Color::Yellow)),
            Span::raw(" cancel"),
        ]),
        Mode::Clone { .. } | Mode::Add { .. } | Mode::AddEnv { .. } => enter_next(),
        Mode::Browse => Line::from(vec![
            Span::styled(" q", Style::default().fg(Color::Yellow)),
            Span::raw(" quit  "),
            Span::styled("j/k", Style::default().fg(Color::Yellow)),
            Span::raw(" navigate  "),
            Span::styled("l", Style::default().fg(Color::Yellow)),
            Span::raw(" expand  "),
            Span::styled("h", Style::default().fg(Color::Yellow)),
            Span::raw(" collapse  "),
            Span::styled("r", Style::default().fg(Color::Yellow)),
            Span::raw(" reveal  "),
            Span::styled("e", Style::default().fg(Color::Yellow)),
            Span::raw(" edit  "),
            Span::styled("a", Style::default().fg(Color::Yellow)),
            Span::raw(" add  "),
            Span::styled("c", Style::default().fg(Color::Yellow)),
            Span::raw(" clone  "),
            Span::styled("d", Style::default().fg(Color::Yellow)),
            Span::raw(" delete  "),
            Span::styled("u", Style::default().fg(Color::Yellow)),
            Span::raw(" undo"),
        ]),
    };

    let bar = Paragraph::new(line);
    frame.render_widget(bar, area);
}
