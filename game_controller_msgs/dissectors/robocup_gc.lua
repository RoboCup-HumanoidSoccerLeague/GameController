--[[
    Wireshark dissector for the RoboCup GameController protocol
    ------------------------------------------------------------

    Messages (see game_controller_msgs/src/ and game_controller_net/src/):

      UDP 3838  Control message                GameController -> players   "RGme", version 20, 158 bytes
      UDP 3838  Control message (true data)    GameController -> monitors  "RGTD", version 20, 158 bytes
      UDP 3939  Status message                 players -> GameController   "RGrt", version 4, 32 bytes
      UDP 3940  Forwarded status message       GameController -> monitors  IPv4 address of the player
                                                                           + status message, 36 bytes
      UDP 3636  Monitor request                monitors -> GameController  "RGTr", version 0, 5 bytes

    Team messages (UDP 10000 + team number) have no fixed format and are not dissected.

    The layouts follow game_controller_msgs/headers/RoboCupGameControlData.h. The structs are
    packed and multi-byte values are little endian.

    Installation: copy (or symlink) this file into Wireshark's personal Lua plugins directory
    (see Help > About Wireshark > Folders, e.g. ~/.local/lib/wireshark/plugins), or load it
    explicitly with `wireshark -X lua_script:robocup_gc.lua`. Display filter: `robocup_gc`.
]]

local robocup_gc = Proto("robocup_gc", "RoboCup GameController")

-----------------------------------------------------------------------
-- Constants
-----------------------------------------------------------------------

-- game_controller_msgs/src/lib.rs
local CONTROL_MESSAGE_PORT = 3838
local STATUS_MESSAGE_PORT = 3939
local STATUS_MESSAGE_FORWARD_PORT = STATUS_MESSAGE_PORT + 1
local MONITOR_REQUEST_PORT = 3636

-- game_controller_msgs/headers/RoboCupGameControlData.h
local CONTROL_MESSAGE_HEADER = "RGme"
local CONTROL_MESSAGE_TRUE_DATA_HEADER = "RGTD"
local CONTROL_MESSAGE_VERSION = 20
local STATUS_MESSAGE_HEADER = "RGrt"
local STATUS_MESSAGE_VERSION = 4
local MAX_NUM_PLAYERS = 20
local KICKING_TEAM_NONE = 255

-- game_controller_msgs/src/monitor_request.rs
local MONITOR_REQUEST_HEADER = "RGTr"
local MONITOR_REQUEST_VERSION = 0

local ROBOT_INFO_SIZE = 3
local TEAM_INFO_SIZE = 10 + MAX_NUM_PLAYERS * ROBOT_INFO_SIZE -- 70
local CONTROL_MESSAGE_SIZE = 18 + 2 * TEAM_INFO_SIZE -- 158
local STATUS_MESSAGE_SIZE = 32
local FORWARDED_STATUS_MESSAGE_SIZE = 4 + STATUS_MESSAGE_SIZE -- 36
local MONITOR_REQUEST_SIZE = 5

-----------------------------------------------------------------------
-- Value strings
-----------------------------------------------------------------------

local competition_type_vals = {
    [0] = "Small",
    [1] = "Middle",
    [2] = "Large",
}

local game_phase_vals = {
    [0] = "Normal",
    [1] = "Penalty Shoot-out",
    [2] = "Extra Time",
    [3] = "Timeout",
}

local state_vals = {
    [0] = "Initial",
    [1] = "Ready",
    [2] = "Set",
    [3] = "Playing",
    [4] = "Finished",
}

local set_play_vals = {
    [0] = "None",
    [1] = "Direct Free Kick",
    [2] = "Indirect Free Kick",
    [3] = "Penalty Kick",
    [4] = "Throw-in",
    [5] = "Goal Kick",
    [6] = "Corner Kick",
}

local team_colour_vals = {
    [0] = "Blue",
    [1] = "Red",
    [2] = "Yellow",
    [3] = "Black",
    [4] = "White",
    [5] = "Green",
    [6] = "Orange",
    [7] = "Purple",
    [8] = "Brown",
    [9] = "Gray",
}

