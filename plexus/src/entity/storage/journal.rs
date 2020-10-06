use std::hash::Hash;

use crate::entity::storage::hash::FnvEntityMap;
use crate::entity::storage::{
    AsStorage, AsStorageMut, Dispatch, Get, Insert, InsertWithKey, Key, Remove, Sequence, Storage,
    StorageObject,
};
use crate::entity::Entity;

pub trait Unjournaled {}

enum Mutation<E>
where
    E: Entity,
{
    Insert(E::Key, E),
    Remove(E::Key),
    // NOTE: This will probably be the most abundant mutation in the log.
    Write(E::Key, E),
}

#[derive(Default)]
struct Log<E>
where
    E: Entity,
{
    mutations: Vec<Mutation<E>>,
}

#[derive(Default)]
pub struct Journaled<E>
where
    E: Entity,
{
    storage: E::Storage,
    log: Log<E>,
}

// TODO: Is it possible to parameterize on entity storage instead of using
//       bespoke implementations for each storage type?
impl<E, K> AsStorage<E> for Journaled<E>
where
    E: Entity<Key = K, Storage = FnvEntityMap<E>>,
    K: Key,
    K::Inner: 'static + Eq + Hash,
{
    fn as_storage(&self) -> &StorageObject<E> {
        // It is essential that this returns `self` and does NOT simply forward
        // to the `storage` field.
        self
    }
}

impl<E, K> AsStorageMut<E> for Journaled<E>
where
    E: Entity<Key = K, Storage = FnvEntityMap<E>>,
    K: Key,
    K::Inner: 'static + Eq + Hash,
{
    fn as_storage_mut(&mut self) -> &mut StorageObject<E> {
        // It is essential that this returns `self` and does NOT simply forward
        // to the `storage` field.
        self
    }
}

#[cfg(not(all(nightly, feature = "unstable")))]
impl<E> Dispatch<E> for Journaled<E>
where
    E: Entity,
{
    type Object = StorageObject<E>;
}

#[cfg(all(nightly, feature = "unstable"))]
#[rustfmt::skip]
impl<E> Dispatch<E> for Journaled<E>
where
    E: Entity,
{
    type Object<'a> where E: 'a = StorageObject<'a, E>;
}

impl<E> Get<E> for Journaled<E>
where
    E: Entity,
{
    fn get(&self, key: &E::Key) -> Option<&E> {
        self.storage.get(key)
    }

    fn get_mut(&mut self, key: &E::Key) -> Option<&mut E> {
        self.storage.get_mut(key)
    }
}

impl<E> Insert<E> for Journaled<E>
where
    E: Entity,
    E::Storage: Insert<E>,
{
    fn insert(&mut self, entity: E) -> E::Key {
        self.storage.insert(entity)
    }
}

impl<E> InsertWithKey<E> for Journaled<E>
where
    E: Entity,
    E::Storage: InsertWithKey<E>,
{
    fn insert_with_key(&mut self, key: &E::Key, entity: E) -> Option<E> {
        self.storage.insert_with_key(key, entity)
    }
}

impl<E> Remove<E> for Journaled<E>
where
    E: Entity,
{
    fn remove(&mut self, key: &E::Key) -> Option<E> {
        self.storage.remove(key)
    }
}

impl<E> Sequence<E> for Journaled<E>
where
    E: Entity,
{
    fn len(&self) -> usize {
        self.storage.len()
    }

    fn iter<'a>(&'a self) -> Box<dyn 'a + ExactSizeIterator<Item = (E::Key, &E)>> {
        self.storage.iter()
    }

    fn iter_mut<'a>(&'a mut self) -> Box<dyn 'a + ExactSizeIterator<Item = (E::Key, &mut E)>> {
        self.storage.iter_mut()
    }

    fn keys<'a>(&'a self) -> Box<dyn 'a + ExactSizeIterator<Item = E::Key>> {
        self.storage.keys()
    }
}
