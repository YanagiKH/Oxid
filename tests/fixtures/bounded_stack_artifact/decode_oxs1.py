#!/usr/bin/env python3
"""Independent OXS1 byte decoder; imports no Oxid implementation or oracle data."""

import argparse
import json
from pathlib import Path
import struct
import sys


class InvalidArtifact(ValueError):
    def __init__(self, reason, offset):
        super().__init__(reason)
        self.reason = reason
        self.offset = offset


class EvaluationOverflow(ArithmeticError):
    def __init__(self, row):
        super().__init__("checked i32 arithmetic overflow")
        self.row = row


def decode(data):
    """Validate the entire artifact before evaluating any instruction."""
    if len(data) != 80:
        raise InvalidArtifact("length", -1)
    for offset, expected in enumerate(b"OXS1"):
        if data[offset] != expected:
            raise InvalidArtifact("magic", offset)
    count = data[4]
    if not 1 <= count <= 15:
        raise InvalidArtifact("count", 4)
    rows = list(struct.iter_unpack("<BI", data[5:]))
    height = 0
    for index, (opcode, operand) in enumerate(rows[:count]):
        start = 5 + 5 * index
        if opcode not in (1, 2, 3):
            raise InvalidArtifact("opcode", start)
        if operand > 0x7FFF_FFFF:
            raise InvalidArtifact("high-bit", start + 4)
        if opcode == 1:
            height += 1
        else:
            if operand != 0:
                offset = next(p for p in range(start + 1, start + 5) if data[p])
                raise InvalidArtifact("operator-operand", offset)
            if height < 2:
                raise InvalidArtifact("stack-underflow", start)
            height -= 1
    if height != 1:
        raise InvalidArtifact("stack-residual", 4)
    tail_start = 5 + count * 5
    for offset in range(tail_start, 80):
        if data[offset] != 0:
            raise InvalidArtifact("tail", offset)

    # Python arithmetic is unbounded: enforce the existing checked i32 bound
    # explicitly after every add/multiply, before a later instruction runs.
    stack = []
    for index, (opcode, operand) in enumerate(rows[:count]):
        if opcode == 1:
            stack.append(operand)
        else:
            right = stack.pop()
            left = stack.pop()
            value = left + right if opcode == 2 else left * right
            if value > 0x7FFF_FFFF:
                raise EvaluationOverflow(index)
            stack.append(value)
    return {"count": count, "rows": rows[:count], "value": stack[0]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("artifact", type=Path)
    args = parser.parse_args()
    try:
        # One extra byte is sufficient to reject all trailing-data cases.
        with args.artifact.open("rb") as stream:
            data = stream.read(81)
        result = decode(data)
    except OSError as error:
        print(json.dumps({"error": "io", "detail": str(error)}), file=sys.stderr)
        return 74
    except InvalidArtifact as error:
        print(json.dumps({"error": error.reason, "offset": error.offset}), file=sys.stderr)
        return 65
    except EvaluationOverflow as error:
        print(json.dumps({"error": "E0604", "row": error.row}), file=sys.stderr)
        return 1
    print(json.dumps(result, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
