#!/usr/bin/env python3
"""Strict host projection of the staged OPA1 parser observation.

This is a test adapter, never a parser/runtime authority. Success requires the
complete token records from the independently built canonical lexer observing
the same source bytes. Candidate rows do not supply expected lexer or AST facts.
The initial projection intentionally rejects unimplemented row families.
"""
import argparse
import json
from pathlib import Path
import re


KINDS = ("Trivia Ident Number String Fn Struct Mod Use Pub Ampersand Dot Let Mut "
         "Return Break Continue If While Else True False LParen RParen LBrace "
         "RBrace Colon Comma Semi Equal EqualEqual NotEqual Not AndAnd OrOr Less "
         "LessEqual Greater GreaterEqual Arrow Minus Plus Star Slash Percent "
         "Unsupported Invalid Eof").split()
SPELLINGS = dict(zip(KINDS[4:44], (
    "fn struct mod use pub & . let mut return break continue if while else true "
    "false ( ) { } : , ; = == != ! && || < <= > >= -> - + * / %").split()))
MESSAGES = {
    1: "expected a top-level function declaration",
    2: "expected function name", 3: "expected parameter name",
    4: "parameter requires an explicit type",
    5: "expected `bool`, `i32` or `()` type",
    6: "only the unit type `()` is supported here",
    7: "expected `(`", 8: "expected `)`",
    9: "function requires an explicit return type after `->`",
    10: "expected function body `{`", 11: "expected `}` before end of file",
    13: "expected a bool, i32 or unit expression",
    17: "statement requires `;`",
    21: "loop transfer requires `;`; values and labels are unavailable",
    22: "grouping requires `)`",
    23: "comparison operators cannot be chained; use parentheses",
    24: "call requires `)`",
    25: "expected binding name",
    26: "binding requires an initializer",
    27: "expected if body `{`",
    28: "expected else body `{`",
    29: "expected while body `{`",
}
FAMILIES = {1: "module", 2: "import", 3: "record", 4: "enum",
            5: "reference_type", 6: "array_type", 7: "qualified_type",
            8: "match", 9: "array_literal", 10: "qualified_value_or_call",
            11: "field_index_length", 12: "indexing", 13: "record_literal",
            14: "borrow_argument"}
EXPRESSION_KINDS = set(range(15, 37))
IMPLEMENTED = set(range(1, 15)) | EXPRESSION_KINDS
# Local shape constraints validate the supplied tree; they never select a parse
# from tokens or supply expected AST facts in a differential comparison.
BINARY = {
    24: ("Arithmetic", "Add", "Plus", 4),
    25: ("Arithmetic", "Subtract", "Minus", 4),
    26: ("Arithmetic", "Multiply", "Star", 5),
    27: ("Arithmetic", "Divide", "Slash", 5),
    28: ("Arithmetic", "Remainder", "Percent", 5),
    29: ("Comparison", "Equal", "EqualEqual", 3),
    30: ("Comparison", "NotEqual", "NotEqual", 3),
    31: ("Comparison", "Less", "Less", 3),
    32: ("Comparison", "LessEqual", "LessEqual", 3),
    33: ("Comparison", "Greater", "Greater", 3),
    34: ("Comparison", "GreaterEqual", "GreaterEqual", 3),
    35: ("Logical", "And", "AndAnd", 2),
    36: ("Logical", "Or", "OrOr", 1),
}


class ObservationError(ValueError):
    """Malformed or internally inconsistent observation; never a language error."""


class ProjectionUnsupported(ObservationError):
    """A well-tagged row belongs to a projection stage not implemented here."""


def require(condition, message):
    if not condition:
        raise ObservationError(message)


def span(start, end):
    return {"file_id": 0, "start": start, "end": end}


