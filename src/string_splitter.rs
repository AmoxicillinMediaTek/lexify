//! AST-backed string splitting and randomized lookup-table transformation.
use full_moon::{
    ast::{Expression, Stmt},
    visitors::VisitorMut,
};
use rand::{seq::SliceRandom, Rng, RngCore};
use std::collections::{HashMap, HashSet};

type Entry = (usize, Vec<u8>);

#[derive(Clone, Debug)]
pub struct Options {
    pub split_size: usize,
    pub lookup_tables: bool,
    pub randomize_lookup: bool,
    pub shared_lookup: bool,
}

struct Splitter<'a, R> {
    name: String,
    options: &'a Options,
    rng: &'a mut R,
    tables: Vec<Vec<Entry>>,
    shared: HashMap<Vec<u8>, usize>,
    changed: bool,
}

impl<R: RngCore> Splitter<'_, R> {
    fn split(&mut self, bytes: &[u8]) -> Vec<Vec<u8>> {
        if bytes.is_empty() {
            return vec![Vec::new(), Vec::new()];
        }
        let max = self.options.split_size.max(1);
        let mut chunks = Vec::new();
        let mut pos = 0;
        while pos < bytes.len() {
            let remaining = bytes.len() - pos;
            // Keep at least one byte for a second fragment whenever possible.
            let cap = max
                .min(if chunks.is_empty() && remaining > 1 {
                    remaining - 1
                } else {
                    remaining
                })
                .max(1);
            let size = if self.options.randomize_lookup {
                self.rng.gen_range(1..=cap)
            } else {
                cap
            };
            chunks.push(bytes[pos..pos + size].to_vec());
            pos += size;
        }
        if chunks.len() == 1 {
            chunks.push(Vec::new());
        }
        chunks
    }

    fn next_index(&mut self, entries: &[Entry]) -> usize {
        if !self.options.randomize_lookup {
            return entries.len() + 1;
        }
        loop {
            let index = self.rng.gen_range(1000..=999_999);
            if !entries.iter().any(|(existing, _)| *existing == index) {
                return index;
            }
        }
    }

    fn replace_literal(&mut self, raw: &str) -> Option<Expression> {
        let bytes = crate::string_encrypt::decode_literal(raw)?;
        let chunks = self.split(&bytes);
        let expression = if !self.options.lookup_tables {
            chunks
                .iter()
                .map(|chunk| quote(chunk))
                .collect::<Vec<_>>()
                .join(" .. ")
        } else if self.options.shared_lookup {
            let mut indexes = Vec::with_capacity(chunks.len());
            for chunk in chunks {
                let index = if let Some(index) = self.shared.get(&chunk) {
                    *index
                } else {
                    let index = self.next_index(&self.tables[0].clone());
                    self.tables[0].push((index, chunk.clone()));
                    self.shared.insert(chunk, index);
                    index
                };
                indexes.push(index);
            }
            indexes
                .into_iter()
                .map(|index| format!("{}[{index}]", self.name))
                .collect::<Vec<_>>()
                .join(" .. ")
        } else {
            let mut table = Vec::<Entry>::new();
            let mut indexes = Vec::with_capacity(chunks.len());
            for chunk in chunks {
                let index = if let Some((index, _)) =
                    table.iter().find(|(_, existing)| *existing == chunk)
                {
                    *index
                } else {
                    let index = self.next_index(&table);
                    table.push((index, chunk));
                    index
                };
                indexes.push(index);
            }
            if self.options.randomize_lookup {
                table.shuffle(self.rng);
            }
            let table_id = self.tables.len() + 1;
            self.tables.push(table);
            indexes
                .into_iter()
                .map(|index| format!("{}[{table_id}][{index}]", self.name))
                .collect::<Vec<_>>()
                .join(" .. ")
        };
        self.changed = true;
        // The replacement is a concatenation, which binds looser than unary operators and `^`.
        // Parenthesising keeps the literal's original precedence in every expression position.
        parse_expression(&format!("({expression})"))
    }
}

fn quote(bytes: &[u8]) -> String {
    let escaped = bytes
        .iter()
        .map(|byte| format!("\\{:03}", byte))
        .collect::<String>();
    format!("\"{escaped}\"")
}

impl<R: RngCore> VisitorMut for Splitter<'_, R> {
    fn visit_expression_end(&mut self, expression: Expression) -> Expression {
        match expression {
            Expression::String(token) => {
                let raw = token.token().to_string();
                self.replace_literal(&raw)
                    .unwrap_or(Expression::String(token))
            }
            other => other,
        }
    }
}

fn parse_expression(source: &str) -> Option<Expression> {
    let ast = full_moon::parse(&format!("local __lexify_expression = {source}")).ok()?;
    let expression = match ast.nodes().stmts().next()? {
        Stmt::LocalAssignment(assignment) => assignment.expressions().iter().next().cloned(),
        _ => None,
    };
    expression
}

fn fresh_name(source: &str, rng: &mut impl RngCore) -> String {
    let used: HashSet<String> = crate::lexer::lex(source)
        .unwrap_or_default()
        .into_iter()
        .filter(|token| token.kind == crate::lexer::Kind::Ident)
        .map(|token| token.text)
        .collect();
    loop {
        let name = format!("_lx{:08x}", rng.next_u32());
        if !used.contains(&name) {
            return name;
        }
    }
}

fn table_source(name: &str, tables: &[Vec<Entry>], shared: bool) -> String {
    let values = if shared {
        tables
            .first()
            .into_iter()
            .flatten()
            .map(|(index, bytes)| format!("[{index}]={}", quote(bytes)))
            .collect::<Vec<_>>()
            .join(",")
    } else {
        tables
            .iter()
            .map(|table| {
                format!(
                    "{{{}}}",
                    table
                        .iter()
                        .map(|(index, bytes)| format!("[{index}]={}", quote(bytes)))
                        .collect::<Vec<_>>()
                        .join(",")
                )
            })
            .collect::<Vec<_>>()
            .join(",")
    };
    format!("local {name}={{{values}}};")
}

/// Rewrites actual string-literal expression nodes, then serializes and reparses the AST.
pub fn transform(
    source: &str,
    rng: &mut impl RngCore,
    options: &Options,
) -> Result<String, String> {
    let ast = full_moon::parse(source)
        .map_err(|errors| format!("string-split AST parse error: {errors:?}"))?;
    let name = fresh_name(source, rng);
    let mut splitter = Splitter {
        name: name.clone(),
        options,
        rng,
        tables: if options.shared_lookup {
            vec![Vec::new()]
        } else {
            Vec::new()
        },
        shared: HashMap::new(),
        changed: false,
    };
    let transformed = splitter.visit_ast(ast);
    if !splitter.changed {
        return Ok(transformed.to_string());
    }
    let result = if options.lookup_tables {
        format!(
            "{}\n{}",
            table_source(&name, &splitter.tables, options.shared_lookup),
            transformed
        )
    } else {
        transformed.to_string()
    };
    full_moon::parse(&result)
        .map_err(|errors| format!("string-split output AST validation failed: {errors:?}"))?;
    Ok(result)
}
