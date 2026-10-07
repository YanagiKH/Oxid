#!/usr/bin/env python3
"""Strict host-only projection of complete bounded static observations.

The unchanged parser adapter validates OPA1 and the independent token tape. The
resolution adapter supplies structural ordering and copies candidate bindings
and literal values. This adapter copies the candidate's complete semantic column;
it never looks up a name, converts a literal, infers a type, or computes flow.
Structural diagnostic checks are not a second checker: canonical comparison is
still required to establish the reported constraint and first-error ordering.
"""
import argparse
import copy
import json
from pathlib import Path

import parser_ast_observation as syntax
import static_resolution_observation as resolution


ObservationError = syntax.ObservationError
require = syntax.require
SCHEMA = "canonical-static-observation-1"
OPA_SIZE = resolution.OPA_SIZE
HEADER_END = resolution.HEADER_END
RESOLUTION_END = resolution.RESOLUTION_END
TOTAL_SIZE = resolution.TOTAL_SIZE
TYPES = resolution.TYPES

# A fixed sixteen-value representation of the observed four bits, never a
# traversal of statements or a calculation of expected control-flow outcomes.
FLOW_VIEWS = tuple({
    "mask": mask, "fallthrough": bool(mask & 1), "returns": bool(mask & 2),
    "breaks": bool(mask & 4), "continues": bool(mask & 8),
    "debug": "FlowSummary { fallthrough: %s, returns: %s, breaks: %s, continues: %s }" %
             tuple(str(bool(mask & bit)).lower() for bit in (1, 2, 4, 8)),
} for mask in range(16))


def _validate_facts(structure, facts, semantic):
    # Reuse the complete existing resolution role/ownership checks. The zero
    # argument is only its resolution-stage guard, not a supplied semantic fact.
    resolution._validate_facts(structure, facts, [0] * 129)
    require(not any(semantic[len(structure.rows):]), "nonzero inactive or sentinel semantic cells")
    roots = {structure.row(ref)["d"] for ref in structure.functions}
    typed_roles = {1, *resolution.LOCAL_KINDS, *syntax.EXPRESSION_KINDS}
    for ref, row in enumerate(structure.rows, 1):
        kind, observed = row["kind"], semantic[ref - 1]
        if kind in typed_roles:
            require(observed in TYPES, "missing or invalid semantic type fact")
        elif kind == 5:
            require(1 <= observed <= 15, "missing or invalid complete block flow mask")
            require(ref not in roots or observed == 2, "function root flow must be returns-only")
        else:
            require(observed == 0, "nonzero semantic fact for an unused row role")

        # Only agreement between already-present facts is checked here. No
        # expression typing rules or flow operations generate expected facts.
        if kind == 1:
            require(observed == facts[row["c"] - 1], "function result facts disagree")
        elif kind == 2:
            require(observed == facts[row["a"] - 1], "parameter type facts disagree")
        elif kind in (6, 7) and row["b"]:
            require(observed == facts[row["b"] - 1], "local annotation facts disagree")
        elif kind in (19, 20):
            require(observed == semantic[facts[ref - 1] - 1], "referenced type facts disagree")


def _project_hir(structure, facts, semantic, source):
    hir = resolution._project_hir(structure, facts, source)
    for owner, function in enumerate(hir["functions"]):
        # The signature's result is represented in both columns. Agreement was
        # validated above; publish the semantic result from the Function row.
        function["signature"]["result"] = TYPES[semantic[structure.functions[owner] - 1]]
        for local, ref in zip(function["locals"], structure.locals[owner]):
            local["ty"] = TYPES[semantic[ref - 1]]
        for expression, ref in zip(function["expressions"], structure.expressions[owner]):
            expression["ty"] = TYPES[semantic[ref - 1]]
        for block, ref in zip(function["blocks"], structure.blocks[owner]):
            block["flow"] = copy.deepcopy(FLOW_VIEWS[semantic[ref - 1]])
    return hir


