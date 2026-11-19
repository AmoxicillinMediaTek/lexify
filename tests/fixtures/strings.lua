-- Fixture covering empty, short, long, UTF-8, escaped, quoted, nested, table,
-- repeated, binary and long-bracket strings, plus precedence-sensitive positions.
local out = {}
local function emit(tag, value) out[#out + 1] = tag .. "=" .. tostring(value) end

emit("empty", "[" .. "]" .. #"")
emit("single", "a")
emit("short", "hello world")
emit("long", "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
emit("utf8", "héllo 世界")
emit("escapes", "tab\there\nnl\\slash\"quote\'single")
emit("hexdec", "\x41\066\u{43}")
emit("zskip", "a\z
        b")
emit("longbracket", [==[raw [[nested]] text]==])
emit("multiline", [[
line one
line two]])
emit("binary", "a\0b\255c")

-- Precedence-sensitive positions: length, negation, arithmetic coercion, power.
emit("len", #"abcdef")
emit("arith", "10" - 1)
emit("pow", 2 ^ "3")
emit("concat", "x" .. "y" .. "z")

-- Nested expressions, tables, call sugar and method calls.
local t = { name = "hello world", ["key one"] = "value", "positional" }
emit("table", t.name .. "/" .. t["key one"] .. "/" .. t[1])
local repeated = "shared literal"
emit("repeat1", repeated)
emit("repeat2", "shared literal")

local function outer(a)
  return function(b) return a .. ":" .. b .. ":" .. "shared literal" end
end
emit("closure", outer("alpha")("beta"))
emit("fmt", string.format("%s-%02d", "pad", 7))
emit("method", ("Ab"):lower())
emit("call", ("xy"):rep(2))

-- Call sugar with a bare string literal.
print "call-sugar-ok"

-- Loop and branch usage, string keys in conditionals.
for i = 1, 3 do
  if i == 2 then emit("loop" .. i, "two") else emit("loop" .. i, i) end
end

-- String used as a statement-level index chain and in a generic-for key.
local map = { ["alpha"] = 1, ["beta"] = 2 }
local keys = {}
for k, v in pairs(map) do keys[#keys + 1] = k end
table.sort(keys)
for _, k in ipairs(keys) do emit("map", k .. map[k]) end

print(table.concat(out, "\n"))
print("DONE", #out)