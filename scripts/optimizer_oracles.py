"""Independent optimizer recurrences, model gradients and checkpoint readers."""
from decimal import Decimal, localcontext
import math
import struct
import zlib

import cpu_training_oracles as model_oracle


def bits(value):
    return struct.unpack('<Q', struct.pack('<d', value))[0]


def value(pattern):
    return struct.unpack('<d', struct.pack('<Q', pattern))[0]


def config(kind='sgd', rate=None, clip=0.0, factor=0.0, every=0):
    if kind not in ('sgd', 'adam', 'adamw'):
        raise ValueError('unknown optimizer')
    adaptive = kind != 'sgd'
    return {'kind': ('sgd', 'adam', 'adamw').index(kind),
            'rate': (.01 if adaptive else .05) if rate is None else rate,
            'beta1': .9 if adaptive else 0., 'beta2': .999 if adaptive else 0.,
            'epsilon': 1e-8 if adaptive else 1., 'decay': .1 if kind == 'adamw' else 0.,
            'clip': clip, 'factor': factor, 'every': every}


def initial_state(settings, elements, steps=0):
    return {**settings, 'rows': 1, 'columns': elements, 'steps': steps,
            'power1': 1., 'power2': 1.,
            'first': [0.] * elements if settings['kind'] else [],
            'second': [0.] * elements if settings['kind'] else []}


def clipped(gradient, maximum):
    if not all(math.isfinite(g) for g in gradient):
        raise ValueError('non-finite derivative')
    if not maximum:
        return list(gradient)
    # Decimal norm construction is independent of the implementation's scaled
    # floating-point sum. It also handles overflow and subnormal clip limits.
    with localcontext() as context:
        context.prec = 110
        values = [Decimal.from_float(g) for g in gradient]
        norm = sum((g*g for g in values), Decimal(0)).sqrt()
        cap = Decimal.from_float(maximum)
        return [float(g / norm * cap) for g in values] if norm > cap else list(gradient)


