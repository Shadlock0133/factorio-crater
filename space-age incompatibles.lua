---@module "defs"

local i = 0;
for _, mod in pairs(mods) do
    if mod.releases[1] and mod.releases[1].info_json.factorio_version == "2.0" then
        for _, dep in pairs(mod.releases[1].info_json.dependencies) do
            if dep.name == "space-age" and dep.prefix == "incompatible" then
                print(mod.name)
                i = i + 1
            end
        end
    end
end
print("count: " .. i)
