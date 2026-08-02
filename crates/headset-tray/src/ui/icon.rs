//! The headset icon, as pixels. Pure: no OS, no window, no device.
//!
//! The artwork is `assets/headset.svg`, which is the committed source of truth.
//! It is rasterised here rather than shipped as a bitmap so one file serves the
//! tray's runtime `HICON`, the multi-size `.ico` embedded in the executable, and
//! the tests that keep those from drifting apart.
//!
//! The rasteriser is written by hand — a path parser, cubic flattening, and a
//! scanline fill — because the alternative is a dependency, and the file uses
//! four path commands and one fill rule.

/// Straight-alpha BGRA.
pub const FILL: u32 = 0xFF_F0F0F0;
pub const OUTLINE: u32 = 0xFF_1A1A1A;
pub const CLEAR: u32 = 0x00_000000;

/// The artwork. Public domain (see `THIRD_PARTY_NOTICES.md`).
const SVG: &str = include_str!("../../assets/headset.svg");

/// Samples per axis. Coverage is averaged over `SS * SS` samples per pixel,
/// which is what makes the diagonals smooth instead of stair-stepped.
const SS: usize = 4;

/// Extracts the `d` attribute of the first `<path>`.
///
/// Deliberately anchored to `<path`: matching `d="` alone also matches the tail
/// of `enable-background="`, which is a real mistake this file has already made
/// once.
fn path_data(svg: &str) -> &str {
    let tag = &svg[svg.find("<path").expect("the artwork has a path")..];
    let attr = &tag[tag.find(" d=\"").expect("the path has a d attribute") + 4..];
    &attr[..attr.find('"').expect("the d attribute is terminated")]
}

/// The `viewBox` side. Square, and asserted so a replacement artwork with a
/// different box fails loudly rather than rendering off-centre.
fn view_box(svg: &str) -> f32 {
    let a = &svg[svg.find("viewBox=\"").expect("the artwork has a viewBox") + 9..];
    let v = &a[..a.find('"').expect("the viewBox is terminated")];
    let nums: Vec<f32> = v
        .split_whitespace()
        .filter_map(|n| n.parse().ok())
        .collect();
    assert_eq!(nums.len(), 4, "viewBox should have four numbers: {v}");
    assert_eq!(
        nums[2], nums[3],
        "the artwork is expected to be square: {v}"
    );
    nums[2]
}

/// Flattens the path into closed polygons, in viewBox coordinates.
///
/// Handles `M`, `L`, `C` and `Z`, absolute only, which is what the artwork uses.
/// An unexpected command panics rather than being skipped: silently dropping a
/// curve would produce a subtly wrong icon instead of an obvious failure.
fn polygons(d: &str) -> Vec<Vec<(f32, f32)>> {
    let mut nums = Vec::new();
    let mut cmds = Vec::new();
    let mut cur = String::new();
    for ch in d.chars() {
        if ch.is_ascii_alphabetic() {
            if !cur.is_empty() {
                nums.push(cur.parse::<f32>().expect("a number"));
                cur.clear();
            }
            cmds.push((ch, nums.len()));
        } else if ch == ',' || ch.is_whitespace() {
            if !cur.is_empty() {
                nums.push(cur.parse::<f32>().expect("a number"));
                cur.clear();
            }
        } else if ch == '-' && !cur.is_empty() && !cur.ends_with(['e', 'E']) {
            nums.push(cur.parse::<f32>().expect("a number"));
            cur.clear();
            cur.push(ch);
        } else {
            cur.push(ch);
        }
    }
    if !cur.is_empty() {
        nums.push(cur.parse::<f32>().expect("a number"));
    }

    let mut out: Vec<Vec<(f32, f32)>> = Vec::new();
    let mut poly: Vec<(f32, f32)> = Vec::new();
    let mut at = (0.0f32, 0.0f32);

    for (i, (cmd, start)) in cmds.iter().enumerate() {
        let end = cmds.get(i + 1).map(|(_, s)| *s).unwrap_or(nums.len());
        let args = &nums[*start..end];
        match cmd {
            'M' => {
                if poly.len() > 2 {
                    out.push(std::mem::take(&mut poly));
                }
                poly.clear();
                at = (args[0], args[1]);
                poly.push(at);
                // Any further pairs after a moveto are implicit linetos.
                for p in args[2..].chunks_exact(2) {
                    at = (p[0], p[1]);
                    poly.push(at);
                }
            }
            'L' => {
                for p in args.chunks_exact(2) {
                    at = (p[0], p[1]);
                    poly.push(at);
                }
            }
            'C' => {
                for c in args.chunks_exact(6) {
                    let (p0, p1, p2, p3) = (at, (c[0], c[1]), (c[2], c[3]), (c[4], c[5]));
                    // Fixed subdivision. The artwork's curves are short relative
                    // to the canvas, and 16 segments is well under a pixel at
                    // the largest size rendered.
                    const STEPS: usize = 16;
                    for s in 1..=STEPS {
                        let t = s as f32 / STEPS as f32;
                        let u = 1.0 - t;
                        let x = u * u * u * p0.0
                            + 3.0 * u * u * t * p1.0
                            + 3.0 * u * t * t * p2.0
                            + t * t * t * p3.0;
                        let y = u * u * u * p0.1
                            + 3.0 * u * u * t * p1.1
                            + 3.0 * u * t * t * p2.1
                            + t * t * t * p3.1;
                        poly.push((x, y));
                    }
                    at = p3;
                }
            }
            'Z' | 'z' => {
                if poly.len() > 2 {
                    out.push(std::mem::take(&mut poly));
                }
                poly.clear();
            }
            other => panic!("the artwork uses path command {other}, which is not supported"),
        }
    }
    if poly.len() > 2 {
        out.push(poly);
    }
    out
}

