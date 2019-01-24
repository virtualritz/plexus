//! Higher order geometric traits.
//!
//! This module defines higher order traits for operations on a graph. It also
//! provides aliases for geometric types to improve readability of type
//! constraints. These traits can be used as contraints to prove to the
//! compiler that certain operations are supported without specifying
//! complicated relationships.
//!
//! The traits in this module have blanket implementations that apply when
//! certain geometric and operational traits are implemented. For example, if a
//! type implements `AsPosition` and the `Output` type of that implementation
//! also implements `Cross` and `Normalize`, then a `Geometry` using that type
//! as its `Vertex` attribute will likely implement the `FaceNormal` trait in
//! this module.
//!
//! # Examples
//!
//! A function that subdivides triangular faces in a mesh:
//!
//! ```rust
//! use plexus::prelude::*;
//! ```

use std::ops::{Add, Sub};

use self::alias::*;
use crate::geometry::convert::AsPosition;
use crate::geometry::ops::{Average, Cross, Interpolate, Normalize, Project};
use crate::geometry::Geometry;
use crate::graph::container::Reborrow;
use crate::graph::storage::convert::AsStorage;
use crate::graph::topology::{Edge, Face, Vertex};
use crate::graph::view::{EdgeView, FaceView};
use crate::graph::GraphError;

pub trait FaceNormal: Geometry {
    type Normal;

    fn normal<M>(face: FaceView<M, Self>) -> Result<Self::Normal, GraphError>
    where
        M: Reborrow,
        M::Target: AsStorage<Edge<Self>> + AsStorage<Face<Self>> + AsStorage<Vertex<Self>>;
}

impl<G> FaceNormal for G
where
    G: Geometry,
    G::Vertex: AsPosition,
    VertexPosition<G>: Clone + Sub,
    <VertexPosition<G> as Sub>::Output: Cross,
    <<VertexPosition<G> as Sub>::Output as Cross>::Output: Normalize,
{
    type Normal = <<VertexPosition<G> as Sub>::Output as Cross>::Output;

    fn normal<M>(face: FaceView<M, Self>) -> Result<Self::Normal, GraphError>
    where
        M: Reborrow,
        M::Target: AsStorage<Edge<Self>> + AsStorage<Face<Self>> + AsStorage<Vertex<Self>>,
    {
        let positions = face
            .reachable_vertices()
            .take(3)
            .map(|vertex| vertex.geometry.as_position().clone())
            .collect::<Vec<_>>();
        let (a, b, c) = (&positions[0], &positions[1], &positions[2]);
        let ab = a.clone() - b.clone();
        let bc = b.clone() - c.clone();
        Ok(ab.cross(bc).normalize())
    }
}

pub trait FaceCentroid: Geometry {
    type Centroid;

    fn centroid<M>(face: FaceView<M, Self>) -> Result<Self::Centroid, GraphError>
    where
        M: Reborrow,
        M::Target: AsStorage<Edge<Self>> + AsStorage<Face<Self>> + AsStorage<Vertex<Self>>;
}

impl<G> FaceCentroid for G
where
    G: Geometry,
    G::Vertex: Average,
{
    type Centroid = G::Vertex;

    fn centroid<M>(face: FaceView<M, Self>) -> Result<Self::Centroid, GraphError>
    where
        M: Reborrow,
        M::Target: AsStorage<Edge<Self>> + AsStorage<Face<Self>> + AsStorage<Vertex<Self>>,
    {
        Ok(G::Vertex::average(
            face.reachable_vertices()
                .map(|vertex| vertex.geometry.clone()),
        ))
    }
}

pub trait EdgeMidpoint: Geometry {
    type Midpoint;

    fn midpoint<M>(edge: EdgeView<M, Self>) -> Result<Self::Midpoint, GraphError>
    where
        M: Reborrow,
        M::Target: AsStorage<Edge<Self>> + AsStorage<Vertex<Self>>;
}

impl<G> EdgeMidpoint for G
where
    G: Geometry,
    G::Vertex: AsPosition,
    VertexPosition<G>: Clone + Interpolate,
{
    type Midpoint = <VertexPosition<G> as Interpolate>::Output;

    fn midpoint<M>(edge: EdgeView<M, Self>) -> Result<Self::Midpoint, GraphError>
    where
        M: Reborrow,
        M::Target: AsStorage<Edge<Self>> + AsStorage<Vertex<Self>>,
    {
        let a = edge
            .reachable_source_vertex()
            .ok_or_else(|| GraphError::TopologyNotFound)?
            .geometry
            .as_position()
            .clone();
        let b = edge
            .reachable_destination_vertex()
            .ok_or_else(|| GraphError::TopologyNotFound)?
            .geometry
            .as_position()
            .clone();
        Ok(a.midpoint(b))
    }
}

pub trait EdgeLateral: Geometry {
    type Lateral;

    fn lateral<M>(edge: EdgeView<M, Self>) -> Result<Self::Lateral, GraphError>
    where
        M: Reborrow,
        M::Target: AsStorage<Edge<Self>> + AsStorage<Vertex<Self>>;
}

impl<G> EdgeLateral for G
where
    G: Geometry,
    G::Vertex: AsPosition,
    VertexPosition<G>: Clone
        + Add<
            <<VertexPosition<G> as Sub>::Output as Project>::Output,
            Output = VertexPosition<G>,
        > + Sub,
    <VertexPosition<G> as Sub>::Output: Normalize + Project,
{
    type Lateral = <VertexPosition<G> as Sub>::Output;

    fn lateral<M>(edge: EdgeView<M, Self>) -> Result<Self::Lateral, GraphError>
    where
        M: Reborrow,
        M::Target: AsStorage<Edge<Self>> + AsStorage<Vertex<Self>>,
    {
        let a = edge
            .reachable_source_vertex()
            .ok_or_else(|| GraphError::TopologyNotFound)?
            .geometry
            .as_position()
            .clone();
        let b = edge
            .reachable_destination_vertex()
            .ok_or_else(|| GraphError::TopologyNotFound)?
            .geometry
            .as_position()
            .clone();
        let c = edge
            .reachable_opposite_edge()
            .ok_or_else(|| GraphError::TopologyNotFound)?
            .reachable_previous_edge()
            .ok_or_else(|| GraphError::TopologyNotFound)?
            .reachable_destination_vertex()
            .ok_or_else(|| GraphError::TopologyNotFound)?
            .geometry
            .as_position()
            .clone();
        let ab = a - b.clone();
        let cb = c.clone() - b.clone();
        let p = b + ab.project(cb);
        Ok((p - c).normalize())
    }
}

pub mod alias {
    use std::ops::Mul;

    use super::*;

    pub type VertexPosition<G> = <<G as Geometry>::Vertex as AsPosition>::Target;
    pub type ScaledFaceNormal<G, T> = <<G as FaceNormal>::Normal as Mul<T>>::Output;
    pub type ScaledEdgeLateral<G, T> = <<G as EdgeLateral>::Lateral as Mul<T>>::Output;
}