local penalty_vals = {
    [0] = "None",
    [1] = "Illegal Positioning",
    [2] = "Motion in Set",
    [3] = "Motion in Stop",
    [4] = "Local Game Stuck",
    [5] = "Incapable Robot",
    [6] = "Pick-up",
    [7] = "Ball Holding",
    [8] = "Leaving the Field",
    [9] = "Playing with Arms/Hands",
    [10] = "Pushing",
    [11] = "Cautioned",
    [12] = "Sent Off",
    [13] = "Substitute",
}

local bool_vals = {
    [0] = "No",
    [1] = "Yes",
}

local function val_to_str(vals, value)
    return vals[value] or string.format("Unknown (%d)", value)
end

-----------------------------------------------------------------------
-- Protocol fields
-----------------------------------------------------------------------

local f = {
    -- Common
    type = ProtoField.string("robocup_gc.type", "Message Type"),
    header = ProtoField.string("robocup_gc.header", "Header"),
    version = ProtoField.uint8("robocup_gc.version", "Version", base.DEC),

    -- Control message (RoboCupGameControlData)
    true_data = ProtoField.bool("robocup_gc.control.true_data", "True Data"),
    packet_number = ProtoField.uint8("robocup_gc.control.packet_number", "Packet Number", base.DEC),
    players_per_team = ProtoField.uint8("robocup_gc.control.players_per_team", "Players per Team", base.DEC),
    competition_type = ProtoField.uint8("robocup_gc.control.competition_type", "Competition Type", base.DEC, competition_type_vals),
    stopped = ProtoField.uint8("robocup_gc.control.stopped", "Stopped", base.DEC, bool_vals),
    game_phase = ProtoField.uint8("robocup_gc.control.game_phase", "Game Phase", base.DEC, game_phase_vals),
    state = ProtoField.uint8("robocup_gc.control.state", "State", base.DEC, state_vals),
    set_play = ProtoField.uint8("robocup_gc.control.set_play", "Set Play", base.DEC, set_play_vals),
    first_half = ProtoField.uint8("robocup_gc.control.first_half", "First Half", base.DEC, bool_vals),
    kicking_team = ProtoField.uint8("robocup_gc.control.kicking_team", "Kicking Team", base.DEC),
    secs_remaining = ProtoField.int16("robocup_gc.control.secs_remaining", "Seconds Remaining", base.DEC),
    secondary_time = ProtoField.int16("robocup_gc.control.secondary_time", "Secondary Time", base.DEC),

    -- TeamInfo
    team_number = ProtoField.uint8("robocup_gc.control.team.number", "Team Number", base.DEC),
    field_player_colour = ProtoField.uint8("robocup_gc.control.team.field_player_colour", "Field Player Colour", base.DEC, team_colour_vals),
    goalkeeper_colour = ProtoField.uint8("robocup_gc.control.team.goalkeeper_colour", "Goalkeeper Colour", base.DEC, team_colour_vals),
    goalkeeper = ProtoField.uint8("robocup_gc.control.team.goalkeeper", "Goalkeeper", base.DEC),
    score = ProtoField.uint8("robocup_gc.control.team.score", "Score", base.DEC),
    penalty_shot = ProtoField.uint8("robocup_gc.control.team.penalty_shot", "Penalty Shot Counter", base.DEC),
    single_shots = ProtoField.uint16("robocup_gc.control.team.single_shots", "Single Shots", base.HEX),
    message_budget = ProtoField.uint16("robocup_gc.control.team.message_budget", "Message Budget", base.DEC),

    -- RobotInfo
    player_number = ProtoField.uint8("robocup_gc.control.player.number", "Player Number", base.DEC),
    penalty = ProtoField.uint8("robocup_gc.control.player.penalty", "Penalty", base.DEC, penalty_vals),
    secs_till_unpenalised = ProtoField.uint8("robocup_gc.control.player.secs_till_unpenalised", "Seconds till Unpenalised", base.DEC),
    cautions = ProtoField.uint8("robocup_gc.control.player.cautions", "Cautions", base.DEC),

    -- Status message (RoboCupGameControlReturnData)
    status_player_number = ProtoField.uint8("robocup_gc.status.player_number", "Player Number", base.DEC),
    status_team_number = ProtoField.uint8("robocup_gc.status.team_number", "Team Number", base.DEC),
    fallen = ProtoField.uint8("robocup_gc.status.fallen", "Fallen", base.DEC, bool_vals),
    pose_x = ProtoField.float("robocup_gc.status.pose.x", "X [mm]"),
    pose_y = ProtoField.float("robocup_gc.status.pose.y", "Y [mm]"),
    pose_theta = ProtoField.float("robocup_gc.status.pose.theta", "Theta [rad]"),
    ball_age = ProtoField.float("robocup_gc.status.ball_age", "Ball Age [s]"),
    ball_x = ProtoField.float("robocup_gc.status.ball.x", "X [mm]"),
    ball_y = ProtoField.float("robocup_gc.status.ball.y", "Y [mm]"),

    -- Forwarded status message
    forward_source = ProtoField.ipv4("robocup_gc.forward.source", "Original Sender"),
}

