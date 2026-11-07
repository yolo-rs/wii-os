local stdout_buffer = ""

local function on_stdout_write(emu, virt_addr, phys_addr, size, value)
    local ch = value & 0xFF
    local done = (value & 0x100) ~= 0
    if ch == 0x0A then
        log("hazel says: " .. stdout_buffer)
        stdout_buffer = ""
    elseif ch ~= 0 then
        stdout_buffer = stdout_buffer .. string.char(ch)
    end
    if done then
        stdout_buffer = ""
    end
end

traps = {
    bus_write_post = {
        virt = {
            [0xCC007000] = on_stdout_write,
        },
    },
}
