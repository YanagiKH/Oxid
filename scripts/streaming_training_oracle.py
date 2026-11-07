"""Independent wire fixtures and analytic SGD for sequential training checks."""
import math
import re
import struct

SAMPLE = [(-1., -1., -.5), (-1., 1., -1.5), (0., -1., 1.5), (0., 1., .5),
          (1., -1., 3.5), (1., 1., 2.5), (.5, -.5, 2.25), (-.5, .5, -.25)]


def encode(rows):
    return b'OXDS0001'+struct.pack('<IIII', len(rows), 2, 1, 0)+b''.join(struct.pack('<ddd', *row) for row in rows)


def grid(count, features=2, targets=1):
    header = b'OXDS0001'+struct.pack('<IIII', count, features, targets, 0)
    values = [value for i in range(count) for value in
              [float(i)+.5*column for column in range(features)]+[-2.*i-column for column in range(targets)]]
    return header+struct.pack('<'+str(len(values))+'d', *values)


def reader_cases():
    base = grid(5)
    cases = [('five', base, 0, 5, 2), ('single', grid(1), 0, 1, 128),
             ('single-max-size', grid(1), 0, 1, 2147483647), ('wide-layout', grid(7, 3, 2), 0, 7, 3),
             ('cross-buffer', grid(257), 0, 257, 17), ('larger-than-heap', grid(4099), 0, 4099, 17),
             ('empty', b'', 2, 0, 2), ('short-header', base[:13], 2, 0, 2),
             ('magic', b'BADMAGIC'+base[8:], 1, 0, 2),
             ('short-first', base[:31], 2, 0, 2), ('short-last', base[:-1], 2, 0, 2),
             ('extra-byte', base+b'x', 3, 0, 2), ('invalid-size', base, 4, 0, 0),
             ('missing', None, 1002, 0, 2),
             ('batch-memory', grid(4099), 8, 0, 1024),
             ('shape-overflow', b'OXDS0001'+struct.pack('<IIII', 2147483647, 2, 1, 0), 10, 0, 2147483647)]
    for label, offset, word in [('zero-rows', 8, 0), ('rows-overflow', 8, 2147483648),
                                ('zero-features', 12, 0), ('zero-targets', 16, 0),
                                ('wide-features', 12, 268435456), ('wide-targets', 16, 268435456),
                                ('reserved', 20, 1)]:
        changed = bytearray(base)
        struct.pack_into('<I', changed, offset, word)
        cases.append((label, bytes(changed), 1, 0, 2))
    changed = bytearray(base)
    struct.pack_into('<II', changed, 12, 268435455, 1)
    cases.append(('sum-overflow', bytes(changed), 1, 0, 2))
    for label, word in [('quiet-nan', 0x7ff8000000000042), ('signaling-nan', 0x7ff0000000000042),
                        ('positive-infinity', 0x7ff0000000000000), ('negative-infinity', 0xfff0000000000000)]:
        changed = bytearray(base)
        struct.pack_into('<Q', changed, 24+4*24+8, word)
        cases.append((label, bytes(changed), 9, 0, 2))
    return cases


def train(rows, epochs, size):
    state = 42

    def draw():
        nonlocal state
        state ^= (state << 13) & 0xffffffff
        state ^= state >> 17
        state ^= (state << 5) & 0xffffffff
        state &= 0xffffffff
        return (state >> 8)/16777216.*2.-1.

    first = [[draw()*math.sqrt(3.) for _ in range(8)] for _ in range(2)]
    bias = [0.]*8
    second = [draw()*math.sqrt(.75) for _ in range(8)]
    output_bias = 0.

    def forward(row):
        hidden = [max(0., sum(row[i]*first[i][h] for i in range(2))+bias[h]) for h in range(8)]
        return hidden, sum(hidden[h]*second[h] for h in range(8))+output_bias

    def loss():
        return sum((forward(row)[1]-row[2])**2 for row in rows)/len(rows)

    initial = loss()
    for _ in range(epochs):
        for start in range(0, len(rows), size):
            batch = rows[start:start+size]
            g1, gb1, g2, gb2 = [[0.]*8 for _ in range(2)], [0.]*8, [0.]*8, 0.
            for row in batch:
                hidden, prediction = forward(row)
                gradient = 2*(prediction-row[2])/len(batch)
                gb2 += gradient
                for h in range(8):
                    g2[h] += hidden[h]*gradient
                    back = gradient*second[h] if hidden[h] > 0 else 0.
                    gb1[h] += back
                    for i in range(2):
                        g1[i][h] += row[i]*back
            for h in range(8):
                second[h] -= .05*g2[h]
                bias[h] -= .05*gb1[h]
                for i in range(2):
                    first[i][h] -= .05*g1[i][h]
            output_bias -= .05*gb2
    return initial, loss()


def check_summary(body, rows, epochs, size):
    found = re.fullmatch(rb'Rows: (\d+)\nEpochs: (\d+)\nBatches: (\d+)\nInitial MSE: ([^\n]+)\nFinal MSE: ([^\n]+)\n', body)
    counters = (len(rows), epochs, epochs*((len(rows)-1)//size+1))
    if not found or tuple(map(int, found.groups()[:3])) != counters:
        raise ValueError('training counters or summary differ')
    expected = train(rows, epochs, size)
    if not all(math.isclose(float(got), want, rel_tol=8e-6, abs_tol=1e-12)
               for got, want in zip(found.groups()[3:], expected)):
        raise ValueError('training differs from independent analytic SGD')
    return expected
