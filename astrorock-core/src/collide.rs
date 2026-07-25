//! # Player-vs-object collision orchestration
//!
//! The port of `PlayersCollideObject` (`players.cpp`) for the single
//! local player: the ship hull, its current shot pool, and its bombs
//! each walk an object system's sprite pools. Handler order is the
//! original's — the object takes the collider's damage first, then the
//! collider takes the object's contact damage (shields make the ship
//! deal 1000 and take nothing; shots vanish on hit; bombs sail on).

use crate::bombers::{Bombers, BOMBER_COLLIDE_DAMAGE};
use crate::events::Events;
use crate::explosion::Explosions;
use crate::fastdeaths::{FastDeaths, FAST_DEATH_COLLIDE_DAMAGE};
use crate::gloops::{Gloops, GLOOP_COLLIDE_DAMAGE};
use crate::goodies::Goodies;
use crate::hks::{Hks, HK_COLLIDE_DAMAGE};
use crate::pship::PlayerShip;
use crate::rand::Rand;
use crate::rect::Rect;
use crate::rocks::Rocks;
use crate::spikeballs::{SpikeBalls, SPIKEBALL_COLLIDE_DAMAGE};
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
    /// Little-rock deaths roll goody drops inline (`AddGoody`).
    pub goodies: &'a mut Goodies,
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

