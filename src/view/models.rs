//! The Warcraft III cast, rebuilt from primitives.
//!
//! `GREEN TD 9.3c PEIN.w3x` dresses its hundred and thirty-one towers and
//! thirty-six creeps in stock Warcraft III models - a Dread Lord, a Rock Golem,
//! an Arcane Observatory, a Snowman. Nothing here can load a `.mdl`, so every
//! one of them is rebuilt out of the eight primitives the renderer has:
//! spheres, capsules, cones, prisms, cylinders, pyramids, boxes and quads.
//!
//! Three rules, learned the hard way.
//!
//! **Detail is the whole job.** A figure made of six shapes reads as a chess
//! piece however well it is lit. These are built the way the originals are:
//! boots, greaves, belt, tabard, breastplate, pauldrons, neck, head, helm,
//! crest, bracers, hands, and a weapon with a grip, a guard, a blade and a
//! pommel. Thirty to sixty pieces, not six.
//!
//! **Colour is material, not identity.** Steel is grey, leather is brown, skin
//! is skin. Only the cloth, the crest, the banner and whatever glows take the
//! unit's own colour - which is its attack type for a tower and its armour type
//! for a creep. A model tinted end to end in one hue is a silhouette in a
//! coloured fog, and that is what these looked like before.
//!
//! **A crowd is not a portrait.** A wave is a hundred and fifty monsters on
//! forty pixels of road, so nothing in a creep model may cost what a tower's
//! can: no additive glow, no wing membranes, no buckles. [`Pose::fine`] is the
//! switch, and it is off for anything that arrives in numbers.
//!
//! Both [`super::towers`] and [`super::monsters`] draw through here; they only
//! add their own dressing - a plinth and a range ring, or a health bar.

use crate::game::defs::Model;
use crate::gfx::draw::{Color, DrawList, Material, Shape, mix, rgba};

/// Where and how big a model is drawn, and what its animation clock reads.
#[derive(Clone, Copy)]
pub struct Pose {
    pub pos: [f32; 2],
    /// Ground height the model stands on.
    pub z: f32,
    pub yaw: f32,
    /// Half-width, in tiles. Everything else is a multiple of this.
    pub r: f32,
    /// Animation clock, in radians.
    pub t: f32,
    /// Whether the legs are moving.
    pub walk: bool,
    /// Whether this instance may add additive glow sprites and the finer
    /// detail. A tower is one of a hundred; a monster is one of seven hundred.
    pub lights: bool,
}

impl Pose {
    /// Offset by a rotated local (forward, right) pair.
    pub fn at(&self, fwd: f32, right: f32) -> [f32; 2] {
        let (c, s) = (self.yaw.cos(), self.yaw.sin());
        [
            self.pos[0] + c * fwd - s * right,
            self.pos[1] + s * fwd + c * right,
        ]
    }
    pub fn p3(&self, fwd: f32, right: f32, up: f32) -> [f32; 3] {
        let p = self.at(fwd, right);
        [p[0], p[1], self.z + up]
    }
    /// An additive glow, if this instance is allowed one.
    ///
    /// Radius and power are both bounded here rather than at the call sites. A
    /// model asks for the glow it wants at its own scale; what it gets is
    /// something six of them standing in a row can still be told apart through.
    /// An additive highlight on a part: a gem, an eye, a rune, a flame.
    ///
    /// The cap used to be twice the model's own radius, which is not a cap at
    /// all - it let a builder ask for a sphere twice the size of the thing it
    /// was lighting, and every tower on the board rendered inside a translucent
    /// dome wider than its own plinth. Whatever a builder asks for, a glow may
    /// not grow past about half the model: past that it stops reading as
    /// something on the unit and starts reading as a bubble around it.
    fn glow(&self, d: &mut DrawList, at: [f32; 3], radius: f32, power: f32, col: Color) {
        if self.lights {
            d.glow(at, radius.min(self.r * 0.55), power.min(0.18), col);
        }
    }
    /// A piece of detail only a tower gets. A hundred and fifty creeps cannot
    /// each afford a belt buckle.
    #[inline]
    fn fine(&self) -> bool {
        self.lights
    }
}

/// The materials a model is built out of.
///
/// `body` is the unit's own colour - attack type for a tower, armour type for a
/// creep - and it appears on cloth, crests and gems only. Everything else is
/// the colour the material actually is.
#[derive(Clone, Copy)]
pub struct Skin {
    /// The identity colour. Cloth, banners, crests, gems.
    pub body: Color,
    /// A darker version of it, for trim and shadowed cloth.
    pub dark: Color,
    /// Bone, teeth, horn, tusk.
    pub bone: Color,
    /// What glows: eyes, runes, fire, magic.
    pub glow: Color,
    /// Flesh.
    pub skin: Color,
    /// Fabric. Takes some of the identity colour.
    pub cloth: Color,
    /// Straps, boots, hide.
    pub leather: Color,
    /// Armour, blades, fittings.
    pub steel: Color,
    /// Dark iron: buckles, rivets, chain.
    pub iron: Color,
    /// Timber: hafts, shields, carts.
    pub wood: Color,
}

impl Skin {
    /// The palette a model is built in, with `accent` - armour type for a
    /// creep, attack type for a tower - kept for the parts meant to carry it.
    ///
    /// See [`Hide`] for why species owns the palette and the accent does not.
    pub fn wearing(m: Model, accent: [f32; 3], flash: f32) -> Skin {
        let h = hide_of(m);
        let b = mix(accent, [1.0, 1.0, 1.0], flash * 0.5);
        let hit = |c: [f32; 3]| rgba(mix(c, [1.0, 0.86, 0.76], flash * 0.55), 1.0);
        Skin {
            // Banners, crests and gems - the only parts wearing the accent.
            body: rgba(b, 1.0),
            dark: rgba(mix(b, [0.04, 0.04, 0.06], 0.55), 1.0),
            bone: hit(BONE_C),
            // What glows is the species' own light, pulled a little towards the
            // accent so a Chaos tower still reads as Chaos across the board.
            glow: rgba(mix(h.lume, b, 0.30), 1.0),
            skin: hit(h.flesh),
            cloth: hit(mix(h.cloth, b, 0.22)),
            leather: hit(mix(h.cloth, [0.26, 0.18, 0.12], 0.45)),
            steel: hit(h.metal),
            iron: hit(mix(h.metal, [0.10, 0.10, 0.12], 0.55)),
            wood: hit([0.33, 0.23, 0.14]),
        }
    }

    pub fn of(base: [f32; 3], flash: f32) -> Skin {
        let b = mix(base, [1.0, 1.0, 1.0], flash * 0.75);
        let hit = |c: [f32; 3]| rgba(mix(c, [1.0, 0.85, 0.75], flash * 0.7), 1.0);
        Skin {
            body: rgba(b, 1.0),
            dark: rgba(mix(b, [0.04, 0.04, 0.06], 0.55), 1.0),
            bone: hit([0.58, 0.55, 0.44]),
            glow: rgba(mix(b, [1.0, 0.82, 0.48], 0.55), 1.0),
            skin: hit([0.47, 0.35, 0.27]),
            cloth: rgba(mix([0.34, 0.30, 0.28], b, 0.55), 1.0),
            leather: hit([0.28, 0.19, 0.13]),
            steel: hit([0.40, 0.42, 0.46]),
            iron: hit([0.20, 0.21, 0.24]),
            wood: hit([0.33, 0.23, 0.14]),
        }
    }
}

/// What a model is *made of*, in Warcraft III's own colours.
///
/// The first cut of this port had one rule - colour carries armour type for a
/// creep and attack type for a tower - and it is why a wave of three hundred
/// looked like three hundred of the same grey figurine. Armour type is one of
/// five colours, so an entire wave shared a palette, and a desaturated one: the
/// lane band measured rgb(84, 105, 91) with three hundred monsters on it.
///
/// Warcraft III does the opposite. An orc is green, a gnoll is tan, a naga is
/// teal, a skeleton is bone, and you read an army by its species. So species
/// owns the palette here, and the armour or attack colour is demoted to an
/// accent - a banner, a crest, a gem, whatever glows - which is a small part of
/// the silhouette and still says what you are shooting at.
///
/// `flesh` is skin, fur, scale, bark or a machine's body; `cloth` is fabric and
/// trim; `metal` is armour and blades; `lume` is what glows.
struct Hide {
    flesh: [f32; 3],
    cloth: [f32; 3],
    metal: [f32; 3],
    lume: [f32; 3],
}

const fn hide(flesh: [f32; 3], cloth: [f32; 3], metal: [f32; 3], lume: [f32; 3]) -> Hide {
    Hide {
        flesh,
        cloth,
        metal,
        lume,
    }
}

/// Shared metals, so armour and machinery do not all read alike.
const STEEL_C: [f32; 3] = [0.40, 0.42, 0.46];
const IRON_C: [f32; 3] = [0.23, 0.24, 0.27];
const BRONZE_C: [f32; 3] = [0.44, 0.33, 0.17];
const STONE_C: [f32; 3] = [0.40, 0.39, 0.35];
const BONE_C: [f32; 3] = [0.68, 0.65, 0.52];

fn hide_of(m: Model) -> Hide {
    use Model::*;
    match m {
        // ------------------------------------------------------------ people
        Acolyte => hide([0.55, 0.57, 0.49], [0.17, 0.13, 0.21], IRON_C, [0.45, 0.95, 0.55]),
        Archer => hide([0.60, 0.53, 0.68], [0.15, 0.35, 0.29], STEEL_C, [0.70, 0.95, 0.60]),
        Mage => hide([0.71, 0.55, 0.43], [0.19, 0.27, 0.56], STEEL_C, [0.45, 0.80, 1.00]),
        Warrior => hide([0.34, 0.49, 0.24], [0.36, 0.22, 0.14], IRON_C, [0.95, 0.70, 0.25]),
        Demon => hide([0.50, 0.19, 0.20], [0.16, 0.10, 0.14], IRON_C, [0.55, 1.00, 0.30]),
        Brute => hide([0.53, 0.38, 0.26], [0.36, 0.25, 0.16], BRONZE_C, [0.95, 0.75, 0.35]),
        Troll => hide([0.28, 0.45, 0.44], [0.46, 0.30, 0.16], BRONZE_C, [0.60, 1.00, 0.75]),
        Gnoll => hide([0.50, 0.40, 0.26], [0.37, 0.26, 0.17], IRON_C, [0.95, 0.80, 0.40]),
        Skeleton => hide(BONE_C, [0.18, 0.16, 0.15], IRON_C, [0.45, 1.00, 0.50]),
        Wraith => hide([0.47, 0.58, 0.72], [0.16, 0.19, 0.28], IRON_C, [0.50, 0.85, 1.00]),
        Naga => hide([0.19, 0.44, 0.46], [0.30, 0.22, 0.36], BRONZE_C, [0.40, 0.95, 1.00]),
        Rifleman => hide([0.73, 0.55, 0.41], [0.43, 0.21, 0.15], STEEL_C, [0.95, 0.80, 0.35]),
        Villager => hide([0.72, 0.56, 0.44], [0.54, 0.45, 0.33], IRON_C, [0.95, 0.85, 0.50]),
        Panda => hide([0.78, 0.76, 0.70], [0.24, 0.42, 0.33], BRONZE_C, [0.95, 0.75, 0.35]),
        // ------------------------------------------------------------ beasts
        Bear => hide([0.36, 0.26, 0.17], [0.30, 0.22, 0.15], BRONZE_C, [0.95, 0.80, 0.40]),
        Mammoth => hide([0.42, 0.32, 0.22], [0.38, 0.24, 0.15], BRONZE_C, [0.95, 0.85, 0.55]),
        Centaur => hide([0.43, 0.30, 0.20], [0.34, 0.23, 0.15], BRONZE_C, [0.95, 0.78, 0.35]),
        Lizard => hide([0.31, 0.44, 0.23], [0.40, 0.28, 0.16], BRONZE_C, [0.95, 0.75, 0.30]),
        Crab => hide([0.60, 0.30, 0.18], [0.34, 0.22, 0.16], BRONZE_C, [0.95, 0.70, 0.35]),
        Spider => hide([0.22, 0.16, 0.26], [0.16, 0.12, 0.18], IRON_C, [0.55, 1.00, 0.40]),
        Serpent => hide([0.23, 0.43, 0.28], [0.20, 0.30, 0.22], BRONZE_C, [0.70, 1.00, 0.50]),
        Turtle => hide([0.30, 0.38, 0.26], [0.34, 0.30, 0.20], BRONZE_C, [0.60, 0.95, 0.60]),
        Ent => hide([0.30, 0.24, 0.16], [0.23, 0.42, 0.19], BRONZE_C, [0.70, 1.00, 0.45]),
        Golem => hide(STONE_C, [0.30, 0.28, 0.24], IRON_C, [0.60, 0.85, 1.00]),
        Giant => hide([0.35, 0.42, 0.46], [0.26, 0.30, 0.32], STEEL_C, [0.55, 0.85, 1.00]),
        Infernal => hide([0.25, 0.21, 0.19], [0.20, 0.14, 0.12], IRON_C, [1.00, 0.50, 0.15]),
        FlameLord => hide([0.55, 0.22, 0.10], [0.35, 0.15, 0.08], IRON_C, [1.00, 0.62, 0.18]),
        // ------------------------------------------------------------- wings
        Gyrocopter => hide([0.44, 0.36, 0.24], [0.40, 0.30, 0.18], STEEL_C, [0.95, 0.80, 0.35]),
        Phoenix => hide([0.80, 0.42, 0.12], [0.60, 0.26, 0.10], BRONZE_C, [1.00, 0.72, 0.25]),
        Harpy => hide([0.44, 0.30, 0.50], [0.30, 0.20, 0.34], IRON_C, [0.85, 0.60, 1.00]),
        Dragon => hide([0.52, 0.39, 0.20], [0.38, 0.28, 0.16], BRONZE_C, [0.95, 0.78, 0.30]),
        FrostWyrm => hide([0.60, 0.70, 0.79], [0.28, 0.36, 0.44], STEEL_C, [0.55, 0.88, 1.00]),
        // ---------------------------------------------------------- machines
        Turret => hide(STEEL_C, [0.30, 0.28, 0.26], IRON_C, [0.95, 0.75, 0.30]),
        Turbolazer => hide([0.38, 0.42, 0.48], [0.24, 0.28, 0.34], STEEL_C, [0.45, 0.85, 1.00]),
        RebelTurret => hide([0.42, 0.38, 0.30], [0.34, 0.26, 0.18], IRON_C, [1.00, 0.65, 0.25]),
        Vulcan => hide([0.34, 0.36, 0.40], [0.26, 0.26, 0.28], IRON_C, [1.00, 0.60, 0.20]),
        SamSite => hide([0.40, 0.42, 0.44], [0.30, 0.32, 0.34], STEEL_C, [1.00, 0.35, 0.25]),
        Cannon => hide([0.30, 0.31, 0.34], [0.36, 0.26, 0.16], IRON_C, [1.00, 0.62, 0.20]),
        MeatWagon => hide([0.36, 0.28, 0.20], [0.44, 0.30, 0.28], IRON_C, [0.60, 1.00, 0.45]),
        Ship => hide([0.42, 0.31, 0.19], [0.56, 0.50, 0.40], BRONZE_C, [0.95, 0.80, 0.40]),
        // --------------------------------------------------------- buildings
        Obelisk => hide(STONE_C, [0.28, 0.30, 0.34], BRONZE_C, [0.55, 0.80, 1.00]),
        MagicTower => hide([0.44, 0.44, 0.46], [0.22, 0.28, 0.48], STEEL_C, [0.50, 0.80, 1.00]),
        Observatory => hide([0.46, 0.44, 0.40], [0.28, 0.34, 0.42], BRONZE_C, [0.60, 0.90, 1.00]),
        DemonGate => hide([0.26, 0.22, 0.24], [0.20, 0.12, 0.16], IRON_C, [0.60, 1.00, 0.35]),
        Altar => hide([0.44, 0.42, 0.37], [0.34, 0.24, 0.30], BRONZE_C, [0.95, 0.80, 0.35]),
        Burrow => hide([0.36, 0.28, 0.19], [0.34, 0.30, 0.20], BRONZE_C, [0.95, 0.78, 0.35]),
        Tentacle => hide([0.40, 0.20, 0.34], [0.26, 0.14, 0.22], IRON_C, [0.80, 0.45, 1.00]),
        // -------------------------------------------------- props and effects
        Wisp => hide([0.55, 0.80, 0.60], [0.30, 0.45, 0.34], BRONZE_C, [0.65, 1.00, 0.70]),
        SkullPile => hide(BONE_C, [0.22, 0.20, 0.17], IRON_C, [0.50, 1.00, 0.50]),
        IceTorch => hide([0.58, 0.70, 0.80], [0.28, 0.36, 0.44], STEEL_C, [0.55, 0.88, 1.00]),
        EggSack => hide([0.38, 0.46, 0.26], [0.28, 0.34, 0.20], BRONZE_C, [0.65, 1.00, 0.45]),
        Snowman => hide([0.80, 0.83, 0.86], [0.44, 0.24, 0.20], IRON_C, [0.70, 0.90, 1.00]),
        ThornsAura => hide([0.30, 0.40, 0.22], [0.24, 0.34, 0.18], BRONZE_C, [0.70, 1.00, 0.45]),
        CommandAura => hide([0.46, 0.38, 0.24], [0.42, 0.28, 0.16], BRONZE_C, [1.00, 0.82, 0.35]),
        ControlMagic => hide([0.42, 0.40, 0.50], [0.28, 0.26, 0.40], STEEL_C, [0.70, 0.70, 1.00]),
        DarkPortal => hide([0.28, 0.22, 0.30], [0.20, 0.14, 0.24], IRON_C, [0.75, 0.45, 1.00]),
    }
}

