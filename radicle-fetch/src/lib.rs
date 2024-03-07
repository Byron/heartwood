pub mod git;
pub mod handle;
pub mod policy;
pub mod transport;

pub(crate) mod sigrefs;

mod refs;
mod stage;
mod state;

use gix_protocol::handshake;
pub use handle::Handle;
pub use policy::{Allowed, BlockList, Scope};
use radicle::storage::ReadRepository as _;
pub use state::{FetchLimit, FetchResult};
pub use transport::Transport;

use std::io;
use std::time::Instant;

use radicle::crypto::PublicKey;
use radicle::storage::refs::RefsAt;
use state::FetchState;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("failed to perform fetch handshake")]
    Handshake {
        #[source]
        err: io::Error,
    },
    #[error("failed to load `rad/id`")]
    Identity {
        #[source]
        err: Box<dyn std::error::Error + Send + Sync + 'static>,
    },
    #[error(transparent)]
    Protocol(#[from] state::error::Protocol),
    #[error("missing `rad/id`")]
    MissingRadId,
    #[error("attempted to replicate from self")]
    ReplicateSelf,
}

/// Pull changes from the `remote`.
///
/// It is expected that the local peer has a copy of the repository
/// and is pulling new changes. If the repository does not exist, then
/// [`clone`] should be used.
pub fn pull<S>(
    handle: &mut Handle<S>,
    limit: FetchLimit,
    remote: PublicKey,
    refs_at: Option<Vec<RefsAt>>,
) -> Result<FetchResult, Error>
where
    S: transport::ConnectionStream,
{
    let start = Instant::now();
    let local = *handle.local();
    if local == remote {
        return Err(Error::ReplicateSelf);
    }
    let handshake = perform_handshake(handle)?;
    let state = FetchState::default();

    // N.b. ensure that we ignore the local peer's key.
    handle.blocked.extend([local]);
    let result = state
        .run(handle, &handshake, limit, remote, refs_at)
        .map_err(Error::Protocol);

    log::debug!(
        target: "fetch",
        "Finished pull of {} ({}ms)",
        handle.repo.id(),
        start.elapsed().as_millis()
    );
    result
}

/// Clone changes from the `remote`.
///
/// It is expected that the local peer has an empty repository which
/// they want to populate with the `remote`'s view of the project.
pub fn clone<S>(
    handle: &mut Handle<S>,
    limit: FetchLimit,
    remote: PublicKey,
) -> Result<FetchResult, Error>
where
    S: transport::ConnectionStream,
{
    let start = Instant::now();
    if *handle.local() == remote {
        return Err(Error::ReplicateSelf);
    }
    let handshake = perform_handshake(handle)?;
    let state = FetchState::default();
    let result = state
        .run(handle, &handshake, limit, remote, None)
        .map_err(Error::Protocol);

    log::debug!(
        target: "fetch",
        "Finished clone of {} ({}ms)",
        handle.repo.id(),
        start.elapsed().as_millis(),
    );
    result
}

fn perform_handshake<S>(handle: &mut Handle<S>) -> Result<handshake::Outcome, Error>
where
    S: transport::ConnectionStream,
{
    handle.transport.handshake().map_err(|err| {
        log::warn!(target: "fetch", "Failed to perform handshake: {err}");
        Error::Handshake { err }
    })
}
