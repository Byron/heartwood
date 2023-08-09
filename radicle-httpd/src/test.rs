use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use std::{env, fs};

use axum::body::{Body, Bytes};
use axum::http::{Method, Request};
use axum::Router;
use serde_json::Value;
use time::OffsetDateTime;
use tower::ServiceExt;

use radicle::cob::issue::Issues;
use radicle::cob::patch::{MergeTarget, Patches};
use radicle::crypto::ssh::keystore::MemorySigner;
use radicle::crypto::ssh::Keystore;
use radicle::crypto::{KeyPair, Seed, Signer};
use radicle::git::{raw as git2, RefString};
use radicle::node;
use radicle::node::address as AddressStore;
use radicle::node::routing as RoutingStore;
use radicle::node::tracking::store as TrackingStore;
use radicle::profile;
use radicle::profile::Home;
use radicle::storage::ReadStorage;
use radicle::Storage;
use radicle_crypto::test::signer::MockSigner;

use crate::api::{auth, Context};

pub const RID: &str = "rad:z4FucBZHZMCsxTyQE1dfE2YR59Qbp";
pub const HEAD: &str = "e8c676b9e3b42308dc9d218b70faa5408f8e58ca";
pub const PARENT: &str = "ee8d6a29304623a78ebfa5eeed5af674d0e58f83";
pub const INITIAL_COMMIT: &str = "f604ce9fd5b7cc77b7609beda45ea8760bee78f7";
pub const DID: &str = "did:key:z6MknSLrJoTcukLrE435hVNQT4JUhbvWLX4kUzqkEStBU8Vi";
pub const ISSUE_ID: &str = "0b0b8ca3b75e109971f87d92c1a6c930e87484c6";
pub const ISSUE_DISCUSSION_ID: &str = "7466975f0bef37b459887824a9655f3e78262522";
pub const ISSUE_COMMENT_ID: &str = "24ee306c508cd731a8427612dbdd826209096f99";
pub const SESSION_ID: &str = "u9MGAkkfkMOv0uDDB2WeUHBT7HbsO2Dy";
pub const TIMESTAMP: u64 = 1671125284;
pub const CONTRIBUTOR_RID: &str = "rad:z4XaCmN3jLSeiMvW15YTDpNbDHFhG";
pub const CONTRIBUTOR_DID: &str = "did:key:z6Mkk7oqY4pPxhMmGEotDYsFo97vhCj85BLY1H256HrJmjN8";
pub const CONTRIBUTOR_NID: &str = "z6Mkk7oqY4pPxhMmGEotDYsFo97vhCj85BLY1H256HrJmjN8";
pub const CONTRIBUTOR_ISSUE_ID: &str = "7466975f0bef37b459887824a9655f3e78262522";
pub const CONTRIBUTOR_PATCH_ID: &str = "e651ae5869a2c1ac8ad4f6deae4cc835656ffa25";
pub const CONTRIBUTOR_COMMENT_1: &str = "d0bb75b2c72ab8b5486d39f6cf5f41f104b63cb1";
pub const CONTRIBUTOR_COMMENT_2: &str = "2a4ec5bcb1be09c1f2213f418c0159fff894b989";

/// Create a new profile.
pub fn profile(home: &Path, seed: [u8; 32]) -> radicle::Profile {
    let home = Home::new(home).unwrap();
    let storage = Storage::open(home.storage()).unwrap();
    let keystore = Keystore::new(&home.keys());
    let keypair = KeyPair::from_seed(Seed::from(seed));

    radicle::storage::git::transport::local::register(storage.clone());
    keystore.store(keypair.clone(), "radicle", None).unwrap();

    radicle::Profile {
        home,
        storage,
        keystore,
        public_key: keypair.pk.into(),
        config: profile::Config {
            node: node::Config::new(node::Alias::new("seed")),
        },
    }
}

pub fn seed(dir: &Path) -> Context {
    let home = dir.join("radicle");
    let profile = profile(home.as_path(), [0xff; 32]);
    let signer = Box::new(MockSigner::from_seed([0xff; 32]));

    crate::logger::init().ok();

    seed_with_signer(dir, profile, &signer)
}

pub fn contributor(dir: &Path) -> Context {
    let mut seed = [0xff; 32];
    *seed.last_mut().unwrap() = 0xee;

    let home = dir.join("radicle");
    let profile = profile(home.as_path(), seed);
    let signer = MemorySigner::load(&profile.keystore, None).unwrap();

    seed_with_signer(dir, profile, &signer)
}