const FLESH: Material = Material::CHITIN;
const CLOTH: Material = Material::FOLIAGE;
const STEEL: Material = Material::METAL;
const IRON: Material = Material::DARK_METAL;
const STONE: Material = Material::STONE;
const WOOD: Material = Material::WOOD;

// ---------------------------------------------------------------- dispatch

pub fn draw(d: &mut DrawList, m: Model, p: &Pose, s: &Skin) {
    use Model::*;
    match m {
        // people
        Acolyte => robed(d, p, s, Robe::Acolyte),
        Mage => robed(d, p, s, Robe::Mage),
        Wraith => robed(d, p, s, Robe::Wraith),
        Archer => soldier(d, p, s, Arms::Bow),
        Rifleman => soldier(d, p, s, Arms::Gun),
        Warrior => soldier(d, p, s, Arms::Blade),
        Villager => soldier(d, p, s, Arms::None),
        Skeleton => skeleton(d, p, s),
        Demon => demon(d, p, s),
        Brute => brute(d, p, s),
        Panda => panda(d, p, s),
        Troll => troll(d, p, s),
        Gnoll => gnoll(d, p, s),
        Giant => giant(d, p, s),
        Naga => naga(d, p, s),
        // beasts
        Bear => beast(d, p, s, Beast::Bear),
        Mammoth => beast(d, p, s, Beast::Mammoth),
        Centaur => centaur(d, p, s),
        Lizard => beast(d, p, s, Beast::Lizard),
        Crab => crab(d, p, s),
        Spider => spider(d, p, s),
        Serpent => serpent(d, p, s),
        Turtle => turtle(d, p, s),
        Ent => ent(d, p, s),
        Golem => golem(d, p, s),
        Infernal => infernal(d, p, s),
        FlameLord => flame_lord(d, p, s),
        // wings
        Gyrocopter => gyrocopter(d, p, s),
        Phoenix => phoenix(d, p, s),
        Harpy => harpy(d, p, s),
        Dragon => dragon(d, p, s, false),
        FrostWyrm => dragon(d, p, s, true),
        // machines
        Turret => turret(d, p, s, 1),
        Turbolazer => turret(d, p, s, 2),
        RebelTurret => turret(d, p, s, 3),
        Vulcan => turret(d, p, s, 4),
        SamSite => sam_site(d, p, s),
        Cannon => cannon(d, p, s),
        MeatWagon => meat_wagon(d, p, s),
        Ship => ship(d, p, s),
        // buildings
        Obelisk => obelisk(d, p, s),
        MagicTower => magic_tower(d, p, s),
        Observatory => observatory(d, p, s),
        DemonGate => demon_gate(d, p, s),
        Altar => altar(d, p, s),
        Burrow => burrow(d, p, s),
        Tentacle => tentacle(d, p, s),
        // props and pure effects
        Wisp => wisp(d, p, s),
        SkullPile => skull_pile(d, p, s),
        IceTorch => ice_torch(d, p, s),
        EggSack => egg_sack(d, p, s),
        Snowman => snowman(d, p, s),
        ThornsAura => aura_ring(d, p, s, 0),
        CommandAura => aura_ring(d, p, s, 1),
        ControlMagic => aura_ring(d, p, s, 2),
        DarkPortal => dark_portal(d, p, s),
    }
}

// ---------------------------------------------------------------- parts

/// A squashed sphere, oriented with the pose.
#[allow(clippy::too_many_arguments)]
fn blob(d: &mut DrawList, p: &Pose, z: f32, l: f32, w: f32, h: f32, col: Color, mat: Material) {
    d.shape(
        Shape::Sphere,
        [p.pos[0], p.pos[1], p.z + z],
        [l, w, h],
        p.yaw,
        0.0,
        col,
        mat,
        0.0,
    );
}

/// Two legs under a standing figure, swinging opposite each other, with a boot
/// on the end of each.
#[allow(clippy::too_many_arguments)]
fn stride(
    d: &mut DrawList,
    p: &Pose,
    hip_z: f32,
    thick: f32,
    leg: Color,
    boot: Color,
    mat: Material,
) {
    let r = p.r;
    for (i, side) in [-1.0f32, 1.0].iter().enumerate() {
        let phase = p.t * 2.2 + i as f32 * std::f32::consts::PI;
        let swing = if p.walk { phase.sin() * r * 0.30 } else { 0.0 };
        let hip = p.p3(0.0, side * r * 0.34, hip_z);
        let knee = p.p3(swing * 0.5, side * r * 0.36, hip_z * 0.45);
        let foot = p.p3(swing, side * r * 0.36, swing.abs() * 0.35);
        d.link(Shape::Capsule, hip, knee, r * thick, leg, mat, 0.0);
        d.link(Shape::Capsule, knee, foot, r * thick * 0.88, leg, mat, 0.0);
        if p.fine() {
            // The boot, pointing the way the figure faces.
            d.shape(
                Shape::Box,
                [foot[0], foot[1], foot[2] + r * 0.10],
                [r * 0.40, r * 0.26, r * 0.20],
                p.yaw,
                0.0,
                boot,
                Material::CHITIN,
                0.0,
            );
        }
    }
}

/// Capsule legs stepping in diagonal pairs, for anything with four or more.
fn legs(d: &mut DrawList, p: &Pose, col: Color, count: usize, spread: f32, len: f32) {
    let r = p.r;
    for i in 0..count {
        let fwd = if count <= 2 || i < 2 { 1.0 } else { -1.0 };
        let side = if i % 2 == 0 { 1.0 } else { -1.0 };
        let phase = p.t * 2.0 + (i as f32) * std::f32::consts::FRAC_PI_2;
        let lift = if p.walk {
            (phase.sin() * 0.5 + 0.5) * r * 0.30
        } else {
            0.0
        };
        let swing = if p.walk { phase.cos() * r * 0.18 } else { 0.0 };
        let hip = p.at(fwd * r * spread, side * r * spread);
        let foot = p.at(fwd * r * spread + swing, side * r * spread);
        d.link(
            Shape::Capsule,
            [hip[0], hip[1], p.z + len + lift],
            [foot[0], foot[1], p.z + lift * 0.5],
            r * 0.26,
            col,
            FLESH,
            0.0,
        );
    }
}

fn eyes(d: &mut DrawList, p: &Pose, at: [f32; 2], z: f32, size: f32, spread: f32, col: Color) {
    for s in [-1.0f32, 1.0] {
        let (c, sn) = (p.yaw.cos(), p.yaw.sin());
        let e = [
            at[0] + c * size * 0.55 - sn * s * spread,
            at[1] + sn * size * 0.55 + c * s * spread,
        ];
        d.sphere_lit([e[0], e[1], z], size * 0.42, col, 0.5);
    }
}

/// A belt with a buckle, which is most of what tells a torso from a barrel.
fn belt(d: &mut DrawList, p: &Pose, z: f32, w: f32, s: &Skin) {
    let r = p.r;
    d.shape(
        Shape::Cylinder,
        p.p3(0.0, 0.0, z),
        [w, w * 0.82, r * 0.16],
        p.yaw,
        0.0,
        s.leather,
        FLESH,
        0.0,
    );
    if p.fine() {
        d.shape(
            Shape::Box,
            p.p3(w * 0.44, 0.0, z + r * 0.02),
            [r * 0.10, r * 0.22, r * 0.16],
            p.yaw,
            0.0,
            s.steel,
            STEEL,
            0.0,
        );
    }
}

/// Shoulders: a rounded pauldron with a raised rim.
fn pauldrons(d: &mut DrawList, p: &Pose, z: f32, out: f32, size: f32, s: &Skin) {
    let r = p.r;
    for side in [-1.0f32, 1.0] {
        d.shape(
            Shape::Sphere,
            p.p3(0.0, side * out, z),
            [size * 1.05, size * 1.25, size * 0.85],
            p.yaw,
            0.0,
            s.steel,
            STEEL,
            0.0,
        );
        if p.fine() {
            d.shape(
                Shape::Cylinder,
                p.p3(0.0, side * out * 1.12, z - size * 0.30),
                [size * 1.15, size * 1.15, r * 0.06],
                p.yaw,
                0.0,
                s.iron,
                IRON,
                0.0,
            );
        }
    }
}

/// An arm from shoulder to hand, with a bracer and a fist.
#[allow(clippy::too_many_arguments)]
fn arm(
    d: &mut DrawList,
    p: &Pose,
    from: [f32; 3],
    elbow: [f32; 3],
    hand: [f32; 3],
    thick: f32,
    s: &Skin,
    armoured: bool,
) {
    let r = p.r;
    let upper = if armoured { s.steel } else { s.skin };
    d.link(Shape::Capsule, from, elbow, r * thick, upper, FLESH, 0.0);
    d.link(
        Shape::Capsule,
        elbow,
        hand,
        r * thick * 0.86,
        s.skin,
        FLESH,
        0.0,
    );
    if p.fine() {
        let mid = [
            (elbow[0] + hand[0]) * 0.5,
            (elbow[1] + hand[1]) * 0.5,
            (elbow[2] + hand[2]) * 0.5,
        ];
        d.sphere(mid, r * thick * 1.5, s.leather, FLESH);
    }
    d.sphere(hand, r * thick * 1.35, s.skin, FLESH);
}

/// A sword: grip, guard, blade, fuller, pommel.
fn sword(d: &mut DrawList, p: &Pose, grip: [f32; 3], tip: [f32; 3], s: &Skin) {
    let r = p.r;
    let dir = [tip[0] - grip[0], tip[1] - grip[1], tip[2] - grip[2]];
    let at = |k: f32| {
        [
            grip[0] + dir[0] * k,
            grip[1] + dir[1] * k,
            grip[2] + dir[2] * k,
        ]
    };
    d.link(
        Shape::Cylinder,
        at(-0.14),
        at(0.10),
        r * 0.055,
        s.leather,
        FLESH,
        0.0,
    );
    if p.fine() {
        d.sphere(at(-0.18), r * 0.11, s.steel, STEEL);
    }
    let g = at(0.12);
    d.shape(
        Shape::Box,
        g,
        [r * 0.09, r * 0.52, r * 0.09],
        p.yaw,
        0.0,
        s.steel,
        STEEL,
        0.0,
    );
    d.link(Shape::Prism, at(0.14), at(0.94), r * 0.105, s.steel, STEEL, 0.0);
    d.link(Shape::Cone, at(0.90), tip, r * 0.10, s.steel, STEEL, 0.0);
    if p.fine() {
        d.link(
            Shape::Capsule,
            at(0.20),
            at(0.86),
            r * 0.028,
            s.bone,
            STEEL,
            0.0,
        );
    }
}

/// A round shield with a rim and a boss.
fn shield(d: &mut DrawList, p: &Pose, at: [f32; 3], size: f32, s: &Skin) {
    let r = p.r;
    d.shape(
        Shape::Cylinder,
        at,
        [size, size, r * 0.10],
        p.yaw,
        std::f32::consts::FRAC_PI_2,
        s.wood,
        WOOD,
        0.0,
    );
    if p.fine() {
        d.shape(
            Shape::Cylinder,
            at,
            [size * 1.10, size * 1.10, r * 0.05],
            p.yaw,
            std::f32::consts::FRAC_PI_2,
            s.steel,
            STEEL,
            0.0,
        );
        d.sphere(at, size * 0.34, s.steel, STEEL);
        d.shape(
            Shape::Quad,
            at,
            [size * 0.9, size * 0.9, 1.0],
            p.yaw,
            std::f32::consts::FRAC_PI_2,
            s.body,
            CLOTH,
            0.0,
        );
    }
}

/// A helmet: dome, brow, nose guard, and a crest in the unit's colour.
fn helm(d: &mut DrawList, p: &Pose, head: [f32; 2], z: f32, size: f32, s: &Skin, crest: bool) {
    let r = p.r;
    d.shape(
        Shape::Sphere,
        [head[0], head[1], z + size * 0.12],
        [size * 1.12, size * 1.12, size * 1.05],
        p.yaw,
        0.0,
        s.steel,
        STEEL,
        0.0,
    );
    if p.fine() {
        d.shape(
            Shape::Cylinder,
            [head[0], head[1], z - size * 0.10],
            [size * 1.18, size * 1.18, r * 0.06],
            p.yaw,
            0.0,
            s.iron,
            IRON,
            0.0,
        );
        d.link(
            Shape::Prism,
            [head[0], head[1], z + size * 0.20],
            p.p3(size * 0.92, 0.0, z - size * 0.42 - p.z),
            r * 0.05,
            s.steel,
            STEEL,
            0.0,
        );
    }
    if crest {
        for k in 0..4 {
            let f = (k as f32 - 1.5) * 0.22;
            d.link(
                Shape::Cone,
                [head[0], head[1], z + size * 0.9],
                p.p3(f * size, 0.0, z + size * (1.55 - f.abs() * 0.5) - p.z),
                r * 0.055,
                s.body,
                CLOTH,
                0.0,
            );
        }
    }
}

/// Horns curving off a head.
fn horns(d: &mut DrawList, p: &Pose, at: [f32; 2], z: f32, len: f32, col: Color) {
    let r = p.r;
    for side in [-1.0f32, 1.0] {
        let (c, s) = (p.yaw.cos(), p.yaw.sin());
        let base = [at[0] - s * side * r * 0.30, at[1] + c * side * r * 0.30];
        let mid = [
            base[0] - s * side * len * 0.5 - c * len * 0.1,
            base[1] + c * side * len * 0.5 - s * len * 0.1,
        ];
        let tip = [
            mid[0] - s * side * len * 0.35 + c * len * 0.35,
            mid[1] + c * side * len * 0.35 + s * len * 0.35,
        ];
        d.link(
            Shape::Cone,
            [base[0], base[1], z],
            [mid[0], mid[1], z + len * 0.75],
            r * 0.15,
            col,
            STONE,
            0.0,
        );
        d.link(
            Shape::Cone,
            [mid[0], mid[1], z + len * 0.75],
            [tip[0], tip[1], z + len * 1.25],
            r * 0.09,
            col,
            STONE,
            0.0,
        );
    }
}

/// A pair of wings, swept back and beating.
fn wings(d: &mut DrawList, p: &Pose, z: f32, span: f32, col: Color, membrane: bool) {
    let r = p.r;
    let beat = (p.t * 3.0).sin() * 0.35;
    for side in [-1.0f32, 1.0] {
        let root = p.p3(-r * 0.1, side * r * 0.3, z);
        let mid = p.p3(
            r * 0.1,
            side * span * 0.62,
            z + span * 0.30 + beat * span * 0.5,
        );
        let tip = p.p3(-r * 0.7, side * span, z + span * 0.10 + beat * span);
        d.link(Shape::Capsule, root, mid, r * 0.13, col, FLESH, 0.0);
        d.link(Shape::Capsule, mid, tip, r * 0.10, col, FLESH, 0.0);
        if membrane && p.lights {
            let c = [
                (root[0] + mid[0] + tip[0]) / 3.0,
                (root[1] + mid[1] + tip[1]) / 3.0,
                (root[2] + mid[2] + tip[2]) / 3.0,
            ];
            d.shape(
                Shape::Quad,
                c,
                [span * 0.95, span * 0.62, 1.0],
                p.yaw + side * 0.5,
                0.9 + beat,
                col,
                CLOTH,
                0.0,
            );
        }
    }
}

/// A cloak hanging off the shoulders, swaying with the stride.
fn cloak(d: &mut DrawList, p: &Pose, z: f32, len: f32, col: Color) {
    let r = p.r;
    let sway = if p.walk { (p.t * 2.0).sin() * 0.14 } else { 0.0 };
    d.shape(
        Shape::Cone,
        p.p3(-r * 0.42, 0.0, z - len * 0.42),
        [r * 1.55, r * 1.05, len],
        p.yaw + sway,
        std::f32::consts::PI,
        col,
        CLOTH,
        0.0,
    );
}

// ---------------------------------------------------------------- people

enum Robe {
    Acolyte,
    Mage,
    Wraith,
}

