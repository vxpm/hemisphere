pub mod command;

#[derive(Debug, Default)]
pub struct Interface {
    pub command: command::Interface,
}
