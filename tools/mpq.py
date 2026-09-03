"""A minimal MPQ reader that handles the encryption WC3 maps actually use.

mpyq stops at "Encryption is not supported yet", which is every file in a
Warcraft III map. The algorithm is small and well documented, so here it is.
"""

import struct
import zlib

M32 = 0xFFFFFFFF


def _build_crypt_table():
    table = [0] * 0x500
    seed = 0x00100001
    for i in range(0x100):
        index = i
        for _ in range(5):
            seed = (seed * 125 + 3) % 0x2AAAAB
            t1 = (seed & 0xFFFF) << 16
            seed = (seed * 125 + 3) % 0x2AAAAB
            t2 = seed & 0xFFFF
            table[index] = (t1 | t2) & M32
            index += 0x100
    return table


CRYPT = _build_crypt_table()

HASH_TABLE_OFFSET = 0
HASH_NAME_A = 1
HASH_NAME_B = 2
HASH_FILE_KEY = 3


def mpq_hash(name, kind):
    seed1 = 0x7FED7FED
    seed2 = 0xEEEEEEEE
    for ch in name.upper().replace('/', '\\'):
        c = ord(ch)
        seed1 = (CRYPT[(kind << 8) + c] ^ (seed1 + seed2)) & M32
        seed2 = (c + seed1 + seed2 + (seed2 << 5) + 3) & M32
    return seed1


def decrypt(data, key):
    out = bytearray(len(data))
    seed = 0xEEEEEEEE
    n = len(data) // 4
    words = list(struct.unpack('<%dI' % n, bytes(data[: n * 4])))
    for i in range(n):
        seed = (seed + CRYPT[0x400 + (key & 0xFF)]) & M32
        ch = words[i] ^ ((key + seed) & M32)
        key = (((~key << 0x15) & M32) + 0x11111111 | (key >> 0x0B)) & M32
        seed = (ch + seed + (seed << 5) + 3) & M32
        words[i] = ch
    struct.pack_into('<%dI' % n, out, 0, *words)
    if len(data) % 4:
        out[n * 4:] = data[n * 4:]
    return bytes(out)


def decompress(block):
    """One compressed sector: a mask byte then the payload."""
    mask = block[0]
    body = block[1:]
    if mask == 0:
        return body
    if mask & 0x02:  # zlib
        return zlib.decompress(body)
    if mask & 0x10:  # bzip2
        import bz2

        return bz2.decompress(body)
    if mask & 0x08:  # PKWARE implode
        return explode(body)
    raise NotImplementedError('compression mask 0x%02x' % mask)


# ------------------------------------------------------------------ PKWARE
# The "implode" format from PKWARE Data Compression Library. WC3 maps use it
# for most of their smaller files.

_DIST_BITS = [
    2, 4, 4, 5, 5, 5, 5, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6,
    6, 6, 6, 6, 6, 6, 6, 6, 6, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7,
    7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7,
]
_DIST_CODE = [
    0x03, 0x0D, 0x05, 0x19, 0x09, 0x11, 0x01, 0x3E, 0x1E, 0x2E, 0x0E, 0x36,
    0x16, 0x26, 0x06, 0x3A, 0x1A, 0x2A, 0x0A, 0x32, 0x12, 0x22, 0x02, 0x7C,
    0x3C, 0x5C, 0x1C, 0x6C, 0x2C, 0x4C, 0x0C, 0x74, 0x34, 0x54, 0x14, 0x64,
    0x24, 0x44, 0x04, 0x78, 0x38, 0x58, 0x18, 0x68, 0x28, 0x48, 0x08, 0x70,
    0x30, 0x50, 0x10, 0x60, 0x20, 0x40, 0x00,
]
_LEN_BITS = [3, 2, 3, 3, 4, 4, 4, 5, 5, 5, 5, 6, 6, 6, 7, 7]
_LEN_CODE = [0x05, 0x03, 0x01, 0x06, 0x0A, 0x02, 0x0C, 0x14,
             0x04, 0x18, 0x08, 0x30, 0x10, 0x20, 0x40, 0x00]
_LEN_BASE = [0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07,
             0x08, 0x0A, 0x0E, 0x16, 0x26, 0x46, 0x86, 0x106]
_EXTRA_LEN_BITS = [0, 0, 0, 0, 0, 0, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8]


class _Bits:
    def __init__(self, data):
        self.d = data
        self.pos = 0
        self.bit = 0

    def get(self, n):
        v = 0
        for i in range(n):
            if self.pos >= len(self.d):
                raise EOFError
            b = (self.d[self.pos] >> self.bit) & 1
            v |= b << i
            self.bit += 1
            if self.bit == 8:
                self.bit = 0
                self.pos += 1
        return v


