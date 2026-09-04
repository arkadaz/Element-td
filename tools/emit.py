"""Emits src/game/greentd.rs: the roster, the upgrade graph and the waves.

Everything written here comes out of `GREEN TD 9.3c PEIN.w3x`. Run
`extract.py` first to unpack the archive, then this.
"""

import json
import re

import w3obj

units, _ = w3obj.parse(open('greentd/war3map.w3u', 'rb').read(), False)
abils, _ = w3obj.parse(open('greentd/war3map.w3a', 'rb').read(), True)
src = open('greentd/script.j', encoding='latin-1').read()

# Warcraft III measures distance in world units, 128 to a tile. Ranges convert
# one to one, so a 900 range tower reaches seven tiles here exactly as there.
RANGE_DIV = 128.0

# ---------------------------------------------------------------- families
#
# The name prefix is how a unit is placed in a family: the map has no family
# field, only a naming convention it keeps rigidly.
FAMILIES = [
    # tag,            name prefix,                  role
    ('Single',        'Single shot Tower',          None),
    ('Siege',         'Siege Tower',                None),
    ('Bouncing',      'Bouncing Tower',             None),
    ('Multi',         'Multi Tower',                None),
    ('Corruption',    'Corruption Tower',           None),
    ('Air',           'Air Tower',                  None),
    ('Chaos',         'Chaos tower',                None),
    ('Destruction',   'Destruction Tower',          None),
    ('Aura',          'Aura Tower',                 None),
    ('Damage',        'Damage Tower',               None),
    ('Speed',         'Speed Tower',                None),
    ('Demon',         'Demon Tower',                None),
    ('King',          'King Tower',                 None),
    ('SuperChaos',    'Supper chaos tower',         None),
    ('SuperDestruct', 'Super destruction Tower',    None),
    ('SuperMulti',    'Super Multi Tower',          None),
    ('SuperBounce',   'Super Bouncing Tower',       None),
    ('Slow',          'Slow Tower',                 None),
    ('Frost',         'Frost Tower',                None),
    ('Poison',        'Poison Tower',               None),
    ('Critical',      'Critical Tower',             None),
    ('Troll',         'Troll Tower',                None),
    ('Fire',          'Fire Tower',                 None),
    ('OneStrike',     'One-Strike Kill Tower',      None),
]

ATTACK = {
    '': 'Normal', 'normal': 'Normal', 'siege': 'Siege', 'magic': 'Magic',
    'chaos': 'Chaos', 'spells': 'Spells', 'hero': 'Hero', 'unknown': 'Normal',
    'pierce': 'Pierce',
}

ARMOUR = {
    'none': 'Unarmoured', 'normal': 'Unarmoured', 'small': 'Light',
    'medium': 'Medium', 'large': 'Heavy', 'fort': 'Fortified',
    'hero': 'Hero', 'divine': 'Divine',
}