/// Scales and centres the drawing so it fills `box_side`, less a margin.
///
/// The margin is not decoration: the halo is dilated outward by a pixel, so a
/// glyph flush with the edge would have its halo clipped.
fn fit_to_canvas(polys: &mut [Vec<(f32, f32)>], box_side: f32) {
    const MARGIN: f32 = 0.04;

    let pts = || polys.iter().flat_map(|p| p.iter());
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for (x, y) in pts() {
        x0 = x0.min(*x);
        y0 = y0.min(*y);
        x1 = x1.max(*x);
        y1 = y1.max(*y);
    }
    let (w, h) = (x1 - x0, y1 - y0);
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    // One scale for both axes: the drawing must not be stretched.
    let usable = box_side * (1.0 - 2.0 * MARGIN);
    let s = (usable / w).min(usable / h);
    let (ox, oy) = (
        (box_side - w * s) / 2.0 - x0 * s,
        (box_side - h * s) / 2.0 - y0 * s,
    );
    for poly in polys.iter_mut() {
        for p in poly.iter_mut() {
            *p = (p.0 * s + ox, p.1 * s + oy);
        }
    }
}

/// Fills the polygons into a boolean mask of `size * size`, nonzero winding.
fn fill_mask(polys: &[Vec<(f32, f32)>], size: usize, scale: f32) -> Vec<bool> {
    let mut mask = vec![false; size * size];
    let mut xs: Vec<(f32, i32)> = Vec::new();

    for row in 0..size {
        let y = (row as f32 + 0.5) / scale;
        xs.clear();
        for poly in polys {
            for i in 0..poly.len() {
                let (x0, y0) = poly[i];
                let (x1, y1) = poly[(i + 1) % poly.len()];
                if (y0 <= y) == (y1 <= y) {
                    continue; // no crossing
                }
                let t = (y - y0) / (y1 - y0);
                xs.push((x0 + t * (x1 - x0), if y1 > y0 { 1 } else { -1 }));
            }
        }
        if xs.is_empty() {
            continue;
        }
        xs.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

        let mut winding = 0;
        for w in xs.windows(2) {
            winding += w[0].1;
            if winding == 0 {
                continue;
            }
            let (a, b) = (w[0].0 * scale, w[1].0 * scale);
            let (lo, hi) = (
                a.ceil().max(0.0) as usize,
                b.ceil().min(size as f32) as usize,
            );
            for col in lo..hi {
                mask[row * size + col] = true;
            }
        }
    }
    mask
}

