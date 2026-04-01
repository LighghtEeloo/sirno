//! The knowledge graph: entries connected by dependencies and affinities,
//! mapped to code through groundings, with locks guarding sensitive entries.

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::edge::{Affinity, Dependency};
use crate::entry::{Entry, EntryId};
use crate::grounding::Grounding;
use tracing::debug;

/// Bidirectional index over directed dependency edges.
///
/// Maintains full dependency records alongside forward (`from → [to]`) and
/// reverse (`to → [from]`) adjacency sets. All mutations go through methods
/// that preserve this invariant.
#[derive(Clone, Debug, Default)]
pub struct DependencyIndex {
    /// Full dependency records keyed by endpoints.
    edges: HashMap<(EntryId, EntryId), Dependency>,
    /// `from` → set of entries whose validity depends on `from`.
    forward: HashMap<EntryId, HashSet<EntryId>>,
    /// `to` → set of entries that `to` depends on.
    reverse: HashMap<EntryId, HashSet<EntryId>>,
}

impl DependencyIndex {
    /// Record a dependency edge. Returns `true` if the edge was new.
    ///
    /// Re-inserting an existing endpoint pair replaces the stored relation
    /// metadata while preserving adjacency.
    pub fn insert(&mut self, dep: Dependency) -> bool {
        let from = dep.from().clone();
        let to = dep.to().clone();
        let is_new = self.edges.insert((from.clone(), to.clone()), dep).is_none();
        if is_new {
            self.forward.entry(from.clone()).or_default().insert(to.clone());
            self.reverse.entry(to).or_default().insert(from);
        }
        is_new
    }

    /// Remove a dependency edge. Returns `true` if the edge existed.
    pub fn remove(&mut self, dep: &Dependency) -> bool {
        let existed = self.edges.remove(&dep.key()).is_some();
        if existed {
            let mut remove_forward = false;
            if let Some(s) = self.forward.get_mut(dep.from()) {
                s.remove(dep.to());
                remove_forward = s.is_empty();
            }
            if remove_forward {
                self.forward.remove(dep.from());
            }

            let mut remove_reverse = false;
            if let Some(s) = self.reverse.get_mut(dep.to()) {
                s.remove(dep.from());
                remove_reverse = s.is_empty();
            }
            if remove_reverse {
                self.reverse.remove(dep.to());
            }
        }
        existed
    }