# ---------------------------------------------------------------- models
#
# The map dresses every unit in a stock Warcraft III model. Nothing here can
# load a .mdl, so each one is mapped to the archetype it reads as and rebuilt
# out of primitives in `view/`. The key is the model file's own stem, matched
# exactly, because substrings lie - "Ent" is inside "CentaurKhan".
MODELS = {
    # --- robed and hooded
    'acolyte': 'Acolyte',
    # --- bow and blade, slight build
    'archer': 'Archer', 'huntress': 'Archer', 'dryad': 'Archer',
    'skeletonarcher': 'Archer', 'assassin': 'Archer',
    'sylvanuswindrunner': 'Archer', 'bloodelfspellthief': 'Archer',
    'herowarden': 'Archer',
    # --- staff and orb
    'kael': 'Mage', 'sorceress_v1': 'Mage', 'proudmoore': 'Mage',
    'dranaimage': 'Mage', 'dranaiakama': 'Mage', 'shaman': 'Mage',
    'skeletonmage': 'Mage', 'eredarwarlock': 'Mage', 'chaoswarlock': 'Mage',
    'elvensorcerer': 'Mage', 'herokeeperofthegroveghost': 'Mage',
    # --- armoured, upright, carrying a blade
    'heroblademaster': 'Warrior', 'herochaosblademaster': 'Warrior',
    'herodemonhunter': 'Warrior', 'arthas': 'Warrior',
    'undeadarthas': 'Warrior', 'illidanevil': 'Warrior',
    'lordgarithos': 'Warrior', 'uther': 'Warrior',
    'hero_felorc_general': 'Warrior', 'chaosgrunt': 'Warrior',
    'hellscream': 'Warrior', 'groundwalker_heavy': 'Warrior',
    'chaoswarlord': 'Warrior', 'angel new 0': 'Warrior',
    # --- horned and winged
    'herodreadlord': 'Demon', 'doomguard': 'Demon', 'felgaurdblue': 'Demon',
    'kiljaeden': 'Demon', 'tichondrius': 'Demon', 'demonessblue': 'Demon',
    # --- heavy shoulders
    'tauren': 'Brute', 'abominationcin': 'Brute', 'heromountainking': 'Brute',
    'herotaurenchieftaincin': 'Brute', 'zombie': 'Brute',
    # --- hunched, tusked
    'foresttroll': 'Troll', 'icetroll': 'Troll', 'darktroll': 'Troll',
    'foresttrolltrapper': 'Troll', 'darktrolltrapper': 'Troll',
    'gnollwarden': 'Gnoll',
    # --- bones and spectres
    'skeleton': 'Skeleton',
    'revenant': 'Wraith', 'revenantofthewaves': 'Wraith',
    'voidwalker': 'Wraith', 'spiritofvengeance': 'Wraith',
    'facelessone': 'Wraith', 'cold wright': 'Wraith',
    'forgottenone': 'Wraith', 'cloudoffog': 'Wraith',
    # --- serpent-tailed
    'nagasiren': 'Naga', 'ladyvashj': 'Naga', 'murgulreaver': 'Naga',
    # --- the odd ones out
    'rifleman': 'Rifleman', 'villagerkid1': 'Villager',
    'pandarenbrewmaster': 'Panda',
    # --- beasts
    'grizzlybear': 'Bear', 'mammothblack': 'Mammoth',
    'centaurkhan': 'Centaur', 'thunderlizardvizier': 'Lizard',
    'lobstrokkblue': 'Crab', 'spiderv6': 'Spider', 'zergling': 'Spider',
    'heronerubianwidow': 'Spider', 'hydra': 'Serpent',
    'seaelemental': 'Serpent', 'giantseaturtle': 'Turtle', 'ent': 'Ent',
    'rockgolem': 'Golem', 'irongolem': 'Golem', 'fleshgolem': 'Golem',
    'golemstatue': 'Golem', 'mountaingiant': 'Giant',
    'infernal': 'Infernal', 'infernalcannonflame': 'Infernal',
    'lavaspawn': 'Infernal', 'heroflamelord': 'FlameLord',
    # --- things with wings
    'phoenix': 'Phoenix', 'harpyqueen': 'Harpy', 'bronzedragon': 'Dragon',
    'dragonbuilding': 'Dragon', 'firedragon_fireblade': 'Dragon',
    # --- machines
    'turret': 'Turret', 'turbolazer': 'Turbolazer',
    'rebelturret': 'RebelTurret', 'vulcan-b': 'Vulcan', 'samsite': 'SamSite',
    'cannon': 'Cannon', 'cannon_portrait': 'Cannon', 'meatwagon': 'MeatWagon',
    'humandestroyership': 'Ship', 'undeaddestroyership': 'Ship',
    # --- buildings
    'bridgeobelisk': 'Obelisk', 'icecrownobelisk': 'Obelisk',
    'elvenguardmagictower': 'MagicTower',
    'arcaneobservatory': 'Observatory', 'demongate': 'DemonGate',
    'sacrificialaltar': 'Altar', 'trollburrow': 'Burrow',
    'forgottenonetent': 'Tentacle',
    # --- props and pure effects
    'wisp': 'Wisp', 'skullpile0': 'SkullPile', 'icetorch': 'IceTorch',
    'eggsack0': 'EggSack', 'snowman': 'Snowman', 'thornsaura': 'ThornsAura',
    'commandaura': 'CommandAura', 'controlmagictarget': 'ControlMagic',
    'zombifytarget': 'DarkPortal',
}

