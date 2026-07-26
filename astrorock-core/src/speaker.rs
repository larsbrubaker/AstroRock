//! # The speaker — port of `pSpeakerSprite` (`AstroRock.cpp`)
//!
//! An indestructible jukebox drifting through the world. Anything
//! that touches it dies — every enemy list collides against it with
//! 20000 damage (`CollideSpriteFunc(20000, pSpeakerSprite, 0)`, the
//! scores discarded like the original's ignored returns) — while the
//! player merely takes a 10-point bump (a shield spark instead when
//! shielded). Bumping OR shooting it triggers `SkipMusic`: sustained
//! contact drops the soundtrack to 7025 Hz like a finger on the
//! record, recovering 90 counts after the last touch.
//!
//! `ResetAll` repositions it randomly each level; it's never hidden
//! (the original asserts `GetVisible()` around every use).

use crate::bombers::Bombers;
use crate::collide::{damage_rock, rock_at, rock_count, CollideCtx};
use crate::fastdeaths::FastDeaths;
use crate::gloops::Gloops;
use crate::hks::Hks;
use crate::pship::PlayerShip;
use crate::rand::Rand;
use crate::rect::Rect;
use crate::rocks::Rocks;
use crate::sequence;
use crate::spikeballs::SpikeBalls;
use crate::sprite::Sprite;

/// Contact damage dealt BY the speaker to enemies and rocks.
const SPEAKER_TOUCH_DAMAGE: u32 = 20000;
/// Contact damage taken by an unshielded player.
const SPEAKER_PLAYER_DAMAGE: u32 = 10;
/// The radar blip color (`RadarDrawOn(pSpeakerSprite, 144)`).
pub const SPEAKER_RADAR_COLOR: u8 = 144;

pub struct Speaker {
    pub sprite: Sprite,
}

impl Speaker {
    pub fn new() -> Self {
        let mut sprite = Sprite::new();
        sprite.set_sequence(sequence::speak());
        sprite.reset();
        Self { sprite }
    }

    /// The `ResetAll` tail: `Reset()` then a NetRand position.
    pub fn reset(&mut self, world_w: u32, world_h: u32, net_rand: &mut Rand) {
        self.sprite.reset();
        self.sprite.x_pos = net_rand.rand(world_w) as f32;
        self.sprite.y_pos = net_rand.rand(world_h) as f32;
    }

    /// `pSpeakerSprite->Update()`.
    pub fn update(&mut self, clip: &Rect, rand: &mut Rand) {
        let _ = self.sprite.update(clip, rand);
    }
}

impl Default for Speaker {
    fn default() -> Self {
        Self::new()
    }
}

/// The enemy-vs-speaker pass, in the original call order: Rocks,
/// Gloops, SpikeBalls, HKs, Bombers, FastDeaths each take 20000 on
/// contact (nobody is awarded the score).
#[allow(clippy::too_many_arguments)]
pub fn speaker_vs_world(
    speaker: &Speaker,
    rocks: &mut Rocks,
    gloops: &mut Gloops,
    spikeballs: &mut SpikeBalls,
    hks: &mut Hks,
    bombers: &mut Bombers,
    fastdeaths: &mut FastDeaths,
    ctx: &mut CollideCtx,
) {
    let sp = &speaker.sprite;

    for class in 0..3usize {
        for i in 0..rock_count(rocks, class) {
            let hit = {
                let rock = rock_at(rocks, class, i);
                rock.visible && rock.collide_sprite(sp, &ctx.clip)
            };
            if hit {
                let _ = damage_rock(rocks, class, i, SPEAKER_TOUCH_DAMAGE, ctx);
            }
        }
    }
    for i in 0..gloops.pool().len() {
        if gloops.pool()[i].visible && gloops.pool()[i].collide_sprite(sp, &ctx.clip) {
            let _ = gloops.damage(
                i,
                SPEAKER_TOUCH_DAMAGE,
                ctx.world,
                ctx.explosions,
                ctx.events,
            );
        }
    }
    for i in 0..spikeballs.pool().len() {
        if spikeballs.pool()[i].visible && spikeballs.pool()[i].collide_sprite(sp, &ctx.clip) {
            let _ = spikeballs.damage(
                i,
                SPEAKER_TOUCH_DAMAGE,
                ctx.net_rand,
                ctx.world,
                ctx.explosions,
                ctx.events,
            );
        }
    }
    for i in 0..hks.pool().len() {
        if hks.pool()[i].visible && hks.pool()[i].collide_sprite(sp, &ctx.clip) {
            let _ = hks.damage(
                i,
                SPEAKER_TOUCH_DAMAGE,
                ctx.world,
                ctx.explosions,
                ctx.events,
            );
        }
    }
    for i in 0..bombers.pool().len() {
        if bombers.pool()[i].visible && bombers.pool()[i].collide_sprite(sp, &ctx.clip) {
            let _ = bombers.damage(
                i,
                SPEAKER_TOUCH_DAMAGE,
                ctx.world,
                ctx.explosions,
                ctx.events,
            );
        }
    }
    for i in 0..fastdeaths.pool().len() {
        if fastdeaths.pool()[i].visible && fastdeaths.pool()[i].collide_sprite(sp, &ctx.clip) {
            let _ = fastdeaths.damage(
                i,
                SPEAKER_TOUCH_DAMAGE,
                ctx.world,
                ctx.explosions,
                ctx.events,
            );
        }
    }
}

