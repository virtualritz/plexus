use either::Either;
use std::cmp;
use std::marker::PhantomData;
use std::mem;
use std::ops::{Deref, DerefMut};
use theon::query::{Intersection, Line, Plane};
use theon::space::{EuclideanSpace, FiniteDimensional, Scalar, Vector};
use typenum::U3;

use crate::geometry::AsPosition;
use crate::graph::borrow::{Reborrow, ReborrowMut};
use crate::graph::geometry::{FaceCentroid, FaceNormal, FacePlane, GraphGeometry, VertexPosition};
use crate::graph::mutation::face::{
    self, FaceBridgeCache, FaceExtrudeCache, FaceInsertCache, FacePokeCache, FaceRemoveCache,
    FaceSplitCache,
};
use crate::graph::mutation::{Consistent, Mutable, Mutate, Mutation};
use crate::graph::storage::key::{ArcKey, FaceKey, VertexKey};
use crate::graph::storage::payload::{ArcPayload, EdgePayload, FacePayload, VertexPayload};
use crate::graph::storage::{AsStorage, AsStorageMut, StorageProxy};
use crate::graph::view::edge::{ArcView, OrphanArcView};
use crate::graph::view::vertex::{OrphanVertexView, VertexView};
use crate::graph::view::{FromKeyedSource, IntoKeyedSource, IntoView, OrphanView, View};
use crate::graph::{GraphError, OptionExt, ResultExt, Selector};

use Selector::ByIndex;

// TODO: The API for faces and interior paths presents fuzzy distinctions; many
//       operations supported by `FaceView` could be supported by
//       `InteriorPathView` as well (specifically, all topological operations
//       where a `FacePayload` is unnecessary). In essence, a face is simply an
//       interior path with an associated payload that describes its path and
//       geometry. The geometry is the most notable difference, keeping in mind
//       that in a consistent graph all arcs are part of an interior path.

pub trait InteriorPath<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<ArcPayload<G>>,
    G: GraphGeometry,
{
    fn reachable_vertices(&self) -> VertexCirculator<&M::Target, G>
    where
        M::Target: AsStorage<VertexPayload<G>>,
    {
        self.reachable_arcs().into()
    }

    fn vertices(&self) -> VertexCirculator<&M::Target, G>
    where
        M::Target: AsStorage<VertexPayload<G>> + Consistent,
    {
        self.reachable_vertices()
    }

    fn reachable_arcs(&self) -> ArcCirculator<&M::Target, G>;

    fn arcs(&self) -> ArcCirculator<&M::Target, G>
    where
        M::Target: Consistent,
    {
        self.reachable_arcs()
    }

    fn arity(&self) -> usize
    where
        M::Target: Consistent,
    {
        self.arcs().count()
    }

    fn distance(
        &self,
        source: Selector<VertexKey>,
        destination: Selector<VertexKey>,
    ) -> Result<usize, GraphError>
    where
        M::Target: AsStorage<VertexPayload<G>> + Consistent,
    {
        let arity = self.arity();
        let select = |selector: Selector<_>| {
            selector
                .index_or_else(|key| {
                    self.vertices()
                        .map(|vertex| vertex.key())
                        .enumerate()
                        .find(|(_, a)| *a == key)
                        .map(|(index, _)| index)
                        .ok_or_else(|| GraphError::TopologyNotFound)
                })
                .and_then(|index| {
                    if index >= arity {
                        Err(GraphError::TopologyNotFound)
                    }
                    else {
                        Ok(index)
                    }
                })
        };
        let source = select(source)? as isize;
        let destination = select(destination)? as isize;
        let difference = (source - destination).abs() as usize;
        Ok(cmp::min(difference, arity - difference))
    }
}

/// View of a face.
///
/// Provides traversals, queries, and mutations related to faces in a graph.
/// See the module documentation for more information about topological views.
///
/// Faces are notated similarly to paths. A triangular face with a perimeter
/// formed by vertices $A$, $B$, and $C$ is notated $\Overrightarrow{\\{A, B,
/// C\\}}$ (using a double-struck arrow).
pub struct FaceView<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<FacePayload<G>>,
    G: GraphGeometry,
{
    inner: View<M, FacePayload<G>>,
}

impl<M, G> FaceView<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<FacePayload<G>>,
    G: GraphGeometry,
{
    fn into_inner(self) -> View<M, FacePayload<G>> {
        let FaceView { inner, .. } = self;
        inner
    }

    fn interior_reborrow(&self) -> FaceView<&M::Target, G> {
        self.inner.interior_reborrow().into()
    }

    /// Gets the key for the face.
    pub fn key(&self) -> FaceKey {
        self.inner.key()
    }
}

impl<M, G> FaceView<M, G>
where
    M: Reborrow + ReborrowMut,
    M::Target: AsStorage<FacePayload<G>>,
    G: GraphGeometry,
{
    fn interior_reborrow_mut(&mut self) -> FaceView<&mut M::Target, G> {
        self.inner.interior_reborrow_mut().into()
    }
}

impl<'a, M, G> FaceView<&'a mut M, G>
where
    M: 'a + AsStorage<FacePayload<G>> + AsStorageMut<FacePayload<G>>,
    G: 'a + GraphGeometry,
{
    /// Converts a mutable view into an orphan view.
    pub fn into_orphan(self) -> OrphanFaceView<'a, G> {
        self.into_inner().into_orphan().into()
    }

    /// Converts a mutable view into an immutable view.
    ///
    /// This is useful when mutations are not (or no longer) needed and mutual
    /// access is desired.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # extern crate decorum;
    /// # extern crate nalgebra;
    /// # extern crate plexus;
    /// #
    /// use decorum::N64;
    /// use nalgebra::Point3;
    /// use plexus::graph::MeshGraph;
    /// use plexus::prelude::*;
    /// use plexus::primitive::cube::Cube;
    ///
    /// # fn main() {
    /// let mut graph = Cube::new()
    ///     .polygons_with_position::<Point3<N64>>()
    ///     .collect::<MeshGraph<Point3<f64>>>();
    /// let key = graph.faces().nth(0).unwrap().key();
    /// let face = graph.face_mut(key).unwrap().extrude(1.0).into_ref();
    ///
    /// // This would not be possible without conversion into an immutable view.
    /// let _ = face.into_arc();
    /// let _ = face.into_arc().into_next_arc();
    /// # }
    /// ```
    pub fn into_ref(self) -> FaceView<&'a M, G> {
        self.into_inner().into_ref().into()
    }

    /// Reborrows the view and constructs another mutable view from a given
    /// key.
    ///
    /// This allows for fallible traversals from a mutable view without the
    /// need for direct access to the source `MeshGraph`. If the given function
    /// emits a key, then that key will be used to convert this view into
    /// another. If no key is emitted, then the original mutable view is
    /// returned.
    pub fn with_ref<T, K, F>(self, f: F) -> Either<Result<T, GraphError>, Self>
    where
        T: FromKeyedSource<(K, &'a mut M)>,
        F: FnOnce(FaceView<&M, G>) -> Option<K>,
    {
        if let Some(key) = f(self.interior_reborrow()) {
            let (_, storage) = self.into_inner().into_keyed_source();
            Either::Left(
                T::from_keyed_source((key, storage)).ok_or_else(|| GraphError::TopologyNotFound),
            )
        }
        else {
            Either::Right(self)
        }
    }
}