# Anything unmapped is a soldier of some kind. Nothing should reach this - the
# emitter prints whatever did.
DEFAULT_MODEL = 'Warrior'
UNMAPPED = set()

# Six creeps keep the model of the unit they were cloned from, so there is no
# path to read - only the base id.
CREEP_MODELS = {
    'hgyr': 'Gyrocopter', 'oshm': 'Mage', 'uban': 'Wraith',
    'unec': 'Wraith', 'uabo': 'Brute', 'ufro': 'FrostWyrm',
}


def model_of(path):
    if not path:
        return None
    stem = path.replace(chr(92), '/').rsplit('/', 1)[-1].rsplit('.', 1)[0]
    m = MODELS.get(stem.lower())
    if m is None:
        UNMAPPED.add(stem)
        return DEFAULT_MODEL
    return m


def targets_of(g):
    """'air,enemies,ground' and friends -> what this tower can shoot at."""
    g = (g or '').lower()
    air = 'air' in g
    ground = 'ground' in g
    if 'enemies' in g and not air and not ground:
        # Chaos and Destruction list only "enemies". They have damage, splash
        # and a projectile, so they plainly attack - the map's author simply
        # never ticked the ground box. Ground is what they hit.
        return 'GroundOnly'
    if not air and not ground:
        return 'Nothing'
    if air and ground:
        return 'Both'
    return 'AirOnly' if air else 'GroundOnly'


# Seven waves are units the map placed without renaming, so there is no `unam`
# override in `war3map.w3u` to read: their names live in Warcraft III's own
# UnitData.slk, which is not in the archive. Falling back to the raw id meant
# waves six and seven announced themselves in the HUD as "hkni" and "hgyr".
#
# These are the stock ids the map actually uses, and nothing else - if a future
# map version places a different stock unit the emitter prints it as an unnamed
# wave rather than silently inventing a name.
STOCK_NAMES = {
    'hkni': 'Knight',
    'hgyr': 'Flying Machine',
    'oshm': 'Shaman',
    'uabo': 'Abomination',
    'uban': 'Banshee',
    'ufro': 'Frost Wyrm',
    'unec': 'Necromancer',
}


def unit_name(u, uid):
    """The name to show a player, and a complaint if there is not one."""
    named = w3obj.one(u, 'unam')
    if named:
        return named
    stock = STOCK_NAMES.get(uid)
    if stock:
        return stock
    print('  no name for stock unit %r - it will show as its id' % uid)
    return uid


def level_of(name, prefix):
    """The map's own level number, from the tail of the name."""
    tail = name[len(prefix):]
    m = re.search(r'(\d+)', tail)
    if m:
        return int(m.group(1))
    return 2 if 'Perfect' in tail else 1


# ---------------------------------------------------------------- abilities
#
# Each ability the towers carry, and the fields worth reading out of it. The
# key is the *base* ability, because the map clones stock ones and the clones
# keep the base id even when the name is blank.

def abil_fields(uid):
    """Every ability on a unit, as {base_id: {field: value}}."""
    out = {}
    for a in (w3obj.one(units[uid], 'uabi') or '').split(','):
        v = abils.get(a)
        if not v:
            continue
        vals = {}
        for f, lv in v['mods'].items():
            vals[f] = lv[0][1]
        out.setdefault(v['base'], {}).update(vals)
        out[v['base']]['_tip'] = w3obj.one(v, 'aub1') or ''
    return out


