//! Visibility adapter only; copied canonical files are unmodified.
mod diagnostic;
mod lexer;
mod source;
mod project {
    pub(super) mod budget;
}

pub(crate) fn observe(text: String) {
    let mut sources = source::SourceMap::new();
    let mut allocator = project::budget::Allocator::default();
    let id = sources
        .try_add("stdin.ox".into(), text, &mut allocator)
        .expect("bounded observer source allocation failed");
    match lexer::lex_with_limit(sources.get(id), lexer::MAX_TOKENS) {
        Ok(tokens) => {
            print!("{{\"status\":\"ok\",\"tokens\":[");
            for (index, token) in tokens.iter().enumerate() {
                if index != 0 {
                    print!(",");
                }
                print!(
                    "{{\"id\":{},\"kind\":\"{:?}\",\"file_id\":{},\"start\":{},\"end\":{}}}",
                    token.kind as usize + 1,
                    token.kind,
                    token.span.file.0,
                    token.span.start,
                    token.span.end
                );
            }
            println!("]}}");
        }
        Err(diagnostic) => {
            println!(
                "{{\"status\":\"diagnostic\",\"diagnostic\":{}}}",
                diagnostic.render_json(&sources)
            );
        }
    }
}