/// Reachable API.
impl<M, G> FaceView<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<ArcPayload<G>> + AsStorage<FacePayload<G>>,
    G: GraphGeometry,
{
    pub(in crate::graph) fn into_reachable_arc(self) -> Option<ArcView<M, G>> {
        let key = self.arc;
        self.into_inner().rekey_map(key)
    }

    pub(in crate::graph) fn reachable_arc(&self) -> Option<ArcView<&M::Target, G>> {
        let key = self.arc;
        self.inner.interior_reborrow().rekey_map(key)
    }

    pub(in crate::graph) fn reachable_interior_arcs(
        &self,
    ) -> impl Clone + Iterator<Item = ArcView<&M::Target, G>> {
        <Self as InteriorPath<_, _>>::reachable_arcs(self)
    }

    pub(in crate::graph) fn reachable_neighboring_faces(
        &self,
    ) -> impl Clone + Iterator<Item = FaceView<&M::Target, G>> {
        FaceCirculator::from(<Self as InteriorPath<_, _>>::reachable_arcs(self))
    }
}

impl<M, G> FaceView<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<ArcPayload<G>> + AsStorage<FacePayload<G>> + Consistent,
    G: GraphGeometry,
{
    /// Converts the face into its interior path.
    pub fn into_interior_path(self) -> InteriorPathView<M, G> {
        let key = self.arc().key();
        self.into_inner().rekey_map(key).expect_consistent()
    }

    /// Converts the face into its leading arc.
    pub fn into_arc(self) -> ArcView<M, G> {
        self.into_reachable_arc().expect_consistent()
    }

    /// Gets the interior path of the face.
    pub fn interior_path(&self) -> InteriorPathView<&M::Target, G> {
        let key = self.arc().key();
        self.inner
            .interior_reborrow()
            .rekey_map(key)
            .expect_consistent()
    }

    /// Gets the leading arc of the face.
    pub fn arc(&self) -> ArcView<&M::Target, G> {
        self.reachable_arc().expect_consistent()
    }

    /// Gets an iterator of views over the arcs in the face's interior path.
    pub fn interior_arcs(&self) -> impl Clone + Iterator<Item = ArcView<&M::Target, G>> {
        self.reachable_interior_arcs()
    }

    /// Gets an iterator of views over neighboring faces.
    pub fn neighboring_faces(&self) -> impl Clone + Iterator<Item = FaceView<&M::Target, G>> {
        self.reachable_neighboring_faces()
    }

    /// Gets the arity of the face. This is the number of arcs that form the
    /// face's interior path.
    pub fn arity(&self) -> usize {
        <Self as InteriorPath<_, _>>::arity(self)
    }
}

/// Reachable API.
impl<M, G> FaceView<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<ArcPayload<G>> + AsStorage<FacePayload<G>> + AsStorage<VertexPayload<G>>,
    G: GraphGeometry,
{
    pub(in crate::graph) fn reachable_vertices(
        &self,
    ) -> impl Clone + Iterator<Item = VertexView<&M::Target, G>> {
        <Self as InteriorPath<_, _>>::reachable_vertices(self)
    }
}

impl<M, G> FaceView<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<ArcPayload<G>>
        + AsStorage<FacePayload<G>>
        + AsStorage<VertexPayload<G>>
        + Consistent,
    G: GraphGeometry,
{
    /// Gets an iterator of views over the vertices that form the face.
    pub fn vertices(&self) -> impl Clone + Iterator<Item = VertexView<&M::Target, G>> {
        // TODO: This does not use the `InteriorPath` trait directly to prevent
        //       dead code warnings on `reachable_vertices`. It would be
        //       preferable for `reachable_vertices` to be implemented directly
        //       and then used to implement `InteriorPath`, but that trait must
        //       specify a concrete type for its non-consuming iterators, so
        //       the code is reused the other way around. This could be changed
        //       when GATs land in Rust.
        self.reachable_vertices()
    }

    pub fn centroid(&self) -> G::Centroid
    where
        G: FaceCentroid,
    {
        G::centroid(self.interior_reborrow()).expect_consistent()
    }

    pub fn normal(&self) -> G::Normal
    where
        G: FaceNormal,
    {
        G::normal(self.interior_reborrow()).expect_consistent()
    }

    pub fn plane(&self) -> Result<G::Plane, GraphError>
    where
        G: FacePlane,
    {
        G::plane(self.interior_reborrow())
    }
}

/// Reachable API.
impl<M, G> FaceView<M, G>
where
    M: Reborrow + ReborrowMut,
    M::Target: AsStorage<ArcPayload<G>> + AsStorageMut<ArcPayload<G>> + AsStorage<FacePayload<G>>,
    G: GraphGeometry,
{
    pub(in crate::graph) fn reachable_interior_orphan_arcs(
        &mut self,
    ) -> impl Iterator<Item = OrphanArcView<G>> {
        ArcCirculator::from(self.interior_reborrow_mut())
    }
}

impl<M, G> FaceView<M, G>
where
    M: Reborrow + ReborrowMut,
    M::Target: AsStorage<ArcPayload<G>>
        + AsStorageMut<ArcPayload<G>>
        + AsStorage<FacePayload<G>>
        + Consistent,
    G: GraphGeometry,
{
    /// Gets an iterator of orphan views over the arcs in the face's interior
    /// path.
    pub fn interior_orphan_arcs(&mut self) -> impl Iterator<Item = OrphanArcView<G>> {
        self.reachable_interior_orphan_arcs()
    }
}

/// Reachable API.
impl<M, G> FaceView<M, G>
where
    M: Reborrow + ReborrowMut,
    M::Target: AsStorage<ArcPayload<G>> + AsStorage<FacePayload<G>> + AsStorageMut<FacePayload<G>>,
    G: GraphGeometry,
{
    pub(in crate::graph) fn reachable_neighboring_orphan_faces(
        &mut self,
    ) -> impl Iterator<Item = OrphanFaceView<G>> {
        FaceCirculator::from(ArcCirculator::from(self.interior_reborrow_mut()))
    }
}

impl<M, G> FaceView<M, G>
where
    M: Reborrow + ReborrowMut,
    M::Target: AsStorage<ArcPayload<G>>
        + AsStorage<FacePayload<G>>
        + AsStorageMut<FacePayload<G>>
        + Consistent,
    G: GraphGeometry,
{
    /// Gets an iterator of orphan views over neighboring faces.
    pub fn neighboring_orphan_faces(&mut self) -> impl Iterator<Item = OrphanFaceView<G>> {
        self.reachable_neighboring_orphan_faces()
    }
}

