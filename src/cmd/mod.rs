pub mod env;
pub mod hook;
pub mod new;

use crate::err::Error;

use env::EnvCommand;
use hook::HookCommand;
use new::NewCommand;

pub type Result = std::result::Result<(), Error>;

#[derive(Debug)]
pub enum CommandKind {
    Env(Box<EnvCommand>),
    Hook(Box<HookCommand>),
    New(Box<NewCommand>),
}
