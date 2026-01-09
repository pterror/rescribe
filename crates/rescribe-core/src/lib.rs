//! rescribe-core: Universal document intermediate representation.
//!
//! This crate provides the core types for representing documents in a
//! format-agnostic way, enabling lossless conversion between formats.

mod document;
mod fidelity;
mod node;
mod properties;
mod resource;
mod traits;

pub use document::*;
pub use fidelity::*;
pub use node::*;
pub use properties::*;
pub use resource::*;
pub use traits::*;
