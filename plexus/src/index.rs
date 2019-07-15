//! Indexing and aggregation.
//!
//! This module provides _indexers_, which disambiguate geometry and produce
//! the minimal set of topological and geometric data. This is primarily used
//! to index streams of $n$-gons (`NGon`, `Triangle`, etc.) into raw buffers or
//! polygonal mesh data structures like `MeshBuffer`.
//!
//! The primary interface of this module are the `IndexVertices` and
//! `CollectWithIndexer` traits along with the `HashIndexer` and `LruIndexer`
//! types.
//!
//! Indexing produces an _index buffer_, which describes the topology of mesh
//! data by providing groups of indices into geometric data (a _vertex
//! buffer_). Plexus supports both _structured_ (_polygonal_) and
//! _unstructured_ (_flat_) index buffers, which group indices explicitly and
//! implicitly, respectively. See the grouping types `Flat` and `Structured`.
//!
//! Flat index buffers directly store individual indices, such as `Vec<usize>`.
//! Because there is no explicit structure, arity must by constant, but
//! arbitrarily sized $n$-gons are trivially supported. Flat index buffers tend
//! to be more useful for rendering, especially triangular buffers, because
//! rendering pipelines typically expect a simple contiguous buffer of index
//! data. See `MeshBuffer3`.
//!
//! Structured index buffers contain sub-structures that group their indices,
//! such as `Vec<Triangle<usize>>`. Structured index buffers typically contain
//! `Triangle`s, `Quad`s, or `Polygon`s, all of which preserve the topology of
//! a mesh even if its arity is non-constant.
//!
//! # Examples
//!
//! Indexing data for a cube to create raw buffers and a `MeshBuffer`:
//!
//! ```rust
//! # extern crate decorum;
//! # extern crate nalgebra;
//! # extern crate plexus;
//! #
//! use decorum::N64;
//! use nalgebra::Point3;
//! use plexus::buffer::MeshBuffer3;
//! use plexus::index::{Flat3, HashIndexer};
//! use plexus::prelude::*;
//! use plexus::primitive::cube::Cube;
//!
//! # fn main() {
//! let (indices, positions) = Cube::new()
//!     .polygons_with_position::<Point3<N64>>()
//!     .triangulate()
//!     .index_vertices::<Flat3, _>(HashIndexer::default());
//! let buffer = MeshBuffer3::<u32, _>::from_raw_buffers(indices, positions).unwrap();
//! # }
//! ```

use itertools::{Itertools, MinMaxResult};
use num::{Integer, NumCast, Unsigned};
use std::cmp;
use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::Hash;
use std::marker::PhantomData;
use theon::ops::Map;
use typenum::{NonZero, U3, U4};

use crate::primitive::decompose::IntoVertices;
use crate::primitive::{ConstantArity, Polygon, Quad, Topological, Triangle};
use crate::Arity;

pub trait Grouping {
    type Item;
}

/// Index buffer.
pub trait IndexBuffer<R>
where
    R: Grouping,
{
    type Index: Copy + Integer + NumCast + Unsigned;

    fn arity(&self) -> Arity;
}

impl<A, N> IndexBuffer<Flat<A, N>> for Vec<N>
where
    A: NonZero + typenum::Unsigned,
    N: Copy + Integer + NumCast + Unsigned,
{
    type Index = N;

    fn arity(&self) -> Arity {
        Arity::Uniform(A::USIZE)
    }
}

impl<N> IndexBuffer<Structured<Polygon<N>>> for Vec<Polygon<N>>
where
    N: Copy + Integer + NumCast + Unsigned,
{
    type Index = <Polygon<N> as Topological>::Vertex;

    fn arity(&self) -> Arity {
        match self.iter().map(|polygon| polygon.arity()).minmax() {
            MinMaxResult::OneElement(arity) => Arity::Uniform(arity),
            MinMaxResult::MinMax(min, max) => Arity::NonUniform(min, max),
            _ => Arity::Uniform(0),
        }
    }
}

impl<P> IndexBuffer<Structured<P>> for Vec<P>
where
    P: ConstantArity + Topological,
    P::Vertex: Copy + Integer + NumCast + Unsigned,
{
    type Index = P::Vertex;

    fn arity(&self) -> Arity {
        Arity::Uniform(P::ARITY)
    }
}