/// `PlayersCollideObject(&HKs, 1)`. HK shots collide before HK bodies
/// (`HKsCollideSprite`/`List` order); an HK shot that hits anything
/// explodes via `HideSprite` → `ExploSprite`. Returns true if the ship
/// died.
pub fn player_vs_hks(ship: &mut PlayerShip, hks: &mut Hks, ctx: &mut CollideCtx) -> bool {
    let mut died = false;

    // Hull pass. HK shots first ("check shots even after all the hks
    // are dead"), then HK bodies.
    if ship.sprite.visible {
        let shield = ship.shield_on;
        if hks.engaged() {
            for pi in 0..hks.shots.len() {
                for si in 0..hks.shots[pi].len() {
                    let hit = {
                        let shot = hks.shots[pi].get(si);
                        shot.visible && shot.collide_sprite(&ship.sprite, &ctx.clip)
                    };
                    if hit {
                        ctx.explosions.explo_sprite(
                            hks.shots[pi].get_mut(si),
                            ctx.world,
                            ctx.events,
                        );
                        if !shield {
                            if damage_player(ship, hks.shot_damage, ctx) {
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
        if hks.active() {
            let dmg_to_hk = if shield {
                SHIP_SHIELD_DAMAGE
            } else {
                SHIP_COLLIDE_DAMAGE
            };
            for i in 0..hks.pool().len() {
                let hit = {
                    let h = &hks.pool()[i];
                    h.visible && h.collide_sprite(&ship.sprite, &ctx.clip)
                };
                if hit {
                    let score = hks.damage(i, dmg_to_hk, ctx.world, ctx.explosions, ctx.events);
                    ship.add_score(score);
                    if !shield {
                        if damage_player(ship, HK_COLLIDE_DAMAGE, ctx) {
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

    // Player shot pool pass: HK shots can be shot down (both vanish,
    // the HK shot exploding), then HK bodies take shot damage.
    let shot_damage = ship.shots[ship.cur_shots].damage;
    if hks.engaged() {
        for pi in 0..hks.shots.len() {
            for hs in 0..hks.shots[pi].len() {
                for ps in 0..15usize {
                    let hit = {
                        let hk_shot = hks.shots[pi].get(hs);
                        let player_shot = ship.shots[ship.cur_shots].iter().nth(ps).expect("pool");
                        hk_shot.visible
                            && player_shot.visible
                            && hk_shot.collide_sprite(player_shot, &ctx.clip)
                    };
                    if hit {
                        ctx.explosions.explo_sprite(
                            hks.shots[pi].get_mut(hs),
                            ctx.world,
                            ctx.events,
                        );
                        let cur = ship.cur_shots;
                        ship.shots[cur].hide(ps);
                    }
                }
            }
        }
    }
    if hks.active() {
        for hi in 0..hks.pool().len() {
            for ps in 0..15usize {
                let hit = {
                    let h = &hks.pool()[hi];
                    let player_shot = ship.shots[ship.cur_shots].iter().nth(ps).expect("pool");
                    h.visible && player_shot.visible && h.collide_sprite(player_shot, &ctx.clip)
                };
                if hit {
                    let score = hks.damage(hi, shot_damage, ctx.world, ctx.explosions, ctx.events);
                    ship.add_score(score);
                    let cur = ship.cur_shots;
                    ship.shots[cur].hide(ps);
                }
            }
        }
    }

    // Bomb pass: HK shots explode on bombs (bomb sails on), HK bodies
    // take bomb damage.
    let bomb_damage = ship.bombs.damage;
    if hks.engaged() {
        for pi in 0..hks.shots.len() {
            for hs in 0..hks.shots[pi].len() {
                let hit = {
                    let hk_shot = hks.shots[pi].get(hs);
                    hk_shot.visible
                        && ship
                            .bombs
                            .iter()
                            .any(|b| b.visible && hk_shot.collide_sprite(b, &ctx.clip))
                };
                if hit {
                    ctx.explosions
                        .explo_sprite(hks.shots[pi].get_mut(hs), ctx.world, ctx.events);
                }
            }
        }
    }
    if hks.active() {
        for hi in 0..hks.pool().len() {
            let hit = {
                let h = &hks.pool()[hi];
                h.visible
                    && ship
                        .bombs
                        .iter()
                        .any(|b| b.visible && h.collide_sprite(b, &ctx.clip))
            };
            if hit {
                let score = hks.damage(hi, bomb_damage, ctx.world, ctx.explosions, ctx.events);
                ship.add_score(score);
            }
        }
    }

    died
}

/// `PlayersCollideObject(&Bombers, 1)`. Bomber bombs collide before
/// bomber bodies; the bombs are destructible (`DamageBomb`) and damage
/// the ship on contact. Returns true if the ship died.
pub fn player_vs_bombers(
    ship: &mut PlayerShip,
    bombers: &mut Bombers,
    ctx: &mut CollideCtx,
) -> bool {
    let mut died = false;

    // Hull pass: bombs first, then bomber bodies.
    if ship.sprite.visible {
        let shield = ship.shield_on;
        let dmg_dealt = if shield {
            SHIP_SHIELD_DAMAGE
        } else {
            SHIP_COLLIDE_DAMAGE
        };
        if bombers.engaged() {
            for pi in 0..bombers.bombs.len() {
                for bi in 0..bombers.bombs[pi].len() {
                    let (hit, bomb_damage) = {
                        let bomb = bombers.bombs[pi].get(bi);
                        (
                            bomb.visible && bomb.collide_sprite(&ship.sprite, &ctx.clip),
                            bombers.bombs[pi].damage,
                        )
                    };
                    if hit {
                        bombers.bombs[pi].damage_bomb(
                            bi,
                            dmg_dealt,
                            ctx.world,
                            ctx.explosions,
                            ctx.events,
                        );
                        if !shield {
                            if damage_player(ship, bomb_damage, ctx) {
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
        if bombers.active() {
            for i in 0..bombers.pool().len() {
                let hit = {
                    let b = &bombers.pool()[i];
                    b.visible && b.collide_sprite(&ship.sprite, &ctx.clip)
                };
                if hit {
                    let score = bombers.damage(i, dmg_dealt, ctx.world, ctx.explosions, ctx.events);
                    ship.add_score(score);
                    if !shield {
                        if damage_player(ship, BOMBER_COLLIDE_DAMAGE, ctx) {
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

    // Player shot pass: shots chip bomber bombs, then bomber bodies.
    let shot_damage = ship.shots[ship.cur_shots].damage;
    if bombers.engaged() {
        for pi in 0..bombers.bombs.len() {
            for bi in 0..bombers.bombs[pi].len() {
                for ps in 0..15usize {
                    let hit = {
                        let bomb = bombers.bombs[pi].get(bi);
                        let player_shot = ship.shots[ship.cur_shots].iter().nth(ps).expect("pool");
                        bomb.visible
                            && player_shot.visible
                            && bomb.collide_sprite(player_shot, &ctx.clip)
                    };
                    if hit {
                        bombers.bombs[pi].damage_bomb(
                            bi,
                            shot_damage,
                            ctx.world,
                            ctx.explosions,
                            ctx.events,
                        );
                        let cur = ship.cur_shots;
                        ship.shots[cur].hide(ps);
                    }
                }
            }
        }
    }
    if bombers.active() {
        for i in 0..bombers.pool().len() {
            for ps in 0..15usize {
                let hit = {
                    let b = &bombers.pool()[i];
                    let player_shot = ship.shots[ship.cur_shots].iter().nth(ps).expect("pool");
                    b.visible && player_shot.visible && b.collide_sprite(player_shot, &ctx.clip)
                };
                if hit {
                    let score =
                        bombers.damage(i, shot_damage, ctx.world, ctx.explosions, ctx.events);
                    ship.add_score(score);
                    let cur = ship.cur_shots;
                    ship.shots[cur].hide(ps);
                }
            }
        }
    }

    // Player bomb pass.
    let player_bomb_damage = ship.bombs.damage;
    if bombers.engaged() {
        for pi in 0..bombers.bombs.len() {
            for bi in 0..bombers.bombs[pi].len() {
                let hit = {
                    let bomb = bombers.bombs[pi].get(bi);
                    bomb.visible
                        && ship
                            .bombs
                            .iter()
                            .any(|b| b.visible && bomb.collide_sprite(b, &ctx.clip))
                };
                if hit {
                    bombers.bombs[pi].damage_bomb(
                        bi,
                        player_bomb_damage,
                        ctx.world,
                        ctx.explosions,
                        ctx.events,
                    );
                }
            }
        }
    }
    if bombers.active() {
        for i in 0..bombers.pool().len() {
            let hit = {
                let b = &bombers.pool()[i];
                b.visible
                    && ship
                        .bombs
                        .iter()
                        .any(|pb| pb.visible && b.collide_sprite(pb, &ctx.clip))
            };
            if hit {
                let score =
                    bombers.damage(i, player_bomb_damage, ctx.world, ctx.explosions, ctx.events);
                ship.add_score(score);
            }
        }
    }

    died
}

/// `PlayersCollideObject(&SpikeBalls, 1)`. The blast-box (`DoBang`)
/// check runs first against the hull; body collide runs even at zero
/// count in the sprite path (the original wraps only the bang loop in
/// `if (NumSpikeBalls)`). Returns true if the ship died.
pub fn player_vs_spikeballs(
    ship: &mut PlayerShip,
    spikeballs: &mut SpikeBalls,
    ctx: &mut CollideCtx,
) -> bool {
    let mut died = false;

    if ship.sprite.visible {
        let shield = ship.shield_on;
        let dmg_dealt = if shield {
            SHIP_SHIELD_DAMAGE
        } else {
            SHIP_COLLIDE_DAMAGE
        };

        // One-beat blast boxes.
        if spikeballs.active() {
            for i in 0..spikeballs.slots() {
                if let Some(rect) = spikeballs.bang_rect(i) {
                    if ship.sprite.collide_rect(&rect) && !shield {
                        if damage_player(ship, spikeballs.bang_damage, ctx) {
                            died = true;
                        }
                        if !ship.sprite.visible {
                            return died;
                        }
                    }
                }
            }
        }

        // Body contact (unconditional in the sprite path).
        for i in 0..spikeballs.pool().len() {
            let hit = {
                let b = &spikeballs.pool()[i];
                b.visible && b.collide_sprite(&ship.sprite, &ctx.clip)
            };
            if hit {
                let score = spikeballs.damage(
                    i,
                    dmg_dealt,
                    ctx.net_rand,
                    ctx.world,
                    ctx.explosions,
                    ctx.events,
                );
                ship.add_score(score);
                if !shield {
                    if damage_player(ship, SPIKEBALL_COLLIDE_DAMAGE, ctx) {
                        died = true;
                    }
                    if !ship.sprite.visible {
                        return died;
                    }
                }
            }
        }
    }

    // Player shots.
    let shot_damage = ship.shots[ship.cur_shots].damage;
    for bi in 0..spikeballs.pool().len() {
        for ps in 0..15usize {
            let hit = {
                let b = &spikeballs.pool()[bi];
                let player_shot = ship.shots[ship.cur_shots].iter().nth(ps).expect("pool");
                b.visible && player_shot.visible && b.collide_sprite(player_shot, &ctx.clip)
            };
            if hit {
                let score = spikeballs.damage(
                    bi,
                    shot_damage,
                    ctx.net_rand,
                    ctx.world,
                    ctx.explosions,
                    ctx.events,
                );
                ship.add_score(score);
                let cur = ship.cur_shots;
                ship.shots[cur].hide(ps);
            }
        }
    }

    // Player bombs.
    let bomb_damage = ship.bombs.damage;
    for bi in 0..spikeballs.pool().len() {
        let hit = {
            let b = &spikeballs.pool()[bi];
            b.visible
                && ship
                    .bombs
                    .iter()
                    .any(|pb| pb.visible && b.collide_sprite(pb, &ctx.clip))
        };
        if hit {
            let score = spikeballs.damage(
                bi,
                bomb_damage,
                ctx.net_rand,
                ctx.world,
                ctx.explosions,
                ctx.events,
            );
            ship.add_score(score);
        }
    }

    died
}

/// `PlayersCollideObject(&FastDeaths, 1)`. Returns true if the ship
/// died.
pub fn player_vs_fastdeaths(
    ship: &mut PlayerShip,
    fastdeaths: &mut FastDeaths,
    ctx: &mut CollideCtx,
) -> bool {
    let mut died = false;

    if ship.sprite.visible {
        let shield = ship.shield_on;
        let dmg_dealt = if shield {
            SHIP_SHIELD_DAMAGE
        } else {
            SHIP_COLLIDE_DAMAGE
        };
        for i in 0..fastdeaths.pool().len() {
            let hit = {
                let f = &fastdeaths.pool()[i];
                f.visible && f.collide_sprite(&ship.sprite, &ctx.clip)
            };
            if hit {
                let score = fastdeaths.damage(i, dmg_dealt, ctx.world, ctx.explosions, ctx.events);
                ship.add_score(score);
                if !shield {
                    if damage_player(ship, FAST_DEATH_COLLIDE_DAMAGE, ctx) {
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
    for fi in 0..fastdeaths.pool().len() {
        for ps in 0..15usize {
            let hit = {
                let f = &fastdeaths.pool()[fi];
                let player_shot = ship.shots[ship.cur_shots].iter().nth(ps).expect("pool");
                f.visible && player_shot.visible && f.collide_sprite(player_shot, &ctx.clip)
            };
            if hit {
                let score =
                    fastdeaths.damage(fi, shot_damage, ctx.world, ctx.explosions, ctx.events);
                ship.add_score(score);
                let cur = ship.cur_shots;
                ship.shots[cur].hide(ps);
            }
        }
    }

    let bomb_damage = ship.bombs.damage;
    for fi in 0..fastdeaths.pool().len() {
        let hit = {
            let f = &fastdeaths.pool()[fi];
            f.visible
                && ship
                    .bombs
                    .iter()
                    .any(|pb| pb.visible && f.collide_sprite(pb, &ctx.clip))
        };
        if hit {
            let score = fastdeaths.damage(fi, bomb_damage, ctx.world, ctx.explosions, ctx.events);
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
        _ => rocks.damage_lit(
            i,
            damage,
            ctx.goodies,
            ctx.net_rand,
            ctx.world,
            ctx.explosions,
            ctx.events,
        ),
    }
}
