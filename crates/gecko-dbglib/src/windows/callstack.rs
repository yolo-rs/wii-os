use egui::{Color32, Context, RichText, ScrollArea};
use gecko::SystemId;
use gecko::gekko::Gekko;
use gecko::mmio::Mmio;
use image::symbols::SymbolTable;

const MAX_FRAMES: usize = 64;
const RAM_LO: u32 = 0x8000_0000;
const RAM_HI: u32 = 0x817F_FFFF;

fn is_valid_ram(addr: u32) -> bool {
    (RAM_LO..=RAM_HI).contains(&addr)
}

pub struct CallFrame {
    pub addr: u32,
    pub symbol: Option<String>,
    pub offset: u32,
}

/// Walk the PowerPC stack using the back-chain / saved-LR convention.
///
/// PowerPC EABI stack frame layout (32-bit):
///   [SP + 0] = back chain (pointer to caller's frame)
///   [SP + 4] = saved LR (written by the callee that owns this frame)
///
/// We emit the current PC, then LR (handles leaf functions that never save
/// LR to the stack), then walk back-chain pointers reading saved LRs.
pub fn walk_callstack<const SYSTEM: SystemId>(
    cpu: &Gekko,
    mmio: &Mmio<SYSTEM>,
    symbols: Option<&SymbolTable>,
) -> Vec<CallFrame> {
    let mut frames = Vec::new();

    let resolve = |addr: u32| -> (Option<String>, u32) {
        if let Some(syms) = symbols
            && let Some(sym) = syms.lookup(addr)
        {
            return (Some(sym.name.clone()), addr - sym.addr);
        }
        (None, 0)
    };

    let push = |frames: &mut Vec<CallFrame>, addr: u32| {
        let (sym, off) = resolve(addr);
        frames.push(CallFrame {
            addr,
            symbol: sym,
            offset: off,
        });
    };

    // Frame 0: current PC
    push(&mut frames, cpu.pc);

    // Frame 1: LR, immediate return address
    // For leaf functions LR is the only way to find the caller since they
    // don't create a stack frame or save LR to memory.
    let lr = cpu.spr.lr;
    if lr != 0 && is_valid_ram(lr) {
        push(&mut frames, lr);
    }

    // Walk the back-chain, reading saved LR from each frame
    let mut sp = cpu.gprs[1]; // r1 = stack pointer

    for _ in 0..MAX_FRAMES {
        if !is_valid_ram(sp) {
            break;
        }

        let back_chain = mmio.virt_read_u32(sp);

        if back_chain == 0 || !is_valid_ram(back_chain) {
            break;
        }

        // Stack grows downward, so back chain should be at a higher address.
        if back_chain <= sp {
            break;
        }

        // The LR save slot at [SP + 4] was written by the function that
        // created this frame (i.e. the callee that set up SP).
        let saved_lr = mmio.virt_read_u32(sp + 4);

        if saved_lr != 0 && is_valid_ram(saved_lr) {
            // Avoid duplicating the LR we already emitted above.
            if frames.len() < 2 || saved_lr != frames[1].addr {
                push(&mut frames, saved_lr);
            }
        }

        sp = back_chain;
    }

    frames
}

pub fn show_callstack<const SYSTEM: SystemId>(
    ctx: &Context,
    open: &mut bool,
    cpu: &Gekko,
    mmio: &Mmio<SYSTEM>,
    symbols: Option<&SymbolTable>,
) {
    let frames = walk_callstack(cpu, mmio, symbols);

    egui::Window::new("Call Stack").open(open).show(ctx, |ui| {
        ui.label(
            RichText::new(format!("{} frames", frames.len()))
                .small()
                .color(Color32::from_rgb(140, 140, 140)),
        );

        ui.separator();

        ScrollArea::vertical().id_salt("callstack_scroll").show(ui, |ui| {
            egui::Grid::new("callstack_grid")
                .num_columns(3)
                .striped(true)
                .min_col_width(0.0)
                .show(ui, |ui| {
                    for (i, frame) in frames.iter().enumerate() {
                        // Frame index
                        ui.label(
                            RichText::new(format!("#{i}"))
                                .monospace()
                                .color(Color32::from_rgb(100, 100, 100)),
                        );

                        // Address
                        ui.label(
                            RichText::new(format!("{:#010X}", frame.addr))
                                .monospace()
                                .color(Color32::from_rgb(150, 220, 150)),
                        );

                        // Symbol name + offset
                        if let Some(name) = &frame.symbol {
                            let label = if frame.offset > 0 {
                                format!("{}+{:#X}", name, frame.offset)
                            } else {
                                name.clone()
                            };
                            ui.label(RichText::new(label).monospace().color(Color32::from_rgb(100, 180, 255)));
                        } else {
                            ui.label(RichText::new("???").monospace().color(Color32::from_rgb(100, 100, 100)));
                        }

                        ui.end_row();
                    }
                });
        });
    });
}