robocup_gc.fields = {}
for _, field in pairs(f) do
    table.insert(robocup_gc.fields, field)
end

local e = {
    too_short = ProtoExpert.new("robocup_gc.too_short", "Message too short",
        expert.group.MALFORMED, expert.severity.ERROR),
    too_long = ProtoExpert.new("robocup_gc.too_long", "Message too long",
        expert.group.MALFORMED, expert.severity.WARN),
    wrong_version = ProtoExpert.new("robocup_gc.wrong_version", "Unexpected version",
        expert.group.PROTOCOL, expert.severity.WARN),
    invalid_value = ProtoExpert.new("robocup_gc.invalid_value", "Invalid value",
        expert.group.PROTOCOL, expert.severity.WARN),
}

robocup_gc.experts = { e.too_short, e.too_long, e.wrong_version, e.invalid_value }

-----------------------------------------------------------------------
-- Helper functions
-----------------------------------------------------------------------

local function format_time(secs)
    local sign = secs < 0 and "-" or ""
    secs = math.abs(secs)
    return string.format("%s%d:%02d", sign, math.floor(secs / 60), secs % 60)
end

local function is_nan(value)
    return value ~= value
end

-- Adds header and version. Returns false if the message is too short to be dissected. A
-- message that is too long is still dissected (the GameController rejects it, though).
local function add_header(tree, buffer, offset, size, expected_version)
    local length = buffer:len() - offset
    tree:add(f.header, buffer(offset, 4))
    if length < 5 then
        tree:add_proto_expert_info(e.too_short,
            string.format("Message too short: expected %d bytes, got %d", size, length))
        return false
    end
    local version_item = tree:add(f.version, buffer(offset + 4, 1))
    local version = buffer(offset + 4, 1):uint()
    if version ~= expected_version then
        version_item:add_proto_expert_info(e.wrong_version,
            string.format("Unexpected version %d (expected %d)", version, expected_version))
    end
    if length < size then
        tree:add_proto_expert_info(e.too_short,
            string.format("Message too short: expected %d bytes, got %d", size, length))
        return false
    elseif length > size then
        tree:add_proto_expert_info(e.too_long,
            string.format("Message too long: expected %d bytes, got %d", size, length))
    end
    return true
end

-----------------------------------------------------------------------
-- RobotInfo
--
--   +0 penalty
--   +1 secsTillUnpenalised
--   +2 cautions
-----------------------------------------------------------------------

local function dissect_robot_info(tree, buffer, offset, player)
    local penalty = buffer(offset, 1):uint()
    local secs_till_unpenalised = buffer(offset + 1, 1):uint()
    local cautions = buffer(offset + 2, 1):uint()

    local summary = penalty == 0 and "No penalty" or val_to_str(penalty_vals, penalty)
    if secs_till_unpenalised > 0 then
        summary = string.format("%s, %d s", summary, secs_till_unpenalised)
    end
    if cautions > 0 then
        summary = string.format("%s, %d caution%s", summary, cautions, cautions == 1 and "" or "s")
    end

    local player_tree = tree:add(buffer(offset, ROBOT_INFO_SIZE),
        string.format("Player %d: %s", player, summary))
    player_tree:add(f.player_number, player):set_generated()
    player_tree:add(f.penalty, buffer(offset, 1))
    player_tree:add(f.secs_till_unpenalised, buffer(offset + 1, 1))
    player_tree:add(f.cautions, buffer(offset + 2, 1))
end

