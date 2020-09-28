use slotmap::hop::HopSlotMap;
use slotmap::Key as SlotKey;

use crate::entity::storage::journal::Unjournaled;
use crate::entity::storage::{
    AsStorage, AsStorageMut, Dispatch, Get, InnerKey, Insert, IntrinsicStorage, Key, Remove,
    Sequence, StorageObject,
};
use crate::entity::Entity;

pub type SlotEntityMap<E> = HopSlotMap<InnerKey<<E as Entity>::Key>, E>;

impl<E> AsStorage<E> for HopSlotMap<InnerKey<E::Key>, E>
where
    E: 'static + Entity,
    E::Storage: Dispatch<E, Object = dyn IntrinsicStorage<E>>,
    InnerKey<E::Key>: SlotKey,
{
    fn as_storage(&self) -> &StorageObject<E> {
        self
    }
}

impl<E> AsStorageMut<E> for HopSlotMap<InnerKey<E::Key>, E>
where
    E: 'static + Entity,
    E::Storage: Dispatch<E, Object = dyn IntrinsicStorage<E>>,
    InnerKey<E::Key>: SlotKey,
{
    fn as_storage_mut(&mut self) -> &mut StorageObject<E> {
        self
    }
}

impl<E> Dispatch<E> for HopSlotMap<InnerKey<E::Key>, E>
where
    E: Entity,
    InnerKey<E::Key>: SlotKey,
{
    type Object = dyn IntrinsicStorage<E>;
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