/// Reachable API.
impl<M, G> FaceView<M, G>
where
    M: Reborrow + ReborrowMut,
    M::Target: AsStorage<ArcPayload<G>>
        + AsStorage<FacePayload<G>>
        + AsStorage<VertexPayload<G>>
        + AsStorageMut<VertexPayload<G>>,
    G: GraphGeometry,
{
    pub(in crate::graph) fn reachable_orphan_vertices(
        &mut self,
    ) -> impl Iterator<Item = OrphanVertexView<G>> {
        VertexCirculator::from(ArcCirculator::from(self.interior_reborrow_mut()))
    }
}

impl<M, G> FaceView<M, G>
where
    M: Reborrow + ReborrowMut,
    M::Target: AsStorage<ArcPayload<G>>
        + AsStorage<FacePayload<G>>
        + AsStorage<VertexPayload<G>>
        + AsStorageMut<VertexPayload<G>>
        + Consistent,
    G: GraphGeometry,
{
    /// Gets an iterator of orphan views over the vertices that form the face.
    pub fn orphan_vertices(&mut self) -> impl Iterator<Item = OrphanVertexView<G>> {
        self.reachable_orphan_vertices()
    }

    /// Flattens the face by translating the positions of all vertices into a
    /// best-fit plane.
    ///
    /// Returns an error if a best-fit plane could not be computed or positions
    /// could not be translated into the plane.
    pub fn flatten(&mut self) -> Result<(), GraphError>
    where
        G: FacePlane<Plane = Plane<VertexPosition<G>>>,
        G::Vertex: AsPosition,
        VertexPosition<G>: EuclideanSpace + FiniteDimensional<N = U3>,
    {
        if self.arity() == 3 {
            return Ok(());
        }
        let plane = self.plane()?;
        for mut vertex in self.orphan_vertices() {
            let position = vertex.position().clone();
            let line = Line::<VertexPosition<G>> {
                origin: position,
                direction: plane.normal,
            };
            // TODO: If the intersection yields no result, then this may fail
            //       after mutating positions in the graph. Consider using
            //       read/write stages to avoid partial completion.
            let distance = plane
                .intersection(&line)
                .ok_or_else(|| GraphError::Geometry)?;
            let translation = line.direction.get().clone() * distance;
            *vertex.geometry.as_position_mut() = position + translation;
        }
        Ok(())
    }
}