-----------------------------------------------------------------------
-- TeamInfo
--
--   +0  teamNumber
--   +1  fieldPlayerColour
--   +2  goalkeeperColour
--   +3  goalkeeper
--   +4  score
--   +5  penaltyShot
--   +6  singleShots[2]
--   +8  messageBudget[2]
--   +10 players[MAX_NUM_PLAYERS]
-----------------------------------------------------------------------

local function dissect_team_info(tree, buffer, offset, index)
    local number = buffer(offset, 1):uint()
    local score = buffer(offset + 4, 1):uint()
    local team_tree = tree:add(buffer(offset, TEAM_INFO_SIZE),
        string.format("Team %d: #%d, %s, Score %d", index, number,
            val_to_str(team_colour_vals, buffer(offset + 1, 1):uint()), score))

    team_tree:add(f.team_number, buffer(offset, 1))
    team_tree:add(f.field_player_colour, buffer(offset + 1, 1))
    team_tree:add(f.goalkeeper_colour, buffer(offset + 2, 1))
    local goalkeeper_item = team_tree:add(f.goalkeeper, buffer(offset + 3, 1))
    if buffer(offset + 3, 1):uint() == 0 then
        goalkeeper_item:append_text(" (None)")
    end
    team_tree:add(f.score, buffer(offset + 4, 1))

    local penalty_shot = buffer(offset + 5, 1):uint()
    team_tree:add(f.penalty_shot, buffer(offset + 5, 1))
    local single_shots_item = team_tree:add_le(f.single_shots, buffer(offset + 6, 2))
    if penalty_shot > 0 then
        -- Bit n - 1 is set if the n-th penalty shot was successful.
        local single_shots = buffer(offset + 6, 2):le_uint()
        local results = {}
        for shot = 1, math.min(penalty_shot, 16) do
            results[shot] = math.floor(single_shots / 2 ^ (shot - 1)) % 2 == 1 and "goal" or "miss"
        end
        single_shots_item:append_text(" (" .. table.concat(results, ", ") .. ")")
    end
    team_tree:add_le(f.message_budget, buffer(offset + 8, 2))

    for player = 1, MAX_NUM_PLAYERS do
        dissect_robot_info(team_tree, buffer, offset + 10 + (player - 1) * ROBOT_INFO_SIZE, player)
    end
end

-----------------------------------------------------------------------
-- Control message (RoboCupGameControlData)
--
--   0  header[4]
--   4  version
--   5  packetNumber
--   6  playersPerTeam
--   7  competitionType
--   8  stopped
--   9  gamePhase
--  10  state
--  11  setPlay
--  12  firstHalf
--  13  kickingTeam
--  14  secsRemaining[2]
--  16  secondaryTime[2]
--  18  teams[2]
-----------------------------------------------------------------------