def update(parameters, gradient, state):
    if len(parameters) != state['rows'] * state['columns'] or len(parameters) != len(gradient):
        raise ValueError('shape mismatch')
    if state['steps'] == (1 << 64) - 1:
        raise OverflowError('step counter')
    if not all(math.isfinite(p) for p in parameters):
        raise ValueError('non-finite parameter')
    derivatives = clipped(gradient, state['clip'])
    rate = state['rate']
    if state['every']:
        rate *= state['factor'] ** (state['steps'] // state['every'])
    next_state = {**state, 'steps': state['steps'] + 1,
                  'first': list(state['first']), 'second': list(state['second'])}
    if state['kind']:
        next_state['power1'] *= state['beta1']
        next_state['power2'] *= state['beta2']
    values = []
    for index, (parameter, g) in enumerate(zip(parameters, derivatives)):
        if not state['kind']:
            candidate = parameter - rate * g
            checked = [candidate]
        else:
            m = state['beta1'] * state['first'][index] + (1. - state['beta1']) * g
            v = state['beta2'] * state['second'][index] + (1. - state['beta2']) * (g*g)
            corrected_m = m / (1. - next_state['power1'])
            corrected_v = v / (1. - next_state['power2'])
            direction = corrected_m / (math.sqrt(corrected_v) + state['epsilon'])
            candidate = parameter - rate * direction
            if state['decay']:
                candidate -= rate * state['decay'] * parameter
            next_state['first'][index] = m
            next_state['second'][index] = v
            checked = [m, v, corrected_m, corrected_v, direction, candidate]
        if not all(math.isfinite(x) for x in checked):
            raise ValueError('non-finite update')
        values.append(candidate)
    return values, next_state


def crc(data):
    return data + struct.pack('<I', zlib.crc32(data))


def parse_state(data):
    if not 132 <= len(data) <= 16777216 or data[:8] != b'OXOP0001':
        raise ValueError('optimizer checkpoint header')
    if struct.unpack_from('<I', data, len(data)-4)[0] != zlib.crc32(data[:-4]):
        raise ValueError('optimizer checksum')
    kind, rows, columns, reserved = struct.unpack_from('<IIII', data, 8)
    steps, = struct.unpack_from('<Q', data, 24)
    rate, beta1, beta2, epsilon, decay, clip, power1, power2 = struct.unpack_from('<8d', data, 32)
    elements, factor_bits, payload, every = struct.unpack_from('<4Q', data, 96)
    factor = value(factor_bits)
    if kind > 2 or reserved or max(rows, columns) > 0x7fffffff or rows * columns > 268435455:
        raise ValueError('optimizer dimensions or kind')
    if elements != rows * columns or payload != (16 * elements if kind else 0) or len(data) != 132 + payload:
        raise ValueError('optimizer lengths')
    if not all(math.isfinite(x) for x in (rate, beta1, beta2, epsilon, decay, clip, power1, power2, factor)):
        raise ValueError('optimizer non-finite metadata')
    if rate < 0 or decay < 0 or clip < 0 or not 0 <= factor <= 1 or (not every and factor != 0):
        raise ValueError('optimizer configuration')
    if not kind:
        if (beta1, beta2, epsilon, decay) != (0., 0., 1., 0.):
            raise ValueError('SGD configuration')
    elif not 0 <= beta1 < 1 or not 0 <= beta2 < 1 or epsilon <= 0 or (kind == 1 and decay != 0):
        raise ValueError('adaptive configuration')
    if not kind or steps == 0:
        if (power1, power2) != (1., 1.):
            raise ValueError('initial powers')
    elif not 0 <= power1 <= beta1 or not 0 <= power2 <= beta2:
        raise ValueError('adaptive powers')
    first = list(struct.unpack_from(f'<{elements}d', data, 128)) if kind else []
    second = list(struct.unpack_from(f'<{elements}d', data, 128+8*elements)) if kind else []
    if not all(math.isfinite(x) for x in first+second) or any(x < 0 for x in second):
        raise ValueError('optimizer moments')
    if steps == 0 and any(x != 0 for x in first+second):
        raise ValueError('nonzero initial moments')
    return {'kind': kind, 'rows': rows, 'columns': columns, 'steps': steps,
            'rate': rate, 'beta1': beta1, 'beta2': beta2, 'epsilon': epsilon,
            'decay': decay, 'clip': clip, 'power1': power1, 'power2': power2,
            'factor': factor, 'every': every, 'first': first, 'second': second}


def encode_state(state):
    elements = state['rows'] * state['columns']
    payload = struct.pack(f'<{2*elements}d', *(state['first']+state['second'])) if state['kind'] else b''
    body = b'OXOP0001' + struct.pack('<IIIIQ8dQdQQ', state['kind'], state['rows'], state['columns'], 0,
                                  state['steps'], state['rate'], state['beta1'], state['beta2'],
                                  state['epsilon'], state['decay'], state['clip'], state['power1'], state['power2'],
                                  elements, state['factor'], len(payload), state['every']) + payload
    result = crc(body)
    parse_state(result)
    return result


def session(model, optimizer):
    model_oracle.parse_checkpoint(model)
    parse_state(optimizer)
    return crc(b'OXTR0001' + struct.pack('<QQ', len(model), len(optimizer)) + model + optimizer)


def parse_session(data):
    if data[:8] == b'OXMD0001':
        model = model_oracle.parse_checkpoint(data)
        return {'model': model, 'optimizer': initial_state(config(), len(model['parameters']), model['updates']), 'legacy': True}
    if not 28 <= len(data) <= 16777216 or data[:8] != b'OXTR0001':
        raise ValueError('training checkpoint header')
    if struct.unpack_from('<I', data, len(data)-4)[0] != zlib.crc32(data[:-4]):
        raise ValueError('training checksum')
    ml, sl = struct.unpack_from('<QQ', data, 8)
    if ml + sl != len(data) - 28:
        raise ValueError('training lengths')
    model = model_oracle.parse_checkpoint(data[24:24+ml])
    state = parse_state(data[24+ml:-4])
    if (state['rows'], state['columns']) != (1, len(model['parameters'])):
        raise ValueError('training optimizer shape')
    return {'model': model, 'optimizer': state, 'legacy': False}


def ulp_distance(left, right):
    if not math.isfinite(left) or not math.isfinite(right):
        raise ValueError('ULP comparison requires finite values')
    def ordered(number):
        pattern = bits(number)
        return ~pattern & ((1 << 64)-1) if pattern >> 63 else pattern | (1 << 63)
    return abs(ordered(left) - ordered(right))


def model_gradient(parameters):
    inputs = [[0., 0.], [0., 1.], [1., 0.], [1., 1.]]
    expected = [0., 1., 1., 0.]
    w1, b1, w2, b2 = parameters[:16], parameters[16:24], parameters[24:32], parameters[32]
    preactivation, hidden, residual = [], [], []
    for row in range(4):
        z = [sum(inputs[row][k]*w1[k*8+j] for k in range(2)) + b1[j] for j in range(8)]
        h = [max(0., x) for x in z]
        preactivation.append(z)
        hidden.append(h)
        residual.append(.5 * (sum(h[k]*w2[k] for k in range(8)) + b2 - expected[row]))
    gradient = [0.] * 33
    for row in range(4):
        dz = [residual[row]*w2[j] if preactivation[row][j] > 0 else 0. for j in range(8)]
        for k in range(2):
            for j in range(8):
                gradient[k*8+j] += inputs[row][k]*dz[j]
        for j in range(8):
            gradient[16+j] += dz[j]
            gradient[24+j] += hidden[row][j]*residual[row]
        gradient[32] += residual[row]
    return gradient


def trained(kind, steps, decay=None):
    parameters = list(model_oracle.initial_parameters()[0])
    settings = config(kind)
    if decay is not None:
        settings['decay'] = decay
    state = initial_state(settings, len(parameters))
    for _ in range(steps):
        parameters, state = update(parameters, model_gradient(parameters), state)
    return parameters, state


def invalid_states():
    """Malformed encodings, including semantic changes with coherent CRCs."""
    original = encode_state(initial_state(config('adam'), 3))
    result = [(f'truncated-{cut}', original[:cut]) for cut in (0, 1, 8, 23, 31, 127, 128, 131, len(original)-1)]
    result.append(('trailing-byte', original+b'\0'))
    changed = bytearray(original); changed[-1] ^= 1
    result.append(('bad-crc', bytes(changed)))
    fields = [('kind', 8, '<I', 3), ('reserved', 20, '<I', 1),
              ('negative-row', 12, '<I', 0xffffffff), ('shape-overflow', 12, '<I', 268435455),
              ('element-count', 96, '<Q', (1 << 64)-1), ('payload-length', 112, '<Q', (1 << 64)-1),
              ('negative-rate', 32, '<d', -1.), ('nan-rate', 32, '<d', math.nan),
              ('beta1-one', 40, '<d', 1.), ('negative-beta1', 40, '<d', -.1),
              ('beta2-one', 48, '<d', 1.), ('infinite-beta2', 48, '<d', math.inf),
              ('zero-epsilon', 56, '<d', 0.), ('nan-epsilon', 56, '<d', math.nan),
              ('adam-decay', 64, '<d', .1), ('negative-clip', 72, '<d', -1.),
              ('invalid-initial-power', 80, '<d', .9), ('nan-power', 88, '<d', math.nan),
              ('initial-first-moment', 128, '<d', 1.), ('negative-variance', 152, '<d', -1.),
              ('nan-moment', 128, '<d', math.nan), ('unscheduled-factor', 104, '<d', .5),
              ('large-schedule-factor', 104, '<d', 1.1), ('negative-schedule-factor', 104, '<d', -.1),
              ('noninitial-powers', 24, '<Q', 1)]
    for name, offset, fmt, item in fields:
        changed = bytearray(original[:-4]); struct.pack_into(fmt, changed, offset, item)
        result.append((name, crc(changed)))
    sgd = encode_state(initial_state(config(), 3))
    changed = bytearray(sgd[:-4]); struct.pack_into('<d', changed, 40, .1)
    result.append(('sgd-beta', crc(changed)))
    changed = bytearray(original[:-4]); changed[:8] = b'OXOP0002'
    result.append(('unknown-version', crc(changed)))
    for name, body in result:
        try:
            parse_state(body)
        except ValueError:
            continue
        raise ValueError('negative encoding control is valid: ' + name)
    return result
