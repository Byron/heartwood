mod id;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;
use std::fmt::Write as _;
use std::marker::PhantomData;
use std::ops::{Deref, Not};
use std::path::Path;
use std::str::FromStr;

use nonempty::NonEmpty;
use once_cell::sync::Lazy;
use radicle_git_ext::Oid;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::canonical::formatter::CanonicalFormatter;
use crate::crypto;
use crate::crypto::{Signature, Unverified, Verified};
use crate::git;
use crate::identity::{project::Project, Did};
use crate::storage;
use crate::storage::git::trailers;
use crate::storage::{ReadRepository, RemoteId};

pub use crypto::PublicKey;
pub use id::*;

/// Path to the identity document in the identity branch.
pub static PATH: Lazy<&Path> = Lazy::new(|| Path::new("radicle.json"));
/// Maximum length of a string in the identity document.
pub const MAX_STRING_LENGTH: usize = 255;
/// Maximum number of a delegates in the identity document.
pub const MAX_DELEGATES: usize = 255;

#[derive(Error, Debug)]
pub enum DocError {
    #[error("invalid commit: {0}")]
    Commit(&'static str),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid delegates: {0}")]
    Delegates(&'static str),
    #[error("invalid signature for {0}: {1}")]
    Signature(PublicKey, crypto::Error),
    #[error("invalid commit trailers: {0}")]
    Trailers(#[from] trailers::Error),
    #[error("invalid version `{0}`")]
    Version(u32),
    #[error("invalid threshold `{0}`: {1}")]
    Threshold(usize, &'static str),
    #[error("git: {0}")]
    GitExt(#[from] git::Error),
    #[error("git: {0}")]
    Git(#[from] git2::Error),
}

impl DocError {
    /// Whether this error is caused by the document not being found.
    pub fn is_not_found(&self) -> bool {
        match self {
            Self::GitExt(git::Error::NotFound(_)) => true,
            Self::GitExt(git::Error::Git(e)) if git::is_not_found_err(e) => true,
            Self::Git(err) if git::is_not_found_err(err) => true,
            _ => false,
        }
    }
}

/// Identifies an identity document payload type.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
// TODO: Restrict values.
pub struct PayloadId(String);

impl fmt::Display for PayloadId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl PayloadId {
    /// Project payload type.
    pub fn project() -> Self {
        Self(String::from("xyz.radicle.project"))
    }
}

#[derive(Debug, Error)]
pub enum PayloadError {
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("payload '{0}' not found in identity document")]
    NotFound(PayloadId),
}

/// Payload value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Payload {
    value: serde_json::Value,
}

impl From<serde_json::Value> for Payload {
    fn from(value: serde_json::Value) -> Self {
        Self { value }
    }
}

impl Deref for Payload {
    type Target = serde_json::Value;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

/// A verified identity document at a specific commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocAt {
    /// The commit at which this document exists.
    pub commit: Oid,
    /// The document blob at this commit.
    pub blob: Oid,
    /// The parsed document.
    pub doc: Doc<Verified>,
    /// The validated commit signatures.
    pub sigs: HashMap<PublicKey, Signature>,
}

impl Deref for DocAt {
    type Target = Doc<Verified>;

    fn deref(&self) -> &Self::Target {
        &self.doc
    }
}

/// Repository visibility.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum Visibility {
    /// Anyone and everyone.
    #[default]
    Public,
    /// Delegates plus the allowed DIDs.
    Private {
        #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
        allow: BTreeSet<Did>,
    },
}

#[derive(Error, Debug)]
#[error("'{0}' is not a valid visibility type")]
pub struct VisibilityParseError(String);

impl FromStr for Visibility {
    type Err = VisibilityParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "public" => Ok(Visibility::Public),
            "private" => Ok(Visibility::private([])),
            _ => Err(VisibilityParseError(s.to_owned())),
        }
    }
}

impl Visibility {
    /// Check whether the visibility is public.
    pub fn is_public(&self) -> bool {
        matches!(self, Self::Public)
    }

    /// Private visibility with list of allowed DIDs beyond the repository delegates.
    pub fn private(allow: impl IntoIterator<Item = Did>) -> Self {
        Self::Private {
            allow: BTreeSet::from_iter(allow),
        }
    }
}

/// An identity document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Doc<V> {
    /// The payload section.
    pub payload: BTreeMap<PayloadId, Payload>,
    /// The delegates section.
    pub delegates: NonEmpty<Did>,
    /// The signature threshold.
    pub threshold: usize,
    /// Repository visibility.
    #[serde(default, skip_serializing_if = "Visibility::is_public")]
    pub visibility: Visibility,

