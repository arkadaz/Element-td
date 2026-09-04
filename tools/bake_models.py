"""Download the curated CC0 models and bake them into one blob the game loads.

    python bake_models.py            # everything in MODELS
    python bake_models.py Orc Dragon # just these

Writes `assets/models.bin`, plus `assets/CREDITS.md`. Downloads are cached in
`tools/glb/` and never re-fetched, so a rebuild is offline and reproducible.

What the bake does, and why each step is not the game's job:

- **Poses the figure.** These packs ship in a T-pose with the walk cycle in an
  animation clip. The game has no skinning - it draws seven hundred units and
  cannot afford per-frame joint maths - so the skinning is done once, here.
- **Turns the model the right way up.** glTF is Y-up; the game is Z-up.
- **Normalises size.** Every model comes out one unit tall standing on z = 0
  with its footprint centred, so `Pose::r` means the same thing for all of them.
- **Bakes colour per vertex.** Every material in these packs is a flat
  `baseColorFactor`, so no texture path is needed anywhere in the engine.
"""

import json
import os
import struct
import subprocess
import sys

import gltf

HERE = os.path.dirname(os.path.abspath(__file__))
CACHE = os.path.join(HERE, 'glb')
OUT = os.path.abspath(os.path.join(HERE, '..', 'assets'))

# archetype -> (poly.pizza id, animation clip, where in the clip to freeze)
#
# 'auto' picks Walk, then Run, then Idle, by name. An index overrides it.
#
# Triangle count is a real constraint here and not a stylistic one. Seven
# hundred monsters may be circling at once, so a model's count is multiplied by
# seven hundred in the worst frame the game permits: the first pick for Ship was
# 20,632 triangles, which is fourteen million a frame. Anything over about eight
# thousand needs a lighter model, and `no_baked_model_is_too_heavy_for_a_full_wave`
# is the test that says so.
#
# The id is pinned rather than searched, so the game's models cannot change
# because somebody uploaded something new. `find_models.py` proposes these;
# this table is the decision. The fraction is where in the clip to freeze:
# 0.35 is a little past the contact pose, which reads as mid-stride.
MODELS = {
    'Acolyte': ('j4rVPvxyLg', 'auto', 0.35),  # Monkroose by Quaternius
    'Archer': ('52fgB0PCFu', 'auto', 0.35),  # Archery Second Age by Quaternius
    'Mage': ('o87Upt5uHX', 'auto', 0.35),  # Wizard by Quaternius
    'Warrior': ('5vO2YJsPEf', 'auto', 0.35),  # Orc by Quaternius
    'Demon': ('LnfIziKv4o', 'auto', 0.35),  # Demon by Quaternius
    'Brute': ('UgBmRVnQ9h', 'auto', 0.35),  # Enemy Large by Quaternius
    'Gnoll': ('P1gU3Qkr9r', 'auto', 0.35),  # Wolf by Quaternius
    'Skeleton': ('wODZYCgX5Z', 'auto', 0.35),  # Skeleton by Quaternius
    'Wraith': ('TX8r9WBXpe', 'auto', 0.35),  # Ghost Skull by Quaternius
    'Villager': ('7pn3R6hPvE', 'auto', 0.35),  # Farmer by Quaternius
    'Panda': ('q1uJ28Hs8T', 'auto', 0.35),  # Panda by Quaternius
    'Centaur': ('qvTrSG9pZF', 'auto', 0.35),  # Horse by Quaternius
    'Lizard': ('cnlGH2UcDd', 'auto', 0.35),  # Velociraptor by Quaternius
    'Crab': ('Gs3yfsV5lB', 'auto', 0.35),  # Crab Enemy by Quaternius
    'Spider': ('yRYJiAJyiM', 'auto', 0.35),  # Spider by Quaternius
    'Serpent': ('x9x0viZs8V', 'auto', 0.35),  # Snake by Quaternius
    'Naga': ('x9x0viZs8V', 'auto', 0.35),  # Snake by Quaternius
    'Giant': ('BldaiPtyJa', 'auto', 0.35),  # Giant by Quaternius
    'Dragon': ('VBvzjFIYws', 'auto', 0.35),  # Dragon by Quaternius
    'FrostWyrm': ('LlwD0QNUPj', 'auto', 0.35),  # Dragon Evolved by Quaternius
    'Turret': ('mXKbcMPLSS', 'auto', 0.35),  # Turret by Kenney
    'Turbolazer': ('mNJ6poH7Cp', 'auto', 0.35),  # Turret Cannon by Quaternius
    'RebelTurret': ('ekTQhbJId7', 'auto', 0.35),  # Turret Gun by Quaternius
    'Cannon': ('J15vlPVvKK', 'auto', 0.35),  # Cannon by Quaternius
    'Ship': ('cIzO4MBPqI', 'auto', 0.35),  # Sail Ship by Quaternius
    'MagicTower': ('iuMDwgTRMU', 'auto', 0.35),  # Tower by Quaternius
    'Observatory': ('cMxuj2gt7D', 'auto', 0.35),  # Watch Tower by Quaternius
    'Altar': ('CE2Mn7lh6A', 'auto', 0.35),  # Temple by Quaternius
    'Burrow': ('wxi3kAu5ey', 'auto', 0.35),  # Hut by Quaternius
    'Tentacle': ('BR1vpIvvvv', 'auto', 0.35),  # Tentacle by Quaternius
    'SkullPile': ('ExZmhOIjka', 'auto', 0.35),  # Skull by Quaternius
    'Snowman': ('UBldLFdI7l', 'auto', 0.35),  # Snowman by Polygonal Mind
    'ControlMagic': ('xVFoDfiScT', 'auto', 0.35),  # Crystal by iPoly3D
    'CommandAura': ('i0PZVuVlYv', 'auto', 0.35),  # Crown by Quaternius
    'Infernal': ('iHEuXiH6Aj', 'auto', 0.35),  # Goleling Evolved by Quaternius
    'Harpy': ('gZ2ExU9OAB', 'auto', 0.35),  # Birb by Quaternius
}


