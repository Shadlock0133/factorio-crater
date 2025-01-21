---@meta

---@class Mod
---@field category string
---@field changelog string?
---@field created_at string
---@field downloads_count string
---@field deprecated boolean
---@field description string
---@field homepage string
---@field images Image[]
---@field license License?
---@field name string
---@field owner string
---@field releases Release[]
---@field score number?
---@field source_url string?
---@field summary string
---@field tags string[]?
---@field thumbnail string?
---@field title string
---@field updated_at string

---@class Image
---@field id string
---@field thumbnail string
---@field url string

---@class License
---@field description string
---@field id string
---@field name string
---@field title string
---@field url string

---@class Release
---@field download_url string
---@field file_name string
---@field info_json InfoJson
---@field release_at string
---@field sha1 string
---@field version string

---@class InfoJson
---@field dependencies Dep[]
---@field factorio_version string

---@class Dep
---@field original string
---@field prefix Prefix
---@field name string
---@field version string

---@alias Prefix
---| "incompatible"
---| "optional"
---| "hidden-optional"
---| "load-order-independent"
---| "required"

_G.mods = mods ---@type table<string, Mod>