    #[serde(skip)]
    verified: PhantomData<V>,
}

impl<V> Doc<V> {
    /// Check whether this document and the associated repository is visible to the given peer.
    pub fn is_visible_to(&self, peer: &PublicKey) -> bool {
        match &self.visibility {
            Visibility::Public => true,
            Visibility::Private { allow } => {
                allow.contains(&Did::from(*peer)) || self.is_delegate(peer)
            }
        }
    }

    pub fn canonical_head(repo: &storage::git::Repository) -> Result<Oid, DocError> {
        repo.backend
            .refname_to_id(storage::git::CANONICAL_IDENTITY.as_str())
            .map(Oid::from)
            .map_err(DocError::from)
    }

    pub fn head<R: ReadRepository>(remote: &RemoteId, repo: &R) -> Result<Oid, DocError> {
        repo.reference_oid(remote, &git::refs::storage::IDENTITY_BRANCH)
            .map_err(DocError::from)
    }

    pub fn blob_at<R: ReadRepository>(commit: Oid, repo: &R) -> Result<git2::Blob, DocError> {
        repo.blob_at(commit, Path::new(&*PATH))
            .map_err(DocError::from)
    }

    pub fn is_delegate(&self, key: &crypto::PublicKey) -> bool {
        self.delegates.contains(&key.into())
    }
}

impl Doc<Verified> {
    pub fn encode(&self) -> Result<(git::Oid, Vec<u8>), DocError> {
        let mut buf = Vec::new();
        let mut serializer =
            serde_json::Serializer::with_formatter(&mut buf, CanonicalFormatter::new());

        self.serialize(&mut serializer)?;
        let oid = git2::Oid::hash_object(git2::ObjectType::Blob, &buf)?;

        Ok((oid.into(), buf))
    }

    /// Attempt to add a new delegate to the document. Returns `true` if it wasn't there before.
    pub fn delegate(&mut self, key: &crypto::PublicKey) -> bool {
        let delegate = Did::from(key);
        if self.delegates.iter().all(|id| id != &delegate) {
            self.delegates.push(delegate);
            return true;
        }
        false
    }

    pub fn rescind(&mut self, key: &crypto::PublicKey) -> Result<Option<Did>, DocError> {
        let delegate = Did::from(key);
        let (matches, delegates) = self.delegates.iter().partition(|d| **d == delegate);
        match NonEmpty::from_vec(delegates) {
            Some(delegates) => {
                self.delegates = delegates;
                if self.threshold > self.delegates.len() {
                    return Err(DocError::Threshold(
                        self.threshold,
                        "the thresholds exceeds the new delegate count after removal",
                    ));
                }
                Ok(matches.is_empty().not().then_some(delegate))
            }
            None => Err(DocError::Delegates("cannot remove the last delegate")),
        }
    }

    /// Get the project payload, if it exists and is valid, out of this document.
    pub fn project(&self) -> Result<Project, PayloadError> {
        let value = self
            .payload
            .get(&PayloadId::project())
            .ok_or_else(|| PayloadError::NotFound(PayloadId::project()))?;
        let proj: Project = serde_json::from_value((**value).clone())?;

        Ok(proj)
    }

    pub fn sign<G: crypto::Signer>(&self, signer: &G) -> Result<(git::Oid, Signature), DocError> {
        let (oid, _) = self.encode()?;
        let sig = signer.sign(oid.as_bytes());

        Ok((oid, sig))
    }

    pub fn canonical(repo: &storage::git::Repository) -> Result<DocAt, DocError> {
        let oid = Self::canonical_head(repo)?;
        Self::load_at(oid, repo)
    }

    pub fn load_at<R: ReadRepository>(oid: Oid, repo: &R) -> Result<DocAt, DocError> {
        let blob = Self::blob_at(oid, repo)?;
        let doc = Doc::from_json(blob.content())?.verified()?;
        let commit = repo.commit(oid)?;
        let msg = commit
            .message_raw()
            .ok_or(DocError::Commit("commit message is not UTF-8"))?;
        let sigs = trailers::parse_signatures(msg)?;

        for (pk, sig) in &sigs {
            if let Err(err) = pk.verify(blob.id().as_bytes(), sig) {
                return Err(DocError::Signature(*pk, err));
            }
        }
        Ok(DocAt {
            commit: oid,
            doc,
            blob: blob.id().into(),
            sigs,
        })
    }

