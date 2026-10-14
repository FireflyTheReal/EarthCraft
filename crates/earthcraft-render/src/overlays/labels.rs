//! Screen-space occupancy, preserving the caller's ordered label admission.
use emath::Rect;
use rstar::{AABB, RTree, RTreeObject};
use std::ops::ControlFlow;

struct LabelRect(Rect);

impl RTreeObject for LabelRect {
    type Envelope = AABB<[f32; 2]>;
    fn envelope(&self) -> Self::Envelope {
        AABB::from_corners([self.0.min.x, self.0.min.y], [self.0.max.x, self.0.max.y])
    }
}

#[derive(Default)]
pub struct LabelIndex {
    tree: RTree<LabelRect>,
}
impl LabelIndex {
    pub fn is_empty(&self) -> bool {
        self.tree.size() == 0
    }
    pub fn len(&self) -> usize {
        self.tree.size()
    }
    pub fn try_insert(&mut self, rect: Rect) -> bool {
        let entry = LabelRect(rect);
        // Internal traversal short-circuits without allocating an iterator stack.
        // AABB and egui both treat touching edges as intersection.
        let overlap = self
            .tree
            .locate_in_envelope_intersecting_int(entry.envelope(), |_| ControlFlow::Break(()))
            .is_break();
        if overlap {
            return false;
        }
        self.tree.insert(entry);
        true
    }
}

#[cfg(test)]
mod tests;
