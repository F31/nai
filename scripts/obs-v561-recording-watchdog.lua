-- Dedicated isolated acceptance recorder only. No networking or process control.
local obs = obslua
local root, nonce = "", ""
local started, requested, stopped = false, false, false
local media_source, last_sequence, ticks, armed, revoked = "", 0, 0, false, false

local function read_token(path)
    local file = io.open(path, "rb")
    if not file then return nil end
    local token = file:read(33) -- bounded: oversized and newline-suffixed tokens fail
    file:close()
    return token
end

local function set_media(enabled)
    local source = obs.obs_get_source_by_name(media_source)
    if not source then return false end
    local scene_source = obs.obs_frontend_get_current_scene()
    local scene = scene_source and obs.obs_scene_from_source(scene_source)
    local item = scene and obs.obs_scene_find_source(scene, media_source)
    if not enabled then obs.obs_source_set_muted(source, true) end
    if item then
        obs.obs_sceneitem_set_visible(item, enabled)
        if enabled then obs.obs_source_set_muted(source, false) end
    end
    if scene_source then obs.obs_source_release(scene_source) end
    obs.obs_source_release(source)
    return item ~= nil
end

local function poll_media()
    if media_source == "" or not started or stopped or revoked then return end
    local file = io.open(root .. "/media.lease", "rb")
    local value = file and file:read(42)
    if file then file:close() end
    local sequence = value and #value == 41 and value:sub(1,33) == nonce .. ":"
        and value:sub(34):match("^%d+$") and tonumber(value:sub(34))
    if sequence == 0 then
        revoked = true
        set_media(false)
        print("nai media lease revoked explicitly")
        return
    end
    if sequence and sequence > last_sequence then
        last_sequence, ticks = sequence, 5
        if not armed then
            armed = set_media(true)
            if armed then print("nai media lease armed") end
        end
    elseif armed then
        ticks = ticks - 1
        if ticks <= 0 then
            revoked = true
            set_media(false)
            print("nai media lease revoked")
        end
    end
end

local function event(code)
    if code == obs.OBS_FRONTEND_EVENT_RECORDING_STARTED then started = true end
    if code == obs.OBS_FRONTEND_EVENT_RECORDING_STOPPED and started then
        stopped = true
        if armed then set_media(false) end
        local file = io.open(root .. "/stop.receipt", "wb")
        if file then file:write(nonce); file:close() end
    end
end

local function poll()
    poll_media()
    if started and not stopped and not requested
        and read_token(root .. "/stop.request") == nonce
        and obs.obs_frontend_recording_active() then
        requested = true
        obs.obs_frontend_recording_stop()
    end
end

function script_description()
    return "nai isolated recording: run-bound normal stop; never starts recording or streaming."
end

function script_load(settings)
    root = obs.obs_data_get_string(settings, "artifacts")
    nonce = obs.obs_data_get_string(settings, "nonce")
    media_source = obs.obs_data_get_string(settings, "media_source") or ""
    if #nonce ~= 32 or not nonce:match("^[0-9a-f]+$") or root == "" then return end
    obs.obs_frontend_add_event_callback(event)
    obs.timer_add(poll, 100)
end

function script_unload()
    obs.timer_remove(poll)
    obs.obs_frontend_remove_event_callback(event)
end
