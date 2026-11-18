local greeting = "Hello, Lua!"
local function make_counter(start)
    local value = start
    return function(step)
        value = value + step
        return value
    end
end

local counter = make_counter(3)
if counter(2) > 4 then
    print(greeting, counter(1))
end