impl<'a, M, G> FaceView<&'a mut M, G>
where
    M: AsStorage<ArcPayload<G>>
        + AsStorage<EdgePayload<G>>
        + AsStorage<FacePayload<G>>
        + AsStorage<VertexPayload<G>>
        + Default
        + Mutable<G>,
    G: 'a + GraphGeometry,
{
    /// Splits the face by bisecting it with a composite edge inserted between
    /// two non-neighboring vertices within the face's perimeter.
    ///
    /// The vertices can be chosen by key or index, where index selects the
    /// $n^\text{th}$ vertex within the face's interior path.
    ///
    /// This can be thought of as the opposite of `merge`.
    ///
    /// Returns the inserted arc that spans from the source vertex to the
    /// destination vertex if successful. If a face $\Overrightarrow{\\{A, B,
    /// C, D\\}}$ is split from $A$ to $C$, then it will be decomposed into
    /// $\Overrightarrow{\\{A, B, C\\}}$ and $\Overrightarrow{\\{C, D, A\\}}$
    /// and the arc $\overrightarrow{AC}$ will be returned.
    ///
    /// # Errors
    ///
    /// Returns an error if either of the given vertices cannot be found, are
    /// not within the face's perimeter, or the distance between the vertices
    /// along the interior path is less than two.
    ///
    /// # Examples
    ///
    /// Splitting a quadrilateral face:
    ///
    /// ```rust
    /// # extern crate nalgebra;
    /// # extern crate plexus;
    /// #
    /// use nalgebra::Point2;
    /// use plexus::graph::MeshGraph;
    /// use plexus::prelude::*;
    /// use plexus::primitive::Quad;
    ///
    /// # fn main() {
    /// let mut graph = MeshGraph::<Point2<f64>>::from_raw_buffers(
    ///     vec![Quad::new(0usize, 1, 2, 3)],
    ///     vec![(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)],
    /// )
    /// .unwrap();
    /// let key = graph.faces().nth(0).unwrap().key();
    /// let arc = graph
    ///     .face_mut(key)
    ///     .unwrap()
    ///     .split(ByIndex(0), ByIndex(2))
    ///     .unwrap()
    ///     .into_ref();
    /// # }
    /// ```
    pub fn split(
        self,
        source: Selector<VertexKey>,
        destination: Selector<VertexKey>,
    ) -> Result<ArcView<&'a mut M, G>, GraphError> {
        let source = source.key_or_else(|index| {
            self.vertices()
                .nth(index)
                .ok_or_else(|| GraphError::TopologyNotFound)
                .map(|vertex| vertex.key())
        })?;
        let destination = destination.key_or_else(|index| {
            self.vertices()
                .nth(index)
                .ok_or_else(|| GraphError::TopologyNotFound)
                .map(|vertex| vertex.key())
        })?;
        let (abc, storage) = self.into_inner().into_keyed_source();
        // Errors can easily be caused by inputs to this function. Allow errors
        // from the snapshot to propagate.
        let cache = FaceSplitCache::snapshot(&storage, abc, source, destination)?;
        Ok(Mutation::replace(storage, Default::default())
            .commit_with(move |mutation| face::split_with_cache(mutation, cache))
            .map(|(storage, arc)| (arc, storage).into_view().expect_consistent())
            .expect_consistent())
    }

    /// Merges the face into a neighboring face over their shared composite
    /// edge.
    ///
    /// The neighboring face can be chosen by key or index, where index selects
    /// the $n^\text{th}$ neighbor of the face.
    ///
    /// This can be thought of as the opposite of `split`.
    ///
    /// Returns the merged face if successful.
    ///
    /// # Errors
    ///
    /// Returns an error if the destination face cannot be found or is not a
    /// neighbor of the initiating face.
    ///
    /// # Examples
    ///
    /// Merging two neighboring quadrilateral faces:
    ///
    /// ```rust
    /// # extern crate nalgebra;
    /// # extern crate plexus;
    /// #
    /// use nalgebra::Point2;
    /// use plexus::graph::MeshGraph;
    /// use plexus::prelude::*;
    /// use plexus::primitive::Quad;
    ///
    /// # fn main() {
    /// let mut graph = MeshGraph::<Point2<f64>>::from_raw_buffers(
    ///     vec![Quad::new(0usize, 1, 2, 3), Quad::new(0, 3, 4, 5)],
    ///     vec![
    ///         (0.0, 0.0),  // 0
    ///         (1.0, 0.0),  // 1
    ///         (1.0, 1.0),  // 2
    ///         (0.0, 1.0),  // 3
    ///         (-1.0, 1.0), // 4
    ///         (-1.0, 0.0), // 5
    ///     ],
    /// )
    /// .unwrap();
    ///
    /// let key = graph.faces().nth(0).unwrap().key();
    /// let face = graph
    ///     .face_mut(key)
    ///     .unwrap()
    ///     .merge(ByIndex(0))
    ///     .unwrap()
    ///     .into_ref();
    /// # }
    /// ```
    pub fn merge(self, destination: Selector<FaceKey>) -> Result<Self, GraphError> {
        let destination = destination.key_or_else(|index| {
            self.neighboring_faces()
                .nth(index)
                .ok_or_else(|| GraphError::TopologyNotFound)
                .map(|face| face.key())
        })?;
        let ab = self
            .interior_arcs()
            .find(|arc| match arc.opposite_arc().face() {
                Some(face) => face.key() == destination,
                _ => false,
            })
            .map(|arc| arc.key())
            .ok_or_else(|| GraphError::TopologyNotFound)?;
        let geometry = self.geometry.clone();
        // TODO: Batch this operation by using the mutation API instead.
        let (_, storage) = self.into_inner().into_keyed_source();
        Ok(ArcView::from_keyed_source((ab, storage))
            .expect_consistent()
            .remove()
            // Removing an edge between faces must yield a vertex.
            .expect_consistent()
            .into_outgoing_arc()
            .into_interior_path()
            .get_or_insert_face_with(|| geometry))
    }

    /// Connects faces with equal arity with faces inserted along their
    /// perimeters.
    ///
    /// The inserted faces are always quadrilateral. Both the initiating face
    /// and destination face are removed.
    ///
    /// # Errors
    ///
    /// Returns an error if the destination face cannot be found or the arity
    /// of the face and its destination are not the same.
    pub fn bridge(self, destination: FaceKey) -> Result<(), GraphError> {
        let (source, storage) = self.into_inner().into_keyed_source();
        // Errors can easily be caused by inputs to this function. Allow errors
        // from the snapshot to propagate.
        let cache = FaceBridgeCache::snapshot(&storage, source, destination)?;
        Ok(Mutation::replace(storage, Default::default())
            .commit_with(move |mutation| face::bridge_with_cache(mutation, cache))
            .map(|_| ())
            .expect_consistent())
    }

    /// Decomposes the face into triangles. Does nothing if the face is
    /// triangular.
    ///
    /// Returns the terminating face of the decomposition.
    pub fn triangulate(self) -> Self {
        let mut face = self;
        while face.arity() > 3 {
            face = face
                .split(ByIndex(0), ByIndex(2))
                .expect_consistent()
                .into_face()
                .expect_consistent();
        }
        face
    }

    /// Subdivides the face about a vertex. A triangle fan is formed from each
    /// arc in the face's perimeter and the vertex.
    ///
    /// Poking inserts a new vertex with geometry provided by the given
    /// function.
    ///
    /// Returns the inserted vertex.
    ///
    /// # Examples
    ///
    /// Forming a pyramid from a triangular face:
    ///
    /// ```rust
    /// # extern crate nalgebra;
    /// # extern crate plexus;
    /// #
    /// use nalgebra::Point3;
    /// use plexus::graph::MeshGraph;
    /// use plexus::prelude::*;
    /// use plexus::primitive::Triangle;
    /// use plexus::AsPosition;
    ///
    /// # fn main() {
    /// let mut graph = MeshGraph::<Point3<f64>>::from_raw_buffers(
    ///     vec![Triangle::new(0usize, 1, 2)],
    ///     vec![(-1.0, 0.0, 0.0), (1.0, 0.0, 0.0), (0.0, 2.0, 0.0)],
    /// )
    /// .unwrap();
    /// let key = graph.faces().nth(0).unwrap().key();
    /// let mut face = graph.face_mut(key).unwrap();
    ///
    /// // See `poke_with_offset`, which provides this functionality.
    /// let mut geometry = face.centroid();
    /// let position = geometry.as_position().clone() + face.normal();
    /// face.poke_with(move || {
    ///     *geometry.as_position_mut() = position;
    ///     geometry
    /// });
    /// # }
    /// ```
    pub fn poke_with<F>(self, f: F) -> VertexView<&'a mut M, G>
    where
        F: FnOnce() -> G::Vertex,
    {
        let (abc, storage) = self.into_inner().into_keyed_source();
        let cache = FacePokeCache::snapshot(&storage, abc, f()).expect_consistent();
        Mutation::replace(storage, Default::default())
            .commit_with(move |mutation| face::poke_with_cache(mutation, cache))
            .map(|(storage, vertex)| (vertex, storage).into_view().expect_consistent())
            .expect_consistent()
    }

    /// Subdivides the face about its centroid. A triangle fan is formed from
    /// each arc in the face's perimeter and a vertex inserted at the centroid.
    ///
    /// Returns the inserted vertex.
    pub fn poke_at_centroid(self) -> VertexView<&'a mut M, G>
    where
        G: FaceCentroid<Centroid = VertexPosition<G>>,
        G::Vertex: AsPosition,
    {
        let mut geometry = self.arc().source_vertex().geometry.clone();
        let centroid = self.centroid();
        self.poke_with(move || {
            *geometry.as_position_mut() = centroid;
            geometry
        })
    }

    /// Subdivides the face about its centroid. A triangle fan is formed from
    /// each arc in the face's perimeter and a vertex inserted at the centroid.
    /// The inserted vertex is then translated along the initiating face's
    /// normal by the given offset.
    ///
    /// Returns the inserted vertex.
    ///
    /// # Examples
    ///
    /// Constructing a "spikey" sphere:
    ///
    /// ```rust
    /// # extern crate decorum;
    /// # extern crate nalgebra;
    /// # extern crate plexus;
    /// #
    /// use decorum::N64;
    /// use nalgebra::Point3;
    /// use plexus::graph::MeshGraph;
    /// use plexus::prelude::*;
    /// use plexus::primitive::sphere::UvSphere;
    ///
    /// # fn main() {
    /// let mut graph = UvSphere::new(16, 8)
    ///     .polygons_with_position::<Point3<N64>>()
    ///     .collect::<MeshGraph<Point3<f64>>>();
    /// let keys = graph.faces().map(|face| face.key()).collect::<Vec<_>>();
    /// for key in keys {
    ///     graph.face_mut(key).unwrap().poke_with_offset(0.5);
    /// }
    /// # }
    /// ```
    pub fn poke_with_offset<T>(self, offset: T) -> VertexView<&'a mut M, G>
    where
        T: Into<Scalar<VertexPosition<G>>>,
        G: FaceCentroid<Centroid = VertexPosition<G>>
            + FaceNormal<Normal = Vector<VertexPosition<G>>>,
        G::Vertex: AsPosition,
        VertexPosition<G>: EuclideanSpace,
    {
        let mut geometry = self.arc().source_vertex().geometry.clone();
        let position = self.centroid() + (self.normal() * offset.into());
        self.poke_with(move || {
            *geometry.as_position_mut() = position;
            geometry
        })
    }

    pub fn extrude<T>(self, offset: T) -> FaceView<&'a mut M, G>
    where
        T: Into<Scalar<VertexPosition<G>>>,
        G: FaceNormal<Normal = Vector<VertexPosition<G>>>,
        G::Vertex: AsPosition,
        VertexPosition<G>: EuclideanSpace,
    {
        let translation = self.normal() * offset.into();
        let (abc, storage) = self.into_inner().into_keyed_source();
        let cache = FaceExtrudeCache::snapshot(&storage, abc, translation).expect_consistent();
        Mutation::replace(storage, Default::default())
            .commit_with(move |mutation| face::extrude_with_cache(mutation, cache))
            .map(|(storage, face)| (face, storage).into_view().expect_consistent())
            .expect_consistent()
    }

    /// Removes the face.
    ///
    /// Returns the interior path of the face.
    pub fn remove(self) -> InteriorPathView<&'a mut M, G> {
        let (abc, storage) = self.into_inner().into_keyed_source();
        let cache = FaceRemoveCache::snapshot(&storage, abc).expect_consistent();
        Mutation::replace(storage, Default::default())
            .commit_with(move |mutation| face::remove_with_cache(mutation, cache))
            .map(|(storage, face)| (face.arc, storage).into_view().expect_consistent())
            .expect_consistent()
    }
}

