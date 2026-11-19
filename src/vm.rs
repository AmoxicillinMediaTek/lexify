//! Bytecode-style virtualization.
//!
//! Every eligible block is lowered into a program-counter loop: the statements become
//! instructions keyed by shuffled opaque opcodes and are reached through a dispatch
//! chain. The instruction bodies still contain real Lua, so behaviour is preserved;
//! only the layout changes.
//!
//! A block is only virtualized when that is provably safe:
//! * at least [`MIN_STATEMENTS`] statements, so the loop is worth its overhead;
//! * no `goto`, label, `break` or `continue` at the block's own level, since those
//!   would rebind to the dispatcher instead of the original target;
//! * no `<const>`/`<close>` attributes;
//! * no local declared twice, and no local read before its declaration (hoisting a
//!   local past such a read would change which variable the read resolves to).

use full_moon::ast::{Block, LastStmt, Stmt};
use full_moon::node::Node;
use full_moon::visitors::Visitor;
use rand::{Rng, RngCore};
use std::collections::HashSet;

/// Blocks smaller than this stay as they are; the dispatch chain costs more than it hides.
const MIN_STATEMENTS: usize = 4;
/// Each round virtualizes the deepest eligible blocks, then re-parses. Real files
/// are a handful of levels deep, so this never gets close to the cap.
const MAX_ROUNDS: usize = 8;

#[derive(Clone, Debug)]
enum Kind {
    Plain,
    /// A `local` declaration. `names` are hoisted, `values` tells whether an
    /// initialiser follows (a bare `local x` degrades to a no-op instruction).
    Declare {
        names: Vec<String>,
        values: bool,
    },
    /// `local function name ...`, rewritten to `name = function ...`.
    DeclareFunction {
        name: String,
        rest: usize,
    },
}

#[derive(Clone, Debug)]
struct Instr {
    start: usize,
    end: usize,
    kind: Kind,
    /// `return` ends the enclosing function, so no program-counter update follows it.
    returns: bool,
}

impl Instr {
    fn declared(&self) -> Vec<&str> {
        match &self.kind {
            Kind::Declare { names, .. } => names.iter().map(String::as_str).collect(),
            Kind::DeclareFunction { name, .. } => vec![name.as_str()],
            Kind::Plain => Vec::new(),
        }
    }
}

#[derive(Clone, Debug)]
struct Candidate {
    start: usize,
    end: usize,
    instrs: Vec<Instr>,
}

#[derive(Default)]
struct Collector {
    blocks: Vec<Candidate>,
}

impl Visitor for Collector {
    fn visit_block(&mut self, block: &Block) {
        if let Some(candidate) = collect(block) {
            self.blocks.push(candidate);
        }
    }
}

fn collect(block: &Block) -> Option<Candidate> {
    let mut instrs = Vec::new();
    for stmt in block.stmts() {
        let (start, end) = stmt.range()?;
        let kind = match stmt {
            // Labels and gotos only resolve inside their own lexical block, so a
            // dispatcher would silently break the jump.
            Stmt::Goto(_) | Stmt::Label(_) => return None,
            Stmt::LocalAssignment(local) => {
                let names = local
                    .names()
                    .iter()
                    .map(|name| name.token().to_string())
                    .collect::<Vec<_>>();
                if names.is_empty() {
                    return None;
                }
                Kind::Declare {
                    names,
                    values: local.expressions().iter().next().is_some(),
                }
            }
            Stmt::LocalFunction(local) => {
                let name = local.name().token().to_string();
                let rest = local.name().end_position()?.bytes();
                if rest <= start.bytes() {
                    return None;
                }
                Kind::DeclareFunction { name, rest }
            }
            _ => Kind::Plain,
        };
        instrs.push(Instr {
            start: start.bytes(),
            end: end.bytes(),
            kind,
            returns: false,
        });
    }
    if let Some(last) = block.last_stmt() {
        match last {
            LastStmt::Return(ret) => {
                let (start, end) = ret.range()?;
                instrs.push(Instr {
                    start: start.bytes(),
                    end: end.bytes(),
                    kind: Kind::Plain,
                    returns: true,
                });
            }
            // `break`/`continue` inside a dispatcher would leave the dispatcher, not
            // the user's loop, so those blocks stay untouched.
            _ => return None,
        }
    }
    if instrs.len() < MIN_STATEMENTS {
        return None;
    }
    Some(Candidate {
        start: instrs.first()?.start,
        end: instrs.last()?.end,
        instrs,
    })
}

