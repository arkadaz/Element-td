"""Download the curated CC0 models and bake them into one blob the game loads.

    python bake_models.py            # everything in MODELS
    python bake_models.py Orc Dragon # just these

Writes `assets/models.bin`, plus `assets/CREDITS.md`. Downloads are cached in
`tools/glb/`, `tools/quaternius_turrets/` and `tools/kenney_nature/`, so a
rebuild is offline and reproducible.

What the bake does, and why each step is not the game's job:

- **Poses the figure.** These packs ship in a T-pose with the walk cycle in an
  animation clip. The game has no skinning - it draws seven hundred units and
  cannot afford per-frame joint maths - so the skinning is done once, here.
- **Turns the model the right way up.** glTF is Y-up; the game is Z-up.
- **Normalises size.** Upright models come out one unit tall, while very wide
  aircraft, ships and winged creatures are also constrained by their footprint.
  Everything stands on z = 0 with its footprint centred, so `Pose::r` means the
  same visual occupancy for all of them rather than turning a short helicopter
  rotor into a four-tile monster.
- **Bakes colour per vertex.** Source material separation remains readable,
  while the runtime adds tiled albedo/normal detail through its shared
  triplanar material array.
"""

import json
import os
import struct
import subprocess
import sys
import zipfile

import gltf

HERE = os.path.dirname(os.path.abspath(__file__))
CACHE = os.path.join(HERE, 'glb')
OUT = os.path.abspath(os.path.join(HERE, '..', 'assets'))
NATURE_CACHE = os.path.join(HERE, 'kenney_nature')
NATURE_ZIP = os.path.join(HERE, 'kenney_nature-kit.zip')
NATURE_URL = ('https://kenney.nl/media/pages/assets/nature-kit/'
              '37ac38a37b-1677698939/kenney_nature-kit.zip')
TURRET_CACHE = os.path.join(HERE, 'quaternius_turrets', 'OBJ')
TURRET_SOURCE = 'https://quaternius.com/packs/turretpack.html'

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

# Accent hue per tower family. The Quaternius source pack uses two copper bands;
# remapping the lighter band gives each command-card family a battlefield
# identity while retaining the authored gunmetal bodies.
TOWER_ACCENTS = {
    'TowerSeed': (0.12, 1.00, 0.34),
    'TowerSiege': (1.00, 0.46, 0.08),
    'TowerBounce': (0.42, 0.20, 1.00),
    'TowerMulti': (0.08, 0.58, 1.00),
    'TowerCorrupt': (0.05, 1.00, 0.38),
    'TowerAir': (0.05, 0.82, 1.00),
    'TowerChaos': (1.00, 0.05, 0.08),
    'TowerDestroy': (1.00, 0.20, 0.03),
    'TowerAura': (0.72, 0.12, 1.00),
    'TowerDemon': (1.00, 0.05, 0.52),
    'TowerKing': (1.00, 0.68, 0.08),
}

# Actual weapon towers from Quaternius' CC0 Steampunk Turret Pack. Every
# command family owns its own silhouettes; repeated final entries stay inside
# that family only, where the runtime's apex dressing makes the last upgrade a
# visibly stronger version of the same weapon. No house, market or barracks is
# allowed in this roster.
_TURRET_STAGE_STEMS = {
    'TowerSeed': ['Gun', 'Gun_2', 'Gun_3', 'Gun_4'],
    'TowerSiege': ['Cannon_1', 'Cannon_3', 'Cannon_5', 'Cannon_7'],
    'TowerBounce': ['Bomber_1', 'Bomber_2', 'GearCannon_1', 'GearCannon_1'],
    'TowerMulti': ['Gun_5', 'Gun_6', 'Laser_MachineGun', 'Laser_MachineGun'],
    'TowerCorrupt': ['Teleporter1', 'Teleporter2', 'Teleporter3', 'Teleporter3'],
    'TowerAir': ['Upwards_1', 'Upwards_2', 'Upwards_3_', 'Upwards_3_'],
    'TowerChaos': ['Laser_1', 'Laser_2', 'Laser_Doble', 'Laser_Doble'],
    'TowerDestroy': ['Long_1', 'Long_2', 'Cannon_6', 'Cannon_6'],
    'TowerAura': ['Teleporter4', 'Teleporter5', 'Laser_Quad', 'Laser_Quad'],
    'TowerDemon': ['Cannon_2', 'Cannon_4', 'GearCannon_2', 'GearCannon_2'],
    'TowerKing': ['Gun_7', 'Gun_8', 'Gun_9', 'Gun_10'],
}

TURRET_MODELS = {
    '%s%d' % (family, stage): stem
    for family, stems in _TURRET_STAGE_STEMS.items()
    for stage, stem in enumerate(stems)
}

