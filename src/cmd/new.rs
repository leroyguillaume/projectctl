use crate::cli::NewCommandArguments;

use super::{Command, CommandKind, Result};

#[derive(Debug)]
pub struct NewCommand {
    _args: NewCommandArguments,
}

impl NewCommand {
    pub fn _new(args: NewCommandArguments) -> Self {
        Self { _args: args }
    }
}

impl Command for NewCommand {
    fn kind(self) -> CommandKind {
        CommandKind::New(self)
    }

    fn run(self) -> Result {
        todo!();
    }
}

#[cfg(test)]
mod test {
    use super::*;

    mod new_command {
        use super::*;

        mod new {
            use super::*;

            #[test]
            fn cmd() {
                let args = NewCommandArguments::default_for_test();
                let cmd = NewCommand::_new(args.clone());
                assert_eq!(cmd._args, args);
            }
        }

        mod kind {
            use super::*;

            #[test]
            fn new() {
                let cmd = NewCommand::_new(NewCommandArguments::default_for_test());
                match cmd.kind() {
                    CommandKind::New(_) => (),
                }
            }
        }
    }
}
