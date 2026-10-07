#!/usr/bin/env python3
"""Strict, host-only projection of complete bounded resolver observations.

OPA1 structure and the independently observed token tape are validated by the
unchanged parser adapter. STF1 tag 2 means resolution finished with typing still
pending only when every resolver row role below is satisfied. The permanent
synthetic carrier probe is not a resolver observation. This adapter never looks
up a name, converts a decimal literal, infers a type, or computes control flow.
Candidate-supplied bindings, primitive codes, values and loop targets survive
projection unchanged; canonical differential comparison establishes correctness.
"""
import argparse
import copy
import json
from pathlib import Path

import parser_ast_observation as syntax


ObservationError = syntax.ObservationError
require = syntax.require
SCHEMA = "canonical-resolution-observation-1"
OPA_SIZE, HEADER_END, RESOLUTION_END, TOTAL_SIZE = 1559, 1575, 2091, 2607
TYPES = {1: "bool", 2: "i32", 3: "()"}
LOCAL_KINDS = {2, 6, 7}


def _extent(row):
    return row["start"], row["end"]


def _at(value):
    return value["start"], value["end"]


class Structure:
    """Index only validated AST ownership and canonical structural ordering."""

    def __init__(self, wire, ast):
        self.rows, self.ast = wire["rows"], ast
        self.owners, self.functions, self.locals, self.blocks, self.expressions = {}, [], {}, {}, {}
        self.while_bodies = set()
        ref = wire["items"]
        while ref:
            owner = len(self.functions)
            self.functions.append(ref)
            self.locals[owner], self.blocks[owner], self.expressions[owner] = [], [], []
            function = self.row(ref)
            self.owners[ref] = owner
            parameter = function["b"]
            while parameter:
                self.owners[parameter] = owner
                self.owners[self.row(parameter)["a"]] = owner
                self.locals[owner].append(parameter)
                parameter = self.row(parameter)["next"]
            self.owners[function["c"]] = owner
            self._block(function["d"], owner)
            ref = function["next"]
        require(len(self.owners) == len(self.rows), "structural index omitted validated rows")
        self.local_ids = {ref: index for refs in self.locals.values() for index, ref in enumerate(refs)}
        self.block_ids = {ref: index for refs in self.blocks.values() for index, ref in enumerate(refs)}
        self.expression_ids = {ref: index for refs in self.expressions.values() for index, ref in enumerate(refs)}
        self.expression_syntax = {_at(value["span"]): value for value in ast["expressions"]}
        self.statement_syntax = {_at(value["span"]): value for function in ast["functions"]
                                 for block in function["blocks"] for value in block["body"]}

    def row(self, ref):
        return self.rows[ref - 1]

    def _expression(self, ref, owner):
        row = self.row(ref)
        self.owners[ref] = owner
        kind = row["kind"]
        if kind == 20:
            argument = row["b"]
            while argument:
                self._expression(argument, owner)
                argument = self.row(argument)["next"]
        elif kind in (21, 22, 23):
            self._expression(row["a"], owner)
        elif kind in syntax.BINARY:
            self._expression(row["a"], owner)
            self._expression(row["b"], owner)
        self.expressions[owner].append(ref)

    def _block(self, ref, owner):
        self.owners[ref] = owner
        self.blocks[owner].append(ref)
        statement = self.row(ref)["b"]
        while statement:
            row = self.row(statement)
            self.owners[statement] = owner
            kind = row["kind"]
            if kind in (6, 7):
                if row["b"]:
                    self.owners[row["b"]] = owner
                self._expression(row["c"], owner)
                self.locals[owner].append(statement)
            elif kind == 8:
                self._expression(row["c"], owner)
            elif kind in (9, 10) and row["a"]:
                self._expression(row["a"], owner)
            elif kind in (13, 14):
                self._expression(row["a"], owner)
                if kind == 14:
                    self.while_bodies.add(row["b"])
                self._block(row["b"], owner)
                if kind == 13 and row["c"]:
                    self._block(row["c"], owner)
            statement = row["next"]


