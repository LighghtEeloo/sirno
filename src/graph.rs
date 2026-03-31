//! The knowledge graph: entries connected by dependencies and affinities,
//! mapped to code through groundings, with locks guarding sensitive entries.

use std::collections::{BTreeSet, HashMap, HashSet};

use crate::edge::{Affinity, Dependency};
use crate::entry::{Entry, EntryId};
use crate::grounding::Grounding;

/// Bidirectional index over directed dependency edges.
///
/// Maintains forward (`from → [to]`) and reverse (`to → [from]`) adjacency
/// sets in lockstep. All mutations go through methods that preserve this
/// invariant.
#[derive(Clone, Debug, Default)]
pub struct DependencyIndex {
    /// `from` → set of entries whose validity depends on `from`.
    forward: HashMap<EntryId, HashSet<EntryId>>,
    /// `to` → set of entries that `to` depends on.
    reverse: HashMap<EntryId, HashSet<EntryId>>,
}

impl DependencyIndex {
    /// Record a dependency edge. Returns `true` if the edge was new.
    pub fn insert(&mut self, dep: &Dependency) -> bool {
        let fwd_new = self.forward.entry(dep.from.clone()).or_default().insert(dep.to.clone());
        self.reverse.entry(dep.to.clone()).or_default().insert(dep.from.clone());
        fwd_new
    }

    /// Remove a dependency edge. Returns `true` if the edge existed.
    pub fn remove(&mut self, dep: &Dependency) -> bool {
        let existed = self.forward.get_mut(&dep.from).is_some_and(|s| s.remove(&dep.to));
        if existed {
            if let Some(s) = self.reverse.get_mut(&dep.to) {
                s.remove(&dep.from);
            }
        }
        existed
    }

    /// Remove all edges involving the given entry (as source or target).
    pub fn remove_entry(&mut self, id: &EntryId) {
        // Remove forward edges from `id`.
        if let Some(targets) = self.forward.remove(id) {
            for t in &targets {
                if let Some(s) = self.reverse.get_mut(t) {
                    s.remove(id);
                }
            }
        }
        // Remove reverse edges to `id`.
        if let Some(sources) = self.reverse.remove(id) {
            for s in &sources {
                if let Some(fwd) = self.forward.get_mut(s) {
                    fwd.remove(id);
                }
            }
        }
    }

    /// Entries whose validity is contingent on `from` (forward neighbors).
    pub fn dependents_of(&self, from: &EntryId) -> impl Iterator<Item = &EntryId> {
        self.forward.get(from).into_iter().flat_map(|s| s.iter())
    }

    /// Entries that `to` depends on (reverse neighbors).
    pub fn dependencies_of(&self, to: &EntryId) -> impl Iterator<Item = &EntryId> {
        self.reverse.get(to).into_iter().flat_map(|s| s.iter())
    }
}

/// Errors arising from graph operations.
#[derive(Debug, thiserror::Error)]
pub enum GraphError {
    #[error("entry already exists: {0}")]
    DuplicateEntry(EntryId),

    #[error("entry not found: {0}")]
    EntryNotFound(EntryId),
}

/// The knowledge graph.
///
/// Holds entries keyed by id, edges (dependencies and affinities), groundings
/// per entry, and the set of locked entry ids.
#[derive(Clone, Debug, Default)]
pub struct Graph {
    entries: HashMap<EntryId, Entry>,
    dependencies: DependencyIndex,
    affinities: BTreeSet<Affinity>,
    groundings: HashMap<EntryId, Vec<Grounding>>,
    locks: HashSet<EntryId>,
}

impl Graph {
    /// Construct an empty graph.
    pub fn new() -> Self {
        Self::default()
    }

    // -- entries -------------------------------------------------------------

    /// Insert a new entry. Fails if an entry with the same id already exists.
    pub fn insert_entry(&mut self, entry: Entry) -> Result<(), GraphError> {
        let id = entry.id().clone();
        if self.entries.contains_key(&id) {
            return Err(GraphError::DuplicateEntry(id));
        }
        self.entries.insert(id, entry);
        Ok(())
    }

    /// Look up an entry by id.
    pub fn entry(&self, id: &EntryId) -> Option<&Entry> {
        self.entries.get(id)
    }

    /// Mutable access to an entry by id.
    pub fn entry_mut(&mut self, id: &EntryId) -> Option<&mut Entry> {
        self.entries.get_mut(id)
    }

    /// Remove an entry and all its edges, groundings, and locks.
    pub fn remove_entry(&mut self, id: &EntryId) -> Option<Entry> {
        let entry = self.entries.remove(id)?;
        self.dependencies.remove_entry(id);
        self.affinities.retain(|a| !a.contains(id));
        self.groundings.remove(id);
        self.locks.remove(id);
        Some(entry)
    }

    /// Iterate over all entries.
    pub fn entries(&self) -> impl Iterator<Item = (&EntryId, &Entry)> {
        self.entries.iter()
    }

    // -- dependencies --------------------------------------------------------

    /// Add a dependency edge. Both endpoints must exist.
    pub fn add_dependency(&mut self, dep: Dependency) -> Result<bool, GraphError> {
        if !self.entries.contains_key(&dep.from) {
            return Err(GraphError::EntryNotFound(dep.from.clone()));
        }
        if !self.entries.contains_key(&dep.to) {
            return Err(GraphError::EntryNotFound(dep.to.clone()));
        }
        Ok(self.dependencies.insert(&dep))
    }

    /// Remove a dependency edge.
    pub fn remove_dependency(&mut self, dep: &Dependency) -> bool {
        self.dependencies.remove(dep)
    }

    /// Entries whose validity depends on the given entry.
    pub fn dependents_of(&self, id: &EntryId) -> impl Iterator<Item = &EntryId> {
        self.dependencies.dependents_of(id)
    }

    /// Entries that the given entry depends on.
    pub fn dependencies_of(&self, id: &EntryId) -> impl Iterator<Item = &EntryId> {
        self.dependencies.dependencies_of(id)
    }

    // -- affinities ----------------------------------------------------------

    /// Add an affinity edge. Returns `true` if it was new.
    pub fn add_affinity(&mut self, affinity: Affinity) -> bool {
        self.affinities.insert(affinity)
    }

    /// Remove an affinity edge.
    pub fn remove_affinity(&mut self, affinity: &Affinity) -> bool {
        self.affinities.remove(affinity)
    }

    // -- groundings ----------------------------------------------------------

    /// Attach a grounding to an entry. The entry must exist.
    pub fn add_grounding(&mut self, id: &EntryId, grounding: Grounding) -> Result<(), GraphError> {
        if !self.entries.contains_key(id) {
            return Err(GraphError::EntryNotFound(id.clone()));
        }
        self.groundings.entry(id.clone()).or_default().push(grounding);
        Ok(())
    }

    /// All groundings for an entry.
    pub fn groundings(&self, id: &EntryId) -> &[Grounding] {
        self.groundings.get(id).map_or(&[], |v| v.as_slice())
    }

    // -- locks ---------------------------------------------------------------

    /// Lock an entry, requiring approval for future mutations.
    pub fn lock(&mut self, id: &EntryId) -> Result<bool, GraphError> {
        if !self.entries.contains_key(id) {
            return Err(GraphError::EntryNotFound(id.clone()));
        }
        Ok(self.locks.insert(id.clone()))
    }

    /// Unlock an entry.
    pub fn unlock(&mut self, id: &EntryId) -> bool {
        self.locks.remove(id)
    }

    /// Whether the entry is locked.
    pub fn is_locked(&self, id: &EntryId) -> bool {
        self.locks.contains(id)
    }
}