    pub fn init(
        doc: &[u8],
        remote: &RemoteId,
        signatures: &[(&PublicKey, Signature)],
        repo: &git2::Repository,
    ) -> Result<git::Oid, DocError> {
        let tree = git::write_tree(*PATH, doc, repo)?;
        let oid = Doc::commit(remote, &tree, "Initialize Radicle\n", &[], signatures, repo)?;

        Ok(oid)
    }

    pub fn update(
        &self,
        remote: &RemoteId,
        msg: &str,
        signatures: &[(&PublicKey, Signature)],
        repo: &git2::Repository,
    ) -> Result<git::Oid, DocError> {
        let (_, doc) = self.encode()?;
        let tree = git::write_tree(*PATH, doc.as_slice(), repo)?;
        let id_ref = git::refs::storage::id(remote);
        let head = repo.find_reference(&id_ref)?.peel_to_commit()?;
        let oid = Doc::commit(remote, &tree, msg, &[&head], signatures, repo)?;

        Ok(oid)
    }

    fn commit(
        remote: &RemoteId,
        tree: &git2::Tree,
        msg: &str,
        parents: &[&git2::Commit],
        signatures: &[(&PublicKey, Signature)],
        repo: &git2::Repository,
    ) -> Result<git::Oid, DocError> {
        let sig = repo
            .signature()
            .or_else(|_| git2::Signature::now("radicle", remote.to_string().as_str()))?;

        #[cfg(debug_assertions)]
        let sig = if let Ok(s) = std::env::var("RAD_COMMIT_TIME") {
            // SAFETY: Only used in test code.
            #[allow(clippy::unwrap_used)]
            let timestamp = s.trim().parse::<i64>().unwrap();
            let time = git2::Time::new(timestamp, 0);
            git2::Signature::new("radicle", remote.to_string().as_str(), &time)?
        } else {
            sig
        };

        let mut msg = format!("{}\n\n", msg.trim());
        for (key, sig) in signatures {
            writeln!(&mut msg, "{}: {key} {sig}", trailers::SIGNATURE_TRAILER)
                .expect("in-memory writes don't fail");
        }

        let id_ref = git::refs::storage::id(remote);
        let oid = repo.commit(Some(&id_ref), &sig, &sig, &msg, tree, parents)?;

        Ok(oid.into())
    }

    #[cfg(any(test, feature = "test"))]
    pub(crate) fn unverified(self) -> Doc<Unverified> {
        Doc {
            payload: self.payload,
            delegates: self.delegates,
            threshold: self.threshold,
            visibility: self.visibility,
            verified: PhantomData,
        }
    }
}

impl Doc<Unverified> {
    pub fn initial(project: Project, delegate: Did, visibility: Visibility) -> Self {
        Self::new(project, NonEmpty::new(delegate), 1, visibility)
    }

    pub fn new(
        project: Project,
        delegates: NonEmpty<Did>,
        threshold: usize,
        visibility: Visibility,
    ) -> Self {
        let project =
            serde_json::to_value(project).expect("Doc::initial: payload must be serializable");

        Self {
            payload: BTreeMap::from_iter([(PayloadId::project(), Payload::from(project))]),
            delegates,
            threshold,
            visibility,
            verified: PhantomData,
        }
    }

    pub fn from_json(bytes: &[u8]) -> Result<Self, DocError> {
        serde_json::from_slice(bytes).map_err(DocError::from)
    }

    pub fn verified(self) -> Result<Doc<Verified>, DocError> {
        if self.delegates.len() > MAX_DELEGATES {
            return Err(DocError::Delegates("number of delegates cannot exceed 255"));
        }
        if self.delegates.is_empty() {
            return Err(DocError::Delegates("delegate list cannot be empty"));
        }
        if self.threshold > self.delegates.len() {
            return Err(DocError::Threshold(
                self.threshold,
                "threshold cannot exceed number of delegates",
            ));
        }
        if self.threshold == 0 {
            return Err(DocError::Threshold(
                self.threshold,
                "threshold cannot be zero",
            ));
        }

        Ok(Doc {
            payload: self.payload,
            delegates: self.delegates,
            threshold: self.threshold,
            visibility: self.visibility,
            verified: PhantomData,
        })
    }