def diagnostic(source, code, stage, message, start, end):
    def location(offset):
        return source[:offset].count(b"\n") + 1, offset - source.rfind(b"\n", 0, offset)
    line, column = location(start)
    end_line, end_column = location(end)
    return {"schema_version": 1, "edition": "typed-preview", "kind": "diagnostic",
            "severity": "error", "code": code, "stage": stage, "message": message,
            "primary": {**span(start, end), "path": "stdin.ox", "line": line,
                        "column": column, "end_line": end_line, "end_column": end_column},
            "secondary": [], "notes": []}


def validate_tokens(tokens, source):
    """Validate externally observed records; do not synthesize a lexer oracle."""
    require(isinstance(tokens, list) and 1 <= len(tokens) <= 129,
            "success needs the complete independently observed token tape")
    prior = 0
    for index, token in enumerate(tokens):
        require(isinstance(token, dict) and set(token) == {"id", "kind", "file_id", "start", "end"},
                "unexpected canonical token fields")
        require(all(type(token[key]) is int for key in ("id", "file_id", "start", "end")),
                "token integer fields must be integers")
        kind, start, end = token["id"], token["start"], token["end"]
        require(1 <= kind <= 47 and token["kind"] == KINDS[kind - 1], "token kind/id mismatch")
        require(token["file_id"] == 0 and start == prior and start <= end <= len(source),
                "token source identity or contiguous coverage mismatch")
        if index == len(tokens) - 1:
            require((kind, start, end) == (47, len(source), len(source)), "invalid final EOF")
        else:
            require(kind != 47 and start < end, "invalid non-final token")
        spelling = source[start:end]
        if token["kind"] in SPELLINGS:
            require(spelling == SPELLINGS[token["kind"]].encode(), "token spelling/source mismatch")
        elif kind == 2:
            require(re.fullmatch(rb"[A-Za-z_][A-Za-z_0-9]*", spelling) is not None,
                    "identifier token/source mismatch")
        elif kind == 3:
            require(re.fullmatch(rb"[0-9][A-Za-z_0-9.]*", spelling) is not None,
                    "number token/source mismatch")
        prior = end


def decode_wire(raw, source):
    """Decode column byte planes and validate transport bounds/inactive storage."""
    require(isinstance(source, bytes) and len(source) <= 128 and source.isascii(),
            "source outside bounded ASCII domain")
    require(isinstance(raw, bytes) and len(raw) >= 11 and raw[:4] == b"OPA1",
            "invalid OPA1 header")
    error, detail, start, end, count, items, used = raw[4:11]
    require(used == len(source), "source byte count mismatch")
    require(error in range(7), "unknown error tag; internal error 9 is transport failure")
    require(0 <= start <= end <= used, "diagnostic span outside source")
    if error:
        require(len(raw) == 11 and count == items == 0, "failure retained partial AST or trailing bytes")
        require(detail != 0, "failure requires an explicit detail")
        return {"error": error, "detail": detail, "start": start, "end": end, "rows": [], "items": 0}
    require(len(raw) == 1559 and detail == start == end == 0, "invalid success extent or header")
    require(count <= 128 and items <= count, "row count or item head outside capacity")
    columns = [[sum(raw[11 + col * 516 + plane * 129 + i] << (8 * plane)
                    for plane in range(4)) for i in range(129)] for col in range(3)]
    rows = []
    for index, (header, ab, cd) in enumerate(zip(*columns)):
        if index >= count:
            require(header == ab == cd == 0, "nonzero inactive row or reserved row 129")
            continue
        kind, lo, hi, next_row = header & 63, (header >> 6) & 255, (header >> 14) & 255, header >> 22
        fields = [ab & 255, ab >> 8, cd & 255, cd >> 8]
        require(header <= 0x7fffffff and 1 <= kind <= 36 and next_row <= count,
                "invalid row kind, next reference or signed header")
        require(lo <= hi <= used and all(value <= 128 for value in fields),
                "row span or payload outside capacity")
        rows.append({"kind": kind, "start": lo, "end": hi,
                     **dict(zip(("a", "b", "c", "d"), fields)), "next": next_row})
    require(bool(count) == bool(items), "nonempty row table requires an item head")
    return {"error": 0, "detail": 0, "start": 0, "end": 0, "rows": rows, "items": items}


