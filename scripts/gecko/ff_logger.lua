-- Hooks OSReport / OSReportLn / OSPanic in Final Fantasy Crystal Chronicles: The Crystal Bearers
-- It also includes the Apploader logs.
--
-- Addresses are from the FF main.dol:
--   OSReport   @ 0x803756F0  -  OSReport(fmt, ...)
--   OSReportLn @ 0x80378D40  -  OSReportLn(fmt, ...)
--   OSPanic    @ 0x80378DC0  -  OSPanic(file, line, fmt, ...) [no return]

include("apploader_osreport.lua")

local OS_REPORT    = 0x803756F0
local OS_REPORT_LN = 0x80378D40
local OS_PANIC     = 0x80378DC0

local function on_panic(emu)
    local file = read_cstring(emu, emu:gpr(3))
    local line = emu:gpr(4)
    local fmt  = read_cstring(emu, emu:gpr(5))
    emit("[OSPanic] ", printf(emu, fmt, printf_args(emu, 6)))
    log(string.format("[OSPanic] at %s:%d", file, line))
end

traps.cpu_pre[OS_REPORT]    = dispatch_printf("[OSReport] ")
traps.cpu_pre[OS_REPORT_LN] = dispatch_printf("[OSReportLn] ")
traps.cpu_pre[OS_PANIC]     = on_panic