/// Grows a mask by `r` samples in each direction, separably.
///
/// Used for the dark halo behind the light glyph, which is what lets the icon
/// read on a light taskbar as well as a dark one.
fn dilate(mask: &[bool], size: usize, r: usize) -> Vec<bool> {
    let mut h = vec![false; size * size];
    for y in 0..size {
        for x in 0..size {
            if mask[y * size + x] {
                let lo = x.saturating_sub(r);
                let hi = (x + r + 1).min(size);
                for nx in lo..hi {
                    h[y * size + nx] = true;
                }
            }
        }
    }
    let mut v = vec![false; size * size];
    for y in 0..size {
        for x in 0..size {
            if h[y * size + x] {
                let lo = y.saturating_sub(r);
                let hi = (y + r + 1).min(size);
                for ny in lo..hi {
                    v[ny * size + x] = true;
                }
            }
        }
    }
    v
}

/// Box-averages an `n * SS` mask down to `n`, giving per-pixel coverage.
fn downsample(mask: &[bool], n: usize) -> Vec<f32> {
    let big = n * SS;
    let mut out = vec![0.0; n * n];
    for y in 0..n {
        for x in 0..n {
            let mut hit = 0;
            for sy in 0..SS {
                for sx in 0..SS {
                    if mask[(y * SS + sy) * big + (x * SS + sx)] {
                        hit += 1;
                    }
                }
            }
            out[y * n + x] = hit as f32 / (SS * SS) as f32;
        }
    }
    out
}

fn channel(c: u32, shift: u32) -> f32 {
    ((c >> shift) & 0xFF) as f32
}

/// The icon as straight-alpha BGRA, `n` x `n`, row-major top-down.
pub fn icon_pixels(n: usize) -> Vec<u32> {
    let mut polys = polygons(path_data(SVG));
    // Fit the artwork to the canvas rather than honouring the viewBox. This
    // drawing sits in the middle ~60% of its box, and rendering it as authored
    // wastes half the pixels of a 16 px tray icon on empty margin.
    fit_to_canvas(&mut polys, view_box(SVG));
    let big = n * SS;
    let scale = big as f32 / view_box(SVG);

    let glyph = fill_mask(&polys, big, scale);
    // One target pixel of halo at every size, which is what keeps the shape
    // legible against a taskbar of either colour.
    let halo = dilate(&glyph, big, SS);

    let fill_cov = downsample(&glyph, n);
    let halo_cov = downsample(&halo, n);

    let mut px = vec![CLEAR; n * n];
    for i in 0..n * n {
        let (af, ao) = (fill_cov[i], halo_cov[i]);
        if af <= 0.0 && ao <= 0.0 {
            continue;
        }
        // Source-over: the light glyph on top of the dark halo.
        let alpha = af + ao * (1.0 - af);
        if alpha <= 0.0 {
            continue;
        }
        let mix = |shift: u32| {
            let c = (channel(FILL, shift) * af + channel(OUTLINE, shift) * ao * (1.0 - af)) / alpha;
            (c.round().clamp(0.0, 255.0) as u32) << shift
        };
        px[i] =
            ((alpha * 255.0).round().clamp(0.0, 255.0) as u32) << 24 | mix(16) | mix(8) | mix(0);
    }
    px
}

/// Sizes Windows asks for: taskbar and tray, Start menu, and Explorer's larger
/// views. 256 costs about 256 KB uncompressed and is what keeps the largest
/// views sharp.
pub const ICON_SIZES: [usize; 5] = [16, 32, 48, 128, 256];

