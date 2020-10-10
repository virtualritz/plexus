use fool::BoolExt as _;
use ordered_multimap::ListOrderedMultimap as LinkedMultiMap;
use std::hash::Hash;

use crate::entity::storage::hash::FnvEntityMap;
use crate::entity::storage::slot::{SlotEntityMap, SlotKey};
use crate::entity::storage::{
    AsStorage, AsStorageMut, Dispatch, Enumerate, Get, Insert, InsertWithKey, Key, Remove, Storage,
    StorageObject,
};
use crate::entity::Entity;

// TODO: Implement `Journaled` such that it does not mutate its source storage
//       until the log is committed.

pub trait Unjournaled {}

pub trait JournalState {
    type State;

    fn state(&self) -> Self::State;
}

pub trait SyntheticKey<T> {
    #[must_use]
    fn synthesize(state: &mut T) -> Self;
}

enum Mutation<E>
where
    E: Entity,
{
    Insert(E),
    Remove,
    Write(E),
}

// TODO: The type parameter `T` is only used to implement `AsStorage`. Is there
//       a way to write a generic implementation that also allows for an
//       implicit conversion from `&Journaled<_, _>` to a storage object?
pub struct Journaled<T, E>
where
    T: Default + Dispatch<E> + JournalState + Storage<E> + Unjournaled,
    E: Entity<Storage = T>,
{
    storage: T,
    log: LinkedMultiMap<E::Key, Mutation<E>>,
    state: T::State,
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

// TODO: Is a general implementation possible? See `Journaled`.
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

// TODO: Is a general implementation possible? See `Journaled`.
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
    T: Default + Dispatch<E> + JournalState + Storage<E> + Unjournaled,
    E: Entity<Storage = T>,
{
    type Object = StorageObject<E>;
}

#[cfg(all(nightly, feature = "unstable"))]
#[rustfmt::skip]
impl<T, E> Dispatch<E> for Journaled<T, E>
where
    T: Default + Dispatch<E> + JournalState + Storage<E> + Unjournaled,
    E: Entity<Storage = T>,
{
    type Object<'a> where E: 'a = StorageObject<'a, E>;
}

impl<T, E> Enumerate<E> for Journaled<T, E>
where
    T: Default + Dispatch<E> + JournalState + Storage<E> + Unjournaled,
    E: Entity<Storage = T>,
{
    fn len(&self) -> usize {
        todo!()
    }

    fn iter<'a>(&'a self) -> Box<dyn 'a + ExactSizeIterator<Item = (E::Key, &E)>> {
        todo!()
    }

    fn iter_mut<'a>(&'a mut self) -> Box<dyn 'a + ExactSizeIterator<Item = (E::Key, &mut E)>> {
        todo!()
    }

    fn keys<'a>(&'a self) -> Box<dyn 'a + ExactSizeIterator<Item = E::Key>> {
        todo!()
    }
}

impl<T, E> Get<E> for Journaled<T, E>
where
    T: Default + Dispatch<E> + JournalState + Storage<E> + Unjournaled,
    E: Entity<Storage = T>,
{
    fn get(&self, key: &E::Key) -> Option<&E> {
        if let Some(mutation) = self.log.get_all(key).next_back() {
            match mutation {
                Mutation::Insert(ref entity) | Mutation::Write(ref entity) => Some(entity),
                Mutation::Remove => None,
            }
        }
        else {
            self.storage.get(key)
        }
    }

    fn get_mut(&mut self, key: &E::Key) -> Option<&mut E> {
        // TODO: Should mutations be aggregated in the log?
        let entity = self.get(key).cloned();
        if let Some(entity) = entity {
            self.log.append(*key, Mutation::Write(entity));
            if let Mutation::Write(ref mut entity) = self.log.back_mut().unwrap().1 {
                Some(entity)
            }
            else {
                unreachable!()
            }
        }
        else {
            None
        }
    }
}

impl<T, E> Insert<E> for Journaled<T, E>
where
    T: Default + Dispatch<E> + Insert<E> + JournalState + Storage<E> + Unjournaled,
    E: Entity<Storage = T>,
    E::Key: SyntheticKey<T::State>,
{
    fn insert(&mut self, entity: E) -> E::Key {
        let key = SyntheticKey::synthesize(&mut self.state);
        self.log.append(key, Mutation::Insert(entity));
        key
    }
}

impl<T, E> InsertWithKey<E> for Journaled<T, E>
where
    T: Default + Dispatch<E> + InsertWithKey<E> + JournalState + Storage<E> + Unjournaled,
    E: Entity<Storage = T>,
{
    fn insert_with_key(&mut self, key: &E::Key, entity: E) -> Option<E> {
        let occupant = self.get(key).cloned();
        self.log
            .append(*key, Mutation::Insert(entity))
            .and_then(|| occupant)
    }
}

impl<T, E> Remove<E> for Journaled<T, E>
where
    T: Default + Dispatch<E> + JournalState + Storage<E> + Unjournaled,
    E: Entity<Storage = T>,
{
    fn remove(&mut self, key: &E::Key) -> Option<E> {
        let occupant = self.get(key).cloned();
        self.log.append(*key, Mutation::Remove);
        occupant
    }
}