/// `Players.CollideSpriteFunc(10, pSpeakerSprite, SkipMusic)` for the
/// single local player: hull contact costs 10 HP (a shield spark when
/// shielded — `PlayShotHit`), and shots/bombs spark against the grille
/// without harming it. Returns (touched — the caller runs `SkipMusic`
/// per touch, died).
pub fn player_vs_speaker(
    ship: &mut PlayerShip,
    speaker: &Speaker,
    ctx: &mut CollideCtx,
) -> (bool, bool) {
    let sp = &speaker.sprite;
    let mut touched = false;
    let mut died = false;

    if ship.sprite.visible && sp.collide_sprite(&ship.sprite, &ctx.clip) {
        touched = true;
        if ship.shield_on {
            let (x, y) = (ship.sprite.x_pos, ship.sprite.y_pos);
            ctx.explosions.play_shot_hit_at(x, y, ctx.world, ctx.events);
        } else if crate::collide::damage_player(ship, SPEAKER_PLAYER_DAMAGE, ctx) {
            died = true;
        }
    }

    // Shots spark on the grille (`PlayShotHit` — the shot itself is
    // not consumed, exactly like the original handler).
    let cur = ship.cur_shots;
    for si in 0..15usize {
        let hit_at = {
            let shot = ship.shots[cur].iter().nth(si).expect("pool");
            (shot.visible && sp.collide_sprite(shot, &ctx.clip)).then_some((shot.x_pos, shot.y_pos))
        };
        if let Some((x, y)) = hit_at {
            touched = true;
            ctx.explosions.play_shot_hit_at(x, y, ctx.world, ctx.events);
        }
    }
    let bomb_hits: Vec<(f32, f32)> = ship
        .bombs
        .iter()
        .filter(|b| b.visible && sp.collide_sprite(b, &ctx.clip))
        .map(|b| (b.x_pos, b.y_pos))
        .collect();
    for (x, y) in bomb_hits {
        touched = true;
        ctx.explosions.play_shot_hit_at(x, y, ctx.world, ctx.events);
    }

    (touched, died)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::Events;
    use crate::explosion::Explosions;
    use crate::goodies::Goodies;
    use crate::intermission::LevelStats;
    use crate::virtual_frame::VirtualFrame;

    fn ctx_parts() -> (VirtualFrame, Explosions, Events, Rand, Goodies, LevelStats) {
        let mut world = VirtualFrame::new(2048, 1024);
        world.set_on_screen_rect(Rect::new(0, 0, 640, 384));
        (
            world,
            Explosions::new(),
            Events::new(),
            Rand::new(),
            Goodies::new(),
            LevelStats::new(),
        )
    }

    #[test]
    fn touching_the_speaker_hurts_the_player_not_the_speaker() {
        let (world, mut ex, mut ev, mut nr, mut go, mut st) = ctx_parts();
        let mut ctx = CollideCtx {
            world: &world,
            explosions: &mut ex,
            events: &mut ev,
            net_rand: &mut nr,
            goodies: &mut go,
            stats: &mut st,
            clip: Rect::new(0, 0, 2048, 1024),
            ship_immune: false,
        };
        let mut speaker = Speaker::new();
        speaker.sprite.x_pos = 500.0;
        speaker.sprite.y_pos = 500.0;

        let mut ship = PlayerShip::new();
        ship.reset(3);
        ship.sprite.visible = true;
        ship.sprite.x_pos = 500.0;
        ship.sprite.y_pos = 500.0;
        let hp = ship.sprite.hp;

        let (touched, died) = player_vs_speaker(&mut ship, &speaker, &mut ctx);
        assert!(touched, "overlapping ship should touch");
        assert!(!died);
        assert_eq!(ship.sprite.hp, hp - SPEAKER_PLAYER_DAMAGE);
        assert!(speaker.sprite.visible, "the speaker is indestructible");
    }

    #[test]
    fn rocks_die_on_the_grille() {
        let (world, mut ex, mut ev, mut nr, mut go, mut st) = ctx_parts();
        let mut rocks = Rocks::new();
        rocks.reset(0, &mut nr);
        let mut ctx = CollideCtx {
            world: &world,
            explosions: &mut ex,
            events: &mut ev,
            net_rand: &mut nr,
            goodies: &mut go,
            stats: &mut st,
            clip: Rect::new(0, 0, 2048, 1024),
            ship_immune: false,
        };
        let mut speaker = Speaker::new();
        let idx = rocks.big().iter().position(|s| s.visible).expect("a rock");
        speaker.sprite.x_pos = rocks.big()[idx].x_pos;
        speaker.sprite.y_pos = rocks.big()[idx].y_pos;
        let before = rocks.num_big;

        let mut gloops = Gloops::new();
        let mut spikeballs = SpikeBalls::new();
        let mut hks = Hks::new();
        let mut bombers = Bombers::new();
        let mut fastdeaths = FastDeaths::new();
        speaker_vs_world(
            &speaker,
            &mut rocks,
            &mut gloops,
            &mut spikeballs,
            &mut hks,
            &mut bombers,
            &mut fastdeaths,
            &mut ctx,
        );
        assert!(
            rocks.num_big < before,
            "20000 damage should shatter the touching rock"
        );
    }
}
