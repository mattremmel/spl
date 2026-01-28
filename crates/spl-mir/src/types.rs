//! Core MIR types for places and projections.
//!
//! This module defines the fundamental types for representing memory locations
//! in MIR: locals, places, and projections.

/// A local variable in MIR.
///
/// Locals are indexed storage slots for values. Local 0 is always the return place.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Local(pub u32);

impl Local {
    /// The return place is always local 0.
    pub const RETURN_PLACE: Local = Local(0);

    /// Create a new local with the given index.
    pub fn new(index: u32) -> Self {
        Local(index)
    }

    /// Get the index of this local.
    pub fn index(self) -> u32 {
        self.0
    }
}

/// A field index for struct/tuple field access.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FieldIdx(pub u32);

impl FieldIdx {
    /// Create a new field index.
    pub fn new(index: u32) -> Self {
        FieldIdx(index)
    }

    /// Get the index value.
    pub fn index(self) -> u32 {
        self.0
    }
}

/// A projection element describing how to access a sub-part of a place.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum PlaceElem {
    /// Dereference a pointer/reference: `*place`.
    Deref,
    /// Access a field: `place.field`.
    Field(FieldIdx),
    /// Index into an array/slice: `place[index]`.
    Index(Local),
    /// Constant index into an array: `place[const]`.
    ConstantIndex { offset: u64, from_end: bool },
    /// A subslice: `place[from..to]`.
    Subslice { from: u64, to: u64 },
    /// Downcast to a specific variant (for enums).
    Downcast(u32),
}

/// A place is a path to a memory location.
///
/// Places describe where a value lives. A place is either:
/// - A local variable directly
/// - A projection from another place (field access, deref, index)
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Place {
    /// The base local variable.
    pub local: Local,
    /// Projections applied to reach the final location.
    pub projection: Vec<PlaceElem>,
}

impl Place {
    /// Create a place referring directly to a local.
    pub fn from_local(local: Local) -> Self {
        Place {
            local,
            projection: Vec::new(),
        }
    }

    /// Create a place with a single field projection.
    pub fn field(local: Local, field: FieldIdx) -> Self {
        Place {
            local,
            projection: vec![PlaceElem::Field(field)],
        }
    }

    /// Create a place with a deref projection.
    pub fn deref(local: Local) -> Self {
        Place {
            local,
            projection: vec![PlaceElem::Deref],
        }
    }

    /// Add a projection to this place.
    pub fn project(mut self, elem: PlaceElem) -> Self {
        self.projection.push(elem);
        self
    }

    /// Returns true if this place has no projections (is just a local).
    pub fn is_local(&self) -> bool {
        self.projection.is_empty()
    }
}

