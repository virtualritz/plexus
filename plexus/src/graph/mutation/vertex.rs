use crate::entity::borrow::Reborrow;
use crate::entity::storage::{AsStorage, AsStorageMut, Fuse, StorageObject};
use crate::entity::Entity;
use crate::graph::core::Core;
use crate::graph::data::{Data, GraphData, Parametric};
use crate::graph::edge::ArcKey;
use crate::graph::mutation::edge::{self, EdgeRemoveCache};
use crate::graph::mutation::{Consistent, Mutable, Mutation};
use crate::graph::vertex::{Vertex, VertexKey, VertexView};
use crate::graph::GraphError;
use crate::transact::Transact;

type OwnedCore<G> = Core<G, <Vertex<G> as Entity>::Storage, (), (), ()>;
type RefCore<'a, G> = Core<G, &'a StorageObject<Vertex<G>>, (), (), ()>;

pub struct VertexMutation<M>
where
    M: Parametric,
{
    // TODO: Use and require journaled storage.
    storage: <Vertex<Data<M>> as Entity>::Storage,
}

impl<M, G> VertexMutation<M>
where
    M: Parametric<Data = G>,
    G: GraphData,
{
    pub fn to_ref_core(&self) -> RefCore<G> {
        Core::empty().fuse(self.storage.as_storage())
    }

    pub fn connect_outgoing_arc(&mut self, a: VertexKey, ab: ArcKey) -> Result<(), GraphError> {
        self.with_vertex_mut(a, |vertex| vertex.arc = Some(ab))
    }

    // TODO: See `edge::split_with_cache`.
    #[allow(dead_code)]
    pub fn disconnect_outgoing_arc(&mut self, a: VertexKey) -> Result<Option<ArcKey>, GraphError> {
        self.with_vertex_mut(a, |vertex| vertex.arc.take())
    }

    fn with_vertex_mut<T, F>(&mut self, a: VertexKey, mut f: F) -> Result<T, GraphError>
    where
        F: FnMut(&mut Vertex<G>) -> T,
    {
        let vertex = self
            .storage
            .as_storage_mut()
            .get_mut(&a)
            .ok_or_else(|| GraphError::TopologyNotFound)?;
        Ok(f(vertex))
    }
}

impl<M, G> AsStorage<Vertex<G>> for VertexMutation<M>
where
    M: Parametric<Data = G>,
    G: GraphData,
{
    fn as_storage(&self) -> &StorageObject<Vertex<G>> {
        &self.storage
    }
}

impl<M, G> From<OwnedCore<G>> for VertexMutation<M>
where
    M: Parametric<Data = G>,
    G: GraphData,
{
    fn from(core: OwnedCore<G>) -> Self {
        let (vertices, ..) = core.unfuse();
        VertexMutation { storage: vertices }
    }
}

impl<M, G> Transact<OwnedCore<G>> for VertexMutation<M>
where
    M: Parametric<Data = G>,
    G: GraphData,
{
    type Output = OwnedCore<G>;
    type Error = GraphError;

    fn commit(self) -> Result<Self::Output, Self::Error> {
        let VertexMutation {
            storage: vertices, ..
        } = self;
        // In a consistent graph, all vertices must have a leading arc.
        for (_, vertex) in vertices.iter() {
            if vertex.arc.is_none() {
                return Err(GraphError::TopologyMalformed);
            }
        }
        Ok(Core::empty().fuse(vertices))
    }
}

pub struct VertexRemoveCache {
    cache: Vec<EdgeRemoveCache>,
}

impl VertexRemoveCache {
    pub fn from_vertex<B>(vertex: VertexView<B>) -> Result<Self, GraphError>
    where
        B: Reborrow,
        B::Target: AsStorage<Vertex<Data<B>>> + Consistent + Parametric,
    {
        let _ = vertex;
        unimplemented!()
    }
}

pub fn insert<M, N>(mut mutation: N, geometry: <Data<M> as GraphData>::Vertex) -> VertexKey
where
    N: AsMut<Mutation<M>>,
    M: Mutable,
{
    mutation
        .as_mut()
        .storage
        .as_storage_mut()
        .insert(Vertex::new(geometry))
}

pub fn remove<M, N>(
    mut mutation: N,
    cache: VertexRemoveCache,
) -> Result<Vertex<Data<M>>, GraphError>
where
    N: AsMut<Mutation<M>>,
    M: Mutable,
{
    let VertexRemoveCache { cache } = cache;
    for cache in cache {
        edge::remove(mutation.as_mut(), cache)?;
    }
    unimplemented!()
}
