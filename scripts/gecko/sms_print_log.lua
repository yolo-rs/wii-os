local function read_cstring(emu, addr)
    local chars = {}
    for i = 0, 255 do
        local b = emu:read_u8(addr + i)
        if b == 0 then break end
        chars[#chars + 1] = string.char(b)
    end
    return table.concat(chars)
end

local function on_vsnprintf(emu)
    local fmt = emu:gpr(5)
    local str = read_cstring(emu, fmt)
    log(string.format("[vsnprintf] fmt=0x%08X lr=0x%08X: %s", fmt, emu:lr(), str))
end

local GAME_STATES = {
    [1] = "RUNNING", [2] = "LOAD_LEVEL", [3] = "PLAY",
    [4] = "RESET?", [5] = "MOVIE?", [6] = "MENU?",
    [7] = "EXIT", [8] = "UNK8", [9] = "UNK9",
}

-- GameStateLoop top of while loop
local function on_game_state_check(emu)
    local state = emu:read_u8(0x803E9700 + 8)
    local name = GAME_STATES[state] or "UNKNOWN"
    log(string.format("[GameStateLoop] state=%d (%s)", state, name))
end

-- ShutdownSystem entry
local function on_shutdown(emu)
    log(string.format("[ShutdownSystem] entered! lr=0x%08X", emu:lr()))
end

-- OSExitThread entry: dump stack to find which thread
local function on_exit_thread(emu)
    local sp = emu:gpr(1)
    -- walk stack frames to get a backtrace
    local bt = {}
    for i = 0, 7 do
        local saved_lr = emu:read_u32(sp + 4)
        bt[#bt + 1] = string.format("0x%08X", saved_lr)
        local prev_sp = emu:read_u32(sp)
        if prev_sp == 0 or prev_sp == sp then break end
        sp = prev_sp
    end
    log(string.format("[OSExitThread] r3=%d lr=0x%08X backtrace: %s",
        emu:gpr(3), emu:lr(), table.concat(bt, " <- ")))
end

-- DVDGetResetStatus return
local function on_reset_status(emu)
    local v = emu:gpr(3)
    if v > 0x7FFFFFFF then v = v - 0x100000000 end
    log(string.format("[DVDGetResetStatus] -> %d lr=0x%08X", v, emu:lr()))
end

-- DVDWorkerThread_Main sets initialized flag
local function on_dvd_thread_init_flag(emu)
    log(string.format("[DVDWorkerThread] setting init flag r13-0x5C6C = 1"))
end

-- DVDWorkerThread_Main entry
local function on_dvd_thread_entry(emu)
    log(string.format("[DVDWorkerThread] entered!"))
end

-- DVDWorkerThread_Main early return (flag==0 path)
local function on_dvd_thread_early_return(emu)
    local flag = emu:read_u32(emu:gpr(13) - 0x5C6C)
    log(string.format("[DVDWorkerThread] early return path! flag=0x%08X", flag))
end

-- DVDWorkerThread_Main message loop entry
local function on_dvd_thread_msg_loop(emu)
    log("[DVDWorkerThread] entering message loop")
end

-- OSCreateThread(thread, entry, arg, stack_top, stack_size, priority, detached)
local function on_create_thread(emu)
    local entry = emu:gpr(4)
    local prio = emu:gpr(9)
    log(string.format("[OSCreateThread] entry=0x%08X prio=%d lr=0x%08X", entry, prio, emu:lr()))
end

-- Hook each thread entry to see which ones run and which one exits
local function make_thread_entry_hook(name)
    return function(emu)
        log(string.format("[Thread:%s] entered (0x%08X)", name, emu:pc()))
    end
end

traps = {
    cpu_pre = {
        [0x003397A4] = on_vsnprintf,
        [0x803397A4] = on_vsnprintf,
        [0x002A63B0] = on_game_state_check,
        [0x802A63B0] = on_game_state_check,
        [0x002A69CC] = on_shutdown,
        [0x802A69CC] = on_shutdown,
        [0x00348A68] = on_exit_thread,
        [0x80348A68] = on_exit_thread,
        [0x0034E1D8] = on_reset_status,
        [0x8034E1D8] = on_reset_status,
        [0x00311170] = on_dvd_thread_entry,
        [0x80311170] = on_dvd_thread_entry,
        [0x003111D0] = on_dvd_thread_init_flag,
        [0x803111D0] = on_dvd_thread_init_flag,
        [0x00311260] = on_dvd_thread_early_return,
        [0x80311260] = on_dvd_thread_early_return,
        [0x00311210] = on_dvd_thread_msg_loop,
        [0x80311210] = on_dvd_thread_msg_loop,
        [0x00348948] = on_create_thread,
        [0x80348948] = on_create_thread,
        [0x002C54B8] = make_thread_entry_hook("802C54B8"),
        [0x802C54B8] = make_thread_entry_hook("802C54B8"),
        [0x002A9184] = make_thread_entry_hook("802A9184"),
        [0x802A9184] = make_thread_entry_hook("802A9184"),
        [0x002A7878] = make_thread_entry_hook("802A7878"),
        [0x802A7878] = make_thread_entry_hook("802A7878"),
        [0x0035DB08] = function(emu)
            log(string.format("[GXWaitDrawDone] stb 0x61 to WGP, r0=0x%02X", emu:gpr(0)))
        end,
        [0x8035DB08] = function(emu)
            log(string.format("[GXWaitDrawDone] stb 0x61 to WGP, r0=0x%02X", emu:gpr(0)))
        end,
        [0x0035DB10] = function(emu)
            log(string.format("[GXWaitDrawDone] stw 0x%08X to WGP", emu:gpr(0)))
        end,
        [0x8035DB10] = function(emu)
            log(string.format("[GXWaitDrawDone] stw 0x%08X to WGP", emu:gpr(0)))
        end,
        [0x003492E0] = function(emu)
            log(string.format("[OSSleepThread] entered lr=0x%08X", emu:lr()))
        end,
        [0x803492E0] = function(emu)
            log(string.format("[OSSleepThread] entered lr=0x%08X", emu:lr()))
        end,
    },
}
