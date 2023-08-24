// Copyright © 2021 The Radicle Link Contributors

use git_ext::Oid;

pub mod store;
pub use store::{Contents, EntryId, Storage, Template, Timestamp};

use crate::signatures::ExtendedSignature;

/// A single change in the change graph. The layout of changes in the repository
/// is specified in the RFC (docs/rfc/0662-collaborative-objects.adoc)
/// under "Change Commits".
pub type Entry = store::Entry<Oid, Oid, ExtendedSignature>;
