use "../stdlib/frontend/bytecode.ox";

fn main() {
    const instructions = [
        {op: "const", value: {z: [1, true, null], a: "record"}},
        {op: "store", name: "payload"},
        {op: "load", name: "payload"},
        {op: "call", name: "consume", argc: 1},
        {op: "jump_if_false", target: 7},
        {op: "print"},
        {op: "return"},
        {op: "halt"}
    ];
    const expected = "OXBC|1|AST|1\n0|CONST|{\"a\":\"record\",\"z\":[1,true,null]}\n1|STORE|\"payload\"\n2|LOAD|\"payload\"\n3|CALL|{\"argc\":1,\"name\":\"consume\"}\n4|JUMP_IF_FALSE|7\n5|PRINT|null\n6|RETURN|null\n7|HALT|null\n";
    const emitted = emit_bytecode(instructions);
    assert(emitted == expected, "bytecode emitter output differs from the canonical byte stream");

    const source_record = {z: [1, true, null], a: "record"};
    const encoded = json_stringify(source_record);
    assert(encoded == "{\"a\":\"record\",\"z\":[1,true,null]}", "record JSON must use canonical key ordering");
    const decoded = json_parse(encoded);
    assert(decoded.a == "record", "record property access failed after JSON parsing");
    assert(json_stringify(decoded) == encoded, "JSON record roundtrip changed canonical bytes");

    const manifest = canonical_compiler_manifest();
    assert(manifest.bytecode_format == 1, "bytecode format version changed unexpectedly");
    assert(manifest.ast_format == 1, "AST format version changed unexpectedly");
    assert(manifest.providers.emitter == "oxid", "compiler manifest emitter ownership is incorrect");
}
