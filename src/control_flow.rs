//! Truthiness-normalizing control-flow transforms. They do not reorder blocks or evaluate conditions twice.
use crate::lexer::Token;
/// Wrap eligible if/while/repeat conditions in `not not (...)`. Lua conditionals use truthiness,
/// so this preserves side effects and branch choice while producing an equivalent explicit boolean.
pub fn transform(tokens: &mut Vec<Token>) -> usize {
    let mut edits = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        let (endword, allow_then) = match tokens[i].text.as_str() {
            "if" => ("then", false),
            "while" => ("do", true),
            _ => {
                i += 1;
                continue;
            }
        };
        let mut j = i + 1;
        let mut depth = 0i32;
        let mut found = None;
        while j < tokens.len() {
            let x = tokens[j].text.as_str();
            if matches!(x, "(" | "[" | "{") {
                depth += 1
            } else if matches!(x, ")" | "]" | "}") {
                depth -= 1
            }
            if depth == 0 && (x == endword || (allow_then && x == "then")) {
                found = Some(j);
                break;
            }
            j += 1
        }
        if let Some(e) = found {
            if e > i + 1 {
                edits.push((i + 1, e));
            }
        }
        i += 1;
    }
    for (a, b) in edits.iter().rev() {
        let mut p = crate::lexer::lex("not not (").unwrap();
        let mut q = crate::lexer::lex(")").unwrap();
        p.insert(
            0,
            crate::lexer::Token {
                kind: crate::lexer::Kind::Space,
                text: " ".into(),
                line: 0,
            },
        );
        q.push(crate::lexer::Token {
            kind: crate::lexer::Kind::Space,
            text: " ".into(),
            line: 0,
        });
        tokens.splice(*b..*b, q.drain(..));
        tokens.splice(*a..*a, p.drain(..));
    }
    edits.len()
}
