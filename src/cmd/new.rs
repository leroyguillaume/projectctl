use std::fmt::{self, Debug, Formatter};

use crate::{
    cli::NewCommandArguments,
    git::{DefaultGit, Git},
};

use super::{Command, CommandKind, Result};

pub struct NewCommand {
    _args: NewCommandArguments,
    _git: Box<dyn Git>,
}

impl NewCommand {
    pub fn new(args: NewCommandArguments) -> Self {
        Self {
            _args: args,
            _git: Box::new(DefaultGit),
        }
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

impl Debug for NewCommand {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("NewCommand")
            .field("args", &self._args)
            .finish()
    }
}

#[cfg(test)]
mod test {
    use crate::git::StubGit;

    use super::*;

    mod new_command {
        use super::*;

        mod new {
            use super::*;

            #[test]
            fn cmd() {
                let args = NewCommandArguments::default_for_test();
                let cmd = NewCommand::new(args.clone());
                assert_eq!(cmd._args, args);
            }
        }

        mod kind {
            use super::*;

            #[test]
            fn new() {
                let cmd = NewCommand {
                    _args: NewCommandArguments::default_for_test(),
                    _git: Box::new(StubGit::new()),
                };
                match cmd.kind() {
                    CommandKind::New(_) => (),
                }
            }
        }
    }
}