/// The robed silhouette: a bell of cloth, a mantle over the shoulders and a
/// hood with a shadow in it. Fifteen of the map's towers wear one.
fn robed(d: &mut DrawList, p: &Pose, s: &Skin, kind: Robe) {
    let r = p.r;
    let float = match kind {
        Robe::Wraith => (p.t * 1.2).sin() * r * 0.12 + r * 0.25,
        _ => 0.0,
    };
    let hem = p.z + float;
    let head_z = float + r * 2.05;

    // The bell of the robe, in two courses so it has a waist.
    d.shape(
        Shape::Cone,
        [p.pos[0], p.pos[1], hem],
        [r * 1.60, r * 1.35, r * 1.30],
        p.yaw,
        0.0,
        s.cloth,
        CLOTH,
        0.0,
    );
    d.shape(
        Shape::Cone,
        [p.pos[0], p.pos[1], hem + r * 1.05],
        [r * 1.12, r * 0.98, r * 0.85],
        p.yaw,
        0.0,
        s.cloth,
        CLOTH,
        0.0,
    );
    if p.fine() {
        for k in 0..3 {
            let a = (k as f32 - 1.0) * 0.45;
            let q = p.at(r * 0.85 * a.cos(), r * 0.95 * a.sin());
            d.link(
                Shape::Capsule,
                [q[0], q[1], hem + r * 0.06],
                [q[0], q[1], hem + r * 1.15],
                r * 0.055,
                s.dark,
                CLOTH,
                0.0,
            );
        }
    }
    belt(d, p, float + r * 1.22, r * 0.95, s);
    d.shape(
        Shape::Cone,
        p.p3(0.0, 0.0, float + r * 1.86),
        [r * 1.28, r * 1.12, r * 0.60],
        p.yaw,
        std::f32::consts::PI,
        s.dark,
        CLOTH,
        0.0,
    );
    d.shape(
        Shape::Sphere,
        p.p3(-r * 0.06, 0.0, float + r * 2.10),
        [r * 0.86, r * 0.82, r * 0.86],
        p.yaw,
        0.0,
        s.cloth,
        CLOTH,
        0.0,
    );
    // The hood's opening: a dark hollow with two lights in it.
    let face = p.at(r * 0.42, 0.0);
    d.shape(
        Shape::Sphere,
        [face[0], face[1], p.z + head_z - r * 0.02],
        [r * 0.34, r * 0.52, r * 0.50],
        p.yaw,
        0.0,
        rgba([0.04, 0.04, 0.05], 1.0),
        CLOTH,
        0.0,
    );
    eyes(d, p, face, p.z + head_z, r * 0.30, r * 0.15, s.glow);

    match kind {
        Robe::Acolyte => {
            let hands = p.p3(r * 0.52, 0.0, float + r * 1.32);
            d.sphere(hands, r * 0.24, s.skin, FLESH);
            d.link(
                Shape::Capsule,
                hands,
                p.p3(r * 0.60, 0.0, float + r * 0.70),
                r * 0.025,
                s.iron,
                IRON,
                0.0,
            );
            d.sphere_lit(p.p3(r * 0.60, 0.0, float + r * 0.60), r * 0.24, s.glow, 0.4);
        }
        Robe::Mage => {
            // A staff: shaft, wrapped grip, a claw of prongs and a stone.
            let foot = p.p3(r * 0.55, r * 0.60, float);
            let top = p.p3(r * 0.42, r * 0.66, float + r * 2.60);
            d.link(Shape::Cylinder, foot, top, r * 0.075, s.wood, WOOD, 0.0);
            if p.fine() {
                for k in 0..3 {
                    let f = 0.42 + k as f32 * 0.07;
                    d.shape(
                        Shape::Cylinder,
                        p.p3(r * 0.50, r * 0.63, float + r * 2.60 * f),
                        [r * 0.11, r * 0.11, r * 0.05],
                        p.yaw,
                        0.0,
                        s.leather,
                        FLESH,
                        0.0,
                    );
                }
                for k in 0..3 {
                    let a = k as f32 * 2.094 + p.yaw;
                    d.link(
                        Shape::Cone,
                        [top[0], top[1], top[2] - r * 0.10],
                        [
                            top[0] + a.cos() * r * 0.22,
                            top[1] + a.sin() * r * 0.22,
                            top[2] + r * 0.30,
                        ],
                        r * 0.045,
                        s.iron,
                        IRON,
                        0.0,
                    );
                }
            }
            d.sphere_lit([top[0], top[1], top[2] + r * 0.16], r * 0.34, s.glow, 0.55);
            p.glow(d, [top[0], top[1], top[2] + r * 0.16], r * 1.4, 0.6, s.glow);
            arm(
                d,
                p,
                p.p3(0.0, -r * 0.55, float + r * 1.80),
                p.p3(r * 0.35, -r * 0.72, float + r * 1.45),
                p.p3(r * 0.70, -r * 0.55, float + r * 1.70),
                0.10,
                s,
                false,
            );
        }
        Robe::Wraith => {
            for k in 0..5 {
                let a = k as f32 * 1.257 + p.yaw;
                let q = p.at(a.cos() * r * 0.62, a.sin() * r * 0.58);
                d.link(
                    Shape::Cone,
                    [q[0], q[1], hem + r * 0.55],
                    [q[0], q[1], hem - r * 0.30],
                    r * 0.16,
                    s.dark,
                    CLOTH,
                    0.0,
                );
            }
            for side in [-1.0f32, 1.0] {
                d.link(
                    Shape::Capsule,
                    p.p3(0.0, side * r * 0.55, float + r * 1.75),
                    p.p3(r * 0.55, side * r * 0.62, float + r * 1.25),
                    r * 0.07,
                    s.bone,
                    STONE,
                    0.0,
                );
            }
            p.glow(d, p.p3(0.0, 0.0, head_z), r * 2.0, 0.25, s.glow);
        }
    }
}

enum Arms {
    None,
    Blade,
    Bow,
    Gun,
}

/// The soldier: greaves, tabard, breastplate, pauldrons, helm, and whatever it
/// carries. Eleven towers and half the creep roster are one of these.
fn soldier(d: &mut DrawList, p: &Pose, s: &Skin, arms: Arms) {
    let r = p.r;
    let hip = r * 0.98;
    let chest = hip + r * 0.66;
    stride(d, p, hip, 0.19, s.steel, s.leather, STEEL);

    // Torso: a tapered plate over a padded gambeson.
    d.shape(
        Shape::Cone,
        p.p3(0.0, 0.0, hip - r * 0.10),
        [r * 1.00, r * 0.72, r * 0.90],
        p.yaw,
        std::f32::consts::PI,
        s.cloth,
        CLOTH,
        0.0,
    );
    d.shape(
        Shape::Sphere,
        p.p3(0.0, 0.0, chest),
        [r * 0.90, r * 1.10, r * 0.72],
        p.yaw,
        0.0,
        s.steel,
        STEEL,
        0.0,
    );
    if p.fine() {
        d.shape(
            Shape::Quad,
            p.p3(r * 0.46, 0.0, hip + r * 0.18),
            [r * 0.62, r * 1.05, 1.0],
            p.yaw + std::f32::consts::FRAC_PI_2,
            0.15,
            s.body,
            CLOTH,
            0.0,
        );
        for k in 0..2 {
            d.shape(
                Shape::Cylinder,
                p.p3(0.0, 0.0, chest - r * (0.10 + 0.18 * k as f32)),
                [r * 0.94, r * 0.80, r * 0.055],
                p.yaw,
                0.0,
                s.iron,
                IRON,
                0.0,
            );
        }
    }
    belt(d, p, hip + r * 0.02, r * 0.86, s);
    pauldrons(d, p, chest + r * 0.16, r * 0.62, r * 0.42, s);

    let head = p.at(r * 0.05, 0.0);
    d.link(
        Shape::Cylinder,
        p.p3(0.0, 0.0, chest + r * 0.20),
        p.p3(0.0, 0.0, chest + r * 0.42),
        r * 0.17,
        s.skin,
        FLESH,
        0.0,
    );
    d.shape(
        Shape::Sphere,
        [head[0], head[1], p.z + chest + r * 0.66],
        [r * 0.50, r * 0.46, r * 0.54],
        p.yaw,
        0.0,
        s.skin,
        FLESH,
        0.0,
    );
    helm(
        d,
        p,
        head,
        p.z + chest + r * 0.70,
        r * 0.50,
        s,
        matches!(arms, Arms::Blade),
    );
    eyes(d, p, head, p.z + chest + r * 0.62, r * 0.20, r * 0.16, s.glow);

    let swing = if p.walk { (p.t * 2.2).sin() * r * 0.18 } else { 0.0 };
    match arms {
        Arms::None => {
            for side in [-1.0f32, 1.0] {
                arm(
                    d,
                    p,
                    p.p3(0.0, side * r * 0.58, chest + r * 0.10),
                    p.p3(side * swing, side * r * 0.66, chest - r * 0.34),
                    p.p3(side * swing * 1.4, side * r * 0.62, hip - r * 0.02),
                    0.115,
                    s,
                    false,
                );
            }
        }
        Arms::Blade => {
            let grip = p.p3(r * 0.62, -r * 0.78, chest + r * 0.16);
            arm(
                d,
                p,
                p.p3(0.0, -r * 0.60, chest + r * 0.10),
                p.p3(r * 0.42, -r * 0.86, chest - r * 0.22),
                grip,
                0.115,
                s,
                true,
            );
            sword(d, p, grip, p.p3(r * 0.32, -r * 0.95, chest + r * 1.90), s);
            let sh = p.p3(r * 0.46, r * 0.80, chest - r * 0.06);
            arm(
                d,
                p,
                p.p3(0.0, r * 0.60, chest + r * 0.10),
                p.p3(r * 0.30, r * 0.78, chest - r * 0.26),
                sh,
                0.115,
                s,
                true,
            );
            shield(d, p, sh, r * 0.62, s);
            cloak(d, p, chest + r * 0.24, r * 1.65, s.body);
        }
        Arms::Bow => {
            let grip = p.p3(r * 0.72, -r * 0.55, chest + r * 0.08);
            arm(
                d,
                p,
                p.p3(0.0, -r * 0.56, chest + r * 0.08),
                p.p3(r * 0.40, -r * 0.62, chest - r * 0.16),
                grip,
                0.10,
                s,
                false,
            );
            let top = p.p3(r * 0.58, -r * 0.62, chest + r * 1.15);
            let bot = p.p3(r * 0.58, -r * 0.62, chest - r * 1.00);
            d.link(Shape::Capsule, grip, top, r * 0.055, s.wood, WOOD, 0.0);
            d.link(Shape::Capsule, grip, bot, r * 0.055, s.wood, WOOD, 0.0);
            if p.fine() {
                d.link(
                    Shape::Cone,
                    top,
                    p.p3(r * 0.40, -r * 0.62, chest + r * 1.42),
                    r * 0.04,
                    s.bone,
                    STONE,
                    0.0,
                );
                d.link(
                    Shape::Cone,
                    bot,
                    p.p3(r * 0.40, -r * 0.62, chest - r * 1.28),
                    r * 0.04,
                    s.bone,
                    STONE,
                    0.0,
                );
            }
            d.link(Shape::Capsule, top, bot, r * 0.022, s.bone, CLOTH, 0.0);
            arm(
                d,
                p,
                p.p3(0.0, r * 0.52, chest + r * 0.08),
                p.p3(-r * 0.18, r * 0.60, chest - r * 0.06),
                p.p3(r * 0.10, -r * 0.10, chest + r * 0.14),
                0.10,
                s,
                false,
            );
            let qa = p.p3(-r * 0.48, r * 0.34, chest - r * 0.44);
            let qb = p.p3(-r * 0.42, r * 0.52, chest + r * 0.62);
            d.link(Shape::Cylinder, qa, qb, r * 0.17, s.leather, FLESH, 0.0);
            if p.fine() {
                for k in 0..3 {
                    let f = (k as f32 - 1.0) * 0.10;
                    d.link(
                        Shape::Cone,
                        qb,
                        [qb[0] + f, qb[1] + f * 0.4, qb[2] + r * 0.34],
                        r * 0.035,
                        s.body,
                        CLOTH,
                        0.0,
                    );
                }
            }
        }
        Arms::Gun => {
            let butt = p.p3(-r * 0.15, -r * 0.42, chest + r * 0.02);
            let muzzle = p.p3(r * 1.70, -r * 0.38, chest + r * 0.22);
            d.link(
                Shape::Prism,
                butt,
                p.p3(r * 0.45, -r * 0.40, chest + r * 0.10),
                r * 0.13,
                s.wood,
                WOOD,
                0.0,
            );
            d.link(
                Shape::Cylinder,
                p.p3(r * 0.40, -r * 0.40, chest + r * 0.10),
                muzzle,
                r * 0.07,
                s.iron,
                IRON,
                0.0,
            );
            if p.fine() {
                d.shape(
                    Shape::Cylinder,
                    p.p3(r * 1.05, -r * 0.39, chest + r * 0.16),
                    [r * 0.13, r * 0.13, r * 0.06],
                    p.yaw,
                    std::f32::consts::FRAC_PI_2,
                    s.steel,
                    STEEL,
                    0.0,
                );
                d.sphere(muzzle, r * 0.09, s.iron, IRON);
            }
            for side in [-1.0f32, 1.0] {
                arm(
                    d,
                    p,
                    p.p3(0.0, side * r * 0.56, chest + r * 0.10),
                    p.p3(r * 0.32, side * r * 0.50, chest - r * 0.12),
                    p.p3(
                        r * (0.55 + 0.35 * (side * 0.5 + 0.5)),
                        -r * 0.40,
                        chest + r * 0.12,
                    ),
                    0.105,
                    s,
                    false,
                );
            }
        }
    }
}

/// Bones: the same frame with everything filled in taken out.
fn skeleton(d: &mut DrawList, p: &Pose, s: &Skin) {
    let r = p.r;
    let hip = r * 0.92;
    let chest = hip + r * 0.70;
    stride(d, p, hip, 0.09, s.bone, s.bone, STONE);
    d.shape(
        Shape::Cylinder,
        p.p3(0.0, 0.0, hip + r * 0.10),
        [r * 0.52, r * 0.40, r * 0.20],
        p.yaw,
        0.0,
        s.bone,
        STONE,
        0.0,
    );
    d.link(
        Shape::Capsule,
        p.p3(0.0, 0.0, hip),
        p.p3(0.0, 0.0, chest + r * 0.30),
        r * 0.075,
        s.bone,
        STONE,
        0.0,
    );
    for k in 0..5 {
        let f = k as f32;
        let z = p.z + hip + r * (0.24 + 0.16 * f);
        let w = r * (0.80 - 0.045 * f);
        d.shape(
            Shape::Cylinder,
            [p.pos[0], p.pos[1], z],
            [w, w * 0.72, r * 0.055],
            p.yaw,
            0.0,
            s.bone,
            STONE,
            0.0,
        );
    }
    d.shape(
        Shape::Cylinder,
        p.p3(0.0, 0.0, chest + r * 0.26),
        [r * 0.66, r * 0.42, r * 0.08],
        p.yaw,
        0.0,
        s.bone,
        STONE,
        0.0,
    );
    let head = p.at(r * 0.06, 0.0);
    d.shape(
        Shape::Sphere,
        [head[0], head[1], p.z + chest + r * 0.62],
        [r * 0.46, r * 0.44, r * 0.50],
        p.yaw,
        0.0,
        s.bone,
        STONE,
        0.0,
    );
    d.shape(
        Shape::Box,
        [head[0], head[1], p.z + chest + r * 0.74],
        [r * 0.48, r * 0.46, r * 0.10],
        p.yaw,
        0.0,
        s.bone,
        STONE,
        0.0,
    );
    d.shape(
        Shape::Prism,
        [head[0], head[1], p.z + chest + r * 0.38],
        [r * 0.38, r * 0.16, 1.0],
        p.yaw,
        0.0,
        s.bone,
        STONE,
        0.0,
    );
    eyes(d, p, head, p.z + chest + r * 0.64, r * 0.22, r * 0.14, s.glow);
    for side in [-1.0f32, 1.0] {
        let shoulder = p.p3(0.0, side * r * 0.46, chest + r * 0.22);
        let elbow = p.p3(r * 0.22, side * r * 0.70, chest - r * 0.22);
        let hand = p.p3(r * 0.44, side * r * 0.66, hip + r * 0.06);
        d.link(Shape::Capsule, shoulder, elbow, r * 0.062, s.bone, STONE, 0.0);
        d.link(Shape::Capsule, elbow, hand, r * 0.055, s.bone, STONE, 0.0);
        if side > 0.0 {
            sword(
                d,
                p,
                hand,
                p.p3(r * 0.30, side * r * 0.80, chest + r * 1.30),
                s,
            );
        }
    }
}

/// Horns, wings and a bad attitude: the Dread Lord, the Doom Guard, Kil'jaeden.
fn demon(d: &mut DrawList, p: &Pose, s: &Skin) {
    let r = p.r;
    let hip = r * 1.10;
    let chest = hip + r * 0.82;
    stride(d, p, hip, 0.24, s.skin, s.iron, FLESH);
    if p.fine() {
        for side in [-1.0f32, 1.0] {
            d.shape(
                Shape::Cone,
                p.p3(0.0, side * r * 0.36, r * 0.10),
                [r * 0.28, r * 0.24, r * 0.22],
                p.yaw,
                std::f32::consts::PI,
                s.iron,
                IRON,
                0.0,
            );
        }
    }
    d.shape(
        Shape::Cone,
        p.p3(0.0, 0.0, hip),
        [r * 1.10, r * 0.80, r * 1.00],
        p.yaw,
        std::f32::consts::PI,
        s.skin,
        FLESH,
        0.0,
    );
    d.shape(
        Shape::Sphere,
        p.p3(0.0, 0.0, chest),
        [r * 1.00, r * 1.42, r * 0.82],
        p.yaw,
        0.0,
        s.skin,
        FLESH,
        0.0,
    );
    if p.fine() {
        d.shape(
            Shape::Cylinder,
            p.p3(r * 0.30, 0.0, chest + r * 0.06),
            [r * 0.80, r * 1.10, r * 0.14],
            p.yaw,
            0.0,
            s.iron,
            IRON,
            0.0,
        );
    }
    belt(d, p, hip + r * 0.06, r * 0.92, s);
    for side in [-1.0f32, 1.0] {
        d.shape(
            Shape::Cone,
            p.p3(-r * 0.05, side * r * 0.86, chest + r * 0.16),
            [r * 0.70, r * 0.62, r * 0.62],
            p.yaw,
            side * 0.55,
            s.iron,
            IRON,
            0.0,
        );
        for k in 0..2 {
            let f = k as f32 * 0.28 - 0.14;
            d.link(
                Shape::Cone,
                p.p3(f * r, side * r * 0.92, chest + r * 0.42),
                p.p3(f * r, side * r * 1.06, chest + r * 0.95),
                r * 0.07,
                s.bone,
                STONE,
                0.0,
            );
        }
        arm(
            d,
            p,
            p.p3(0.0, side * r * 0.90, chest),
            p.p3(r * 0.34, side * r * 1.10, chest - r * 0.55),
            p.p3(r * 0.66, side * r * 0.95, hip - r * 0.05),
            0.15,
            s,
            false,
        );
    }
    let head = p.at(r * 0.12, 0.0);
    d.shape(
        Shape::Sphere,
        [head[0], head[1], p.z + chest + r * 0.80],
        [r * 0.68, r * 0.56, r * 0.60],
        p.yaw,
        0.0,
        s.skin,
        FLESH,
        0.0,
    );
    d.shape(
        Shape::Box,
        [head[0], head[1], p.z + chest + r * 0.94],
        [r * 0.62, r * 0.60, r * 0.14],
        p.yaw,
        0.0,
        s.dark,
        FLESH,
        0.0,
    );
    horns(d, p, head, p.z + chest + r * 0.98, r * 0.85, s.bone);
    eyes(d, p, head, p.z + chest + r * 0.80, r * 0.28, r * 0.18, s.glow);
    wings(d, p, chest + r * 0.30, r * 2.3, s.dark, true);
    p.glow(d, p.p3(0.0, 0.0, chest), r * 2.0, 0.25, s.glow);
}