    pub fn load_at<R: ReadRepository>(commit: Oid, repo: &R) -> Result<(Self, Oid), DocError> {
        let blob = Self::blob_at(commit, repo)?;
        let doc = Doc::from_json(blob.content())?;

        Ok((doc, blob.id().into()))
    }

    pub fn load<R: ReadRepository>(remote: &RemoteId, repo: &R) -> Result<(Self, Oid), DocError> {
        let oid = Self::head(remote, repo)?;

        Self::load_at(oid, repo)
    }
}

#[cfg(test)]
mod test {
    use radicle_crypto::test::signer::MockSigner;
    use radicle_crypto::Signer as _;

    use crate::rad;
    use crate::storage::git::transport;
    use crate::storage::git::Storage;
    use crate::storage::{ReadStorage as _, WriteStorage as _};
    use crate::test::arbitrary;
    use crate::test::fixtures;

    use super::*;
    use qcheck_macros::quickcheck;

    #[test]
    fn test_canonical_example() {
        let tempdir = tempfile::tempdir().unwrap();
        let storage = Storage::open(tempdir.path().join("storage")).unwrap();

        transport::local::register(storage.clone());

        let delegate = MockSigner::from_seed([0xff; 32]);
        let (repo, _) = fixtures::repository(tempdir.path().join("working"));
        let (id, _, _) = rad::init(
            &repo,
            "heartwood",
            "Radicle Heartwood Protocol & Stack",
            git::refname!("master"),
            Visibility::default(),
            &delegate,
            &storage,
        )
        .unwrap();

        assert_eq!(
            delegate.public_key().to_human(),
            String::from("z6MknSLrJoTcukLrE435hVNQT4JUhbvWLX4kUzqkEStBU8Vi")
        );
        assert_eq!(
            (*id).to_string(),
            "d96f425412c9f8ad5d9a9a05c9831d0728e2338d"
        );
        assert_eq!(id.urn(), String::from("rad:z42hL2jL4XNk6K8oHQaSWfMgCL7ji"));
    }

    #[test]
    fn test_not_found() {
        let tempdir = tempfile::tempdir().unwrap();
        let storage = Storage::open(tempdir.path().join("storage")).unwrap();
        let remote = arbitrary::gen::<RemoteId>(1);
        let proj = arbitrary::gen::<Id>(1);
        let repo = storage.create(proj).unwrap();
        let oid = git2::Oid::from_str("2d52a53ce5e4f141148a5f770cfd3ead2d6a45b8").unwrap();

        let err = Doc::<Unverified>::head(&remote, &repo).unwrap_err();
        assert!(err.is_not_found());

        let err = Doc::<Unverified>::load_at(oid.into(), &repo).unwrap_err();
        assert!(err.is_not_found());
    }

    #[test]
    fn test_canonical_doc() {
        let tempdir = tempfile::tempdir().unwrap();
        let storage = Storage::open(tempdir.path().join("storage")).unwrap();
        transport::local::register(storage.clone());

        let (working, _) = fixtures::repository(tempdir.path().join("working"));

        let delegate = MockSigner::from_seed([0xff; 32]);
        let (rid, doc, _) = rad::init(
            &working,
            "heartwood",
            "Radicle Heartwood Protocol & Stack",
            git::refname!("master"),
            Visibility::default(),
            &delegate,
            &storage,
        )
        .unwrap();
        let repo = storage.repository(rid).unwrap();

        assert_eq!(doc, Doc::canonical(&repo).unwrap().doc);
    }

    #[quickcheck]
    fn prop_encode_decode(doc: Doc<Verified>) {
        let (_, bytes) = doc.encode().unwrap();
        assert_eq!(Doc::from_json(&bytes).unwrap().verified().unwrap(), doc);
    }

    #[test]
    fn test_visibility_json() {
        use std::str::FromStr;

        assert_eq!(
            serde_json::to_value(Visibility::Public).unwrap(),
            serde_json::json!({ "type": "public" })
        );
        assert_eq!(
            serde_json::to_value(Visibility::private([])).unwrap(),
            serde_json::json!({ "type": "private" })
        );
        assert_eq!(
            serde_json::to_value(Visibility::private([Did::from_str(
                "did:key:z6MksFqXN3Yhqk8pTJdUGLwATkRfQvwZXPqR2qMEhbS9wzpT"
            )
            .unwrap()]))
            .unwrap(),
            serde_json::json!({ "type": "private", "allow": ["did:key:z6MksFqXN3Yhqk8pTJdUGLwATkRfQvwZXPqR2qMEhbS9wzpT"] })
        );
    }
}
