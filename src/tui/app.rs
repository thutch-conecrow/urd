use std::collections::HashSet;
use std::path::PathBuf;

use anyhow::Result;

use crate::store::types::{Item, Store, apply_default_environments, save_store};

const UNDO_STACK_LIMIT: usize = 50;

/// A visible row in the item list.
#[derive(Debug, Clone)]
pub enum Row {
    /// An item header: displays the item ID and sensitivity badge.
    ItemHeader(String),
    /// An environment value under an item: (`item_id`, `env_name`).
    EnvValue(String, String),
    /// A declared environment with no value (validation warning).
    MissingEnv(String, String),
}

/// Inline text input state.
#[derive(Debug, Clone)]
pub struct InputState {
    pub buffer: String,
    pub cursor: usize,
}

impl InputState {
    pub fn new(initial: &str) -> Self {
        let cursor = initial.len();
        Self {
            buffer: initial.to_string(),
            cursor,
        }
    }

    pub fn insert_char(&mut self, c: char) {
        self.buffer.insert(self.cursor, c);
        self.cursor += c.len_utf8();
    }

    pub fn delete_back(&mut self) {
        if self.cursor > 0 {
            let prev = self.buffer[..self.cursor]
                .char_indices()
                .next_back()
                .map_or(0, |(i, _)| i);
            self.buffer.drain(prev..self.cursor);
            self.cursor = prev;
        }
    }

    pub fn delete_forward(&mut self) {
        if self.cursor < self.buffer.len() {
            let next = self.buffer[self.cursor..]
                .char_indices()
                .nth(1)
                .map_or(self.buffer.len(), |(i, _)| self.cursor + i);
            self.buffer.drain(self.cursor..next);
        }
    }

    pub fn move_left(&mut self) {
        if self.cursor > 0 {
            self.cursor = self.buffer[..self.cursor]
                .char_indices()
                .next_back()
                .map_or(0, |(i, _)| i);
        }
    }

    pub fn move_right(&mut self) {
        if self.cursor < self.buffer.len() {
            self.cursor = self.buffer[self.cursor..]
                .char_indices()
                .nth(1)
                .map_or(self.buffer.len(), |(i, _)| self.cursor + i);
        }
    }

    pub const fn move_home(&mut self) {
        self.cursor = 0;
    }

    pub const fn move_end(&mut self) {
        self.cursor = self.buffer.len();
    }

}

/// The current interaction mode.
#[derive(Debug, Clone)]
pub enum Mode {
    Browse,
    /// Confirming deletion.
    ConfirmDelete(DeleteTarget),
    /// Editing an environment value inline.
    EditValue {
        item_id: String,
        env: String,
        input: InputState,
    },
    /// Editing catalog metadata for an item.
    EditMetadata {
        item_id: String,
        fields: MetadataFields,
        active_field: MetadataField,
    },
    /// Adding a new item (multi-step wizard).
    Add {
        step: AddStep,
        /// Accumulated values from previous steps.
        id: String,
        env: String,
        sensitivity: usize,
        value: String,
        description: String,
        origin: String,
    },
    /// Cloning a value to another environment.
    Clone {
        item_id: String,
        source_env: String,
        input: InputState,
    },
    /// Adding a new environment value to an existing item.
    AddEnv {
        item_id: String,
        step: AddEnvStep,
        env: String,
    },
    /// Cloning an entire item to a new ID.
    CloneItem {
        source_id: String,
        input: InputState,
    },
}

/// Steps for adding an env to an existing item.
#[derive(Debug, Clone)]
pub enum AddEnvStep {
    Env(InputState),
    Value(InputState),
}

/// Which metadata field is currently focused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetadataField {
    Description,
    Sensitivity,
    Origin,
    Tags,
    Environments,
}

impl MetadataField {
    pub const ALL: [Self; 5] = [
        Self::Description,
        Self::Sensitivity,
        Self::Origin,
        Self::Tags,
        Self::Environments,
    ];

    pub const fn next(self) -> Self {
        match self {
            Self::Description => Self::Sensitivity,
            Self::Sensitivity => Self::Origin,
            Self::Origin => Self::Tags,
            Self::Tags => Self::Environments,
            Self::Environments => Self::Description,
        }
    }