/// All shoulders and no neck: the Tauren, the Abomination, the Mountain King.
fn brute(d: &mut DrawList, p: &Pose, s: &Skin) {
    let r = p.r;
    let hip = r * 0.84;
    let chest = hip + r * 0.74;
    stride(d, p, hip, 0.28, s.skin, s.leather, FLESH);
    blob(d, p, chest, r * 1.20, r * 1.62, r * 1.05, s.skin, FLESH);
    if p.fine() {
        for k in 0..2 {
            let a = (k as f32 - 0.5) * 0.9;
            d.link(
                Shape::Capsule,
                p.p3(r * 0.55, -r * 0.85 * a.cos(), chest + r * 0.45),
                p.p3(r * 0.55, r * 0.85 * a.cos(), chest - r * 0.45),
                r * 0.09,
                s.leather,
                FLESH,
                0.0,
            );
        }
    }
    belt(d, p, hip + r * 0.04, r * 1.00, s);
    for side in [-1.0f32, 1.0] {
        let swing = if p.walk {
            (p.t * 2.2 + side).sin() * r * 0.22
        } else {
            0.0
        };
        d.sphere(
            p.p3(0.0, side * r * 1.10, chest + r * 0.34),
            r * 0.52,
            s.skin,
            FLESH,
        );
        arm(
            d,
            p,
            p.p3(0.0, side * r * 1.10, chest + r * 0.25),
            p.p3(swing * 0.5, side * r * 1.28, chest - r * 0.55),
            p.p3(swing, side * r * 1.22, hip - r * 0.42),
            0.26,
            s,
            false,
        );
    }
    let head = p.at(r * 0.35, 0.0);
    d.shape(
        Shape::Sphere,
        [head[0], head[1], p.z + chest + r * 0.72],
        [r * 0.72, r * 0.64, r * 0.62],
        p.yaw,
        0.0,
        s.skin,
        FLESH,
        0.0,
    );
    d.shape(
        Shape::Sphere,
        p.p3(r * 0.90, 0.0, chest + r * 0.58),
        [r * 0.42, r * 0.36, r * 0.30],
        p.yaw,
        0.0,
        s.dark,
        FLESH,
        0.0,
    );
    horns(d, p, head, p.z + chest + r * 0.82, r * 0.85, s.bone);
    eyes(d, p, head, p.z + chest + r * 0.76, r * 0.26, r * 0.20, s.glow);
}

/// The Brewmaster: round, wide-legged, and carrying a barrel.
fn panda(d: &mut DrawList, p: &Pose, s: &Skin) {
    let r = p.r;
    let hip = r * 0.68;
    let chest = hip + r * 0.72;
    let pale = s.bone;
    stride(d, p, hip, 0.26, s.dark, s.leather, FLESH);
    blob(d, p, chest, r * 1.35, r * 1.45, r * 1.30, s.dark, FLESH);
    d.shape(
        Shape::Sphere,
        p.p3(r * 0.55, 0.0, chest - r * 0.10),
        [r * 0.75, r * 0.95, r * 0.95],
        p.yaw,
        0.0,
        pale,
        FLESH,
        0.0,
    );
    belt(d, p, hip + r * 0.30, r * 1.10, s);
    for side in [-1.0f32, 1.0] {
        let swing = if p.walk {
            (p.t * 2.0 + side).sin() * r * 0.16
        } else {
            0.0
        };
        arm(
            d,
            p,
            p.p3(0.0, side * r * 0.95, chest + r * 0.30),
            p.p3(r * 0.25, side * r * 1.05, chest - r * 0.15),
            p.p3(r * 0.35 + swing, side * r * 1.05, chest - r * 0.55),
            0.22,
            s,
            false,
        );
    }
    let head = p.at(r * 0.42, 0.0);
    d.sphere(
        [head[0], head[1], p.z + chest + r * 1.00],
        r * 0.72,
        pale,
        FLESH,
    );
    for side in [-1.0f32, 1.0] {
        let e = p.at(r * 0.25, side * r * 0.48);
        d.sphere([e[0], e[1], p.z + chest + r * 1.40], r * 0.30, s.dark, FLESH);
        if p.fine() {
            let q = p.at(r * 0.60, side * r * 0.26);
            d.shape(
                Shape::Sphere,
                [q[0], q[1], p.z + chest + r * 1.08],
                [r * 0.16, r * 0.20, r * 0.24],
                p.yaw,
                0.0,
                s.dark,
                FLESH,
                0.0,
            );
        }
    }
    d.shape(
        Shape::Sphere,
        p.p3(r * 0.80, 0.0, chest + r * 0.92),
        [r * 0.34, r * 0.34, r * 0.28],
        p.yaw,
        0.0,
        s.dark,
        FLESH,
        0.0,
    );
    eyes(d, p, head, p.z + chest + r * 1.06, r * 0.20, r * 0.24, s.glow);
    let ba = p.p3(-r * 0.85, -r * 0.38, chest + r * 0.10);
    let bb = p.p3(-r * 0.85, r * 0.38, chest + r * 0.10);
    d.link(Shape::Cylinder, ba, bb, r * 0.52, s.wood, WOOD, 0.0);
    if p.fine() {
        for k in 0..2 {
            let f = (k as f32 - 0.5) * 0.44;
            d.shape(
                Shape::Cylinder,
                p.p3(-r * 0.85, f * r, chest + r * 0.10),
                [r * 0.56, r * 0.56, r * 0.06],
                p.yaw,
                std::f32::consts::FRAC_PI_2,
                s.iron,
                IRON,
                0.0,
            );
        }
    }
}

/// Hunched, long-armed, tusked. Every Troll Tower.
fn troll(d: &mut DrawList, p: &Pose, s: &Skin) {
    let r = p.r;
    let hip = r * 0.76;
    let chest = hip + r * 0.66;
    stride(d, p, hip, 0.21, s.skin, s.leather, FLESH);
    d.shape(
        Shape::Sphere,
        p.p3(r * 0.12, 0.0, chest),
        [r * 1.35, r * 1.05, r * 1.00],
        p.yaw,
        -0.32,
        s.skin,
        FLESH,
        0.0,
    );
    if p.fine() {
        d.shape(
            Shape::Quad,
            p.p3(r * 0.62, 0.0, hip - r * 0.05),
            [r * 0.70, r * 0.80, 1.0],
            p.yaw + std::f32::consts::FRAC_PI_2,
            0.2,
            s.body,
            CLOTH,
            0.0,
        );
        for k in 0..5 {
            let a = (k as f32 - 2.0) * 0.34;
            d.sphere(
                p.p3(r * (0.55 - a.abs() * 0.1), a * r * 0.5, chest + r * 0.28),
                r * 0.10,
                s.bone,
                STONE,
            );
        }
    }
    for side in [-1.0f32, 1.0] {
        let swing = if p.walk {
            (p.t * 2.4 + side).sin() * r * 0.26
        } else {
            0.0
        };
        arm(
            d,
            p,
            p.p3(r * 0.20, side * r * 0.74, chest + r * 0.16),
            p.p3(r * 0.50, side * r * 0.92, chest - r * 0.35),
            p.p3(r * 0.62 + swing, side * r * 0.86, hip - r * 0.48),
            0.17,
            s,
            false,
        );
    }
    let head = p.at(r * 0.88, 0.0);
    d.shape(
        Shape::Sphere,
        [head[0], head[1], p.z + chest + r * 0.34],
        [r * 0.80, r * 0.56, r * 0.54],
        p.yaw,
        -0.18,
        s.skin,
        FLESH,
        0.0,
    );
    d.shape(
        Shape::Prism,
        p.p3(r * 1.05, 0.0, chest + r * 0.16),
        [r * 0.44, r * 0.24, 1.0],
        p.yaw,
        0.0,
        s.dark,
        FLESH,
        0.0,
    );
    for side in [-1.0f32, 1.0] {
        d.link(
            Shape::Cone,
            p.p3(r * 1.02, side * r * 0.22, chest + r * 0.14),
            p.p3(r * 1.16, side * r * 0.30, chest + r * 0.66),
            r * 0.085,
            s.bone,
            STONE,
            0.0,
        );
    }
    if p.fine() {
        for k in 0..4 {
            let a = (k as f32 - 1.5) * 0.3;
            d.link(
                Shape::Cone,
                [head[0], head[1], p.z + chest + r * 0.60],
                p.p3(r * (0.55 - a.abs() * 0.2), a * r * 0.5, chest + r * 1.10),
                r * 0.075,
                s.body,
                CLOTH,
                0.0,
            );
        }
    }
    eyes(d, p, head, p.z + chest + r * 0.46, r * 0.22, r * 0.16, s.glow);
    let butt = p.p3(-r * 0.55, -r * 0.80, chest - r * 0.60);
    let tip = p.p3(r * 1.30, -r * 0.62, chest + r * 1.00);
    d.link(Shape::Cylinder, butt, tip, r * 0.055, s.wood, WOOD, 0.0);
    if p.fine() {
        d.link(
            Shape::Cone,
            p.p3(r * 1.05, -r * 0.66, chest + r * 0.78),
            tip,
            r * 0.10,
            s.bone,
            STONE,
            0.0,
        );
    }
}

/// A hyena on two legs: a stooped spine, a long muzzle and a ragged crest.
fn gnoll(d: &mut DrawList, p: &Pose, s: &Skin) {
    let r = p.r;
    let hip = r * 0.80;
    let chest = hip + r * 0.58;
    stride(d, p, hip, 0.16, s.skin, s.leather, FLESH);
    d.shape(
        Shape::Sphere,
        p.p3(0.0, 0.0, chest),
        [r * 1.18, r * 0.92, r * 1.10],
        p.yaw,
        -0.24,
        s.skin,
        FLESH,
        0.0,
    );
    belt(d, p, hip + r * 0.02, r * 0.80, s);
    for k in 0..5 {
        let q = p.at(-r * (0.10 + k as f32 * 0.22), 0.0);
        d.link(
            Shape::Cone,
            [q[0], q[1], p.z + chest + r * 0.35],
            [q[0], q[1], p.z + chest + r * (0.85 - k as f32 * 0.08)],
            r * 0.10,
            s.dark,
            CLOTH,
            0.0,
        );
    }
    for side in [-1.0f32, 1.0] {
        arm(
            d,
            p,
            p.p3(0.0, side * r * 0.60, chest + r * 0.12),
            p.p3(r * 0.30, side * r * 0.72, chest - r * 0.34),
            p.p3(r * 0.48, side * r * 0.66, hip - r * 0.18),
            0.12,
            s,
            false,
        );
    }
    let head = p.at(r * 0.75, 0.0);
    d.sphere(
        [head[0], head[1], p.z + chest + r * 0.44],
        r * 0.46,
        s.skin,
        FLESH,
    );
    d.shape(
        Shape::Cone,
        p.p3(r * 1.22, 0.0, chest + r * 0.32),
        [r * 0.32, r * 0.28, r * 0.62],
        p.yaw,
        std::f32::consts::FRAC_PI_2,
        s.dark,
        FLESH,
        0.0,
    );
    for side in [-1.0f32, 1.0] {
        d.link(
            Shape::Cone,
            p.p3(r * 0.62, side * r * 0.26, chest + r * 0.62),
            p.p3(r * 0.50, side * r * 0.40, chest + r * 1.12),
            r * 0.08,
            s.dark,
            FLESH,
            0.0,
        );
    }
    eyes(d, p, head, p.z + chest + r * 0.52, r * 0.18, r * 0.15, s.glow);
    let grip = p.p3(r * 0.45, -r * 0.68, chest - r * 0.10);
    d.shape(
        Shape::Prism,
        [grip[0], grip[1], grip[2] - r * 0.55],
        [r * 0.60, r * 0.16, 1.0],
        p.yaw,
        0.0,
        s.steel,
        STEEL,
        0.0,
    );
}

/// The Mountain Giant: a slab of rock with a tree in its hand.
fn giant(d: &mut DrawList, p: &Pose, s: &Skin) {
    let r = p.r;
    let hip = r * 1.20;
    let chest = hip + r * 1.00;
    stride(d, p, hip, 0.36, s.steel, s.iron, STONE);
    d.shape(
        Shape::Prism,
        p.p3(0.0, 0.0, hip),
        [r * 1.70, r * 1.50, 1.0],
        p.yaw,
        0.0,
        s.steel,
        STONE,
        0.0,
    );
    blob(
        d,
        p,
        chest + r * 0.20,
        r * 1.30,
        r * 1.90,
        r * 1.00,
        s.steel,
        STONE,
    );
    for side in [-1.0f32, 1.0] {
        d.shape(
            Shape::Prism,
            p.p3(0.0, side * r * 1.20, chest + r * 0.55),
            [r * 0.90, r * 0.70, 1.0],
            p.yaw + side * 0.6,
            0.0,
            s.iron,
            STONE,
            0.0,
        );
        if p.fine() {
            d.shape(
                Shape::Sphere,
                p.p3(-r * 0.25, side * r * 1.20, chest + r * 0.90),
                [r * 0.55, r * 0.55, r * 0.18],
                p.yaw,
                0.0,
                rgba([0.16, 0.26, 0.12], 1.0),
                Material::FOLIAGE,
                0.0,
            );
        }
        d.link(
            Shape::Capsule,
            p.p3(0.0, side * r * 1.25, chest + r * 0.35),
            p.p3(r * 0.30, side * r * 1.40, hip - r * 0.60),
            r * 0.36,
            s.steel,
            STONE,
            0.0,
        );
    }
    let head = p.at(r * 0.20, 0.0);
    d.shape(
        Shape::Prism,
        [head[0], head[1], p.z + chest + r * 0.95],
        [r * 0.80, r * 0.62, 1.0],
        p.yaw,
        0.0,
        s.iron,
        STONE,
        0.0,
    );
    eyes(d, p, head, p.z + chest + r * 1.00, r * 0.26, r * 0.24, s.glow);
    let butt = p.p3(r * 0.30, -r * 1.50, hip - r * 0.40);
    let top = p.p3(r * 0.10, -r * 1.70, chest + r * 1.60);
    d.link(Shape::Cylinder, butt, top, r * 0.20, s.wood, WOOD, 0.0);
    for k in 0..3 {
        let a = k as f32 * 2.094;
        d.sphere(
            [
                top[0] + a.cos() * r * 0.35,
                top[1] + a.sin() * r * 0.35,
                top[2] + r * 0.30,
            ],
            r * 0.75,
            rgba([0.18, 0.32, 0.15], 1.0),
            Material::FOLIAGE,
        );
    }
}

/// Serpent below, torso above: the Naga Siren, Lady Vashj, the Murgul.
fn naga(d: &mut DrawList, p: &Pose, s: &Skin) {
    let r = p.r;
    let sway = (p.t * 1.6).sin() * 0.5;
    for k in 0..6 {
        let f = k as f32;
        let a = p.yaw + sway * (f / 6.0);
        let q = p.at(
            -f * r * 0.34 * a.cos().abs() - f * r * 0.10,
            (f * 0.5).sin() * r * 0.42,
        );
        d.shape(
            Shape::Sphere,
            [q[0], q[1], p.z + r * (0.34 - f * 0.035).max(0.10)],
            [
                r * (0.92 - f * 0.12),
                r * (0.80 - f * 0.10),
                r * (0.60 - f * 0.08),
            ],
            a,
            0.0,
            if k % 2 == 0 { s.skin } else { s.dark },
            FLESH,
            0.0,
        );
    }
    let chest = r * 1.05;
    blob(d, p, chest, r * 0.80, r * 1.05, r * 0.75, s.skin, FLESH);
    if p.fine() {
        for k in 0..3 {
            d.shape(
                Shape::Cylinder,
                p.p3(r * 0.30, 0.0, chest - r * (0.10 + 0.16 * k as f32)),
                [r * 0.62, r * 0.72, r * 0.05],
                p.yaw,
                0.0,
                s.body,
                FLESH,
                0.0,
            );
        }
    }
    for side in [-1.0f32, 1.0] {
        arm(
            d,
            p,
            p.p3(0.0, side * r * 0.55, chest + r * 0.18),
            p.p3(r * 0.42, side * r * 0.78, chest + r * 0.30),
            p.p3(r * 0.70, side * r * 0.58, chest + r * 0.62),
            0.10,
            s,
            false,
        );
    }
    let head = p.at(r * 0.15, 0.0);
    d.sphere(
        [head[0], head[1], p.z + chest + r * 0.72],
        r * 0.50,
        s.skin,
        FLESH,
    );
    for k in 0..5 {
        let a = (k as f32 - 2.0) * 0.42;
        let tip = p.at(-r * 0.55 - a.abs() * r * 0.1, a * r * 1.1);
        d.link(
            Shape::Cone,
            [head[0], head[1], p.z + chest + r * 0.80],
            [tip[0], tip[1], p.z + chest + r * 1.35],
            r * 0.09,
            s.body,
            CLOTH,
            0.0,
        );
    }
    eyes(d, p, head, p.z + chest + r * 0.76, r * 0.22, r * 0.16, s.glow);
}

// ---------------------------------------------------------------- beasts

enum Beast {
    Bear,
    Mammoth,
    Lizard,
}

