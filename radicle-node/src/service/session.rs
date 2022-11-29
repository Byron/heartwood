use crate::service::message;
use crate::service::message::Message;
use crate::service::storage;
use crate::service::{Link, LocalTime, NodeId, Reactor, Rng};

#[derive(Debug, Copy, Clone, Default, PartialEq, Eq)]
pub enum PingState {
    #[default]
    /// The peer has not been sent a ping.
    None,
    /// A ping has been sent and is waiting on the peer's response.
    AwaitingResponse(u16),
    /// The peer was successfully pinged.
    Ok,
}

#[derive(Debug, Default, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum State {
    /// Pre-handshake state.
    /// TODO(cloudhead): Remove once noise handshake is implemented.
    #[default]
    Initial,
    /// State after successful handshake.
    Negotiated {
        /// The peer's unique identifier.
        id: NodeId,
        since: LocalTime,
        ping: PingState,
    },
    /// When a peer is disconnected.
    Disconnected { since: LocalTime },
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("wrong protocol version in message: {0}")]
    WrongVersion(u32),
    #[error("invalid announcement timestamp: {0}")]
    InvalidTimestamp(u64),
    #[error("session not found for node `{0}`")]
    NotFound(NodeId),
    #[error("verification failed on fetch: {0}")]
    VerificationFailed(#[from] storage::VerifyError),
    #[error("peer misbehaved")]
    Misbehavior,
    #[error("peer timed out")]
    Timeout,
    #[error("handshake error")]
    Handshake(String),
}

/// A peer session. Each connected peer will have one session.
#[derive(Debug, Clone)]
pub struct Session {
    /// Peer id.
    pub id: NodeId,
    /// Connection direction.
    pub link: Link,
    /// Whether we should attempt to re-connect
    /// to this peer upon disconnection.
    pub persistent: bool,
    /// Peer connection state.
    pub state: State,
    /// Peer subscription.
    pub subscribe: Option<message::Subscribe>,
    /// Last time a message was received from the peer.
    pub last_active: LocalTime,

    /// Connection attempts. For persistent peers, Tracks
    /// how many times we've attempted to connect. We reset this to zero
    /// upon successful connection.
    attempts: usize,

    /// Source of entropy.
    rng: Rng,
}

impl Session {
    pub fn new(id: NodeId, link: Link, persistent: bool, rng: Rng) -> Self {
        Self {
            id,
            state: State::Initial,
            link,
            subscribe: None,
            persistent,
            last_active: LocalTime::default(),
            attempts: 0,
            rng,
        }
    }

    pub fn is_negotiated(&self) -> bool {
        matches!(self.state, State::Negotiated { .. })
    }

    pub fn attempts(&self) -> usize {
        self.attempts
    }

    pub fn attempted(&mut self) {
        self.attempts += 1;
    }

    pub fn connected(&mut self, _link: Link) {
        self.attempts = 0;
    }

    pub fn ping(&mut self, reactor: &mut Reactor) -> Result<(), Error> {
        if let State::Negotiated { ping, .. } = &mut self.state {
            let msg = message::Ping::new(&mut self.rng);
            *ping = PingState::AwaitingResponse(msg.ponglen);

            reactor.write(self.id, Message::Ping(msg));
        }
        Ok(())
    }

    pub fn node_id(&self) -> Option<NodeId> {
        if let State::Negotiated { id, .. } = &self.state {
            return Some(*id);
        }
        None
    }
}