    pub const fn prev(self) -> Self {
        match self {
            Self::Description => Self::Environments,
            Self::Sensitivity => Self::Description,
            Self::Origin => Self::Sensitivity,
            Self::Tags => Self::Origin,
            Self::Environments => Self::Tags,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Description => "description",
            Self::Sensitivity => "sensitivity",
            Self::Origin => "origin",
            Self::Tags => "tags",
            Self::Environments => "environments",
        }
    }
}

/// Editable metadata fields.
#[derive(Debug, Clone)]
pub struct MetadataFields {
    pub description: InputState,
    pub sensitivity: usize, // index into SENSITIVITY_OPTIONS
    pub origin: InputState,
    pub tags: InputState,
    pub environments: InputState,
}

pub const SENSITIVITY_OPTIONS: [&str; 3] = ["plaintext", "sensitive", "secret"];

/// Which step of the add flow we're on.
#[derive(Debug, Clone)]
pub enum AddStep {
    Id(InputState),
    Env(InputState),
    Sensitivity(usize),
    Value(InputState),
    Description(InputState),
    Origin(InputState),
    Tags(InputState),
}

impl AddStep {
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Id(_) => "Item ID",
            Self::Env(_) => "Environment",
            Self::Sensitivity(_) => "Sensitivity",
            Self::Value(_) => "Value",
            Self::Description(_) => "Description (optional, Enter to skip)",
            Self::Origin(_) => "Origin (optional, Enter to skip)",
            Self::Tags(_) => "Tags (optional, comma-separated, Enter to skip)",
        }
    }
}

/// What the delete confirmation is targeting.
#[derive(Debug, Clone)]
pub enum DeleteTarget {
    /// Delete an entire item and all its values.
    Item(String),
    /// Delete a single environment value from an item.
    EnvValue(String, String),
}

pub struct App {
    pub store: Store,
    pub store_path: PathBuf,
    pub mode: Mode,
    /// Active search filter (None = no filter).
    pub filter: Option<InputState>,
    /// Which item IDs are expanded (showing their env values).
    pub expanded: HashSet<String>,
    /// Which item IDs have their values revealed.
    pub revealed: HashSet<String>,
    /// Whether all items are revealed globally.
    pub reveal_all: bool,
    /// The currently selected index into `visible_rows`.
    pub selected: usize,
    /// Scroll offset for the list panel.
    pub scroll_offset: usize,
    /// Computed list of visible rows based on expanded state.
    pub visible_rows: Vec<Row>,
    /// Undo stack: previous store states.
    pub undo_stack: Vec<Store>,
    /// Redo stack: states undone that can be restored.
    pub redo_stack: Vec<Store>,
    /// Status message shown briefly after an action.
    pub status_message: Option<String>,
    pub should_quit: bool,
}

impl App {
    pub fn new(store: Store, store_path: PathBuf) -> Self {
        let mut app = Self {
            store,
            store_path,
            mode: Mode::Browse,
            filter: None,
            expanded: HashSet::new(),
            revealed: HashSet::new(),
            reveal_all: false,
            selected: 0,
            scroll_offset: 0,
            visible_rows: Vec::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            status_message: None,
            should_quit: false,
        };
        app.rebuild_rows();
        app
    }

    /// Rebuild the flat list of visible rows from the store and expanded state.
    pub fn rebuild_rows(&mut self) {
        self.visible_rows.clear();

        let filter_str = self
            .filter
            .as_ref()
            .map(|f| f.buffer.to_lowercase());

        for (id, item) in self.store.iter() {
            if let Some(ref f) = filter_str
                && !f.is_empty()
                && !id.to_lowercase().contains(f.as_str())
            {
                continue;
            }
            self.visible_rows.push(Row::ItemHeader(id.clone()));
            if self.expanded.contains(id) {
                for env in item.values.keys() {
                    self.visible_rows
                        .push(Row::EnvValue(id.clone(), env.clone()));
                }
                // Show declared envs that are missing values
                for env in &item.environments {
                    if !item.values.contains_key(env) {
                        self.visible_rows
                            .push(Row::MissingEnv(id.clone(), env.clone()));
                    }
                }
            }
        }
        // Clamp selected index
        if self.visible_rows.is_empty() {
            self.selected = 0;
        } else if self.selected >= self.visible_rows.len() {
            self.selected = self.visible_rows.len() - 1;
        }
    }

