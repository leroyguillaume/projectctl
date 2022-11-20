use crate::err::Error;

pub type Result = std::result::Result<(), Error>;

#[derive(Debug)]
pub enum CommandKind {}

pub trait Command {
    fn kind(&self) -> CommandKind;

    fn run(&self) -> Result;
}