impl<M, G> Clone for FaceView<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<FacePayload<G>>,
    G: GraphGeometry,
    View<M, FacePayload<G>>: Clone,
{
    fn clone(&self) -> Self {
        FaceView {
            inner: self.inner.clone(),
        }
    }
}

impl<M, G> Copy for FaceView<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<FacePayload<G>>,
    G: GraphGeometry,
    View<M, FacePayload<G>>: Copy,
{
}

impl<M, G> Deref for FaceView<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<FacePayload<G>>,
    G: GraphGeometry,
{
    type Target = FacePayload<G>;

    fn deref(&self) -> &Self::Target {
        self.inner.deref()
    }
}

impl<M, G> DerefMut for FaceView<M, G>
where
    M: Reborrow + ReborrowMut,
    M::Target: AsStorage<FacePayload<G>> + AsStorageMut<FacePayload<G>>,
    G: GraphGeometry,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner.deref_mut()
    }
}

impl<M, G> From<View<M, FacePayload<G>>> for FaceView<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<FacePayload<G>>,
    G: GraphGeometry,
{
    fn from(view: View<M, FacePayload<G>>) -> Self {
        FaceView { inner: view }
    }
}

impl<M, G> FromKeyedSource<(FaceKey, M)> for FaceView<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<FacePayload<G>>,
    G: GraphGeometry,
{
    fn from_keyed_source(source: (FaceKey, M)) -> Option<Self> {
        View::<_, FacePayload<_>>::from_keyed_source(source).map(|view| view.into())
    }
}

impl<M, G> InteriorPath<M, G> for FaceView<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<ArcPayload<G>> + AsStorage<FacePayload<G>>,
    G: GraphGeometry,
{
    fn reachable_arcs(&self) -> ArcCirculator<&M::Target, G> {
        ArcCirculator::from(self.interior_reborrow())
    }
}

/// Orphan view of a face.
///
/// Provides mutable access to a face's geometry. See the module documentation
/// for more information about topological views.
pub struct OrphanFaceView<'a, G>
where
    G: 'a + GraphGeometry,
{
    inner: OrphanView<'a, FacePayload<G>>,
}

impl<'a, G> OrphanFaceView<'a, G>
where
    G: 'a + GraphGeometry,
{
    pub fn key(&self) -> FaceKey {
        self.inner.key()
    }
}

impl<'a, G> Deref for OrphanFaceView<'a, G>
where
    G: 'a + GraphGeometry,
{
    type Target = FacePayload<G>;

    fn deref(&self) -> &Self::Target {
        self.inner.deref()
    }
}

impl<'a, G> DerefMut for OrphanFaceView<'a, G>
where
    G: 'a + GraphGeometry,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner.deref_mut()
    }
}

impl<'a, G> From<OrphanView<'a, FacePayload<G>>> for OrphanFaceView<'a, G>
where
    G: 'a + GraphGeometry,
{
    fn from(view: OrphanView<'a, FacePayload<G>>) -> Self {
        OrphanFaceView { inner: view }
    }
}