    /// Get the item ID for the currently selected row.
    pub fn selected_item_id(&self) -> Option<&str> {
        self.visible_rows.get(self.selected).map(|row| match row {
            Row::ItemHeader(id) | Row::EnvValue(id, _) | Row::MissingEnv(id, _) => id.as_str(),
        })
    }

    /// Get the `Item` for the currently selected row.
    pub fn selected_item(&self) -> Option<(&str, &Item)> {
        self.selected_item_id()
            .and_then(|id| self.store.get(id).map(|item| (id, item)))
    }

    /// Get the currently selected row (cloned).
    pub fn selected_row(&self) -> Option<&Row> {
        self.visible_rows.get(self.selected)
    }

    pub const fn move_down(&mut self) {
        if !self.visible_rows.is_empty() && self.selected < self.visible_rows.len() - 1 {
            self.selected += 1;
        }
    }

    pub const fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn expand_selected(&mut self) {
        if let Some(id) = self.selected_item_id() {
            let id = id.to_string();
            if self.expanded.insert(id) {
                self.rebuild_rows();
            }
        }
    }

    pub fn collapse_selected(&mut self) {
        if let Some(id) = self.selected_item_id() {
            let id = id.to_string();
            if self.expanded.remove(&id) {
                self.rebuild_rows();
                if let Some(pos) = self
                    .visible_rows
                    .iter()
                    .position(|r| matches!(r, Row::ItemHeader(hid) if hid == &id))
                {
                    self.selected = pos;
                }
            }
        }
    }

    pub fn expand_all(&mut self) {
        for id in self.store.keys() {
            self.expanded.insert(id.clone());
        }
        self.rebuild_rows();
    }

    pub fn collapse_all(&mut self) {
        self.expanded.clear();
        self.rebuild_rows();
    }

    /// Whether a given item's values should be shown in plaintext.
    pub fn is_revealed(&self, id: &str) -> bool {
        self.reveal_all || self.revealed.contains(id)
    }

    /// Toggle reveal for the currently selected item.
    pub fn toggle_reveal_selected(&mut self) {
        if let Some(id) = self.selected_item_id() {
            let id = id.to_string();
            if !self.revealed.remove(&id) {
                self.revealed.insert(id);
            }
        }
    }

    /// Activate search/filter mode.
    pub fn start_search(&mut self) {
        self.filter = Some(InputState::new(""));
    }

    /// Clear the search filter.
    pub fn clear_search(&mut self) {
        self.filter = None;
        self.rebuild_rows();
    }

    /// Toggle global reveal.
    pub const fn toggle_reveal_all(&mut self) {
        self.reveal_all = !self.reveal_all;
    }

    /// Snapshot the current store onto the undo stack (call before mutating).
    fn push_undo(&mut self) {
        if self.undo_stack.len() >= UNDO_STACK_LIMIT {
            self.undo_stack.remove(0);
        }
        self.undo_stack.push(self.store.clone());
        self.redo_stack.clear();
    }

    /// Undo the last mutation.
    pub fn undo(&mut self) -> Result<()> {
        if let Some(prev) = self.undo_stack.pop() {
            if self.redo_stack.len() >= UNDO_STACK_LIMIT {
                self.redo_stack.remove(0);
            }
            self.redo_stack.push(self.store.clone());
            self.store = prev;
            save_store(&self.store_path, &self.store)?;
            self.rebuild_rows();
            self.status_message = Some("Undone".into());
        } else {
            self.status_message = Some("Nothing to undo".into());
        }
        Ok(())
    }

    /// Redo the last undone mutation.
    pub fn redo(&mut self) -> Result<()> {
        if let Some(next) = self.redo_stack.pop() {
            if self.undo_stack.len() >= UNDO_STACK_LIMIT {
                self.undo_stack.remove(0);
            }
            self.undo_stack.push(self.store.clone());
            self.store = next;
            save_store(&self.store_path, &self.store)?;
            self.rebuild_rows();
            self.status_message = Some("Redone".into());
        } else {
            self.status_message = Some("Nothing to redo".into());
        }
        Ok(())
    }

