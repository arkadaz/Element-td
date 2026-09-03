//! The types the generated Green Circle TD tables are written in, and the
//! Warcraft III damage rules they depend on.
//!
//! This is the part that had to be understood rather than copied. The map's
//! numbers are meaningless without the two mechanics underneath them - armour
//! *values* that scale into the hundreds, and an attack-type table where one
//! row is unlike all the others - and together those two are the whole counter
//! system the game is built on.

// ---------------------------------------------------------------- families

/// A tower family. Each is a linear ladder of levels; a tower is a family plus
/// how far up it you have paid.
#[derive(Clone, Copy, PartialEq, Eq, Debug, PartialOrd, Ord)]
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
    Demon,
    King,
    Slow,
    Poison,
    Critical,
    Troll,
    Fire,
    OneStrike,
}

impl Family {
    pub fn name(self) -> &'static str {
        match self {
            Family::Single => "Single shot",
            Family::Siege => "Siege",
            Family::Bouncing => "Bouncing",
            Family::Multi => "Multi",
            Family::Corruption => "Corruption",
            Family::Air => "Air",
            Family::Chaos => "Chaos",
            Family::Destruction => "Destruction",
            Family::Aura => "Aura",
            Family::Demon => "Demon",
            Family::King => "King",
            Family::Slow => "Slow",
            Family::Poison => "Poison",
            Family::Critical => "Critical",
            Family::Troll => "Troll",
            Family::Fire => "Fire",
            Family::OneStrike => "One-Strike Kill",
        }
    }

    /// One line on what the family is for.
    pub fn role(self) -> &'static str {
        match self {
            Family::Single => "The 10 gold seed. Specialises into six others.",
            Family::Siege => "Splash damage, and the longest ladder in the game.",
            Family::Bouncing => "Every shot leaps between targets.",
            Family::Multi => "Fires on several targets at once.",
            Family::Corruption => "Enormous damage, slow, no splash.",
            Family::Air => "The only tower that answers what flies.",
            Family::Chaos => "Chaos damage: nothing resists it.",
            Family::Destruction => "Chaos splash. Wide and unresisted.",
            Family::Aura => "Fires nothing. Makes its neighbours better.",
            Family::Demon => "Spell damage, and a chance to kill outright.",
            Family::King => "Sees further than anything else, and hits like a siege engine.",
            Family::Slow => "Fires nothing. Everything nearby crawls.",
            Family::Poison => "Fast, cheap, and it ladders further than anything.",
            Family::Critical => "Long range, and the crits get absurd.",
            Family::Troll => "Chaos damage at short range, and it roots.",
            Family::Fire => "Slow, magic, and it buffs the towers around it.",
            Family::OneStrike => "Almost never fires. Deletes what it hits.",
        }
    }

    /// Whether this family can be bought from the shop, or is only reached by
    /// specialising a Single shot Tower.
    pub fn buildable(self) -> bool {
        !matches!(
            self,
            Family::Slow
                | Family::Poison
                | Family::Critical
                | Family::Troll
                | Family::Fire
                | Family::OneStrike
        )
    }
}

/// What a Single shot Tower may become. This is the one branching choice in the
/// game, and it costs ten gold to get to it.
pub const SPECIALISATIONS: [Family; 6] = [
    Family::Poison,
    Family::Critical,
    Family::Troll,
    Family::Fire,
    Family::Slow,
    Family::OneStrike,
];

// ---------------------------------------------------------------- damage

/// Warcraft III attack types.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Attack {
    Normal,
    Siege,
    Magic,
    Chaos,
    Spells,
    Hero,
}

/// Warcraft III armour types.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ArmourType {
    Unarmoured,
    Light,
    Medium,
    Heavy,
    Fortified,
    Hero,
    /// Takes **five percent** from normal, siege and magic - and full damage
    /// from chaos and spells. It is the reason the Troll, Chaos, Destruction
    /// and Demon families exist, and it arrives every fifth wave.
    Divine,
}

impl Attack {
    pub fn name(self) -> &'static str {
        match self {
            Attack::Normal => "Normal",
            Attack::Siege => "Siege",
            Attack::Magic => "Magic",
            Attack::Chaos => "Chaos",
            Attack::Spells => "Spells",
            Attack::Hero => "Hero",
        }
    }
    pub fn color(self) -> [f32; 3] {
        match self {
            Attack::Normal => [0.85, 0.80, 0.62],
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
            ArmourType::Divine => "Divine",
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
            ArmourType::Divine => "Only Chaos and Spells hurt it. Everything else does 5%.",
            ArmourType::Hero => "Siege and Magic do half. Chaos is unaffected.",
            ArmourType::Medium => "Normal does 150%. Siege and Magic do less.",
            ArmourType::Heavy => "Magic does double.",
            ArmourType::Fortified => "Siege does 150%. Magic barely scratches it.",
            ArmourType::Light => "Magic does 125%.",
            ArmourType::Unarmoured => "Siege does 150%. Nothing is resisted.",
        }
    }
}