def _column(raw):
    require(len(raw) == 516, "incomplete fact column")
    values = []
    for cell in range(129):
        bits = sum(raw[plane * 129 + cell] << (8 * plane) for plane in range(4))
        values.append(bits - (1 << 32) if bits & (1 << 31) else bits)
    return values


def _validate_facts(structure, facts, semantic):
    require(not any(semantic), "resolution-only observations require an all-zero semantic column")
    require(not any(facts[len(structure.rows):]), "nonzero inactive or sentinel resolution cells")
    function_ids = {ref: index for index, ref in enumerate(structure.functions)}
    for ref, row in enumerate(structure.rows, 1):
        kind, value = row["kind"], facts[ref - 1]
        if kind == 1:
            require(value == function_ids[ref] + 1, "Function fact is not its dense DefId + 1")
        elif kind in LOCAL_KINDS:
            require(value == structure.local_ids[ref] + 1, "local fact is not its dense LocalId + 1")
        elif kind in (3, 4):
            require(value in TYPES, "missing or invalid primitive type fact")
        elif kind in (8, 19, 20, 11, 12):
            require(1 <= value <= len(structure.rows), "resolution target is outside active rows")
            target = structure.row(value)
            if kind in (8, 19):
                require(target["kind"] in LOCAL_KINDS and structure.owners[value] == structure.owners[ref],
                        "local reference requires a declaration in the same function")
            elif kind == 20:
                require(target["kind"] == 1, "call reference requires a Function row")
            else:
                require(target["kind"] == 5 and value in structure.while_bodies
                        and structure.owners[value] == structure.owners[ref],
                        "loop reference requires a while-body Block in the same function")
        elif kind != 15:
            require(value == 0, "nonzero resolution fact for an unused row role")


def _project_hir(structure, facts, source):
    def value(ref):
        return facts[ref - 1]

    def primitive(ref):
        return TYPES[value(ref)]

    def text(at):
        return source[at["start"]:at["end"]].decode("ascii")

    functions = []
    for owner, ref in enumerate(structure.functions):
        row, ast = structure.row(ref), structure.ast["functions"][owner]
        local_refs, expression_refs = structure.locals[owner], structure.expressions[owner]
        expressions, locals_, params = [], [], []
        # The syntax observer's expression IDs are program-wide; HIR IDs restart
        # per function. This renumbering follows structure, never resolution.
        expression_ids = {structure.expression_syntax[_extent(structure.row(r))]["id"]: index
                          for index, r in enumerate(expression_refs)}
        for local_ref in local_refs:
            local = structure.row(local_ref)
            if local["kind"] == 2:
                at, annotation, mutable = syntax.span(*_extent(local)), primitive(local["a"]), False
                params.append(annotation)
            else:
                statement = structure.statement_syntax[_extent(local)]
                at, mutable = statement["name"], statement["mutable"]
                annotation = primitive(local["b"]) if local["b"] else None
            locals_.append({"id": value(local_ref) - 1, "name": text(at), "mutable": mutable,
                            "span": at, "annotation": annotation})
        for expression_ref in expression_refs:
            expression = copy.deepcopy(structure.expression_syntax[_extent(structure.row(expression_ref))])
            expression["id"] = structure.expression_ids[expression_ref]
            kind = expression["kind"]
            if kind == "Number":
                expression.pop("digits")
                expression.pop("negative")
                expression.update(kind="I32", value=value(expression_ref))
            elif kind == "Name":
                expression.pop("name")
                expression.update(kind="Local", local=value(value(expression_ref)) - 1)
            elif kind == "Call":
                expression.pop("callee")
                expression["target"] = value(value(expression_ref)) - 1
                expression["args"] = [expression_ids[arg] for arg in expression["args"]]
            for field in ("operand", "left", "right"):
                if field in expression:
                    expression[field] = expression_ids[expression[field]]
            expressions.append(expression)
        blocks = []
        for block_ref in structure.blocks[owner]:
            block_id = structure.block_ids[block_ref]
            block = copy.deepcopy(ast["blocks"][block_id])
            statement_ref = structure.row(block_ref)["b"]
            for statement in block["body"]:
                kind = statement["kind"]
                if kind == "Let":
                    for field in ("mutable", "name", "annotation"):
                        statement.pop(field)
                    statement["local"] = value(statement_ref) - 1
                elif kind == "Assign":
                    statement["target_span"] = statement.pop("name")
                    statement["local"] = value(value(statement_ref)) - 1
                elif kind in ("Break", "Continue"):
                    statement["target"] = structure.block_ids[value(statement_ref)]
                elif kind == "While":
                    statement["loop_id"] = statement["body"]
                for field in ("init", "value", "condition"):
                    if field in statement and statement[field] is not None:
                        statement[field] = expression_ids[statement[field]]
                statement_ref = structure.row(statement_ref)["next"]
            blocks.append(block)
        functions.append({"id": value(ref) - 1, "name": text(ast["name"]),
                          "signature": {"span": ast["name"], "params": params, "result": primitive(row["c"])},
                          "body": ast["body"], "end": ast["end"], "locals": locals_,
                          "expressions": expressions, "blocks": blocks})
    return {"functions": functions}


