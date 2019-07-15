use crate::graph::geometry::GraphGeometry;
use crate::graph::storage::payload::{ArcPayload, EdgePayload, FacePayload, VertexPayload};
use crate::graph::storage::{AsStorage, AsStorageMut, StorageProxy};

pub trait AsVertexStorage<G>: AsStorage<VertexPayload<G>>
where
    G: GraphGeometry,
{
    #[inline(always)]
    fn as_vertex_storage(&self) -> &StorageProxy<VertexPayload<G>> {
        self.as_storage()
    }
}

impl<T, G> AsVertexStorage<G> for T
where
    T: AsStorage<VertexPayload<G>>,
    G: GraphGeometry,
{
}

pub trait AsVertexStorageMut<G>: AsStorageMut<VertexPayload<G>>
where
    G: GraphGeometry,
{
    #[inline(always)]
    fn as_vertex_storage_mut(&mut self) -> &mut StorageProxy<VertexPayload<G>> {
        self.as_storage_mut()
    }
}

impl<T, G> AsVertexStorageMut<G> for T
where
    T: AsStorageMut<VertexPayload<G>>,
    G: GraphGeometry,
{
}

pub trait AsArcStorage<G>: AsStorage<ArcPayload<G>>
where
    G: GraphGeometry,
{
    #[inline(always)]
    fn as_arc_storage(&self) -> &StorageProxy<ArcPayload<G>> {
        self.as_storage()
    }
}

impl<T, G> AsArcStorage<G> for T
where
    T: AsStorage<ArcPayload<G>>,
    G: GraphGeometry,
{
}

pub trait AsArcStorageMut<G>: AsStorageMut<ArcPayload<G>>
where
    G: GraphGeometry,
{
    #[inline(always)]
    fn as_arc_storage_mut(&mut self) -> &mut StorageProxy<ArcPayload<G>> {
        self.as_storage_mut()
    }
}

impl<T, G> AsArcStorageMut<G> for T
where
    T: AsStorageMut<ArcPayload<G>>,
    G: GraphGeometry,
{
}

pub trait AsEdgeStorage<G>: AsStorage<EdgePayload<G>>
where
    G: GraphGeometry,
{
    #[inline(always)]
    fn as_edge_storage(&self) -> &StorageProxy<EdgePayload<G>> {
        self.as_storage()
    }
}

impl<T, G> AsEdgeStorage<G> for T
where
    T: AsStorage<EdgePayload<G>>,
    G: GraphGeometry,
{
}

pub trait AsEdgeStorageMut<G>: AsStorageMut<EdgePayload<G>>
where
    G: GraphGeometry,
{
    #[inline(always)]
    fn as_edge_storage_mut(&mut self) -> &mut StorageProxy<EdgePayload<G>> {
        self.as_storage_mut()
    }
}

impl<T, G> AsEdgeStorageMut<G> for T
where
    T: AsStorageMut<EdgePayload<G>>,
    G: GraphGeometry,
{
}

pub trait AsFaceStorage<G>: AsStorage<FacePayload<G>>
where
    G: GraphGeometry,
{
    #[inline(always)]
    fn as_face_storage(&self) -> &StorageProxy<FacePayload<G>> {
        self.as_storage()
    }
}

impl<T, G> AsFaceStorage<G> for T
where
    T: AsStorage<FacePayload<G>>,
    G: GraphGeometry,
{
}

pub trait AsFaceStorageMut<G>: AsStorageMut<FacePayload<G>>
where
    G: GraphGeometry,
{
    #[inline(always)]
    fn as_face_storage_mut(&mut self) -> &mut StorageProxy<FacePayload<G>> {
        self.as_storage_mut()
    }
}

impl<T, G> AsFaceStorageMut<G> for T
where
    T: AsStorageMut<FacePayload<G>>,
    G: GraphGeometry,
{
}