/// The four-legged frame, dressed three ways.
fn beast(d: &mut DrawList, p: &Pose, s: &Skin, kind: Beast) {
    let r = p.r;
    let (leg, body_l, body_h) = match kind {
        Beast::Bear => (r * 0.60, r * 2.0, r * 1.35),
        Beast::Mammoth => (r * 0.85, r * 2.4, r * 1.75),
        Beast::Lizard => (r * 0.45, r * 2.6, r * 1.00),
    };
    let bz = leg + body_h * 0.55;
    legs(d, p, s.dark, 4, 0.58, leg);
    blob(d, p, bz, body_l, r * 1.45, body_h, s.skin, FLESH);

    let head = p.at(body_l * 0.62, 0.0);
    match kind {
        Beast::Bear => {
            d.sphere(
                [head[0], head[1], p.z + bz + r * 0.25],
                r * 0.75,
                s.dark,
                FLESH,
            );
            d.shape(
                Shape::Sphere,
                p.p3(body_l * 0.85, 0.0, bz + r * 0.12),
                [r * 0.34, r * 0.30, r * 0.26],
                p.yaw,
                0.0,
                s.skin,
                FLESH,
                0.0,
            );
            for side in [-1.0f32, 1.0] {
                let e = p.at(body_l * 0.52, side * r * 0.44);
                d.sphere([e[0], e[1], p.z + bz + r * 0.80], r * 0.30, s.dark, FLESH);
            }
            if p.fine() {
                d.shape(
                    Shape::Sphere,
                    p.p3(body_l * 0.20, 0.0, bz + body_h * 0.42),
                    [r * 0.90, r * 1.10, r * 0.55],
                    p.yaw,
                    0.0,
                    s.dark,
                    CLOTH,
                    0.0,
                );
            }
            eyes(d, p, head, p.z + bz + r * 0.32, r * 0.22, r * 0.22, s.glow);
        }
        Beast::Mammoth => {
            d.sphere(
                [head[0], head[1], p.z + bz + r * 0.20],
                r * 0.90,
                s.dark,
                FLESH,
            );
            let curl = (p.t * 1.4).sin() * r * 0.2;
            let mut prev = [head[0], head[1], p.z + bz];
            for k in 1..4 {
                let f = k as f32;
                let q = p.p3(
                    body_l * (0.62 + f * 0.12) + curl * f * 0.3,
                    0.0,
                    bz - body_h * 0.20 * f,
                );
                d.link(
                    Shape::Capsule,
                    prev,
                    q,
                    r * (0.22 - f * 0.03),
                    s.dark,
                    FLESH,
                    0.0,
                );
                prev = q;
            }
            for side in [-1.0f32, 1.0] {
                d.link(
                    Shape::Cone,
                    p.p3(body_l * 0.60, side * r * 0.45, bz - r * 0.05),
                    p.p3(body_l * 1.15, side * r * 0.70, bz + r * 0.35),
                    r * 0.16,
                    s.bone,
                    STONE,
                    0.0,
                );
                let e = p.at(body_l * 0.48, side * r * 0.70);
                d.shape(
                    Shape::Sphere,
                    [e[0], e[1], p.z + bz + r * 0.40],
                    [r * 0.18, r * 0.62, r * 0.60],
                    p.yaw,
                    0.0,
                    s.dark,
                    FLESH,
                    0.0,
                );
            }
            for k in 0..5 {
                let q = p.at((k as f32 - 2.0) * r * 0.42, 0.0);
                d.sphere(
                    [q[0], q[1], p.z + bz + body_h * 0.45],
                    r * 0.50,
                    s.skin,
                    CLOTH,
                );
            }
            eyes(d, p, head, p.z + bz + r * 0.36, r * 0.20, r * 0.28, s.glow);
        }
        Beast::Lizard => {
            d.shape(
                Shape::Cone,
                [head[0], head[1], p.z + bz - r * 0.05],
                [r * 0.85, r * 0.70, r * 0.95],
                p.yaw,
                std::f32::consts::FRAC_PI_2,
                s.dark,
                FLESH,
                0.0,
            );
            for k in 0..7 {
                let q = p.at(body_l * 0.35 - k as f32 * 0.32 * r, 0.0);
                d.link(
                    Shape::Cone,
                    [q[0], q[1], p.z + bz + body_h * 0.35],
                    [
                        q[0],
                        q[1],
                        p.z + bz + body_h * 0.35 + r * (0.50 - k as f32 * 0.04),
                    ],
                    r * 0.12,
                    s.bone,
                    STONE,
                    0.0,
                );
            }
            let sway = (p.t * 2.0).sin() * 0.5;
            let mut prev = p.p3(-body_l * 0.5, 0.0, bz - r * 0.1);
            for k in 1..4 {
                let f = k as f32;
                let q = p.p3(-body_l * (0.5 + f * 0.28), sway * f * r * 0.3, leg * 0.9);
                d.link(
                    Shape::Capsule,
                    prev,
                    q,
                    r * (0.26 - f * 0.05),
                    s.skin,
                    FLESH,
                    0.0,
                );
                prev = q;
            }
            eyes(d, p, head, p.z + bz + r * 0.20, r * 0.22, r * 0.20, s.glow);
        }
    }
}

/// Horse below, rider above.
fn centaur(d: &mut DrawList, p: &Pose, s: &Skin) {
    let r = p.r;
    let leg = r * 0.72;
    let bz = leg + r * 0.70;
    legs(d, p, s.dark, 4, 0.62, leg);
    blob(d, p, bz, r * 2.1, r * 1.20, r * 1.05, s.skin, FLESH);
    let sway = (p.t * 1.8).sin() * 0.4;
    d.link(
        Shape::Cone,
        p.p3(-r * 1.05, 0.0, bz + r * 0.30),
        p.p3(-r * 1.55, sway * r * 0.5, leg * 0.6),
        r * 0.16,
        s.dark,
        CLOTH,
        0.0,
    );
    let chest = bz + r * 1.05;
    blob(d, p, chest, r * 0.80, r * 1.05, r * 0.80, s.skin, FLESH);
    belt(d, p, bz + r * 0.55, r * 0.85, s);
    for side in [-1.0f32, 1.0] {
        arm(
            d,
            p,
            p.p3(0.0, side * r * 0.62, chest + r * 0.18),
            p.p3(r * 0.45, side * r * 0.80, chest - r * 0.10),
            p.p3(r * 0.85, side * r * 0.70, chest + r * 0.30),
            0.12,
            s,
            false,
        );
    }
    let head = p.at(r * 0.95, 0.0);
    d.sphere(
        [head[0], head[1], p.z + chest + r * 0.70],
        r * 0.52,
        s.skin,
        FLESH,
    );
    horns(d, p, head, p.z + chest + r * 0.78, r * 0.60, s.bone);
    eyes(d, p, head, p.z + chest + r * 0.74, r * 0.22, r * 0.16, s.glow);
    let grip = p.p3(r * 1.0, -r * 0.80, chest + r * 0.10);
    let top = p.p3(r * 0.75, -r * 0.95, chest + r * 1.60);
    d.link(Shape::Capsule, grip, top, r * 0.075, s.wood, WOOD, 0.0);
    d.shape(
        Shape::Prism,
        top,
        [r * 0.55, r * 0.16, 1.0],
        p.yaw,
        0.0,
        s.steel,
        STEEL,
        0.0,
    );
    if p.fine() {
        d.link(
            Shape::Cone,
            [top[0], top[1], top[2] + r * 0.10],
            [top[0], top[1], top[2] + r * 0.45],
            r * 0.06,
            s.steel,
            STEEL,
            0.0,
        );
    }
}

/// Eight legs and a low body.
fn spider(d: &mut DrawList, p: &Pose, s: &Skin) {
    let r = p.r;
    let bz = r * 0.72;
    for k in 0..8 {
        let a = (k as f32 / 8.0) * std::f32::consts::TAU + 0.4;
        let phase = p.t * 3.0 + k as f32;
        let lift = if p.walk {
            (phase.sin() * 0.5 + 0.5) * r * 0.22
        } else {
            0.0
        };
        let knee = p.at(a.cos() * r * 1.05, a.sin() * r * 1.05);
        let foot = p.at(a.cos() * r * 1.75, a.sin() * r * 1.75);
        d.link(
            Shape::Capsule,
            [p.pos[0], p.pos[1], p.z + bz],
            [knee[0], knee[1], p.z + bz + r * 0.45],
            r * 0.10,
            s.dark,
            Material::CHITIN,
            0.0,
        );
        d.link(
            Shape::Capsule,
            [knee[0], knee[1], p.z + bz + r * 0.45],
            [foot[0], foot[1], p.z + lift],
            r * 0.07,
            s.dark,
            Material::CHITIN,
            0.0,
        );
    }
    d.shape(
        Shape::Sphere,
        p.p3(-r * 0.55, 0.0, bz + r * 0.10),
        [r * 1.30, r * 1.15, r * 0.95],
        p.yaw,
        0.0,
        s.skin,
        Material::CHITIN,
        0.0,
    );
    blob(d, p, bz, r * 1.00, r * 0.90, r * 0.70, s.dark, Material::CHITIN);
    let head = p.at(r * 0.85, 0.0);
    d.sphere(
        [head[0], head[1], p.z + bz + r * 0.05],
        r * 0.55,
        s.dark,
        Material::CHITIN,
    );
    if p.fine() {
        for k in 0..3 {
            d.shape(
                Shape::Sphere,
                p.p3(-r * (0.35 + k as f32 * 0.30), 0.0, bz + r * 0.55),
                [r * 0.28, r * 0.34, r * 0.10],
                p.yaw,
                0.0,
                s.body,
                Material::CHITIN,
                0.0,
            );
        }
    }
    for row in 0..2 {
        eyes(
            d,
            p,
            head,
            p.z + bz + r * (0.18 + row as f32 * 0.20),
            r * 0.16,
            r * (0.12 + row as f32 * 0.10),
            s.glow,
        );
    }
    for side in [-1.0f32, 1.0] {
        d.link(
            Shape::Cone,
            p.p3(r * 1.15, side * r * 0.22, bz - r * 0.1),
            p.p3(r * 1.55, side * r * 0.34, bz - r * 0.35),
            r * 0.10,
            s.bone,
            Material::CHITIN,
            0.0,
        );
    }
}

/// The Lobstrokk: a shell, two claws and a lot of legs.
fn crab(d: &mut DrawList, p: &Pose, s: &Skin) {
    let r = p.r;
    let bz = r * 0.60;
    legs(d, p, s.dark, 6, 0.72, bz * 0.8);
    d.shape(
        Shape::Sphere,
        p.p3(0.0, 0.0, bz),
        [r * 1.55, r * 1.85, r * 0.85],
        p.yaw,
        0.0,
        s.skin,
        Material::CHITIN,
        0.0,
    );
    if p.fine() {
        for k in 0..3 {
            d.shape(
                Shape::Cylinder,
                p.p3(r * (0.5 - k as f32 * 0.45), 0.0, bz + r * 0.40),
                [r * (1.30 - k as f32 * 0.2), r * 1.55, r * 0.08],
                p.yaw,
                0.0,
                s.dark,
                Material::CHITIN,
                0.0,
            );
        }
    }
    for side in [-1.0f32, 1.0] {
        let snap = if p.walk {
            (p.t * 3.0 + side).sin() * 0.2
        } else {
            0.0
        };
        let elbow = p.p3(r * 0.95, side * r * 1.05, bz + r * 0.15);
        d.link(
            Shape::Capsule,
            p.p3(r * 0.20, side * r * 0.95, bz),
            elbow,
            r * 0.18,
            s.dark,
            Material::CHITIN,
            0.0,
        );
        for jaw in [-1.0f32, 1.0] {
            d.shape(
                Shape::Cone,
                [elbow[0], elbow[1], elbow[2] + jaw * r * 0.16],
                [r * 0.60, r * 0.24, r * 0.30],
                p.yaw + snap * jaw,
                std::f32::consts::FRAC_PI_2 + jaw * 0.30,
                s.skin,
                Material::CHITIN,
                0.0,
            );
        }
    }
    for side in [-1.0f32, 1.0] {
        let stalk = p.p3(r * 0.55, side * r * 0.30, bz + r * 0.85);
        d.link(
            Shape::Capsule,
            p.p3(r * 0.45, side * r * 0.28, bz + r * 0.35),
            stalk,
            r * 0.06,
            s.dark,
            Material::CHITIN,
            0.0,
        );
        d.sphere_lit(stalk, r * 0.18, s.glow, 0.4);
    }
}

/// Many heads on many necks.
fn serpent(d: &mut DrawList, p: &Pose, s: &Skin) {
    let r = p.r;
    let bz = r * 0.85;
    blob(d, p, bz, r * 1.60, r * 1.40, r * 1.00, s.skin, FLESH);
    legs(d, p, s.dark, 4, 0.55, r * 0.50);
    for (i, side) in [-1.0f32, 0.0, 1.0].iter().enumerate() {
        let wob = (p.t * 1.7 + i as f32 * 2.0).sin() * r * 0.22;
        let neck = p.p3(r * 0.55, side * r * 0.55, bz + r * 0.55);
        let mid = p.p3(r * 0.95 + wob * 0.5, side * r * 0.70, bz + r * 1.05);
        let head = p.p3(r * 1.35 + wob, side * r * 0.85, bz + r * 1.35);
        d.link(Shape::Capsule, neck, mid, r * 0.22, s.skin, FLESH, 0.0);
        d.link(Shape::Capsule, mid, head, r * 0.18, s.skin, FLESH, 0.0);
        d.shape(
            Shape::Cone,
            head,
            [r * 0.52, r * 0.42, r * 0.65],
            p.yaw,
            std::f32::consts::FRAC_PI_2,
            s.dark,
            FLESH,
            0.0,
        );
        d.sphere_lit(
            [head[0], head[1], head[2] + r * 0.12],
            r * 0.12,
            s.glow,
            0.45,
        );
    }
    if p.fine() {
        for k in 0..4 {
            let q = p.at(-r * (0.2 + k as f32 * 0.35), 0.0);
            d.link(
                Shape::Cone,
                [q[0], q[1], p.z + bz + r * 0.45],
                [q[0], q[1], p.z + bz + r * 0.85],
                r * 0.10,
                s.body,
                FLESH,
                0.0,
            );
        }
    }
}

/// A shell you have to get around.
fn turtle(d: &mut DrawList, p: &Pose, s: &Skin) {
    let r = p.r;
    let bz = r * 0.55;
    legs(d, p, s.dark, 4, 0.75, r * 0.40);
    d.shape(
        Shape::Sphere,
        p.p3(0.0, 0.0, bz),
        [r * 2.0, r * 1.90, r * 1.30],
        p.yaw,
        0.0,
        s.dark,
        STONE,
        0.0,
    );
    for k in 0..6 {
        let a = k as f32 * 1.047;
        let q = p.at(a.cos() * r * 0.75, a.sin() * r * 0.70);
        d.shape(
            Shape::Prism,
            [q[0], q[1], p.z + bz + r * 0.55],
            [r * 0.50, r * 0.22, 1.0],
            a,
            0.0,
            s.steel,
            STONE,
            0.0,
        );
    }
    let head = p.at(r * 1.55, 0.0);
    d.shape(
        Shape::Sphere,
        [head[0], head[1], p.z + bz - r * 0.05],
        [r * 0.70, r * 0.50, r * 0.45],
        p.yaw,
        0.0,
        s.skin,
        FLESH,
        0.0,
    );
    eyes(d, p, head, p.z + bz + r * 0.08, r * 0.18, r * 0.16, s.glow);
}

/// A walking tree.
fn ent(d: &mut DrawList, p: &Pose, s: &Skin) {
    let r = p.r;
    let bark = rgba([0.26, 0.19, 0.13], 1.0);
    let leafy = rgba([0.16, 0.30, 0.13], 1.0);
    for side in [-1.0f32, 1.0] {
        let swing = if p.walk {
            (p.t * 1.8 + side).sin() * r * 0.20
        } else {
            0.0
        };
        d.link(
            Shape::Capsule,
            p.p3(0.0, side * r * 0.45, r * 1.10),
            p.p3(swing, side * r * 0.60, 0.0),
            r * 0.30,
            bark,
            WOOD,
            0.0,
        );
        if p.fine() {
            for k in 0..2 {
                let a = (k as f32 - 0.5) * 1.2;
                d.link(
                    Shape::Cone,
                    p.p3(0.0, side * r * 0.60, r * 0.30),
                    p.p3(a.cos() * r * 0.5, side * r * 0.85, 0.0),
                    r * 0.10,
                    bark,
                    WOOD,
                    0.0,
                );
            }
        }
    }
    d.link(
        Shape::Cone,
        p.p3(0.0, 0.0, r * 2.60),
        p.p3(0.0, 0.0, r * 0.90),
        r * 0.85,
        bark,
        WOOD,
        0.0,
    );
    for side in [-1.0f32, 1.0] {
        d.link(
            Shape::Capsule,
            p.p3(0.0, side * r * 0.60, r * 2.20),
            p.p3(r * 0.60, side * r * 1.55, r * 1.30),
            r * 0.20,
            bark,
            WOOD,
            0.0,
        );
        if p.fine() {
            for k in 0..3 {
                let a = (k as f32 - 1.0) * 0.5;
                d.link(
                    Shape::Cone,
                    p.p3(r * 0.60, side * r * 1.55, r * 1.30),
                    p.p3(r * (0.85 + a * 0.3), side * r * 1.90, r * (1.05 + a * 0.4)),
                    r * 0.08,
                    bark,
                    WOOD,
                    0.0,
                );
            }
        }
    }
    for k in 0..4 {
        let a = k as f32 * 1.571 + p.yaw;
        let q = p.at(a.cos() * r * 0.62, a.sin() * r * 0.62);
        d.sphere(
            [q[0], q[1], p.z + r * (2.85 + 0.18 * (k as f32 % 2.0))],
            r * 1.25,
            mix4(leafy, s.body, 0.20),
            Material::FOLIAGE,
        );
    }
    let face = p.at(r * 0.70, 0.0);
    d.shape(
        Shape::Sphere,
        [face[0], face[1], p.z + r * 2.25],
        [r * 0.22, r * 0.60, r * 0.55],
        p.yaw,
        0.0,
        rgba([0.16, 0.11, 0.07], 1.0),
        WOOD,
        0.0,
    );
    eyes(d, p, face, p.z + r * 2.32, r * 0.24, r * 0.22, s.glow);
}

