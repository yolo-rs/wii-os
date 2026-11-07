local OSREPORT_STUB = 0x81300000

function read_cstring(emu, addr)
    local chars = {}
    for i = 0, 1023 do
        local b = emu:read_u8(addr + i)
        if b == 0 then break end
        chars[#chars + 1] = string.char(b)
    end
    return table.concat(chars)
end

function printf_args(emu, first_gpr)
    local gprs = {}
    for r = first_gpr, 10 do
        gprs[#gprs + 1] = emu:gpr(r)
    end

    local fprs = {}
    for r = 1, 8 do
        fprs[#fprs + 1] = emu:fpr(r)
    end

    return { gpr = gprs, fpr = fprs, first_gpr = first_gpr }
end

function printf(emu, fmt, args)
    local out = {}
    local i = 1
    local gpr_args = args.gpr or args
    local fpr_args = args.fpr or {}
    local gpr_slot = (args.first_gpr or 4) - 3
    local gpr_idx = 1
    local fpr_idx = 1

    local function next_arg()
        local v = gpr_args[gpr_idx] or 0
        gpr_idx = gpr_idx + 1
        gpr_slot = gpr_slot + 1
        return v
    end

    local function next_float_arg()
        local v = fpr_args[fpr_idx]
        fpr_idx = fpr_idx + 1
        if gpr_slot % 2 ~= 0 then
            gpr_idx = gpr_idx + 1
            gpr_slot = gpr_slot + 1
        end
        gpr_idx = gpr_idx + 2
        gpr_slot = gpr_slot + 2
        if v == nil then
            v = 0
        end
        return v
    end

    while i <= #fmt do
        local c = fmt:sub(i, i)
        if c ~= "%" then
            out[#out + 1] = c
            i = i + 1
        else
            i = i + 1
            local spec = "%"
            while fmt:sub(i, i):match("[%-+ #0]") do
                spec = spec .. fmt:sub(i, i)
                i = i + 1
            end

            if fmt:sub(i, i) == "*" then
                local width = next_arg()
                if width >= 0x80000000 then width = width - 0x100000000 end
                if width < 0 then
                    spec = spec .. "-"
                    width = -width
                end
                spec = spec .. tostring(width)
                i = i + 1
            else
                while fmt:sub(i, i):match("%d") do
                    spec = spec .. fmt:sub(i, i)
                    i = i + 1
                end
            end

            if fmt:sub(i, i) == "." then
                i = i + 1
                if fmt:sub(i, i) == "*" then
                    local precision = next_arg()
                    if precision >= 0x80000000 then precision = precision - 0x100000000 end
                    if precision >= 0 then
                        spec = spec .. "." .. tostring(precision)
                    end
                    i = i + 1
                else
                    spec = spec .. "."
                    while fmt:sub(i, i):match("%d") do
                        spec = spec .. fmt:sub(i, i)
                        i = i + 1
                    end
                end
            end
            local length = ""
            if fmt:sub(i, i + 1) == "ll" or fmt:sub(i, i + 1) == "hh" then
                length = fmt:sub(i, i + 1)
                i = i + 2
            elseif fmt:sub(i, i):match("[hljztL]") then
                length = fmt:sub(i, i)
                i = i + 1
            end
            local kind = fmt:sub(i, i)
            i = i + 1
            if kind == "%" then
                out[#out + 1] = "%"
            elseif kind == "d" or kind == "i" then
                local v = next_arg()
                if v >= 0x80000000 then v = v - 0x100000000 end
                out[#out + 1] = string.format(spec .. "d", v)
            elseif kind == "u" then
                out[#out + 1] = string.format(spec .. "u", next_arg())
            elseif kind == "x" then
                out[#out + 1] = string.format(spec .. "x", next_arg())
            elseif kind == "X" then
                out[#out + 1] = string.format(spec .. "X", next_arg())
            elseif kind == "s" then
                out[#out + 1] = read_cstring(emu, next_arg())
            elseif kind == "c" then
                out[#out + 1] = string.char(next_arg() % 256)
            elseif kind:match("[fFeEgG]") then
                out[#out + 1] = string.format(spec .. kind, next_float_arg())
            else
                out[#out + 1] = spec .. length .. kind
            end
        end
    end
    return table.concat(out)
end

function emit(prefix, msg)
    msg = msg:gsub("[\r\n]+$", "")
    for line in (msg .. "\n"):gmatch("([^\n]*)\n") do
        if #line > 0 then
            log(prefix .. line)
        end
    end
end

function dispatch_printf(prefix)
    return function(emu)
        local fmt = read_cstring(emu, emu:gpr(3))
        emit(prefix, printf(emu, fmt, printf_args(emu, 4)))
    end
end

traps = traps or {}
traps.cpu_pre = traps.cpu_pre or {}
traps.cpu_pre[OSREPORT_STUB] = dispatch_printf("[apploader] ")
