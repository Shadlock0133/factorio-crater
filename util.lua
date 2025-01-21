---@generic T, U
---@param table table<T, U>
---@param value U
---@return boolean
table.contains = function(table, value)
    for _, x in pairs(table) do
        if value == x then
            return true
        end
    end
    return false
end

---@generic T, U
---@param table table<T, U>
---@param f fun(value: U): boolean
---@return boolean
table.all = function(table, f)
    for _, x in pairs(table) do
        if f(x) then
            return true
        end
    end
    return false
end
