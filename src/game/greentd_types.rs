//! The types the generated Green Circle TD tables are written in, and the
//! Warcraft III rules they depend on.
//!
//! This is the part that had to be understood rather than copied. The map's
//! numbers are meaningless without three things underneath them: armour
//! *values* that climb into the hundreds, an attack table the map rewrites
//! wholesale in `war3mapMisc.txt`, and an upgrade *graph* rather than a set of
//! straight ladders.

// ---------------------------------------------------------------- families

/// A tower family. Most are a straight ladder; three of them branch, and the
/// branches are families of their own so nothing is ambiguous.
#[derive(Clone, Copy, PartialEq, Eq, Debug, PartialOrd, Ord, Hash)]
pub enum Family {
    Single,
    Siege,
    Bouncing,
    Multi,
    Corruption,
    Air,
    Chaos,
    Destruction,
    Aura,
    Damage,
    Speed,
    Demon,
    King,
    SuperChaos,
    SuperDestruct,
    SuperMulti,
    SuperBounce,
    Slow,
    Frost,
    Poison,
    Critical,
    Troll,
    Fire,
    OneStrike,
}

impl Family {
    pub fn name(self) -> &'static str {
        use Family::*;
        match self {
            Single => "Single shot",
            Siege => "Siege",
            Bouncing => "Bouncing",
            Multi => "Multi",
            Corruption => "Corruption",
            Air => "Air",
            Chaos => "Chaos",
            Destruction => "Destruction",
            Aura => "Aura",
            Damage => "Damage Aura",
            Speed => "Speed Aura",
            Demon => "Demon",
            King => "King",
            SuperChaos => "Super Chaos",
            SuperDestruct => "Super Destruction",
            SuperMulti => "Super Multi",
            SuperBounce => "Super Bouncing",
            Slow => "Slow",
            Frost => "Frost",
            Poison => "Poison",
            Critical => "Critical",
            Troll => "Troll",
            Fire => "Fire",
            OneStrike => "One-Strike Kill",
        }
    }

    /// A name that fits on a command card.
    ///
    /// The full names run to "Super Destruction", and eleven cards across six
    /// hundred points leaves about seven characters each - so the shop was
    /// showing "Destru..", "Bounci.." and "Single ..". These are the same names
    /// with nothing that has to be cut off.
    pub fn short(self) -> &'static str {
        use Family::*;
        match self {
            Single => "Seed",
            Siege => "Siege",
            Bouncing => "Bounce",
            Multi => "Multi",
            Corruption => "Corrupt",
            Air => "Air",
            Chaos => "Chaos",
            Destruction => "Destroy",
            Aura => "Aura",
            Damage => "Damage",
            Speed => "Speed",
            Demon => "Demon",
            King => "King",
            SuperChaos => "S.Chaos",
            SuperDestruct => "S.Destr",
            SuperMulti => "S.Multi",
            SuperBounce => "S.Bounce",
            Slow => "Slow",
            Frost => "Frost",
            Poison => "Poison",
            Critical => "Crit",
            Troll => "Troll",
            Fire => "Fire",
            OneStrike => "1-Shot",
        }
    }

    /// One line on what the family is for.
    pub fn role(self) -> &'static str {
        use Family::*;
        match self {
            Single => "Ten gold. Becomes one of six towers you cannot buy.",
            Siege => "Splash damage, and the longest ladder in the game.",
            Bouncing => "Every shot leaps on to the next target.",
            Multi => "Fires on several targets at once.",
            Corruption => "Strips armour off whatever it hits.",
            Air => "The only tower built for what flies.",
            Chaos => "Chaos damage. Nothing resists it, not even Immune.",
            Destruction => "Chaos splash. Wide, and unresisted.",
            Aura => "Fires nothing. Branches into Damage or Speed.",
            Damage => "Every tower near it hits harder.",
            Speed => "Every tower near it fires faster.",
            Demon => "Spell damage, and a chance to kill outright.",
            King => "Sees furthest, and opens the four Super towers.",
            SuperChaos => "The end of the King's road. Chaos, at range.",
            SuperDestruct => "The end of the King's road. Chaos, everywhere.",
            SuperMulti => "The end of the King's road. Ten targets at once.",
            SuperBounce => "The end of the King's road. The shot never stops.",
            Slow => "Fires nothing. Everything near it crawls.",
            Frost => "What the Slow Tower becomes. It breathes.",
            Critical => "Long range, and the crits get absurd.",
            Poison => "Cheap, fast, and it ladders further than anything.",
            Troll => "Chaos at short range, it roots, and it works itself up.",
            Fire => "Magic damage, and it makes its neighbours hit harder.",
            OneStrike => "Hero damage: a hundred times the number on the card.",
        }
    }
}

