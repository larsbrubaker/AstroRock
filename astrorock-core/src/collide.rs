//! # Player-vs-object collision orchestration
//!
//! The port of `PlayersCollideObject` (`players.cpp`) for the single
//! local player: the ship hull, its current shot pool, and its bombs
//! each walk an object system's sprite pools. Handler order is the
//! original's — the object takes the collider's damage first, then the
//! collider takes the object's contact damage (shields make the ship
//! deal 1000 and take nothing; shots vanish on hit; bombs sail on).

use crate::events::Events;
use crate::explosion::Explosions;
use crate::gloops::{Gloops, GLOOP_COLLIDE_DAMAGE};
use crate::pship::PlayerShip;
use crate::rand::Rand;
use crate::rect::Rect;
use crate::rocks::Rocks;
use crate::virtual_frame::VirtualFrame;

/// `#define SHIPCOLLIDEDAMAGE 50`
pub const SHIP_COLLIDE_DAMAGE: u32 = 50;
/// `#define SHIPSHIELDDAMAGE 1000`
pub const SHIP_SHIELD_DAMAGE: u32 = 1000;
/// `BIG/MED/LITCOLLIDEDAMAGE` — rock contact damage by class.
pub const BIG_COLLIDE_DAMAGE: u32 = 150;
pub const MED_COLLIDE_DAMAGE: u32 = 100;
pub const LIT_COLLIDE_DAMAGE: u32 = 50;

/// Shared mutable context for one collision pass.
pub struct CollideCtx<'a> {
    pub world: &'a VirtualFrame,
    pub explosions: &'a mut Explosions,
    pub events: &'a mut Events,
    pub net_rand: &'a mut Rand,
    pub clip: Rect,
}

/// `DamagePlayer`/`KillPlayer` — returns true if the ship died.
pub fn damage_player(ship: &mut PlayerShip, damage: u32, ctx: &mut CollideCtx) -> bool {
    if damage >= ship.sprite.hp {
        ship.sprite.hp = 0;
        ctx.explosions
            .explo_sprite(&mut ship.sprite, ctx.world, ctx.events);
        ship.remove_ship();
        true
    } else {
        ship.sprite.hp -= damage;
        false
    }
}

/// `PlayersCollideObject(&Rocks, 1)`. Returns true if the ship died.
pub fn player_vs_rocks(ship: &mut PlayerShip, rocks: &mut Rocks, ctx: &mut CollideCtx) -> bool {
    let mut died = false;

    // Ship hull vs rocks.
    if ship.sprite.visible {
        let shield = ship.shield_on;
        let dmg_to_rock = if shield {
            SHIP_SHIELD_DAMAGE
        } else {
            SHIP_COLLIDE_DAMAGE
        };
        for class in 0..3usize {
            let count = rock_count(rocks, class);
            for i in 0..count {
                let hit = {
                    let rock = rock_at(rocks, class, i);
                    rock.visible && rock.collide_sprite(&ship.sprite, &ctx.clip)
                };
                if hit {
                    let score = damage_rock(rocks, class, i, dmg_to_rock, ctx);
                    ship.add_score(score);
                    if !shield {
                        let contact =
                            [BIG_COLLIDE_DAMAGE, MED_COLLIDE_DAMAGE, LIT_COLLIDE_DAMAGE][class];
                        if damage_player(ship, contact, ctx) {
                            died = true;
                        }
                        if !ship.sprite.visible {
                            return died;
                        }
                    }
                }
            }
        }
    }

    // Current shot pool vs rocks (rocks outer, shots inner, like the
    // list walk); a hit hides the shot (`TallyShotHits`).
    let shot_damage = ship.shots[ship.cur_shots].damage;
    for class in 0..3usize {
        let rock_total = rock_count(rocks, class);
        for ri in 0..rock_total {
            for si in 0..15usize {
                let hit = {
                    let rock = rock_at(rocks, class, ri);
                    let shot = ship.shots[ship.cur_shots].iter().nth(si).expect("pool");
                    rock.visible && shot.visible && rock.collide_sprite(shot, &ctx.clip)
                };
                if hit {
                    let score = damage_rock(rocks, class, ri, shot_damage, ctx);
                    ship.add_score(score);
                    let cur = ship.cur_shots;
                    ship.shots[cur].hide(si);
                }
            }
        }
    }

    // Bombs vs rocks (bomb unaffected).
    let bomb_damage = ship.bombs.damage;
    for class in 0..3usize {
        let rock_total = rock_count(rocks, class);
        for ri in 0..rock_total {
            let hit = {
                let rock = rock_at(rocks, class, ri);
                rock.visible
                    && ship
                        .bombs
                        .iter()
                        .any(|b| b.visible && rock.collide_sprite(b, &ctx.clip))
            };
            if hit {
                let score = damage_rock(rocks, class, ri, bomb_damage, ctx);
                ship.add_score(score);
            }
        }
    }

    died
}