def read_abilities(uid):
    """The map's ability numbers, in this game's units."""
    a = abil_fields(uid)
    r = {
        'crit_chance': 0.0, 'crit_mult': 0.0,
        'multishot': 0, 'bounce': 0,
        'poison_dps': 0.0, 'poison_slow': 0.0, 'poison_dur': 0.0,
        'slow_amt': 0.0, 'slow_range': 0.0,
        'dmg_aura': 0.0, 'speed_aura': 0.0, 'aura_range': 0.0,
        'burn_dps': 0.0, 'burn_range': 0.0,
        'root_chance': 0.0, 'root_dur': 0.0,
        'kill_chance': 0.0,
        'armour_pen': 0,
        'frenzy': 0.0, 'frenzy_dur': 0.0, 'frenzy_cd': 0.0,
    }

    if 'AOcr' in a:  # Critical Strike
        v = a['AOcr']
        r['crit_chance'] = v.get('Ocr1', 0.0) / 100.0
        r['crit_mult'] = v.get('Ocr2', 2.0)

    if 'Aroc' in a:  # "Mutilshot" - fires on several targets at once
        r['multishot'] = int(a['Aroc'].get('Efk3', 0))

    if 'Amgr' in a:  # Moon Glaive - the shot leaps on
        r['bounce'] = 3

    if 'ACvs' in a:  # the poison sting
        v = a['ACvs']
        r['poison_dps'] = v.get('Poi1', 0.0)
        r['poison_slow'] = v.get('Poi2', 0.0)
        r['poison_dur'] = v.get('adur', v.get('ahdu', 0.0))

    if 'Aasl' in a:  # Cloud Slow / Icy Sting
        v = a['Aasl']
        r['slow_amt'] = abs(v.get('Slo1', 0.0))
        r['slow_range'] = v.get('aare', 0.0) / RANGE_DIV

    if 'SCae' in a:  # the Slow Tower's aura: faster towers around it
        r['speed_aura'] = max(r['speed_aura'], a['SCae'].get('Oae2', 0.0))
        r['aura_range'] = max(r['aura_range'], a['SCae'].get('aare', 0.0) / RANGE_DIV)

    if 'ACac' in a:  # the Fire Tower's damage aura
        r['dmg_aura'] = max(r['dmg_aura'], a['ACac'].get('Cac1', 0.0))
        r['aura_range'] = max(r['aura_range'], a['ACac'].get('aare', 0.0) / RANGE_DIV)

    if 'AEar' in a:  # Damage Tower
        r['dmg_aura'] = max(r['dmg_aura'], a['AEar'].get('Ear1', 0.0))
        r['aura_range'] = max(r['aura_range'], a['AEar'].get('aare', 0.0) / RANGE_DIV)

    if 'AOae' in a:  # Speed Tower
        r['speed_aura'] = max(r['speed_aura'], a['AOae'].get('Oae1', 0.0))
        r['aura_range'] = max(r['aura_range'], a['AOae'].get('aare', 0.0) / RANGE_DIV)

    if 'Atdg' in a:  # immolation: everything close by burns
        r['burn_dps'] = a['Atdg'].get('Tdg1', 0.0)
        r['burn_range'] = a['Atdg'].get('aare', 0.0) / RANGE_DIV

    if 'AEsb' in a:  # the Snowman's breath
        r['burn_dps'] = max(r['burn_dps'], a['AEsb'].get('Esf1', 0.0))
        r['burn_range'] = max(r['burn_range'], a['AEsb'].get('aare', 0.0) / RANGE_DIV)
        r['slow_amt'] = max(r['slow_amt'], a['AEsb'].get('Esf2', 0.0))

    if 'AIbx' in a:  # the Troll Tower's roots
        v = a['AIbx']
        r['root_chance'] = v.get('Hbh1', 0.0) / 100.0
        m = re.search(r'(\d+) second', v.get('_tip', ''))
        r['root_dur'] = float(m.group(1)) if m else 2.0

    if 'ACbh' in a:  # the Demon Tower kills outright
        r['kill_chance'] = a['ACbh'].get('Hbh1', 0.0) / 100.0

    if 'AIcb' in a:  # Corruption: the target's armour goes away
        r['armour_pen'] = int(a['AIcb'].get('Iarp', 0))

    if 'Afzy' in a:  # the Troll Tower works itself into a frenzy
        v = a['Afzy']
        r['frenzy'] = v.get('Blo1', 0.0)
        r['frenzy_dur'] = v.get('adur', 0.0)
        r['frenzy_cd'] = v.get('acdn', 0.0)

    return r


# ---------------------------------------------------------------- the roster

