import struct
import os

import idaapi
import ida_bytes
import ida_entry
import ida_ida
import ida_name
import ida_segment


APPLOADER_FILE_CODE_OFFSET = 0x20
APPLOADER_RUNTIME_BASE = 0x81200000
APPLOADER_FILE_LOAD_BASE = APPLOADER_RUNTIME_BASE - APPLOADER_FILE_CODE_OFFSET
APPLOADER_HEADER_SIZE = 0x20


SYSTEM_REGIONS = [
    (0x80000000, 0x80004000, ".os_lowmem",     "DATA"),
    (0xC0000000, 0xC0004000, ".os_lowmem_unc", "DATA"),
    (0xCC000000, 0xCC008004, ".mmio_gc",       "DATA"),
    (0xCD000000, 0xCD008000, ".mmio_wii",      "DATA"),
]

OS_GLOBALS = {
    0x80000000: ("OSGameCode", 4),
    0x80000004: ("OSMakerCode", 2),
    0x80000006: ("OSDiscNumber", 1),
    0x80000007: ("OSDiscVersion", 1),
    0x80000008: ("OSAudioStreaming", 1),
    0x80000009: ("OSStreamingBufferSize", 1),
    0x80000018: ("OSWiiMagic", 4),
    0x8000001C: ("OSGameCubeMagic", 4),
    0x80000020: ("OSNintendoBootCode", 4),
    0x80000024: ("OSVersion", 4),
    0x80000028: ("OSPhysicalMEM1Size", 4),
    0x8000002C: ("OSConsoleType", 4),
    0x80000030: ("OSArenaLo", 4),
    0x80000034: ("OSArenaHi", 4),
    0x80000038: ("OSFstLocation", 4),
    0x8000003C: ("OSFstMaxLength", 4),
    0x80000040: ("OSDebuggerHook", 4),
    0x80000044: ("OSDebuggerHookSize", 4),
    0x80000048: ("OSCurrentContextPhys", 4),
    0x8000004C: ("OSExceptionHandlerVector", 4),
    0x800000C0: ("OSCurrentContext", 4),
    0x800000C4: ("OSUserInterruptMask", 4),
    0x800000C8: ("OSExceptionType", 4),
    0x800000CC: ("OSVideoMode", 4),
    0x800000D0: ("OSARAMSize", 4),
    0x800000D4: ("OSCurrentFunction", 4),
    0x800000D8: ("OSDefaultThread", 4),
    0x800000DC: ("OSEarliestThread", 4),
    0x800000E0: ("OSLastThread", 4),
    0x800000E4: ("OSCurrentThread", 4),
    0x800000F0: ("OSPhysicalMEM2Size", 4),
    0x800000F4: ("OSConsoleSimulatedMEM2Size", 4),
    0x800000F8: ("OSBusClockSpeed", 4),
    0x800000FC: ("OSCpuClockSpeed", 4),
    0x80003100: ("BI2_PhysicalMEM1Size", 4),
    0x80003104: ("BI2_SimulatedMEM1Size", 4),
    0x8000310C: ("BI2_MEM1ArenaStart", 4),
    0x80003110: ("BI2_MEM1ArenaEnd", 4),
    0x80003118: ("BI2_PhysicalMEM2Size", 4),
    0x8000311C: ("BI2_SimulatedMEM2Size", 4),
    0x80003120: ("BI2_MEM2EndForPPC", 4),
    0x80003124: ("BI2_UsableMEM2Start", 4),
    0x80003128: ("BI2_UsableMEM2End", 4),
    0x80003130: ("BI2_IPCBufferStart", 4),
    0x80003134: ("BI2_IPCBufferEnd", 4),
    0x80003138: ("BI2_HollywoodVersion", 4),
    0x80003140: ("BI2_IOSVersion", 4),
    0x80003144: ("BI2_IOSBuildDate", 4),
    0x80003148: ("BI2_IOSReservedHeapStart", 4),
    0x8000314C: ("BI2_IOSReservedHeapEnd", 4),
    0x80003158: ("BI2_GDDRVendorCode", 4),
    0x8000315C: ("BI2_BootIndicator", 1),
    0x8000315D: ("BI2_LegacyDIModeFlag", 1),
    0x8000315E: ("BI2_DevkitBootProgramVersion", 2),
    0x80003180: ("BI2_GameID", 4),
    0x80003184: ("BI2_ApplicationType", 1),
    0x80003186: ("BI2_ApplicationType2", 1),
    0x80003188: ("BI2_MinimumIOSVersion", 4),
    0x80003198: ("BI2_DataPartitionOffset", 4),
    0x8000319C: ("BI2_DiscLayerType", 1),
}


