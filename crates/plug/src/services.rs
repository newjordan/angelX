use std::io;
use std::process::{ChildStdin, ChildStdout, Command, ExitStatus};

pub trait OwnedChild: Send {
    fn take_stdin(&mut self) -> Option<ChildStdin>;
    fn take_stdout(&mut self) -> Option<ChildStdout>;
    fn alive(&mut self) -> bool;
    fn retire(&mut self) -> io::Result<ExitStatus>;
}

pub trait Processes: Send + Sync {
    fn spawn_owned(&self, command: &mut Command) -> io::Result<Box<dyn OwnedChild>>;
}