    /// Initiate delete confirmation for the currently selected row.
    pub fn initiate_delete(&mut self) {
        let target = match self.selected_row() {
            Some(Row::ItemHeader(id)) => DeleteTarget::Item(id.clone()),
            Some(Row::EnvValue(id, env)) => DeleteTarget::EnvValue(id.clone(), env.clone()),
            Some(Row::MissingEnv(_, _)) | None => return,
        };
        self.mode = Mode::ConfirmDelete(target);
    }

    /// Execute a confirmed delete.
    pub fn confirm_delete(&mut self) -> Result<()> {
        let Mode::ConfirmDelete(ref target) = self.mode else {
            return Ok(());
        };
        let target = target.clone();
        self.push_undo();

        match &target {
            DeleteTarget::Item(id) => {
                self.store.remove(id);
                self.expanded.remove(id);
                self.revealed.remove(id);
                self.status_message = Some(format!("Deleted {id}"));
            }
            DeleteTarget::EnvValue(id, env) => {
                if let Some(item) = self.store.get_mut(id) {
                    item.values.remove(env);
                }
                self.status_message = Some(format!("Removed {env} from {id}"));
            }
        }

        save_store(&self.store_path, &self.store)?;
        self.rebuild_rows();
        self.mode = Mode::Browse;
        Ok(())
    }

    /// Enter edit mode — dispatches based on whether cursor is on header or env value.
    pub fn initiate_edit(&mut self) {
        match self.selected_row() {
            Some(Row::ItemHeader(_)) => self.initiate_edit_metadata(),
            Some(Row::EnvValue(_, _)) => self.initiate_edit_value(),
            Some(Row::MissingEnv(id, env)) => {
            self.mode = Mode::EditValue {
                item_id: id.clone(),
                env: env.clone(),
                input: InputState::new(""),
            };
        }
        None => {}
        }
    }

    /// Enter metadata edit mode for the currently selected item header.
    fn initiate_edit_metadata(&mut self) {
        let Some(Row::ItemHeader(id)) = self.selected_row() else {
            return;
        };
        let id = id.clone();
        let Some(item) = self.store.get(&id) else {
            return;
        };

        let sensitivity_index = item
            .sensitivity
            .as_ref()
            .map_or(0, |s| match s {
                crate::store::types::Sensitivity::Plaintext => 0,
                crate::store::types::Sensitivity::Sensitive => 1,
                crate::store::types::Sensitivity::Secret => 2,
            });

        let fields = MetadataFields {
            description: InputState::new(item.description.as_deref().unwrap_or("")),
            sensitivity: sensitivity_index,
            origin: InputState::new(item.origin.as_deref().unwrap_or("")),
            tags: InputState::new(&item.tags.join(", ")),
            environments: InputState::new(&item.environments.join(", ")),
        };

        self.mode = Mode::EditMetadata {
            item_id: id,
            fields,
            active_field: MetadataField::Description,
        };
    }

    /// Confirm metadata edits and save.
    pub fn confirm_edit_metadata(&mut self) -> Result<()> {
        let Mode::EditMetadata {
            ref item_id,
            ref fields,
            ..
        } = self.mode
        else {
            return Ok(());
        };

        let item_id = item_id.clone();
        let desc = fields.description.buffer.clone();
        let sens_idx = fields.sensitivity;
        let origin = fields.origin.buffer.clone();
        let tags_str = fields.tags.buffer.clone();
        let envs_str = fields.environments.buffer.clone();

        self.push_undo();

        if let Some(item) = self.store.get_mut(&item_id) {
            item.description = if desc.is_empty() { None } else { Some(desc) };
            item.sensitivity = Some(match sens_idx {
                1 => crate::store::types::Sensitivity::Sensitive,
                2 => crate::store::types::Sensitivity::Secret,
                _ => crate::store::types::Sensitivity::Plaintext,
            });
            item.origin = if origin.is_empty() {
                None
            } else {
                Some(origin)
            };
            item.tags = if tags_str.is_empty() {
                Vec::new()
            } else {
                tags_str.split(',').map(|s| s.trim().to_string()).collect()
            };
            item.environments = if envs_str.is_empty() {
                Vec::new()
            } else {
                envs_str.split(',').map(|s| s.trim().to_string()).collect()
            };
        }

        save_store(&self.store_path, &self.store)?;
        self.status_message = Some(format!("Updated metadata for {item_id}"));
        self.mode = Mode::Browse;
        Ok(())
    }