    /// Remove all edges involving the given entry (as source or target).
    pub fn remove_entry(&mut self, id: &EntryId) {
        // Remove forward edges from `id`.
        if let Some(targets) = self.forward.remove(id) {
            for t in &targets {
                self.edges.remove(&(id.clone(), t.clone()));
                if let Some(s) = self.reverse.get_mut(t) {
                    s.remove(id);
                }
            }
        }
        // Remove reverse edges to `id`.
        if let Some(sources) = self.reverse.remove(id) {
            for s in &sources {
                self.edges.remove(&(s.clone(), id.clone()));
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

    /// Look up a dependency by endpoints.
    pub fn get(&self, from: &EntryId, to: &EntryId) -> Option<&Dependency> {
        self.edges.get(&(from.clone(), to.clone()))
    }

    /// Iterate over all dependency records.
    pub fn iter(&self) -> impl Iterator<Item = &Dependency> {
        self.edges.values()
    }

    /// Remove meaning attachments that point to the given entry.
    pub fn clear_meanings_for(&mut self, id: &EntryId) -> usize {
        let mut cleared = 0;
        for dependency in self.edges.values_mut() {
            if dependency.meaning_matches(id) && dependency.clear_meaning() {
                cleared += 1;
            }
        }
        cleared
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
    affinities: BTreeMap<(EntryId, EntryId), Affinity>,
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
        self.affinities.retain(|_, affinity| !affinity.contains(id));
        let cleared_dependency_meanings = self.dependencies.clear_meanings_for(id);
        let mut cleared_affinity_meanings = 0;
        for affinity in self.affinities.values_mut() {
            if affinity.meaning_matches(id) && affinity.clear_meaning() {
                cleared_affinity_meanings += 1;
            }
        }
        self.groundings.remove(id);
        self.locks.remove(id);
        debug!(
            entry = %id,
            cleared_dependency_meanings,
            cleared_affinity_meanings,
            "removed entry from graph"
        );
        Some(entry)
    }

    /// Iterate over all entries.
    pub fn entries(&self) -> impl Iterator<Item = (&EntryId, &Entry)> {
        self.entries.iter()
    }

    fn ensure_entry_exists(&self, id: &EntryId) -> Result<(), GraphError> {
        if self.entries.contains_key(id) {
            Ok(())
        } else {
            Err(GraphError::EntryNotFound(id.clone()))
        }
    }

    // -- dependencies --------------------------------------------------------

    /// Add a dependency edge. Both endpoints must exist.
    pub fn add_dependency(&mut self, dep: Dependency) -> Result<bool, GraphError> {
        self.ensure_entry_exists(dep.from())?;
        self.ensure_entry_exists(dep.to())?;
        if let Some(meaning) = dep.meaning() {
            self.ensure_entry_exists(meaning)?;
        }
        let from = dep.from().clone();
        let to = dep.to().clone();
        let meaning = dep.meaning().cloned();
        let is_new = self.dependencies.insert(dep);
        debug!(from = %from, to = %to, meaning = ?meaning, is_new, "recorded dependency");
        Ok(is_new)
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

    /// Look up a dependency by endpoints.
    pub fn dependency(&self, from: &EntryId, to: &EntryId) -> Option<&Dependency> {
        self.dependencies.get(from, to)
    }

    /// Iterate over all dependency records.
    pub fn dependencies(&self) -> impl Iterator<Item = &Dependency> {
        self.dependencies.iter()
    }

    // -- affinities ----------------------------------------------------------

    /// Add an affinity edge. All referenced entries must exist.
    ///
    /// Re-inserting an existing endpoint pair replaces the stored relation
    /// metadata while preserving the endpoints.
    pub fn add_affinity(&mut self, affinity: Affinity) -> Result<bool, GraphError> {
        self.ensure_entry_exists(affinity.a())?;
        self.ensure_entry_exists(affinity.b())?;
        if let Some(meaning) = affinity.meaning() {
            self.ensure_entry_exists(meaning)?;
        }
        let a = affinity.a().clone();
        let b = affinity.b().clone();
        let meaning = affinity.meaning().cloned();
        let is_new = self.affinities.insert(affinity.key(), affinity).is_none();
        debug!(a = %a, b = %b, meaning = ?meaning, is_new, "recorded affinity");
        Ok(is_new)
    }

    /// Remove an affinity edge.
    pub fn remove_affinity(&mut self, affinity: &Affinity) -> bool {
        self.affinities.remove(&affinity.key()).is_some()
    }

    /// Iterate over all affinity records.
    pub fn affinities(&self) -> impl Iterator<Item = &Affinity> {
        self.affinities.values()
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

#[cfg(test)]
mod tests {
    use super::Graph;
    use crate::edge::{Affinity, Dependency};
    use crate::entry::{Entry, EntryId};

    fn entry(id: &str) -> Entry {
        Entry::new(EntryId::new(id), format!("{id} description"), format!("{id} explanation"))
    }

    #[test]
    fn removing_meaning_entry_clears_dependency_meaning() {
        let cause = EntryId::new("cause");
        let target = EntryId::new("target");
        let meaning = EntryId::new("dependency-meaning");

        let mut graph = Graph::new();
        graph.insert_entry(entry(cause.as_str())).unwrap();
        graph.insert_entry(entry(target.as_str())).unwrap();
        graph.insert_entry(entry(meaning.as_str())).unwrap();

        graph
            .add_dependency(
                Dependency::new(cause.clone(), target.clone()).with_meaning(meaning.clone()),
            )
            .unwrap();
        assert_eq!(
            graph.dependency(&cause, &target).and_then(|dependency| dependency.meaning()),
            Some(&meaning)
        );

        graph.remove_entry(&meaning).unwrap();

        assert_eq!(
            graph.dependency(&cause, &target).and_then(|dependency| dependency.meaning()),
            None
        );
    }

    #[test]
    fn affinity_meaning_must_exist() {
        let left = EntryId::new("left");
        let right = EntryId::new("right");
        let missing = EntryId::new("missing-meaning");

        let mut graph = Graph::new();
        graph.insert_entry(entry(left.as_str())).unwrap();
        graph.insert_entry(entry(right.as_str())).unwrap();

        let affinity = Affinity::new(left, right).unwrap().with_meaning(missing.clone());
        let error = graph.add_affinity(affinity).unwrap_err();

        match error {
            | crate::graph::GraphError::EntryNotFound(id) => assert_eq!(id, missing),
            | other => panic!("unexpected error: {other:?}"),
        }
    }
}