// ---------------------------------------------------------------- damage

/// Warcraft III attack types.
///
/// All seven, not only the five this map's roster happens to use: the table
/// below is the engine's, and a tower added later that deals Pierce should not
/// need this enum edited.
#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Attack {
    Normal,
    Pierce,
    Siege,
    Magic,
    Chaos,
    Spells,
    Hero,
}

/// Warcraft III armour types.
///
/// The campaign only ever sends Unarmoured, Medium, Hero and Immune, but the
/// other three are the engine's and the extractor maps onto them, so a map
/// change that starts using Light armour is data rather than code.
#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ArmourType {
    Unarmoured,
    Light,
    Medium,
    Heavy,
    Fortified,
    Hero,
    /// The map calls these waves **Immune**, and they arrive every fifth wave.
    /// They take five percent from everything except Chaos and Hero damage.
    Divine,
}

impl Attack {
    pub fn name(self) -> &'static str {
        match self {
            Attack::Normal => "Normal",
            Attack::Pierce => "Pierce",
            Attack::Siege => "Siege",
            Attack::Magic => "Magic",
            Attack::Chaos => "Chaos",
            Attack::Spells => "Spells",
            Attack::Hero => "Hero",
        }
    }
    /// What a player needs to know about this attack type in one line.
    pub fn note(self) -> &'static str {
        match self {
            Attack::Chaos => "Immune waves take it in full.",
            Attack::Hero => "A hundred times damage, Immune included.",
            _ => "Immune waves take five percent.",
        }
    }
    pub fn color(self) -> [f32; 3] {
        match self {
            Attack::Normal => [0.85, 0.80, 0.62],
            Attack::Pierce => [0.72, 0.84, 0.55],
            Attack::Siege => [0.92, 0.62, 0.30],
            Attack::Magic => [0.46, 0.70, 1.00],
            Attack::Chaos => [1.00, 0.36, 0.32],
            Attack::Spells => [0.78, 0.50, 1.00],
            Attack::Hero => [1.00, 0.88, 0.45],
        }
    }
}

impl ArmourType {
    pub fn name(self) -> &'static str {
        match self {
            ArmourType::Unarmoured => "Unarmoured",
            ArmourType::Light => "Light",
            ArmourType::Medium => "Medium",
            ArmourType::Heavy => "Heavy",
            ArmourType::Fortified => "Fortified",
            ArmourType::Hero => "Hero",
            ArmourType::Divine => "Immune",
        }
    }
    pub fn color(self) -> [f32; 3] {
        match self {
            ArmourType::Divine => [1.00, 0.85, 0.35],
            ArmourType::Hero => [0.72, 0.52, 0.95],
            ArmourType::Medium => [0.62, 0.66, 0.76],
            ArmourType::Fortified => [0.58, 0.92, 0.90],
            _ => [0.58, 0.72, 0.52],
        }
    }
    /// What a player has to be told about this armour, in one line.
    pub fn counter(self) -> &'static str {
        match self {
            ArmourType::Divine => "Takes 5% from everything but Chaos and Hero.",
            _ => "No attack type is resisted. Armour is the whole defence.",
        }
    }
}

