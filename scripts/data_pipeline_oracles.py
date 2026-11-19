"""Independent data formats, epoch ordering and minibatch gradient equations."""
import math
import struct
import zlib

import optimizer_oracles as optimizer

MAXIMUM = (1 << 64) - 1


def crc(body):
    return body + struct.pack('<I', zlib.crc32(body))


def checked(body, magic, minimum):
    if not minimum <= len(body) <= 16777216 or body[:8] != magic:
        raise ValueError('header')
    if struct.unpack_from('<I', body, len(body)-4)[0] != zlib.crc32(body[:-4]):
        raise ValueError('checksum')


def dataset(inputs, targets):
    rows = len(inputs)
    if not rows or rows != len(targets):
        raise ValueError('rows')
    features, outputs = len(inputs[0]), len(targets[0])
    if min(features, outputs) <= 0 or any(len(row) != features for row in inputs) or any(len(row) != outputs for row in targets):
        raise ValueError('columns')
    values = [x for row in inputs for x in row] + [x for row in targets for x in row]
    if len(values) > 2097147 or not all(math.isfinite(x) for x in values):
        raise ValueError('values')
    return crc(b'OXDS0001'+struct.pack('<IIIIQ', rows, features, outputs, 0, 8*len(values))
               + struct.pack(f'<{len(values)}d', *values))


def parse_dataset(body):
    checked(body, b'OXDS0001', 36)
    rows, features, outputs, reserved, length = struct.unpack_from('<IIIIQ', body, 8)
    count = rows * (features + outputs)
    if reserved or min(rows, features, outputs) <= 0 or count > 2097147 or length != count*8 or len(body) != 36+length:
        raise ValueError('dimensions or length')
    values = list(struct.unpack_from(f'<{count}d', body, 32))
    if not all(math.isfinite(x) for x in values):
        raise ValueError('non-finite value')
    first = values[:rows*features]
    second = values[rows*features:]
    return {'rows': rows, 'features': features, 'outputs': outputs,
            'inputs': [first[i:i+features] for i in range(0, len(first), features)],
            'targets': [second[i:i+outputs] for i in range(0, len(second), outputs)]}


def random_word(state):
    state ^= (state << 13) & 0xffffffff
    state ^= state >> 17
    return (state ^ (state << 5)) & 0xffffffff


def reorder(state):
    result = {**state, 'order': list(range(state['rows']))}
    if result['shuffle']:
        for index in range(result['rows']-1, 0, -1):
            bound = index + 1
            limit = 0xffffffff - 0xffffffff % bound
            while True:
                result['rng'] = random_word(result['rng'])
                draw = result['rng'] - 1
                if draw < limit:
                    other = draw % bound
                    result['order'][index], result['order'][other] = result['order'][other], result['order'][index]
                    break
    return result


def sampler(rows, size, shuffle, seed):
    if not 0 < rows <= 536870911 or not 0 < size <= 0x7fffffff or not 0 <= seed <= 0xffffffff:
        raise ValueError('sampler settings')
    return reorder({'rows': rows, 'size': size, 'shuffle': bool(shuffle), 'rng': seed or 1831565813,
                    'cursor': 0, 'epoch': 0, 'batches': 0, 'order': []})


def indices(state):
    return state['order'][state['cursor']:state['cursor']+state['size']]