# Pinned file ids from the pack's official public Google Drive folder. Only
# OBJ geometry is required: the pack uses two flat source materials, which are
# remapped below to a dark gunmetal and the owning tower family's accent.
TURRET_FILES = {
    'Bomber_1': '1e6B82mbzXpWajFy3ah7tbGNCHgfoZ_ab',
    'Bomber_2': '13a7_ZhrlRHhKjVweKEPJRUSvNU6qFkEL',
    'Cannon_1': '1oYQJAf0a_0OJ0kYuA04geD7P82envJdB',
    'Cannon_2': '1nHFksyg560HsXXz4vXRBaEFb-0AUzAGu',
    'Cannon_3': '1sy3l9vK_qFmvkAs0p9TfT9sRIHLzkg-B',
    'Cannon_4': '1Iolky3_DV2fEazPGad4UtE8Pwvfk92GJ',
    'Cannon_5': '12M76yHwtw_-PB87ZBs42-JjbgCpAW1gH',
    'Cannon_6': '1A4GeRg5WQHOTmZLuirju7Ix70Dxfuo9X',
    'Cannon_7': '1fMH0NKfJRof13BZN4XultEa8JjfK110v',
    'GearCannon_1': '1IJJRZvzRxc5PR52uqJWRXp3vaYl0X17_',
    'GearCannon_2': '1V95USK7R3s3KvO61qQdxQZNar6b55GcV',
    'Gun': '1wYilSB_2Ig1xZ4rNoUttVbPfVHB8OGjr',
    'Gun_2': '1O_Ro3s2YvnNj8Fwaehwk_DjeXeUs7YGe',
    'Gun_3': '1E3KJx_M-05J5fmv0wTAhuoBPttyXpZuv',
    'Gun_4': '16sSIPQiPrxld1aRzjZfCneslbhJrxzFE',
    'Gun_5': '1kSh4MyHh9nu2gebn3w7Rj8IC1Lk9prVf',
    'Gun_6': '1URRMz2I6_22uS6qFrb8uNGZ2_7sZo5wO',
    'Gun_7': '1raKpc3OTgdJCfuxvQDjWb1eR69X3D_zh',
    'Gun_8': '15Ri-oDTe-ks9a7PU3VHkBXsA3ivJI-1x',
    'Gun_9': '1SAEoWmtQUFSETE_Y0a2fG88u3Ud6lVqq',
    'Gun_10': '1Sp17zRi-AjQczIO12XIGDeuOYgdQlwB3',
    'Laser_1': '1NhqQZ-lmlpNF67Mp8v-ZSDkIvjJURnKz',
    'Laser_2': '12n11qz80lzsL7jInatJ8blCEVICF871v',
    'Laser_Doble': '15kW8U07HtmQUxhY-HTozdg-lUcHIIrdf',
    'Laser_MachineGun': '1BZkYuAbar4AZAYPf2_9gxWsJ2MItTc4s',
    'Laser_Quad': '1Pl1xBBSf_1oOvxrSIrU1DEe2G5fgv-bz',
    'Long_1': '1XYMqW-TTpDkw0mPZTDJ1t48onS8eLkk7',
    'Long_2': '1cNyOkHC9WmkTb0PlmReXjuAIKGiEHCth',
    'Teleporter1': '1gMVjbAF1IJGh3CrhbrZg2BO3wkn0rw-s',
    'Teleporter2': '1LGB0B4NHoqca_LaXZB7WUsEy_x6r-Sis',
    'Teleporter3': '1-hNoT2ypjcnroKtyP42trqdNw0J4-Hxq',
    'Teleporter4': '11DZkHJLTCwnnZS5qAVAJPTAhhcB-6Lvp',
    'Teleporter5': '1kSH7_96-2OGJMGJwTevJZgM0lXcKK8s-',
    'Upwards_1': '19NP11HfUQan9LLCXgEQcP0PQtYP8Y09U',
    'Upwards_2': '16vcuL_R_fVS8YC1Et50VFE3qkyfIp6PX',
    'Upwards_3_': '171p6f65P9PCrIFzSXaOmVmlVWkmQOAKm',
}