pub trait Push<R, P>: IndexBuffer<R>
where
    R: Grouping,
    P: Topological<Vertex = Self::Index>,
    P::Vertex: Copy + Integer + NumCast + Unsigned,
{
    fn push(&mut self, index: P);
}

impl<A, N, P> Push<Flat<A, N>, P> for Vec<N>
where
    A: NonZero + typenum::Unsigned,
    N: Copy + Integer + NumCast + Unsigned,
    P: ConstantArity + IntoVertices + Topological<Vertex = N>,
{
    fn push(&mut self, index: P) {
        for index in index.into_vertices() {
            self.push(index);
        }
    }
}

impl<P> Push<Structured<P>, P> for Vec<P>
where
    P: Topological,
    P::Vertex: Copy + Integer + NumCast + Unsigned,
    Self: IndexBuffer<Structured<P>, Index = P::Vertex>,
{
    fn push(&mut self, index: P) {
        self.push(index);
    }
}

/// Flat index buffer grouping.
///
/// A flat (unstructured) index buffer with a constant arity. Arity is
/// specified using a type constant from the
/// [typenum](https://crates.io/crates/typenum) crate.
///
/// # Examples
///
/// Creating a `MeshBuffer` with a flat and triangular index buffer:
///
/// ```rust
/// use plexus::buffer::MeshBuffer;
/// use plexus::index::Flat;
/// use plexus::prelude::*;
/// use plexus::U3;
///
/// let mut buffer = MeshBuffer::<Flat<U3, usize>, (f64, f64, f64)>::default();
/// ```
#[derive(Debug)]
pub struct Flat<A = U3, N = usize>
where
    A: NonZero + typenum::Unsigned,
    N: Copy + Integer + NumCast + Unsigned,
{
    phantom: PhantomData<(A, N)>,
}

impl<A, N> Grouping for Flat<A, N>
where
    A: NonZero + typenum::Unsigned,
    N: Copy + Integer + NumCast + Unsigned,
{
    /// Flat index buffers directly contain indices. These indices are
    /// implicitly grouped by the arity of the buffer.
    type Item = N;
}

/// Alias for a flat and triangular index buffer.
pub type Flat3<N = usize> = Flat<U3, N>;
/// Alias for a flat and quadrilateral index buffer.
pub type Flat4<N = usize> = Flat<U4, N>;

/// Structured index buffer grouping.
///
/// A structured index buffer of triangles, quads, etc. Useful if a buffer
/// representing a mesh comprised of both triangles and quads is needed
/// (triangulation is not required).
///
/// # Examples
///
/// Creating a `MeshBuffer` with a structured index buffer:
///
/// ```rust
/// use plexus::buffer::MeshBuffer;
/// use plexus::index::Structured;
/// use plexus::prelude::*;
/// use plexus::primitive::Polygon;
///
/// let mut buffer = MeshBuffer::<Structured<Polygon<usize>>, (f64, f64, f64)>::default();
/// ```
#[derive(Debug)]
pub struct Structured<P = Polygon<usize>>
where
    P: Topological,
    P::Vertex: Copy + Integer + NumCast + Unsigned,
{
    phantom: PhantomData<P>,
}

impl<P> Grouping for Structured<P>
where
    P: Topological,
    P::Vertex: Copy + Integer + NumCast + Unsigned,
{
    /// `Topological` index buffers contain $n$-gons that explicitly group
    /// their indices.
    type Item = P;
}

/// Alias for a structured and triangular index buffer.
pub type Structured3<N = usize> = Structured<Triangle<N>>;
/// Alias for a structured and quadrilateral index buffer.
pub type Structured4<N = usize> = Structured<Quad<N>>;
/// Alias for a structured index buffer with varying arity.
pub type StructuredN<N = usize> = Structured<Polygon<N>>;

/// Vertex indexer.
///
/// Disambiguates arbitrary vertex data and emits a one-to-one mapping of
/// indices to vertices.
pub trait Indexer<T, K>
where
    T: Topological,
{
    /// Indexes a vertex using a keying function.
    ///
    /// Returns a tuple containing the index and optionally vertex data. Vertex
    /// data is only returned if the data has not yet been indexed, otherwise
    /// `None` is returned.
    fn index<F>(&mut self, vertex: T::Vertex, f: F) -> (usize, Option<T::Vertex>)
    where
        F: Fn(&T::Vertex) -> &K;
}