/// True when no other candidate lives inside this one, so splices stay disjoint.
fn is_leaf(candidate: &Candidate, all: &[Candidate]) -> bool {
    !all.iter().any(|other| {
        (other.start, other.end) != (candidate.start, candidate.end)
            && other.start >= candidate.start
            && other.end <= candidate.end
    })
}

fn contains_ident(text: &str, name: &str) -> bool {
    text.match_indices(name).any(|(index, _)| {
        let before = text[..index].chars().next_back();
        let after = text[index + name.len()..].chars().next();
        !before.is_some_and(is_ident_char) && !after.is_some_and(is_ident_char)
    })
}

fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// `<const>`/`<close>` change when a local may be assigned, so those blocks stay put.
fn has_attribute(text: &str) -> bool {
    let bytes = text.as_bytes();
    for (index, byte) in bytes.iter().enumerate() {
        if *byte != b'<' {
            continue;
        }
        let mut end = index + 1;
        while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_') {
            end += 1;
        }
        if end > index + 1 && bytes.get(end) == Some(&b'>') {
            return true;
        }
    }
    false
}

fn count_ident(text: &str, name: &str) -> usize {
    text.match_indices(name)
        .filter(|(index, _)| {
            let before = text[..*index].chars().next_back();
            let after = text[index + name.len()..].chars().next();
            !before.is_some_and(is_ident_char) && !after.is_some_and(is_ident_char)
        })
        .count()
}

/// Everything `collect` cannot check without the source text.
fn safe(source: &str, candidate: &Candidate) -> bool {
    let mut declared: Vec<&str> = Vec::new();
    for (index, instr) in candidate.instrs.iter().enumerate() {
        let text = &source[instr.start..instr.end];
        if has_attribute(text) {
            return false;
        }
        for name in instr.declared() {
            if declared.contains(&name) {
                return false;
            }
            if candidate.instrs[..index]
                .iter()
                .any(|earlier| contains_ident(&source[earlier.start..earlier.end], name))
            {
                return false;
            }
            // `local x = x` reads the outer `x`; hoisting would read the hoisted one.
            // A `local function` may mention itself, so only plain declarations are checked.
            if matches!(instr.kind, Kind::Declare { .. }) && count_ident(text, name) > 1 {
                return false;
            }
            declared.push(name);
        }
    }
    true
}

fn hex(opcode: u32) -> String {
    format!("0x{opcode:x}")
}

fn opcodes(count: usize, rng: &mut impl RngCore) -> Vec<u32> {
    let mut seen = HashSet::new();
    let mut ops = Vec::with_capacity(count);
    while ops.len() < count {
        let opcode = rng.gen_range(0x1000..0xffffff);
        if seen.insert(opcode) {
            ops.push(opcode);
        }
    }
    ops
}

fn fresh_name(source: &str, rng: &mut impl RngCore) -> String {
    loop {
        let name = format!("_lx{:06x}", rng.gen_range(0..0x1000000));
        if !contains_ident(source, &name) {
            return name;
        }
    }
}

fn body(source: &str, instr: &Instr) -> String {
    match &instr.kind {
        Kind::Plain => source[instr.start..instr.end].to_string(),
        // Drop the `local` keyword: the name is declared once, ahead of the loop.
        Kind::Declare { values, .. } => {
            if *values {
                source[instr.start + "local".len()..instr.end]
                    .trim_start()
                    .to_string()
            } else {
                String::new()
            }
        }
        Kind::DeclareFunction { name, rest } => {
            format!("{name}=function{}", &source[*rest..instr.end])
        }
    }
}

