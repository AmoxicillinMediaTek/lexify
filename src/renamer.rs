//! Conservative lexical scope analysis for Lua locals, parameters and loop variables.
use crate::lexer::{Kind, Token};
use rand::{Rng, RngCore};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, PartialEq, Eq)]
enum ScopeKind {
    Root,
    Function,
    Block,
    IfBranch,
    Repeat,
}
struct Scope {
    parent: Option<usize>,
    kind: ScopeKind,
}
struct Binding {
    name: String,
    scope: usize,
    start: usize,
    declaration: Vec<usize>,
    replacement: String,
}

pub fn fresh_identifier(rng: &mut impl RngCore, used: &mut HashSet<String>) -> String {
    // Six glyphs per position gives 6^6..6^11 distinct names, so realistic files never starve the
    // pool. The counter fallback keeps the loop terminating even if the random draws run dry.
    const ALPHABET: &[u8] = b"Il1O0o";
    for _ in 0..64 {
        let mut s = String::from("_");
        for _ in 0..rng.gen_range(6..=11) {
            s.push(ALPHABET[rng.gen_range(0..ALPHABET.len())] as char)
        }
        if used.insert(s.clone()) {
            return s;
        }
    }
    let mut index = 0u64;
    loop {
        let s = format!("_lx{index:x}");
        if used.insert(s.clone()) {
            return s;
        }
        index += 1;
    }
}
fn is_kw(s: &str) -> bool {
    matches!(
        s,
        "and"
            | "break"
            | "do"
            | "else"
            | "elseif"
            | "end"
            | "false"
            | "for"
            | "function"
            | "goto"
            | "if"
            | "in"
            | "local"
            | "nil"
            | "not"
            | "or"
            | "repeat"
            | "return"
            | "then"
            | "true"
            | "until"
            | "while"
            | "continue"
            | "type"
            | "export"
    )
}
fn matching(tokens: &[Token], open: usize, left: &str, right: &str) -> Option<usize> {
    let mut d = 0;
    for (i, token) in tokens.iter().enumerate().skip(open) {
        if token.text == left {
            d += 1
        } else if token.text == right {
            d -= 1;
            if d == 0 {
                return Some(i);
            }
        }
    }
    None
}
fn ancestor(mut s: usize, target: usize, scopes: &[Scope]) -> bool {
    loop {
        if s == target {
            return true;
        }
        match scopes[s].parent {
            Some(p) => s = p,
            None => return false,
        }
    }
}