/// Stacked slabs of rock with light in the joints.
fn golem(d: &mut DrawList, p: &Pose, s: &Skin) {
    let r = p.r;
    let hip = r * 0.92;
    let chest = hip + r * 0.88;
    stride(d, p, hip, 0.32, s.steel, s.iron, STONE);
    for (i, (w, h)) in [(1.55f32, 0.55f32), (1.35, 0.50), (1.10, 0.45)]
        .iter()
        .enumerate()
    {
        d.shape(
            Shape::Prism,
            p.p3(0.0, 0.0, hip + r * (0.10 + 0.55 * i as f32)),
            [r * w, r * h, 1.0],
            p.yaw + i as f32 * 0.3,
            0.0,
            if i % 2 == 0 { s.steel } else { s.iron },
            STONE,
            0.0,
        );
        if p.fine() {
            d.shape(
                Shape::Cylinder,
                p.p3(0.0, 0.0, hip + r * (0.36 + 0.55 * i as f32)),
                [r * (w * 0.72), r * (w * 0.62), r * 0.045],
                p.yaw,
                0.0,
                s.glow,
                Material::GEM,
                0.35,
            );
        }
    }
    for side in [-1.0f32, 1.0] {
        d.sphere(
            p.p3(0.0, side * r * 1.05, chest + r * 0.20),
            r * 0.52,
            s.iron,
            STONE,
        );
        d.link(
            Shape::Capsule,
            p.p3(0.0, side * r * 1.00, chest + r * 0.15),
            p.p3(r * 0.25, side * r * 1.15, hip - r * 0.55),
            r * 0.30,
            s.steel,
            STONE,
            0.0,
        );
        d.sphere(
            p.p3(r * 0.28, side * r * 1.18, hip - r * 0.62),
            r * 0.42,
            s.iron,
            STONE,
        );
    }
    let head = p.at(r * 0.10, 0.0);
    d.shape(
        Shape::Prism,
        [head[0], head[1], p.z + chest + r * 0.70],
        [r * 0.78, r * 0.50, 1.0],
        p.yaw,
        0.0,
        s.iron,
        STONE,
        0.0,
    );
    eyes(d, p, head, p.z + chest + r * 0.78, r * 0.26, r * 0.20, s.glow);
    p.glow(d, p.p3(0.0, 0.0, chest), r * 1.6, 0.2, s.glow);
}

/// A boulder that is on fire.
fn infernal(d: &mut DrawList, p: &Pose, s: &Skin) {
    let r = p.r;
    let hip = r * 0.85;
    let chest = hip + r * 0.90;
    let ember = rgba([1.0, 0.42, 0.12], 1.0);
    stride(d, p, hip, 0.34, s.iron, s.iron, STONE);
    for k in 0..6 {
        let a = k as f32 * 1.047 + p.yaw;
        let q = p.at(a.cos() * r * 0.40, a.sin() * r * 0.45);
        d.shape(
            Shape::Prism,
            [
                q[0],
                q[1],
                p.z + chest - r * 0.25 + (k as f32 % 3.0) * r * 0.32,
            ],
            [r * 0.85, r * 0.52, 1.0],
            a,
            0.0,
            s.iron,
            STONE,
            0.0,
        );
    }
    for side in [-1.0f32, 1.0] {
        d.link(
            Shape::Capsule,
            p.p3(0.0, side * r * 0.95, chest + r * 0.20),
            p.p3(r * 0.30, side * r * 1.10, hip - r * 0.50),
            r * 0.30,
            s.iron,
            STONE,
            0.0,
        );
        d.sphere(
            p.p3(r * 0.32, side * r * 1.12, hip - r * 0.58),
            r * 0.40,
            s.iron,
            STONE,
        );
    }
    let head = p.at(r * 0.10, 0.0);
    d.sphere(
        [head[0], head[1], p.z + chest + r * 0.75],
        r * 0.55,
        s.iron,
        STONE,
    );
    eyes(d, p, head, p.z + chest + r * 0.80, r * 0.30, r * 0.20, ember);
    for k in 0..5 {
        let a = k as f32 * 1.257 + p.t;
        let q = p.at(a.cos() * r * 0.50, a.sin() * r * 0.50);
        let flick = (p.t * 6.0 + k as f32).sin() * 0.5 + 0.5;
        d.link(
            Shape::Cone,
            [q[0], q[1], p.z + chest - r * 0.2],
            [q[0], q[1], p.z + chest + r * (0.4 + flick * 0.5)],
            r * 0.16,
            ember,
            Material::GEM,
            0.0,
        );
        p.glow(
            d,
            [q[0], q[1], p.z + chest + flick * r * 0.5],
            r * (0.9 + flick * 0.5),
            0.8,
            ember,
        );
    }
}

/// Fire with a shape: no legs, a whirl of flame and a crown.
fn flame_lord(d: &mut DrawList, p: &Pose, s: &Skin) {
    let r = p.r;
    let ember = mix4(rgba([1.0, 0.48, 0.14], 1.0), s.body, 0.25);
    let core = mix4(rgba([1.0, 0.86, 0.45], 1.0), s.glow, 0.25);
    let float = (p.t * 1.4).sin() * r * 0.14 + r * 0.35;
    d.shape(
        Shape::Cone,
        p.p3(0.0, 0.0, float),
        [r * 1.50, r * 1.40, r * 2.10],
        p.yaw + p.t * 0.6,
        0.0,
        ember,
        Material::GEM,
        0.0,
    );
    blob(
        d,
        p,
        float + r * 1.85,
        r * 0.90,
        r * 1.00,
        r * 0.70,
        core,
        Material::GEM,
    );
    for k in 0..6 {
        let a = k as f32 * 1.047 + p.t * 1.5;
        let q = p.at(a.cos() * r * 0.80, a.sin() * r * 0.80);
        let h = (p.t * 5.0 + k as f32).sin() * 0.5 + 0.5;
        d.link(
            Shape::Cone,
            [q[0], q[1], p.z + float + r * 0.30],
            [q[0], q[1], p.z + float + r * (1.2 + h * 0.8)],
            r * 0.22,
            ember,
            Material::GEM,
            0.0,
        );
        d.sphere_lit(
            [q[0], q[1], p.z + float + r * (1.4 + h)],
            r * 0.20,
            core,
            0.6,
        );
    }
    p.glow(d, p.p3(0.0, 0.0, float + r * 1.2), r * 3.0, 0.9, ember);
    let face = p.at(r * 0.30, 0.0);
    eyes(
        d,
        p,
        face,
        p.z + float + r * 1.95,
        r * 0.26,
        r * 0.20,
        rgba([1.0, 1.0, 0.9], 1.0),
    );
}

// ---------------------------------------------------------------- wings

/// A rotor, a cockpit and a lot of noise.
fn gyrocopter(d: &mut DrawList, p: &Pose, s: &Skin) {
    let r = p.r;
    blob(d, p, r * 0.15, r * 1.35, r * 0.95, r * 0.85, s.steel, STEEL);
    d.sphere_lit(p.p3(r * 0.55, 0.0, r * 0.25), r * 0.62, s.glow, 0.2);
    d.link(
        Shape::Capsule,
        p.p3(-r * 0.60, 0.0, r * 0.20),
        p.p3(-r * 1.90, 0.0, r * 0.35),
        r * 0.13,
        s.iron,
        IRON,
        0.0,
    );
    d.shape(
        Shape::Prism,
        p.p3(-r * 1.85, 0.0, r * 0.60),
        [r * 0.45, r * 0.12, 1.0],
        p.yaw + std::f32::consts::FRAC_PI_2,
        0.0,
        s.body,
        STEEL,
        0.0,
    );
    if p.fine() {
        for side in [-1.0f32, 1.0] {
            d.link(
                Shape::Capsule,
                p.p3(-r * 1.70, 0.0, r * 0.35),
                p.p3(-r * 1.70, side * r * 0.55, r * 0.40),
                r * 0.05,
                s.iron,
                IRON,
                0.0,
            );
            d.link(
                Shape::Capsule,
                p.p3(r * 0.40, side * r * 0.42, -r * 0.30),
                p.p3(-r * 0.50, side * r * 0.42, -r * 0.30),
                r * 0.05,
                s.iron,
                IRON,
                0.0,
            );
        }
    }
    let mast = p.p3(0.0, 0.0, r * 1.05);
    d.link(
        Shape::Cylinder,
        p.p3(0.0, 0.0, r * 0.55),
        mast,
        r * 0.10,
        s.iron,
        IRON,
        0.0,
    );
    for k in 0..3 {
        let a = p.t * 9.0 + k as f32 * 2.094;
        let tip = [p.pos[0] + a.cos() * r * 1.95, p.pos[1] + a.sin() * r * 1.95];
        d.link(
            Shape::Capsule,
            mast,
            [tip[0], tip[1], mast[2]],
            r * 0.06,
            s.iron,
            IRON,
            0.0,
        );
    }
}

/// A bird made of fire.
fn phoenix(d: &mut DrawList, p: &Pose, s: &Skin) {
    let r = p.r;
    let ember = rgba([1.0, 0.55, 0.16], 1.0);
    let core = rgba([1.0, 0.88, 0.50], 1.0);
    blob(
        d,
        p,
        r * 0.10,
        r * 1.30,
        r * 0.70,
        r * 0.70,
        ember,
        Material::GEM,
    );
    let head = p.at(r * 0.95, 0.0);
    d.sphere_lit([head[0], head[1], p.z + r * 0.35], r * 0.40, core, 0.55);
    d.shape(
        Shape::Cone,
        p.p3(r * 1.40, 0.0, r * 0.30),
        [r * 0.24, r * 0.20, r * 0.50],
        p.yaw,
        std::f32::consts::FRAC_PI_2,
        s.bone,
        STONE,
        0.0,
    );
    for k in 0..3 {
        let a = (k as f32 - 1.0) * 0.3;
        d.link(
            Shape::Cone,
            [head[0], head[1], p.z + r * 0.55],
            p.p3(r * (0.70 - a.abs() * 0.2), a * r * 0.4, r * 0.95),
            r * 0.06,
            core,
            Material::GEM,
            0.0,
        );
    }
    wings(d, p, r * 0.25, r * 2.4, ember, false);
    for k in 0..5 {
        let a = (k as f32 - 2.0) * 0.24;
        let tip = p.at(-r * 2.1, a * r * 1.2);
        d.link(
            Shape::Cone,
            p.p3(-r * 0.70, a * r * 0.3, r * 0.15),
            [tip[0], tip[1], p.z + r * 0.35],
            r * 0.12,
            ember,
            Material::GEM,
            0.0,
        );
    }
    p.glow(d, p.p3(0.0, 0.0, r * 0.3), r * 2.6, 0.85, ember);
}

/// Wings, claws and a scream.
fn harpy(d: &mut DrawList, p: &Pose, s: &Skin) {
    let r = p.r;
    blob(d, p, r * 0.05, r * 0.75, r * 0.85, r * 1.00, s.skin, FLESH);
    if p.fine() {
        for k in 0..3 {
            d.shape(
                Shape::Sphere,
                p.p3(r * 0.35, 0.0, r * (0.10 - k as f32 * 0.22)),
                [r * 0.30, r * 0.55, r * 0.20],
                p.yaw,
                0.0,
                s.body,
                CLOTH,
                0.0,
            );
        }
    }
    let head = p.at(r * 0.25, 0.0);
    d.sphere([head[0], head[1], p.z + r * 0.85], r * 0.46, s.dark, FLESH);
    eyes(d, p, head, p.z + r * 0.88, r * 0.20, r * 0.15, s.glow);
    for k in 0..3 {
        let a = (k as f32 - 1.0) * 0.28;
        let tip = p.at(-r * 0.55, a * r * 0.45);
        d.link(
            Shape::Cone,
            [head[0], head[1], p.z + r * 0.95],
            [tip[0], tip[1], p.z + r * 1.15],
            r * 0.10,
            s.dark,
            CLOTH,
            0.0,
        );
    }
    wings(d, p, r * 0.35, r * 2.1, s.dark, true);
    for side in [-1.0f32, 1.0] {
        let knee = p.p3(0.0, side * r * 0.32, -r * 0.45);
        d.link(
            Shape::Capsule,
            p.p3(0.0, side * r * 0.30, -r * 0.05),
            knee,
            r * 0.09,
            s.dark,
            FLESH,
            0.0,
        );
        d.link(
            Shape::Cone,
            knee,
            p.p3(r * 0.22, side * r * 0.32, -r * 0.72),
            r * 0.08,
            s.bone,
            STONE,
            0.0,
        );
    }
}

/// Long neck, long tail, big wings. Frost or flesh.
fn dragon(d: &mut DrawList, p: &Pose, s: &Skin, skeletal: bool) {
    let r = p.r;
    let body = if skeletal { s.bone } else { s.skin };
    let trim = if skeletal { s.bone } else { s.dark };
    blob(d, p, 0.0, r * 1.70, r * 1.05, r * 0.90, body, FLESH);
    let mut prev = p.p3(r * 0.80, 0.0, r * 0.20);
    for k in 1..5 {
        let f = k as f32;
        let q = p.p3(r * (0.80 + f * 0.42), 0.0, r * (0.20 + f * 0.26));
        d.link(
            Shape::Capsule,
            prev,
            q,
            r * (0.30 - f * 0.04),
            body,
            FLESH,
            0.0,
        );
        prev = q;
    }
    d.shape(
        Shape::Cone,
        prev,
        [r * 0.70, r * 0.48, r * 0.85],
        p.yaw,
        std::f32::consts::FRAC_PI_2,
        trim,
        FLESH,
        0.0,
    );
    if p.fine() {
        d.shape(
            Shape::Prism,
            [prev[0], prev[1], prev[2] - r * 0.18],
            [r * 0.42, r * 0.16, 1.0],
            p.yaw,
            0.0,
            trim,
            FLESH,
            0.0,
        );
    }
    horns(d, p, [prev[0], prev[1]], prev[2] + r * 0.15, r * 0.60, s.bone);
    eyes(
        d,
        p,
        [prev[0], prev[1]],
        prev[2] + r * 0.10,
        r * 0.22,
        r * 0.18,
        s.glow,
    );
    wings(d, p, r * 0.35, r * 3.0, trim, !skeletal);
    for side in [-1.0f32, 1.0] {
        d.link(
            Shape::Capsule,
            p.p3(r * 0.30, side * r * 0.55, -r * 0.10),
            p.p3(r * 0.55, side * r * 0.72, -r * 0.70),
            r * 0.13,
            body,
            FLESH,
            0.0,
        );
    }
    let sway = (p.t * 1.5).sin() * 0.6;
    let mut tprev = p.p3(-r * 0.80, 0.0, r * 0.05);
    for k in 1..6 {
        let f = k as f32;
        let q = p.p3(
            -r * (0.80 + f * 0.50),
            sway * f * r * 0.20,
            r * (0.05 + f * 0.05),
        );
        d.link(
            Shape::Capsule,
            tprev,
            q,
            r * (0.24 - f * 0.035),
            body,
            FLESH,
            0.0,
        );
        tprev = q;
    }
    if skeletal {
        for k in 0..5 {
            let q = p.at(r * (0.45 - k as f32 * 0.32), 0.0);
            d.shape(
                Shape::Cylinder,
                [q[0], q[1], p.z],
                [r * (1.0 - k as f32 * 0.08), r * 0.75, r * 0.055],
                p.yaw,
                0.0,
                s.bone,
                STONE,
                0.0,
            );
        }
        p.glow(d, p.p3(0.0, 0.0, 0.0), r * 2.4, 0.5, s.glow);
    } else if p.fine() {
        for k in 0..6 {
            let q = p.at(r * (0.60 - k as f32 * 0.34), 0.0);
            d.link(
                Shape::Cone,
                [q[0], q[1], p.z + r * 0.38],
                [q[0], q[1], p.z + r * 0.70],
                r * 0.09,
                s.body,
                FLESH,
                0.0,
            );
        }
    }
}

// ---------------------------------------------------------------- machines

/// Four flavours of gun emplacement, growing meaner with the level.
fn turret(d: &mut DrawList, p: &Pose, s: &Skin, kind: u8) {
    let r = p.r;
    d.cylinder(p.p3(0.0, 0.0, 0.0), r * 1.85, r * 0.40, 0.0, s.iron, IRON);
    if p.fine() {
        for k in 0..8 {
            let a = k as f32 * 0.785;
            d.sphere(
                p.p3(a.cos() * r * 0.80, a.sin() * r * 0.80, r * 0.42),
                r * 0.10,
                s.steel,
                STEEL,
            );
        }
    }
    d.cylinder(
        p.p3(0.0, 0.0, r * 0.38),
        r * 1.30,
        r * 0.55,
        p.yaw,
        s.steel,
        STEEL,
    );
    d.shape(
        Shape::Prism,
        p.p3(r * 0.35, 0.0, r * 0.60),
        [r * 0.70, r * 0.45, 1.0],
        p.yaw,
        0.0,
        s.iron,
        IRON,
        0.0,
    );
    let barrel_z = r * 0.85;
    let n = match kind {
        1 => 1,
        2 => 2,
        3 => 2,
        _ => 4,
    };
    let len = r * (1.3 + 0.25 * kind as f32);
    for i in 0..n {
        let off = (i as f32 - (n as f32 - 1.0) * 0.5) * r * 0.34;
        d.link(
            Shape::Cylinder,
            p.p3(r * 0.20, off, barrel_z),
            p.p3(len, off, barrel_z),
            r * (0.16 - 0.02 * n as f32).max(0.06),
            s.steel,
            STEEL,
            0.0,
        );
        if p.fine() {
            d.shape(
                Shape::Cylinder,
                p.p3(len * 0.98, off, barrel_z),
                [r * 0.16, r * 0.16, r * 0.08],
                p.yaw,
                std::f32::consts::FRAC_PI_2,
                s.iron,
                IRON,
                0.0,
            );
        }
    }
    if kind >= 3 {
        d.sphere_lit(
            p.p3(-r * 0.35, 0.0, barrel_z + r * 0.45),
            r * 0.36,
            s.glow,
            0.35,
        );
    }
    if kind == 4 {
        d.link(
            Shape::Capsule,
            p.p3(r * 0.10, r * 0.55, barrel_z - r * 0.10),
            p.p3(-r * 0.90, r * 0.70, r * 0.35),
            r * 0.14,
            s.iron,
            IRON,
            0.0,
        );
        d.shape(
            Shape::Box,
            p.p3(-r * 1.05, r * 0.72, r * 0.30),
            [r * 0.55, r * 0.40, r * 0.45],
            p.yaw,
            0.0,
            s.body,
            IRON,
            0.0,
        );
    }
}