/// Encodes the icon as a Windows `.ico`.
///
/// The format is a 6-byte header, one 16-byte directory entry per image, then
/// the images. Each image is a DIB whose declared height is **twice** the real
/// height: the format expects a colour bitmap followed by an AND mask. The mask
/// is left all-zero because the 32-bit colour data carries its own alpha.
///
/// Written by hand rather than with an image crate: this is the whole format,
/// and the project takes no dependency it can avoid.
pub fn encode_ico(sizes: &[usize]) -> Vec<u8> {
    const HEADER: usize = 6;
    const ENTRY: usize = 16;
    const DIB_HEADER: usize = 40;

    let mask_stride = |n: usize| n.div_ceil(32) * 4;
    let image_len = |n: usize| DIB_HEADER + n * n * 4 + n * mask_stride(n);

    let mut out = Vec::new();
    out.extend_from_slice(&0u16.to_le_bytes()); // reserved
    out.extend_from_slice(&1u16.to_le_bytes()); // type: icon
    out.extend_from_slice(&(sizes.len() as u16).to_le_bytes());

    let mut offset = HEADER + ENTRY * sizes.len();
    for &n in sizes {
        // 256 does not fit in a byte and is encoded as zero.
        let dim = if n >= 256 { 0u8 } else { n as u8 };
        out.push(dim); // width
        out.push(dim); // height
        out.push(0); // palette size: none
        out.push(0); // reserved
        out.extend_from_slice(&1u16.to_le_bytes()); // planes
        out.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
        out.extend_from_slice(&(image_len(n) as u32).to_le_bytes());
        out.extend_from_slice(&(offset as u32).to_le_bytes());
        offset += image_len(n);
    }

    for &n in sizes {
        let px = icon_pixels(n);

        out.extend_from_slice(&(DIB_HEADER as u32).to_le_bytes()); // biSize
        out.extend_from_slice(&(n as i32).to_le_bytes()); // biWidth
        out.extend_from_slice(&((n * 2) as i32).to_le_bytes()); // biHeight
        out.extend_from_slice(&1u16.to_le_bytes()); // biPlanes
        out.extend_from_slice(&32u16.to_le_bytes()); // biBitCount
        out.extend_from_slice(&0u32.to_le_bytes()); // biCompression: BI_RGB
        out.extend_from_slice(&0u32.to_le_bytes()); // biSizeImage
        out.extend_from_slice(&0i32.to_le_bytes()); // biXPelsPerMeter
        out.extend_from_slice(&0i32.to_le_bytes()); // biYPelsPerMeter
        out.extend_from_slice(&0u32.to_le_bytes()); // biClrUsed
        out.extend_from_slice(&0u32.to_le_bytes()); // biClrImportant

        // Bottom-up, which is what a positive biHeight means.
        for y in (0..n).rev() {
            for x in 0..n {
                out.extend_from_slice(&px[y * n + x].to_le_bytes());
            }
        }
        out.resize(out.len() + n * mask_stride(n), 0);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alpha(p: u32) -> u8 {
        (p >> 24) as u8
    }

    #[test]
    fn the_artwork_parses_into_three_closed_subpaths() {
        // A headband and two ear pieces. A parser that dropped a curve would
        // still produce polygons, so the count is worth pinning.
        let polys = polygons(path_data(SVG));
        assert_eq!(polys.len(), 3, "expected three subpaths");
        for (i, p) in polys.iter().enumerate() {
            assert!(
                p.len() > 8,
                "subpath {i} flattened to only {} points",
                p.len()
            );
        }
    }

    #[test]
    fn the_icon_is_neither_empty_nor_solid_at_every_size() {
        for n in [16, 32, 48, 128, 256] {
            let px = icon_pixels(n);
            assert_eq!(px.len(), n * n, "size {n} produced the wrong pixel count");
            // Counted on substantially-opaque pixels. Any-alpha counts the
            // anti-aliased fringe and the halo too, which at 16 px inflates an
            // ordinary glyph to 80% of the canvas and says nothing about
            // whether the shape is legible.
            let drawn = px.iter().filter(|p| alpha(**p) > 0x80).count();
            assert!(
                drawn > n * n / 50,
                "size {n} drew almost nothing: {drawn}px"
            );
            assert!(
                drawn < n * n * 3 / 4,
                "size {n} drew almost everything: {drawn}px"
            );
        }
    }

    #[test]
    fn the_left_side_carries_the_boom_microphone() {
        // The artwork is deliberately asymmetric: the left subpath is far longer
        // than the right because it draws a boom mic. That is the feature that
        // makes this a headset rather than headphones, and dropping it while
        // "simplifying" the path parser would be easy and quiet.
        for n in [48, 128] {
            let px = icon_pixels(n);
            // The ear cups bottom out around 88% of the height; only the boom
            // reaches below that, sweeping down from the left cup toward the
            // centre. Two earlier versions of this test guessed its position
            // wrongly, so it asserts the one thing that is structural: there is
            // ink below the cups at all.
            let below = ((n * 9 / 10)..n)
                .flat_map(|y| (0..n).map(move |x| (x, y)))
                .filter(|(x, y)| alpha(px[y * n + x]) > 0x40)
                .count();
            assert!(
                below > n / 8,
                "size {n}: expected the boom below the ear cups, found {below} pixels"
            );
        }
    }

    #[test]
    fn the_edges_are_anti_aliased() {
        // The whole reason for supersampling. Without it every pixel is fully
        // opaque or fully clear and the diagonals stair-step.
        let px = icon_pixels(48);
        let partial = px.iter().filter(|p| (1..255).contains(&alpha(**p))).count();
        assert!(partial > 100, "only {partial} partially transparent pixels");
    }

    #[test]
    fn the_glyph_sits_on_a_darker_halo() {
        // What lets the icon read on a light taskbar as well as a dark one.
        let px = icon_pixels(48);
        let light = px
            .iter()
            .filter(|p| (**p & 0xFF) > 0xC0 && alpha(**p) > 0x80)
            .count();
        let dark = px
            .iter()
            .filter(|p| (**p & 0xFF) < 0x60 && alpha(**p) > 0x80)
            .count();
        assert!(
            light > 50,
            "expected a light glyph, found {light} light pixels"
        );
        assert!(dark > 50, "expected a dark halo, found {dark} dark pixels");
    }

    fn u16_at(b: &[u8], o: usize) -> u16 {
        u16::from_le_bytes([b[o], b[o + 1]])
    }
    fn u32_at(b: &[u8], o: usize) -> u32 {
        u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
    }

    #[test]
    fn the_ico_header_declares_an_icon_with_one_entry_per_size() {
        let ico = encode_ico(&ICON_SIZES);
        assert_eq!(u16_at(&ico, 0), 0, "reserved");
        assert_eq!(u16_at(&ico, 2), 1, "type 1 = icon");
        assert_eq!(u16_at(&ico, 4) as usize, ICON_SIZES.len());
    }

    #[test]
    fn every_directory_entry_points_inside_the_file() {
        // A wrong offset or length yields an icon Windows silently refuses to
        // load, which looks exactly like "the icon didn't work" with no error.
        let ico = encode_ico(&ICON_SIZES);
        for (i, size) in ICON_SIZES.iter().enumerate() {
            let e = 6 + i * 16;
            let declared = ico[e] as usize;
            assert_eq!(
                declared,
                if *size == 256 { 0 } else { *size },
                "256 is encoded as 0; {size} was encoded as {declared}"
            );
            assert_eq!(u16_at(&ico, e + 4), 1, "planes");
            assert_eq!(u16_at(&ico, e + 6), 32, "bit count");
            let len = u32_at(&ico, e + 8) as usize;
            let off = u32_at(&ico, e + 12) as usize;
            assert!(
                off + len <= ico.len(),
                "entry {i} runs past the end of the file"
            );
            assert_eq!(
                u32_at(&ico, off),
                40,
                "each image starts with a 40-byte header"
            );
            assert_eq!(u32_at(&ico, off + 4) as usize, *size, "biWidth");
            assert_eq!(
                u32_at(&ico, off + 8) as usize,
                size * 2,
                "biHeight is doubled: colour bitmap plus AND mask"
            );
        }
    }

    #[test]
    fn the_committed_icon_matches_what_the_code_generates() {
        // The one that actually catches a forgotten regeneration.
        let committed: &[u8] = include_bytes!("../../assets/headset.ico");
        let fresh = encode_ico(&ICON_SIZES);
        // Compared by length and first difference rather than with assert_eq on
        // the vectors: a mismatch on 350 KB of pixels prints 350 KB of pixels,
        // which buries the one line telling you what to do about it.
        let first_diff = committed
            .iter()
            .zip(fresh.iter())
            .position(|(a, b)| a != b)
            .map(|i| i.to_string())
            .unwrap_or_else(|| "none".into());
        assert!(
            committed == fresh.as_slice(),
            "assets/headset.ico is stale ({} bytes committed, {} generated, first difference at {}); \
             regenerate it with \
             `cargo run -p headset-tray -- --export-icon crates/headset-tray/assets/headset.ico`",
            committed.len(),
            fresh.len(),
            first_diff
        );
    }
}