rows = []
by_uid = {}
for uid, v in units.items():
    n = w3obj.one(v, 'unam')
    if not n:
        continue
    fam = None
    prefix = None
    for tag, pre, _ in FAMILIES:
        if n.startswith(pre) and (prefix is None or len(pre) > len(prefix)):
            fam, prefix = tag, pre
    if fam is None:
        continue
    dice = w3obj.one(v, 'ua1d') or 0
    sides = w3obj.one(v, 'ua1s') or 0
    base = w3obj.one(v, 'ua1b') or 0
    dmg = base + dice * (sides + 1) / 2.0 if dice else base
    rows.append({
        'uid': uid,
        'fam': fam,
        'lvl': level_of(n, prefix),
        'name': n,
        'gold': w3obj.one(v, 'ugol') or 0,
        # Point value is what selling pays. A handful of towers do not set
        # one, and for those the map falls back to the build cost.
        'refund': w3obj.one(v, 'upoi') or (w3obj.one(v, 'ugol') or 0),
        'dmg': dmg,
        'cd': max(w3obj.one(v, 'ua1c') or 1.0, 0.10),
        'rng': (w3obj.one(v, 'ua1r') or 900) / RANGE_DIV,
        'aoe': (w3obj.one(v, 'ua1f') or 0) / RANGE_DIV,
        'atk': ATTACK.get((w3obj.one(v, 'ua1t') or '').strip(), 'Normal'),
        'targets': targets_of(w3obj.one(v, 'ua1g')),
        'model': model_of(w3obj.one(v, 'umdl')) or DEFAULT_MODEL,
        'scale': w3obj.one(v, 'usca') or 1.0,
        'abil': read_abilities(uid),
        'up': [x for x in (w3obj.one(v, 'uupt') or '').split(',') if x],
    })

# Ordered the way the shop and the tooltips read: family by family, and within
# a family by the map's own level number.
order = {tag: i for i, (tag, _, _) in enumerate(FAMILIES)}
rows.sort(key=lambda r: (order[r['fam']], r['lvl'], r['gold']))
index = {r['uid']: i for i, r in enumerate(rows)}
for r in rows:
    r['upgrades'] = [index[u] for u in r['up'] if u in index]

# The shop is exactly the set of towers nothing upgrades into.
reachable = set()
for r in rows:
    reachable.update(r['upgrades'])
for i, r in enumerate(rows):
    r['root'] = i not in reachable

# `step` is how far up its family a tower stands, so the HUD can say "4 of 20".
step = {}
for r in rows:
    r['step'] = step.get(r['fam'], 0)
    step[r['fam']] = r['step'] + 1

def wave_count(body):
    """How many creeps the wave sends.

    Most waves write a plain integer, but eight of them write a JASS character
    literal instead - `set udg_integer14='}'` is a hundred and twenty-five.
    Read literally those waves send nothing at all, which is why they looked
    empty the first time round.
    """
    m = re.search(r"udg_integer14=(\d+)", body)
    if m:
        return int(m.group(1))
    m = re.search(r"udg_integer14='(.)'", body)
    if m:
        return ord(m.group(1))
    return 0


# ---------------------------------------------------------------- waves

wave_rows = []
blocks = re.findall(
    r'function Trig_Waves(\d+)_Actions takes nothing returns nothing(.*?)endfunction',
    src, re.S)
for num, body in sorted(blocks, key=lambda b: int(b[0])):
    uid = re.search(r"udg_integer12='(....)'", body)
    if not uid:
        continue
    cnt = wave_count(body)
    wait = re.search(r'PolledWait\(([\d.]+)\)', body)
    tag = re.search(r'Wave \d+ of 36\|r \|CFF\w{6}(\w+)', body)
    u = units.get(uid.group(1), {'base': '', 'mods': {}})
    # Only 'fly' and 'hover' leave the ground. The map moved its five airborne
    # creeps to hover, which in Warcraft III makes them targetable from the
    # ground too - but every one of them is a dragon, a harpy or a gyrocopter,
    # and the Air Tower exists for them, so they fly here.
    mv = (w3obj.one(u, 'umvt') or '').strip().lower()
    wave_rows.append({
        'wave': int(num),
        'uid': uid.group(1),
        'name': unit_name(u, uid.group(1)),
        'count': cnt,
        'hp': w3obj.one(u, 'uhpm') or 100,
        'armour': w3obj.one(u, 'udef') or 0,
        'atype': ARMOUR.get((w3obj.one(u, 'udty') or 'normal').strip().lower(), 'Unarmoured'),
        'speed': w3obj.one(u, 'umvs') or 400,
        'flying': mv in ('fly', 'hover'),
        'model': model_of(w3obj.one(u, 'umdl')) or CREEP_MODELS.get(u.get('base', ''), 'Warrior'),
        'scale': w3obj.one(u, 'usca') or 1.0,
        'gap': float(wait.group(1)) if wait else 50.0,
        'tag': tag.group(1) if tag else '',
    })

