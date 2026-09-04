"""Bake the in-engine command-card atlas into egui's raw RGB blob.

    python bake_icons.py

The 24x4 source atlas remains in ``assets/tower_icons.png``. Each 64px cell is
a headless render of the same staged downloaded assembly used on the board, so
the command card is a faithful preview rather than separate concept art.
"""

import os
import struct

from PIL import Image, ImageOps


HERE = os.path.dirname(os.path.abspath(__file__))
ASSETS = os.path.abspath(os.path.join(HERE, '..', 'assets'))
SOURCE = os.path.join(ASSETS, 'tower_icons.png')
DEST = os.path.join(ASSETS, 'tower_icons.bin')
SIZE = (1536, 256)
MAGIC = 0x41495447  # "GTIA"


def main():
    image = Image.open(SOURCE).convert('RGB')
    image = ImageOps.fit(image, SIZE, Image.Resampling.LANCZOS)
    # The renderer's deliberately simple PNG writer is uncompressed. Keep the
    # reviewable source but normalise it to a compact production PNG here.
    image.save(SOURCE, optimize=True)
    body = image.tobytes()
    with open(DEST, 'wb') as out:
        out.write(struct.pack('<IIII', MAGIC, 1, SIZE[0], SIZE[1]))
        out.write(body)
    print('wrote %s  %dx%d  %.1f KB' % (DEST, SIZE[0], SIZE[1],
                                         (16 + len(body)) / 1024.0))


if __name__ == '__main__':
    main()
