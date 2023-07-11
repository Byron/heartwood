pub mod args;
pub use args::{Args, Error, Help};
pub mod format;
pub mod io;
pub use io::{proposal, signer};
pub mod patch;

use std::ffi::OsString;
use std::process;

pub use radicle_term::*;

use radicle::profile::Profile;

use crate::terminal;

/// Context passed to all commands.
pub trait Context {
    /// Return the currently active profile, or an error if no profile is active.
    fn profile(&self) -> Result<Profile, anyhow::Error>;
}

impl Context for Profile {
    fn profile(&self) -> Result<Profile, anyhow::Error> {
        Ok(self.clone())
    }
}

impl<F> Context for F
where
    F: Fn() -> Result<Profile, anyhow::Error>,
{
    fn profile(&self) -> Result<Profile, anyhow::Error> {
        self()
    }
}

/// A command that can be run.
pub trait Command<A: Args, C: Context> {
    /// Run the command, given arguments and a context.
    fn run(self, args: A, context: C) -> anyhow::Result<()>;
}

impl<F, A: Args, C: Context> Command<A, C> for F
where
    F: FnOnce(A, C) -> anyhow::Result<()>,
{
    fn run(self, args: A, context: C) -> anyhow::Result<()> {
        self(args, context)
    }
}

pub fn run_command<A, C>(help: Help, action: &str, cmd: C) -> !
where
    A: Args,
    C: Command<A, fn() -> anyhow::Result<Profile>>,
{
    let args = std::env::args_os().skip(1).collect();

    run_command_args(help, action, cmd, args)
}

pub fn run_command_args<A, C>(help: Help, action: &str, cmd: C, args: Vec<OsString>) -> !
where
    A: Args,
    C: Command<A, fn() -> anyhow::Result<Profile>>,
{
    use io as term;

    let options = match A::from_args(args) {
        Ok((opts, unparsed)) => {
            if let Err(err) = args::finish(unparsed) {
                term::error(err);
                process::exit(1);
            }
            opts
        }
        Err(err) => {
            match err.downcast_ref::<Error>() {
                Some(Error::Help) => {
                    term::help(help.name, help.version, help.description, help.usage);
                    process::exit(0);
                }
                Some(Error::Usage) => {
                    term::usage(help.name, help.usage);
                    process::exit(1);
                }
                _ => {}
            };
            eprintln!(
                "{} {} rad {}: {err}",
                Paint::red(ERROR_PREFIX),
                Paint::red("Error:"),
                help.name,
            );

            if let Some(Error::WithHint { hint, .. }) = err.downcast_ref::<Error>() {
                eprintln!("{}", Paint::yellow(hint));
            }

            process::exit(1);
        }
    };

    match cmd.run(options, self::profile) {
        Ok(()) => process::exit(0),
        Err(err) => {
            terminal::fail(&format!("{action} failed"), &err);
            process::exit(1);
        }
    }
}

/// Get the default profile. Fails if there is no profile.
pub fn profile() -> Result<Profile, anyhow::Error> {
    match Profile::load() {
        Ok(profile) => Ok(profile),
        Err(e) => Err(args::Error::WithHint {
            err: anyhow::anyhow!("Could not load radicle profile: {e}"),
            hint: "To setup your radicle profile, run `rad auth`.",
        }
        .into()),
    }
}

pub fn fail(header: &str, error: &anyhow::Error) {
    let err = error.to_string();
    let err = err.trim_end();
    let separator = if err.contains('\n') { ":\n" } else { ": " };

    println!(
        "{ERROR_PREFIX} {}{}{error}",
        Paint::red(header).bold(),
        Paint::red(separator),
    );

    if let Some(Error::WithHint { hint, .. }) = error.downcast_ref::<Error>() {
        println!("{} {}", ERROR_HINT_PREFIX, Paint::yellow(hint));
        blank();
    }
}
