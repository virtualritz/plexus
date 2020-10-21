use slotmap::hop::HopSlotMap;
use slotmap::KeyData;
use std::mem;
use std::num::NonZeroU32;

use crate::entity::storage::journal::{JournalState, SyntheticKey, Unjournaled};
use crate::entity::storage::{
    AsStorage, AsStorageMut, Dispatch, Enumerate, Get, InnerKey, Insert, IntrinsicStorage, Key,
    Remove, StorageObject,
};
use crate::entity::{Entity, Payload};

pub use slotmap::Key as SlotKey;

pub type SlotEntityMap<E> = HopSlotMap<InnerKey<<E as Entity>::Key>, E>;

impl<K> SyntheticKey<u32> for K
where
    K: Key,
    InnerKey<K>: SlotKey,
{
    fn synthesize(state: &mut u32) -> Self {
        struct SyntheticKey {
            _index: u32,
            _version: NonZeroU32,
        }

        unsafe {
            let key = SyntheticKey {
                _index: *state,
                _version: NonZeroU32::new_unchecked(u32::MAX - 1),
            };
            // TODO: This may overflow.
            *state += 1;
            Key::from_inner(mem::transmute::<_, KeyData>(key).into())
        }
    }
}

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

impl<E> Enumerate<E> for HopSlotMap<InnerKey<E::Key>, E>
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

    fn iter_mut<'a>(&'a mut self) -> Box<dyn 'a + ExactSizeIterator<Item = (E::Key, &mut E::Data)>>
    where
        E: Payload,
    {
        Box::new(
            self.iter_mut()
                .map(|(key, entity)| (E::Key::from_inner(key), entity.get_mut())),
        )
    }

    fn keys<'a>(&'a self) -> Box<dyn 'a + ExactSizeIterator<Item = E::Key>> {
        Box::new(self.keys().map(E::Key::from_inner))
    }
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

// TODO: Use more sophisticated state to avoid overflows and use 64 bits of the
//       key rather than just the 32 bits of the index. See `SyntheticKey`.
impl<E> JournalState for HopSlotMap<InnerKey<E::Key>, E>
where
    E: Entity,
    InnerKey<E::Key>: SlotKey,
{
    type State = u32;

    fn state(&self) -> Self::State {
        // TODO: This may overflow.
        (self.capacity() + 1) as u32
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

impl<K, T> Unjournaled for HopSlotMap<K, T>
where
    K: SlotKey,
    T: Copy,
{
}
