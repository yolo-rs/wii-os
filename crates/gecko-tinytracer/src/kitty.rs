/// Render pixels (packed 0x00RRGGBB u32s) to the terminal via the Kitty
/// graphics protocol.
///
/// Protocol: APC escape  \x1b_G<key=value,...>;<base64-payload>\x1b\\
///   a=T     – transmit + display immediately
///   f=32    – 32-bit RGBA pixels
///   s=W,v=H – dimensions
///   m=1     – more chunks follow; m=0 – last (or only) chunk
#[rustfmt::skip]
pub fn render_xfb(pixels: &[u32], width: usize, height: usize) {
    use std::io::Write as _;
    use base64::Engine as _;

    // Convert packed RGB to RGBA (alpha = 0xFF).
    let mut rgba: Vec<u8> = Vec::with_capacity(width * height * 4);
    for &px in pixels {
        rgba.push(((px >> 16) & 0xFF) as u8); // R
        rgba.push(((px >>  8) & 0xFF) as u8); // G
        rgba.push(( px        & 0xFF) as u8); // B
        rgba.push(0xFF);                      // A
    }

    let encoded = base64::engine::general_purpose::STANDARD.encode(&rgba);

    // Kitty protocol requires chunks of at most 4096 base64 characters
    const CHUNK: usize = 4096;
    let chunks: Vec<&str> = encoded
        .as_bytes()
        .chunks(CHUNK)
        .map(|c| std::str::from_utf8(c).unwrap())
        .collect();

    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    for (idx, chunk) in chunks.iter().enumerate() {
        let more = if idx + 1 < chunks.len() { 1 } else { 0 };
        if idx == 0 {
            write!(
                out,
                "\x1b_Ga=T,f=32,s={},v={},m={};{}\x1b\\",
                width, height, more, chunk
            )
            .unwrap();
        } else {
            write!(out, "\x1b_Gm={};{}\x1b\\", more, chunk).unwrap();
        }
    }

    writeln!(out).unwrap();
}