fn seed_with_signer<G: Signer>(dir: &Path, profile: radicle::Profile, signer: &G) -> Context {
    let tracking_db = dir.join("radicle").join("node").join("tracking.db");
    let routing_db = dir.join("radicle").join("node").join("routing.db");
    let addresses_db = dir.join("radicle").join("node").join("addresses.db");

    TrackingStore::Config::open(tracking_db).unwrap();
    RoutingStore::Table::open(routing_db).unwrap();
    AddressStore::Book::open(addresses_db).unwrap();

    let workdir = dir.join("hello-world");

    env::set_var("RAD_COMMIT_TIME", TIMESTAMP.to_string());

    fs::create_dir_all(&workdir).unwrap();

    // add commits to workdir (repo)
    let repo = git2::Repository::init(&workdir).unwrap();
    let tree =
        radicle::git::write_tree(Path::new("README"), "Hello World!\n".as_bytes(), &repo).unwrap();

    let sig_time = git2::Time::new(1673001014, 0);
    let sig = git2::Signature::new("Alice Liddell", "alice@radicle.xyz", &sig_time).unwrap();

    let oid = repo
        .commit(Some("HEAD"), &sig, &sig, "Initial commit\n", &tree, &[])
        .unwrap();
    let commit = repo.find_commit(oid).unwrap();

    repo.checkout_tree(tree.as_object(), None).unwrap();

    let tree = radicle::git::write_tree(
        Path::new("CONTRIBUTING"),
        "Thank you very much!\n".as_bytes(),
        &repo,
    )
    .unwrap();
    let sig_time = git2::Time::new(1673002014, 0);
    let sig = git2::Signature::new("Alice Liddell", "alice@radicle.xyz", &sig_time).unwrap();

    let oid2 = repo
        .commit(
            Some("HEAD"),
            &sig,
            &sig,
            "Add contributing file\n",
            &tree,
            &[&commit],
        )
        .unwrap();
    let commit2 = repo.find_commit(oid2).unwrap();

    repo.checkout_tree(tree.as_object(), None).unwrap();

    fs::create_dir(workdir.join("dir1")).unwrap();
    fs::write(
        workdir.join("dir1").join("README"),
        "Hello World from dir1!\n",
    )
    .unwrap();
    let mut index = repo.index().unwrap();
    index
        .add_all(["."], git2::IndexAddOption::DEFAULT, None)
        .unwrap();
    index.write().unwrap();

    let oid = index.write_tree().unwrap();
    let tree = repo.find_tree(oid).unwrap();

    let sig_time = git2::Time::new(1673003014, 0);
    let sig = git2::Signature::new("Alice Liddell", "alice@radicle.xyz", &sig_time).unwrap();
    repo.commit(
        Some("HEAD"),
        &sig,
        &sig,
        "Add another folder\n",
        &tree,
        &[&commit2],
    )
    .unwrap();

    // rad init
    let repo = git2::Repository::open(&workdir).unwrap();
    let name = "hello-world".to_string();
    let description = "Rad repository for tests".to_string();
    let branch = RefString::try_from("master").unwrap();
    let (id, _, _) =
        radicle::rad::init(&repo, &name, &description, branch, signer, &profile.storage).unwrap();

    let storage = &profile.storage;
    let repo = storage.repository(id).unwrap();
    let mut issues = Issues::open(&repo).unwrap();
    let _ = issues
        .create(
            "Issue #1".to_string(),
            "Change 'hello world' to 'hello everyone'".to_string(),
            &[],
            &[],
            signer,
        )
        .unwrap();

    // eq. rad patch open
    let mut patches = Patches::open(&repo).unwrap();
    let oid = radicle::git::Oid::from_str(HEAD).unwrap();
    let base = radicle::git::Oid::from_str(PARENT).unwrap();
    let _ = patches
        .create(
            "A new `hello world`",
            "change `hello world` in README to something else",
            MergeTarget::Delegates,
            base,
            oid,
            &[],
            signer,
        )
        .unwrap();

    let options = crate::Options {
        aliases: std::collections::HashMap::new(),
        listen: std::net::SocketAddr::from(([0, 0, 0, 0], 8080)),
        cache: Some(crate::DEFAULT_CACHE_SIZE),
    };

    Context::new(Arc::new(profile), &options)
}

/// Adds an authorized session to the Context::sessions HashMap.
pub async fn create_session(ctx: Context) {
    let issued_at = OffsetDateTime::now_utc();
    let mut sessions = ctx.sessions().write().await;
    sessions.insert(
        String::from(SESSION_ID),
        auth::Session {
            status: auth::AuthState::Authorized,
            public_key: ctx.profile().public_key,
            issued_at,
            expires_at: issued_at
                .checked_add(auth::AUTHORIZED_SESSIONS_EXPIRATION)
                .unwrap(),
        },
    );
}

pub async fn get(app: &Router, path: impl ToString) -> Response {
    Response(
        app.clone()
            .oneshot(request(path, Method::GET, None, None))
            .await
            .unwrap(),
    )
}

pub async fn post(
    app: &Router,
    path: impl ToString,
    body: Option<Body>,
    auth: Option<String>,
) -> Response {
    Response(
        app.clone()
            .oneshot(request(path, Method::POST, body, auth))
            .await
            .unwrap(),
    )
}

pub async fn patch(
    app: &Router,
    path: impl ToString,
    body: Option<Body>,
    auth: Option<String>,
) -> Response {
    Response(
        app.clone()
            .oneshot(request(path, Method::PATCH, body, auth))
            .await
            .unwrap(),
    )
}

pub async fn put(
    app: &Router,
    path: impl ToString,
    body: Option<Body>,
    auth: Option<String>,
) -> Response {
    Response(
        app.clone()
            .oneshot(request(path, Method::PUT, body, auth))
            .await
            .unwrap(),
    )
}

fn request(
    path: impl ToString,
    method: Method,
    body: Option<Body>,
    auth: Option<String>,
) -> Request<Body> {
    let mut request = Request::builder()
        .method(method)
        .uri(path.to_string())
        .header("Content-Type", "application/json");
    if let Some(token) = auth {
        request = request.header("Authorization", format!("Bearer {token}"));
    }

    request.body(body.unwrap_or_else(Body::empty)).unwrap()
}

#[derive(Debug)]
pub struct Response(axum::response::Response);

impl Response {
    pub async fn json(self) -> Value {
        let body = hyper::body::to_bytes(self.0.into_body()).await.unwrap();
        serde_json::from_slice(&body).unwrap()
    }

    pub fn status(&self) -> axum::http::StatusCode {
        self.0.status()
    }

    pub async fn body(self) -> Bytes {
        hyper::body::to_bytes(self.0.into_body()).await.unwrap()
    }
}