def _file_size(li):
    here = li.tell()
    li.seek(0, 2)
    size = li.tell()
    li.seek(here)
    return size


def _read_at(li, off, size):
    here = li.tell()
    li.seek(off)
    data = li.read(size)
    li.seek(here)
    return data


def _u32be(raw, off):
    return struct.unpack(">I", raw[off:off + 4])[0]


def _add_segment(start, end, name, sclass, perm):
    seg = idaapi.segment_t()
    seg.start_ea = start
    seg.end_ea = end
    seg.bitness = 1
    seg.align = idaapi.saRelByte
    seg.comb = idaapi.scPub
    seg.perm = perm
    return ida_segment.add_segm_ex(seg, name, sclass, idaapi.ADDSEG_NOSREG)


def _name_dword(ea, name):
    ida_bytes.create_dword(ea, 4)
    ida_name.set_name(ea, name, ida_name.SN_FORCE)


def _define_function(ea, name):
    ida_name.set_name(ea, name, ida_name.SN_FORCE)
    ida_entry.add_entry(ea, ea, name, True)


def _add_system_regions():
    rw = ida_segment.SEGPERM_READ | ida_segment.SEGPERM_WRITE
    for start, end, name, sclass in SYSTEM_REGIONS:
        if _add_segment(start, end, name, sclass, rw):
            ida_bytes.put_bytes(start, b"\x00" * (end - start))


def _apply_os_globals():
    for ea, (name, size) in OS_GLOBALS.items():
        if not ida_segment.getseg(ea):
            continue
        if size == 1:
            ida_bytes.create_byte(ea, 1)
        elif size == 2:
            ida_bytes.create_word(ea, 2)
        elif size == 4:
            ida_bytes.create_dword(ea, 4)
        ida_name.set_name(ea, name, ida_name.SN_FORCE)


def accept_file(li, filename):
    if os.path.basename(filename).lower() != "apploader.bin":
        return 0
    return {"format": "Nintendo GameCube/Wii apploader"}


def load_file(li, neflags, format):
    size = _file_size(li)
    raw = _read_at(li, 0, size)

    idaapi.set_processor_type("PPC", idaapi.SETPROC_LOADER)
    ida_ida.inf_set_be(True)
    ida_ida.inf_set_app_bitness(32)

    image_start = APPLOADER_FILE_LOAD_BASE
    image_end = APPLOADER_FILE_LOAD_BASE + size

    _add_segment(image_start, image_start + APPLOADER_HEADER_SIZE, ".apploader_header", "DATA", ida_segment.SEGPERM_READ)
    ida_bytes.put_bytes(image_start, raw[:APPLOADER_HEADER_SIZE])

    _add_segment(
        APPLOADER_RUNTIME_BASE,
        image_end,
        ".apploader",
        "CODE",
        ida_segment.SEGPERM_READ | ida_segment.SEGPERM_WRITE | ida_segment.SEGPERM_EXEC,
    )
    ida_bytes.put_bytes(APPLOADER_RUNTIME_BASE, raw[APPLOADER_FILE_CODE_OFFSET:])

    ida_name.set_name(image_start, "apploader_build_date", ida_name.SN_FORCE)
    ida_bytes.create_strlit(image_start, 10, idaapi.STRTYPE_C)
    _name_dword(image_start + 0x10, "apploader_entry")
    _name_dword(image_start + 0x14, "apploader_size")
    _name_dword(image_start + 0x18, "apploader_trailer_size")

    _add_system_regions()
    _apply_os_globals()

    entry_addr = _u32be(raw, 0x10)
    if APPLOADER_RUNTIME_BASE <= entry_addr < image_end:
        _define_function(entry_addr, "Apploader_Entry")
    else:
        _define_function(APPLOADER_RUNTIME_BASE, "apploader_start")

    return 1
