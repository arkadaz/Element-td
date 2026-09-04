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
    'Troll': ('S7jYW6Amye', 'auto', 0.35),  # Blue Demon by Quaternius
    'Rifleman': ('Btfn3G5Xv4', 'auto', 0.35),  # SWAT by Quaternius
    'Bear': ('P1gU3Qkr9r', 'auto', 0.35),  # Wolf by Quaternius
    'Mammoth': ('26zM1outCr', 'auto', 0.35),  # Cow by Quaternius
    'Turtle': ('xmwfRzGvPv', 'auto', 0.35),  # Turtle Character by Polygonal Mind
    'Ent': ('kBomlgZ5xu', 'auto', 0.35),  # Tree Spiral by Quaternius
    'Golem': ('71gomWolax', 'auto', 0.35),  # Goleling by Quaternius
    'FlameLord': ('Azj9hJwwwG', 'auto', 0.35),  # Bonfire by Quaternius
    'Gyrocopter': ('EQJ2MECUbx', 'auto', 0.35),  # Helicopter by kazuma
    'Phoenix': ('gZ2ExU9OAB', 'auto', 0.35),  # Birb by Quaternius
    'Vulcan': ('ekTQhbJId7', 'auto', 0.35),  # Turret Gun by Quaternius
    'SamSite': ('9awwTQWYux', 'auto', 0.35),  # Rocket by hat_my_guy
    'MeatWagon': ('NBDHe8J7f9', 'auto', 0.35),  # Broken Cart by Quaternius
    'Obelisk': ('wLubNpOTX4', 'auto', 0.35),  # Column by Quaternius
    'DemonGate': ('QwWdOcNIMh', 'auto', 0.35),  # Arch by Quaternius
    'Wisp': ('hKZtOOMadH', 'auto', 0.35),  # Planet by Quaternius
    'IceTorch': ('ySa8IekR6i', 'auto', 0.35),  # Crystal by iPoly3D
    'EggSack': ('ngjyRi84lk', 'auto', 0.35),  # Egg by Quaternius
    'ThornsAura': ('HsEJgRLQWX', 'auto', 0.35),  # Cactus by Quaternius
    'DarkPortal': ('R5tsCpWwD2', 'auto', 0.35),  # Landing Pad by Kay Lousberg
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
    """Two poses of the walk, Z-up, one unit tall, feet on the ground, centred.

    Returns `(vertices, deltas)`: the first pose, and the offset from it to the
    second. The game stores both and blends between them in the vertex shader,
    which turns a statue that slides into something that walks.

    Two poses rather than a skeleton because the game has no skinning and cannot
    afford it - seven hundred units means seven hundred sets of joint matrices a
    frame. Two rather than four because two is the whole of a walk: the halves
    of the cycle are the same pose with the legs swapped, so blending back and
    forth between the contact poses at 0.0 and 0.5 of the clip *is* the stride.

    The two are sampled from one clip, so vertex order and count match by
    construction and the correspondence needs no work.
    """
    tris = gltf.triangles(path, anim=anim, at=at)
    if not tris:
        raise SystemExit('%s baked to nothing' % path)
    # Half a cycle further on, wrapped.
    other = gltf.triangles(path, anim=anim, at=(at + 0.5) % 1.0)
    if len(other) != len(tris):
        # Cannot happen from one clip on one mesh, but a model with no
        # animation at all returns its rest pose twice, and that is fine.
        other = tris

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

    pos_b = [conv(p) for p, _n, _c in other]
    nrm_b = [conv(n) for _p, n, _c in other]

    out, delta = [], []
    for i, (p, n, c) in enumerate(zip(pos, nrm, col)):
        a = ((p[0] - cx) * k, (p[1] - cy) * k, (p[2] - lo[2]) * k)
        out.append((a, n, c))
        # The second pose is normalised by the *first* pose's transform, not by
        # its own. Fitting each to its own bounds would scale the model slightly
        # differently in each half of the stride, and the figure would pulse.
        q = pos_b[i]
        b = ((q[0] - cx) * k, (q[1] - cy) * k, (q[2] - lo[2]) * k)
        m = nrm_b[i]
        delta.append((
            (b[0] - a[0], b[1] - a[1], b[2] - a[2]),
            (m[0] - n[0], m[1] - n[1], m[2] - n[2]),
        ))
    return out, delta


