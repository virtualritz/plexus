use fnv::FnvBuildHasher;
use fool::BoolExt as _;
use ordered_multimap::ListOrderedMultimap as LinkedMultiMap;
use std::collections::{HashMap, HashSet};
use std::hash::Hash;

use crate::entity::storage::hash::FnvEntityMap;
use crate::entity::storage::slot::{SlotEntityMap, SlotKey};
use crate::entity::storage::{
    AsStorage, AsStorageMut, DependantKey, Dispatch, EntityError, Enumerate, Get, Insert,
    InsertWithKey, Key, Remove, Storage, StorageObject,
};
use crate::entity::{Entity, Payload};

// TODO: Should mutations be aggregated in the log? For a given key, the
//       complete history may not be necessary.

pub type Rekeying<K> = HashMap<K, K, FnvBuildHasher>;

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

impl<E> Mutation<E>
where
    E: Entity,
{
    pub fn as_entity(&self) -> Option<&E> {
        match *self {
            Mutation::Insert(ref entity) | Mutation::Write(ref entity) => Some(entity),
            Mutation::Remove => None,
        }
    }

    pub fn as_entity_mut(&mut self) -> Option<&mut E> {
        match *self {
            Mutation::Insert(ref mut entity) | Mutation::Write(ref mut entity) => Some(entity),
            Mutation::Remove => None,
        }
    }
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

impl<T, E> Journaled<T, E>
where
    T: Default + Dispatch<E> + JournalState + Storage<E> + Unjournaled,
    E: Entity<Storage = T>,
{
    pub fn abort(self) -> T {
        self.storage
    }
}

impl<T, E> Journaled<T, E>
where
    T: Default + Dispatch<E> + Insert<E> + JournalState + Storage<E> + Unjournaled,
    E: Entity<Storage = T>,
    E::Key: SyntheticKey<T::State>,
{
    pub fn commit_and_rekey(self) -> (T, Rekeying<E::Key>) {
        let Journaled {
            mut storage,
            mut log,
            ..
        } = self;
        let mut rekeying = Rekeying::<_>::default();
        for (key, mutation) in log
            .drain_pairs()
            .flat_map(|(key, mut entry)| entry.next_back().map(|mutation| (key, mutation)))
        {
            // TODO: Should unmapped keys be inserted into the rekeying? Note
            //       that removing such keys may complicate rekeying of
            //       dependent keys.
            let rekey = match mutation {
                Mutation::Insert(entity) | Mutation::Write(entity) => {
                    if let Some(occupant) = storage.get_mut(&key) {
                        *occupant = entity;
                        key
                    }
                    else {
                        storage.insert(entity)
                    }
                }
                Mutation::Remove => {
                    // This key may only exist in the log i.e., if an entity is
                    // inserted and then removed while journaled. In that case,
                    // this is a no-op.
                    storage.remove(&key);
                    key
                }
            };
            rekeying.insert(key, rekey);
        }
        (storage, rekeying)
    }
}

impl<T, E> Journaled<T, E>
where
    T: Default + Dispatch<E> + InsertWithKey<E> + JournalState + Storage<E> + Unjournaled,
    E: Entity<Storage = T>,
    E::Key: DependantKey,
{
    pub fn commit_with_rekeying(
        self,
        rekeying: &Rekeying<<E::Key as DependantKey>::Foreign>,
    ) -> Result<T, EntityError> {
        todo!()
    }
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
        let n = self.storage.len();
        // Count inserted entities in the log.
        let p = self
            .log
            .pairs()
            .filter_map(|(_, entry)| {
                entry
                    .into_iter()
                    .rev()
                    .find(|mutation| !matches!(mutation, Mutation::Write(_)))
                    .filter(|mutation| matches!(mutation, Mutation::Insert(_)))
            })
            .count();
        // Count removed entities in the log.
        let q = self
            .log
            .pairs()
            .filter_map(|(_, entry)| {
                entry
                    .into_iter()
                    .rev()
                    .find(|mutation| !matches!(mutation, Mutation::Write(_)))
                    .filter(|mutation| matches!(mutation, Mutation::Remove))
            })
            .count();
        n + p - q
    }

    fn iter<'a>(&'a self) -> Box<dyn 'a + Iterator<Item = (E::Key, &E)>> {
        let keys: HashSet<_, FnvBuildHasher> = self.log.keys().collect();
        Box::new(
            self.storage
                .iter()
                .filter(move |(key, _)| !keys.contains(key))
                .chain(self.log.pairs().flat_map(move |(key, mut entry)| {
                    entry
                        .next_back()
                        .and_then(|mutation| mutation.as_entity().map(|entity| (*key, entity)))
                })),
        )
    }

    // This does not require logging, because only keys and user data are
    // exposed. Items yielded by this iterator are not recorded as writes.
    fn iter_mut<'a>(&'a mut self) -> Box<dyn 'a + Iterator<Item = (E::Key, &mut E::Data)>>
    where
        E: Payload,
    {
        let keys: HashSet<_, FnvBuildHasher> = self.log.keys().cloned().collect();
        Box::new(
            self.storage
                .iter_mut()
                .filter(move |(key, _)| !keys.contains(key))
                .chain(self.log.pairs_mut().flat_map(move |(key, mut entry)| {
                    entry.next_back().and_then(|mutation| {
                        mutation
                            .as_entity_mut()
                            .map(|entity| (*key, entity.get_mut()))
                    })
                })),
        )
    }

    fn keys<'a>(&'a self) -> Box<dyn 'a + Iterator<Item = E::Key>> {
        // TODO: This boxes the iterator twice.
        Box::new(self.iter().map(|(key, _)| key))
    }
}

impl<T, E> Get<E> for Journaled<T, E>
where
    T: Default + Dispatch<E> + JournalState + Storage<E> + Unjournaled,
    E: Entity<Storage = T>,
{
    fn get(&self, key: &E::Key) -> Option<&E> {
        if let Some(mutation) = self.log.get_all(key).next_back() {
            mutation.as_entity()
        }
        else {
            self.storage.get(key)
        }
    }

    fn get_mut(&mut self, key: &E::Key) -> Option<&mut E> {
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
