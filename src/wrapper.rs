//! Output wrappers.
//!
//! The finished chunk is invoked from inside a randomly chosen facade, so the outer
//! shape of the file looks like a loader instead of the user's script. Every style
//! runs the chunk exactly once with the original varargs and forwards its results,
//! so behaviour (and printed output) is unchanged.

use rand::{Rng, RngCore};

/// Number of available styles; [`transform`] picks one at random on every run.
pub const STYLES: usize = 5;

/// Render one style. `style` is taken modulo [`STYLES`], `tag` only decorates the output.
pub fn render(style: usize, source: &str, tag: &str) -> String {
    let invoke = format!("(function(...)\n{source}\nend)(...)");
    match style % STYLES {
        // return(function(...) ... end)(...)
        0 => format!("return {invoke}"),
        // return select(...)
        1 => format!(
            "local r=table.pack({invoke})return select(1,table.unpack(r,1,r.n))"
        ),
        // ("watermark"):gsub('.+', function(...) ... end)
        2 => format!(
            "local a=table.pack(...)local r;(\"lx-{tag}\"):gsub(\".+\",function()r=table.pack((function(...)\n{source}\nend)(table.unpack(a,1,a.n)))return\"\"end)return table.unpack(r,1,r.n)"
        ),
        // return setmetatable({...}):L()
        3 => format!(
            "local o=setmetatable({{}},{{__index={{L=function(_,...)return {invoke} end}}}})local r=table.pack(o:L(...))return table.unpack(r,1,r.n)"
        ),
        // return({...})
        _ => format!("local r=table.pack({invoke})return table.unpack({{table.unpack(r,1,r.n)}},1,r.n)"),
    }
}

/// Wrap `source` in a random style.
pub fn transform(source: &str, rng: &mut impl RngCore) -> String {
    let style = rng.gen_range(0..STYLES);
    let tag = format!("{:08x}", rng.gen::<u32>());
    render(style, source, &tag)
}

#[cfg(test)]
mod tests {
    use super::{render, STYLES};

    #[test]
    fn every_style_is_valid_lua_and_keeps_the_chunk() {
        let source = "print('hi')";
        for style in 0..STYLES {
            let output = render(style, source, "deadbeef");
            assert!(output.contains("print('hi')"), "style {style}: {output}");
            crate::parser::parse(&output).unwrap_or_else(|e| panic!("style {style}: {e}"));
        }
    }
}