impl Attack {
    /// Row in [`super::greentd::DAMAGE`].
    pub fn idx(self) -> usize {
        self as usize
    }
}

impl ArmourType {
    /// Column in [`super::greentd::DAMAGE`].
    pub fn idx(self) -> usize {
        self as usize
    }
}

/// The attack-versus-armour table, as this map rewrites it.
///
/// Warcraft III ships a seven-by-seven table of counters; `war3mapMisc.txt` in
/// this map throws all of it away:
///
/// ```text
/// DamageBonusNormal=1,1,1,1,1,1,0.05,1
/// DamageBonusPierce=1,1,1,1,1,1,0.05,1
/// DamageBonusSiege =1,1,1,1,1,1,0.05,1
/// DamageBonusMagic =1,1,1,1,1,1,0.05,1
/// DamageBonusSpells=1,1,1,1,1,1,0.05,1
/// DamageBonusHero  =100,100,100,100,100,100,100,100
/// ```
///
/// The seventh column is Divine. So every ordinary attack does full damage to
/// everything, and five percent to the Immune waves; Chaos - which Warcraft III
/// hard-codes at 1.0 and no file can change - does full damage to those too;
/// and Hero damage is multiplied by a hundred against anything at all, which is
/// the whole of the One-Strike Kill Tower.
///
/// That one table is why the roster looks the way it does. There is no rock,
/// paper, scissors here: there is armour, and there is the fifth wave.
///
/// The numbers are **generated** from that file by `tools/emit.py` rather than
/// transcribed. They were hand-written once, which meant a map version that
/// changed those six lines would have disagreed with the code and nothing would
/// have said so.
pub fn type_mult(a: Attack, d: ArmourType) -> f32 {
    super::greentd::DAMAGE[a.idx()][d.idx()]
}

/// How much of a hit an armour *value* absorbs.
///
/// Warcraft III's curve, unchanged: each point is worth 6% of a point, and they
/// stack with diminishing returns rather than linearly, so armour never reaches
/// total immunity. It gets close, though - the last wave carries 200 armour and
/// takes 8% of what it is hit with, and wave 33 carries 700, which is 2%. That
/// is why the tower ladders climb to six figures of damage.
pub fn armour_mult(armour: i32) -> f32 {
    let a = armour as f32 * 0.06;
    if armour >= 0 {
        1.0 / (1.0 + a)
    } else {
        // Negative armour amplifies, and is capped the way the engine caps it.
        2.0 - 0.94f32.powi(-armour)
    }
}

/// Everything applied together: the type table, then the armour value.
pub fn damage_taken(base: f32, attack: Attack, armour: i32, kind: ArmourType) -> f32 {
    base * type_mult(attack, kind) * armour_mult(armour)
}

// ---------------------------------------------------------------- models

/// Which Warcraft III unit a thing is wearing.
///
/// The map dresses all hundred and thirty-one towers and thirty-six creeps in
/// stock Warcraft III models. Nothing here can load a `.mdl`, so each one is
/// rebuilt out of primitives in `view/towers.rs` and `view/monsters.rs`; this
/// is the list of what has to be built. `tools/emit.py` maps the map's model
/// paths onto it.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Model {
    // people
    Acolyte,
    Archer,
    Mage,
    Warrior,
    Demon,
    Brute,
    Troll,
    Gnoll,
    Skeleton,
    Wraith,
    Naga,
    Rifleman,
    Villager,
    Panda,
    // beasts
    Bear,
    Mammoth,
    Centaur,
    Lizard,
    Crab,
    Spider,
    Serpent,
    Turtle,
    Ent,
    Golem,
    Giant,
    Infernal,
    FlameLord,
    // wings
    Gyrocopter,
    Phoenix,
    Harpy,
    Dragon,
    FrostWyrm,
    // machines
    Turret,
    Turbolazer,
    RebelTurret,
    Vulcan,
    SamSite,
    Cannon,
    MeatWagon,
    Ship,
    // buildings
    Obelisk,
    MagicTower,
    Observatory,
    DemonGate,
    Altar,
    Burrow,
    Tentacle,
    // props and pure effects
    Wisp,
    SkullPile,
    IceTorch,
    EggSack,
    Snowman,
    ThornsAura,
    CommandAura,
    ControlMagic,
    DarkPortal,
}

