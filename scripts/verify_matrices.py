#!/usr/bin/env python3
"""Compare f64 matrix products and transposes with independent Python results."""
import math
import random
import struct
from pathlib import Path

import verify_float_vector as runner

ROOT = Path(__file__).resolve().parents[1]
EXAMPLE = Path('examples/typed-matrix')
SOURCES = (Path('packages/float_vector/oxid.toml'), Path('packages/float_vector/src/lib.ox'),
           Path('packages/matrix/oxid.toml'), Path('packages/matrix/src/lib.ox'),
           EXAMPLE / 'oxid.toml', EXAMPLE / 'src/main.ox', EXAMPLE / 'tests/contracts.ox')


def bits(value):
    if math.isnan(value):
        return 0x7ff8000000000000
    return struct.unpack('<Q', struct.pack('<d', value))[0]


def value(pattern):
    return struct.unpack('<d', struct.pack('<Q', pattern))[0]


def matrix_cases():
    cases = []
    def add(rows, inner, columns, left, right):
        assert len(left) == rows * inner and len(right) == inner * columns
        product = []
        for row in range(rows):
            for column in range(columns):
                total = 0.0
                for index in range(inner):
                    total = total + value(left[row * inner + index]) * value(right[index * columns + column])
                product.append(bits(total))
        transposed = [left[row * inner + column] for column in range(inner) for row in range(rows)]
        cases.append((rows, inner, columns, left, right, product, transposed))
    add(2, 3, 2, [bits(float(i)) for i in range(1, 7)], [bits(float(i)) for i in range(7, 13)])
    # Separate multiplication rounds to one; a fused multiply/add would differ.
    add(1, 2, 1, [bits(-1.0), bits(1.0 + 2.0**-27)], [bits(1.0), bits(1.0 - 2.0**-27)])
    special = [0, 1 << 63, 1, (1 << 63) | 1, 0x0010000000000000,
               0x7fefffffffffffff, 0x7ff0000000000000, 0xfff0000000000000,
               0x7ff0000000000001, 0xfff8000000000042]
    for pattern in special:
        add(1, 1, 1, [pattern], [bits(1.0)])
    for shape in [(0, 0, 0), (0, 3, 2), (2, 0, 3), (2, 3, 0)]:
        rows, inner, columns = shape
        add(rows, inner, columns, [bits(1.0)] * (rows * inner), [bits(2.0)] * (inner * columns))
    rng = random.Random(0x6d6174726978)
    for _ in range(16):
        rows, inner, columns = [rng.randint(1, 4) for _ in range(3)]
        def values(count):
            return [bits(math.ldexp(float(rng.randint(-10000, 10000)), rng.randint(-20, 20))) for _ in range(count)]
        add(rows, inner, columns, values(rows * inner), values(inner * columns))
    return cases


def oracle_program(case):
    rows, inner, columns, left, right, product, transposed = case
    arrays = ''.join(f'    let {name}: [u64; {len(values)}] = [' + ', '.join(str(v) + 'u64' for v in values) + '];\n'
                     for name, values in [('left_bits', left), ('right_bits', right), ('product_bits', product), ('transpose_bits', transposed)])
    return '''use alloc::bytes::GrowStatus;
use crate::matrix::Read;
use crate::vector::new;
use crate::vector::try_push;
use crate::matrix::Matrix;
use crate::matrix::FromVector;
use crate::matrix::MatrixResult;
use crate::matrix::rows;
use crate::matrix::columns;
use crate::matrix::from_vector;
use crate::matrix::get;
use crate::matrix::transpose;
use crate::matrix::matmul;
fn create(row_count: i32, column_count: i32, patterns: &[u64]) -> FromVector {
    let mut elements = new();
    let mut index = 0;
    while index < patterns.len() {
        let pattern = patterns[index];
        let element = f64::from_bits(pattern);
        let appended = try_push(&mut elements, element);
        match appended {
            GrowStatus::Ok => {},
            GrowStatus::CapacityOverflow => { return from_vector(elements, -1, -1); },
            GrowStatus::AllocationFailed => { return from_vector(elements, -1, -1); },
            GrowStatus::MemoryLimit => { return from_vector(elements, -1, -1); },
        }
        index = index + 1;
    }
    return from_vector(elements, row_count, column_count);
}
fn same(matrix: &Matrix, row_count: i32, column_count: i32, patterns: &[u64]) -> bool {
    if rows(&*matrix) != row_count || columns(&*matrix) != column_count { return false; }
    if patterns.len() != row_count * column_count { return false; }
    let mut index = 0;
    while index < patterns.len() {
        let found = get(&*matrix, index / column_count, index % column_count);
        match found {
            Read::Value(element) => { if element.to_bits() != patterns[index] { return false; } },
            Read::OutOfBounds => { return false; },
        }
        index = index + 1;
    }
    return true;
}
fn verify(left: Matrix, right: Matrix, product_bits: &[u64], transpose_bits: &[u64], left_bits: &[u64], right_bits: &[u64]) -> i32 {
    let row_count = rows(&left);
    let inner = columns(&left);
    let column_count = columns(&right);
    let multiplied = matmul(&left, &right);
    match multiplied {
        MatrixResult::Ok(product) => { if !same(&product, row_count, column_count, &*product_bits) { return 1; } },
        MatrixResult::Err(error) => { return 2; },
    }
    let swapped = transpose(&left);
    match swapped {
        MatrixResult::Ok(result) => { if !same(&result, inner, row_count, &*transpose_bits) { return 3; } },
        MatrixResult::Err(error) => { return 4; },
    }
    if !same(&left, row_count, inner, &*left_bits) { return 5; }
    if !same(&right, inner, column_count, &*right_bits) { return 6; }
    return 0;
}
fn main() -> i32 {
''' + arrays + f'''    let first = create({rows}, {inner}, &left_bits);
    let second = create({inner}, {columns}, &right_bits);
''' + '''    match first {
        FromVector::Ok(left) => {
            match second {
                FromVector::Ok(right) => { return verify(left, right, &product_bits, &transpose_bits, &left_bits, &right_bits); },
                FromVector::InvalidShape(rejected) => { return 7; },
            }
        },
        FromVector::InvalidShape(rejected) => { return 8; },
    }
}
'''


if __name__ == '__main__':
    # Share command capture and source-free execution with the vector verifier.
    # Each expected matrix is calculated above, independently of Oxid sources.
    runner.main_for_package(ROOT, EXAMPLE, SOURCES, 'matrix-example', 'matrix',
                            [('product', (ROOT / EXAMPLE / 'src/main.ox').read_text(), b'154\n'),
                             ('contracts', (ROOT / EXAMPLE / 'tests/contracts.ox').read_text(), b'0\n')]
                            + [(f'matrices-{i//8}', runner.combine_programs([oracle_program(case) for case in matrix_cases()[i:i+8]], 'matrix_case'), b'0\n')
                               for i in range(0, len(matrix_cases()), 8)],
                            {'matrix_cases': len(matrix_cases())}, __file__)
