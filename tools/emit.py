"""Emits src/game/greentd.rs: the roster and waves, straight from the map."""

import json
import re
import w3obj

units, _ = w3obj.parse(open('greentd/war3map.w3u', 'rb').read(), False)
abils, _ = w3obj.parse(open('greentd/war3map.w3a', 'rb').read(), True)
src = open('greentd/script.j', encoding='latin-1').read()
ab_name = {k: (w3obj.one(v, 'anam') or '') for k, v in abils.items()}

# Warcraft III measures distance in world units, 128 to a tile. The map is
# 96x96 tiles holding eight arenas of roughly twenty tiles each, which is the
# same size as this board's circuit - so ranges convert one to one and a 900
# range tower reaches seven tiles here exactly as it does there.
RANGE_DIV = 128.0

FAMILIES = [
    # tag,               name prefix,               buildable from the shop
    ('Single',           'Single shot Tower',        True),
    ('Siege',            'Siege Tower',              True),
    ('Bouncing',         'Bouncing Tower',           True),
    ('Multi',            'Multi Tower',              True),
    ('Corruption',       'Corruption Tower',         True),
    ('Air',              'Air Tower',                True),
    ('Chaos',            'Chaos tower',              True),
    ('Destruction',      'Destruction Tower',        True),
    ('Aura',             'Aura Tower',               True),
    ('Demon',            'Demon Tower',              True),
    ('King',             'King Tower',               True),
    # these six are reached by specialising the 10 gold Single shot Tower
    ('Slow',             'Slow Tower',               False),
    ('Poison',           'Poison Tower',             False),
    ('Critical',         'Critical Tower',           False),
    ('Troll',            'Troll Tower',              False),
    ('Fire',             'Fire Tower',               False),
    ('OneStrike',        'One-Strike Kill Tower',    False),
]
PREFIX = {t: p for t, p, _ in FAMILIES}

ATTACK = {
    '': 'Normal', 'normal': 'Normal', 'siege': 'Siege', 'magic': 'Magic',
    'chaos': 'Chaos', 'spells': 'Spells', 'hero': 'Hero', 'unknown': 'Normal',
}

ABIL_FLAGS = [
    ('Critical', 'CRIT'),
    ('Mutilshot', 'MULTISHOT'),
    ('Roots', 'ROOTS'),
    ('Slow', 'SLOW'),
    ('KIll', 'INSTAKILL'),
    ('Corruption Bonus', 'CORRUPTION'),
    ('Damage Aura', 'DAMAGE_AURA'),
    ('Speed Aura', 'SPEED_AURA'),
]

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


rows = []
for k, v in units.items():
    n = w3obj.one(v, 'unam')
    if not n:
        continue
    fam = next((t for t, p, _ in FAMILIES if n.startswith(p)), None)
    if fam is None:
        continue
    dice = w3obj.one(v, 'ua1d') or 0
    sides = w3obj.one(v, 'ua1s') or 0
    base = w3obj.one(v, 'ua1b') or 0
    dmg = base + dice * (sides + 1) / 2.0 if dice else base
    tail = n[len(PREFIX[fam]):]
    m = re.search(r'(\d+)', tail)
    lvl = int(m.group(1)) if m else 1
    ab = ' '.join(ab_name.get(a, a) for a in (w3obj.one(v, 'uabi') or '').split(',') if a)
    flags = [f for key, f in ABIL_FLAGS if key in ab]
    rows.append({
        'fam': fam, 'lvl': lvl, 'name': n, 'gold': w3obj.one(v, 'ugol') or 0,
        'dmg': dmg, 'cd': max(w3obj.one(v, 'ua1c') or 1.0, 0.10),
        'rng': (w3obj.one(v, 'ua1r') or 900) / RANGE_DIV,
        'aoe': (w3obj.one(v, 'ua1f') or 0) / RANGE_DIV,
        'atk': ATTACK.get((w3obj.one(v, 'ua1t') or '').strip(), 'Normal'),
        'targets': targets_of(w3obj.one(v, 'ua1g')),
        'flags': flags,
    })

out = []
w = out.append
w('//! The Green Circle TD roster and wave table.')
w('//!')
w('//! **Generated from `GREEN TD 9.3c PEIN.w3x` - do not hand-edit.**')
w('//!')
w('//! Every number here is the map\'s own: gold costs, damage, cooldowns, attack')
w('//! types, creep health and armour. Ranges are the only thing changed, divided')
w('//! by %g - the map.s own tiles-to-units ratio - so a 900 range tower reaches' % RANGE_DIV)
w('//! seven tiles here exactly as it does there.')
w('')
w('use super::greentd_types::*;')
w('')

w('/// Every level of every tower, grouped by family, cheapest first.')
w('pub static LEVELS: &[TowerLevel] = &[')
for tag, prefix, _ in FAMILIES:
    fr = sorted([r for r in rows if r['fam'] == tag], key=lambda r: (r['lvl'], r['gold']))
    if not fr:
        continue
    w('    // ---- %s' % tag)
    for i, r in enumerate(fr):
        flags = ' | '.join('Flag::%s' % f for f in r['flags']) or 'Flag::NONE'
        w('    TowerLevel {')
        w('        family: Family::%s,' % tag)
        w('        step: %d,' % i)
        w('        name: %s,' % json.dumps(r['name']))
        w('        gold: %d,' % r['gold'])
        w('        damage: %.1f,' % r['dmg'])
        w('        cooldown: %.2f,' % r['cd'])
        w('        range: %.2f,' % r['rng'])
        w('        splash: %.2f,' % r['aoe'])
        w('        attack: Attack::%s,' % r['atk'])
        w('        targets: Targets::%s,' % r['targets'])
        w('        flags: %s,' % flags)
        w('    },')
w('];')
w('')

# ---------------------------------------------------------------- waves
blocks = re.findall(
    r'function Trig_Waves(\d+)_Actions takes nothing returns nothing(.*?)endfunction', src, re.S)
w('/// The thirty-six waves, in order.')
w('pub static WAVES: &[WaveRow] = &[')
for num, body in sorted(blocks, key=lambda b: int(b[0])):
    uid = re.search(r"udg_integer12='(....)'", body)
    cnt = re.search(r'udg_integer14=(\d+)', body)
    if not uid:
        continue
    u = units.get(uid.group(1), {'mods': {}})
    nm = w3obj.one(u, 'unam') or uid.group(1)
    hp = w3obj.one(u, 'uhpm') or 100
    armour = w3obj.one(u, 'udef') or 0
    at = (w3obj.one(u, 'udty') or 'normal').strip().lower()
    at = {'none': 'Unarmoured', 'normal': 'Unarmoured', 'medium': 'Medium',
          'hero': 'Hero', 'divine': 'Divine', 'large': 'Heavy', 'small': 'Light',
          'fort': 'Fortified'}.get(at, 'Unarmoured')
    spd = w3obj.one(u, 'umvs') or 400
    flying = uid.group(1) in ('hgyr',)
    w('    WaveRow { wave: %s, name: %s, count: %s, hp: %s.0, armour: %s, armour_type: ArmourType::%s, speed: %s.0, flying: %s },'
      % (num, json.dumps(nm), cnt.group(1) if cnt else 0, hp, armour, at, spd,
         'true' if flying else 'false'))
w('];')

open('../greentd_gen.rs', 'w', encoding='utf-8', newline='\n').write('\n'.join(out) + '\n')
print('emitted %d tower levels and %d waves' % (len(rows), len(blocks)))