def _owned_route(rows, source):
    # This is only the public syntax-route discriminator. Type facts are never
    # filled from these spellings, and the adapter grants no source authority.
    return any(row["kind"] == 3 and source[row["start"]:row["end"]] not in (b"bool", b"i32")
               for row in rows)


def _project_diagnostic(header, structure, source, tokens):
    kind, start, end, secondary_start, secondary_end, label = header[6:12]
    require(1 <= kind <= 7, "not a resolver diagnostic kind")
    require(0 <= start < end <= len(source), "invalid resolver diagnostic primary span")
    require(not any(header[12:]), "resolver diagnostic retained unused type or arity fields")
    primary = start, end
    rows = structure.rows

    def identifier(row):
        if row["kind"] in (1, 2, 3):
            return _extent(row)
        return _at(tokens[row["a"] - 1])

    if kind == 3:
        require(label == 1 and 0 <= secondary_start < secondary_end <= len(source),
                "duplicate binding requires its first-declaration secondary")
        secondary = secondary_start, secondary_end
        declarations = [(ref, row) for ref, row in enumerate(rows, 1) if row["kind"] in (1, 2, 6, 7)]
        later = [(ref, row) for ref, row in declarations if identifier(row) == primary]
        earlier = [(ref, row) for ref, row in declarations if identifier(row) == secondary]
        require(len(later) == len(earlier) == 1 and primary != secondary,
                "duplicate diagnostic spans must identify distinct declarations")
        later_ref, later_row = later[0]
        earlier_ref, earlier_row = earlier[0]
        require(source[start:end] == source[secondary_start:secondary_end],
                "duplicate diagnostic names do not match")
        if later_row["kind"] == 1:
            require(earlier_row["kind"] == 1 and structure.owners[earlier_ref] < structure.owners[later_ref],
                    "duplicate function secondary must be an earlier function")
        elif earlier_row["kind"] != 1:
            require(structure.owners[earlier_ref] == structure.owners[later_ref]
                    and structure.local_ids[earlier_ref] < structure.local_ids[later_ref],
                    "duplicate local secondary must be an earlier local in the same function")
        code, message = "E0201", "duplicate binding; shadowing is unavailable in typed-preview"
    else:
        require(secondary_start == secondary_end == label == 0,
                "resolver diagnostic retained an unused secondary")
        allowed = {1: {8, 19}, 2: {20}, 4: {3}, 5: {15}, 6: {11}, 7: {12}}[kind]
        contexts = [row for row in rows if row["kind"] in allowed
                    and (identifier(row) if kind in (1, 2, 4) else _extent(row)) == primary]
        require(len(contexts) == 1, "resolver diagnostic primary does not match its AST context")
        name = source[start:end].decode("ascii")
        if kind in (1, 2):
            code = "E0200"
            message = "unknown " + ("local" if kind == 1 else "direct function") + " `" + name + "`"
        elif kind == 4:
            require(source[start:end] not in (b"bool", b"i32"), "unknown type diagnostic names a primitive")
            code, message = "E0202", "unknown type `" + (name[:61] + "..." if len(name) > 64 else name) + "`"
        elif kind == 5:
            code, message = "E0203", "decimal literal is outside the i32 range [-2147483648, 2147483647]"
        else:
            code = "E0204"
            message = "`" + ("break" if kind == 6 else "continue") + "` requires an enclosing while in the same function"
    diagnostic = syntax.diagnostic(source, code, "resolve", message, start, end)
    if kind == 3:
        secondary_span = syntax.diagnostic(source, code, "resolve", "", secondary_start, secondary_end)["primary"]
        diagnostic["secondary"] = [{"span": secondary_span, "message": "first declared here"}]
    result = {"schema": SCHEMA, "status": "diagnostic", "phase": "resolve", "diagnostic": diagnostic}
    if _owned_route(rows, source):
        result["route"] = "public_owned"
    return result


