-- angelx-kingdom-export.lua
-- Read-only export of the currently loaded fortress, for the angelX Delve's
-- Dwarf Kingdom expansion map. It changes nothing in the game.
--
-- Run from the DFHack console:    angelx-kingdom-export
-- or from a shell:                dfhack-run angelx-kingdom-export
--
-- Writes under <Dwarf Fortress folder>/dfhack-config/angelx-kingdom-export/:
--   kingdom.json        identity, wealth, citizens, squads, buildings, rooms,
--                       locations, stocks, artifacts, animals, dead, events
--   tiles/z<NNN>.txt    one file per z-level that has anything revealed:
--                       a SHAPE grid, a MAT grid, a LIQUID grid and a ROCK grid
--                       (see the legend in kingdom.json -> tiles.legend)
--   prospect.txt        DFHack's prospect report
-- Each section runs on its own; a failure is listed in kingdom.json "log".

local json = require('json')
local OUT = 'dfhack-config/angelx-kingdom-export'
dfhack.filesystem.mkdir_recursive(OUT)
dfhack.filesystem.mkdir_recursive(OUT .. '/tiles')

local log = {}
local function note(s) table.insert(log, s); print('[angelx-export] ' .. s) end

local function tr(name, english)
  if not name then return nil end
  local f = (dfhack.translation and dfhack.translation.translateName) or dfhack.TranslateName
  local ok, r = pcall(f, name, english)
  if ok then return r end
  return nil
end

local function enum_name(enum, v)
  local ok, n = pcall(function() return enum[v] end)
  if ok and n then return tostring(n) end
  return tostring(v)
end

local function section(label, fn)
  local t0 = os.clock()
  local ok, res = pcall(fn)
  local dt = string.format('%.1fs', os.clock() - t0)
  if ok then note('OK   ' .. label .. ' ' .. dt); return res end
  note('FAIL ' .. label .. ' ' .. dt .. ': ' .. tostring(res))
  return { error = tostring(res) }
end

local function write_text(path, text)
  local f = io.open(path, 'w')
  if f then f:write(text or ''); f:close() end
end

local function matstr(mt, mi)
  local ok, m = pcall(dfhack.matinfo.decode, mt, mi)
  if ok and m then return m:toString() end
  return nil
end

local function unit_ref(u)
  if not u then return nil end
  local vn = dfhack.units.getVisibleName(u)
  return { id = u.id, name = tr(vn, false), name_english = tr(vn, true),
           profession = dfhack.units.getProfessionName(u) }
end

local function hf_ref(hfid)
  if not hfid or hfid < 0 then return nil end
  local hf = df.historical_figure.find(hfid)
  if not hf then return { hf = hfid } end
  local race = nil
  pcall(function() race = df.global.world.raws.creatures.all[hf.race].creature_id end)
  return { hf = hfid, name = tr(hf.name, false), name_english = tr(hf.name, true),
           unit = hf.unit_id, race = race }
end

local pi = df.global.plotinfo
local site = df.world_site.find(pi.site_id)

-- 1. Identity -----------------------------------------------------------------
local fortress = section('fortress', function()
  local civ = df.historical_entity.find(pi.civ_id)
  local grp = df.historical_entity.find(pi.group_id)
  local m = df.global.world.map
  local wd = df.global.world.world_data
  local r = {
    site_id = pi.site_id,
    site_name = site and tr(site.name, false),
    site_name_english = site and tr(site.name, true),
    civ_id = pi.civ_id,
    civ_name = civ and tr(civ.name, false),
    civ_name_english = civ and tr(civ.name, true),
    group_id = pi.group_id,
    group_name = grp and tr(grp.name, false),
    group_name_english = grp and tr(grp.name, true),
    world_name = tr(wd.name, false),
    world_name_english = tr(wd.name, true),
    year = df.global.cur_year,
    year_tick = df.global.cur_year_tick,
    map_tiles = { x = m.x_count, y = m.y_count, z = m.z_count },
    region_origin = { x = m.region_x, y = m.region_y, z = m.region_z },
  }
  pcall(function()
    local w = df.global.world.world_data
    r.surface_biome = nil
    r.elevation_z = df.global.world.map.region_z
  end)
  return r
end)