impl From<Local> for Place {
    fn from(local: Local) -> Self {
        Place::from_local(local)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Phase 1 Tests: Core IR Types

    #[test]
    fn local_id_is_copy_and_eq() {
        let local1 = Local(1);
        let local2 = local1; // Copy
        let local3 = Local(1);

        assert_eq!(local1, local2);
        assert_eq!(local1, local3);

        // Different locals are not equal
        let local4 = Local(2);
        assert_ne!(local1, local4);
    }

    #[test]
    fn local_zero_is_return_place() {
        assert_eq!(Local::RETURN_PLACE, Local(0));
        assert_eq!(Local::RETURN_PLACE.index(), 0);
    }

    #[test]
    fn local_new_and_index() {
        let local = Local::new(42);
        assert_eq!(local.index(), 42);
        assert_eq!(local, Local(42));
    }

    #[test]
    fn place_from_local() {
        let local = Local(5);
        let place = Place::from_local(local);

        assert_eq!(place.local, local);
        assert!(place.projection.is_empty());
        assert!(place.is_local());
    }

    #[test]
    fn place_from_local_via_into() {
        let local = Local(5);
        let place: Place = local.into();

        assert_eq!(place.local, local);
        assert!(place.is_local());
    }

    #[test]
    fn place_with_field_projection() {
        let local = Local(1);
        let field = FieldIdx::new(2);
        let place = Place::field(local, field);

        assert_eq!(place.local, local);
        assert_eq!(place.projection.len(), 1);
        assert_eq!(place.projection[0], PlaceElem::Field(field));
        assert!(!place.is_local());
    }

    #[test]
    fn place_with_deref_projection() {
        let local = Local(1);
        let place = Place::deref(local);

        assert_eq!(place.local, local);
        assert_eq!(place.projection.len(), 1);
        assert_eq!(place.projection[0], PlaceElem::Deref);
    }

    #[test]
    fn place_chained_projections() {
        let local = Local(1);
        let place = Place::from_local(local)
            .project(PlaceElem::Deref)
            .project(PlaceElem::Field(FieldIdx(0)));

        assert_eq!(place.local, local);
        assert_eq!(place.projection.len(), 2);
        assert_eq!(place.projection[0], PlaceElem::Deref);
        assert_eq!(place.projection[1], PlaceElem::Field(FieldIdx(0)));
    }

    #[test]
    fn place_index_projection() {
        let base = Local(1);
        let index = Local(2);
        let place = Place::from_local(base).project(PlaceElem::Index(index));

        assert_eq!(place.projection[0], PlaceElem::Index(index));
    }

    #[test]
    fn field_idx_new_and_index() {
        let field = FieldIdx::new(10);
        assert_eq!(field.index(), 10);
        assert_eq!(field, FieldIdx(10));
    }

    #[test]
    fn place_elem_equality() {
        assert_eq!(PlaceElem::Deref, PlaceElem::Deref);
        assert_eq!(PlaceElem::Field(FieldIdx(1)), PlaceElem::Field(FieldIdx(1)));
        assert_ne!(PlaceElem::Field(FieldIdx(1)), PlaceElem::Field(FieldIdx(2)));
        assert_ne!(PlaceElem::Deref, PlaceElem::Field(FieldIdx(0)));
    }

    #[test]
    fn constant_index_projection() {
        let place = Place::from_local(Local(0)).project(PlaceElem::ConstantIndex {
            offset: 5,
            from_end: false,
        });

        match &place.projection[0] {
            PlaceElem::ConstantIndex { offset, from_end } => {
                assert_eq!(*offset, 5);
                assert!(!from_end);
            }
            _ => panic!("expected ConstantIndex"),
        }
    }

    #[test]
    fn subslice_projection() {
        let place = Place::from_local(Local(0)).project(PlaceElem::Subslice { from: 1, to: 5 });

        match &place.projection[0] {
            PlaceElem::Subslice { from, to } => {
                assert_eq!(*from, 1);
                assert_eq!(*to, 5);
            }
            _ => panic!("expected Subslice"),
        }
    }

    #[test]
    fn downcast_projection() {
        let place = Place::from_local(Local(0)).project(PlaceElem::Downcast(2));

        assert_eq!(place.projection[0], PlaceElem::Downcast(2));
    }

    // Additional coverage tests

    #[test]
    fn local_hash() {
        use std::collections::HashMap;

        let mut map = HashMap::new();
        map.insert(Local(1), "one");
        map.insert(Local(2), "two");

        assert_eq!(map.get(&Local(1)), Some(&"one"));
        assert_eq!(map.get(&Local(2)), Some(&"two"));
        assert_eq!(map.get(&Local(3)), None);
    }

    #[test]
    fn field_idx_hash() {
        use std::collections::HashSet;

        let mut set = HashSet::new();
        set.insert(FieldIdx(0));
        set.insert(FieldIdx(1));
        set.insert(FieldIdx(0)); // duplicate

        assert_eq!(set.len(), 2);
    }

    #[test]
    fn place_hash() {
        use std::collections::HashSet;

        let mut set = HashSet::new();
        set.insert(Place::from_local(Local(1)));
        set.insert(Place::from_local(Local(2)));
        set.insert(Place::from_local(Local(1))); // duplicate

        assert_eq!(set.len(), 2);
    }

    #[test]
    fn place_with_same_projections_equal() {
        let place1 = Place::from_local(Local(1))
            .project(PlaceElem::Deref)
            .project(PlaceElem::Field(FieldIdx(2)));
        let place2 = Place::from_local(Local(1))
            .project(PlaceElem::Deref)
            .project(PlaceElem::Field(FieldIdx(2)));

        assert_eq!(place1, place2);
    }

    #[test]
    fn place_different_projections_not_equal() {
        let place1 = Place::from_local(Local(1)).project(PlaceElem::Deref);
        let place2 = Place::from_local(Local(1)).project(PlaceElem::Field(FieldIdx(0)));

        assert_ne!(place1, place2);
    }

    #[test]
    fn place_elem_index_with_different_locals() {
        let elem1 = PlaceElem::Index(Local(1));
        let elem2 = PlaceElem::Index(Local(2));

        assert_ne!(elem1, elem2);
    }

    #[test]
    fn place_elem_constant_index_variants() {
        let from_start = PlaceElem::ConstantIndex {
            offset: 5,
            from_end: false,
        };
        let from_end = PlaceElem::ConstantIndex {
            offset: 5,
            from_end: true,
        };

        assert_ne!(from_start, from_end);
    }

    #[test]
    fn place_deep_projections() {
        // (*(*local).field[index]).subfield
        let place = Place::from_local(Local(1))
            .project(PlaceElem::Deref)
            .project(PlaceElem::Field(FieldIdx(0)))
            .project(PlaceElem::Index(Local(2)))
            .project(PlaceElem::Deref)
            .project(PlaceElem::Field(FieldIdx(1)));

        assert_eq!(place.projection.len(), 5);
        assert!(!place.is_local());
    }
}