local function dissect_control_message(buffer, pinfo, tree, true_data)
    local kind = true_data and "Control Message (True Data)" or "Control Message"
    local subtree = tree:add(robocup_gc, buffer(), "RoboCup GameController " .. kind)
    subtree:add(f.type, kind):set_generated()
    subtree:add(f.true_data, true_data):set_generated()
    pinfo.cols.info:set(kind)

    if not add_header(subtree, buffer, 0, CONTROL_MESSAGE_SIZE, CONTROL_MESSAGE_VERSION) then
        return buffer:len()
    end

    subtree:add(f.packet_number, buffer(5, 1))
    subtree:add(f.players_per_team, buffer(6, 1))
    subtree:add(f.competition_type, buffer(7, 1))
    subtree:add(f.stopped, buffer(8, 1))
    subtree:add(f.game_phase, buffer(9, 1))
    subtree:add(f.state, buffer(10, 1))
    subtree:add(f.set_play, buffer(11, 1))
    subtree:add(f.first_half, buffer(12, 1))
    local kicking_team_item = subtree:add(f.kicking_team, buffer(13, 1))
    if buffer(13, 1):uint() == KICKING_TEAM_NONE then
        kicking_team_item:append_text(" (None)")
    end
    local secs_remaining = buffer(14, 2):le_int()
    subtree:add_le(f.secs_remaining, buffer(14, 2)):append_text(" (" .. format_time(secs_remaining) .. ")")
    local secondary_time = buffer(16, 2):le_int()
    subtree:add_le(f.secondary_time, buffer(16, 2)):append_text(" (" .. format_time(secondary_time) .. ")")

    dissect_team_info(subtree, buffer, 18, 1)
    dissect_team_info(subtree, buffer, 18 + TEAM_INFO_SIZE, 2)

    -- e.g. "Control Message #12: Playing (Normal, 1st half), 3 : 1, 7:32 remaining"
    local game_phase = buffer(9, 1):uint()
    local info = string.format("%s #%d: %s%s (%s", kind, buffer(5, 1):uint(),
        buffer(8, 1):uint() ~= 0 and "Stopped, " or "",
        val_to_str(state_vals, buffer(10, 1):uint()),
        val_to_str(game_phase_vals, game_phase))
    if game_phase == 0 or game_phase == 2 then
        info = info .. (buffer(12, 1):uint() ~= 0 and ", 1st half" or ", 2nd half")
    end
    local set_play = buffer(11, 1):uint()
    if set_play ~= 0 then
        info = info .. ", " .. val_to_str(set_play_vals, set_play)
    end
    info = string.format("%s), #%d %d : %d #%d, %s remaining", info,
        buffer(18, 1):uint(), buffer(18 + 4, 1):uint(),
        buffer(18 + TEAM_INFO_SIZE + 4, 1):uint(), buffer(18 + TEAM_INFO_SIZE, 1):uint(),
        format_time(secs_remaining))
    pinfo.cols.info:set(info)

    return buffer:len()
end

-----------------------------------------------------------------------
-- Status message (RoboCupGameControlReturnData)
--
--   0  header[4]
--   4  version
--   5  playerNum
--   6  teamNum
--   7  fallen
--   8  pose[3]  float
--  20  ballAge   float
--  24  ball[2]  float
-----------------------------------------------------------------------

-- Returns a summary for the info column, or nil if the message is too short.
local function dissect_status_message(buffer, offset, tree)
    tree:add(f.type, "Status Message"):set_generated()
    if not add_header(tree, buffer, offset, STATUS_MESSAGE_SIZE, STATUS_MESSAGE_VERSION) then
        return nil
    end

    -- The validity checks are the same as in game_controller_msgs/src/status_message.rs.
    local player_number = buffer(offset + 5, 1):uint()
    local player_number_item = tree:add(f.status_player_number, buffer(offset + 5, 1))
    if player_number < 1 or player_number > MAX_NUM_PLAYERS then
        player_number_item:add_proto_expert_info(e.invalid_value,
            string.format("Invalid player number (must be 1-%d)", MAX_NUM_PLAYERS))
    end
    local team_number = buffer(offset + 6, 1):uint()
    tree:add(f.status_team_number, buffer(offset + 6, 1))
    local fallen = buffer(offset + 7, 1):uint()
    local fallen_item = tree:add(f.fallen, buffer(offset + 7, 1))
    if fallen > 1 then
        fallen_item:add_proto_expert_info(e.invalid_value, "Invalid fallen value (must be 0 or 1)")
    end

    local function add_float(subtree, field, field_offset, name)
        local range = buffer(offset + field_offset, 4)
        local item = subtree:add_le(field, range)
        if is_nan(range:le_float()) then
            item:add_proto_expert_info(e.invalid_value, name .. " is NaN")
        end
        return item, range:le_float()
    end

    local pose_x = buffer(offset + 8, 4):le_float()
    local pose_y = buffer(offset + 12, 4):le_float()
    local pose_theta = buffer(offset + 16, 4):le_float()
    local pose_tree = tree:add(buffer(offset + 8, 12),
        string.format("Pose: (%.0f mm, %.0f mm, %.1f°)", pose_x, pose_y, math.deg(pose_theta)))
    add_float(pose_tree, f.pose_x, 8, "Pose x")
    add_float(pose_tree, f.pose_y, 12, "Pose y")
    local theta_item = add_float(pose_tree, f.pose_theta, 16, "Pose theta")
    theta_item:append_text(string.format(" (%.1f°)", math.deg(pose_theta)))

    local ball_age_item, ball_age = add_float(tree, f.ball_age, 20, "Ball age")
    if ball_age == -1 then
        ball_age_item:append_text(" (never seen)")
    end

    local ball_tree = tree:add(buffer(offset + 24, 8),
        string.format("Ball (relative): (%.0f mm, %.0f mm)",
            buffer(offset + 24, 4):le_float(), buffer(offset + 28, 4):le_float()))
    add_float(ball_tree, f.ball_x, 24, "Ball x")
    add_float(ball_tree, f.ball_y, 28, "Ball y")

    return string.format("Team %d, Player %d%s", team_number, player_number,
        fallen == 1 and ", fallen" or "")
