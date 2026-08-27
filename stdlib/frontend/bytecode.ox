fn bytecode_format_version() => 1;
fn ast_format_version() => 1;

fn canonical_compiler_manifest() {
    return {
        schema: 1,
        compiler: "oxid-self-host",
        bytecode_format: bytecode_format_version(),
        ast_format: ast_format_version(),
        providers: {
            emitter: "oxid",
            lexer: "stage0",
            parser: "stage0",
            diagnostics: "stage0",
            modules: "stage0"
        },
        parity_gate: "byte-for-byte",
        parity_required_before_switch: true
    };
}

fn canonical_compiler_manifest_json() {
    return json_stringify(canonical_compiler_manifest());
}

fn bytecode_header() {
    return "OXBC|" + str(bytecode_format_version()) + "|AST|" + str(ast_format_version());
}

fn require_instruction_field(instruction, field, index) {
    assert(type_of(instruction) == "record", "instruction " + str(index) + " must be a record");
    assert(has_key(instruction, field), "instruction " + str(index) + " is missing `" + field + "`");
    return get(instruction, field);
}

fn require_string_operand(instruction, field, index) {
    const value = require_instruction_field(instruction, field, index);
    assert(type_of(value) == "string", "instruction " + str(index) + " field `" + field + "` must be a string");
    return value;
}

fn require_number_operand(instruction, field, index) {
    const value = require_instruction_field(instruction, field, index);
    assert(type_of(value) == "number", "instruction " + str(index) + " field `" + field + "` must be a number");
    return value;
}

fn canonical_opcode(op, index) {
    if op == "const" { return "CONST"; }
    if op == "load" { return "LOAD"; }
    if op == "store" { return "STORE"; }
    if op == "call" { return "CALL"; }
    if op == "jump" { return "JUMP"; }
    if op == "jump_if_false" { return "JUMP_IF_FALSE"; }
    if op == "nop" { return "NOP"; }
    if op == "pop" { return "POP"; }
    if op == "add" { return "ADD"; }
    if op == "sub" { return "SUB"; }
    if op == "mul" { return "MUL"; }
    if op == "div" { return "DIV"; }
    if op == "equal" { return "EQUAL"; }
    if op == "less" { return "LESS"; }
    if op == "print" { return "PRINT"; }
    if op == "return" { return "RETURN"; }
    if op == "halt" { return "HALT"; }
    assert(false, "instruction " + str(index) + " has unsupported opcode `" + op + "`");
    return "";
}

fn canonical_operand(op, instruction, index) {
    if op == "const" {
        return require_instruction_field(instruction, "value", index);
    }
    if op == "load" or op == "store" {
        return require_string_operand(instruction, "name", index);
    }
    if op == "call" {
        const name = require_string_operand(instruction, "name", index);
        const argc = require_number_operand(instruction, "argc", index);
        assert(argc >= 0, "instruction " + str(index) + " field `argc` cannot be negative");
        return {name: name, argc: argc};
    }
    if op == "jump" or op == "jump_if_false" {
        const target = require_number_operand(instruction, "target", index);
        assert(target >= 0, "instruction " + str(index) + " field `target` cannot be negative");
        return target;
    }
    return null;
}

fn emit_instruction(index, instruction) {
    const op = require_string_operand(instruction, "op", index);
    const opcode = canonical_opcode(op, index);
    const operand = canonical_operand(op, instruction, index);
    return str(index) + "|" + opcode + "|" + json_stringify(operand);
}

fn emit_bytecode(instructions) {
    assert(type_of(instructions) == "array", "bytecode emitter requires an instruction array");
    var lines = [bytecode_header()];
    var index = 0;
    for instruction in instructions {
        push(lines, emit_instruction(index, instruction));
        index = index + 1;
    }
    return join_text(lines, "\n") + "\n";
}