def fetch(mid):
    """The .glb for a poly.pizza id, cached on disk."""
    os.makedirs(CACHE, exist_ok=True)
    path = os.path.join(CACHE, '%s.glb' % mid)
    if os.path.exists(path) and os.path.getsize(path) > 0:
        return path
    page = subprocess.run(
        ['curl', '-sL', '--max-time', '30', 'https://poly.pizza/m/%s' % mid],
        capture_output=True).stdout.decode('utf-8', 'replace')
    import re
    m = re.search(r'(https://static\.poly\.pizza/[0-9a-f-]+\.glb)', page)
    if not m:
        raise SystemExit('no .glb on the page for %s' % mid)
    subprocess.run(['curl', '-sL', '--max-time', '90', '-o', path, m.group(1)],
                   check=True)
    ttl = re.search(r'<h1[^>]*>([^<]*)</h1>', page)
    who = re.search(r'href="/u/([^"]+)"', page)
    meta = {'id': mid, 'url': m.group(1),
            'title': ttl.group(1).strip() if ttl else '',
            'creator': who.group(1) if who else ''}
    json.dump(meta, open(path + '.json', 'w'))
    return path


def bake_one(path, anim, at):
    """Posed, Z-up, one unit tall, feet on the ground, centred."""
    tris = gltf.triangles(path, anim=anim, at=at)
    if not tris:
        raise SystemExit('%s baked to nothing' % path)

    # glTF is Y-up and right-handed; the game is Z-up. Sending -z to y keeps the
    # handedness, so faces still wind the way the renderer expects and nothing
    # comes out inside-out.
    def conv(v):
        return (v[0], -v[2], v[1])

    pos = [conv(p) for p, _n, _c in tris]
    nrm = [conv(n) for _p, n, _c in tris]
    col = [c for _p, _n, c in tris]

    lo = [min(p[i] for p in pos) for i in range(3)]
    hi = [max(p[i] for p in pos) for i in range(3)]
    height = max(hi[2] - lo[2], 1e-4)
    k = 1.0 / height
    cx = (lo[0] + hi[0]) * 0.5
    cy = (lo[1] + hi[1]) * 0.5

    out = []
    for p, n, c in zip(pos, nrm, col):
        out.append((
            ((p[0] - cx) * k, (p[1] - cy) * k, (p[2] - lo[2]) * k),
            n, c))
    return out


MAGIC = 0x4D445447  # "GTDM"


def main(only):
    os.makedirs(OUT, exist_ok=True)
    names, blobs, credits = [], [], []
    for arch, (mid, anim, at) in MODELS.items():
        if only and arch.lower() not in only:
            continue
        path = fetch(mid)
        verts = bake_one(path, anim, at)
        names.append(arch)
        blobs.append(verts)
        meta_path = path + '.json'
        meta = json.load(open(meta_path)) if os.path.exists(meta_path) else {}
        credits.append((arch, meta.get('title', ''), meta.get('creator', ''),
                        meta.get('url', '')))
        print('  %-12s %6d tris  %s by %s'
              % (arch, len(verts) // 3, meta.get('title', '?'),
                 meta.get('creator', '?')))

    body = bytearray()
    index = bytearray()
    for name, verts in zip(names, blobs):
        first = len(body) // 36
        for p, n, c in verts:
            body += struct.pack('<9f', p[0], p[1], p[2], n[0], n[1], n[2],
                                c[0], c[1], c[2])
        nb = name.encode('ascii')
        index += struct.pack('<B', len(nb)) + nb
        index += struct.pack('<II', first, len(verts))

    blob = struct.pack('<III', MAGIC, 1, len(names)) + bytes(index) + bytes(body)
    dst = os.path.join(OUT, 'models.bin')
    open(dst, 'wb').write(blob)
    print('\nwrote %s  %d models  %d vertices  %.1f KB'
          % (dst, len(names), len(body) // 36, len(blob) / 1024.0))

    with open(os.path.join(OUT, 'CREDITS.md'), 'w', newline='\n') as f:
        f.write('# Model credits\n\n')
        f.write('Every model in `models.bin` is CC0 (public domain). They are\n')
        f.write('baked by `tools/bake_models.py` from the sources below.\n\n')
        f.write('| Archetype | Model | Author | Source |\n')
        f.write('| --- | --- | --- | --- |\n')
        for arch, title, who, url in credits:
            f.write('| %s | %s | %s | %s |\n' % (arch, title, who, url))


if __name__ == '__main__':
    main([a.lower() for a in sys.argv[1:]])