/// `PlayersCollideObject(&Gloops, 1)`. Returns true if the ship died.
pub fn player_vs_gloops(ship: &mut PlayerShip, gloops: &mut Gloops, ctx: &mut CollideCtx) -> bool {
    if !gloops.active() {
        return false;
    }
    let mut died = false;

    if ship.sprite.visible {
        let shield = ship.shield_on;
        let dmg_to_gloop = if shield {
            SHIP_SHIELD_DAMAGE
        } else {
            SHIP_COLLIDE_DAMAGE
        };
        for i in 0..gloops.pool().len() {
            let hit = {
                let g = &gloops.pool()[i];
                g.visible && g.collide_sprite(&ship.sprite, &ctx.clip)
            };
            if hit {
                let score = gloops.damage(i, dmg_to_gloop, ctx.world, ctx.explosions, ctx.events);
                ship.add_score(score);
                if !shield {
                    if damage_player(ship, GLOOP_COLLIDE_DAMAGE, ctx) {
                        died = true;
                    }
                    if !ship.sprite.visible {
                        return died;
                    }
                }
            }
        }
    }

    let shot_damage = ship.shots[ship.cur_shots].damage;
    for gi in 0..gloops.pool().len() {
        for si in 0..15usize {
            let hit = {
                let g = &gloops.pool()[gi];
                let shot = ship.shots[ship.cur_shots].iter().nth(si).expect("pool");
                g.visible && shot.visible && g.collide_sprite(shot, &ctx.clip)
            };
            if hit {
                let score = gloops.damage(gi, shot_damage, ctx.world, ctx.explosions, ctx.events);
                ship.add_score(score);
                let cur = ship.cur_shots;
                ship.shots[cur].hide(si);
            }
        }
    }

    let bomb_damage = ship.bombs.damage;
    for gi in 0..gloops.pool().len() {
        let hit = {
            let g = &gloops.pool()[gi];
            g.visible
                && ship
                    .bombs
                    .iter()
                    .any(|b| b.visible && g.collide_sprite(b, &ctx.clip))
        };
        if hit {
            let score = gloops.damage(gi, bomb_damage, ctx.world, ctx.explosions, ctx.events);
            ship.add_score(score);
        }
    }

    died
}

fn rock_count(rocks: &Rocks, class: usize) -> usize {
    match class {
        0 => rocks.big().len(),
        1 => rocks.med().len(),
        _ => rocks.lit().len(),
    }
}

fn rock_at(rocks: &Rocks, class: usize, i: usize) -> &crate::sprite::Sprite {
    match class {
        0 => &rocks.big()[i],
        1 => &rocks.med()[i],
        _ => &rocks.lit()[i],
    }
}

fn damage_rock(
    rocks: &mut Rocks,
    class: usize,
    i: usize,
    damage: u32,
    ctx: &mut CollideCtx,
) -> u32 {
    match class {
        0 => rocks.damage_big(
            i,
            damage,
            ctx.net_rand,
            ctx.world,
            ctx.explosions,
            ctx.events,
        ),
        1 => rocks.damage_med(
            i,
            damage,
            ctx.net_rand,
            ctx.world,
            ctx.explosions,
            ctx.events,
        ),
        _ => rocks.damage_lit(i, damage, ctx.world, ctx.explosions, ctx.events),
    }
}
