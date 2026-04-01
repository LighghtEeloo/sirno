//! Sirno: Semantic Intermediate Representation of Nominal Obligations.
//!
//! A graph-shaped knowledge database for codebases. Sirno mediates between
//! abstract design knowledge and concrete code through a structured graph
//! of named, agent-maintained knowledge units.
//!
//! Entries own the graph's dedicated explanatory text. Other structures refer
//! to entries when they need prose and otherwise keep only operational data.

pub mod edge;
pub mod entry;
pub mod graph;
pub mod grounding;
pub mod mutation;
pub mod obligation;
pub mod session;