impl Model {
    /// Whether the model stands on the ground or hangs above it. Used to check
    /// that a wave the map flies is drawn as something with wings.
    #[allow(dead_code)]
    pub fn airborne(self) -> bool {
        matches!(
            self,
            Model::Gyrocopter
                | Model::Phoenix
                | Model::Harpy
                | Model::Dragon
                | Model::FrostWyrm
                | Model::Wisp
                | Model::ThornsAura
                | Model::CommandAura
                | Model::ControlMagic
        )
    }

    /// How wide it is, in tiles, before the unit's own scale is applied.
    pub fn radius(self) -> f32 {
        use Model::*;
        match self {
            Villager | Wisp => 0.20,
            Acolyte | Archer | Skeleton | Gnoll => 0.26,
            Mage | Warrior | Rifleman | Wraith | Naga | Troll | Harpy => 0.30,
            Panda | Bear | Spider | Crab | Turtle | Phoenix | Gyrocopter => 0.34,
            Demon | Brute | Golem | Infernal | FlameLord | Centaur | Lizard => 0.40,
            Serpent | Ent | MeatWagon | Ship | Dragon | FrostWyrm => 0.46,
            Giant | Mammoth => 0.52,
            _ => 0.36,
        }
    }
}

// ---------------------------------------------------------------- data rows

/// What a tower is allowed to shoot at.
///
/// Nine of the ten Air Towers can hit *nothing but* what flies, which is the
/// map's way of making anti-air a deliberate purchase; Siege, Bouncing, Chaos
/// and Destruction are stuck on the ground. Everything else covers both.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Targets {
    Both,
    GroundOnly,
    AirOnly,
    /// Auras and slows: they never attack.
    Nothing,
}

impl Targets {
    pub fn can_hit(self, flying: bool) -> bool {
        match self {
            Targets::Both => true,
            Targets::GroundOnly => !flying,
            Targets::AirOnly => flying,
            Targets::Nothing => false,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Targets::Both => "Ground + Air",
            Targets::GroundOnly => "Ground only",
            Targets::AirOnly => "Air only",
            Targets::Nothing => "Does not attack",
        }
    }
}

/// Everything a tower level does beyond hitting one thing for one number.
///
/// Every field is the map's own: crit chances out of `AOcr`, poison out of
/// `ACvs`, auras out of `AEar`, `AOae` and `ACac`, roots out of `AIbx`, the
/// outright kill out of `ACbh`, armour stripping out of `AIcb`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Abil {
    /// Chance in 0..1, and what the hit is multiplied by.
    pub crit_chance: f32,
    pub crit_mult: f32,
    /// How many separate targets one attack hits.
    pub multishot: u32,
    /// How many times a shot leaps on after its first target.
    pub bounce: u32,
    /// Damage per second, the slow it applies, and how long both last.
    pub poison_dps: f32,
    pub poison_slow: f32,
    pub poison_dur: f32,
    /// A standing slow on everything inside `slow_range`.
    pub slow_amt: f32,
    pub slow_range: f32,
    /// Bonuses to every tower inside `aura_range`.
    pub dmg_aura: f32,
    pub speed_aura: f32,
    pub aura_range: f32,
    /// Damage per second to everything inside `burn_range`, with no shot fired.
    pub burn_dps: f32,
    pub burn_range: f32,
    /// Chance of holding the target still, and for how long.
    pub root_chance: f32,
    pub root_dur: f32,
    /// Chance an attack simply kills whatever it hits.
    pub kill_chance: f32,
    /// Armour taken off the target for this hit.
    pub armour_pen: i32,
    /// Attack rate it gives *itself*, how long for, and how often.
    pub frenzy: f32,
    pub frenzy_dur: f32,
    pub frenzy_cd: f32,
}

