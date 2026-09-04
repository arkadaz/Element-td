"""Download the CC0 ground textures and bake them into one raw blob.

    python bake_textures.py

Writes `assets/textures.bin`: a small array of square RGBA layers, raw, with no
container. Raw because the game would otherwise need a PNG or JPEG decoder in
Rust to read them, and a decoder is a dependency and a failure mode for
something that is decided entirely at bake time. The blob is what the GPU wants
anyway.

Sources are ambientCG, whose whole library is CC0. Their API is public and the
zips are direct downloads, so this is reproducible without an account.
"""

import io
import os
import struct
import subprocess
import zipfile

from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))
CACHE = os.path.join(HERE, 'tex')
OUT = os.path.abspath(os.path.join(HERE, '..', 'assets'))

# Every layer is this square. 256 is small for a close-up and exactly right for
# what this is: ground seen from a camera twenty-six tiles up, tiled once per
# tile, where the job of the texture is to break up a flat fill rather than to
# survive being looked at.
SIZE = 256

# The layer order is the contract with `view::mod`, which picks a layer per
# tile. Adding one means adding it here and using it there; nothing else.
LAYERS = [
    ('grass', 'Grass004'),
    ('dirt', 'Ground054'),
    # The HUD's own materials. They live in the same blob as the ground because
    # they are the same thing - a normalised albedo tinted by a colour the game
    # already chose - and one blob is one loader, one failure mode and one set
    # of credits.
    ('wood', 'Planks037A'),
    ('stone', 'Rock051'),
]

MAGIC = 0x58455447  # "GTEX"


def fetch(asset):
    os.makedirs(CACHE, exist_ok=True)
    zpath = os.path.join(CACHE, '%s.zip' % asset)
    if not os.path.exists(zpath) or os.path.getsize(zpath) == 0:
        url = 'https://ambientcg.com/get?file=%s_1K-JPG.zip' % asset
        subprocess.run(['curl', '-sL', '--max-time', '180', '-o', zpath, url],
                       check=True)
    return zpath


def colour_layer(asset):
    """The albedo map, square, RGBA, normalised to a linear mean of one half.

    Normalising is the whole trick. A photographic albedo carries the material's
    *brightness* as well as its grain, and the game already has a brightness for
    each surface - a palette measured against a Warcraft III screenshot. Sampling
    the raw texture threw that away: the dirt corridors came out pale grey
    because ambientCG's ground albedo averages rgb(153, 147, 88) and the map's
    trodden earth is far darker than that.

    So each layer is rescaled until its linear average is 0.5. The shader then
    multiplies by twice the sample, which averages to exactly one and leaves the
    measured colour intact - the texture supplies variation and nothing else.
    """
    z = zipfile.ZipFile(fetch(asset))
    name = next(n for n in z.namelist()
                if n.lower().endswith('_color.jpg'))
    im = Image.open(io.BytesIO(z.read(name))).convert('RGB')
    im = im.resize((SIZE, SIZE), Image.LANCZOS)

    px = list(im.getdata())
    n = float(len(px))

    def to_lin(v):
        v /= 255.0
        return v / 12.92 if v <= 0.04045 else ((v + 0.055) / 1.055) ** 2.4

    def to_srgb(v):
        v = max(0.0, min(1.0, v))
        v = v * 12.92 if v <= 0.0031308 else 1.055 * (v ** (1 / 2.4)) - 0.055
        return int(round(v * 255.0))

    lin = [tuple(to_lin(c) for c in p) for p in px]
    # One scale for all three channels, so normalising cannot shift the hue.
    mean = sum((p[0] + p[1] + p[2]) / 3.0 for p in lin) / n
    k = 0.5 / max(mean, 1e-4)
    im.putdata([tuple(to_srgb(c * k) for c in p) for p in lin])
    return im


def main():
    os.makedirs(OUT, exist_ok=True)
    body = bytearray()
    names = []
    for name, asset in LAYERS:
        im = colour_layer(asset)
        body += im.convert('RGBA').tobytes()
        names.append((name, asset))
        px = im.resize((1, 1), Image.LANCZOS).getpixel((0, 0))
        print('  %-7s %-12s average rgb%s' % (name, asset, px))

    head = struct.pack('<IIII', MAGIC, 1, SIZE, len(LAYERS))
    dst = os.path.join(OUT, 'textures.bin')
    open(dst, 'wb').write(head + bytes(body))
    print('\nwrote %s  %d layers of %dx%d  %.1f KB'
          % (dst, len(LAYERS), SIZE, SIZE, (len(head) + len(body)) / 1024.0))

    creds = os.path.join(OUT, 'CREDITS.md')
    with open(creds, 'a', newline='\n') as f:
        f.write('\n## Ground textures\n\n')
        f.write('CC0, from [ambientCG](https://ambientcg.com/), baked by\n')
        f.write('`tools/bake_textures.py`.\n\n')
        f.write('| Layer | Material | Source |\n| --- | --- | --- |\n')
        for name, asset in names:
            f.write('| %s | %s | https://ambientcg.com/view?id=%s |\n'
                    % (name, asset, asset))


if __name__ == '__main__':
    main()