impl<'a, M, G> FromKeyedSource<(FaceKey, &'a mut M)> for OrphanFaceView<'a, G>
where
    M: AsStorage<FacePayload<G>> + AsStorageMut<FacePayload<G>>,
    G: 'a + GraphGeometry,
{
    fn from_keyed_source(source: (FaceKey, &'a mut M)) -> Option<Self> {
        OrphanView::<FacePayload<_>>::from_keyed_source(source).map(|view| view.into())
    }
}

/// View of an interior path.
///
/// Interior paths are closed paths formed by arcs and their immediate
/// neighboring arcs. In a consistent graph, every arc forms such a path. Such
/// paths may or may not be occupied by faces.
///
/// Interior paths have no associated payload and do not directly expose
/// geometry (`InteriorPathView` does not implement `Deref`).
///
/// An interior path with a perimeter formed by vertices $A$, $B$, and $C$ is
/// notated $\overrightarrow{\\{A, B, C\\}}$.
///
/// See the module documentation for more information about topological views.
pub struct InteriorPathView<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<ArcPayload<G>> + Consistent,
    G: GraphGeometry,
{
    inner: View<M, ArcPayload<G>>,
}

impl<M, G> InteriorPathView<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<ArcPayload<G>> + Consistent,
    G: GraphGeometry,
{
    fn into_inner(self) -> View<M, ArcPayload<G>> {
        let InteriorPathView { inner, .. } = self;
        inner
    }

    /// Gets the arity of the interior path. This is the number of arcs that
    /// form the path.
    pub fn arity(&self) -> usize {
        <Self as InteriorPath<_, _>>::arity(self)
    }

    /// Gets an iterator of views over the arcs within the interior path.
    pub fn arcs(&self) -> impl Clone + Iterator<Item = ArcView<&M::Target, G>> {
        <Self as InteriorPath<_, _>>::arcs(self)
    }

    fn interior_reborrow(&self) -> InteriorPathView<&M::Target, G> {
        self.inner.interior_reborrow().into()
    }
}

impl<'a, M, G> InteriorPathView<&'a mut M, G>
where
    M: AsStorage<ArcPayload<G>> + AsStorageMut<ArcPayload<G>> + Consistent,
    G: 'a + GraphGeometry,
{
    /// Converts a mutable view into an immutable view.
    ///
    /// This is useful when mutations are not (or no longer) needed and mutual
    /// access is desired.
    pub fn into_ref(self) -> InteriorPathView<&'a M, G> {
        self.into_inner().into_ref().into()
    }

    /// Reborrows the view and constructs another mutable view from a given
    /// key.
    ///
    /// This allows for fallible traversals from a mutable view without the
    /// need for direct access to the source `MeshGraph`. If the given function
    /// emits a key, then that key will be used to convert this view into
    /// another. If no key is emitted, then the original mutable view is
    /// returned.
    pub fn with_ref<T, K, F>(self, f: F) -> Either<Result<T, GraphError>, Self>
    where
        T: FromKeyedSource<(K, &'a mut M)>,
        F: FnOnce(InteriorPathView<&M, G>) -> Option<K>,
    {
        if let Some(key) = f(self.interior_reborrow()) {
            let (_, storage) = self.into_inner().into_keyed_source();
            Either::Left(
                T::from_keyed_source((key, storage)).ok_or_else(|| GraphError::TopologyNotFound),
            )
        }
        else {
            Either::Right(self)
        }
    }
}

impl<M, G> InteriorPathView<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<ArcPayload<G>> + AsStorage<VertexPayload<G>> + Consistent,
    G: GraphGeometry,
{
    /// Converts the interior path into its originating arc.
    pub fn into_arc(self) -> ArcView<M, G> {
        self.into_inner().into()
    }

    /// Gets the originating arc of the interior path.
    pub fn arc(&self) -> ArcView<&M::Target, G> {
        self.inner.interior_reborrow().into()
    }

    /// Gets the distance (number of arcs) between two vertices within the
    /// interior path.
    pub fn distance(
        &self,
        source: Selector<VertexKey>,
        destination: Selector<VertexKey>,
    ) -> Result<usize, GraphError> {
        <Self as InteriorPath<_, _>>::distance(self, source, destination)
    }

    /// Gets an iterator of views over the vertices within the interior path.
    pub fn vertices(&self) -> impl Clone + Iterator<Item = VertexView<&M::Target, G>> {
        <Self as InteriorPath<_, _>>::vertices(self)
    }
}

impl<M, G> InteriorPathView<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<ArcPayload<G>> + AsStorage<FacePayload<G>> + Consistent,
    G: GraphGeometry,
{
    /// Converts the interior path into its face.
    ///
    /// If the path has no associated face, then `None` is returned.
    pub fn into_face(self) -> Option<FaceView<M, G>> {
        let inner = self.into_inner();
        let key = inner.face;
        key.map(move |key| inner.rekey_map(key).expect_consistent())
    }

    /// Gets the face of the interior path.
    ///
    /// If the path has no associated face, then `None` is returned.
    pub fn face(&self) -> Option<FaceView<&M::Target, G>> {
        let key = self.inner.face;
        key.map(|key| {
            self.inner
                .interior_reborrow()
                .rekey_map(key)
                .expect_consistent()
        })
    }
}

impl<'a, M, G> InteriorPathView<&'a mut M, G>
where
    M: AsStorage<VertexPayload<G>>
        + AsStorage<ArcPayload<G>>
        + AsStorage<FacePayload<G>>
        + Default
        + Mutable<G>,
    G: 'a + GraphGeometry,
{
    /// Gets the face of the interior path or inserts a face if one does not
    /// already exist.
    ///
    /// Returns the inserted face.
    pub fn get_or_insert_face(self) -> FaceView<&'a mut M, G> {
        self.get_or_insert_face_with(|| Default::default())
    }

    /// Gets the face of the interior path or inserts a face if one does not
    /// already exist.
    ///
    /// If a face is inserted, then the given function is used to get the
    /// geometry for the face.
    ///
    /// Returns the inserted face.
    pub fn get_or_insert_face_with<F>(self, f: F) -> FaceView<&'a mut M, G>
    where
        F: FnOnce() -> G::Face,
    {
        let key = self.inner.face;
        if let Some(key) = key {
            self.into_inner().rekey_map(key).expect_consistent()
        }
        else {
            let vertices = self
                .vertices()
                .map(|vertex| vertex.key())
                .collect::<Vec<_>>();
            let (_, storage) = self.into_inner().into_keyed_source();
            let cache = FaceInsertCache::snapshot(&storage, &vertices, (Default::default(), f()))
                .expect_consistent();
            Mutation::replace(storage, Default::default())
                .commit_with(move |mutation| mutation.insert_face_with_cache(cache))
                .map(|(storage, face)| (face, storage).into_view().expect_consistent())
                .expect_consistent()
        }
    }
}

impl<M, G> From<View<M, ArcPayload<G>>> for InteriorPathView<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<ArcPayload<G>> + Consistent,
    G: GraphGeometry,
{
    fn from(view: View<M, ArcPayload<G>>) -> Self {
        InteriorPathView { inner: view }
    }
}

impl<M, G> FromKeyedSource<(ArcKey, M)> for InteriorPathView<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<ArcPayload<G>> + Consistent,
    G: GraphGeometry,
{
    fn from_keyed_source(source: (ArcKey, M)) -> Option<Self> {
        View::<_, ArcPayload<_>>::from_keyed_source(source).map(|view| view.into())
    }
}

impl<M, G> InteriorPath<M, G> for InteriorPathView<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<ArcPayload<G>> + Consistent,
    G: GraphGeometry,
{
    fn reachable_arcs(&self) -> ArcCirculator<&M::Target, G> {
        ArcCirculator::from(self.interior_reborrow())
    }
}

pub struct VertexCirculator<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<ArcPayload<G>>,
    G: GraphGeometry,
{
    input: ArcCirculator<M, G>,
}

impl<M, G> VertexCirculator<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<ArcPayload<G>>,
    G: GraphGeometry,
{
    fn next(&mut self) -> Option<VertexKey> {
        let ab = self.input.next();
        ab.map(|ab| {
            let (_, b) = ab.into();
            b
        })
    }
}

impl<M, G> Clone for VertexCirculator<M, G>
where
    M: Clone + Reborrow,
    M::Target: AsStorage<ArcPayload<G>>,
    G: GraphGeometry,
{
    fn clone(&self) -> Self {
        VertexCirculator {
            input: self.input.clone(),
        }
    }
}

impl<M, G> From<ArcCirculator<M, G>> for VertexCirculator<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<ArcPayload<G>>,
    G: GraphGeometry,
{
    fn from(input: ArcCirculator<M, G>) -> Self {
        VertexCirculator { input }
    }
}