# ---------------------------------------------------------------- damage table
#
# The single most load-bearing thing in the port, and the one the map does not
# put in its object data: `war3mapMisc.txt` overrides Warcraft III's whole
# attack-versus-armour table. It was hand-transcribed once, which meant a map
# version that changed those six lines would have disagreed with the code
# silently. Now it is read.
#
# The eight columns are the engine's defence types in its own order, which is
# not the order anybody writes them in:
#
#     0 Light  1 Medium  2 Large  3 Fortified  4 Normal  5 Hero  6 Divine  7 None
#
# "None" is what this game calls Unarmoured and "Large" is what it calls Heavy;
# defence type 4 is a legacy class no unit in this map uses.
MISC_COLUMN = {
    'Unarmoured': 7,
    'Light': 0,
    'Medium': 1,
    'Heavy': 2,
    'Fortified': 3,
    'Hero': 5,
    'Divine': 6,
}
ARMOUR_ORDER = ['Unarmoured', 'Light', 'Medium', 'Heavy', 'Fortified', 'Hero', 'Divine']
ATTACK_ORDER = ['Normal', 'Pierce', 'Siege', 'Magic', 'Chaos', 'Spells', 'Hero']


def damage_table():
    """[attack][armour] multipliers, read out of the map's own constants."""
    text = open('greentd/war3mapMisc.txt', encoding='latin-1').read()
    rows = {}
    for line in text.splitlines():
        if not line.startswith('DamageBonus'):
            continue
        key, _, vals = line.partition('=')
        rows[key[len('DamageBonus'):]] = [float(v) for v in vals.split(',')]

    out = []
    for atk in ATTACK_ORDER:
        if atk == 'Chaos':
            # Warcraft III hard-codes Chaos at 1.0 against everything and no
            # file can change it. That one row is why the Chaos, Destruction and
            # Troll families are worth their gold.
            out.append([1.0] * len(ARMOUR_ORDER))
            continue
        cols = rows.get(atk)
        if cols is None or len(cols) < 8:
            raise SystemExit('war3mapMisc.txt has no usable DamageBonus%s' % atk)
        out.append([cols[MISC_COLUMN[a]] for a in ARMOUR_ORDER])
    return out


DAMAGE = damage_table()

# ---------------------------------------------------------------- output

