pub mod env;
pub mod new;

use crate::err::Error;

use env::EnvCommand;
use new::NewCommand;

pub type Result = std::result::Result<(), Error>;

#[derive(Debug)]
pub enum CommandKind {
    Env(Box<EnvCommand>),
    New(Box<NewCommand>),
}
