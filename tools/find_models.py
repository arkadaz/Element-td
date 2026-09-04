"""Search Poly Pizza for a CC0 model for every archetype, and write a catalogue.

This is run by hand, not by the build. It produces `tools/models.json`, which is
a curated file: the search picks candidates, a person picks the winner. Nothing
downstream re-runs a search, so the game's models never change because somebody
else uploaded something.

    python find_models.py            # search everything, write candidates
    python find_models.py orc troll  # just these
"""

import json
import os
import re
import subprocess
import sys
import time

BASE = 'https://poly.pizza'
HERE = os.path.dirname(os.path.abspath(__file__))

# What to search for, per archetype in `Model`. Several archetypes share a
# search - a Warrior and a Knight are both "knight" as far as a CC0 pack is
# concerned - and the curation step is where they get told apart.
TERMS = {
    'Acolyte': 'cultist robed monk', 'Archer': 'archer elf ranger',
    'Mage': 'wizard mage', 'Warrior': 'orc warrior', 'Demon': 'demon',
    'Brute': 'ogre monster big', 'Troll': 'goblin imp', 'Gnoll': 'gnoll hyena wolf man',
    'Skeleton': 'skeleton', 'Wraith': 'ghost wraith', 'Naga': 'naga snake man',
    'Rifleman': 'dwarf soldier gun', 'Villager': 'villager peasant',
    'Panda': 'panda', 'Bear': 'bear animal', 'Mammoth': 'mammoth woolly',
    'Centaur': 'centaur horse', 'Lizard': 'lizard raptor', 'Crab': 'crab',
    'Spider': 'spider', 'Serpent': 'snake serpent', 'Turtle': 'tortoise turtle animal',
    'Ent': 'tree monster ent', 'Golem': 'stone golem enemy', 'Giant': 'giant',
    'Infernal': 'lava golem infernal', 'FlameLord': 'fire monster lava',
    'Gyrocopter': 'helicopter', 'Phoenix': 'bird eagle',
    'Harpy': 'harpy bird woman', 'Dragon': 'dragon', 'FrostWyrm': 'ice dragon',
    'Turret': 'turret', 'Turbolazer': 'laser turret', 'RebelTurret': 'turret gun',
    'Vulcan': 'minigun turret', 'SamSite': 'rocket launcher turret', 'Cannon': 'cannon',
    'MeatWagon': 'cart wagon', 'Ship': 'ship boat',
    'Obelisk': 'stone pillar monolith', 'MagicTower': 'wizard tower',
    'Observatory': 'observatory tower', 'DemonGate': 'stone arch ruins',
    'Altar': 'altar shrine', 'Burrow': 'hut burrow', 'Tentacle': 'tentacle',
    'Wisp': 'crystal light', 'SkullPile': 'skull pile bones', 'IceTorch': 'ice spike crystal',
    'EggSack': 'egg', 'Snowman': 'snowman', 'ThornsAura': 'cactus thorn',
    'CommandAura': 'banner flag', 'ControlMagic': 'crystal orb',
    'DarkPortal': 'magic portal ring',
}

# Creators whose work is CC0 across the board. A hit from one of these is
# preferred over a higher-ranked hit from someone whose licence has to be
# checked one model at a time.
TRUSTED = ('Quaternius', 'Kenney', 'kenney')


def get(url):
    r = subprocess.run(['curl', '-sL', '--max-time', '30', url],
                       capture_output=True)
    return r.stdout.decode('utf-8', 'replace')


def search(term, limit=5):
    h = get('%s/search/%s' % (BASE, term.replace(' ', '%20')))
    ids = []
    for m in re.finditer(r'href="/m/([^"]+)"', h):
        if m.group(1) not in ids:
            ids.append(m.group(1))
        if len(ids) >= limit:
            break
    return ids


def model(mid):
    h = get('%s/m/%s' % (BASE, mid))
    glb = re.search(r'(https://static\.poly\.pizza/[0-9a-f-]+\.glb)', h)
    ttl = re.search(r'<h1[^>]*>([^<]*)</h1>', h) or re.search(r'<title>([^<]*)</title>', h)
    who = re.search(r'href="/u/([^"]+)"', h)
    cc0 = bool(re.search(r'CC0|Creative Commons Zero|Public Domain', h, re.I))
    return {
        'id': mid,
        'title': (ttl.group(1).strip() if ttl else ''),
        'creator': (who.group(1) if who else ''),
        'cc0': cc0,
        'glb': (glb.group(1) if glb else None),
    }


def main(only):
    out = {}
    path = os.path.join(HERE, 'model_candidates.json')
    if os.path.exists(path):
        out = json.load(open(path))
    for arch, term in TERMS.items():
        if only and arch.lower() not in only:
            continue
        rows = []
        for mid in search(term):
            try:
                r = model(mid)
            except Exception as exc:
                print('  ! %s %s' % (mid, exc))
                continue
            if r['glb'] and r['cc0']:
                rows.append(r)
            time.sleep(0.1)
        rows.sort(key=lambda r: (r['creator'] not in TRUSTED,))
        out[arch] = rows
        best = rows[0] if rows else None
        print('%-14s %-22s %s' % (
            arch, term,
            ('%s by %s' % (best['title'][:26], best['creator'])) if best
            else 'NOTHING CC0 FOUND'))
        json.dump(out, open(path, 'w'), indent=1)
    print('\\nwrote %s (%d archetypes)' % (path, len(out)))


if __name__ == '__main__':
    main([a.lower() for a in sys.argv[1:]])
