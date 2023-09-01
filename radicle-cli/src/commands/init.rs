#![allow(clippy::or_fun_call)]
use std::convert::TryFrom;
use std::env;
use std::ffi::OsString;
use std::path::PathBuf;

use anyhow::{anyhow, bail, Context as _};
use serde_json as json;

use radicle::crypto::ssh;
use radicle::git::RefString;
use radicle::identity::Visibility;
use radicle::node::tracking::Scope;
use radicle::node::{Handle, NodeId};
use radicle::profile;

use crate::git;
use crate::terminal as term;
use crate::terminal::args::{Args, Error, Help};
use crate::terminal::Interactive;

pub const HELP: Help = Help {
    name: "init",
    description: "Initialize a project from a git repository",
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad init [<path>] [<option>...]

Options

        --name <string>            Name of the project
        --description <string>     Description of the project
        --default-branch <name>    The default branch of the project
        --scope <scope>            Tracking scope (default: all)
    -u, --set-upstream             Setup the upstream of the default branch
        --setup-signing            Setup the radicle key as a signing key for this repository
        --announce                 Announce the new project to the network
        --no-confirm               Don't ask for confirmation during setup
    -v, --verbose                  Verbose mode
        --help                     Print help
"#,
};

#[derive(Default)]
pub struct Options {
    pub path: Option<PathBuf>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub branch: Option<String>,
    pub interactive: Interactive,
    pub setup_signing: bool,
    pub scope: Scope,
    pub set_upstream: bool,
    pub announce: bool,
    pub verbose: bool,
    pub track: bool,
}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;

        let mut parser = lexopt::Parser::from_args(args);
        let mut path: Option<PathBuf> = None;

        let mut name = None;
        let mut description = None;
        let mut branch = None;
        let mut interactive = Interactive::Yes;
        let mut set_upstream = false;
        let mut setup_signing = false;
        let mut announce = false;
        let mut scope = Scope::All;
        let mut track = true;
        let mut verbose = false;

        while let Some(arg) = parser.next()? {
            match arg {
                Long("name") if name.is_none() => {
                    let value = parser
                        .value()?
                        .to_str()
                        .ok_or(anyhow::anyhow!(
                            "invalid project name specified with `--name`"
                        ))?
                        .to_owned();
                    name = Some(value);
                }
                Long("description") if description.is_none() => {
                    let value = parser
                        .value()?
                        .to_str()
                        .ok_or(anyhow::anyhow!(
                            "invalid project description specified with `--description`"
                        ))?
                        .to_owned();

                    description = Some(value);
                }
                Long("default-branch") if branch.is_none() => {
                    let value = parser
                        .value()?
                        .to_str()
                        .ok_or(anyhow::anyhow!(
                            "invalid branch specified with `--default-branch`"
                        ))?
                        .to_owned();

                    branch = Some(value);
                }
                Long("scope") => {
                    let value = parser.value()?;

                    scope = term::args::parse_value("scope", value)?;
                }
                Long("set-upstream") | Short('u') => {
                    set_upstream = true;
                }
                Long("setup-signing") => {
                    setup_signing = true;
                }
                Long("announce") => {
                    announce = true;
                }
                Long("no-confirm") => {
                    interactive = Interactive::No;
                }
                Long("no-track") => {
                    track = false;
                }
                Long("verbose") | Short('v') => {
                    verbose = true;
                }
                Long("help") | Short('h') => {
                    return Err(Error::Help.into());
                }
                Value(val) if path.is_none() => {
                    path = Some(val.into());
                }
                _ => return Err(anyhow::anyhow!(arg.unexpected())),
            }
        }

        Ok((
            Options {
                path,
                name,
                description,
                branch,
                scope,
                interactive,
                set_upstream,
                setup_signing,
                announce,
                track,
                verbose,
            },
            vec![],
        ))
    }
}

pub fn run(options: Options, ctx: impl term::Context) -> anyhow::Result<()> {
    let profile = ctx.profile()?;

    init(options, &profile)
}