    /// Enter edit-value mode for any row that has an environment (`EnvValue` or `MissingEnv`).
    pub fn initiate_edit_value_any(&mut self) {
        match self.selected_row() {
            Some(Row::EnvValue(_, _)) => self.initiate_edit_value(),
            Some(Row::MissingEnv(id, env)) => {
                self.mode = Mode::EditValue {
                    item_id: id.clone(),
                    env: env.clone(),
                    input: InputState::new(""),
                };
            }
            Some(Row::ItemHeader(_)) | None => {}
        }
    }

    /// Enter edit mode for the currently selected env value.
    fn initiate_edit_value(&mut self) {
        let Some(Row::EnvValue(id, env)) = self.selected_row() else {
            return;
        };
        let id = id.clone();
        let env = env.clone();

        // Get the current value, decrypting if needed
        let current = self
            .store
            .get(&id)
            .and_then(|item| item.values.get(&env))
            .map_or_else(String::new, |v| {
                if crate::crypto::parse_sensitivity(v).is_some() {
                    crate::crypto::decrypt_value(v).unwrap_or_default()
                } else {
                    v.clone()
                }
            });

        self.mode = Mode::EditValue {
            item_id: id,
            env,
            input: InputState::new(&current),
        };
    }

    /// Confirm the current edit and save.
    pub fn confirm_edit_value(&mut self) -> Result<()> {
        let Mode::EditValue {
            ref item_id,
            ref env,
            ref input,
        } = self.mode
        else {
            return Ok(());
        };

        let item_id = item_id.clone();
        let env = env.clone();
        let new_value = input.buffer.clone();

        self.push_undo();

        // Re-encrypt: check catalog sensitivity first, then fall back to existing encrypted value
        let stored_value = if let Some(item) = self.store.get(&item_id) {
            let level = item
                .sensitivity
                .as_ref()
                .and_then(crate::store::types::Sensitivity::to_sensitivity_level)
                .or_else(|| {
                    item.values
                        .get(&env)
                        .and_then(|v| crate::crypto::parse_sensitivity(v))
                });
            if let Some(level) = level {
                crate::crypto::encrypt_value(&new_value, level)?
            } else {
                new_value
            }
        } else {
            new_value
        };

        if let Some(item) = self.store.get_mut(&item_id) {
            item.values.insert(env.clone(), stored_value);
        }

        save_store(&self.store_path, &self.store)?;
        self.rebuild_rows();
        self.status_message = Some(format!("Updated {env} for {item_id}"));
        self.mode = Mode::Browse;
        Ok(())
    }

    /// Initiate clone mode for the currently selected row.
    pub fn initiate_clone(&mut self) {
        match self.selected_row() {
            Some(Row::EnvValue(id, env)) => {
                self.mode = Mode::Clone {
                    item_id: id.clone(),
                    source_env: env.clone(),
                    input: InputState::new(""),
                };
            }
            Some(Row::ItemHeader(id)) => {
                self.mode = Mode::CloneItem {
                    source_id: id.clone(),
                    input: InputState::new(""),
                };
            }
            _ => {}
        }
    }

    /// Confirm clone: copy the value to the target environment.
    pub fn confirm_clone(&mut self) -> Result<()> {
        let Mode::Clone {
            ref item_id,
            ref source_env,
            ref input,
        } = self.mode
        else {
            return Ok(());
        };

        let target_env = input.buffer.trim().to_string();
        if target_env.is_empty() {
            return Ok(());
        }

        let item_id = item_id.clone();
        let source_env = source_env.clone();

        let source_value = self
            .store
            .get(&item_id)
            .and_then(|item| item.values.get(&source_env))
            .cloned();

        let Some(value) = source_value else {
            self.cancel_mode();
            return Ok(());
        };

        self.push_undo();

        let meta = self.store.meta.clone();
        if let Some(item) = self.store.get_mut(&item_id) {
            item.values.insert(target_env.clone(), value);
            if !item.environments.contains(&target_env) {
                item.environments.push(target_env.clone());
                item.environments.sort();
            }
            apply_default_environments(&meta, item);
        }

        save_store(&self.store_path, &self.store)?;
        self.rebuild_rows();
        self.status_message = Some(format!("Cloned {source_env} → {target_env} for {item_id}"));
        self.mode = Mode::Browse;
        Ok(())
    }