out = []
w = out.append
w('//! The Green Circle TD roster, upgrade graph and wave table.')
w('//!')
w('//! **Generated from `GREEN TD 9.3c PEIN.w3x` by `tools/emit.py` - do not')
w('//! hand-edit.**')
w('//!')
w("//! Every number here is the map's own: gold costs, refunds, damage,")
w('//! cooldowns, attack types, ability chances, creep health and armour, and')
w('//! the upgrade graph exactly as the map wires it. Ranges are the one')
w('//! conversion - divided by %g, the map\'s own units-per-tile - so a 900' % RANGE_DIV)
w('//! range tower reaches seven tiles here as it does there.')
w('')
w('use super::greentd_types::*;')
w('')
w('/// Every tower in the map, grouped by family.')
w('pub static LEVELS: &[TowerLevel] = &[')
last = None
for r in rows:
    if r['fam'] != last:
        w('    // ---- %s' % r['fam'])
        last = r['fam']
    a = r['abil']
    w('    TowerLevel {')
    w('        family: Family::%s,' % r['fam'])
    w('        step: %d,' % r['step'])
    w('        name: %s,' % json.dumps(r['name']))
    w('        gold: %d,' % r['gold'])
    w('        refund: %d,' % r['refund'])
    w('        damage: %.1f,' % r['dmg'])
    w('        cooldown: %.2f,' % r['cd'])
    w('        range: %.2f,' % r['rng'])
    w('        splash: %.2f,' % r['aoe'])
    w('        attack: Attack::%s,' % r['atk'])
    w('        targets: Targets::%s,' % r['targets'])
    w('        model: Model::%s,' % r['model'])
    w('        scale: %.2f,' % r['scale'])
    w('        shop: %s,' % ('true' if r['root'] else 'false'))
    w('        upgrades: &[%s],' % ', '.join(str(u) for u in r['upgrades']))
    w('        abil: Abil {')
    w('            crit_chance: %.2f,' % a['crit_chance'])
    w('            crit_mult: %.1f,' % a['crit_mult'])
    w('            multishot: %d,' % a['multishot'])
    w('            bounce: %d,' % a['bounce'])
    w('            poison_dps: %.1f,' % a['poison_dps'])
    w('            poison_slow: %.2f,' % a['poison_slow'])
    w('            poison_dur: %.1f,' % a['poison_dur'])
    w('            slow_amt: %.2f,' % a['slow_amt'])
    w('            slow_range: %.2f,' % a['slow_range'])
    w('            dmg_aura: %.2f,' % a['dmg_aura'])
    w('            speed_aura: %.2f,' % a['speed_aura'])
    w('            aura_range: %.2f,' % a['aura_range'])
    w('            burn_dps: %.1f,' % a['burn_dps'])
    w('            burn_range: %.2f,' % a['burn_range'])
    w('            root_chance: %.2f,' % a['root_chance'])
    w('            root_dur: %.1f,' % a['root_dur'])
    w('            kill_chance: %.2f,' % a['kill_chance'])
    w('            armour_pen: %d,' % a['armour_pen'])
    w('            frenzy: %.2f,' % a['frenzy'])
    w('            frenzy_dur: %.1f,' % a['frenzy_dur'])
    w('            frenzy_cd: %.1f,' % a['frenzy_cd'])
    w('        },')
    w('    },')
w('];')
w('')
w('/// The attack-versus-armour table, indexed `[attack][armour]`.')
w('///')
w('/// Straight out of `war3mapMisc.txt`, which this map uses to throw away')
w('/// Warcraft III\'s whole seven-by-seven counter table. Rows are in the order')
w('/// of `Attack`, columns in the order of `ArmourType`.')
w('pub static DAMAGE: [[f32; 7]; 7] = [')
for atk, row in zip(ATTACK_ORDER, DAMAGE):
    w('    // %s' % atk)
    w('    [%s],' % ', '.join('%.2f' % v for v in row))
w('];')
w('')
w('/// The thirty-six waves, in order.')
w('pub static WAVES: &[WaveRow] = &[')
for r in wave_rows:
    w('    WaveRow {')
    w('        wave: %d,' % r['wave'])
    w('        name: %s,' % json.dumps(r['name']))
    w('        count: %d,' % r['count'])
    w('        hp: %.1f,' % r['hp'])
    w('        armour: %d,' % r['armour'])
    w('        armour_type: ArmourType::%s,' % r['atype'])
    w('        speed: %.1f,' % r['speed'])
    w('        flying: %s,' % ('true' if r['flying'] else 'false'))
    w('        model: Model::%s,' % r['model'])
    w('        scale: %.2f,' % r['scale'])
    w('        gap: %.1f,' % r['gap'])
    w('        tag: %s,' % json.dumps(r['tag']))
    w('    },')
w('];')

open('../src/game/greentd.rs', 'w', encoding='utf-8', newline='\n').write('\n'.join(out) + '\n')

used = sorted({r['model'] for r in rows} | {r['model'] for r in wave_rows if r['model']})
if UNMAPPED:
    print('UNMAPPED: %s' % ', '.join(sorted(UNMAPPED)))
print('%d towers, %d waves, %d shop roots' % (
    len(rows), len(wave_rows), sum(1 for r in rows if r['root'])))
print('models: %s' % ', '.join(used))