def decode(raw, source, tokens=None):
    """Project complete resolution, exact first failure, or inherited syntax failure.

    Exact source bytes and the complete independently observed token tape are
    mandatory for syntax-success observations. Inherited eleven-byte failures
    retain the parser adapter's optional-token behavior, including lexer failures
    for which no complete successful token tape exists. Never emits typed_hir.
    """
    require(isinstance(source, bytes) and len(source) <= 128 and source.isascii(),
            "source outside bounded ASCII domain")
    require(isinstance(raw, bytes) and len(raw) >= 11 and raw[:4] == b"OPA1", "invalid OPA1 header")
    if raw[4]:
        result = syntax.decode(raw, source, tokens)
        if "diagnostic" in result:
            return {"schema": SCHEMA, "status": result["status"],
                    "phase": result["diagnostic"]["stage"], "diagnostic": result["diagnostic"]}
        require(result["status"] == "domain_refusal", "incomplete parser stage is not a resolution observation")
        return {"schema": SCHEMA, "status": "outside_subset", "family": result["family"], "span": result["span"]}
    require(len(raw) >= HEADER_END, "missing STF1 resolver suffix")
    syntax.validate_tokens(tokens, source)
    wire = syntax.decode_wire(raw[:OPA_SIZE], source)
    ast = syntax.project_ast(wire, source, tokens)["ast"]
    header = raw[OPA_SIZE:HEADER_END]
    require(header[:4] == b"STF1" and header[5] == len(wire["rows"]), "invalid STF1 marker or row count")
    require(header[4] in (1, 2), "tag 0 static success is not resolver-pending typing")
    structure = Structure(wire, ast)
    if header[4] == 1:
        require(len(raw) == HEADER_END, "resolver failure retained partial facts or trailing bytes")
        return _project_diagnostic(header, structure, source, tokens)
    require(len(raw) == TOTAL_SIZE and not any(header[6:]), "invalid complete resolver-success framing")
    facts, semantic = _column(raw[HEADER_END:RESOLUTION_END]), _column(raw[RESOLUTION_END:])
    _validate_facts(structure, facts, semantic)
    require(not _owned_route(wire["rows"], source), "unknown-type public route cannot publish scalar resolved HIR")
    return {"schema": SCHEMA, "status": "resolved", "phase": "resolve", "typing": "pending",
            "route": "project_scalar" if any(f["public"] is not None for f in ast["functions"]) else "scalar",
            "ast": ast, "resolved_hir": _project_hir(structure, facts, source)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--observation", type=Path, required=True)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--tokens", type=Path,
                        help="complete independent canonical token tape; required after syntax success")
    args = parser.parse_args()
    try:
        tokens = None
        if args.tokens:
            observed = json.loads(args.tokens.read_text())
            require(isinstance(observed, dict) and observed.get("status") == "ok", "token observer must have succeeded")
            tokens = observed.get("tokens", observed.get("ast", {}).get("tokens"))
        result = decode(args.observation.read_bytes(), args.source.read_bytes(), tokens)
    except (OSError, ValueError) as error:
        parser.exit(2, "invalid resolution observation: " + str(error) + "\n")
    print(json.dumps(result, sort_keys=True))


if __name__ == "__main__":
    main()