def project_failure(wire, source, tokens=None):
    error, detail, start, end = (wire[key] for key in ("error", "detail", "start", "end"))
    if tokens is not None:
        validate_tokens(tokens, source)
        if error in (1, 4, 5) or (error == 2 and detail in (1, 3)):
            require(any(t["start"] == start and t["end"] == end and t["id"] != 1 for t in tokens),
                    "failure span is not an observed nontrivia token")
    if error == 4:
        require(detail in FAMILIES, "unknown domain-refusal family")
        return {"status": "domain_refusal", "family": FAMILIES[detail], "span": span(start, end)}
    if error == 5:
        return {"status": "stage_pending", "detail": detail, "span": span(start, end)}
    code, stage = "E0100", "parse"
    if error == 1:
        require(detail in MESSAGES, "unknown E0100 message ID")
        message = MESSAGES[detail]
    elif error == 2:
        code = "E0101"
        require(detail in (1, 2, 3), "unknown E0101 detail")
        if detail == 2:
            message = "unsupported numeric spelling; expected ASCII decimal literal digits"
        else:
            if detail == 3:
                require(source[start:end] in (b".", b"["), "unsupported punctuation/source mismatch")
            message = "unsupported typed-preview construct `" + source[start:end].decode("ascii") + "`"
    elif error == 3:
        require(detail in (1, 2), "unknown E0400 detail")
        code = "E0400"
        message = ("expression nesting limit exceeded" if detail == 1 else
                   "statement block nesting limit exceeded")
    elif error == 6:
        require(detail in (1, 2) and start < end == len(source), "invalid lexical failure")
        opener = b'"' if detail == 1 else b"/*"
        require(source[start:].startswith(opener), "lexical failure opener/source mismatch")
        stage = "lex"
        message = "unterminated string literal" if detail == 1 else "unterminated block comment"
    else:
        raise ObservationError("expected failure header")
    result = {"status": "lexical_diagnostic" if error == 6 else "diagnostic",
              "diagnostic": diagnostic(source, code, stage, message, start, end)}
    if error != 6:
        result["projection"] = "first_parser_diagnostic"
    return result


