pub mod destroy;
pub mod env;
pub mod hook;
pub mod new;

use destroy::DestroyCommand;
use env::EnvCommand;
use hook::HookCommand;
use new::NewCommand;

#[derive(Debug)]
pub enum CommandKind {
    Destroy(Box<DestroyCommand>),
    Env(Box<EnvCommand>),
    Hook(Box<HookCommand>),
    New(Box<NewCommand>),
}
