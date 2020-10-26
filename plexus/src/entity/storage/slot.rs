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

pub struct State {
    floor: u32,
    index: u32,
    version: u32,
}

// See also the implementation of `JournalState` for `HopSlotMap`.
impl<K> SyntheticKey<State> for K
where
    K: Key,
    InnerKey<K>: SlotKey,
{
    fn synthesize(state: &mut State) -> Self {
        struct SyntheticKeyData {
            _index: u32,
            _version: NonZeroU32,
        }

        let State {
            ref floor,
            index,
            version,
        } = state;
        unsafe {
            let key = SyntheticKeyData {
                _index: *index,
                _version: NonZeroU32::new(*version).expect("zero version in synthesized key"),
            };
            *index = match index.overflowing_add(1) {
                (index, false) => index,
                (_, true) => {
                    *version = version.checked_add(2).expect("exhausted synthesized keys");
                    *floor
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
    type State = State;

    fn state(&self) -> Self::State {
        // TODO: Is this recoverable? Is it useful to propagate such an error?
        let floor = u32::try_from(self.capacity())
            .unwrap()
            .checked_add(1)
            .expect("insufficient capacity for journaling");
        let index = floor;
        let version = 1;
        State {
            floor,
            index,
            version,
        }
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

#[cfg(test)]
mod tests {
    use crate::entity::storage::journal::SyntheticKey;
    use crate::entity::storage::slot::State;
    use crate::entity::storage::tests::NodeKey;

    // TODO: This test exercises implementation details. Is there a better way
    //       to test key synthesis in general?
    #[test]
    fn synthetic_key_index_overflow() {
        let mut state = State {
            floor: u32::MAX - 1,
            index: u32::MAX - 1,
            version: 1,
        };

        let _ = NodeKey::synthesize(&mut state);
        assert_eq!(u32::MAX, state.index);
        assert_eq!(1, state.version);

        let _ = NodeKey::synthesize(&mut state);
        assert_eq!(u32::MAX - 1, state.index);
        assert_ne!(1, state.version);
    }
}
