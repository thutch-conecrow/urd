use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::app::{
    AddEnvStep, AddStep, App, MetadataField, Mode, SENSITIVITY_OPTIONS,
};

pub fn handle_key(app: &mut App, key: KeyEvent) {
    app.status_message = None;

    match &app.mode {
        Mode::Browse => handle_browse(app, key),
        Mode::ConfirmDelete(_) => handle_confirm_delete(app, key),
        Mode::EditValue { .. } => handle_edit_value(app, key),
        Mode::EditMetadata { .. } => handle_edit_metadata(app, key),
        Mode::Clone { .. } => handle_clone(app, key),
        Mode::Add { .. } => handle_add(app, key),
        Mode::AddEnv { .. } => handle_add_env(app, key),
    }
}

fn handle_browse(app: &mut App, key: KeyEvent) {
    // When search filter is active, typing goes to the filter input
    if app.filter.is_some() {
        return handle_search(app, key);
    }

    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => app.should_quit = true,

        // Navigation
        KeyCode::Char('j') | KeyCode::Down => app.move_down(),
        KeyCode::Char('k') | KeyCode::Up => app.move_up(),

        // Expand / collapse
        KeyCode::Char('l') | KeyCode::Right | KeyCode::Enter => app.expand_selected(),
        KeyCode::Char('h') | KeyCode::Left => app.collapse_selected(),
        KeyCode::Char('+') => app.expand_all(),
        KeyCode::Char('-') => app.collapse_all(),

        // Undo / Redo (Ctrl+r before plain r)
        KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            if let Err(e) = app.redo() {
                app.status_message = Some(format!("Redo failed: {e}"));
            }
        }
        KeyCode::Char('u') => {
            if let Err(e) = app.undo() {
                app.status_message = Some(format!("Undo failed: {e}"));
            }
        }

        // Reveal
        KeyCode::Char('r') => app.toggle_reveal_selected(),
        KeyCode::Char('R') => app.toggle_reveal_all(),

        // Delete
        KeyCode::Char('d') => app.initiate_delete(),

        // Edit
        KeyCode::Char('e') => app.initiate_edit(),

        // Add
        KeyCode::Char('a') => app.initiate_add(),

        // Clone
        KeyCode::Char('c') => app.initiate_clone(),

        // Search
        KeyCode::Char('/') => app.start_search(),

        _ => {}
    }
}

fn handle_search(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => app.clear_search(),
        KeyCode::Enter => {
            // Accept filter and return to normal browse (filter stays active but input closes)
            // For now, Esc is the only way to clear. Enter just returns to browse nav.
            // Actually, let's keep it simple: Esc clears filter. No "accept" — filter is live.
            app.clear_search();
        }
        // Arrow navigation still works during search
        KeyCode::Down => app.move_down(),
        KeyCode::Up => app.move_up(),
        _ => {
            if let Some(ref mut input) = app.filter {
                apply_input_key(input, key);
            }
            app.rebuild_rows();
        }
    }
}

fn handle_confirm_delete(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Char('y') => {
            if let Err(e) = app.confirm_delete() {
                app.status_message = Some(format!("Delete failed: {e}"));
                app.cancel_mode();
            }
        }
        KeyCode::Char('n') | KeyCode::Esc => app.cancel_mode(),
        _ => {}
    }
}

fn handle_edit_value(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Enter => {
            if let Err(e) = app.confirm_edit_value() {
                app.status_message = Some(format!("Edit failed: {e}"));
                app.cancel_mode();
            }
        }
        KeyCode::Esc => app.cancel_mode(),
        _ => {
            let Mode::EditValue { ref mut input, .. } = app.mode else {
                return;
            };
            apply_input_key(input, key);
        }
    }
}

fn handle_edit_metadata(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Enter => {
            if let Err(e) = app.confirm_edit_metadata() {
                app.status_message = Some(format!("Edit failed: {e}"));
                app.cancel_mode();
            }
        }
        KeyCode::Esc => app.cancel_mode(),
        KeyCode::Tab => {
            let Mode::EditMetadata {
                ref mut active_field,
                ..
            } = app.mode
            else {
                return;
            };
            *active_field = active_field.next();
        }
        KeyCode::BackTab => {
            let Mode::EditMetadata {
                ref mut active_field,
                ..
            } = app.mode
            else {
                return;
            };
            *active_field = active_field.prev();
        }
        _ => {
            let Mode::EditMetadata {
                ref mut fields,
                active_field,
                ..
            } = app.mode
            else {
                return;
            };

            // Sensitivity field uses left/right to cycle options
            if active_field == MetadataField::Sensitivity {
                match key.code {
                    KeyCode::Left => {
                        if fields.sensitivity > 0 {
                            fields.sensitivity -= 1;
                        }
                    }
                    KeyCode::Right => {
                        if fields.sensitivity < SENSITIVITY_OPTIONS.len() - 1 {
                            fields.sensitivity += 1;
                        }
                    }
                    _ => {}
                }
                return;
            }

            let input = match active_field {
                MetadataField::Description => &mut fields.description,
                MetadataField::Origin => &mut fields.origin,
                MetadataField::Tags => &mut fields.tags,
                MetadataField::Environments => &mut fields.environments,
                MetadataField::Sensitivity => unreachable!(),
            };
            apply_input_key(input, key);
        }
    }
}

fn handle_clone(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => app.cancel_mode(),
        KeyCode::Enter => {
            if let Err(e) = app.confirm_clone() {
                app.status_message = Some(format!("Clone failed: {e}"));
                app.cancel_mode();
            }
        }
        _ => {
            let Mode::Clone { ref mut input, .. } = app.mode else {
                return;
            };
            apply_input_key(input, key);
        }
    }
}

fn handle_add(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => app.cancel_mode(),
        KeyCode::Enter => {
            if let Err(e) = app.advance_add() {
                app.status_message = Some(format!("Add failed: {e}"));
                app.cancel_mode();
            }
        }
        _ => {
            let Mode::Add { ref mut step, .. } = app.mode else {
                return;
            };
            match step {
                AddStep::Sensitivity(idx) => match key.code {
                    KeyCode::Left => {
                        if *idx > 0 {
                            *idx -= 1;
                        }
                    }
                    KeyCode::Right => {
                        if *idx < SENSITIVITY_OPTIONS.len() - 1 {
                            *idx += 1;
                        }
                    }
                    _ => {}
                },
                AddStep::Id(input)
                | AddStep::Env(input)
                | AddStep::Value(input)
                | AddStep::Description(input)
                | AddStep::Origin(input)
                | AddStep::Tags(input) => apply_input_key(input, key),
            }
        }
    }
}

fn handle_add_env(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => app.cancel_mode(),
        KeyCode::Enter => {
            if let Err(e) = app.advance_add_env() {
                app.status_message = Some(format!("Add failed: {e}"));
                app.cancel_mode();
            }
        }
        _ => {
            let Mode::AddEnv { ref mut step, .. } = app.mode else {
                return;
            };
            match step {
                AddEnvStep::Env(input) | AddEnvStep::Value(input) => apply_input_key(input, key),
            }
        }
    }
}

fn apply_input_key(input: &mut super::app::InputState, key: KeyEvent) {
    match key.code {
        KeyCode::Backspace => input.delete_back(),
        KeyCode::Delete => input.delete_forward(),
        KeyCode::Left => input.move_left(),
        KeyCode::Right => input.move_right(),
        KeyCode::Home => input.move_home(),
        KeyCode::End => input.move_end(),
        KeyCode::Char(c) => {
            if !key.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) {
                input.insert_char(c);
            }
        }
        _ => {}
    }
}
