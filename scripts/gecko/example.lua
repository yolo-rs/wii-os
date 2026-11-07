local function on_os_get_system_time(emu)
    local tbu = emu:gpr(3)
    local tbl = emu:gpr(4)
    log(string.format("[OSGetSystemTime] tbu=%08X tbl=%08X", tbu, tbl))
end

traps = {
    cpu_pre = {
        [0x8133949C] = on_os_get_system_time,
    },
}