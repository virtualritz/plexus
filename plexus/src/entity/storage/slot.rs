use slotmap::hop::HopSlotMap;

use crate::entity::storage::journal::Unjournaled;
use crate::entity::storage::{
    AsStorage, AsStorageMut, Dispatch, Get, InnerKey, Insert, IntrinsicStorage, Key, Remove,
    Sequence, StorageObject,
};
use crate::entity::Entity;

pub use slotmap::Key as SlotKey;

pub type SlotEntityMap<E> = HopSlotMap<InnerKey<<E as Entity>::Key>, E>;

impl<E, K> AsStorage<E> for HopSlotMap<InnerKey<K>, E>
where
    E: Entity<Key = K, Storage = Self>,
    K: Key,
    InnerKey<K>: 'static + SlotKey,
{
    fn as_storage(&self) -> &StorageObject<E> {
        self
    }
}

impl<E, K> AsStorageMut<E> for HopSlotMap<InnerKey<K>, E>
where
    E: Entity<Key = K, Storage = Self>,
    K: Key,
    InnerKey<K>: 'static + SlotKey,
{
    fn as_storage_mut(&mut self) -> &mut StorageObject<E> {
        self
    }
}

#[cfg(not(all(nightly, feature = "unstable")))]
impl<E, K> Dispatch<E> for HopSlotMap<InnerKey<K>, E>
where
    E: Entity<Key = K, Storage = Self>,
    K: Key,
    InnerKey<K>: 'static + SlotKey,
{
    type Object = dyn 'static + IntrinsicStorage<E>;
}

#[cfg(all(nightly, feature = "unstable"))]
#[rustfmt::skip]
impl<E, K> Dispatch<E> for HopSlotMap<InnerKey<K>, E>
where
    E: Entity<Key = K, Storage = Self>,
    K: Key,
    InnerKey<K>: 'static + SlotKey,
{
    type Object<'a> where E: 'a = dyn 'a + IntrinsicStorage<E>;
}

impl<E> Get<E> for HopSlotMap<InnerKey<E::Key>, E>
where
    E: Entity,
    InnerKey<E::Key>: SlotKey,
{
    fn get(&self, key: &E::Key) -> Option<&E> {
        self.get(key.into_inner())
    }

    fn get_mut(&mut self, key: &E::Key) -> Option<&mut E> {
        self.get_mut(key.into_inner())
    }
}

impl<E> Insert<E> for HopSlotMap<InnerKey<E::Key>, E>
where
    E: Entity,
    InnerKey<E::Key>: SlotKey,
{
    fn insert(&mut self, entity: E) -> E::Key {
        E::Key::from_inner(self.insert(entity))
    }
}

impl<E> Remove<E> for HopSlotMap<InnerKey<E::Key>, E>
where
    E: Entity,
    InnerKey<E::Key>: SlotKey,
{
    fn remove(&mut self, key: &E::Key) -> Option<E> {
        self.remove(key.into_inner())
    }
}

impl<E> Sequence<E> for HopSlotMap<InnerKey<E::Key>, E>
where
    E: Entity,
    InnerKey<E::Key>: SlotKey,
{
    fn len(&self) -> usize {
        self.len()
    }

    fn iter<'a>(&'a self) -> Box<dyn 'a + ExactSizeIterator<Item = (E::Key, &E)>> {
        Box::new(
            self.iter()
                .map(|(key, entity)| (E::Key::from_inner(key), entity)),
        )
    }

    fn iter_mut<'a>(&'a mut self) -> Box<dyn 'a + ExactSizeIterator<Item = (E::Key, &mut E)>> {
        Box::new(
            self.iter_mut()
                .map(|(key, entity)| (E::Key::from_inner(key), entity)),
        )
    }

    fn keys<'a>(&'a self) -> Box<dyn 'a + ExactSizeIterator<Item = E::Key>> {
        Box::new(self.keys().map(E::Key::from_inner))
    }
}

impl<K, T> Unjournaled for HopSlotMap<K, T>
where
    K: SlotKey,
    T: Copy,
{
}
