use gecko::gamecube::GameCube;

fn main() {
    let entry: u32 = 0x8000_0000;
    let mut emu = GameCube::new(entry);

    emu.mmio.virt_write_u32(entry, 0x4800_0000);

    loop {
        emu.run_until_vsync();
    }
}
