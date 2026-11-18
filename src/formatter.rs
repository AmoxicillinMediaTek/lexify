use crate::lexer::{Kind, Token};
fn word_end(t:&Token)->bool { matches!(t.kind,Kind::Ident|Kind::Number) }
fn word_start(t:&Token)->bool { matches!(t.kind,Kind::Ident|Kind::Number) }
fn merged_operator(a:&str,b:&str)->bool { let s=format!("{a}{b}"); ["...","..","//","==","~=","<=",">=","<<",">>","::","+=","-=","*=","/=","->"].iter().any(|op|s.starts_with(op) && s!=format!("{a} {b}")) }
/// Render tokens, optionally dropping whitespace and comments while retaining token boundaries.
pub fn format(tokens:&[Token], minify:bool)->String {
    let filtered:Vec<&Token>=tokens.iter().filter(|t|t.kind!=Kind::Comment && (!minify || t.kind!=Kind::Space)).collect();
    let mut out=String::new(); let mut prev:Option<&Token>=None;
    for t in filtered { if minify { if let Some(p)=prev { if (word_end(p)&&word_start(t)) || ((p.kind==Kind::Number)&&t.text.starts_with('.')) || merged_operator(&p.text,&t.text) || ((p.text=="end" || p.text=="return" || p.text=="break" || p.text=="continue") && (word_start(t)||t.text=="(")) {out.push(' ')} } }
        out.push_str(&t.text); prev=Some(t);
    } out
}
