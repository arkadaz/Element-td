"""Pulls the files emit.py needs out of the map archive.

Everything in a Warcraft III `.w3x` is encrypted, so this goes through the
hand-written reader in `mpq.py` rather than a library. Run it once before
`emit.py`; the extracted files are not checked in.
"""

import os
import sys

import mpq

BS = chr(92)

MAP = sys.argv[1] if len(sys.argv) > 1 else '../GREEN TD 9.3c PEIN.w3x'
OUT = 'greentd'

WANTED = [
    'war3map.w3u',            # units: the towers and the creeps
    'war3map.w3a',            # abilities: crit, multishot, roots, auras
    'war3map.w3e',            # terrain: 96x96 tiles, heights and textures
    'war3map.w3i',            # map info: name, bounds, players
    'war3map.doo',            # doodads: trees, rocks, torches
    'war3map.w3t',            # items
    'war3map.wts',            # the string table the object files point into
    # The gameplay constants, and with them the rewritten attack-versus-armour
    # table. `greentd_types::type_mult` is hand-written from those six lines, so
    # a map version that changed them would silently disagree with the code -
    # having the file on disk is what makes that checkable.
    'war3mapMisc.txt',
    'war3map.j',              # the trigger script - wave order, gold, lives
    'Scripts' + BS + 'war3map.j',
]

os.makedirs(OUT, exist_ok=True)
a = mpq.Archive(MAP)
for name in WANTED:
    try:
        data = a.read(name)
    except Exception as exc:
        print('  skip %-24s %s' % (name, exc))
        continue
    if data is None:
        print('  skip %-24s not in archive' % name)
        continue
    base = name.replace(BS, '/').rsplit('/', 1)[-1]
    # The trigger script lives at Scripts/war3map.j; emit.py reads it as
    # script.j so the two war3map.j entries do not collide.
    if base == 'war3map.j':
        base = 'script.j'
    with open(os.path.join(OUT, base), 'wb') as f:
        f.write(data)
    print('  %-24s %8d bytes' % (base, len(data)))