/// Hashing vertex indexer.
///
/// This indexer hashes key data for vertices to form an index. This is fast,
/// reliable, and requires no configuration. Prefer this indexer when possible.
///
/// The vertex key data must be hashable (implement `Hash`). Most vertex data
/// includes floating-point values (i.e., `f32` or `f64`), which do not
/// implement `Hash`. To avoid problems with hashing, the
/// [decorum](https://crates.io/crates/decorum) crate can be used. The `Finite`
/// and `NotNan` types are particularly useful for this and will panic if
/// illegal values result from a computation.
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
/// use plexus::index::{Flat3, HashIndexer};
/// use plexus::prelude::*;
/// use plexus::primitive::cube::Cube;
///
/// # fn main() {
/// let (indices, positions) = Cube::new()
///     .polygons_with_position::<Point3<N64>>()
///     .triangulate()
///     .index_vertices::<Flat3, _>(HashIndexer::default());
/// # }
/// ```
pub struct HashIndexer<T, K>
where
    T: Topological,
    K: Clone + Eq + Hash,
{
    hash: HashMap<K, usize>,
    n: usize,
    phantom: PhantomData<T>,
}

impl<T, K> HashIndexer<T, K>
where
    T: Topological,
    K: Clone + Eq + Hash,
{
    /// Creates a new `HashIndexer`.
    pub fn new() -> Self {
        HashIndexer {
            hash: HashMap::new(),
            n: 0,
            phantom: PhantomData,
        }
    }
}

impl<T, K> Default for HashIndexer<T, K>
where
    T: Topological,
    K: Clone + Eq + Hash,
{
    fn default() -> Self {
        HashIndexer::new()
    }
}

impl<T, K> Indexer<T, K> for HashIndexer<T, K>
where
    T: Topological,
    K: Clone + Eq + Hash,
{
    fn index<F>(&mut self, input: T::Vertex, f: F) -> (usize, Option<T::Vertex>)
    where
        F: Fn(&T::Vertex) -> &K,
    {
        let mut vertex = None;
        let mut n = self.n;
        let index = self.hash.entry(f(&input).clone()).or_insert_with(|| {
            vertex = Some(input);
            let m = n;
            n += 1;
            m
        });
        self.n = n;
        (*index, vertex)
    }
}

/// LRU caching vertex indexer.
///
/// This indexer uses a _least recently used_ (LRU) cache to form an index. To
/// function correctly, an adequate cache capacity is necessary. If the
/// capacity is insufficient, then redundant vertex data may be emitted. See
/// `with_capacity`.
///
/// This indexer is useful if the vertex key data cannot be hashed (does not
/// implement `Hash`). If the key data can be hashed, prefer `HashIndexer`
/// instead.
///
/// # Examples
///
/// ```rust
/// # extern crate nalgebra;
/// # extern crate plexus;
/// #
/// use nalgebra::Point3;
/// use plexus::index::{Flat3, LruIndexer};
/// use plexus::prelude::*;
/// use plexus::primitive::sphere::UvSphere;
///
/// # fn main() {
/// let (indices, positions) = UvSphere::new(8, 8)
///     .polygons_with_position::<Point3<f64>>()
///     .triangulate()
///     .index_vertices::<Flat3, _>(LruIndexer::with_capacity(64));
/// # }
/// ```
pub struct LruIndexer<T, K>
where
    T: Topological,
    K: Clone + PartialEq,
{
    lru: Vec<(K, usize)>,
    capacity: usize,
    n: usize,
    phantom: PhantomData<T>,
}

impl<T, K> LruIndexer<T, K>
where
    T: Topological,
    K: Clone + PartialEq,
{
    /// Creates a new `LruIndexer` with a default capacity.
    pub fn new() -> Self {
        LruIndexer::with_capacity(16)
    }

    /// Creates a new `LruIndexer` with the specified capacity.
    ///
    /// The capacity of the cache must be sufficient in order to generate a
    /// unique set of index and vertex data.
    pub fn with_capacity(capacity: usize) -> Self {
        let capacity = cmp::max(1, capacity);
        LruIndexer {
            lru: Vec::with_capacity(capacity),
            capacity,
            n: 0,
            phantom: PhantomData,
        }
    }

    fn find(&self, key: &K) -> Option<(usize, usize)> {
        self.lru
            .iter()
            .enumerate()
            .find(|&(_, entry)| entry.0 == *key)
            .map(|(index, entry)| (index, entry.1))
    }
}

