//! # Sprite list — port of `CSpriteList` (`SpriteList.cpp`)
//!
//! The original is an intrusive doubly-linked list; here it's a Vec
//! arena with stable ids, preserving the linked list's observable
//! behavior: append order is iteration order, removal keeps the order
//! of survivors, and update caches "next" so a sprite destroying
//! itself (timeout) doesn't skip its successor.
//!
//! The cross-list `Collide` walkers (with their restart-on-mutation
//! semantics) are ported alongside their first callers in the gameplay
//! systems — the callback shapes are theirs.

use crate::frame::Frame;
use crate::rand::Rand;
use crate::rect::Rect;
use crate::sprite::{Sprite, UpdateResult};
use crate::virtual_frame::VirtualFrame;

/// Stable handle to a sprite in a list (survives removals of others).
pub type SpriteId = u64;

#[derive(Default)]
pub struct SpriteList {
    entries: Vec<(SpriteId, Sprite)>,
    next_id: SpriteId,
}

impl SpriteList {
    pub fn new() -> Self {
        Self::default()
    }

    /// `Add` — append to the tail.
    pub fn add(&mut self, sprite: Sprite) -> SpriteId {
        let id = self.next_id;
        self.next_id += 1;
        self.entries.push((id, sprite));
        id
    }

    /// `Remove`/`Destroy` — drop by id, preserving the others' order.
    /// Silently ignores unknown ids like the original's list walk.
    pub fn destroy(&mut self, id: SpriteId) {
        if let Some(pos) = self.entries.iter().position(|(eid, _)| *eid == id) {
            self.entries.remove(pos);
        }
    }

    /// `Destroy()` — clear the whole list.
    pub fn destroy_all(&mut self) {
        self.entries.clear();
    }

    /// `GetNum`.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// `CountVisible`.
    pub fn count_visible(&self) -> usize {
        self.entries.iter().filter(|(_, s)| s.visible).count()
    }

    /// `GetSprite(num)` — by list position.
    pub fn get(&self, index: usize) -> &Sprite {
        &self.entries[index].1
    }

    pub fn get_mut(&mut self, index: usize) -> &mut Sprite {
        &mut self.entries[index].1
    }

    pub fn id_at(&self, index: usize) -> SpriteId {
        self.entries[index].0
    }

    pub fn by_id(&self, id: SpriteId) -> Option<&Sprite> {
        self.entries
            .iter()
            .find(|(eid, _)| *eid == id)
            .map(|(_, s)| s)
    }

    pub fn by_id_mut(&mut self, id: SpriteId) -> Option<&mut Sprite> {
        self.entries
            .iter_mut()
            .find(|(eid, _)| *eid == id)
            .map(|(_, s)| s)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Sprite> {
        self.entries.iter().map(|(_, s)| s)
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Sprite> {
        self.entries.iter_mut().map(|(_, s)| s)
    }

    /// `Update` — head-to-tail; a sprite destroying itself (timeout)
    /// removes in place and the walk continues with its successor.
    pub fn update(&mut self, clip: &Rect, rand: &mut Rand) {
        let mut i = 0;
        while i < self.entries.len() {
            match self.entries[i].1.update(clip, rand) {
                UpdateResult::Live => i += 1,
                UpdateResult::Destroy => {
                    self.entries.remove(i);
                }
            }
        }
    }

    /// `Draw` — every sprite, in list order.
    pub fn draw(&self, world: &VirtualFrame, screen: &mut Frame) {
        for (_, sprite) in &self.entries {
            sprite.draw(world, screen);
        }
    }

    /// `Erase` — every sprite's rect, in list order.
    pub fn erase(&self, world: &VirtualFrame, screen: &mut Frame) {
        for (_, sprite) in &self.entries {
            sprite.erase(world, screen);
        }
    }

    /// `Check` — f32 checksum accumulated in list order.
    pub fn check(&self) -> f32 {
        let mut checksum = 0.0f32;
        for (_, sprite) in &self.entries {
            checksum += sprite.check(false);
        }
        checksum
    }

    /// `DoToAll`.
    pub fn do_to_all(&mut self, mut f: impl FnMut(&mut Sprite)) {
        for (_, sprite) in &mut self.entries {
            f(sprite);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sequence;

    fn make_sprite(x: f32) -> Sprite {
        let mut s = Sprite::new();
        s.set_sequence(sequence::ast_small());
        s.x_pos = x;
        s.y_pos = 100.0;
        s.x_delta = 0.0;
        s.y_delta = 0.0;
        s
    }

    fn clip() -> Rect {
        Rect::new(0, 0, 2048, 1024)
    }

    #[test]
    fn append_order_is_iteration_order_and_removal_preserves_it() {
        let mut list = SpriteList::new();
        let a = list.add(make_sprite(1.0));
        let _b = list.add(make_sprite(2.0));
        let _c = list.add(make_sprite(3.0));
        list.destroy(a);
        let xs: Vec<f32> = list.iter().map(|s| s.x_pos).collect();
        assert_eq!(xs, vec![2.0, 3.0]);
    }

    #[test]
    fn timeout_sprite_removes_itself_without_skipping_successor() {
        let mut rand = Rand::new();
        let mut list = SpriteList::new();
        let mut doomed = make_sprite(1.0);
        doomed.duration = 1;
        list.add(doomed);
        let mut mover = make_sprite(2.0);
        mover.x_delta = 5.0;
        list.add(mover);

        list.update(&clip(), &mut rand); // both live (num_moved -> 1)
        assert_eq!(list.len(), 2);
        list.update(&clip(), &mut rand); // doomed destroys itself mid-pass
                                         // Doomed sprite is gone AND the mover still updated this pass.
        assert_eq!(list.len(), 1);
        assert_eq!(list.get(0).x_pos, 12.0);
    }

    #[test]
    fn checksum_accumulates_in_order() {
        let mut list = SpriteList::new();
        list.add(make_sprite(1.5));
        list.add(make_sprite(2.5));
        let expected = list.get(0).check(false) + list.get(1).check(false);
        assert_eq!(list.check().to_bits(), expected.to_bits());
    }

    #[test]
    fn count_visible_skips_hidden() {
        let mut list = SpriteList::new();
        list.add(make_sprite(1.0));
        let hidden = list.add(make_sprite(2.0));
        list.by_id_mut(hidden).unwrap().visible = false;
        assert_eq!(list.len(), 2);
        assert_eq!(list.count_visible(), 1);
    }
}