MAGIC = 0x4D445447  # "GTDM"

# Bytes per vertex, quantised.
#
#   position         3 x i16   snorm, times POS_RANGE
#   (padding)        1 x i16
#   normal           3 x i8    snorm, and one spare byte
#   colour           3 x u8    unorm, and one spare byte
#   position delta   3 x i16   snorm, times POS_RANGE
#   (padding)        1 x i16
#   normal delta     3 x i8    snorm, and one spare byte
#
# Floats were 60 bytes and 28 MB of blob, which is more than the whole rest of
# the game. None of it needs float precision: a model is one unit tall, so a
# 16-bit position is accurate to a quarter of a millimetre at human scale, and
# normals and colours have never needed more than eight bits anywhere.
#
# The padding is not waste - the GPU vertex formats are Snorm16x4 and Snorm8x4,
# so the fourth component has to be there whether it is used or not.
STRIDE = 28

# The largest coordinate a model may have, in units of its own height. Positions
# are stored as a fraction of this. Four is comfortable: models are normalised to
# one unit tall, and the widest of them - a dragon with its wings out - is about
# three across.
POS_RANGE = 4.0


def q16(v):
    """A coordinate as a signed 16-bit fraction of POS_RANGE."""
    x = max(-1.0, min(1.0, v / POS_RANGE))
    return max(-32767, min(32767, int(round(x * 32767.0))))


def q8(v):
    """A signed unit value as a signed byte."""
    x = max(-1.0, min(1.0, v))
    return max(-127, min(127, int(round(x * 127.0))))


def u8(v):
    """An unsigned unit value as an unsigned byte."""
    return max(0, min(255, int(round(max(0.0, min(1.0, v)) * 255.0))))


def main(only):
    os.makedirs(OUT, exist_ok=True)
    names, blobs, credits = [], [], []
    for arch, (mid, anim, at) in MODELS.items():
        if only and arch.lower() not in only:
            continue
        path = fetch(mid)
        verts, delta = bake_one(path, anim, at)
        names.append(arch)
        blobs.append((verts, delta))
        meta_path = path + '.json'
        meta = json.load(open(meta_path)) if os.path.exists(meta_path) else {}
        credits.append((arch, meta.get('title', ''), meta.get('creator', ''),
                        meta.get('url', '')))
        swing = max(
            abs(d[0][0]) + abs(d[0][1]) + abs(d[0][2]) for d in delta
        ) if delta else 0.0
        print('  %-12s %6d tris  swing %.2f  %s by %s'
              % (arch, len(verts) // 3, swing, meta.get('title', '?'),
                 meta.get('creator', '?')))

    body = bytearray()
    index = bytearray()
    for name, (verts, delta) in zip(names, blobs):
        first = len(body) // STRIDE
        for (p, n, c), (dp, dn) in zip(verts, delta):
            body += struct.pack(
                '<4h4b4B4h4b',
                q16(p[0]), q16(p[1]), q16(p[2]), 0,
                q8(n[0]), q8(n[1]), q8(n[2]), 0,
                u8(c[0]), u8(c[1]), u8(c[2]), 255,
                q16(dp[0]), q16(dp[1]), q16(dp[2]), 0,
                q8(dn[0]), q8(dn[1]), q8(dn[2]), 0,
            )
        nb = name.encode('ascii')
        index += struct.pack('<B', len(nb)) + nb
        index += struct.pack('<II', first, len(verts))

    blob = struct.pack('<III', MAGIC, 3, len(names)) + bytes(index) + bytes(body)
    dst = os.path.join(OUT, 'models.bin')
    open(dst, 'wb').write(blob)
    print('\nwrote %s  %d models  %d vertices  %.1f KB'
          % (dst, len(names), len(body) // STRIDE, len(blob) / 1024.0))

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
