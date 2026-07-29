use "../strings.ox";

fn c_header(name) { return "#include <" + name + ">"; }
fn c_library(name) { return "-l" + name; }
fn c_symbol(name) { return "extern " + name; }
fn c_call(symbol, signature) { return "c call " + symbol + " : " + signature; }
fn c_export(symbol, signature) { return "c export " + symbol + " : " + signature; }
fn c_bind(symbol, signature) { return c_call(symbol, signature) + " | " + c_export(symbol, signature); }
fn c_ffi_hint(symbol) { return "bind " + symbol + " through native header or shared library"; }

fn c_bridge_summary() {
    return join_lines([
        c_header("stdint.h"),
        c_header("stddef.h"),
        c_library("m"),
        c_bind("oxid_c_strlen", "size_t(const char*)"),
        c_bind("oxid_c_hash", "uint64_t(const char*)")
    ], ", ");
}