impl Abil {
    pub fn is_aura(&self) -> bool {
        self.dmg_aura > 0.0 || self.speed_aura > 0.0
    }
    /// Roughly how much one attack is worth relative to a plain hit, used to
    /// rank towers in the shop rather than to resolve any actual damage.
    pub fn hit_multiplier(&self) -> f32 {
        let crit = 1.0 + self.crit_chance * (self.crit_mult - 1.0).max(0.0);
        let spread = self.multishot.max(1) as f32 + self.bounce as f32 * 0.6;
        crit * spread
    }
}

/// One tower, at one point on its upgrade path.
pub struct TowerLevel {
    pub family: Family,
    /// How far up its family it stands, from zero.
    pub step: u32,
    /// The map's own name, heroes and all.
    pub name: &'static str,
    /// What it costs to build or upgrade into.
    pub gold: u32,
    /// What selling it pays back - the map's own point value, which is usually
    /// everything sunk into it and is deliberately less at the very top.
    pub refund: u32,
    pub damage: f32,
    /// Seconds between attacks.
    pub cooldown: f32,
    pub range: f32,
    /// Splash radius in tiles; zero for single target.
    pub splash: f32,
    pub attack: Attack,
    pub targets: Targets,
    pub model: Model,
    /// The map's own model scale. Creeps use it; towers are sized by how far up
    /// their family they stand instead, because a tower has to fit its pad.
    #[allow(dead_code)]
    pub scale: f32,
    /// Whether it can be bought outright, rather than only upgraded into.
    pub shop: bool,
    /// What it can become, as indices into `LEVELS`. Usually one; the Single
    /// shot Tower has six, the Aura Tower three and the King Tower five.
    pub upgrades: &'static [u16],
    pub abil: Abil,
}

impl TowerLevel {
    /// Damage per second against something with no armour at all.
    pub fn dps(&self) -> f32 {
        if self.cooldown <= 0.0 {
            0.0
        } else {
            self.damage * self.abil.hit_multiplier() / self.cooldown
        }
    }

    /// What one shot lands across everything it touches, so a splash tower is
    /// not judged on its single-target number alone.
    pub fn effective_dps(&self) -> f32 {
        let spread = if self.splash > 0.0 {
            1.0 + self.splash * 0.8
        } else {
            1.0
        };
        self.dps() * spread + self.abil.poison_dps + self.abil.burn_dps
    }

    pub fn attacks(&self) -> bool {
        self.targets != Targets::Nothing && self.damage > 0.0
    }

    /// The colour it is drawn and listed in - its attack type's, so the board
    /// says at a glance which of your towers can hurt what is coming.
    pub fn color(&self) -> [f32; 3] {
        if self.attacks() {
            self.attack.color()
        } else if self.abil.is_aura() {
            [0.95, 0.86, 0.45]
        } else {
            [0.55, 0.80, 0.95]
        }
    }
}

/// One of the thirty-six waves.
pub struct WaveRow {
    /// The map's own wave number. Read by the extraction tests rather than by
    /// the game, which indexes the table directly.
    #[allow(dead_code)]
    pub wave: u32,
    /// The map's own name for the creep: Troll, Salamander, Bronze Dragon.
    pub name: &'static str,
    pub count: u32,
    pub hp: f32,
    pub armour: i32,
    pub armour_type: ArmourType,
    /// Warcraft III movement speed; 400 is a footman.
    pub speed: f32,
    pub flying: bool,
    pub model: Model,
    pub scale: f32,
    /// Seconds the map waits before starting this wave.
    pub gap: f32,
    /// The map's own banner for the wave: "Air", "Immune", "Hero", "Boss".
    pub tag: &'static str,
}
