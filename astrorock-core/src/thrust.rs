//! # Thrust flames — port of `thrust.cpp`
//!
//! Five flame sequences (animation frames of the exhaust); each draw
//! picks flame `frame` and rotation `rotation` (the ship's facing
//! index) and stamps it at the ship's position.

use crate::frame::Frame;
use crate::sequence;
use crate::sprite::Sprite;
use crate::virtual_frame::VirtualFrame;

pub const NUM_THRUSTS: usize = 5;

pub struct Thrust {
    sprites: Vec<Sprite>,
}

impl Thrust {
    /// `CThrustInit`.
    pub fn new() -> Self {
        let seqs = [
            sequence::thrust0(),
            sequence::thrust1(),
            sequence::thrust2(),
            sequence::thrust3(),
            sequence::thrust4(),
        ];
        let sprites = seqs
            .into_iter()
            .map(|seq| {
                let mut s = Sprite::new();
                s.set_sequence(seq);
                s
            })
            .collect();
        Self { sprites }
    }

    /// `CThrustMoveTo` + `CThrustDraw`.
    pub fn draw(
        &mut self,
        world: &VirtualFrame,
        screen: &mut Frame,
        rotation: i32,
        frame: usize,
        x: i32,
        y: i32,
    ) {
        let s = &mut self.sprites[frame];
        s.cur_frame = rotation as f32;
        s.x_pos = x as f32;
        s.y_pos = y as f32;
        s.draw(world, screen);
    }
}

impl Default for Thrust {
    fn default() -> Self {
        Self::new()
    }
}
