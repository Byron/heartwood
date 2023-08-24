// Copyright © 2022 The Radicle Link Contributors

use crate::{change_graph::ChangeGraph, CollaborativeObject, ObjectId, Store, TypeName};

use super::error;

/// Get a [`CollaborativeObject`], if it exists.
///
/// The `storage` is the backing storage for storing
/// [`crate::Entry`]s at content-addressable locations. Please see
/// [`Store`] for further information.
///
/// The `typename` is the type of object to be found, while the
/// `object_id` is the identifier for the particular object under that
/// type.
pub fn get<S, I>(
    storage: &S,
    typename: &TypeName,
    oid: &ObjectId,
) -> Result<Option<CollaborativeObject>, error::Retrieve>
where
    S: Store<I>,
{
    let tip_refs = storage
        .objects(typename, oid)
        .map_err(|err| error::Retrieve::Refs { err: Box::new(err) })?;
    Ok(ChangeGraph::load(storage, tip_refs.iter(), typename, oid).map(|graph| graph.evaluate()))
}