# A small, deliberately selected subset of Kenney's Nature Kit. More variety
# matters than packing all 330 models into the game, and these silhouettes cover
# the jobs visible from the tactical camera without turning the board into a
# prop catalogue.
NATURE_MODELS = {
    'NatureOak': 'tree_oak',
    'NatureBroadleaf': 'tree_detailed',
    'NaturePineA': 'tree_pineTallA_detailed',
    'NaturePineB': 'tree_pineTallC_detailed',
    'NatureRockA': 'rock_largeA',
    'NatureRockB': 'rock_largeC',
    'NatureBush': 'plant_bushDetailed',
    'NatureStump': 'stump_old',
    'NatureGrass': 'grass_large',
    'NatureFlower': 'flower_yellowA',
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


def ensure_nature():
    """Return the downloaded Nature Kit GLB directory, fetching it once."""
    models = os.path.join(NATURE_CACHE, 'Models', 'GLTF format')
    if os.path.exists(os.path.join(models, 'tree_oak.glb')):
        return models
    if not os.path.exists(NATURE_ZIP) or os.path.getsize(NATURE_ZIP) == 0:
        subprocess.run(['curl', '-sL', '--max-time', '90', '-o', NATURE_ZIP,
                        NATURE_URL], check=True)
    os.makedirs(NATURE_CACHE, exist_ok=True)
    with zipfile.ZipFile(NATURE_ZIP) as archive:
        archive.extractall(NATURE_CACHE)
    if not os.path.exists(os.path.join(models, 'tree_oak.glb')):
        raise SystemExit('Kenney Nature Kit extracted without its GLB models')
    return models


def ensure_turret(stem):
    """Fetch one selected OBJ from the official Quaternius turret pack."""
    os.makedirs(TURRET_CACHE, exist_ok=True)
    path = os.path.join(TURRET_CACHE, stem + '.obj')
    if os.path.exists(path) and os.path.getsize(path) > 100:
        return path
    file_id = TURRET_FILES[stem]
    url = ('https://drive.usercontent.google.com/download?id=%s&export=download&confirm=t'
           % file_id)
    subprocess.run(['curl', '-fL', '--max-time', '90', '-o', path, url], check=True)
    with open(path, 'rb') as f:
        if not f.read(32).lstrip().startswith(b'# Blender'):
            raise SystemExit('Quaternius download was not OBJ: ' + stem)
    return path


def turret_colour(name, material):
    """Turn the pack's two copper bands into gunmetal plus family paint."""
    accent = TOWER_ACCENTS[name.rstrip('0123456789')]
    if material.lower() == 'dark':
        base, amount = (0.075, 0.082, 0.090), 0.23
    else:
        base, amount = (0.28, 0.30, 0.32), 0.58
    return tuple(base[i] * (1.0 - amount) + accent[i] * amount for i in range(3))


def obj_triangles(path, name):
    """Read the small, dependency-free OBJ subset used by the turret pack."""
    positions = [None]
    normals = [None]
    material = 'Light'
    out = []

    def index(raw, count):
        value = int(raw)
        return value if value > 0 else count + value

    with open(path, encoding='utf-8') as source:
        for line in source:
            fields = line.split()
            if not fields:
                continue
            if fields[0] == 'v':
                positions.append(tuple(float(x) for x in fields[1:4]))
            elif fields[0] == 'vn':
                normals.append(tuple(float(x) for x in fields[1:4]))
            elif fields[0] == 'usemtl' and len(fields) > 1:
                material = fields[1]
            elif fields[0] == 'f':
                polygon = []
                for token in fields[1:]:
                    parts = token.split('/')
                    vi = index(parts[0], len(positions))
                    ni = index(parts[2], len(normals)) if len(parts) > 2 and parts[2] else 0
                    polygon.append((positions[vi], normals[ni] if ni else None))
                # Blender writes quads as well as triangles. Fan triangulation
                # preserves their original winding and flat normals.
                for i in range(1, len(polygon) - 1):
                    tri = (polygon[0], polygon[i], polygon[i + 1])
                    face_normal = None
                    if any(n is None for _p, n in tri):
                        a, b, c = (v[0] for v in tri)
                        u = tuple(b[j] - a[j] for j in range(3))
                        v = tuple(c[j] - a[j] for j in range(3))
                        face_normal = (u[1] * v[2] - u[2] * v[1],
                                       u[2] * v[0] - u[0] * v[2],
                                       u[0] * v[1] - u[1] * v[0])
                        length = max(sum(x * x for x in face_normal) ** 0.5, 1e-8)
                        face_normal = tuple(x / length for x in face_normal)
                    colour = turret_colour(name, material)
                    out.extend((p, n or face_normal, colour) for p, n in tri)
    return out


def bake_turret(name, stem):
    """Bake a real weapon silhouette around its authored rotating base."""
    tris = obj_triangles(ensure_turret(stem), name)
    if not tris:
        raise SystemExit(stem + ' baked to nothing')
    # Quaternius authors every turret around source (X,Z)=(0,0). Preserving
    # that pivot stops asymmetric barrels orbiting by almost half a tile when
    # they track. After conversion those two ground axes are still (0,0).
    return bake_triangles(tris, tris, pivot_xy=(0.0, 0.0))


def nature_colour(name, c):
    """Move the kit's turquoise/orange atlas into moss, bark and slate."""
    teal = c[1] > c[0] * 1.7 and c[2] > c[0] * 1.6
    orange = c[0] > c[1] * 1.25 and c[0] > c[2] * 1.7
    pale = min(c) > 0.82
    if 'Rock' in name:
        if teal:  # the authored turf/moss cap
            return (0.105, 0.285, 0.125)
        if orange:
            return (0.225, 0.235, 0.265)
    if name == 'NatureFlower' and orange:
        return (1.0, 0.48, 0.055)
    if teal:
        value = max(c)
        return (value * 0.13, value * 0.46, value * 0.17)
    if orange:
        value = max(c)
        return (value * 0.28, value * 0.16, value * 0.075)
    if pale and 'tree' in name.lower():
        return (0.15, 0.38, 0.16)
    return c


def bake_nature(name, stem):
    """Bake one free Nature Kit prop, preserving its authored value bands."""
    path = os.path.join(ensure_nature(), stem + '.glb')
    tris = []
    for p, n, c in gltf.triangles(path, anim=None):
        tris.append((p, n, nature_colour(name, c)))
    return bake_triangles(tris, tris)


MAX_FOOTPRINT_TO_HEIGHT = 1.35


def bake_one(path, anim, at):
    """Two poses, Z-up, grounded and normalised to a tactical footprint.

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

    return bake_triangles(tris, other)


def bake_triangles(tris, other, pivot_xy=None):
    """Normalise two corresponding triangle soups into the runtime axes."""

    # glTF is Y-up and right-handed; the game is Z-up. Sending -z to y keeps the
    # handedness, so faces still wind the way the renderer expects and nothing
    # comes out inside-out.
    def conv(v):
        return (v[0], -v[2], v[1])

    pos = [conv(p) for p, _n, _c in tris]
    nrm = [conv(n) for _p, n, _c in tris]
    col = [c for _p, _n, c in tris]

    pos_b = [conv(p) for p, _n, _c in other]
    nrm_b = [conv(n) for _p, n, _c in other]

    # Fit both contact poses through one transform. Besides preventing the
    # animation from breathing, this keeps a rotor, wingtip or weapon swing
    # inside the same gameplay footprint throughout the stride.
    envelope = pos + pos_b
    lo = [min(p[i] for p in envelope) for i in range(3)]
    hi = [max(p[i] for p in envelope) for i in range(3)]
    height = max(hi[2] - lo[2], 1e-4)
    footprint = max(hi[0] - lo[0], hi[1] - lo[1], 1e-4)
    k = min(1.0 / height, MAX_FOOTPRINT_TO_HEIGHT / footprint)
    if pivot_xy is None:
        cx = (lo[0] + hi[0]) * 0.5
        cy = (lo[1] + hi[1]) * 0.5
    else:
        cx, cy = pivot_xy

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

    for name, stem in TURRET_MODELS.items():
        if only and name.lower() not in only:
            continue
        verts, delta = bake_turret(name, stem)
        names.append(name)
        blobs.append((verts, delta))
        print('  %-12s %6d tris  Quaternius Steampunk Turret Pack'
              % (name, len(verts) // 3))

    for name, stem in NATURE_MODELS.items():
        if only and name.lower() not in only:
            continue
        verts, delta = bake_nature(name, stem)
        names.append(name)
        blobs.append((verts, delta))
        print('  %-16s %6d tris  Kenney Nature Kit'
              % (name, len(verts) // 3))

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
        f.write('\n## Tower and environment models\n\n')
        f.write('Forty-four staged weapon towers are selected from\n')
        f.write("Quaternius' [Steampunk Turret Pack](%s), " % TURRET_SOURCE)
        f.write('released under CC0 1.0. Trees, rocks, brush and ground details are selected\n')
        f.write("from Kenney's 330-model [Nature Kit](https://kenney.nl/assets/nature-kit), ")
        f.write('also released under CC0 1.0. Both source sets are downloaded and cached by\n')
        f.write('`tools/bake_models.py`; only the baked runtime mesh is shipped.\n')
        f.write('\n## Interface artwork\n\n')
        f.write('`title_backdrop.png` was created for this project with OpenAI\'s built-in image\n')
        f.write('generation on 2026-09-04. It contains no third-party logo or game UI.\n\n')
        f.write('`tower_icons.png` is an in-engine 24x4 contact sheet of the exact staged Quaternius\n')
        f.write('weapon towers used on the battlefield. It contains no separate concept art;\n')
        f.write('the source and runtime assets are produced by the ignored roster render test\n')
        f.write('and `tools/bake_icons.py`.\n')


if __name__ == '__main__':
    main([a.lower() for a in sys.argv[1:]])