impl<T, K> Default for LruIndexer<T, K>
where
    T: Topological,
    K: Clone + PartialEq,
{
    fn default() -> Self {
        LruIndexer::new()
    }
}

impl<T, K> Indexer<T, K> for LruIndexer<T, K>
where
    T: Topological,
    K: Clone + PartialEq,
{
    fn index<F>(&mut self, input: T::Vertex, f: F) -> (usize, Option<T::Vertex>)
    where
        F: Fn(&T::Vertex) -> &K,
    {
        let mut vertex = None;
        let key = f(&input).clone();
        let index = if let Some(entry) = self.find(&key) {
            let vertex = self.lru.remove(entry.0);
            self.lru.push(vertex);
            entry.1
        }
        else {
            vertex = Some(input);
            let m = self.n;
            self.n += 1;
            if self.lru.len() >= self.capacity {
                self.lru.remove(0);
            }
            self.lru.push((key, m));
            m
        };
        (index, vertex)
    }
}

/// Functions for collecting a stream of $n$-gons into raw index and vertex
/// buffers.
///
/// Unlike `IndexVertices`, this trait provides functions that are closed (not
/// parameterized) with respect to grouping. Instead, the trait is implemented
/// for a particular grouping. These functions cannot be used fluently as part
/// of an iterator expression. Generally, `IndexVertices` should be preferred.
pub trait GroupedIndexVertices<R, P>: Sized
where
    R: Grouping,
    P: Topological,
{
    fn index_vertices_with<N, K, F>(self, indexer: N, f: F) -> (Vec<R::Item>, Vec<P::Vertex>)
    where
        N: Indexer<P, K>,
        F: Fn(&P::Vertex) -> &K;

    fn index_vertices<N>(self, indexer: N) -> (Vec<R::Item>, Vec<P::Vertex>)
    where
        N: Indexer<P, P::Vertex>,
    {
        self.index_vertices_with::<N, P::Vertex, _>(indexer, |vertex| vertex)
    }
}

impl<R, P, I> GroupedIndexVertices<R, P> for I
where
    I: Iterator<Item = P>,
    R: Grouping,
    P: Map<<Vec<R::Item> as IndexBuffer<R>>::Index> + Topological,
    P::Output: Topological<Vertex = <Vec<R::Item> as IndexBuffer<R>>::Index>,
    Vec<R::Item>: Push<R, P::Output>,
{
    fn index_vertices_with<N, K, F>(self, mut indexer: N, f: F) -> (Vec<R::Item>, Vec<P::Vertex>)
    where
        N: Indexer<P, K>,
        F: Fn(&P::Vertex) -> &K,
    {
        let mut indices = Vec::new();
        let mut vertices = Vec::new();
        for topology in self {
            Push::push(
                &mut indices,
                topology.map(|vertex| {
                    let (index, vertex) = indexer.index(vertex, &f);
                    if let Some(vertex) = vertex {
                        vertices.push(vertex);
                    }
                    NumCast::from(index).unwrap()
                }),
            );
        }
        (indices, vertices)
    }
}