/// A rack of missiles pointed at the sky - the map's anti-air, and it looks it.
fn sam_site(d: &mut DrawList, p: &Pose, s: &Skin) {
    let r = p.r;
    d.shape(
        Shape::Prism,
        p.p3(0.0, 0.0, 0.0),
        [r * 1.90, r * 0.35, 1.0],
        p.yaw,
        0.0,
        s.iron,
        IRON,
        0.0,
    );
    d.cylinder(
        p.p3(0.0, 0.0, r * 0.32),
        r * 1.10,
        r * 0.40,
        p.yaw,
        s.steel,
        STEEL,
    );
    for i in 0..4 {
        let side = if i % 2 == 0 { -1.0f32 } else { 1.0 };
        let row = (i / 2) as f32;
        let base = p.p3(-r * 0.30 + row * r * 0.20, side * r * 0.42, r * 0.65);
        let tip = p.p3(r * 0.85 + row * r * 0.15, side * r * 0.62, r * 2.05);
        d.link(Shape::Cylinder, base, tip, r * 0.17, s.steel, STEEL, 0.0);
        d.sphere_lit([tip[0], tip[1], tip[2] + r * 0.05], r * 0.12, s.glow, 0.45);
        if p.fine() {
            for f in [-1.0f32, 1.0] {
                d.shape(
                    Shape::Quad,
                    p.p3(r * 0.10 + row * r * 0.18, side * r * 0.52, r * 1.05),
                    [r * 0.30, r * 0.30, 1.0],
                    p.yaw + f * 0.8,
                    1.2,
                    s.body,
                    STEEL,
                    0.0,
                );
            }
        }
    }
    d.link(
        Shape::Cylinder,
        p.p3(-r * 0.75, 0.0, r * 0.55),
        p.p3(-r * 0.75, 0.0, r * 1.15),
        r * 0.07,
        s.iron,
        IRON,
        0.0,
    );
    d.shape(
        Shape::Cone,
        p.p3(-r * 0.75, 0.0, r * 1.30),
        [r * 0.75, r * 0.70, r * 0.35],
        p.yaw + p.t * 0.8,
        -0.9,
        s.steel,
        STEEL,
        0.0,
    );
}

/// One enormous barrel on a carriage.
fn cannon(d: &mut DrawList, p: &Pose, s: &Skin) {
    let r = p.r;
    for side in [-1.0f32, 1.0] {
        d.shape(
            Shape::Cylinder,
            p.p3(-r * 0.30, side * r * 0.95, r * 0.55),
            [r * 1.10, r * 1.10, r * 0.22],
            p.yaw,
            std::f32::consts::FRAC_PI_2,
            s.wood,
            WOOD,
            0.0,
        );
        if p.fine() {
            for k in 0..6 {
                let a = k as f32 * 1.047 + p.yaw;
                d.link(
                    Shape::Capsule,
                    p.p3(-r * 0.30, side * r * 0.95, r * 0.55),
                    p.p3(
                        -r * 0.30 + a.cos() * r * 0.50,
                        side * r * 0.95,
                        r * 0.55 + a.sin() * r * 0.50,
                    ),
                    r * 0.05,
                    s.iron,
                    IRON,
                    0.0,
                );
            }
        }
        d.link(
            Shape::Capsule,
            p.p3(-r * 0.20, side * r * 0.55, r * 0.55),
            p.p3(-r * 1.70, side * r * 0.85, 0.0),
            r * 0.13,
            s.wood,
            WOOD,
            0.0,
        );
    }
    d.link(
        Shape::Cylinder,
        p.p3(-r * 0.55, 0.0, r * 0.80),
        p.p3(r * 1.90, 0.0, r * 1.15),
        r * 0.34,
        s.iron,
        IRON,
        0.0,
    );
    d.sphere(p.p3(-r * 0.65, 0.0, r * 0.78), r * 0.55, s.iron, IRON);
    if p.fine() {
        for k in 0..3 {
            let f = 0.1 + k as f32 * 0.34;
            d.shape(
                Shape::Cylinder,
                p.p3(-r * 0.55 + f * r * 2.45, 0.0, r * (0.80 + f * 0.35)),
                [r * 0.42, r * 0.42, r * 0.08],
                p.yaw,
                std::f32::consts::FRAC_PI_2,
                s.steel,
                STEEL,
                0.0,
            );
        }
    }
    d.cylinder(
        p.p3(r * 1.85, 0.0, r * 1.12),
        r * 0.52,
        r * 0.18,
        p.yaw,
        s.steel,
        STEEL,
    );
}

/// A catapult with something unpleasant on it.
fn meat_wagon(d: &mut DrawList, p: &Pose, s: &Skin) {
    let r = p.r;
    for side in [-1.0f32, 1.0] {
        for fwd in [-0.75f32, 0.75] {
            d.shape(
                Shape::Cylinder,
                p.p3(fwd * r, side * r * 0.95, r * 0.45),
                [r * 0.90, r * 0.90, r * 0.20],
                p.yaw,
                std::f32::consts::FRAC_PI_2,
                s.wood,
                WOOD,
                0.0,
            );
        }
    }
    d.shape(
        Shape::Prism,
        p.p3(0.0, 0.0, r * 0.60),
        [r * 1.70, r * 0.60, 1.0],
        p.yaw,
        0.0,
        s.wood,
        WOOD,
        0.0,
    );
    let pivot = p.p3(-r * 0.30, 0.0, r * 0.95);
    let head = p.p3(-r * 1.50, 0.0, r * 1.95);
    d.link(Shape::Capsule, pivot, head, r * 0.16, s.wood, WOOD, 0.0);
    d.sphere(head, r * 0.50, s.skin, FLESH);
    if p.fine() {
        d.link(
            Shape::Capsule,
            pivot,
            p.p3(r * 0.60, 0.0, r * 1.35),
            r * 0.08,
            s.iron,
            IRON,
            0.0,
        );
    }
    for k in 0..5 {
        let a = k as f32 * 1.257;
        let q = p.at(r * 0.85 + a.cos() * r * 0.28, a.sin() * r * 0.34);
        d.link(
            Shape::Capsule,
            [q[0], q[1], p.z + r * 0.80],
            [q[0], q[1], p.z + r * 1.55],
            r * 0.06,
            s.iron,
            IRON,
            0.0,
        );
    }
    d.sphere(p.p3(r * 0.85, 0.0, r * 1.20), r * 0.42, s.bone, STONE);
}

/// A hull, a mast and a broadside.
fn ship(d: &mut DrawList, p: &Pose, s: &Skin) {
    let r = p.r;
    d.shape(
        Shape::Sphere,
        p.p3(0.0, 0.0, r * 0.35),
        [r * 2.30, r * 0.95, r * 0.60],
        p.yaw,
        0.0,
        s.wood,
        WOOD,
        0.0,
    );
    if p.fine() {
        for k in 0..3 {
            d.shape(
                Shape::Sphere,
                p.p3(0.0, 0.0, r * (0.20 + k as f32 * 0.16)),
                [
                    r * (2.25 - k as f32 * 0.1),
                    r * (0.98 - k as f32 * 0.04),
                    r * 0.05,
                ],
                p.yaw,
                0.0,
                s.leather,
                WOOD,
                0.0,
            );
        }
    }
    d.shape(
        Shape::Prism,
        p.p3(-r * 1.20, 0.0, r * 0.70),
        [r * 0.85, r * 0.55, 1.0],
        p.yaw,
        0.0,
        s.leather,
        WOOD,
        0.0,
    );
    d.link(
        Shape::Cylinder,
        p.p3(r * 1.15, 0.0, r * 0.45),
        p.p3(r * 1.95, 0.0, r * 0.75),
        r * 0.06,
        s.wood,
        WOOD,
        0.0,
    );
    let top = p.p3(r * 0.10, 0.0, r * 2.60);
    d.link(
        Shape::Cylinder,
        p.p3(r * 0.10, 0.0, r * 0.55),
        top,
        r * 0.09,
        s.wood,
        WOOD,
        0.0,
    );
    d.link(
        Shape::Cylinder,
        p.p3(r * 0.10, -r * 0.85, r * 2.30),
        p.p3(r * 0.10, r * 0.85, r * 2.30),
        r * 0.05,
        s.wood,
        WOOD,
        0.0,
    );
    d.shape(
        Shape::Quad,
        p.p3(r * 0.10, 0.0, r * 1.70),
        [r * 1.50, r * 1.60, 1.0],
        p.yaw + std::f32::consts::FRAC_PI_2,
        0.0,
        s.bone,
        CLOTH,
        0.0,
    );
    if p.fine() {
        d.shape(
            Shape::Quad,
            p.p3(r * 0.12, 0.0, r * 1.70),
            [r * 0.60, r * 0.60, 1.0],
            p.yaw + std::f32::consts::FRAC_PI_2,
            0.0,
            s.body,
            CLOTH,
            0.0,
        );
    }
    for side in [-1.0f32, 1.0] {
        for fwd in [-0.6f32, 0.2, 1.0] {
            d.link(
                Shape::Cylinder,
                p.p3(fwd * r, side * r * 0.55, r * 0.45),
                p.p3(fwd * r, side * r * 1.05, r * 0.45),
                r * 0.08,
                s.iron,
                IRON,
                0.0,
            );
        }
    }
}

// ---------------------------------------------------------------- buildings

/// A standing stone with runes cut into it.
fn obelisk(d: &mut DrawList, p: &Pose, s: &Skin) {
    let r = p.r;
    d.shape(
        Shape::Prism,
        p.p3(0.0, 0.0, 0.0),
        [r * 1.70, r * 0.30, 1.0],
        p.yaw,
        0.0,
        s.iron,
        STONE,
        0.0,
    );
    d.shape(
        Shape::Prism,
        p.p3(0.0, 0.0, r * 0.25),
        [r * 1.05, r * 2.60, 1.0],
        p.yaw,
        0.0,
        s.steel,
        STONE,
        0.0,
    );
    d.shape(
        Shape::Pyramid,
        p.p3(0.0, 0.0, r * 2.85),
        [r * 1.05, r * 1.05, r * 0.85],
        p.yaw,
        0.0,
        s.iron,
        STONE,
        0.0,
    );
    for k in 0..3 {
        let z = p.z + r * (0.80 + 0.60 * k as f32);
        let q = p.at(r * 0.53, 0.0);
        d.shape(
            Shape::Quad,
            [q[0], q[1], z],
            [r * 0.36, r * 0.36, 1.0],
            p.yaw,
            0.0,
            s.glow,
            Material::GEM,
            0.55,
        );
    }
    if p.fine() {
        for side in [-1.0f32, 1.0] {
            d.link(
                Shape::Capsule,
                p.p3(0.0, side * r * 0.50, r * 2.70),
                p.p3(0.0, side * r * 0.80, r * 1.90),
                r * 0.03,
                s.iron,
                IRON,
                0.0,
            );
        }
    }
    p.glow(d, p.p3(0.0, 0.0, r * 1.6), r * 1.8, 0.25, s.glow);
}

/// A slim stone tower with a crystal on top.
fn magic_tower(d: &mut DrawList, p: &Pose, s: &Skin) {
    let r = p.r;
    d.cylinder(p.p3(0.0, 0.0, 0.0), r * 1.90, r * 0.35, 0.0, s.iron, STONE);
    d.cylinder(
        p.p3(0.0, 0.0, r * 0.30),
        r * 1.30,
        r * 2.10,
        0.0,
        s.steel,
        STONE,
    );
    if p.fine() {
        for k in 0..4 {
            d.shape(
                Shape::Cylinder,
                p.p3(0.0, 0.0, r * (0.55 + k as f32 * 0.42)),
                [r * 1.36, r * 1.36, r * 0.05],
                p.yaw,
                0.0,
                s.iron,
                STONE,
                0.0,
            );
        }
    }
    for k in 0..4 {
        let a = k as f32 * 1.571 + p.yaw;
        let q = p.at(a.cos() * r * 0.80, a.sin() * r * 0.80);
        d.link(
            Shape::Capsule,
            [q[0], q[1], p.z + r * 0.30],
            [q[0], q[1], p.z + r * 1.70],
            r * 0.14,
            s.iron,
            STONE,
            0.0,
        );
    }
    d.shape(
        Shape::Cone,
        p.p3(0.0, 0.0, r * 2.40),
        [r * 1.55, r * 1.55, r * 1.00],
        p.yaw,
        0.0,
        s.iron,
        STONE,
        0.0,
    );
    if p.fine() {
        for k in 0..3 {
            let a = k as f32 * 2.094 + p.yaw;
            d.link(
                Shape::Cone,
                p.p3(a.cos() * r * 0.35, a.sin() * r * 0.35, r * 3.10),
                p.p3(0.0, 0.0, r * 3.55),
                r * 0.07,
                s.steel,
                STEEL,
                0.0,
            );
        }
    }
    d.shape(
        Shape::Prism,
        p.p3(0.0, 0.0, r * 3.60),
        [r * 0.62, r * 0.80, 1.0],
        p.yaw + p.t * 0.4,
        0.0,
        s.glow,
        Material::GEM,
        0.45,
    );
    p.glow(d, p.p3(0.0, 0.0, r * 3.60), r * 2.0, 0.6, s.glow);
}

/// A dome on a drum, with a lens looking out of it.
fn observatory(d: &mut DrawList, p: &Pose, s: &Skin) {
    let r = p.r;
    d.cylinder(p.p3(0.0, 0.0, 0.0), r * 2.20, r * 0.35, 0.0, s.iron, STONE);
    d.cylinder(
        p.p3(0.0, 0.0, r * 0.30),
        r * 1.85,
        r * 1.20,
        0.0,
        s.steel,
        STONE,
    );
    if p.fine() {
        for k in 0..6 {
            let a = k as f32 * 1.047 + p.yaw;
            let q = p.at(a.cos() * r * 0.92, a.sin() * r * 0.92);
            d.shape(
                Shape::Quad,
                [q[0], q[1], p.z + r * 0.70],
                [r * 0.30, r * 0.45, 1.0],
                a + std::f32::consts::FRAC_PI_2,
                std::f32::consts::FRAC_PI_2,
                s.glow,
                Material::GEM,
                0.30,
            );
        }
    }
    d.shape(
        Shape::Sphere,
        p.p3(0.0, 0.0, r * 1.55),
        [r * 1.95, r * 1.95, r * 1.40],
        p.yaw,
        0.0,
        s.iron,
        STEEL,
        0.0,
    );
    let base = p.p3(0.0, 0.0, r * 1.75);
    let tip = p.p3(r * 1.85, 0.0, r * 3.00);
    d.link(Shape::Cylinder, base, tip, r * 0.26, s.steel, STEEL, 0.0);
    d.shape(
        Shape::Cylinder,
        tip,
        [r * 0.40, r * 0.40, r * 0.10],
        p.yaw,
        std::f32::consts::FRAC_PI_2,
        s.glow,
        Material::GEM,
        0.35,
    );
    for k in 0..2 {
        let a = p.t * (0.6 + k as f32 * 0.35);
        d.shape(
            Shape::Cylinder,
            p.p3(0.0, 0.0, r * (1.6 + k as f32 * 0.35)),
            [
                r * (2.4 - k as f32 * 0.4),
                r * (2.4 - k as f32 * 0.4),
                r * 0.06,
            ],
            a,
            0.35 + k as f32 * 0.3,
            s.glow,
            Material::GEM,
            0.25,
        );
    }
}

/// An arch with something on the other side of it.
fn demon_gate(d: &mut DrawList, p: &Pose, s: &Skin) {
    let r = p.r;
    for side in [-1.0f32, 1.0] {
        d.shape(
            Shape::Prism,
            p.p3(0.0, side * r * 1.15, 0.0),
            [r * 0.70, r * 2.40, 1.0],
            p.yaw,
            0.0,
            s.iron,
            STONE,
            0.0,
        );
        d.sphere(
            p.p3(0.0, side * r * 1.15, r * 2.55),
            r * 0.34,
            s.bone,
            STONE,
        );
        if p.fine() {
            d.link(
                Shape::Cone,
                p.p3(0.0, side * r * 1.15, r * 2.75),
                p.p3(0.0, side * r * 1.15, r * 3.20),
                r * 0.08,
                s.iron,
                IRON,
                0.0,
            );
        }
    }
    d.shape(
        Shape::Prism,
        p.p3(0.0, 0.0, r * 2.35),
        [r * 0.60, r * 1.55, 1.0],
        p.yaw + std::f32::consts::FRAC_PI_2,
        0.0,
        s.iron,
        STONE,
        0.0,
    );
    d.shape(
        Shape::Quad,
        p.p3(0.0, 0.0, r * 1.20),
        [r * 1.90, r * 2.10, 1.0],
        p.yaw + std::f32::consts::FRAC_PI_2,
        0.0,
        s.glow,
        Material::GEM,
        0.60,
    );
    p.glow(d, p.p3(0.0, 0.0, r * 1.20), r * 2.4, 0.7, s.glow);
}