pub fn init(options: Options, profile: &profile::Profile) -> anyhow::Result<()> {
    let cwd = std::env::current_dir()?;
    let path = options.path.unwrap_or_else(|| cwd.clone());
    let path = path.as_path().canonicalize()?;
    let interactive = options.interactive;
    let visibility = Visibility::default();

    term::headline(format!(
        "Initializing radicle 👾 project in {}",
        if path == cwd {
            term::format::highlight(".").to_string()
        } else {
            term::format::highlight(path.display()).to_string()
        }
    ));

    let repo = git::Repository::open(&path)?;
    if let Ok((remote, _)) = git::rad_remote(&repo) {
        if let Some(remote) = remote.url() {
            bail!("repository is already initialized with remote {remote}");
        }
    }

    let signer = term::signer(profile)?;
    let head: String = repo
        .head()
        .ok()
        .and_then(|head| head.shorthand().map(|h| h.to_owned()))
        .ok_or_else(|| anyhow!("error: repository head does not point to any commits"))?;

    let name = options.name.unwrap_or_else(|| {
        let default = path.file_name().map(|f| f.to_string_lossy().to_string());
        term::input("Name", default, None).unwrap()
    });
    let description = options
        .description
        .unwrap_or_else(|| term::input("Description", None, None).unwrap());
    let branch = options.branch.unwrap_or_else(|| {
        if interactive.yes() {
            term::input("Default branch", Some(head), None).unwrap()
        } else {
            head
        }
    });
    let branch = RefString::try_from(branch.clone())
        .map_err(|e| anyhow!("invalid branch name {:?}: {}", branch, e))?;

    let mut node = radicle::Node::new(profile.socket());
    let mut spinner = term::spinner("Initializing...");
    let mut push_cmd = String::from("git push");

    match radicle::rad::init(
        &repo,
        &name,
        &description,
        branch,
        visibility,
        &signer,
        &profile.storage,
    ) {
        Ok((id, doc, _)) => {
            let proj = doc.project()?;

            if options.track && node.is_running() {
                // It's important to track our own repositories to make sure that our node signals
                // interest for them. This ensures that messages relating to them are relayed to us.
                node.track_repo(id, options.scope)?;
            }

            spinner.message(format!(
                "Project {} created",
                term::format::highlight(proj.name())
            ));
            spinner.finish();

            if options.verbose {
                term::blob(json::to_string_pretty(&proj)?);
            }

            if options.set_upstream || git::branch_remote(&repo, proj.default_branch()).is_err() {
                // Setup eg. `master` -> `rad/master`
                radicle::git::set_upstream(
                    &repo,
                    &*radicle::rad::REMOTE_NAME,
                    proj.default_branch(),
                    radicle::git::refs::workdir::branch(proj.default_branch()),
                )?;
            } else {
                push_cmd = format!("git push {}", *radicle::rad::REMOTE_NAME);
            }

            if options.setup_signing {
                // Setup radicle signing key.
                self::setup_signing(profile.id(), &repo, interactive)?;
            }

            if node.is_running() {
                let spinner = term::spinner("Syncing inventory..");
                if let Err(e) = node.sync_inventory() {
                    spinner.error(e);
                } else {
                    spinner.finish();
                }
            }

            if options.announce {
                let spinner = term::spinner("Announcing inventory..");
                if let Err(e) = node.announce_inventory() {
                    spinner.error(e);
                } else {
                    spinner.finish();
                }
            }

            term::blank();
            term::info!(
                "Your project's Repository ID {} is {}.",
                term::format::dim("(RID)"),
                term::format::highlight(id.urn())
            );
            term::info!(
                "You can show it any time by running {}",
                term::format::command("rad .")
            );

            if !options.announce {
                term::blank();
                term::info!(
                    "To publish your project to the network, run {}",
                    term::format::command(push_cmd)
                );
            }
        }
        Err(err) => {
            spinner.failed();
            anyhow::bail!(err);

            // TODO: Handle error: "this repository is already initialized with remote {}"
            // TODO: Handle error: "the `{}` branch was either not found, or has no commits"
        }
    }

    Ok(())
}

/// Setup radicle key as commit signing key in repository.
pub fn setup_signing(
    node_id: &NodeId,
    repo: &git::Repository,
    interactive: Interactive,
) -> anyhow::Result<()> {
    let repo = repo
        .workdir()
        .ok_or(anyhow!("cannot setup signing in bare repository"))?;
    let key = ssh::fmt::fingerprint(node_id);
    let yes = if !git::is_signing_configured(repo)? {
        term::headline(format!(
            "Configuring radicle signing key {}...",
            term::format::tertiary(key)
        ));
        true
    } else if interactive.yes() {
        term::confirm(format!(
            "Configure radicle signing key {} in local checkout?",
            term::format::tertiary(key),
        ))
    } else {
        true
    };

    if yes {
        match git::write_gitsigners(repo, [node_id]) {
            Ok(file) => {
                git::ignore(repo, file.as_path())?;

                term::success!("Created {} file", term::format::tertiary(file.display()));
            }
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                let ssh_key = ssh::fmt::key(node_id);
                let gitsigners = term::format::tertiary(".gitsigners");
                term::success!("Found existing {} file", gitsigners);

                let ssh_keys =
                    git::read_gitsigners(repo).context("error reading .gitsigners file")?;

                if ssh_keys.contains(&ssh_key) {
                    term::success!("Signing key is already in {gitsigners} file");
                } else if term::confirm(format!("Add signing key to {gitsigners}?")) {
                    git::add_gitsigners(repo, [node_id])?;
                }
            }
            Err(err) => {
                return Err(err.into());
            }
        }
        git::configure_signing(repo, node_id)?;

        term::success!(
            "Signing configured in {}",
            term::format::tertiary(".git/config")
        );
    }
    Ok(())
}
