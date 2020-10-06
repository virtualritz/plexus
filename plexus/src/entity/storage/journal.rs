use std::hash::Hash;

use crate::entity::storage::hash::FnvEntityMap;
use crate::entity::storage::slot::{SlotEntityMap, SlotKey};
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
    Write(E::Key, E),
}

#[derive(Default)]
struct Log<E>
where
    E: Entity,
{
    mutations: Vec<Mutation<E>>,
}

// TODO: The type parameter `T` is only used to implement `AsStorage`. Is there
//       a way to write a generic implementation that also allows for an
//       implicit conversion from `&Journaled<_, _>` to a storage object?
#[derive(Default)]
pub struct Journaled<T, E>
where
    T: Default + Dispatch<E> + Storage<E> + Unjournaled,
    E: Entity<Storage = T>,
{
    storage: T,
    log: Log<E>,
}

// TODO: Is a general implementation possible? See `Journaled`.
impl<E, K> AsStorage<E> for Journaled<FnvEntityMap<E>, E>
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

// TODO: Is a general implementation possible? See `Journaled`.
impl<E, K> AsStorage<E> for Journaled<SlotEntityMap<E>, E>
where
    E: Entity<Key = K, Storage = SlotEntityMap<E>>,
    K: Key,
    K::Inner: 'static + SlotKey,
{
    fn as_storage(&self) -> &StorageObject<E> {
        // It is essential that this returns `self` and does NOT simply forward
        // to the `storage` field.
        self
    }
}

impl<E, K> AsStorageMut<E> for Journaled<FnvEntityMap<E>, E>
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

impl<E, K> AsStorageMut<E> for Journaled<SlotEntityMap<E>, E>
where
    E: Entity<Key = K, Storage = SlotEntityMap<E>>,
    K: Key,
    K::Inner: 'static + SlotKey,
{
    fn as_storage_mut(&mut self) -> &mut StorageObject<E> {
        // It is essential that this returns `self` and does NOT simply forward
        // to the `storage` field.
        self
    }
}

#[cfg(not(all(nightly, feature = "unstable")))]
impl<T, E> Dispatch<E> for Journaled<T, E>
where
    T: Default + Dispatch<E> + Storage<E> + Unjournaled,
    E: Entity<Storage = T>,
{
    type Object = StorageObject<E>;
}

#[cfg(all(nightly, feature = "unstable"))]
#[rustfmt::skip]
impl<T, E> Dispatch<E> for Journaled<T, E>
where
    T: Default + Dispatch<E> + Storage<E> + Unjournaled,
    E: Entity<Storage = T>,
{
    type Object<'a> where E: 'a = StorageObject<'a, E>;
}

impl<T, E> Get<E> for Journaled<T, E>
where
    T: Default + Dispatch<E> + Storage<E> + Unjournaled,
    E: Entity<Storage = T>,
{
    fn get(&self, key: &E::Key) -> Option<&E> {
        self.storage.get(key)
    }

    fn get_mut(&mut self, key: &E::Key) -> Option<&mut E> {
        self.storage.get_mut(key)
    }
}

impl<T, E> Insert<E> for Journaled<T, E>
where
    T: Default + Dispatch<E> + Insert<E> + Storage<E> + Unjournaled,
    E: Entity<Storage = T>,
{
    fn insert(&mut self, entity: E) -> E::Key {
        self.storage.insert(entity)
    }
}

impl<T, E> InsertWithKey<E> for Journaled<T, E>
where
    T: Default + Dispatch<E> + InsertWithKey<E> + Storage<E> + Unjournaled,
    E: Entity<Storage = T>,
{
    fn insert_with_key(&mut self, key: &E::Key, entity: E) -> Option<E> {
        self.storage.insert_with_key(key, entity)
    }
}

impl<T, E> Remove<E> for Journaled<T, E>
where
    T: Default + Dispatch<E> + Storage<E> + Unjournaled,
    E: Entity<Storage = T>,
{
    fn remove(&mut self, key: &E::Key) -> Option<E> {
        self.storage.remove(key)
    }
}

impl<T, E> Sequence<E> for Journaled<T, E>
where
    T: Default + Dispatch<E> + Storage<E> + Unjournaled,
    E: Entity<Storage = T>,
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