def explode(data):
    bits = _Bits(data)
    lit_mode = bits.get(8)      # 0 = binary, 1 = ascii
    dict_bits = bits.get(8)     # 4, 5 or 6
    if lit_mode not in (0, 1) or dict_bits not in (4, 5, 6):
        raise ValueError('bad implode header %d %d' % (lit_mode, dict_bits))
    if lit_mode == 1:
        raise NotImplementedError('ascii-mode implode')

    out = bytearray()
    while True:
        try:
            if bits.get(1) == 0:
                out.append(bits.get(8))
                continue
            # a length/distance pair
            code = -1
            val = 0
            nb = 0
            while code < 0:
                val |= bits.get(1) << nb
                nb += 1
                if nb > 8:
                    raise ValueError('bad length code')
                for i, (b, c) in enumerate(zip(_LEN_BITS, _LEN_CODE)):
                    if b == nb and c == val:
                        code = i
                        break
            length = _LEN_BASE[code] + 2
            extra = _EXTRA_LEN_BITS[code]
            if extra:
                length += bits.get(extra)
            if length == 0x208:
                break
            dcode = -1
            val = 0
            nb = 0
            while dcode < 0:
                val |= bits.get(1) << nb
                nb += 1
                if nb > 8:
                    raise ValueError('bad distance code')
                for i, (b, c) in enumerate(zip(_DIST_BITS, _DIST_CODE)):
                    if b == nb and c == val:
                        dcode = i
                        break
            if length == 2:
                dist = (dcode << 2) + bits.get(2) + 1
            else:
                dist = (dcode << dict_bits) + bits.get(dict_bits) + 1
            start = len(out) - dist
            if start < 0:
                raise ValueError('back-reference before start')
            for i in range(length):
                out.append(out[start + i])
        except EOFError:
            break
    return bytes(out)


class Archive:
    def __init__(self, path, offset=None):
        self.raw = open(path, 'rb').read()
        if offset is None:
            offset = self.raw.find(b'MPQ\x1a')
        self.base = offset
        d = self.raw
        o = offset
        magic, header_size, archive_size, fmt, block_size, hash_pos, block_pos, hash_count, block_count = struct.unpack_from(
            '<4sIIHHIIII', d, o
        )
        assert magic == b'MPQ\x1a', magic
        self.sector_size = 512 << block_size
        ht = decrypt(d[o + hash_pos: o + hash_pos + hash_count * 16], mpq_hash('(hash table)', HASH_FILE_KEY))
        bt = decrypt(d[o + block_pos: o + block_pos + block_count * 16], mpq_hash('(block table)', HASH_FILE_KEY))
        self.hash_table = [struct.unpack_from('<IIHHI', ht, i * 16) for i in range(hash_count)]
        self.block_table = [struct.unpack_from('<IIII', bt, i * 16) for i in range(block_count)]

    def _find(self, name):
        n = len(self.hash_table)
        start = mpq_hash(name, HASH_TABLE_OFFSET) % n
        a = mpq_hash(name, HASH_NAME_A)
        b = mpq_hash(name, HASH_NAME_B)
        for i in range(n):
            e = self.hash_table[(start + i) % n]
            if e[4] == 0xFFFFFFFF:
                return None
            if e[0] == a and e[1] == b and e[4] != 0xFFFFFFFE:
                return e[4]
        return None

    def read(self, name):
        idx = self._find(name)
        if idx is None:
            return None
        offset, packed, size, flags = self.block_table[idx]
        if not flags & 0x80000000:
            return None
        pos = self.base + offset
        data = self.raw[pos: pos + packed]

        key = None
        if flags & 0x00010000:
            base = name.replace('/', '\\').rsplit('\\', 1)[-1]
            key = mpq_hash(base, HASH_FILE_KEY)
            if flags & 0x00020000:
                key = ((key + offset) ^ size) & M32

        if flags & 0x01000000:  # single unit
            if key is not None:
                data = decrypt(data, key)
            if flags & 0x00000200 and packed < size:
                return decompress(data)
            return data[:size]

        nsectors = (size + self.sector_size - 1) // self.sector_size
        ntable = (nsectors + 1) * 4
        table = data[:ntable]
        if key is not None:
            table = decrypt(table, (key - 1) & M32)
        offsets = struct.unpack('<%dI' % (nsectors + 1), table)

        out = bytearray()
        for i in range(nsectors):
            chunk = data[offsets[i]: offsets[i + 1]]
            if key is not None:
                chunk = decrypt(chunk, (key + i) & M32)
            want = min(self.sector_size, size - len(out))
            if flags & 0x00000200 and len(chunk) < want:
                chunk = decompress(chunk)
            out += chunk[:want]
        return bytes(out[:size])