// TODO: This iterator could provide a size hint of `(3, None)`, but this is
//       only the case when the underlying mesh is consistent.
impl<'a, M, G> Iterator for VertexCirculator<&'a M, G>
where
    M: 'a + AsStorage<ArcPayload<G>> + AsStorage<VertexPayload<G>>,
    G: 'a + GraphGeometry,
{
    type Item = VertexView<&'a M, G>;

    fn next(&mut self) -> Option<Self::Item> {
        VertexCirculator::next(self).and_then(|key| (key, self.input.storage).into_view())
    }
}

// TODO: This iterator could provide a size hint of `(3, None)`, but this is
//       only the case when the underlying mesh is consistent.
impl<'a, M, G> Iterator for VertexCirculator<&'a mut M, G>
where
    M: 'a + AsStorage<ArcPayload<G>> + AsStorage<VertexPayload<G>> + AsStorageMut<VertexPayload<G>>,
    G: 'a + GraphGeometry,
{
    type Item = OrphanVertexView<'a, G>;

    fn next(&mut self) -> Option<Self::Item> {
        VertexCirculator::next(self).and_then(|key| {
            (key, unsafe {
                mem::transmute::<
                    &'_ mut StorageProxy<VertexPayload<G>>,
                    &'a mut StorageProxy<VertexPayload<G>>,
                >(self.input.storage.as_storage_mut())
            })
                .into_view()
        })
    }
}

pub struct ArcCirculator<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<ArcPayload<G>>,
    G: GraphGeometry,
{
    storage: M,
    arc: Option<ArcKey>,
    breadcrumb: Option<ArcKey>,
    phantom: PhantomData<G>,
}

impl<M, G> ArcCirculator<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<ArcPayload<G>>,
    G: GraphGeometry,
{
    fn next(&mut self) -> Option<ArcKey> {
        self.arc.and_then(|arc| {
            let next = self
                .storage
                .reborrow()
                .as_storage()
                .get(&arc)
                .and_then(|arc| arc.next);
            self.breadcrumb.map(|_| {
                if self.breadcrumb == next {
                    self.breadcrumb = None;
                }
                else {
                    self.arc = next;
                }
                arc
            })
        })
    }
}

impl<M, G> Clone for ArcCirculator<M, G>
where
    M: Clone + Reborrow,
    M::Target: AsStorage<ArcPayload<G>>,
    G: GraphGeometry,
{
    fn clone(&self) -> Self {
        ArcCirculator {
            storage: self.storage.clone(),
            arc: self.arc,
            breadcrumb: self.breadcrumb,
            phantom: PhantomData,
        }
    }
}

impl<M, G> From<FaceView<M, G>> for ArcCirculator<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<ArcPayload<G>> + AsStorage<FacePayload<G>>,
    G: GraphGeometry,
{
    fn from(face: FaceView<M, G>) -> Self {
        let key = face.arc;
        let (_, storage) = face.into_inner().into_keyed_source();
        ArcCirculator {
            storage,
            arc: Some(key),
            breadcrumb: Some(key),
            phantom: PhantomData,
        }
    }
}

impl<M, G> From<InteriorPathView<M, G>> for ArcCirculator<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<ArcPayload<G>> + Consistent,
    G: GraphGeometry,
{
    fn from(path: InteriorPathView<M, G>) -> Self {
        let (key, storage) = path.into_inner().into_keyed_source();
        ArcCirculator {
            storage,
            arc: Some(key),
            breadcrumb: Some(key),
            phantom: PhantomData,
        }
    }
}

// TODO: This iterator could provide a size hint of `(3, None)`, but this is
//       only the case when the underlying mesh is consistent.
impl<'a, M, G> Iterator for ArcCirculator<&'a M, G>
where
    M: 'a + AsStorage<ArcPayload<G>>,
    G: 'a + GraphGeometry,
{
    type Item = ArcView<&'a M, G>;

    fn next(&mut self) -> Option<Self::Item> {
        ArcCirculator::next(self).and_then(|key| (key, self.storage).into_view())
    }
}

// TODO: This iterator could provide a size hint of `(3, None)`, but this is
//       only the case when the underlying mesh is consistent.
impl<'a, M, G> Iterator for ArcCirculator<&'a mut M, G>
where
    M: 'a + AsStorage<ArcPayload<G>> + AsStorageMut<ArcPayload<G>>,
    G: 'a + GraphGeometry,
{
    type Item = OrphanArcView<'a, G>;

    fn next(&mut self) -> Option<Self::Item> {
        ArcCirculator::next(self).and_then(|key| {
            (key, unsafe {
                mem::transmute::<
                    &'_ mut StorageProxy<ArcPayload<G>>,
                    &'a mut StorageProxy<ArcPayload<G>>,
                >(self.storage.as_storage_mut())
            })
                .into_view()
        })
    }
}

pub struct FaceCirculator<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<ArcPayload<G>>,
    G: GraphGeometry,
{
    input: ArcCirculator<M, G>,
}

impl<M, G> FaceCirculator<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<ArcPayload<G>>,
    G: GraphGeometry,
{
    fn next(&mut self) -> Option<FaceKey> {
        while let Some(ba) = self.input.next().map(|ab| ab.opposite()) {
            if let Some(abc) = self
                .input
                .storage
                .reborrow()
                .as_storage()
                .get(&ba)
                .and_then(|opposite| opposite.face)
            {
                return Some(abc);
            }
            else {
                // Skip arcs with no opposing face. This can occur within
                // non-enclosed meshes.
                continue;
            }
        }
        None
    }
}

impl<M, G> Clone for FaceCirculator<M, G>
where
    M: Clone + Reborrow,
    M::Target: AsStorage<ArcPayload<G>>,
    G: GraphGeometry,
{
    fn clone(&self) -> Self {
        FaceCirculator {
            input: self.input.clone(),
        }
    }
}

impl<M, G> From<ArcCirculator<M, G>> for FaceCirculator<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<ArcPayload<G>>,
    G: GraphGeometry,
{
    fn from(input: ArcCirculator<M, G>) -> Self {
        FaceCirculator { input }
    }
}

impl<'a, M, G> Iterator for FaceCirculator<&'a M, G>
where
    M: 'a + AsStorage<ArcPayload<G>> + AsStorage<FacePayload<G>>,
    G: 'a + GraphGeometry,
{
    type Item = FaceView<&'a M, G>;

    fn next(&mut self) -> Option<Self::Item> {
        FaceCirculator::next(self).and_then(|key| (key, self.input.storage).into_view())
    }
}

impl<'a, M, G> Iterator for FaceCirculator<&'a mut M, G>
where
    M: 'a + AsStorage<ArcPayload<G>> + AsStorage<FacePayload<G>> + AsStorageMut<FacePayload<G>>,
    G: 'a + GraphGeometry,
{
    type Item = OrphanFaceView<'a, G>;

    fn next(&mut self) -> Option<Self::Item> {
        FaceCirculator::next(self).and_then(|key| {
            (key, unsafe {
                mem::transmute::<
                    &'_ mut StorageProxy<FacePayload<G>>,
                    &'a mut StorageProxy<FacePayload<G>>,
                >(self.input.storage.as_storage_mut())
            })
                .into_view()
        })
    }
}

