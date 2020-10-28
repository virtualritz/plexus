pub mod edge;
pub mod face;
pub mod path;
pub mod vertex;

use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

use crate::entity::storage::{AsStorage, AsStorageMut, Journaled, StorageObject};
use crate::entity::Entity;
use crate::graph::core::OwnedCore;
use crate::graph::data::{Data, Parametric};
use crate::graph::edge::{Arc, Edge};
use crate::graph::face::Face;
use crate::graph::mutation::face::FaceMutation;
use crate::graph::vertex::Vertex;
use crate::graph::{GraphData, GraphError};
use crate::transact::Transact;

// TODO: The `Transact` trait provides no output on failure. This prevents the
//       direct restoration of a graph that has been journaled. Enhance the
//       `Transact` trait to support this.
// TODO: The mutation API exposes raw entities (see removals). It would be ideal
//       if those types need not be exposed at all, since they have limited
//       utility to users. Is it possible to expose user data instead of
//       entities in these APIs?

/// Marker trait for graph representations that promise to be in a consistent
/// state.
///
/// This trait is only implemented by representations that ensure that their
/// storage is only ever mutated via the mutation API (and therefore is
/// consistent). Note that `Core` does not implement this trait and instead acts
/// as a raw container for topological storage that can be freely manipulated.
///
/// This trait allows code to make assumptions about the data it operates
/// against. For example, views expose an API to user code that assumes that
/// topologies are present and therefore unwraps values.
pub trait Consistent {}

impl<'a, T> Consistent for &'a T where T: Consistent {}

impl<'a, T> Consistent for &'a mut T where T: Consistent {}

// TODO: Can a single type parameter implementing this trait be used in
//       `Mutation`?
pub trait Mode<G>
where
    G: GraphData,
{
    type VertexStorage: AsStorageMut<Vertex<G>>;
    type ArcStorage: AsStorageMut<Arc<G>>;
    type EdgeStorage: AsStorageMut<Edge<G>>;
    type FaceStorage: AsStorageMut<Face<G>>;
}

pub struct Immediate<G> {
    phantom: PhantomData<G>,
}

impl<G> Mode<G> for Immediate<G>
where
    G: GraphData,
{
    type VertexStorage = <Vertex<G> as Entity>::Storage;
    type ArcStorage = <Arc<G> as Entity>::Storage;
    type EdgeStorage = <Edge<G> as Entity>::Storage;
    type FaceStorage = <Face<G> as Entity>::Storage;
}

pub struct Transacted<G> {
    phantom: PhantomData<G>,
}

impl<G> Mode<G> for Transacted<G>
where
    G: GraphData,
{
    type VertexStorage = Journaled<<Vertex<G> as Entity>::Storage, Vertex<G>>;
    type ArcStorage = Journaled<<Arc<G> as Entity>::Storage, Arc<G>>;
    type EdgeStorage = Journaled<<Edge<G> as Entity>::Storage, Edge<G>>;
    type FaceStorage = Journaled<<Face<G> as Entity>::Storage, Face<G>>;
}

/// Graph mutation.
pub struct Mutation<P, M>
where
    P: Mode<Data<M>>,
    M: Consistent + From<OwnedCore<Data<M>>> + Parametric + Into<OwnedCore<Data<M>>>,
{
    inner: FaceMutation<P, M>,
}

impl<P, M, G> Mutation<P, M>
where
    P: Mode<G>,
    M: Consistent + From<OwnedCore<G>> + Parametric<Data = G> + Into<OwnedCore<G>>,
    G: GraphData,
{
}

impl<P, M, G> AsRef<Self> for Mutation<P, M>
where
    P: Mode<G>,
    M: Consistent + From<OwnedCore<G>> + Parametric<Data = G> + Into<OwnedCore<G>>,
    G: GraphData,
{
    fn as_ref(&self) -> &Self {
        self
    }
}

