"""Parser for Warcraft III object-data files (.w3u units, .w3a abilities, ...).

Format, all little-endian:

    int32  version (2 or 3)
    two tables follow, "original" then "custom", each:
        int32 count
        for each object:
            char[4] base id
            char[4] new id           (zero in the original table)
            int32   set count        (version 3 only)
            for each set:
                int32 id count + that many char[4]   (version 3 only)
                int32 mod count
                for each mod:
                    char[4] field id
                    int32   value type   0=int 1=real 2=unreal 3=string
                    int32   level, int32 data   (only in levelled files: w3a/w3q/w3d)
                    <value>
                    int32   end marker
"""

import struct


class Cur:
    def __init__(self, buf):
        self.b = buf
        self.o = 0

    def i32(self):
        v = struct.unpack_from('<i', self.b, self.o)[0]
        self.o += 4
        return v

    def f32(self):
        v = struct.unpack_from('<f', self.b, self.o)[0]
        self.o += 4
        return v

    def tag(self):
        v = self.b[self.o:self.o + 4].decode('latin-1')
        self.o += 4
        return v

    def cstr(self):
        end = self.b.index(b'\0', self.o)
        v = self.b[self.o:end].decode('latin-1')
        self.o = end + 1
        return v

    def done(self):
        return self.o >= len(self.b)


def parse(data, levelled):
    """Returns {id: {'base': base_id, 'mods': {field: [(level, value), ...]}}}."""
    c = Cur(data)
    version = c.i32()
    out = {}
    for _table in range(2):
        count = c.i32()
        for _ in range(count):
            base = c.tag()
            new = c.tag()
            key = new if new.strip('\0') else base
            entry = out.setdefault(key, {'base': base, 'mods': {}})
            sets = c.i32() if version >= 3 else 1
            for _ in range(sets):
                if version >= 3:
                    n_ids = c.i32()
                    c.o += 4 * n_ids
                for _ in range(c.i32()):
                    field = c.tag()
                    vtype = c.i32()
                    level = 0
                    if levelled:
                        level = c.i32()
                        c.i32()  # data pointer
                    if vtype == 0:
                        value = c.i32()
                    elif vtype in (1, 2):
                        value = c.f32()
                    elif vtype == 3:
                        value = c.cstr()
                    else:
                        raise ValueError('bad value type %d at %d' % (vtype, c.o))
                    c.i32()  # end marker
                    entry['mods'].setdefault(field, []).append((level, value))
    return out, version


def one(entry, field, default=None):
    v = entry['mods'].get(field)
    return v[0][1] if v else default