#[cfg(test)]
mod tests {
    use decorum::N64;
    use nalgebra::{Point2, Point3};

    use crate::graph::MeshGraph;
    use crate::index::{HashIndexer, Structured4};
    use crate::prelude::*;
    use crate::primitive::cube::Cube;
    use crate::primitive::sphere::UvSphere;

    type E3 = Point3<N64>;

    #[test]
    fn circulate_over_arcs() {
        let graph = UvSphere::new(3, 2)
            .polygons_with_position::<E3>() // 6 triangles, 18 vertices.
            .collect::<MeshGraph<Point3<f64>>>();
        let face = graph.faces().nth(0).unwrap();

        // All faces should be triangles and should have three edges.
        assert_eq!(3, face.interior_arcs().count());
    }

    #[test]
    fn circulate_over_faces() {
        let graph = UvSphere::new(3, 2)
            .polygons_with_position::<E3>() // 6 triangles, 18 vertices.
            .collect::<MeshGraph<Point3<f64>>>();
        let face = graph.faces().nth(0).unwrap();

        // No matter which face is selected, it should have three neighbors.
        assert_eq!(3, face.neighboring_faces().count());
    }

    #[test]
    fn remove_face() {
        let mut graph = UvSphere::new(3, 2)
            .polygons_with_position::<E3>() // 6 triangles, 18 vertices.
            .collect::<MeshGraph<Point3<f64>>>();

        // The graph should begin with 6 faces.
        assert_eq!(6, graph.face_count());

        // Remove a face from the graph.
        let abc = graph.faces().nth(0).unwrap().key();
        {
            let face = graph.face_mut(abc).unwrap();
            assert_eq!(3, face.arity()); // The face should be triangular.

            let path = face.remove().into_ref();
            assert_eq!(3, path.arity()); // The path should also be triangular.
        }

        // After the removal, the graph should have only 5 faces.
        assert_eq!(5, graph.face_count());
    }

    #[test]
    fn split_face() {
        let mut graph = MeshGraph::<Point2<f32>>::from_raw_buffers_with_arity(
            vec![0u32, 1, 2, 3],
            vec![(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)],
            4,
        )
        .unwrap();
        let abc = graph.faces().nth(0).unwrap().key();
        let arc = graph
            .face_mut(abc)
            .unwrap()
            .split(ByIndex(0), ByIndex(2))
            .unwrap()
            .into_ref();

        assert!(arc.face().is_some());
        assert!(arc.opposite_arc().face().is_some());
        assert_eq!(4, graph.vertex_count());
        assert_eq!(10, graph.arc_count());
        assert_eq!(2, graph.face_count());
    }

    #[test]
    fn extrude_face() {
        let mut graph = UvSphere::new(3, 2)
            .polygons_with_position::<E3>() // 6 triangles, 18 vertices.
            .collect::<MeshGraph<Point3<f64>>>();
        {
            let key = graph.faces().nth(0).unwrap().key();
            let face = graph.face_mut(key).unwrap().extrude(1.0).into_ref();

            // The extruded face, being a triangle, should have three
            // neighboring faces.
            assert_eq!(3, face.neighboring_faces().count());
        }

        assert_eq!(8, graph.vertex_count());
        // The mesh begins with 18 arcs. The extrusion adds three quads
        // with four interior arcs each, so there are `18 + (3 * 4)`
        // arcs.
        assert_eq!(30, graph.arc_count());
        // All faces are triangles and the mesh begins with six such faces. The
        // extruded face remains, in addition to three connective faces, each
        // of which is constructed from quads.
        assert_eq!(9, graph.face_count());
    }

    #[test]
    fn merge_faces() {
        // Construct a graph with two connected quads.
        let mut graph = MeshGraph::<Point2<f32>>::from_raw_buffers_with_arity(
            vec![0u32, 1, 2, 3, 0, 3, 4, 5],
            vec![
                (0.0, 0.0),  // 0
                (1.0, 0.0),  // 1
                (1.0, 1.0),  // 2
                (0.0, 1.0),  // 3
                (-1.0, 1.0), // 4
                (-1.0, 0.0), // 5
            ],
            4,
        )
        .unwrap();

        // The graph should begin with 2 faces.
        assert_eq!(2, graph.face_count());

        // Get the keys for the two faces and join them.
        let abc = graph.faces().nth(0).unwrap().key();
        let def = graph.faces().nth(1).unwrap().key();
        graph.face_mut(abc).unwrap().merge(ByKey(def)).unwrap();

        // After the removal, the graph should have 1 face.
        assert_eq!(1, graph.face_count());
        assert_eq!(6, graph.faces().nth(0).unwrap().arity());
    }

    #[test]
    fn poke_face() {
        let mut graph = Cube::new()
            .polygons_with_position::<E3>() // 6 quads, 24 vertices.
            .collect::<MeshGraph<Point3<f64>>>();
        let key = graph.faces().nth(0).unwrap().key();
        let vertex = graph.face_mut(key).unwrap().poke_at_centroid();

        // Diverging a quad yields a tetrahedron.
        assert_eq!(4, vertex.neighboring_faces().count());

        // Traverse to one of the triangles in the tetrahedron.
        let face = vertex.into_outgoing_arc().into_face().unwrap();

        assert_eq!(3, face.arity());

        // Diverge the triangle.
        let vertex = face.poke_at_centroid();

        assert_eq!(3, vertex.neighboring_faces().count());
    }

    #[test]
    fn triangulate_mesh() {
        let (indices, vertices) = Cube::new()
            .polygons_with_position::<E3>() // 6 quads, 24 vertices.
            .index_vertices::<Structured4, _>(HashIndexer::default());
        let mut graph = MeshGraph::<Point3<N64>>::from_raw_buffers(indices, vertices).unwrap();
        graph.triangulate();

        assert_eq!(8, graph.vertex_count());
        assert_eq!(36, graph.arc_count());
        assert_eq!(18, graph.edge_count());
        // Each quad becomes 2 triangles, so 6 quads become 12 triangles.
        assert_eq!(12, graph.face_count());
    }

    #[test]
    fn interior_path_distance() {
        let graph = MeshGraph::<Point2<f32>>::from_raw_buffers_with_arity(
            vec![0u32, 1, 2, 3],
            vec![(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)],
            4,
        )
        .unwrap();
        let face = graph.faces().nth(0).unwrap();
        let keys = face
            .vertices()
            .map(|vertex| vertex.key())
            .collect::<Vec<_>>();
        let path = face.into_interior_path();
        assert_eq!(2, path.distance(keys[0].into(), keys[2].into()).unwrap());
        assert_eq!(1, path.distance(keys[0].into(), keys[3].into()).unwrap());
        assert_eq!(0, path.distance(keys[0].into(), keys[0].into()).unwrap());
    }
}