def _project_diagnostic(header, structure, source, tokens):
    kind, start, end, secondary_start, secondary_end, label, expected, actual, nargs, argc = header[6:]
    require(1 <= kind <= 14, "unknown static diagnostic kind")
    if kind <= 7:
        result = resolution._project_diagnostic(header, structure, source, tokens)
        result["schema"] = SCHEMA
        return result
    require(not resolution._owned_route(structure.rows, source),
            "unknown-type public route cannot publish a type diagnostic")
    require(0 <= start < end <= len(source), "invalid type diagnostic primary span")
    if label:
        require(0 <= secondary_start < secondary_end <= len(source), "invalid type diagnostic secondary span")
    else:
        require(secondary_start == secondary_end == 0, "unlabelled diagnostic retained a secondary span")
    if kind == 8:
        require(expected in TYPES and actual in TYPES and expected != actual,
                "type mismatch requires distinct primitive type codes")
        require(nargs == argc == 0, "type mismatch retained argument counts")
    elif kind == 10:
        require(expected == actual == 0 and nargs != argc, "arity diagnostic has invalid payload fields")
    else:
        require(expected == actual == nargs == argc == 0, "type diagnostic retained unused payload fields")

    primary, secondary = (start, end), (secondary_start, secondary_end)
    rows = list(enumerate(structure.rows, 1))

    def extent(ref):
        return resolution._extent(structure.row(ref))

    def identifier(ref):
        row = structure.row(ref)
        return extent(ref) if row["kind"] in (1, 2) else resolution._at(tokens[row["a"] - 1])

    def chain(ref):
        refs = []
        while ref:
            refs.append(ref)
            ref = structure.row(ref)["next"]
        return refs

    def declared(kinds):
        return [ref for ref, row in rows if row["kind"] in kinds and identifier(ref) == secondary]

    def same_name(left, right):
        # Verify the two explicitly supplied diagnostic spans; never search
        # lexical scopes or choose a binding for an unobserved reference.
        a, b = identifier(left), identifier(right)
        return source[a[0]:a[1]] == source[b[0]:b[1]]

    def local_secondary(statement, kinds):
        return [ref for ref in declared(kinds) if structure.owners[ref] == structure.owners[statement]
                and same_name(statement, ref)]

    if kind == 8:
        contexts = []
        for ref, row in rows:
            role = row["kind"]
            if label == 0:
                operands = []
                if role in (22, 23, 13, 14):
                    operands = [row["a"]]
                elif role in syntax.BINARY:
                    operands = [row["b"]] if role in (29, 30) else [row["a"], row["b"]]
                elif role == 10:
                    operands = [row["a"]] if row["a"] else [ref]
                contexts.extend((ref, operand) for operand in operands if extent(operand) == primary)
            elif label == 2 and role == 20:
                arguments = chain(row["b"])
                for declaration in declared({1}):
                    parameters = chain(structure.row(declaration)["b"])
                    if same_name(ref, declaration) and len(arguments) == len(parameters):
                        contexts.extend((ref, arg) for arg in arguments if extent(arg) == primary)
            elif label == 3:
                if role in (6, 7) and row["b"] and extent(row["c"]) == primary and identifier(ref) == secondary:
                    contexts.append((ref, row["c"]))
                elif role == 8 and extent(row["c"]) == primary and local_secondary(ref, {7}):
                    contexts.append((ref, row["c"]))
        require(label in (0, 2, 3) and contexts, "type mismatch span/secondary does not match a constraint context")
        code, message = "E0300", "type mismatch: expected %s, found %s" % (TYPES[expected], TYPES[actual])
    elif kind == 9:
        require(label == 0 and any(row["kind"] in (29, 30) and extent(row["a"]) == primary for _, row in rows),
                "unit equality diagnostic requires an equality left operand and no secondary")
        code, message = "E0300", "equality requires i32 or bool operands, found ()"
    elif kind == 10:
        contexts = [(ref, declaration) for ref, row in rows if row["kind"] == 20 and extent(ref) == primary
                    for declaration in declared({1}) if same_name(ref, declaration)
                    and len(chain(row["b"])) == argc
                    and len(chain(structure.row(declaration)["b"])) == nargs]
        require(label == 2 and contexts, "arity diagnostic requires a call, its function secondary and structural counts")
        code, message = "E0301", "wrong argument count: expected %s, found %s" % (nargs, argc)
    elif kind == 11:
        endings = [resolution._at(function["end"]) for function in structure.ast["functions"]]
        require(label == 0 and primary in endings, "missing-return diagnostic requires a function closing brace")
        code, message = "E0302", "function requires an explicit terminal return"
    elif kind in (12, 13):
        following = [row["next"] for _, row in rows if 6 <= row["kind"] <= 14 and row["next"]]
        require(label == 0 and any(extent(ref) == primary for ref in following),
                "unreachable diagnostic requires a whole following statement and no secondary")
        code = "E0303"
        message = "statement after terminal %s is unavailable in typed-preview" % ("return" if kind == 12 else "control transfer")
    else:
        contexts = [ref for ref, row in rows if row["kind"] == 8 and identifier(ref) == primary
                    and local_secondary(ref, {2, 6})]
        require(label == 4 and contexts, "immutable assignment requires its target and immutable declaration secondary")
        code, message = "E0304", "assignment requires a mutable local"

    diagnostic = syntax.diagnostic(source, code, "type", message, start, end)
    if label:
        secondary_span = syntax.diagnostic(source, code, "type", "", secondary_start, secondary_end)["primary"]
        diagnostic["secondary"] = [{"span": secondary_span, "message": {
            2: "function declared here", 3: "binding declared here", 4: "immutable binding declared here",
        }[label]}]
    return {"schema": SCHEMA, "status": "diagnostic", "phase": "type", "diagnostic": diagnostic}