/// Renames resolved local bindings; unresolved names, field keys, and global references are untouched.
pub fn transform(tokens: &mut [Token], rng: &mut impl RngCore) -> usize {
    let mut scopes = vec![Scope {
        parent: None,
        kind: ScopeKind::Root,
    }];
    let mut stack = vec![0usize];
    let mut token_scope = vec![0usize; tokens.len()];
    let mut fn_scopes = HashMap::new();
    let mut do_scopes = HashMap::new();
    for i in 0..tokens.len() {
        let text = tokens[i].text.as_str();
        if text == "end" {
            if stack.len() > 1 {
                stack.pop();
            }
        } else if text == "elseif" || text == "else" {
            if stack.len() > 1 && scopes[*stack.last().unwrap()].kind == ScopeKind::IfBranch {
                stack.pop();
            }
            if text == "else" {
                let id = scopes.len();
                scopes.push(Scope {
                    parent: Some(*stack.last().unwrap()),
                    kind: ScopeKind::IfBranch,
                });
                stack.push(id);
            }
        } else if text == "until" && stack.len() > 1 {
            stack.pop();
        }
        token_scope[i] = *stack.last().unwrap();
        match text {
            "function" => {
                let id = scopes.len();
                scopes.push(Scope {
                    parent: Some(*stack.last().unwrap()),
                    kind: ScopeKind::Function,
                });
                stack.push(id);
                fn_scopes.insert(i, id);
            }
            "then" => {
                let id = scopes.len();
                scopes.push(Scope {
                    parent: Some(*stack.last().unwrap()),
                    kind: ScopeKind::IfBranch,
                });
                stack.push(id);
            }
            "do" => {
                let id = scopes.len();
                scopes.push(Scope {
                    parent: Some(*stack.last().unwrap()),
                    kind: ScopeKind::Block,
                });
                stack.push(id);
                do_scopes.insert(i, id);
            }
            "repeat" => {
                let id = scopes.len();
                scopes.push(Scope {
                    parent: Some(*stack.last().unwrap()),
                    kind: ScopeKind::Repeat,
                });
                stack.push(id);
            }
            _ => {}
        }
        if text == "until"
            && stack.len() > 1
            && scopes[*stack.last().unwrap()].kind == ScopeKind::Repeat
        {
            stack.pop();
        }
    }
    let mut used: HashSet<String> = tokens
        .iter()
        .filter(|t| t.kind == Kind::Ident)
        .map(|t| t.text.clone())
        .collect();
    let mut bindings: Vec<Binding> = Vec::new();
    // Local declarations. Only simple statements are renamed; complex RHS spans are skipped safely.
    for i in 0..tokens.len() {
        if tokens[i].text != "local" {
            continue;
        }
        if tokens.get(i + 1).is_some_and(|t| t.text == "function") {
            if let Some(n) = tokens.get(i + 2).filter(|t| t.kind == Kind::Ident) {
                bindings.push(Binding {
                    name: n.text.clone(),
                    scope: token_scope[i],
                    start: i + 1,
                    declaration: vec![i + 2],
                    replacement: String::new(),
                });
            }
            continue;
        }
        let mut names = Vec::new();
        let mut j = i + 1;
        while j < tokens.len() && tokens[j].kind == Kind::Space {
            j += 1
        }
        while j < tokens.len() && tokens[j].kind == Kind::Ident && !is_kw(&tokens[j].text) {
            names.push(j);
            j += 1;
            while j < tokens.len() && tokens[j].kind == Kind::Space {
                j += 1;
            }
            if tokens.get(j).is_some_and(|t| t.text == ",") {
                j += 1;
                while j < tokens.len() && tokens[j].kind == Kind::Space {
                    j += 1;
                }
            } else {
                break;
            }
        }
        if names.is_empty() {
            continue;
        }
        let has_initializer = tokens.get(j).is_some_and(|t| t.text == "=");
        let start = if has_initializer {
            let mut depth = 0i32;
            let mut boundary = None;
            let mut k = j + 1;
            while k < tokens.len() {
                let x = tokens[k].text.as_str();
                if matches!(x, "(" | "{" | "[") {
                    depth += 1
                } else if matches!(x, ")" | "}" | "]") {
                    depth -= 1
                }
                if depth == 0 && x == ";" {
                    boundary = Some(k + 1);
                    break;
                }
                if depth == 0 && k > j + 1 && tokens[k].line > tokens[k - 1].line {
                    boundary = Some(k);
                    break;
                }
                k += 1;
            }
            let Some(boundary) = boundary else { continue };
            boundary
        } else {
            j
        };
        for n in names {
            bindings.push(Binding {
                name: tokens[n].text.clone(),
                scope: token_scope[i],
                start,
                declaration: vec![n],
                replacement: String::new(),
            });
        }
    }
    // Function parameters are scoped to their function body and naturally capture through nested scopes.
    let mut function_positions: Vec<_> = fn_scopes.iter().map(|(a, b)| (*a, *b)).collect();
    function_positions.sort_unstable();
    for (fi, sid) in function_positions {
        let mut p = fi + 1;
        while p < tokens.len()
            && tokens[p].text != "("
            && tokens[p].text != "do"
            && tokens[p].text != "then"
        {
            p += 1
        }
        if tokens.get(p).is_none_or(|t| t.text != "(") {
            continue;
        }
        let Some(close) = matching(tokens, p, "(", ")") else {
            continue;
        };
        let mut k = p + 1;
        while k < close {
            if tokens[k].kind == Kind::Ident && !is_kw(&tokens[k].text) {
                bindings.push(Binding {
                    name: tokens[k].text.clone(),
                    scope: sid,
                    start: close + 1,
                    declaration: vec![k],
                    replacement: String::new(),
                });
            }
            k += 1;
        }
        // Method syntax supplies an implicit `self` parameter.
        if p >= 2 && tokens[p - 1].text == ":" {
            bindings.push(Binding {
                name: "self".into(),
                scope: sid,
                start: close + 1,
                declaration: vec![],
                replacement: String::new(),
            });
        }
    }
    // Numeric and generic for variables belong only to the loop body.
    for i in 0..tokens.len() {
        if tokens[i].text != "for" {
            continue;
        }
        let mut d = i + 1;
        while d < tokens.len() && tokens[d].text != "do" && tokens[d].text != "end" {
            d += 1
        }
        let Some(&sid) = do_scopes.get(&d) else {
            continue;
        };
        for k in i + 1..d {
            if tokens[k].kind == Kind::Ident
                && !is_kw(&tokens[k].text)
                && tokens
                    .get(k + 1)
                    .is_some_and(|t| t.text == "," || t.text == "=" || t.text == "in")
            {
                bindings.push(Binding {
                    name: tokens[k].text.clone(),
                    scope: sid,
                    start: d + 1,
                    declaration: vec![k],
                    replacement: String::new(),
                });
            }
        }
    }

    // A local is eligible only if its name is not used in its own initializer (which begins before visibility).
    let mut decls = HashSet::new();
    for b in &bindings {
        for &d in &b.declaration {
            decls.insert(d);
        }
    }
    for b in &mut bindings {
        if b.declaration.is_empty() {
            b.replacement = "self".into()
        } else {
            b.replacement = fresh_identifier(rng, &mut used);
        }
    }
    // Index bindings by name so the reference pass never scans every binding for every token.
    let mut by_name: HashMap<&str, Vec<usize>> = HashMap::new();
    for (index, b) in bindings.iter().enumerate() {
        by_name.entry(b.name.as_str()).or_default().push(index);
    }
    // One forward pass records which tokens sit inside a table constructor, replacing the
    // backward brace scan that made renaming quadratic on large files.
    let mut in_table = vec![false; tokens.len()];
    let mut brace_depth = 0usize;
    for (i, token) in tokens.iter().enumerate() {
        if token.kind == Kind::Symbol {
            if token.text == "}" {
                brace_depth = brace_depth.saturating_sub(1);
            } else if token.text == "{" {
                brace_depth += 1;
                continue;
            }
        }
        in_table[i] = brace_depth > 0;
    }
    let mut rewrites: HashMap<usize, String> = HashMap::new();
    for b in &bindings {
        for &d in &b.declaration {
            rewrites.insert(d, b.replacement.clone());
        }
    }
    for i in 0..tokens.len() {
        if decls.contains(&i) || tokens[i].kind != Kind::Ident || is_kw(&tokens[i].text) {
            continue;
        }
        if i > 0
            && (tokens[i - 1].text == "."
                || tokens[i - 1].text == ":"
                || tokens[i - 1].text == "::"
                || tokens[i - 1].text == "goto")
        {
            continue;
        }
        // A name=value field inside a table constructor is a key, not a variable reference.
        if in_table[i] && tokens.get(i + 1).is_some_and(|t| t.text == "=") {
            continue;
        }
        let scope = token_scope[i];
        let Some(indexes) = by_name.get(tokens[i].text.as_str()) else {
            continue;
        };
        // Closest enclosing declaration wins: deepest scope first, then the latest start.
        let mut best: Option<&Binding> = None;
        let mut best_key = (0usize, 0usize);
        for &index in indexes {
            let b = &bindings[index];
            if i < b.start || !ancestor(scope, b.scope, &scopes) {
                continue;
            }
            let key = (scope_depth(b.scope, &scopes), b.start);
            if best.is_none() || key > best_key {
                best = Some(b);
                best_key = key;
            }
        }
        if let Some(b) = best {
            rewrites.insert(i, b.replacement.clone());
        }
    }
    let count = rewrites.len();
    for (i, name) in rewrites {
        tokens[i].text = name;
    }
    count
}
fn scope_depth(mut s: usize, scopes: &[Scope]) -> usize {
    let mut n = 0;
    while let Some(p) = scopes[s].parent {
        n += 1;
        s = p
    }
    n
}

#[cfg(test)]
mod tests {
    use super::fresh_identifier;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;
    use std::collections::HashSet;

    /// The pool used to hold only 4,032 names and the generator spun forever once it filled up,
    /// which froze large files.
    #[test]
    fn identifier_pool_never_runs_dry() {
        let mut rng = ChaCha8Rng::seed_from_u64(9);
        let mut used = HashSet::new();
        let mut names = HashSet::new();
        for _ in 0..6000 {
            let name = fresh_identifier(&mut rng, &mut used);
            assert!(name.starts_with('_') && name.len() >= 7, "{name}");
            names.insert(name);
        }
        assert_eq!(names.len(), 6000, "generated names must stay unique");
    }
}