/// Functions for collecting a stream of $n$-gons into raw index and vertex
/// buffers.
///
/// Unlike `GroupedIndexVertices`, this trait provides functions that are
/// parameterized with respect to grouping. See the `Grouping` trait.
///
/// Using an indexer is not always the most effecient method to create a mesh
/// data structure from a generator. Generators provide per-attribute indices
/// that may be less expensive than an indexer when only a single attribute is
/// needed. For example, see also `vertices_with_position` and
/// `indices_for_position`.
///
/// See `HashIndexer` and `LruIndexer`.
///
/// # Examples
///
///
/// ```rust
/// # extern crate decorum;
/// # extern crate nalgebra;
/// # extern crate plexus;
/// #
/// use decorum::N64;
/// use nalgebra::Point3;
/// use plexus::index::{Flat3, HashIndexer};
/// use plexus::prelude::*;
/// use plexus::primitive::sphere::UvSphere;
///
/// # fn main() {
/// let sphere = UvSphere::new(32, 32);
/// let (indices, positions) = sphere
///     .polygons_with_position::<Point3<N64>>()
///     .triangulate()
///     .index_vertices::<Flat3, _>(HashIndexer::default());
/// # }
/// ```
pub trait IndexVertices<P>
where
    P: Topological,
{
    /// Indexes a stream of $n$-gons into raw index and vertex buffers using
    /// the given grouping, indexer, and keying function.
    fn index_vertices_with<R, N, K, F>(self, indexer: N, f: F) -> (Vec<R::Item>, Vec<P::Vertex>)
    where
        Self: GroupedIndexVertices<R, P>,
        R: Grouping,
        N: Indexer<P, K>,
        F: Fn(&P::Vertex) -> &K,
    {
        GroupedIndexVertices::<R, P>::index_vertices_with(self, indexer, f)
    }

    /// Indexes a stream of $n$-gons into raw index and vertex buffers using
    /// the given grouping and indexer.
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
    /// use plexus::index::{HashIndexer, Structured3};
    /// use plexus::prelude::*;
    /// use plexus::primitive::cube::Cube;
    ///
    /// # fn main() {
    /// // `indices` contains `Triangle`s with index data.
    /// let (indices, positions) = Cube::new()
    ///     .polygons_with_position::<Point3<N64>>()
    ///     .subdivide()
    ///     .triangulate()
    ///     .index_vertices::<Structured3, _>(HashIndexer::default());
    /// # }
    /// ```
    fn index_vertices<R, N>(self, indexer: N) -> (Vec<R::Item>, Vec<P::Vertex>)
    where
        Self: GroupedIndexVertices<R, P>,
        R: Grouping,
        N: Indexer<P, P::Vertex>,
    {
        IndexVertices::<P>::index_vertices_with(self, indexer, |vertex| vertex)
    }
}

impl<P, I> IndexVertices<P> for I
where
    I: Iterator<Item = P>,
    P: Topological,
{
}

pub trait FromIndexer<P, Q>: Sized
where
    P: Topological,
    Q: Topological<Vertex = P::Vertex>,
{
    type Error: Debug;

    fn from_indexer<I, N>(input: I, indexer: N) -> Result<Self, Self::Error>
    where
        I: IntoIterator<Item = P>,
        N: Indexer<Q, P::Vertex>;
}

/// Functions for collecting a stream of $n$-gons into a mesh data structure.
///
/// These functions can be used to collect data from a generator into a
/// `MeshBuffer` or `MeshGraph`.
///
/// See `HashIndexer` and `LruIndexer`.
pub trait CollectWithIndexer<P, Q>
where
    P: Topological,
    Q: Topological<Vertex = P::Vertex>,
{
    /// Collects a stream of $n$-gons into a mesh data structure using the
    /// given indexer.
    ///
    /// Unlike `collect`, this function allows the indexer to be specified.
    ///
    /// # Errors
    ///
    /// Returns an error defined by the implementer if the target type cannot be
    /// constructed from the indexed vertices.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # extern crate decorum;
    /// # extern crate nalgebra;
    /// # extern crate plexus;
    /// #
    /// use decorum::N32;
    /// use nalgebra::Point3;
    /// use plexus::graph::MeshGraph;
    /// use plexus::prelude::*;
    /// use plexus::primitive::cube::Cube;
    /// use plexus::index::HashIndexer;
    ///
    /// # fn main() {
    /// let graph = Cube::new()
    ///     .polygons_with_position::<Point3<N32>>()
    ///     .collect_with_indexer::<MeshGraph<Point3<f32>>, _>(HashIndexer::default())
    ///     .unwrap();
    /// # }
    fn collect_with_indexer<T, N>(self, indexer: N) -> Result<T, T::Error>
    where
        T: FromIndexer<P, Q>,
        N: Indexer<Q, P::Vertex>;
}

impl<P, Q, I> CollectWithIndexer<P, Q> for I
where
    I: Iterator<Item = P>,
    P: Topological,
    Q: Topological<Vertex = P::Vertex>,
{
    fn collect_with_indexer<T, N>(self, indexer: N) -> Result<T, T::Error>
    where
        T: FromIndexer<P, Q>,
        N: Indexer<Q, P::Vertex>,
    {
        T::from_indexer(self, indexer)
    }
}