pub mod new;

use crate::err::Error;

use new::NewCommand;

pub type Result = std::result::Result<(), Error>;

#[derive(Debug)]
pub enum CommandKind {
    New(NewCommand),
}

pub trait Command {
    fn kind(self) -> CommandKind;

    fn run(self) -> Result;
}
