toggle = false
local function hack(emu, virt_addr, phys_addr, size, value)
    log(string.format("Toggling DSP busy bit: %s", toggle and "ON" or "OFF"))
    log(string.format("Original DSP read value: %08X", value))
    if toggle then
        value = value | 0x8000
        toggle = false
    else
        value = value & 0x7FFF
        toggle = true
    end
    log(string.format("Modified DSP read value: %08X", value))
    return value
end

traps = {
    bus_read_post = {
        virt = {
            [0xCC005000] = hack,
            [0xCC005004] = hack,
        },
    },
}