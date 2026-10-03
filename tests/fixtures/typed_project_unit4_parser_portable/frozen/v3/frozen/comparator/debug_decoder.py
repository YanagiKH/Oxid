"""Independent Rust Debug grammar decoder used only to bind raw evidence.

It does not inspect language source, expected results, or producer helpers.
"""
import re


class DecodeError(ValueError):
    pass


TOKEN = re.compile(r'\s*(\.\.|-?[0-9]+|[A-Za-z_][A-Za-z_0-9]*|[\[\]{}():,]|"(?:[^"\\]|\\.)*"|\'(?:[^\'\\]|\\.)*\')', re.S)


def decode_string(token):
    output, index = [], 1
    escapes = {"n": "\n", "r": "\r", "t": "\t", "0": "\0", "\\": "\\", '"': '"', "'": "'"}
    while index < len(token) - 1:
        char = token[index]
        index += 1
        if char != "\\":
            output.append(char)
            continue
        char = token[index]
        index += 1
        if char in escapes:
            output.append(escapes[char])
        elif char == "x":
            output.append(chr(int(token[index:index+2], 16)))
            index += 2
        elif char == "u" and token[index] == "{":
            end = token.index("}", index)
            output.append(chr(int(token[index+1:end], 16)))
            index = end + 1
        else:
            raise DecodeError("unsupported string escape")
    return "".join(output)


def decode(source):
    tokens, cursor = [], 0
    while cursor < len(source):
        match = TOKEN.match(source, cursor)
        if not match:
            if source[cursor:].strip():
                raise DecodeError(f"unrecognized Debug token at {cursor}")
            break
        tokens.append(match[1]); cursor = match.end()
    tokens.append("$END")
    index = 0
    def take(expected=None):
        nonlocal index
        token = tokens[index]
        if expected is not None and token != expected:
            raise DecodeError(f"expected {expected}, got {token}")
        index += 1
        return token
    def sequence(end):
        result = []
        while tokens[index] != end:
            result.append(value())
            if tokens[index] == ",":
                take(",")
            elif tokens[index] != end:
                raise DecodeError("missing sequence comma")
        take(end)
        return result
    def value():
        token = take()
        if token == "None":
            return None
        if token in {"true", "false"}:
            return token == "true"
        if token[0] in "\"'":
            return decode_string(token)
        if re.fullmatch(r"-?[0-9]+", token):
            return int(token)
        if token == "[":
            return sequence("]")
        if token == "(":
            return sequence(")")
        if token == "{":
            entries = []
            while tokens[index] != "}":
                key = value(); take(":"); entries.append([key, value()])
                if tokens[index] == ",":
                    take(",")
                elif tokens[index] != "}":
                    raise DecodeError("missing map comma")
            take("}")
            return {"tag": "$map", "items": entries}
        if not re.fullmatch(r"[A-Za-z_][A-Za-z_0-9]*", token):
            raise DecodeError("invalid tag")
        result = {"tag": token}
        if tokens[index] == "(":
            take("("); result["items"] = sequence(")")
        elif tokens[index] == "{":
            take("{")
            while tokens[index] != "}":
                key = take()
                if key == "..":
                    if token != "SourceProvenance":
                        raise DecodeError("unqualified omitted fields")
                    result["debug_non_exhaustive"] = True
                else:
                    if not re.fullmatch(r"[A-Za-z_][A-Za-z_0-9]*", key) or key in result:
                        raise DecodeError("duplicate or invalid struct field")
                    take(":"); result[key] = value()
                if tokens[index] == ",":
                    take(",")
                elif tokens[index] != "}":
                    raise DecodeError("missing struct comma")
            take("}")
        return result
    result = value()
    take("$END")
    if index != len(tokens):
        raise DecodeError("trailing Debug data")
    return result