impl<P, M, G> AsMut<Self> for Mutation<P, M>
where
    P: Mode<G>,
    M: Consistent + From<OwnedCore<G>> + Parametric<Data = G> + Into<OwnedCore<G>>,
    G: GraphData,
{
    fn as_mut(&mut self) -> &mut Self {
        self
    }
}

impl<P, M, G> AsStorage<Arc<G>> for Mutation<P, M>
where
    P: Mode<G>,
    M: Consistent + From<OwnedCore<G>> + Parametric<Data = G> + Into<OwnedCore<G>>,
    G: GraphData,
{
    fn as_storage(&self) -> &StorageObject<Arc<G>> {
        self.inner.to_ref_core().unfuse().1
    }
}

impl<P, M, G> AsStorage<Edge<G>> for Mutation<P, M>
where
    P: Mode<G>,
    M: Consistent + From<OwnedCore<G>> + Parametric<Data = G> + Into<OwnedCore<G>>,
    G: GraphData,
{
    fn as_storage(&self) -> &StorageObject<Edge<G>> {
        self.inner.to_ref_core().unfuse().2
    }
}

impl<P, M, G> AsStorage<Face<G>> for Mutation<P, M>
where
    P: Mode<G>,
    M: Consistent + From<OwnedCore<G>> + Parametric<Data = G> + Into<OwnedCore<G>>,
    G: GraphData,
{
    fn as_storage(&self) -> &StorageObject<Face<G>> {
        self.inner.to_ref_core().unfuse().3
    }
}

impl<P, M, G> AsStorage<Vertex<G>> for Mutation<P, M>
where
    P: Mode<G>,
    M: Consistent + From<OwnedCore<G>> + Parametric<Data = G> + Into<OwnedCore<G>>,
    G: GraphData,
{
    fn as_storage(&self) -> &StorageObject<Vertex<G>> {
        self.inner.to_ref_core().unfuse().0
    }
}

// TODO: This is a hack. Replace this with delegation.
impl<P, M, G> Deref for Mutation<P, M>
where
    P: Mode<G>,
    M: Consistent + From<OwnedCore<G>> + Parametric<Data = G> + Into<OwnedCore<G>>,
    G: GraphData,
{
    type Target = FaceMutation<P, M>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<P, M, G> DerefMut for Mutation<P, M>
where
    P: Mode<G>,
    M: Consistent + From<OwnedCore<G>> + Parametric<Data = G> + Into<OwnedCore<G>>,
    G: GraphData,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<P, M, G> From<M> for Mutation<P, M>
where
    P: Mode<G>,
    P::VertexStorage: From<<Vertex<G> as Entity>::Storage>,
    P::ArcStorage: From<<Arc<G> as Entity>::Storage>,
    P::EdgeStorage: From<<Edge<G> as Entity>::Storage>,
    P::FaceStorage: From<<Face<G> as Entity>::Storage>,
    M: Consistent + From<OwnedCore<G>> + Parametric<Data = G> + Into<OwnedCore<G>>,
    G: GraphData,
{
    fn from(graph: M) -> Self {
        Mutation {
            inner: graph.into().into(),
        }
    }
}

impl<P, M, G> Parametric for Mutation<P, M>
where
    P: Mode<G>,
    M: Consistent + From<OwnedCore<G>> + Parametric<Data = G> + Into<OwnedCore<G>>,
    G: GraphData,
{
    type Data = G;
}

impl<M, G> Transact<M> for Mutation<Immediate<G>, M>
where
    M: Consistent + From<OwnedCore<G>> + Parametric<Data = G> + Into<OwnedCore<G>>,
    G: GraphData,
{
    type Output = M;
    type Error = GraphError;

    fn commit(self) -> Result<Self::Output, Self::Error> {
        self.inner.commit().map(|core| core.into())
    }
}

pub trait Mutable:
    Consistent + From<OwnedCore<Data<Self>>> + Parametric + Into<OwnedCore<Data<Self>>>
{
}

impl<M, G> Mutable for M
where
    M: Consistent + From<OwnedCore<G>> + Parametric<Data = G> + Into<OwnedCore<G>>,
    G: GraphData,
{
}