-- 2. Wealth -------------------------------------------------------------------
local wealth = section('wealth', function()
  local t = {}
  for k, v in pairs(pi.tasks.wealth) do
    if type(v) == 'number' then t[k] = v end
  end
  return t
end)

-- 3. Squads (built first so citizens can name theirs) -------------------------
local squad_of = {}
local squads = section('squads', function()
  local list = {}
  for _, sq in ipairs(df.global.world.squads.all) do
    if sq.entity_id == pi.group_id then
      local r = { id = sq.id, name = tr(sq.name, false), name_english = tr(sq.name, true),
                  alias = sq.alias, members = {} }
      for i, pos in ipairs(sq.positions) do
        local hf = pos.occupant
        if hf and hf >= 0 then
          local h = df.historical_figure.find(hf)
          local uid = h and h.unit_id or -1
          table.insert(r.members, { slot = i, hf = hf, unit = uid })
          if uid >= 0 then squad_of[uid] = { id = sq.id, name = r.alias ~= '' and r.alias or r.name_english, slot = i } end
        end
      end
      table.insert(list, r)
    end
  end
  return list
end)

-- 4. Citizens -----------------------------------------------------------------
local citizens = section('citizens', function()
  local list = {}
  for _, u in ipairs(dfhack.units.getCitizens()) do
    local ok, e = pcall(function()
      local vn = dfhack.units.getVisibleName(u)
      local r = {
        id = u.id,
        hf = u.hist_figure_id,
        name = tr(vn, false),
        name_english = tr(vn, true),
        readable = dfhack.units.getReadableName(u),
        profession = dfhack.units.getProfessionName(u),
        race = dfhack.units.getRaceName(u),
        age = dfhack.units.getAge(u, true),
        sex = u.sex,
        pos = { x = u.pos.x, y = u.pos.y, z = u.pos.z },
        squad = squad_of[u.id],
        skills = {},
        positions = {},
        traits = {},
        values = {},
      }
      pcall(function() r.stress = u.status.current_soul.personality.stress end)
      pcall(function() r.mood = enum_name(df.mood_type, u.mood) end)
      pcall(function() r.kills = #u.status.current_soul and nil end)
      local soul = u.status.current_soul
      if soul then
        for _, s in ipairs(soul.skills) do
          table.insert(r.skills, { skill = enum_name(df.job_skill, s.id), level = s.rating, experience = s.experience })
        end
        pcall(function()
          for k, v in pairs(soul.personality.traits) do
            if type(v) == 'number' and (v >= 75 or v <= 25) then r.traits[tostring(k)] = v end
          end
        end)
        pcall(function()
          for _, v in ipairs(soul.personality.values) do
            if v.strength >= 21 or v.strength <= -21 then
              table.insert(r.values, { value = enum_name(df.value_type, v.type), strength = v.strength })
            end
          end
        end)
      end
      local okp, pos = pcall(dfhack.units.getNoblePositions, u)
      if okp and pos then
        for _, p in ipairs(pos) do
          if p.position and p.position.code then table.insert(r.positions, p.position.code) end
        end
      end
      return r
    end)
    table.insert(list, ok and e or { error = tostring(e) })
  end
  return list
end)

-- 5. Locations (tavern, temple, library, guildhall, hospital) -----------------
local locations = section('locations', function()
  local list = {}
  if not site then return list end
  for _, ab in ipairs(site.buildings) do
    local ok, e = pcall(function()
      local r = { id = ab.id, type = enum_name(df.abstract_building_type, ab:getType()) }
      local okn, nm = pcall(function() return ab:getName() end)
      if okn and nm then r.name = tr(nm, false); r.name_english = tr(nm, true) end
      pcall(function()
        if ab.deity_type == nil or ab.deity_type < 0 then return end
        r.deity_type = ab.deity_type
        if ab.deity_type == 1 then
          -- a religion's temple: deity_data holds the religion's entity id
          local id = ab.deity_data.Religion
          local ent = df.historical_entity.find(id)
          r.religion = { entity = id, name = ent and tr(ent.name, false), name_english = ent and tr(ent.name, true) }
        else
          r.deity = hf_ref(ab.deity_data.Deity)
        end
      end)
      pcall(function() r.profession = enum_name(df.profession, ab.contents.profession) end)
      return r
    end)
    table.insert(list, ok and e or { error = tostring(e) })
  end
  return list
end)

-- 6. Buildings, furniture, workshops, stockpiles, zones -----------------------
local buildings = section('buildings', function()
  local list = {}
  for _, b in ipairs(df.global.world.buildings.all) do
    local ok, e = pcall(function()
      local bt = b:getType()
      local r = {
        id = b.id,
        type = enum_name(df.building_type, bt),
        name = dfhack.buildings.getName(b),
        x1 = b.x1, y1 = b.y1, x2 = b.x2, y2 = b.y2, z = b.z,
        material = matstr(b.mat_type, b.mat_index),
      }
      local okst, st = pcall(function() return b:getSubtype() end)
      if okst and st and st >= 0 then
        if bt == df.building_type.Workshop then r.subtype = enum_name(df.workshop_type, st)
        elseif bt == df.building_type.Furnace then r.subtype = enum_name(df.furnace_type, st)
        elseif bt == df.building_type.Civzone then r.subtype = enum_name(df.civzone_type, st)
        elseif bt == df.building_type.Trap then r.subtype = enum_name(df.trap_type, st)
        elseif bt == df.building_type.Construction then r.subtype = enum_name(df.construction_type, st)
        else r.subtype = st end
      end
      pcall(function() if b:getBuildStage() < b:getMaxBuildStage() then r.unfinished = true end end)
      -- zones: location link and assigned owner
      pcall(function()
        if bt == df.building_type.Civzone then
          if b.location_id and b.location_id >= 0 then r.location_id = b.location_id end
          if b.assigned_unit_id and b.assigned_unit_id >= 0 then r.owner = b.assigned_unit_id end
          if b.zone_num then r.zone_num = b.zone_num end
        end
      end)
      pcall(function() if b.owner then r.owner = b.owner.id end end)
      -- what a piece of furniture is made of, and its quality
      pcall(function()
        for _, ci in ipairs(b.contained_items) do
          if ci.use_mode == 2 then
            local it = ci.item
            r.item = dfhack.items.getDescription(it, 0, true)
            r.quality = it:getQuality()
            if it.flags.artifact then r.artifact = true end
            break
          end
        end
      end)
      -- workshop/furnace standing orders in progress
      pcall(function()
        if #b.jobs > 0 then
          r.jobs = {}
          for _, j in ipairs(b.jobs) do table.insert(r.jobs, enum_name(df.job_type, j.job_type)) end
        end
      end)
      -- room extents (bitmask of which tiles in the rectangle belong to it)
      pcall(function()
        if b.room and b.room.extents and b.room.width > 0 then
          local rows = {}
          for yy = 0, b.room.height - 1 do
            local row = {}
            for xx = 0, b.room.width - 1 do
              row[#row + 1] = (b.room.extents[yy * b.room.width + xx] ~= 0) and '1' or '0'
            end
            rows[#rows + 1] = table.concat(row)
          end
          r.extents = { x = b.room.x, y = b.room.y, w = b.room.width, h = b.room.height, rows = rows }
        end
      end)
      return r
    end)
    table.insert(list, ok and e or { error = tostring(e) })
  end
  return list
end)

-- 7. Stocks ---------------------------------------------------------------------
local stocks = section('stocks', function()
  local counts = {}
  for _, it in ipairs(df.global.world.items.other.IN_PLAY) do
    pcall(function()
      if it.flags.trader then return end
      local t = enum_name(df.item_type, it:getType())
      local mi = dfhack.matinfo.decode(it)
      local mat = mi and mi:toString() or 'unknown'
      local key = t .. '|' .. mat
      local c = counts[key] or { type = t, material = mat, count = 0, value = 0 }
      c.count = c.count + it:getStackSize()
      local okv, v = pcall(dfhack.items.getValue, it)
      if okv and type(v) == 'number' then c.value = c.value + v end
      counts[key] = c
    end)
  end
  local list = {}
  for _, c in pairs(counts) do table.insert(list, c) end
  table.sort(list, function(a, b)
    if a.type == b.type then return a.material < b.material end
    return a.type < b.type
  end)
  return list
end)

-- 8. Artifacts ------------------------------------------------------------------
local artifacts = section('artifacts', function()
  local list = {}
  for _, a in ipairs(df.global.world.artifacts.all) do
    pcall(function()
      local it = a.item
      if not it then return end
      local here = false
      pcall(function() here = dfhack.items.getPosition(it) ~= nil end)
      local r = {
        id = a.id,
        name = tr(a.name, false), name_english = tr(a.name, true),
        item = dfhack.items.getDescription(it, 0, true),
        type = enum_name(df.item_type, it:getType()),
        material = (dfhack.matinfo.decode(it) or {toString=function() return nil end}):toString(),
        on_map = here,
      }
      pcall(function() r.maker = hf_ref(it.maker) end)
      pcall(function()
        local p = dfhack.items.getPosition(it)
        if p then r.pos = { x = p.x, y = p.y, z = p.z } end
      end)
      if here then table.insert(list, r) end
    end)
  end
  return list
end)

-- 9. Animals: tame creatures of the fort, counted by race ---------------------
local animals = section('animals', function()
  local counts = {}
  for _, u in ipairs(df.global.world.units.active) do
    pcall(function()
      if dfhack.units.isCitizen(u) or dfhack.units.isDead(u) then return end
      if u.civ_id ~= pi.civ_id or not dfhack.units.isTame(u) then return end
      local race = dfhack.units.getRaceName(u)
      counts[race] = (counts[race] or 0) + 1
    end)
  end
  local list = {}
  for k, v in pairs(counts) do table.insert(list, { race = k, count = v }) end
  table.sort(list, function(a, b) return a.count > b.count end)
  return list
end)

-- 9b. Everyone on the map, of every race: citizens, residents, visitors, guests,
--     merchants, invaders, wild things. Intelligent folk get a full record.
local function creature_of(u)
  local ok, cr = pcall(function() return df.global.world.raws.creatures.all[u.race] end)
  return ok and cr or nil
end
local function is_intelligent(cr)
  if not cr then return false end
  local yes = false
  pcall(function() yes = cr.flags.HAS_ANY_INTELLIGENT_LEARNS or cr.flags.HAS_ANY_INTELLIGENT_SPEAKS end)
  return yes and true or false
end
local function unit_role(u)
  local function q(f) local ok, r = pcall(f, u); return ok and r end
  if q(dfhack.units.isDead) then return 'dead' end
  if q(dfhack.units.isCitizen) then return 'citizen' end
  if q(dfhack.units.isResident) then return 'resident' end
  if q(dfhack.units.isMerchant) then return 'merchant' end
  if q(dfhack.units.isDiplomat) then return 'diplomat' end
  if q(dfhack.units.isVisitor) then return 'visitor' end
  if q(dfhack.units.isInvader) then return 'invader' end
  if q(dfhack.units.isUndead) then return 'undead' end
  if q(dfhack.units.isTame) and u.civ_id == pi.civ_id then return 'pet' end
  if q(dfhack.units.isDanger) then return 'hostile' end
  if q(dfhack.units.isWildlife) then return 'wildlife' end
  return 'other'
end
local folk = section('folk', function()
  local list, by_race = {}, {}
  for _, u in ipairs(df.global.world.units.active) do
    pcall(function()
      local cr = creature_of(u)
      local role = unit_role(u)
      local race = cr and cr.creature_id or tostring(u.race)
      local smart = is_intelligent(cr)
      local key = race .. '|' .. role
      local g = by_race[key] or { race = race, race_name = cr and cr.name[0] or race, role = role,
                                  intelligent = smart, count = 0 }
      g.count = g.count + 1
      by_race[key] = g
      if smart and role ~= 'dead' then
        local vn = dfhack.units.getVisibleName(u)
        local r = { id = u.id, hf = u.hist_figure_id, role = role, race = race,
                    race_name = cr and cr.name[0] or race,
                    caste = (pcall(function() return cr.caste[u.caste].caste_id end)) and cr.caste[u.caste].caste_id or nil,
                    name = tr(vn, false), name_english = tr(vn, true),
                    readable = dfhack.units.getReadableName(u),
                    profession = dfhack.units.getProfessionName(u),
                    age = (pcall(dfhack.units.getAge, u, true)) and dfhack.units.getAge(u, true) or nil,
                    sex = u.sex, pos = { x = u.pos.x, y = u.pos.y, z = u.pos.z },
                    civ_id = u.civ_id, squad = squad_of[u.id], skills = {}, positions = {} }
        pcall(function()
          local civ = df.historical_entity.find(u.civ_id)
          if civ then r.civ = tr(civ.name, true) end
        end)
        pcall(function()
          for _, s in ipairs(u.status.current_soul.skills) do
            if s.rating >= 3 then
              table.insert(r.skills, { skill = enum_name(df.job_skill, s.id), level = s.rating })
            end
          end
        end)
        pcall(function()
          for _, p in ipairs(dfhack.units.getNoblePositions(u) or {}) do
            if p.position and p.position.code then table.insert(r.positions, p.position.code) end
          end
        end)
        table.insert(list, r)
      end
    end)
  end
  local groups = {}
  for _, g in pairs(by_race) do table.insert(groups, g) end
  table.sort(groups, function(a, b) return a.count > b.count end)
  return { groups = groups, people = list }
end)

-- 9c. The world's peoples: every civilization and every intelligent race ------
local world_races = section('world_races', function()
  local civs = {}
  for _, e in ipairs(df.global.world.entities.all) do
    pcall(function()
      local t = enum_name(df.historical_entity_type, e.type)
      if t ~= 'Civilization' then return end
      local cr = df.global.world.raws.creatures.all[e.race]
      local sites = 0
      pcall(function() sites = #e.site_links end)
      local pop = 0
      pcall(function() for _, hf in ipairs(e.histfig_ids) do pop = pop + 1 end end)
      table.insert(civs, { id = e.id, name = tr(e.name, false), name_english = tr(e.name, true),
                           race = cr and cr.creature_id, race_name = cr and cr.name[0],
                           sites = sites, figures = pop, ours = (e.id == pi.civ_id) })
    end)
  end
  local races = {}
  for i, cr in ipairs(df.global.world.raws.creatures.all) do
    pcall(function()
      if not is_intelligent(cr) then return end
      local r = { id = i, creature_id = cr.creature_id, name = cr.name[0], plural = cr.name[1],
                  adjective = cr.name[2], castes = {} }
      pcall(function() r.description = cr.caste[0].description end)
      pcall(function() for _, c in ipairs(cr.caste) do table.insert(r.castes, c.caste_id) end end)
      pcall(function() r.source = cr.source_hfid >= 0 and 'generated' or 'raw' end)
      pcall(function() if cr.flags.EQUIPMENT then r.uses_equipment = true end end)
      pcall(function() r.flags = { megabeast = cr.flags.CASTE_MEGABEAST or nil,
                                   night = cr.flags.CASTE_NIGHT_CREATURE_ANY or nil,
                                   underground = cr.flags.CASTE_UNDERGROUND_DEPTH or nil } end)
      table.insert(races, r)
    end)
  end
  local mods = {}
  pcall(function()
    for _, m in ipairs(df.global.world.object_loader.object_load_order_name) do
      table.insert(mods, m.value or tostring(m))
    end
  end)
  return { civilizations = civs, intelligent_races = races, mods = mods }
end)

-- 10. The dead of the fort --------------------------------------------------------
local dead = section('dead', function()
  local list = {}
  for _, u in ipairs(df.global.world.units.all) do
    pcall(function()
      if not dfhack.units.isDead(u) then return end
      if u.civ_id ~= pi.civ_id then return end
      if not (dfhack.units.isDwarf(u) or dfhack.units.isSane(u)) then return end
      local r = unit_ref(u)
      pcall(function()
        local inc = df.incident.find(u.counters.death_id)
        if inc then r.cause = enum_name(df.death_type, inc.death_cause); r.year = inc.event_year end
      end)
      table.insert(list, r)
    end)
  end
  return list
end)

-- 11. The fort's history: every recorded event at this site -----------------------
local events = section('events', function()
  local list = {}
  local counts = {}
  for _, ev in ipairs(df.global.world.history.events) do
    pcall(function()
      local s = nil
      pcall(function() s = ev.site end)
      if s == nil then pcall(function() s = ev.site_id end) end
      if s ~= pi.site_id then return end
      local t = enum_name(df.history_event_type, ev:getType())
      counts[t] = (counts[t] or 0) + 1
      local r = { id = ev.id, year = ev.year, type = t }
      for _, f in ipairs({ 'hfid', 'histfig', 'victim', 'slayer', 'slayer_hf', 'creator_hfid',
                           'artifact', 'artifact_id', 'death_cause', 'building', 'structure',
                           'attacker_civ', 'defender_civ', 'entity', 'civ', 'mood', 'reason',
                           'hist_figure_id', 'snatcher', 'target', 'woundee', 'wounder' }) do
        pcall(function()
          local v = ev[f]
          if type(v) == 'number' and v >= 0 then r[f] = v end
        end)
      end
      table.insert(list, r)
    end)
  end
  return { counts = counts, events = list }
end)

-- 12. Engravings (where the fort's walls tell stories) ---------------------------
local engravings = section('engravings', function()
  local list = {}
  for _, e in ipairs(df.global.world.event.engravings or df.global.world.engravings) do
    pcall(function()
      table.insert(list, { x = e.pos.x, y = e.pos.y, z = e.pos.z, quality = e.quality,
                           art_id = e.art_id, art_subid = e.art_subid })
    end)
  end
  return { count = #list, list = list }
end)

-- 13. The map, tile by tile --------------------------------------------------------
-- SHAPE  ? unrevealed   (space) air   . floor   # wall   < up stair   > down stair
--        X up/down stair   ^ ramp   v ramp top   F fortification   T tree part
--        " shrub/sapling   , boulder/pebbles   ~ brook bed   O endless pit
-- MAT    ? unknown   a air   s soil   r stone   m mineral vein   c constructed
--        g grass   p plant/tree   i ice   f feature (adamantine/etc)   l lava stone
--        h hell/underworld   w pool/brook/river   o root/mushroom
-- LIQUID (space) none   1-7 water depth   A-G magma depth (A=1 .. G=7)
-- ROCK   (space) not a rock wall/floor; otherwise a symbol from tiles.rock_legend
local tiles = section('tiles', function()
  local m = df.global.world.map
  local SH = {
    WALL='#', FLOOR='.', EMPTY=' ', NONE=' ', STAIR_UP='<', STAIR_DOWN='>', STAIR_UPDOWN='X',
    RAMP='^', RAMP_TOP='v', FORTIFICATION='F', TRUNK_BRANCH='T', BRANCH='T', TWIG='T',
    SAPLING='"', SHRUB='"', BOULDER=',', PEBBLES=',', BROOK_BED='~', BROOK_TOP='~', ENDLESS_PIT='O',
  }
  local MT = {
    AIR='a', SOIL='s', STONE='r', MINERAL='m', CONSTRUCTION='c', GRASS_LIGHT='g', GRASS_DARK='g',
    GRASS_DRY='g', GRASS_DEAD='g', PLANT='p', TREE='p', FROZEN_LIQUID='i', FEATURE='f',
    LAVA_STONE='l', HFS='h', UNDERWORLD_GATE='h', POOL='w', BROOK='w', RIVER='w', ROOT='o',
    MUSHROOM='o', MAGMA='l', DRIFTWOOD='p', CAMPFIRE='c', FIRE='c', ASHES='s',
  }
  local tm_ok, tm = pcall(require, 'tile-material')
  local rock_sym, rock_legend, next_sym = {}, {}, 1
  local SYMS = 'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!@$%&*+=-:;<>()[]{}/|'
  local function rock_char(x, y, z)
    if not tm_ok then return ' ' end
    local ok, mi = pcall(tm.GetTileMat, x, y, z)
    if not ok or not mi then return ' ' end
    local key = mi:getToken()
    local c = rock_sym[key]
    if not c then
      if next_sym > #SYMS then return '?' end
      c = SYMS:sub(next_sym, next_sym); next_sym = next_sym + 1
      rock_sym[key] = c
      table.insert(rock_legend, { sym = c, token = key, name = mi:toString() })
    end
    return c
  end
  local levels = {}
  local X, Y = m.x_count, m.y_count
  for z = m.z_count - 1, 0, -1 do
    local shape, mat, liq, rock = {}, {}, {}, {}
    local any = false
    for y = 0, Y - 1 do
      local sr, mr, lr, rr = {}, {}, {}, {}
      for x = 0, X - 1 do
        local tt = dfhack.maps.getTileType(x, y, z)
        local des = dfhack.maps.getTileFlags(x, y, z)
        if not tt or not des or des.hidden then
          sr[x + 1] = '?'; mr[x + 1] = '?'; lr[x + 1] = ' '; rr[x + 1] = ' '
        else
          any = true
          local at = df.tiletype.attrs[tt]
          local sh = enum_name(df.tiletype_shape, at.shape)
          local mn = enum_name(df.tiletype_material, at.material)
          local sc = SH[sh] or '.'
          local mc = MT[mn] or '?'
          sr[x + 1] = sc; mr[x + 1] = mc
          local fs = des.flow_size or 0
          if fs > 0 then
            if des.liquid_type == df.tile_liquid.Magma then lr[x + 1] = string.char(64 + fs)
            else lr[x + 1] = tostring(fs) end
          else lr[x + 1] = ' ' end
          if (mc == 'r' or mc == 'm' or mc == 'l' or mc == 'f' or mc == 's') and (sc == '#' or sc == '.') then
            rr[x + 1] = rock_char(x, y, z)
          else rr[x + 1] = ' ' end
        end
      end
      shape[#shape + 1] = table.concat(sr); mat[#mat + 1] = table.concat(mr)
      liq[#liq + 1] = table.concat(lr); rock[#rock + 1] = table.concat(rr)
    end
    if any then
      local f = io.open(string.format('%s/tiles/z%03d.txt', OUT, z), 'w')
      if f then
        f:write(string.format('Z %d %dx%d\n', z, X, Y))
        f:write('SHAPE\n'); f:write(table.concat(shape, '\n')); f:write('\n')
        f:write('MAT\n'); f:write(table.concat(mat, '\n')); f:write('\n')
        f:write('LIQUID\n'); f:write(table.concat(liq, '\n')); f:write('\n')
        f:write('ROCK\n'); f:write(table.concat(rock, '\n')); f:write('\n')
        f:close()
      end
      table.insert(levels, z)
    end
  end
  return {
    levels = levels,
    rock_legend = rock_legend,
    rock_by_tile = tm_ok,
    legend = {
      SHAPE = '? unrevealed, space air, . floor, # wall, < up stair, > down stair, X up/down stair, ^ ramp, v ramp top, F fortification, T tree, " shrub, , boulder/pebbles, ~ brook, O endless pit',
      MAT = '? unknown, a air, s soil, r stone, m mineral vein, c constructed, g grass, p plant/tree, i ice, f feature, l lava stone, h underworld, w water body, o root/mushroom',
      LIQUID = 'space none, 1-7 water depth, A-G magma depth',
      ROCK = 'space n/a, else symbol from rock_legend',
    },
  }
end)

-- 14. Geology -----------------------------------------------------------------------
local prospect = section('prospect', function()
  local out, status = dfhack.run_command_silent('prospect', 'all')
  write_text(OUT .. '/prospect.txt', out)
  return { status = status, file = 'prospect.txt' }
end)

local data = {
  exported_by = 'angelx-kingdom-export.lua (read-only)',
  fortress = fortress, wealth = wealth, citizens = citizens, squads = squads,
  locations = locations, buildings = buildings, stocks = stocks, artifacts = artifacts,
  animals = animals, folk = folk, world_races = world_races,
  dead = dead, events = events, engravings = engravings,
  tiles = tiles, prospect = prospect, log = log,
}

local ok, err = pcall(json.encode_file, data, OUT .. '/kingdom.json', { pretty = true })
if not ok then ok, err = pcall(json.encode_file, data, OUT .. '/kingdom.json') end
if ok then print('[angelx-export] Done. Files are in ' .. OUT)
else print('[angelx-export] Could not write kingdom.json: ' .. tostring(err)) end
