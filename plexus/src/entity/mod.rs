pub mod borrow;
pub mod dijkstra;
pub mod storage;
pub mod traverse;
pub mod view;

use thiserror::Error;

use crate::entity::storage::{Dispatch, Key, Storage, Unjournaled};

#[derive(Debug, Error, PartialEq)]
pub enum EntityError {
    #[error("required entity not found")]
    EntityNotFound,
    #[error("geometric operation failed")]
    Geometry,
}

#[cfg(not(nightly))]
pub trait Entity: 'static + Copy + Sized {
    type Key: Key;
    type Storage: Default + Dispatch<Self> + Storage<Self> + Unjournaled;
}

#[cfg(nightly)]
pub trait Entity: Copy + Sized {
    type Key: Key;
    type Storage: Default + Dispatch<Self> + Storage<Self> + Unjournaled;
}
