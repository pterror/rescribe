//! rescribe-core: Universal document intermediate representation.
//!
//! This crate provides the core types for representing documents in a
//! format-agnostic way, enabling lossless conversion between formats.

mod document;
mod node;
mod properties;
mod resource;
mod fidelity;
mod traits;

pub use document::*;
pub use node::*;
pub use properties::*;
pub use resource::*;
pub use fidelity::*;
pub use traits::*;