fn lower(source: &str, candidate: &Candidate, rng: &mut impl RngCore) -> String {
    let pc = fresh_name(source, rng);
    let ops = opcodes(candidate.instrs.len(), rng);

    let hoisted: Vec<&str> = candidate
        .instrs
        .iter()
        .flat_map(Instr::declared)
        .collect::<Vec<_>>();

    let mut out = String::new();
    if !hoisted.is_empty() {
        out.push_str(&format!("local {};", hoisted.join(",")));
    }
    out.push_str(&format!("local {pc}={};while {pc} do\n", hex(ops[0])));
    for (index, instr) in candidate.instrs.iter().enumerate() {
        out.push_str(if index == 0 { "if " } else { "elseif " });
        out.push_str(&format!("{pc}=={} then\n", hex(ops[index])));
        let body = body(source, instr);
        if !body.is_empty() {
            out.push_str(&body);
            out.push('\n');
        }
        if instr.returns {
            continue;
        }
        match ops.get(index + 1) {
            Some(next) => out.push_str(&format!("{pc}={};\n", hex(*next))),
            None => out.push_str(&format!("{pc}=nil;\n")),
        }
    }
    out.push_str("end\nend");
    out
}

/// Virtualize every eligible block, deepest first. Returns the transformed source.
pub fn transform(source: &str, rng: &mut impl RngCore) -> Result<String, String> {
    let mut text = source.to_string();
    for _ in 0..MAX_ROUNDS {
        let ast = crate::parser::parse(&text)?.ast;
        let mut collector = Collector::default();
        collector.visit_ast(&ast);
        let eligible: Vec<Candidate> = collector
            .blocks
            .into_iter()
            .filter(|candidate| safe(&text, candidate))
            .collect();
        let leaves: Vec<Candidate> = eligible
            .iter()
            .filter(|candidate| is_leaf(candidate, &eligible))
            .cloned()
            .collect();
        if leaves.is_empty() {
            break;
        }
        let mut edits: Vec<(usize, usize, String)> = leaves
            .iter()
            .map(|candidate| (candidate.start, candidate.end, lower(&text, candidate, rng)))
            .collect();
        edits.sort_by_key(|(start, _, _)| std::cmp::Reverse(*start));
        for (start, end, replacement) in edits {
            text.replace_range(start..end, &replacement);
        }
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::transform;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    fn run(source: &str) -> String {
        let mut rng = ChaCha8Rng::seed_from_u64(7);
        transform(source, &mut rng).unwrap()
    }

    #[test]
    fn lowers_a_block_into_a_dispatcher() {
        let source = "local a = 1\nlocal b = 2\nlocal c = 3\nprint(a + b + c)\n";
        let out = run(source);
        assert!(out.contains("while "), "{out}");
        assert!(out.contains("0x"), "{out}");
        assert!(!out.contains("local a = 1"), "{out}");
        crate::parser::parse(&out).unwrap();
    }

    #[test]
    fn leaves_blocks_it_cannot_safely_lift() {
        // `goto` at block level, a duplicated local, and a local read before it is declared.
        for source in [
            "local x = 1\ngoto skip\n::skip::\nprint(x)\n",
            "local a = 1\nlocal b = 2\nlocal a = 3\nprint(a, b)\n",
            "print(a)\nlocal a = 1\nlocal b = 2\nlocal c = 3\n",
            "local a = a\nlocal b = 2\nlocal c = 3\nprint(a, b, c)\n",
        ] {
            let out = run(source);
            assert!(!out.contains("while "), "{source} -> {out}");
        }
    }

    #[test]
    fn keeps_recursive_local_functions_callable() {
        let source = "local function f(n) if n <= 1 then return 1 end return n * f(n - 1) end\nlocal a = 1\nlocal b = 2\nprint(f(5) + a + b)\n";
        let out = run(source);
        assert!(out.contains("while "), "{out}");
        crate::parser::parse(&out).unwrap();
    }
}
