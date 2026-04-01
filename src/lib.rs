//! Sirno: Semantic Intermediate Representation of Nominal Obligations.
//!
//! A graph-shaped knowledge database for codebases. Sirno mediates between
//! abstract design knowledge and concrete code through a structured graph
//! of named, agent-maintained knowledge units.
//!
//! Entries own the graph's dedicated explanatory text. Other structures refer
//! to entries when they need prose and otherwise keep only operational data.
//!
//! Sirno also has a durable on-disk form: the Sirno data representation.
//! The data representation consists of `Sirno.toml` plus the configured data
//! directory of entry files. Runtime graph state is loaded from that
//! representation and written back to it at commit boundaries.

pub mod edge;
pub mod entry;
pub mod graph;
pub mod grounding;
pub mod mutation;
pub mod obligation;
pub mod repository;
pub mod session;