def decode(raw, source, tokens=None):
    """Decode complete tag-0 facts, first static failure, or inherited syntax failure.

    Tag 2 is always pending and is never accepted as a typed observation. The
    complete independent successful token tape is required after syntax success.
    """
    require(isinstance(source, bytes) and len(source) <= 128 and source.isascii(), "source outside bounded ASCII domain")
    require(isinstance(raw, bytes) and len(raw) >= 11 and raw[:4] == b"OPA1", "invalid OPA1 header")
    if raw[4]:
        result = resolution.decode(raw, source, tokens)
        result["schema"] = SCHEMA
        return result
    require(len(raw) >= HEADER_END, "missing STF1 static suffix")
    syntax.validate_tokens(tokens, source)
    wire = syntax.decode_wire(raw[:OPA_SIZE], source)
    ast = syntax.project_ast(wire, source, tokens)["ast"]
    header = raw[OPA_SIZE:HEADER_END]
    require(header[:4] == b"STF1" and header[5] == len(wire["rows"]), "invalid STF1 marker or row count")
    require(header[4] in (0, 1), "pending/probe tag is not complete static success")
    structure = resolution.Structure(wire, ast)
    if header[4] == 1:
        require(len(raw) == HEADER_END, "static failure retained partial facts or trailing bytes")
        return _project_diagnostic(header, structure, source, tokens)
    require(len(raw) == TOTAL_SIZE and not any(header[6:]), "invalid complete static-success framing")
    facts = resolution._column(raw[HEADER_END:RESOLUTION_END])
    semantic = resolution._column(raw[RESOLUTION_END:])
    _validate_facts(structure, facts, semantic)
    require(not resolution._owned_route(wire["rows"], source), "unknown-type public route cannot publish scalar typed HIR")
    return {"schema": SCHEMA, "status": "ok",
            "route": "project_scalar" if any(f["public"] is not None for f in ast["functions"]) else "scalar",
            "ast": ast, "typed_hir": _project_hir(structure, facts, semantic, source)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--observation", type=Path, required=True)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--tokens", type=Path, help="complete independent canonical token tape; required after syntax success")
    args = parser.parse_args()
    try:
        tokens = None
        if args.tokens:
            observed = json.loads(args.tokens.read_text())
            require(isinstance(observed, dict) and observed.get("status") == "ok", "token observer must have succeeded")
            tokens = observed.get("tokens", observed.get("ast", {}).get("tokens"))
        result = decode(args.observation.read_bytes(), args.source.read_bytes(), tokens)
    except (OSError, ValueError) as error:
        parser.exit(2, "invalid static observation: " + str(error) + "\n")
    print(json.dumps(result, sort_keys=True))


if __name__ == "__main__":
    main()
