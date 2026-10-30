//! Commit relationships, graph layout, paged details, and history filtering.
//!
//! Read relationships off the UI thread, parse once, then keep `GraphLayout` in Rust.
//! Only `GraphLayout::visible_rows` results should cross the future Canvas IPC boundary.

mod graph;
mod interval;
mod layout;
mod print;
pub mod read;

pub use graph::{CommitGraph, CommitId, Edge, EdgeId, GraphError};
pub use layout::{ColorId, GraphLayout, Head};
pub use print::{Direction, EdgeSegment, Row, VisibleRows};

#[cfg(test)]
mod tests;

pub mod details;