    /// Confirm clone-item: duplicate entire item to a new ID.
    pub fn confirm_clone_item(&mut self) -> Result<()> {
        let Mode::CloneItem {
            ref source_id,
            ref input,
        } = self.mode
        else {
            return Ok(());
        };

        let new_id = input.buffer.trim().to_string();
        if new_id.is_empty() {
            return Ok(());
        }

        let source_id = source_id.clone();

        let Some(source_item) = self.store.get(&source_id).cloned() else {
            self.cancel_mode();
            return Ok(());
        };

        self.push_undo();
        self.store.items.insert(new_id.clone(), source_item);
        save_store(&self.store_path, &self.store)?;
        self.expanded.insert(new_id.clone());
        self.rebuild_rows();
        self.status_message = Some(format!("Cloned {source_id} → {new_id}"));
        self.mode = Mode::Browse;
        Ok(())
    }

    /// Initiate add mode based on context.
    pub fn initiate_add(&mut self) {
        match self.selected_row() {
            Some(Row::EnvValue(id, _) | Row::MissingEnv(id, _)) => {
                // Add a new env to this existing item — pre-fill with next missing default
                let id = id.clone();
                let prefill = self.next_missing_default_env(&id).unwrap_or_default();
                self.mode = Mode::AddEnv {
                    item_id: id,
                    step: AddEnvStep::Env(InputState::new(&prefill)),
                    env: String::new(),
                };
            }
            _ => {
                // Add a new item
                self.mode = Mode::Add {
                    step: AddStep::Id(InputState::new("")),
                    id: String::new(),
                    env: String::new(),
                    sensitivity: 0,
                    value: String::new(),
                    description: String::new(),
                    origin: String::new(),
                };
            }
        }
    }

    /// Find the next default environment that the item is missing a value for.
    fn next_missing_default_env(&self, item_id: &str) -> Option<String> {
        let item = self.store.get(item_id)?;
        self.store
            .meta
            .default_environments
            .iter()
            .find(|env| !item.values.contains_key(env.as_str()))
            .cloned()
    }

