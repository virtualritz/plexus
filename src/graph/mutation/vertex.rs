use failure::Error;

use geometry::Geometry;
use graph::mutation::{Commit, Mode, Mutate};
use graph::storage::convert::AsStorage;
use graph::storage::{Bind, Core, EdgeKey, Storage, VertexKey};
use graph::topology::Vertex;
use graph::GraphError;

pub struct VertexMutation<G>
where
    G: Geometry,
{
    storage: Storage<Vertex<G>>,
}

impl<G> VertexMutation<G>
where
    G: Geometry,
{
    pub fn insert_vertex(&mut self, geometry: G::Vertex) -> VertexKey {
        self.storage.insert(Vertex::new(geometry))
    }

    pub fn connect_outgoing_edge(&mut self, a: VertexKey, ab: EdgeKey) -> Result<(), Error> {
        if a == ab.to_vertex_keys().0 {
            let vertex = self
                .storage
                .get_mut(&a)
                .ok_or_else(|| Error::from(GraphError::TopologyNotFound))?;
            vertex.edge = Some(ab);
            Ok(())
        }
        else {
            Err(Error::from(GraphError::TopologyMalformed))
        }
    }

    pub fn disconnect_outgoing_edge(&mut self, a: VertexKey) -> Result<Option<EdgeKey>, Error> {
        let edge = self
            .storage
            .get_mut(&a)
            .ok_or_else(|| Error::from(GraphError::TopologyNotFound))?
            .edge
            .take();
        Ok(edge)
    }
}

impl<G> AsStorage<Vertex<G>> for VertexMutation<G>
where
    G: Geometry,
{
    fn as_storage(&self) -> &Storage<Vertex<G>> {
        &self.storage
    }
}

impl<G> Commit for VertexMutation<G>
where
    G: Geometry,
{
    type Error = Error;

    fn commit(self) -> Result<Self::Mutant, Self::Error> {
        let VertexMutation {
            storage: vertices, ..
        } = self;
        Ok(Core::empty().bind(vertices))
    }
}

impl<G> Mode for VertexMutation<G>
where
    G: Geometry,
{
    type Mutant = Core<Storage<Vertex<G>>, (), ()>;
}

impl<G> Mutate for VertexMutation<G>
where
    G: Geometry,
{
    fn mutate(mutant: Self::Mutant) -> Self {
        let Core { vertices, .. } = mutant;
        VertexMutation { storage: vertices }
    }
}