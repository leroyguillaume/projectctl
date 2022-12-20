pub mod env;
pub mod hook;
pub mod new;

use env::EnvCommand;
use hook::HookCommand;
use new::NewCommand;

#[derive(Debug)]
pub enum CommandKind {
    Env(Box<EnvCommand>),
    Hook(Box<HookCommand>),
    New(Box<NewCommand>),
}