    /// Advance the add wizard to the next step, or save if on the last step.
    pub fn advance_add(&mut self) -> Result<()> {
        let Mode::Add {
            ref step,
            ref mut id,
            ref mut env,
            ref mut sensitivity,
            ref mut value,
            ref mut description,
            ref mut origin,
        } = self.mode
        else {
            return Ok(());
        };

        let next_step = match step {
            AddStep::Id(input) => {
                if input.buffer.trim().is_empty() {
                    return Ok(());
                }
                *id = input.buffer.trim().to_string();
                let env_prefill = self
                    .store
                    .meta
                    .default_environments
                    .first()
                    .map_or("", String::as_str);
                AddStep::Env(InputState::new(env_prefill))
            }
            AddStep::Env(input) => {
                if input.buffer.trim().is_empty() {
                    return Ok(());
                }
                *env = input.buffer.trim().to_string();
                AddStep::Sensitivity(0)
            }
            AddStep::Sensitivity(idx) => {
                *sensitivity = *idx;
                AddStep::Value(InputState::new(""))
            }
            AddStep::Value(input) => {
                if input.buffer.trim().is_empty() {
                    return Ok(());
                }
                value.clone_from(&input.buffer);
                AddStep::Description(InputState::new(""))
            }
            AddStep::Description(input) => {
                description.clone_from(&input.buffer);
                AddStep::Origin(InputState::new(""))
            }
            AddStep::Origin(input) => {
                origin.clone_from(&input.buffer);
                AddStep::Tags(InputState::new(""))
            }
            AddStep::Tags(input) => {
                // Final step — save everything
                let tags_str = input.buffer.clone();
                let id = id.clone();
                let env = env.clone();
                let sens = *sensitivity;
                let val = value.clone();
                let desc = description.clone();
                let orig = origin.clone();

                self.push_undo();

                let meta = self.store.meta.clone();
                let item = self.store.entry(id.clone()).or_default();
                let stored_value = match sens {
                    1 => crate::crypto::encrypt_value(&val, crate::crypto::SensitivityLevel::Sensitive)?,
                    2 => crate::crypto::encrypt_value(&val, crate::crypto::SensitivityLevel::Secret)?,
                    _ => val,
                };
                item.values.insert(env.clone(), stored_value);
                if !desc.is_empty() {
                    item.description = Some(desc);
                }
                if !orig.is_empty() {
                    item.origin = Some(orig);
                }
                if !tags_str.is_empty() {
                    item.tags = tags_str.split(',').map(|s| s.trim().to_string()).collect();
                }
                item.sensitivity = Some(match sens {
                    1 => crate::store::types::Sensitivity::Sensitive,
                    2 => crate::store::types::Sensitivity::Secret,
                    _ => crate::store::types::Sensitivity::Plaintext,
                });
                if !item.environments.contains(&env) {
                    item.environments.push(env.clone());
                    item.environments.sort();
                }
                apply_default_environments(&meta, item);

                save_store(&self.store_path, &self.store)?;
                self.expanded.insert(id.clone());
                self.rebuild_rows();
                self.status_message = Some(format!("Added {id} ({env})"));
                self.mode = Mode::Browse;
                return Ok(());
            }
        };

        // Can't directly assign due to borrow — we need to reconstruct
        if let Mode::Add { ref mut step, .. } = self.mode {
            *step = next_step;
        }
        Ok(())
    }

    /// Advance the add-env wizard.
    pub fn advance_add_env(&mut self) -> Result<()> {
        let Mode::AddEnv {
            ref item_id,
            ref step,
            ref mut env,
        } = self.mode
        else {
            return Ok(());
        };

        match step {
            AddEnvStep::Env(input) => {
                if input.buffer.trim().is_empty() {
                    return Ok(());
                }
                *env = input.buffer.trim().to_string();
                if let Mode::AddEnv { ref mut step, .. } = self.mode {
                    *step = AddEnvStep::Value(InputState::new(""));
                }
            }
            AddEnvStep::Value(input) => {
                if input.buffer.trim().is_empty() {
                    return Ok(());
                }
                let item_id = item_id.clone();
                let env = env.clone();
                let val = input.buffer.clone();

                self.push_undo();

                // Encrypt: check catalog sensitivity first, then fall back to existing encrypted values
                let stored_value = if let Some(item) = self.store.get(&item_id) {
                    let level = item
                        .sensitivity
                        .as_ref()
                        .and_then(crate::store::types::Sensitivity::to_sensitivity_level)
                        .or_else(|| {
                            item.values
                                .values()
                                .find_map(|v| crate::crypto::parse_sensitivity(v))
                        });
                    if let Some(level) = level {
                        crate::crypto::encrypt_value(&val, level)?
                    } else {
                        val
                    }
                } else {
                    val
                };

                let meta = self.store.meta.clone();
                if let Some(item) = self.store.get_mut(&item_id) {
                    item.values.insert(env.clone(), stored_value);
                    if !item.environments.contains(&env) {
                        item.environments.push(env.clone());
                        item.environments.sort();
                    }
                    apply_default_environments(&meta, item);
                }

                save_store(&self.store_path, &self.store)?;
                self.rebuild_rows();
                self.status_message = Some(format!("Added {env} to {item_id}"));
                self.mode = Mode::Browse;
            }
        }
        Ok(())
    }

    /// Cancel the current mode and return to browse.
    pub fn cancel_mode(&mut self) {
        self.mode = Mode::Browse;
    }

    /// Adjust `scroll_offset` so that `selected` is visible within `viewport_height` lines.
    pub const fn adjust_scroll(&mut self, viewport_height: usize) {
        if viewport_height == 0 {
            return;
        }
        if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        } else if self.selected >= self.scroll_offset + viewport_height {
            self.scroll_offset = self.selected - viewport_height + 1;
        }
    }
}
