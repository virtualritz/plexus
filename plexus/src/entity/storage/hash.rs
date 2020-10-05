use fnv::FnvBuildHasher;
use std::collections::HashMap;
use std::hash::{BuildHasher, Hash};

use crate::entity::storage::journal::Unjournaled;
use crate::entity::storage::{
    AsStorage, AsStorageMut, Dispatch, ExtrinsicStorage, Get, InnerKey, InsertWithKey, Key, Remove,
    Sequence, StorageObject,
};
use crate::entity::Entity;

pub type FnvEntityMap<E> = HashMap<InnerKey<<E as Entity>::Key>, E, FnvBuildHasher>;

impl<E, K, H> AsStorage<E> for HashMap<InnerKey<K>, E, H>
where
    E: Entity<Key = K, Storage = Self>,
    K: Key,
    H: 'static + BuildHasher + Default,
    InnerKey<K>: 'static + Eq + Hash,
{
    fn as_storage(&self) -> &StorageObject<E> {
        self
    }
}

impl<E, K, H> AsStorageMut<E> for HashMap<InnerKey<K>, E, H>
where
    E: Entity<Key = K, Storage = Self>,
    K: Key,
    H: 'static + BuildHasher + Default,
    InnerKey<K>: 'static + Eq + Hash,
{
    fn as_storage_mut(&mut self) -> &mut StorageObject<E> {
        self
    }
}

#[cfg(not(nightly))]
impl<E, K, H> Dispatch<E> for HashMap<InnerKey<K>, E, H>
where
    E: Entity<Key = K, Storage = Self>,
    K: Key,
    H: 'static + BuildHasher + Default,
    InnerKey<K>: 'static + Eq + Hash,
{
    type Object = dyn 'static + ExtrinsicStorage<E>;
}

#[cfg(nightly)]
#[rustfmt::skip]
impl<E, K, H> Dispatch<E> for HashMap<InnerKey<K>, E, H>
where
    E: Entity<Key = K, Storage = Self>,
    K: Key,
    H: 'static + BuildHasher + Default,
    InnerKey<K>: 'static + Eq + Hash,
{
    type Object<'a> where E: 'a = dyn 'a + ExtrinsicStorage<E>;
}

impl<E, H> Get<E> for HashMap<InnerKey<E::Key>, E, H>
where
    E: Entity,
    H: BuildHasher + Default,
    InnerKey<E::Key>: Eq + Hash,
{
    fn get(&self, key: &E::Key) -> Option<&E> {
        self.get(&key.into_inner())
    }

    fn get_mut(&mut self, key: &E::Key) -> Option<&mut E> {
        self.get_mut(&key.into_inner())
    }
}

impl<E, H> InsertWithKey<E> for HashMap<InnerKey<E::Key>, E, H>
where
    E: Entity,
    H: BuildHasher + Default,
    InnerKey<E::Key>: Eq + Hash,
{
    fn insert_with_key(&mut self, key: &E::Key, entity: E) -> Option<E> {
        self.insert(key.into_inner(), entity)
    }
}

impl<E, H> Remove<E> for HashMap<InnerKey<E::Key>, E, H>
where
    E: Entity,
    H: BuildHasher + Default,
    InnerKey<E::Key>: Eq + Hash,
{
    fn remove(&mut self, key: &E::Key) -> Option<E> {
        self.remove(&key.into_inner())
    }
}

impl<E, H> Sequence<E> for HashMap<InnerKey<E::Key>, E, H>
where
    E: Entity,
    H: BuildHasher + Default,
    InnerKey<E::Key>: Eq + Hash,
{
    fn len(&self) -> usize {
        self.len()
    }

    fn iter<'a>(&'a self) -> Box<dyn 'a + ExactSizeIterator<Item = (E::Key, &E)>> {
        Box::new(
            self.iter()
                .map(|(key, entity)| (E::Key::from_inner(*key), entity)),
        )
    }

    fn iter_mut<'a>(&'a mut self) -> Box<dyn 'a + ExactSizeIterator<Item = (E::Key, &mut E)>> {
        Box::new(
            self.iter_mut()
                .map(|(key, entity)| (E::Key::from_inner(*key), entity)),
        )
    }

    fn keys<'a>(&'a self) -> Box<dyn 'a + ExactSizeIterator<Item = E::Key>> {
        Box::new(self.keys().map(|key| E::Key::from_inner(*key)))
    }
}

impl<K, T, H> Unjournaled for HashMap<K, T, H>
where
    K: Eq + Hash,
    H: BuildHasher + Default,
{
}