/// A squat block with a bowl cut into the top and stains down the side.
fn altar(d: &mut DrawList, p: &Pose, s: &Skin) {
    let r = p.r;
    d.shape(
        Shape::Prism,
        p.p3(0.0, 0.0, 0.0),
        [r * 2.30, r * 0.30, 1.0],
        p.yaw,
        0.0,
        s.iron,
        STONE,
        0.0,
    );
    d.shape(
        Shape::Prism,
        p.p3(0.0, 0.0, r * 0.25),
        [r * 1.75, r * 1.10, 1.0],
        p.yaw,
        0.0,
        s.steel,
        STONE,
        0.0,
    );
    d.cylinder(
        p.p3(0.0, 0.0, r * 1.30),
        r * 1.40,
        r * 0.30,
        0.0,
        s.iron,
        STONE,
    );
    let flick = (p.t * 5.0).sin() * 0.5 + 0.5;
    d.shape(
        Shape::Cone,
        p.p3(0.0, 0.0, r * 1.55),
        [r * 0.85, r * 0.85, r * (0.8 + flick * 0.5)],
        p.t,
        0.0,
        s.glow,
        Material::GEM,
        0.0,
    );
    p.glow(d, p.p3(0.0, 0.0, r * 1.9), r * 2.2, 0.75, s.glow);
    for k in 0..4 {
        let a = k as f32 * 1.571 + p.yaw + 0.78;
        let q = p.at(a.cos() * r * 1.55, a.sin() * r * 1.55);
        d.shape(
            Shape::Prism,
            [q[0], q[1], p.z + r * 0.25],
            [r * 0.40, r * 1.30, 1.0],
            a,
            0.0,
            s.iron,
            STONE,
            0.0,
        );
        if p.fine() {
            d.sphere([q[0], q[1], p.z + r * 1.65], r * 0.18, s.bone, STONE);
        }
    }
}

/// A mound with a hole in it and timber round the mouth.
fn burrow(d: &mut DrawList, p: &Pose, s: &Skin) {
    let r = p.r;
    d.shape(
        Shape::Sphere,
        p.p3(0.0, 0.0, 0.0),
        [r * 2.30, r * 2.10, r * 1.40],
        p.yaw,
        0.0,
        rgba([0.22, 0.17, 0.11], 1.0),
        Material::EARTH,
        0.0,
    );
    d.shape(
        Shape::Quad,
        p.p3(r * 1.55, 0.0, r * 0.45),
        [r * 1.10, r * 1.10, 1.0],
        p.yaw + std::f32::consts::FRAC_PI_2,
        0.0,
        rgba([0.03, 0.03, 0.04], 1.0),
        Material::EARTH,
        0.0,
    );
    for side in [-1.0f32, 1.0] {
        d.link(
            Shape::Cylinder,
            p.p3(r * 1.55, side * r * 0.65, 0.0),
            p.p3(r * 1.45, side * r * 0.70, r * 1.15),
            r * 0.14,
            s.wood,
            WOOD,
            0.0,
        );
    }
    d.link(
        Shape::Cylinder,
        p.p3(r * 1.45, -r * 0.70, r * 1.15),
        p.p3(r * 1.45, r * 0.70, r * 1.15),
        r * 0.12,
        s.wood,
        WOOD,
        0.0,
    );
    for k in 0..5 {
        let a = k as f32 * 1.257 + p.yaw;
        let q = p.at(a.cos() * r * 0.85, a.sin() * r * 0.85);
        d.link(
            Shape::Cone,
            [q[0], q[1], p.z + r * 1.00],
            [q[0], q[1], p.z + r * 1.85],
            r * 0.10,
            s.wood,
            WOOD,
            0.0,
        );
    }
    if p.fine() {
        d.link(
            Shape::Cylinder,
            p.p3(-r * 0.60, r * 0.90, r * 0.60),
            p.p3(-r * 0.60, r * 0.90, r * 2.60),
            r * 0.06,
            s.wood,
            WOOD,
            0.0,
        );
        d.shape(
            Shape::Quad,
            p.p3(-r * 0.60, r * 0.70, r * 2.20),
            [r * 0.55, r * 0.65, 1.0],
            p.yaw,
            0.0,
            s.body,
            CLOTH,
            0.0,
        );
    }
}

/// Something reaching up out of the ground.
fn tentacle(d: &mut DrawList, p: &Pose, s: &Skin) {
    let r = p.r;
    d.shape(
        Shape::Sphere,
        p.p3(0.0, 0.0, 0.0),
        [r * 2.00, r * 1.90, r * 0.75],
        p.yaw,
        0.0,
        s.dark,
        Material::EARTH,
        0.0,
    );
    for k in 0..5 {
        let a = k as f32 * 1.257 + p.yaw + p.t * 0.15;
        let wob = (p.t * 1.6 + k as f32).sin();
        let base = p.at(a.cos() * r * 0.60, a.sin() * r * 0.60);
        let mid = p.at(a.cos() * r * 1.10 + wob * r * 0.2, a.sin() * r * 1.10);
        let tip = p.at(a.cos() * r * 0.70 + wob * r * 0.5, a.sin() * r * 0.70);
        d.link(
            Shape::Capsule,
            [base[0], base[1], p.z + r * 0.20],
            [mid[0], mid[1], p.z + r * 1.30],
            r * 0.24,
            s.skin,
            FLESH,
            0.0,
        );
        d.link(
            Shape::Cone,
            [mid[0], mid[1], p.z + r * 1.30],
            [tip[0], tip[1], p.z + r * 2.40],
            r * 0.18,
            s.skin,
            FLESH,
            0.0,
        );
        if p.fine() {
            for j in 0..3 {
                let f = 0.3 + j as f32 * 0.25;
                d.sphere(
                    [
                        base[0] + (mid[0] - base[0]) * f,
                        base[1] + (mid[1] - base[1]) * f,
                        p.z + r * (0.20 + 1.10 * f),
                    ],
                    r * 0.12,
                    s.body,
                    FLESH,
                );
            }
        }
    }
    d.sphere_lit(p.p3(0.0, 0.0, r * 0.70), r * 0.50, s.glow, 0.4);
}

// ---------------------------------------------------------------- props

/// A mote of light with a tail.
fn wisp(d: &mut DrawList, p: &Pose, s: &Skin) {
    let r = p.r;
    let float = (p.t * 1.6).sin() * r * 0.22 + r * 1.10;
    d.sphere_lit(p.p3(0.0, 0.0, float), r * 0.80, s.glow, 0.7);
    p.glow(d, p.p3(0.0, 0.0, float), r * 2.6, 0.7, s.glow);
    for k in 0..6 {
        let a = p.t * 2.0 + k as f32 * 1.047;
        let q = p.at(a.cos() * r * 0.85, a.sin() * r * 0.85);
        d.sphere_lit(
            [q[0], q[1], p.z + float + (a * 1.5).sin() * r * 0.30],
            r * 0.16,
            s.glow,
            0.5,
        );
    }
}

/// A cairn of skulls.
fn skull_pile(d: &mut DrawList, p: &Pose, s: &Skin) {
    let r = p.r;
    for row in 0..3 {
        let count = 6 - row * 2;
        let rad = r * (1.05 - row as f32 * 0.30);
        for k in 0..count {
            let a = k as f32 / count as f32 * std::f32::consts::TAU + row as f32 * 0.5 + p.yaw;
            let q = p.at(a.cos() * rad, a.sin() * rad);
            let z = p.z + r * (0.35 + row as f32 * 0.52);
            d.sphere([q[0], q[1], z], r * 0.52, s.bone, STONE);
            if p.fine() {
                d.shape(
                    Shape::Prism,
                    [q[0], q[1], z - r * 0.20],
                    [r * 0.32, r * 0.14, 1.0],
                    a,
                    0.0,
                    s.bone,
                    STONE,
                    0.0,
                );
            }
            let f = [q[0] + a.cos() * r * 0.20, q[1] + a.sin() * r * 0.20];
            d.sphere_lit([f[0], f[1], z + r * 0.05], r * 0.12, s.glow, 0.45);
        }
    }
    p.glow(d, p.p3(0.0, 0.0, r * 0.8), r * 1.8, 0.3, s.glow);
}

/// A brazier of blue fire on a twisted iron stand.
fn ice_torch(d: &mut DrawList, p: &Pose, s: &Skin) {
    let r = p.r;
    let cold = rgba([0.50, 0.80, 1.0], 1.0);
    d.cylinder(p.p3(0.0, 0.0, 0.0), r * 1.30, r * 0.28, 0.0, s.iron, STONE);
    for k in 0..3 {
        let a = k as f32 * 2.094 + p.yaw;
        d.link(
            Shape::Capsule,
            p.p3(a.cos() * r * 0.45, a.sin() * r * 0.45, r * 0.20),
            p.p3(0.0, 0.0, r * 2.10),
            r * 0.09,
            s.iron,
            IRON,
            0.0,
        );
    }
    d.cylinder(
        p.p3(0.0, 0.0, r * 2.05),
        r * 0.95,
        r * 0.35,
        0.0,
        s.iron,
        IRON,
    );
    let flick = (p.t * 4.0).sin() * 0.5 + 0.5;
    d.shape(
        Shape::Cone,
        p.p3(0.0, 0.0, r * 2.35),
        [r * 0.70, r * 0.70, r * (1.0 + flick * 0.6)],
        p.t * 1.5,
        0.0,
        cold,
        Material::GEM,
        0.0,
    );
    p.glow(d, p.p3(0.0, 0.0, r * 2.8), r * 2.4, 0.8, cold);
}

/// A clutch of eggs, and something moving inside them.
fn egg_sack(d: &mut DrawList, p: &Pose, s: &Skin) {
    let r = p.r;
    d.shape(
        Shape::Sphere,
        p.p3(0.0, 0.0, 0.0),
        [r * 2.10, r * 2.00, r * 0.60],
        p.yaw,
        0.0,
        s.dark,
        Material::EARTH,
        0.0,
    );
    for k in 0..7 {
        let a = k as f32 * 0.897 + p.yaw;
        let rad = if k % 2 == 0 { r * 0.85 } else { r * 0.35 };
        let q = p.at(a.cos() * rad, a.sin() * rad);
        let pulse = (p.t * 2.0 + k as f32).sin() * 0.06 + 1.0;
        d.shape(
            Shape::Sphere,
            [q[0], q[1], p.z + r * 0.70 * pulse],
            [r * 0.62 * pulse, r * 0.62 * pulse, r * 0.90 * pulse],
            a,
            0.0,
            s.skin,
            FLESH,
            0.0,
        );
        if p.fine() {
            d.sphere_lit([q[0], q[1], p.z + r * 0.95 * pulse], r * 0.16, s.glow, 0.4);
        }
    }
    p.glow(d, p.p3(0.0, 0.0, r * 0.7), r * 1.8, 0.35, s.glow);
}

/// Three spheres, a carrot and two coals.
fn snowman(d: &mut DrawList, p: &Pose, s: &Skin) {
    let r = p.r;
    let snow = mix4(rgba([0.88, 0.93, 1.0], 1.0), s.body, 0.12);
    for (rad, z) in [(1.25f32, 1.15f32), (0.95, 2.55), (0.70, 3.55)] {
        d.sphere(p.p3(0.0, 0.0, r * z), r * rad * 2.0, snow, STONE);
    }
    let head = p.at(r * 0.30, 0.0);
    eyes(
        d,
        p,
        head,
        p.z + r * 3.70,
        r * 0.20,
        r * 0.20,
        rgba([0.08, 0.08, 0.10], 1.0),
    );
    d.shape(
        Shape::Cone,
        p.p3(r * 0.55, 0.0, r * 3.50),
        [r * 0.26, r * 0.26, r * 0.60],
        p.yaw,
        std::f32::consts::FRAC_PI_2,
        rgba([0.90, 0.48, 0.14], 1.0),
        STONE,
        0.0,
    );
    for side in [-1.0f32, 1.0] {
        let hand = p.p3(r * 0.20, side * r * 2.10, r * 3.30);
        d.link(
            Shape::Capsule,
            p.p3(0.0, side * r * 0.80, r * 2.70),
            hand,
            r * 0.08,
            rgba([0.28, 0.20, 0.13], 1.0),
            WOOD,
            0.0,
        );
        if p.fine() {
            for k in 0..2 {
                let f = (k as f32 - 0.5) * 0.7;
                d.link(
                    Shape::Cone,
                    hand,
                    [
                        hand[0] + f * r * 0.4,
                        hand[1] + side * r * 0.4,
                        hand[2] + r * 0.4,
                    ],
                    r * 0.04,
                    rgba([0.28, 0.20, 0.13], 1.0),
                    WOOD,
                    0.0,
                );
            }
        }
    }
    if p.fine() {
        for k in 0..3 {
            d.sphere(
                p.p3(r * 0.85, 0.0, r * (2.10 + k as f32 * 0.42)),
                r * 0.14,
                rgba([0.08, 0.08, 0.10], 1.0),
                STONE,
            );
        }
    }
    p.glow(
        d,
        p.p3(0.0, 0.0, r * 2.0),
        r * 2.6,
        0.4,
        rgba([0.55, 0.82, 1.0], 1.0),
    );
}

/// The three pure auras: a ring on the ground with something turning over it.
fn aura_ring(d: &mut DrawList, p: &Pose, s: &Skin, kind: u8) {
    let r = p.r;
    d.ground_ring(p.pos, r * 2.4, r * 0.22, s.glow, 48);
    d.ground_ring(p.pos, r * 1.5, r * 0.12, s.body, 36);
    match kind {
        0 => {
            for k in 0..8 {
                let a = k as f32 * 0.785 + p.t * 0.4;
                let q = p.at(a.cos() * r * 1.9, a.sin() * r * 1.9);
                d.link(
                    Shape::Cone,
                    [q[0], q[1], p.z],
                    [q[0], q[1], p.z + r * (0.9 + (a * 3.0).sin() * 0.25)],
                    r * 0.16,
                    s.body,
                    Material::FOLIAGE,
                    0.0,
                );
            }
        }
        1 => {
            d.link(
                Shape::Cylinder,
                p.p3(0.0, 0.0, 0.0),
                p.p3(0.0, 0.0, r * 2.60),
                r * 0.11,
                s.wood,
                WOOD,
                0.0,
            );
            d.shape(
                Shape::Quad,
                p.p3(0.0, r * 0.55, r * 1.90),
                [r * 1.10, r * 1.30, 1.0],
                p.yaw,
                0.0,
                s.body,
                CLOTH,
                0.2,
            );
            for k in 0..3 {
                let a = k as f32 * 2.094 + p.yaw;
                let q = p.at(a.cos() * r * 1.15, a.sin() * r * 1.15);
                d.cylinder([q[0], q[1], p.z], r * 0.60, r * 0.50, 0.0, s.wood, WOOD);
                if p.fine() {
                    d.cylinder(
                        [q[0], q[1], p.z + r * 0.52],
                        r * 0.62,
                        r * 0.05,
                        0.0,
                        s.leather,
                        FLESH,
                    );
                }
            }
        }
        _ => {
            let float = (p.t * 1.2).sin() * r * 0.2 + r * 1.6;
            for k in 0..2 {
                d.shape(
                    Shape::Cylinder,
                    p.p3(0.0, 0.0, float),
                    [
                        r * (1.6 - k as f32 * 0.5),
                        r * (1.6 - k as f32 * 0.5),
                        r * 0.08,
                    ],
                    p.t * (0.7 + k as f32 * 0.5),
                    0.4 + k as f32 * 0.6,
                    s.glow,
                    Material::GEM,
                    0.25,
                );
            }
            d.sphere_lit(p.p3(0.0, 0.0, float), r * 0.50, s.glow, 0.6);
        }
    }
    p.glow(d, p.p3(0.0, 0.0, r * 0.6), r * 3.0, 0.5, s.glow);
}

/// The four Super towers: a hole in the world with claws around it.
fn dark_portal(d: &mut DrawList, p: &Pose, s: &Skin) {
    let r = p.r;
    d.cylinder(p.p3(0.0, 0.0, 0.0), r * 2.60, r * 0.30, 0.0, s.iron, STONE);
    for k in 0..6 {
        let a = k as f32 * 1.047 + p.yaw;
        let base = p.at(a.cos() * r * 1.30, a.sin() * r * 1.30);
        let mid = p.at(a.cos() * r * 1.05, a.sin() * r * 1.05);
        let tip = p.at(a.cos() * r * 0.70, a.sin() * r * 0.70);
        d.link(
            Shape::Cone,
            [base[0], base[1], p.z + r * 0.20],
            [mid[0], mid[1], p.z + r * 1.90],
            r * 0.26,
            s.iron,
            STONE,
            0.0,
        );
        d.link(
            Shape::Cone,
            [mid[0], mid[1], p.z + r * 1.90],
            [tip[0], tip[1], p.z + r * 3.10],
            r * 0.16,
            s.iron,
            STONE,
            0.0,
        );
    }
    let float = r * 1.75;
    for k in 0..3 {
        d.shape(
            Shape::Cylinder,
            p.p3(0.0, 0.0, float),
            [
                r * (1.9 - k as f32 * 0.45),
                r * (1.9 - k as f32 * 0.45),
                r * 0.10,
            ],
            p.t * (1.0 + k as f32 * 0.6),
            0.5 + k as f32 * 0.35,
            s.glow,
            Material::GEM,
            0.30,
        );
    }
    d.sphere_lit(p.p3(0.0, 0.0, float), r * 1.05, s.glow, 0.75);
    p.glow(d, p.p3(0.0, 0.0, float), r * 3.6, 1.0, s.glow);
}

// ---------------------------------------------------------------- colour

fn mix4(a: Color, b: Color, k: f32) -> Color {
    [
        a[0] + (b[0] - a[0]) * k,
        a[1] + (b[1] - a[1]) * k,
        a[2] + (b[2] - a[2]) * k,
        a[3],
    ]
}
