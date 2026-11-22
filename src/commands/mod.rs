pub mod add;
pub mod completions;
pub mod create;
pub mod delete;
pub mod doctor;
pub mod edit;
pub mod info;
pub mod list;
pub mod modulos;
pub mod open;
pub mod search;
pub mod settings_cmd;
pub mod update;

/// Flags shared by every command.
#[derive(Clone, Copy, Debug)]
pub struct Global {
    pub yes: bool,
    pub dry_run: bool,
}
