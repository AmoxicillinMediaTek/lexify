use crate::lexer::{Kind,Token};
/// Parenthesize each table constructor without changing field evaluation order.
pub fn transform(tokens:&mut Vec<Token>) {
    let mut i=0;
    while i<tokens.len() {
        if tokens[i].text=="{"&&tokens[i].kind==Kind::Symbol {
            let open=crate::lexer::lex("(").unwrap().remove(0);let close=crate::lexer::lex(")").unwrap().remove(0);
            tokens.insert(i,open);i+=1;let mut depth=1usize;i+=1;
            while i<tokens.len()&&depth>0 {if tokens[i].text=="{"{depth+=1}else if tokens[i].text=="}"{depth-=1;if depth==0{tokens.insert(i+1,close);break}}i+=1;}
        }
        i+=1;
    }
}
