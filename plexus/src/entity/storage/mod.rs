mod hash;
mod journal;
mod slot;

use fnv::FnvBuildHasher;
use std::collections::HashMap;
use std::hash::Hash;

use crate::entity::Entity;

// TODO: Should this module be flattened or expose sub-modules?
pub use crate::entity::storage::hash::FnvEntityMap;
pub use crate::entity::storage::journal::Unjournaled;
pub use crate::entity::storage::slot::SlotEntityMap;

#[cfg(not(all(nightly, feature = "unstable")))]
pub type StorageObject<E> = <<E as Entity>::Storage as Dispatch<E>>::Object;
#[cfg(all(nightly, feature = "unstable"))]
pub type StorageObject<'a, E> = <<E as Entity>::Storage as Dispatch<E>>::Object<'a>;

pub type Rekeying<E> = HashMap<<E as Entity>::Key, <E as Entity>::Key, FnvBuildHasher>;
pub type InnerKey<K> = <K as Key>::Inner;

pub trait Key: Copy + Eq + Hash + Sized {
    type Inner: Copy + Sized;

    fn from_inner(key: Self::Inner) -> Self;

    fn into_inner(self) -> Self::Inner;
}

#[cfg(not(all(nightly, feature = "unstable")))]
pub trait Dispatch<E>
where
    E: Entity,
{
    type Object: ?Sized + Storage<E>;
}

#[cfg(all(nightly, feature = "unstable"))]
#[rustfmt::skip]
pub trait Dispatch<E>
where
    E: Entity,
{
    type Object<'a>: 'a + ?Sized + Storage<E>
    where
        E: 'a;
}

// TODO: Can GATs be used here while still supporting trait objects?
pub trait Enumerate<E>
where
    E: Entity,
{
    fn len(&self) -> usize;

    fn iter<'a>(&'a self) -> Box<dyn 'a + ExactSizeIterator<Item = (E::Key, &E)>>;

    // TODO: Move this function out of storage traits and into unjournaled
    //       storage types.
    //
    //       This function is hostile to journaling and would require an
    //       iterator that puts writes in the log each time a mutable reference
    //       is pulled from the it. However, it is necessary for categorical
    //       iterators over user data in entities (e.g.,
    //       `MeshGraph::vertex_orphans`). This can be supported with a more
    //       limited function that is exclusive to unjournaled storage.
    fn iter_mut<'a>(&'a mut self) -> Box<dyn 'a + ExactSizeIterator<Item = (E::Key, &mut E)>>;

    fn keys<'a>(&'a self) -> Box<dyn 'a + ExactSizeIterator<Item = E::Key>>;
}

pub trait Get<E>
where
    E: Entity,
{
    fn get(&self, key: &E::Key) -> Option<&E>;

    fn get_mut(&mut self, key: &E::Key) -> Option<&mut E>;

    fn contains_key(&self, key: &E::Key) -> bool {
        self.get(key).is_some()
    }
}

pub trait Insert<E>
where
    E: Entity,
{
    fn insert(&mut self, entity: E) -> E::Key;
}

pub trait InsertWithKey<E>
where
    E: Entity,
{
    fn insert_with_key(&mut self, key: &E::Key, entity: E) -> Option<E>;
}

pub trait Remove<E>
where
    E: Entity,
{
    fn remove(&mut self, key: &E::Key) -> Option<E>;
}

pub trait Storage<E>: AsStorage<E> + AsStorageMut<E> + Enumerate<E> + Get<E> + Remove<E>
where
    E: Entity,
{
}

impl<T, E> Storage<E> for T
where
    T: AsStorage<E> + AsStorageMut<E> + Enumerate<E> + Get<E> + Remove<E>,
    E: Entity,
{
}

pub trait ExtrinsicStorage<E>: InsertWithKey<E> + Storage<E>
where
    E: Entity,
{
}

impl<T, E> ExtrinsicStorage<E> for T
where
    T: InsertWithKey<E> + Storage<E>,
    E: Entity,
{
}

pub trait IntrinsicStorage<E>: Insert<E> + Storage<E>
where
    E: Entity,
{
}

impl<T, E> IntrinsicStorage<E> for T
where
    T: Insert<E> + Storage<E>,
    E: Entity,
{
}

pub trait Fuse<M, T>
where
    M: AsStorage<T>,
    T: Entity,
{
    type Output: AsStorage<T>;

    fn fuse(self, source: M) -> Self::Output;
}

pub trait AsStorage<E>
where
    E: Entity,
{
    fn as_storage(&self) -> &StorageObject<E>;
}

impl<'a, E, T> AsStorage<E> for &'a T
where
    E: Entity,
    T: AsStorage<E> + ?Sized,
{
    fn as_storage(&self) -> &StorageObject<E> {
        <T as AsStorage<E>>::as_storage(self)
    }
}

impl<'a, E, T> AsStorage<E> for &'a mut T
where
    E: Entity,
    T: AsStorage<E> + ?Sized,
{
    fn as_storage(&self) -> &StorageObject<E> {
        <T as AsStorage<E>>::as_storage(self)
    }
}

pub trait AsStorageMut<E>: AsStorage<E>
where
    E: Entity,
{
    fn as_storage_mut(&mut self) -> &mut StorageObject<E>;
}

impl<'a, E, T> AsStorageMut<E> for &'a mut T
where
    E: Entity,
    T: AsStorageMut<E> + ?Sized,
{
    fn as_storage_mut(&mut self) -> &mut StorageObject<E> {
        <T as AsStorageMut<E>>::as_storage_mut(self)
    }
}

pub trait AsStorageOf {
    fn as_storage_of<E>(&self) -> &StorageObject<E>
    where
        E: Entity,
        Self: AsStorage<E>,
    {
        self.as_storage()
    }

    fn as_storage_mut_of<E>(&mut self) -> &mut StorageObject<E>
    where
        E: Entity,
        Self: AsStorageMut<E>,
    {
        self.as_storage_mut()
    }
}

impl<T> AsStorageOf for T {}