end

local function dissect_status(buffer, pinfo, tree)
    local subtree = tree:add(robocup_gc, buffer(), "RoboCup GameController Status Message")
    pinfo.cols.info:set("Status Message")
    local summary = dissect_status_message(buffer, 0, subtree)
    if summary then
        pinfo.cols.info:set("Status Message: " .. summary)
    end
    return buffer:len()
end

-----------------------------------------------------------------------
-- Forwarded status message
--
--   0  IPv4 address of the original sender
--   4  RoboCupGameControlReturnData
-----------------------------------------------------------------------

local function dissect_forwarded_status(buffer, pinfo, tree)
    local subtree = tree:add(robocup_gc, buffer(), "RoboCup GameController Forwarded Status Message")
    subtree:add(f.type, "Forwarded Status Message"):set_generated()
    subtree:add(f.forward_source, buffer(0, 4))
    local source = tostring(buffer(0, 4):ipv4())
    pinfo.cols.info:set("Forwarded Status Message from " .. source)

    local status_tree = subtree:add(buffer(4), "Status Message")
    local summary = dissect_status_message(buffer, 4, status_tree)
    if summary then
        pinfo.cols.info:set(string.format("Forwarded Status Message from %s: %s", source, summary))
    end
    return buffer:len()
end

-----------------------------------------------------------------------
-- Monitor request
--
--   0  header[4]
--   4  version
-----------------------------------------------------------------------

local function dissect_monitor_request(buffer, pinfo, tree)
    local subtree = tree:add(robocup_gc, buffer(), "RoboCup GameController Monitor Request")
    subtree:add(f.type, "Monitor Request"):set_generated()
    pinfo.cols.info:set("Monitor Request")
    add_header(subtree, buffer, 0, MONITOR_REQUEST_SIZE, MONITOR_REQUEST_VERSION)
    return buffer:len()
end

-----------------------------------------------------------------------
-- Main dissector
-----------------------------------------------------------------------

function robocup_gc.dissector(buffer, pinfo, tree)
    local length = buffer:len()
    if length < 4 then
        return 0
    end

    local header = buffer(0, 4):string()
    local is_forwarded_status = length >= 8 and buffer(4, 4):string() == STATUS_MESSAGE_HEADER
    if header ~= CONTROL_MESSAGE_HEADER and header ~= CONTROL_MESSAGE_TRUE_DATA_HEADER
        and header ~= STATUS_MESSAGE_HEADER and header ~= MONITOR_REQUEST_HEADER
        and not is_forwarded_status then
        -- Not ours, let other dissectors try.
        return 0
    end

    pinfo.cols.protocol:set("RoboCup GC")
    if header == CONTROL_MESSAGE_HEADER then
        return dissect_control_message(buffer, pinfo, tree, false)
    elseif header == CONTROL_MESSAGE_TRUE_DATA_HEADER then
        return dissect_control_message(buffer, pinfo, tree, true)
    elseif header == STATUS_MESSAGE_HEADER then
        return dissect_status(buffer, pinfo, tree)
    elseif header == MONITOR_REQUEST_HEADER then
        return dissect_monitor_request(buffer, pinfo, tree)
    else
        return dissect_forwarded_status(buffer, pinfo, tree)
    end
end

-----------------------------------------------------------------------
-- Registration
-----------------------------------------------------------------------

local udp_port = DissectorTable.get("udp.port")
for _, port in ipairs({
    CONTROL_MESSAGE_PORT,
    STATUS_MESSAGE_PORT,
    STATUS_MESSAGE_FORWARD_PORT,
    MONITOR_REQUEST_PORT,
}) do
    udp_port:add(port, robocup_gc)
end
udp_port:add_for_decode_as(robocup_gc)
