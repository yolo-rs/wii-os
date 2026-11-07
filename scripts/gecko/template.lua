local function on_cpu_pre(emu)
    -- inspect CPU state
end

local function on_cpu_post(emu)
    -- inspect results after instruction
end

local function on_bus_read_pre(emu, virt_addr, phys_addr, size)
    -- inspect read request
    -- return nil to keep the normal read
    -- return 0x12345678 to override the read value
    return nil
end

local function on_bus_read_post(emu, virt_addr, phys_addr, size, value)
    -- inspect completed read
end

local function on_bus_write_pre(emu, virt_addr, phys_addr, size, value)
    -- inspect write request
    -- return value to keep it unchanged
    -- return another integer to override it
    return value
end

local function on_bus_write_post(emu, virt_addr, phys_addr, size, value)
    -- inspect completed write
end

traps = {
    cpu_pre = {
        [0x00000000] = on_cpu_pre,
    },
    cpu_post = {
        [0x00000000] = on_cpu_post,
    },
    bus_read_pre = {
        virt = {
            [0x00000000] = on_bus_read_pre,
        },
        phys = {
            [0x00000000] = on_bus_read_pre,
        },
    },
    bus_read_post = {
        virt = {
            [0x00000000] = on_bus_read_post,
        },
        phys = {
            [0x00000000] = on_bus_read_post,
        },
    },
    bus_write_pre = {
        virt = {
            [0x00000000] = on_bus_write_pre,
        },
        phys = {
            [0x00000000] = on_bus_write_pre,
        },
    },
    bus_write_post = {
        virt = {
            [0x00000000] = on_bus_write_post,
        },
        phys = {
            [0x00000000] = on_bus_write_post,
        },
    },
}
