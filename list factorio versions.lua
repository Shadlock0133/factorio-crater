---@module "defs"
require("util")

local versions = {} ---@type string[]
local version_map = {} ---@type table<string, table<string, boolean>>
for name, mod in pairs(mods) do
    for _, release in pairs(mod.releases) do
        local v = release.info_json.factorio_version
        if not table.contains(versions, v) then
            table.insert(versions, v)
            version_map[v] = {}
        end
        version_map[v][name] = true
    end
end

table.sort(versions)

for _, v in ipairs(versions) do
    local file = io.open("by_version/" .. v .. ".txt", "w")
    if not file then
        error("Couldn't open " .. v .. ".txt file")
    end
    local list = {}
    for m, _ in pairs(version_map[v]) do
        table.insert(list, m)
    end
    table.sort(list)
    for _, m in ipairs(list) do
        file:write(m .. '\n')
    end
end
