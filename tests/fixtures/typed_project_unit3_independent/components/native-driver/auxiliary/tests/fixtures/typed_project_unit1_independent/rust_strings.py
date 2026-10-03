"""Render a Unicode Python string as a Rust string literal, without JSON escapes."""
def rust_string_literal(value):
    value.encode('utf-8')  # Invoked display paths must be representable UTF-8.
    escape={'\\':'\\\\','"':'\\"','\n':'\\n','\r':'\\r','\t':'\\t','\0':'\\0'}
    return '"'+''.join(escape.get(c,('\\u{'+format(ord(c),'x')+'}') if ord(c)<32 or ord(c)==127 else c) for c in value)+'"'