def project_ast(wire, source, tokens):
    validate_tokens(tokens, source)
    rows, claimed, cursor, expressions, heights = wire["rows"], set(), 0, [], []

    def take(kind, ref=None):
        nonlocal cursor
        while cursor < len(tokens) and tokens[cursor]["id"] == 1:
            cursor += 1
        require(cursor < len(tokens), "AST consumed beyond EOF")
        token = tokens[cursor]
        require(token["kind"] == kind, "AST/token kind mismatch: expected " + kind)
        require(ref is None or ref == cursor + 1, "row token reference is not the exact canonical slot")
        cursor += 1
        return span(token["start"], token["end"])

    def claim(ref, kinds, linked=False):
        require(1 <= ref <= len(rows), "row reference outside active rows")
        require(ref not in claimed, "cyclic or shared row/list ownership")
        row = rows[ref - 1]
        if row["kind"] not in IMPLEMENTED:
            raise ProjectionUnsupported("projection not implemented for row tag " + str(row["kind"]))
        require(row["kind"] in kinds, "row reference has the wrong kind")
        require(linked or row["next"] == 0, "non-list row retained a next link")
        claimed.add(ref)
        return row

    def zero(row, *fields):
        require(all(row[field] == 0 for field in fields), "nonzero unused logical row field")

    def extent(row, first, last):
        require((row["start"], row["end"]) == (first["start"], last["end"]), "AST span/token mismatch")
        return span(row["start"], row["end"])

    def ty(ref):
        row = claim(ref, {3, 4})
        zero(row, "b", "c", "d")
        if row["kind"] == 3:
            name = take("Ident", row["a"])
            return {"kind": "TypeName", "span": extent(row, name, name), "name": name}
        zero(row, "a")
        first, last = take("LParen"), take("RParen")
        return {"kind": "TypeUnit", "span": extent(row, first, last)}

    def expression(ref, floor=0, linked=False):
        row = claim(ref, EXPRESSION_KINDS, linked=linked)
        kind, fields, height = row["kind"], {}, 1
        require(kind not in BINARY or BINARY[kind][3] >= floor,
                "expression tree violates precedence or associativity")
        if kind not in BINARY:
            zero(row, "c")
        if kind == 15:
            require(row["b"] in (0, 1), "invalid Number negative flag")
            first = take("Minus") if row["b"] else None
            last = take("Number", row["a"])
            require(source[last["start"]:last["end"]].isdigit(), "successful Number is not ASCII decimal")
            fields = {"kind": "Number", "digits": last, "negative": bool(row["b"])}
            first = first or last
        elif kind in (16, 17):
            zero(row, "a", "b")
            first = last = take("True" if kind == 17 else "False")
            fields = {"kind": "Bool", "value": kind == 17}
        elif kind == 18:
            zero(row, "a", "b")
            first, last = take("LParen"), take("RParen")
            fields = {"kind": "Unit"}
        elif kind == 19:
            zero(row, "b")
            first = last = take("Ident", row["a"])
            fields = {"kind": "Name", "name": first}
        elif kind == 20:
            first = take("Ident", row["a"])
            take("LParen")
            args, argument, maximum = [], row["b"], 0
            while argument:
                if args:
                    take("Comma")
                value = expression(argument, linked=True)
                args.append(value)
                maximum = max(maximum, heights[value])
                argument = rows[argument - 1]["next"]
            last = take("RParen")
            fields = {"kind": "Call", "callee": first, "args": args}
            height += maximum
        elif kind == 21:
            zero(row, "b")
            first = take("LParen")
            operand = expression(row["a"])
            last = take("RParen")
            fields = {"kind": "Group", "operand": operand}
            height += heights[operand]
        elif kind in (22, 23):
            first = take("Minus" if kind == 22 else "Not", row["b"])
            operand = expression(row["a"], floor=6)
            child = expressions[operand]
            require(kind != 22 or child["kind"] != "Number" or child["negative"],
                    "minus followed by digits must be a signed Number")
            last = child["span"]
            fields = {"kind": "Negate" if kind == 22 else "Not",
                      "operand": operand, "operator_span": first}
            height += heights[operand]
        else:
            family, op, token, precedence = BINARY[kind]
            left = expression(row["a"], floor=precedence + (family == "Comparison"))
            operator = take(token, row["c"])
            right = expression(row["b"], floor=precedence + 1)
            first, last = expressions[left]["span"], expressions[right]["span"]
            fields = {"kind": family, "op": op, "left": left, "right": right,
                      "operator_span": operator}
            height += max(heights[left], heights[right])
        require(row["d"] == height and height <= 64, "invalid computed expression height")
        expression_id = len(expressions)
        expressions.append({"id": expression_id, "span": extent(row, first, last), **fields})
        heights.append(height)
        return expression_id

    def block(ref, blocks, depth=1):
        require(depth <= 64, "statement block nesting limit exceeded in successful AST")
        row = claim(ref, {5})
        zero(row, "c", "d")
        block_id = len(blocks)
        blocks.append(None)  # Canonical block IDs are allocated on entry.
        first, body, statement = take("LBrace"), [], row["b"]
        while statement:
            current = claim(statement, set(range(6, 15)), linked=True)
            zero(current, "d")
            tag = current["kind"]
            if tag in (6, 7):
                start = take("Let")
                if tag == 7:
                    take("Mut")
                name = take("Ident", current["a"])
                annotation = None
                if current["b"]:
                    take("Colon")
                    annotation = ty(current["b"])
                take("Equal")
                fields = {"kind": "Let", "mutable": tag == 7, "name": name,
                          "annotation": annotation, "init": expression(current["c"])}
            elif tag == 8:
                start = take("Ident", current["a"])
                operator = take("Equal", current["b"])
                fields = {"kind": "Assign", "name": start, "operator_span": operator,
                          "value": expression(current["c"])}
            elif tag == 9:
                zero(current, "b", "c")
                value = expression(current["a"])
                start = expressions[value]["span"]
                fields = {"kind": "Expr", "value": value}
            elif tag == 10:
                zero(current, "b", "c")
                start = take("Return")
                fields = {"kind": "Return", "value": expression(current["a"]) if current["a"] else None}
            elif tag in (11, 12):
                zero(current, "a", "b", "c")
                name = "Break" if tag == 11 else "Continue"
                start, fields = take(name), {"kind": name}
            else:
                start = take("If" if tag == 13 else "While")
                condition = expression(current["a"])
                child = block(current["b"], blocks, depth + 1)
                if tag == 13:
                    otherwise = None
                    if current["c"]:
                        take("Else")
                        otherwise = block(current["c"], blocks, depth + 1)
                    fields = {"kind": "If", "condition": condition,
                              "then_block": child, "else_block": otherwise}
                    end = blocks[otherwise if otherwise is not None else child]["end"]
                else:
                    zero(current, "c")
                    fields = {"kind": "While", "condition": condition, "body": child}
                    end = blocks[child]["end"]
            if tag < 13:
                end = take("Semi")
            body.append({"span": extent(current, start, end), **fields})
            statement = current["next"]
        last = take("RBrace", row["a"])
        blocks[block_id] = {"id": block_id, "span": extent(row, first, last), "end": last, "body": body}
        return block_id

    items, functions, item = [], [], wire["items"]
    while item:
        row = claim(item, {1}, linked=True)
        public = take("Pub", row["a"]) if row["a"] else None
        take("Fn")
        name = take("Ident")
        extent(row, name, name)
        take("LParen")
        params, parameter = [], row["b"]
        while parameter:
            if params:
                take("Comma")
            current = claim(parameter, {2}, linked=True)
            zero(current, "b", "c", "d")
            param_name = take("Ident")
            extent(current, param_name, param_name)
            take("Colon")
            params.append({"name": param_name, "ty": ty(current["a"])})
            parameter = current["next"]
        take("RParen")
        take("Arrow")
        result = ty(row["c"])
        blocks = []
        body = block(row["d"], blocks)
        function_id = len(functions)
        items.append({"kind": "Function", "id": function_id})
        functions.append({"id": function_id, "public": public, "name": name,
                          "params": params, "result": result, "body": body,
                          "blocks": blocks, "end": blocks[body]["end"]})
        item = row["next"]
    take("Eof")
    require(cursor == len(tokens) and len(claimed) == len(rows), "unconsumed tokens or orphan active rows")
    return {"status": "ok", "ast": {"tokens": [dict(t) for t in tokens], "items": items,
                                     "functions": functions, "expressions": expressions}}


def decode(raw, source, tokens=None):
    """Return a full canonical-shaped result, domain refusal, or explicit pending stage."""
    wire = decode_wire(raw, source)
    return project_failure(wire, source, tokens) if wire["error"] else project_ast(wire, source, tokens)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--observation", type=Path, required=True)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--tokens", type=Path, help="JSON from canonical lexer or parser observer")
    args = parser.parse_args()
    tokens = None
    try:
        if args.tokens:
            facts = json.loads(args.tokens.read_text())
            require(isinstance(facts, dict) and facts.get("status") == "ok", "token observer must have succeeded")
            tokens = facts.get("tokens", facts.get("ast", {}).get("tokens"))
        result = decode(args.observation.read_bytes(), args.source.read_bytes(), tokens)
    except (OSError, ValueError) as error:
        parser.exit(2, "invalid parser observation: " + str(error) + "\n")
    print(json.dumps(result, sort_keys=True))


if __name__ == "__main__":
    main()