def advance(state):
    if state['batches'] == MAXIMUM:
        raise OverflowError('batch counter')
    count = (state['rows']-1)//state['size']+1
    batches = state['batches']+1
    result = {**state, 'batches': batches, 'epoch': batches//count, 'cursor': (batches % count)*state['size']}
    return reorder(result) if not result['cursor'] else result


def encode_sampler(state):
    result = crc(b'OXSP0001'+struct.pack('<IIIIIIQQQ', state['rows'], state['size'], int(state['shuffle']),
                                       state['rng'], state['cursor'], 0, state['epoch'], state['batches'],
                                       4*state['rows'])+struct.pack(f'<{state["rows"]}I', *state['order']))
    parse_sampler(result)
    return result


def parse_sampler(body):
    checked(body, b'OXSP0001', 64)
    rows, size, shuffle, rng, cursor, reserved, epoch, batches, length = struct.unpack_from('<IIIIIIQQQ', body, 8)
    if not 0 < rows <= 4194289 or not 0 < size <= 0x7fffffff or shuffle > 1 or not rng or reserved or length != rows*4 or len(body) != 60+length:
        raise ValueError('sampler metadata')
    count = (rows-1)//size+1
    if epoch != batches//count or cursor != (batches % count)*size:
        raise ValueError('inconsistent progress')
    order = list(struct.unpack_from(f'<{rows}I', body, 56))
    if sorted(order) != list(range(rows)) or (not shuffle and order != list(range(rows))):
        raise ValueError('permutation')
    return {'rows': rows, 'size': size, 'shuffle': bool(shuffle), 'rng': rng, 'cursor': cursor,
            'epoch': epoch, 'batches': batches, 'order': order}


def session(training, data, state):
    model = optimizer.parse_session(training)
    source = parse_dataset(data)
    sampling = parse_sampler(state)
    if model['legacy'] or source['rows'] != sampling['rows'] or model['model']['inputs'] != source['features'] or model['model']['outputs'] != source['outputs']:
        raise ValueError('component mismatch')
    return crc(b'OXDP0001'+struct.pack('<QQQ', len(training), len(data), len(state))+training+data+state)


def parse_session(body):
    checked(body, b'OXDP0001', 36)
    first, second, third = struct.unpack_from('<QQQ', body, 8)
    if first + second + third != len(body)-36:
        raise ValueError('component lengths')
    training = optimizer.parse_session(body[32:32+first])
    source = parse_dataset(body[32+first:32+first+second])
    sampling = parse_sampler(body[32+first+second:-4])
    if training['legacy'] or source['rows'] != sampling['rows'] or training['model']['inputs'] != source['features'] or training['model']['outputs'] != source['outputs']:
        raise ValueError('component mismatch')
    return {'training': training, 'dataset': source, 'sampler': sampling}


def initial_session():
    parameters, rng = optimizer.model_oracle.initial_parameters()
    model = crc(b'OXMD0001'+struct.pack('<IIIIQ', 2, 8, 1, rng, 0)+struct.pack('<33d', *parameters))
    state = optimizer.encode_state(optimizer.initial_state(optimizer.config('adam'), 33))
    source = dataset([[0., 0.], [0., 1.], [1., 0.], [1., 1.]], [[0.], [1.], [1.], [0.]])
    return session(optimizer.session(model, state), source, encode_sampler(sampler(4, 3, True, 42)))


def loss_gradient(parameters, inputs, targets, features=2, hidden=8, outputs=1):
    """Differentiate the matrix equations without traversing the Oxid graph."""
    w1_end = features*hidden
    bias_end = w1_end+hidden
    w2_end = bias_end+hidden*outputs
    w1, b1, w2, b2 = parameters[:w1_end], parameters[w1_end:bias_end], parameters[bias_end:w2_end], parameters[w2_end:]
    if len(b2) != outputs or len(inputs) != len(targets) or not inputs:
        raise ValueError('model or batch shape')
    gradient = [0.] * len(parameters)
    objective = 0.
    scale = 2. / (len(inputs)*outputs)
    for x, target in zip(inputs, targets):
        z = [sum(x[k]*w1[k*hidden+j] for k in range(features))+b1[j] for j in range(hidden)]
        h = [max(0., number) for number in z]
        prediction = [sum(h[j]*w2[j*outputs+k] for j in range(hidden))+b2[k] for k in range(outputs)]
        residual = [scale*(value-expected) for value, expected in zip(prediction, target)]
        objective += sum((value-expected)**2 for value, expected in zip(prediction, target))
        dz = [sum(residual[k]*w2[j*outputs+k] for k in range(outputs)) if z[j] > 0 else 0. for j in range(hidden)]
        for k in range(features):
            for j in range(hidden):
                gradient[k*hidden+j] += x[k]*dz[j]
        for j in range(hidden):
            gradient[w1_end+j] += dz[j]
            for k in range(outputs):
                gradient[bias_end+j*outputs+k] += h[j]*residual[k]
        for k in range(outputs):
            gradient[w2_end+k] += residual[k]
    return objective/(len(inputs)*outputs), gradient


def sampling_cases():
    return [{'name': name, 'rows': rows, 'size': size, 'shuffle': shuffle, 'seed': seed,
             'batches': 3*((rows-1)//size+1)+1}
            for name, rows, size, shuffle, seed in [
                ('ordered-tail', 7, 3, False, 1), ('shuffle-one', 7, 3, True, 1),
                ('shuffle-default-seed', 7, 2, True, 0), ('shuffle-high-seed', 7, 4, True, 0xffffffff),
                ('one-row', 1, 1, True, 42), ('oversized-batch', 5, 2147483647, True, 123),
                ('unit-batches', 9, 1, True, 79), ('exact-batches', 8, 4, False, 0)]]
