"""Bake the authored title art into the raw RGB blob egui consumes.

    python bake_title.py

The source PNG remains in `assets/` as the editable master. The game ships the
smaller, fixed-size RGB blob so runtime and web builds need no image decoder.
"""

import os
import struct

from PIL import Image, ImageOps


HERE = os.path.dirname(os.path.abspath(__file__))
ASSETS = os.path.abspath(os.path.join(HERE, '..', 'assets'))
SOURCE = os.path.join(ASSETS, 'title_backdrop.png')
DEST = os.path.join(ASSETS, 'title_backdrop.bin')
SIZE = (1280, 720)
MAGIC = 0x47425447  # "GTBG"


def main():
    image = Image.open(SOURCE).convert('RGB')
    image = ImageOps.fit(image, SIZE, Image.Resampling.LANCZOS)
    body = image.tobytes()
    with open(DEST, 'wb') as out:
        out.write(struct.pack('<IIII', MAGIC, 1, SIZE[0], SIZE[1]))
        out.write(body)
    print('wrote %s  %dx%d  %.1f KB' % (DEST, SIZE[0], SIZE[1],
                                         (16 + len(body)) / 1024.0))


if __name__ == '__main__':
    main()
