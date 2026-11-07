fn main() {
    chipi_build::generate_bindings("spec/gekko.bindings.chipi").expect("chipi codegen failed (gekko)");
    chipi_build::generate_bindings("spec/dsp.bindings.chipi").expect("chipi codegen failed (dsp)");
    chipi_build::generate_bindings("spec/wii_gekko.bindings.chipi").expect("chipi codegen failed (wii gekko)");
    chipi_build::generate_bindings("spec/wii_dsp.bindings.chipi").expect("chipi codegen failed (wii dsp)");
    chipi_build::generate_bindings("spec/gekko_jit.bindings.chipi").expect("chipi codegen failed (gekko jit)");
    chipi_build::generate_bindings("spec/wii_gekko_jit.bindings.chipi").expect("chipi codegen failed (wii gekko jit)");
    chipi_build::generate_bindings("spec/dsp_jit.bindings.chipi").expect("chipi codegen failed (dsp jit)");
}
