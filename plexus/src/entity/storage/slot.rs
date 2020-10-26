use slotmap::hop::HopSlotMap;
use slotmap::KeyData;
use std::convert::TryFrom;
use std::mem;
use std::num::NonZeroU32;

use crate::entity::storage::journal::{JournalState, SyntheticKey, Unjournaled};
use crate::entity::storage::{
    AsStorage, AsStorageMut, Dispatch, Enumerate, Get, IndependentStorage, InnerKey, Insert, Key,
    Remove, StorageObject,
};
use crate::entity::{Entity, Payload};

pub use slotmap::Key as SlotKey;

pub type SlotEntityMap<E> = HopSlotMap<InnerKey<<E as Entity>::Key>, E>;

// See also the implementation of `JournalState` for `HopSlotMap`.
impl<K> SyntheticKey<(u32, u32)> for K
where
    K: Key,
    InnerKey<K>: SlotKey,
{
    fn synthesize(state: &mut (u32, u32)) -> Self {
        struct SyntheticKey {
            _index: u32,
            _version: NonZeroU32,
        }

        unsafe {
            let key = SyntheticKey {
                _index: state.0,
                _version: NonZeroU32::new(state.1).expect("zero version in synthesized key"),
            };
            state.0 = match state.0.overflowing_add(1) {
                (index, false) => index,
                (index, true) => {
                    state.1 = state.1.checked_add(2).expect("exhausted synthesized keys");
                    index
                }
            };
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
    type Object = dyn 'static + IndependentStorage<E>;
}

#[cfg(all(nightly, feature = "unstable"))]
#[rustfmt::skip]
impl<E, K> Dispatch<E> for HopSlotMap<InnerKey<K>, E>
where
    E: Entity<Key = K, Storage = Self>,
    K: Key,
    InnerKey<K>: 'static + SlotKey,
{
    type Object<'a> where E: 'a = dyn 'a + IndependentStorage<E>;
}

impl<E> Enumerate<E> for HopSlotMap<InnerKey<E::Key>, E>
where
    E: Entity,
    InnerKey<E::Key>: SlotKey,
{
    fn len(&self) -> usize {
        self.len()
    }

    fn iter<'a>(&'a self) -> Box<dyn 'a + Iterator<Item = (E::Key, &E)>> {
        Box::new(
            self.iter()
                .map(|(key, entity)| (E::Key::from_inner(key), entity)),
        )
    }

    fn iter_mut<'a>(&'a mut self) -> Box<dyn 'a + Iterator<Item = (E::Key, &mut E::Data)>>
    where
        E: Payload,
    {
        Box::new(
            self.iter_mut()
                .map(|(key, entity)| (E::Key::from_inner(key), entity.get_mut())),
        )
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

// See also the implementation of `SyntheticKey` for wrapped `SlotKey`s.
impl<E> JournalState for HopSlotMap<InnerKey<E::Key>, E>
where
    E: Entity,
    InnerKey<E::Key>: SlotKey,
{
    type State = (u32, u32);

    fn state(&self) -> Self::State {
        // TODO: Is this recoverable? Is it useful to propagate such an error?
        let index = u32::try_from(self.capacity())
            .unwrap()
            .checked_add(1)
            .expect("insufficient capacity for journaling");
        let version = 1;
        (index, version)
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