/// The Warcraft III attack-versus-armour table.
///
/// The `Chaos` row is the important one: it is flat 1.0 against everything,
/// including Divine. That single row is what makes the Troll, Chaos and
/// Destruction families worth their gold, and what makes a board of nothing but
/// Siege towers lose on wave five.
pub fn type_mult(a: Attack, d: ArmourType) -> f32 {
    use ArmourType as D;
    use Attack as A;
    match (a, d) {
        (A::Chaos, _) => 1.0,
        (A::Spells, D::Hero) => 0.70,
        (A::Spells, _) => 1.0,
        (A::Hero, D::Fortified) => 0.50,
        (A::Hero, _) => 1.0,

        (_, D::Divine) => 0.05,

        (A::Normal, D::Medium) => 1.50,
        (A::Normal, D::Fortified) => 0.70,
        (A::Normal, _) => 1.00,

        (A::Siege, D::Unarmoured) => 1.50,
        (A::Siege, D::Medium) => 0.50,
        (A::Siege, D::Fortified) => 1.50,
        (A::Siege, D::Hero) => 0.50,
        (A::Siege, _) => 1.00,

        (A::Magic, D::Light) => 1.25,
        (A::Magic, D::Medium) => 0.75,
        (A::Magic, D::Heavy) => 2.00,
        (A::Magic, D::Fortified) => 0.35,
        (A::Magic, D::Hero) => 0.50,
        (A::Magic, _) => 1.00,
    }
}

/// How much of a hit an armour *value* absorbs.
///
/// Warcraft III's curve, unchanged: each point is worth 6% of a point, and they
/// stack with diminishing returns rather than linearly, so armour never reaches
/// total immunity. It gets close, though - the last wave carries 200 armour and
/// takes 8% of what it is hit with, and one creep in this map has 700, which is
/// 2%. That is why the tower ladder climbs to six figures of damage.
pub fn armour_mult(armour: i32) -> f32 {
    let a = armour as f32 * 0.06;
    if armour >= 0 {
        1.0 / (1.0 + a)
    } else {
        // Negative armour amplifies, and is capped the way the engine caps it.
        2.0 - 0.94f32.powi(-armour)
    }
}

/// Everything applied together: type table, then armour value.
pub fn damage_taken(base: f32, attack: Attack, armour: i32, kind: ArmourType) -> f32 {
    base * type_mult(attack, kind) * armour_mult(armour)
}

// ---------------------------------------------------------------- data rows

/// What a tower is allowed to shoot at.
///
/// Only the Air family can reach what flies, and its first level can hit
/// *nothing else* - which is the map's way of making anti-air a deliberate
/// purchase rather than a side effect. Siege, Chaos and Destruction are stuck
/// on the ground; everything else covers both.
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

/// Bit flags for the abilities a tower level carries.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Flag(pub u16);

impl Flag {
    pub const NONE: Flag = Flag(0);
    pub const CRIT: Flag = Flag(1 << 0);
    pub const MULTISHOT: Flag = Flag(1 << 1);
    pub const ROOTS: Flag = Flag(1 << 2);
    pub const SLOW: Flag = Flag(1 << 3);
    pub const INSTAKILL: Flag = Flag(1 << 4);
    pub const CORRUPTION: Flag = Flag(1 << 5);
    pub const DAMAGE_AURA: Flag = Flag(1 << 6);
    pub const SPEED_AURA: Flag = Flag(1 << 7);
    pub const BOUNCE: Flag = Flag(1 << 8);

    pub fn has(self, f: Flag) -> bool {
        self.0 & f.0 != 0
    }
}

impl std::ops::BitOr for Flag {
    type Output = Flag;
    fn bitor(self, rhs: Flag) -> Flag {
        Flag(self.0 | rhs.0)
    }
}

/// One rung of one family's ladder.
pub struct TowerLevel {
    pub family: Family,
    /// Position in its family's ladder, from zero.
    pub step: u32,
    /// The map's own name, heroes and all.
    pub name: &'static str,
    pub gold: u32,
    pub damage: f32,
    /// Seconds between attacks.
    pub cooldown: f32,
    pub range: f32,
    /// Splash radius in tiles; zero for single target.
    pub splash: f32,
    pub attack: Attack,
    pub targets: Targets,
    pub flags: Flag,
}

impl TowerLevel {
    /// Damage per second against something with no armour at all.
    pub fn dps(&self) -> f32 {
        if self.cooldown <= 0.0 {
            0.0
        } else {
            self.damage / self.cooldown
        }
    }

    /// Roughly what one shot lands across everything it touches, so a splash or
    /// multishot tower is not judged on its single-target number alone.
    pub fn effective_dps(&self) -> f32 {
        let spread = if self.flags.has(Flag::MULTISHOT) {
            3.0
        } else if self.splash > 0.0 {
            1.0 + self.splash * 0.8
        } else {
            1.0
        };
        self.dps() * spread
    }

    pub fn attacks(&self) -> bool {
        self.targets != Targets::Nothing && self.damage > 0.0
    }

    /// The colour it is drawn and listed in - its attack type's, so the board
    /// says at a glance which of your towers can hurt what is coming.
    pub fn color(&self) -> [f32; 3] {
        if self.attacks() {
            self.attack.color()
        } else {
            [0.80, 0.78, 0.62]
        }
    }
}

/// One of the thirty-six waves.
pub struct WaveRow {
    pub wave: u32,
    pub name: &'static str,
    pub count: u32,
    pub hp: f32,
    pub armour: i32,
    pub armour_type: ArmourType,
    /// Warcraft III movement speed; 400 is a footman.
    pub speed: f32,
    pub flying: bool,
}
