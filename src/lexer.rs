//! Token scanner used for safe minification and literal rewriting. Parsing is delegated to full_moon.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind { Ident, Number, String, Symbol, Space, Comment }
#[derive(Clone, Debug)]
pub struct Token { pub kind: Kind, pub text: String, pub line: usize }
fn long_open(b: &[u8], i: usize) -> Option<(usize, usize)> {
    if b.get(i) != Some(&b'[') { return None; }
    let mut j=i+1; while b.get(j)==Some(&b'=') { j+=1; }
    if b.get(j)==Some(&b'[') { Some((j-i-1,j+1)) } else { None }
}
pub fn lex(src: &str) -> Result<Vec<Token>, String> {
    let b=src.as_bytes(); let mut i=0; let mut line=1; let mut col=1; let mut out=Vec::new();
    while i<b.len() {
        let st=i; let (ln,cl)=(line,col); let c=b[i];
        let kind;
        if c.is_ascii_whitespace() { kind=Kind::Space; i+=1; }
        else if c==b'-' && b.get(i+1)==Some(&b'-') {
            i+=2;
            if let Some((eq,after))=long_open(b,i) { let close=format!("]{}]", "=".repeat(eq)); if let Some(p)=src[after..].find(&close){i=after+p+close.len()}else{return Err(format!("line {ln}, column {cl}: unterminated long comment"))} }
            else { while i<b.len() && b[i]!=b'\n' { i+=1; } }
            kind=Kind::Comment;
        } else if c==b'\'' || c==b'"' {
            let quote=c; i+=1; let mut closed=false;
            while i<b.len() { if b[i]==b'\\' { i=(i+2).min(b.len()); } else if b[i]==quote {i+=1;closed=true;break} else {i+=1;} }
            if !closed{return Err(format!("line {ln}, column {cl}: unterminated string literal"))} kind=Kind::String;
        } else if let Some((eq,after))=long_open(b,i) {
            let close=format!("]{}]", "=".repeat(eq)); if let Some(p)=src[after..].find(&close){i=after+p+close.len()}else{return Err(format!("line {ln}, column {cl}: unterminated long string"))} kind=Kind::String;
        } else if c.is_ascii_alphabetic() || c==b'_' { i+=1; while i<b.len() && (b[i].is_ascii_alphanumeric()||b[i]==b'_'){i+=1} kind=Kind::Ident; }
        else if c.is_ascii_digit() || (c==b'.' && b.get(i+1).is_some_and(u8::is_ascii_digit)) {
            let start=i;
            if c==b'0' && b.get(i+1).is_some_and(|x| *x==b'x' || *x==b'X') {
                i+=2; while i<b.len() && b[i].is_ascii_hexdigit(){i+=1;}
                if b.get(i)==Some(&b'.') && b.get(i+1)!=Some(&b'.'){i+=1;while i<b.len()&&b[i].is_ascii_hexdigit(){i+=1;}}
                if b.get(i).is_some_and(|x| *x==b'p' || *x==b'P') {i+=1;if b.get(i).is_some_and(|x| *x==b'+' || *x==b'-'){i+=1;}while i<b.len()&&b[i].is_ascii_digit(){i+=1;}}
            } else {
                if c==b'.' {i+=1;while i<b.len()&&b[i].is_ascii_digit(){i+=1;}}
                else {while i<b.len()&&b[i].is_ascii_digit(){i+=1;}if b.get(i)==Some(&b'.')&&b.get(i+1)!=Some(&b'.'){i+=1;while i<b.len()&&b[i].is_ascii_digit(){i+=1;}}}
                if b.get(i).is_some_and(|x| *x==b'e' || *x==b'E') {i+=1;if b.get(i).is_some_and(|x| *x==b'+' || *x==b'-'){i+=1;}while i<b.len()&&b[i].is_ascii_digit(){i+=1;}}
            }
            if i==start {i+=1;} kind=Kind::Number;
        }
        else { let rest=&src[i..]; let n=["...","..","==","~=","<=",">=","//","<<",">>","+=","-=","*=","/=","::","->"]; if let Some(s)=n.iter().find(|s|rest.starts_with(**s)){i+=s.len()}else{i+=1} kind=Kind::Symbol; }
        let text=src[st..i].to_string();
        for ch in text.chars(){if ch=='\n'{line+=1;col=1}else{col+=1}}
        out.push(Token{kind,text,line:ln});
    } Ok(out)
}
