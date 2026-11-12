//! Themes are places. Each has its own sky, as six keyframes along the evening, its own land
//! against the horizon and its own kind of water: an open swell, a still lake that mirrors its
//! shore, a salt flat under a thin sheet of water.

use rune::Color;
use rune::color::srgb_decode;
use rune::shape::{LineCap, Path, Shape};

/// The window as it first opens, in points: also the smallest it goes.
pub const W: f32 = 380.0;
pub const H: f32 = 540.0;

/// The window's size in points, and where the sea meets the sky in it. The horizon sits at the
/// same share of the height at every size, so a taller window has more sky and more sea, and a
/// wider one a longer horizon; nothing in the scene grows.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Frame {
    pub w: f32,
    pub h: f32,
    pub hz: f32,
}

impl Frame {
    /// The window as it first opens.
    pub const FIRST: Frame = Frame::of(W, H);

    /// A window `w` by `h` points: the horizon 330 points down at the first size.
    pub const fn of(w: f32, h: f32) -> Frame {
        Frame { w, h, hz: h * 11.0 / 18.0 }
    }
}

/// One moment of the evening: zenith, middle and low sky, the sea near and far, the sun, its
/// glow and the clouds.
pub struct Pal {
    pub zen: Color,
    pub mid: Color,
    pub low: Color,
    pub sea_hi: Color,
    pub sea_lo: Color,
    pub sun: Color,
    pub glow: Color,
    pub cloud: Color,
}

/// The sky axis runs 0 (start of a focus block, afternoon) through 1 (the sun touches the sea)
/// to 1.35 (afterglow) and 1.7 (night).
type Keys = [(f32, [u32; 8]); 6];

/// What the water does with the light.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Water {
    /// Open sea: swell lines drifting, the sun a wide broken glitter path.
    Swell,
    /// A still lake: the sky and shore mirrored, a few slow ripples.
    Mirror,
    /// A salt flat under a film of water: pale, glassy, the horizon shimmering in the heat.
    Salt,
}

pub struct Place {
    pub name: &'static str,
    keys: Keys,
    /// Draws the land along the horizon in `near` and `far` tones; `up` is -1 to draw it upside
    /// down below the horizon, as its reflection.
    land: fn(Shape, Color, Color, f32, Frame) -> Shape,
    /// The rock or forest the land is shaded toward.
    rock: u32,
    pub water: Water,
    /// Half-width of the sun's glitter path at its start, in points.
    pub glitter: f32,
}

pub const PLACES: &[Place] = &[
    Place {
        name: "Coast",
        keys: [
            (0.0, [0x5b8cbd, 0x9dbdd4, 0xece3cc, 0x9ab1bd, 0x3d5e75, 0xfffaf0, 0xfff1d2, 0xfbf6ee]),
            (0.45, [0x5874a6, 0xb4abb9, 0xf5cf9b, 0xb3a29c, 0x3a4766, 0xfff0c8, 0xffd99a, 0xfde6c8]),
            (0.8, [0x3e4580, 0xc48a93, 0xffaa68, 0xb38272, 0x2b3056, 0xffd08a, 0xffa560, 0xf6a787]),
            (1.0, [0x2a2b5b, 0x9e5a7c, 0xf07a4a, 0x96595e, 0x1f2347, 0xffb066, 0xff6a35, 0xe5767c]),
            (1.35, [0x221d50, 0x5e3f80, 0xdc7378, 0x4e3a6c, 0x171436, 0xff7a40, 0xd86f80, 0x9a5a8e]),
            (1.7, [0x0b0a26, 0x1c1644, 0x352a5e, 0x201b46, 0x08071c, 0xff7a40, 0x3e2e66, 0x2c2654]),
        ],
        land: coast,
        rock: 0x1a2040,
        water: Water::Swell,
        glitter: 34.0,
    },
    Place {
        name: "Lake",
        keys: [
            (0.0, [0x6f8fb8, 0xa9b9cf, 0xe6dfd6, 0xa3b2c0, 0x4a5d70, 0xfffaf2, 0xfdf0dc, 0xf6f3f0]),
            (0.45, [0x6a78a8, 0xb3aec4, 0xf2d2b4, 0xb0a7ae, 0x434a68, 0xfff1d6, 0xffdcb4, 0xf8e4d6]),
            (0.8, [0x3f5a80, 0x8a9eaa, 0xf0bc9e, 0x8fa2a4, 0x2c4558, 0xffd8ac, 0xffb08c, 0xdcc0b8]),
            (1.0, [0x22364f, 0x4d7480, 0xd8a48a, 0x6a8f92, 0x1b3242, 0xffc89c, 0xeea283, 0xb7c2c0]),
            (1.35, [0x162a3c, 0x2d5560, 0x7fa49a, 0x3b6068, 0x0f2230, 0xff9c74, 0x86b0a6, 0x56767a]),
            (1.7, [0x06121c, 0x0d2430, 0x1d4048, 0x143440, 0x050f16, 0xff8a70, 0x2c5258, 0x1c343a]),
        ],
        land: lake,
        rock: 0x10261e,
        water: Water::Mirror,
        glitter: 9.0,
    },
    Place {
        name: "Mesa",
        keys: [
            (0.0, [0x6f9cc0, 0xc6d0cc, 0xf2dcc0, 0xe6d4c4, 0xb89c8c, 0xfffbee, 0xfff0cc, 0xfbf5ea]),
            (0.45, [0x6a84a6, 0xd2b4a4, 0xf6c48e, 0xe2c0a6, 0xa88272, 0xffefc0, 0xffd08a, 0xfde0c0]),
            (0.8, [0x4a5a86, 0xc98a7c, 0xf2925a, 0xdc9c7e, 0x8a5e58, 0xffcf86, 0xff9a50, 0xf2a07c]),
            (1.0, [0x3c3050, 0xa8606a, 0xe39a58, 0xd39a78, 0x7a4a48, 0xffb468, 0xf08a48, 0xdc8a78]),
            (1.35, [0x2a1d36, 0x74404e, 0xc27e52, 0x8e5a58, 0x3a2228, 0xff8a48, 0xc87a52, 0x865060]),
            (1.7, [0x140c18, 0x2a1824, 0x4a2c2a, 0x362024, 0x120a0e, 0xff7a40, 0x5a3a30, 0x34222a]),
        ],
        land: mesa,
        rock: 0x5a2420,
        water: Water::Salt,
        glitter: 16.0,
    },
];

impl Place {
    pub fn at(index: usize) -> &'static Place {
        &PLACES[index % PLACES.len()]
    }

    /// The sky at `x` along the evening, blended between the two nearest keyframes.
    pub fn pal(&self, x: f32) -> Pal {
        let keys = &self.keys;
        let x = x.clamp(0.0, keys[keys.len() - 1].0);
        let i = keys.iter().rposition(|k| k.0 <= x).unwrap_or(0).min(keys.len() - 2);
        let (a, b) = (keys[i], keys[i + 1]);
        let t = ((x - a.0) / (b.0 - a.0)).clamp(0.0, 1.0);
        let c = |j: usize| rgb(a.1[j]).mix(rgb(b.1[j]), t);
        Pal { zen: c(0), mid: c(1), low: c(2), sea_hi: c(3), sea_lo: c(4), sun: c(5), glow: c(6), cloud: c(7) }
    }

    /// The land against the horizon, toned for this moment of the evening.
    pub fn land(&self, sh: Shape, p: &Pal, f: Frame) -> Shape {
        let (near, far) = self.land_tones(p);
        (self.land)(sh, near, far, 1.0, f)
    }

    /// The land mirrored in still water, a little lighter and bluer than the land itself.
    pub fn reflection(&self, sh: Shape, p: &Pal, strength: f32, f: Frame) -> Shape {
        let (near, far) = self.land_tones(p);
        let tint = |c: Color| c.mix(p.sea_hi, 0.3).alpha(strength);
        (self.land)(sh, tint(near), tint(far), -1.0, f)
    }

    fn land_tones(&self, p: &Pal) -> (Color, Color) {
        let near = p.sea_lo.mix(p.zen, 0.25).mix(rgb(self.rock), 0.35).mix(rgb(0x000000), 0.12);
        (near, near.mix(p.low, 0.45))
    }
}

/// The point `h` above the horizon, or below it when `up` is -1.
fn at(f: Frame, x: f32, h: f32, up: f32) -> [f32; 2] {
    [x, f.hz - up * h]
}

/// A low headland on the left and a far spit of land on the right.
/// The headland keeps to the left edge and the spit to the right; a wider window opens the sea
/// between them.
fn coast(sh: Shape, near: Color, far: Color, up: f32, f: Frame) -> Shape {
    let p = |x, h| at(f, x, h, up);
    let hill = Path::new()
        .move_to(p(-4.0, -1.0))
        .line_to(p(-4.0, 28.0))
        .cubic_to(p(20.0, 34.0), p(44.0, 26.0), p(62.0, 17.0))
        .cubic_to(p(84.0, 8.0), p(104.0, 9.0), p(132.0, 3.0))
        .cubic_to(p(142.0, 1.0), p(150.0, 0.0), p(158.0, -1.0))
        .close();
    let spit = Path::new()
        .move_to(p(f.w - 96.0, -1.0))
        .cubic_to(p(f.w - 70.0, 7.0), p(f.w - 40.0, 9.0), p(f.w - 18.0, 6.0))
        .cubic_to(p(f.w - 8.0, 5.0), p(f.w, 4.0), p(f.w + 4.0, 4.0))
        .line_to(p(f.w + 4.0, -1.0))
        .close();
    sh.fill_path(&spit, far).fill_path(&hill, near)
}

/// A hazy mountain ridge across the far shore, and pines down to the water on either side.
/// The ridge spans the far shore at any width; the pines keep to the two edges.
fn lake(mut sh: Shape, near: Color, far: Color, up: f32, f: Frame) -> Shape {
    let p = |x, h| at(f, x, h, up);
    let ridge: Vec<[f32; 2]> = [
        (-4.0, 0.0),
        (-4.0, 26.0),
        (30.0, 34.0),
        (62.0, 52.0),
        (84.0, 44.0),
        (104.0, 58.0),
        (126.0, 40.0),
        (150.0, 24.0),
        (176.0, 9.0),
        (204.0, 4.0),
        (234.0, 12.0),
        (262.0, 40.0),
        (284.0, 38.0),
        (318.0, 50.0),
        (350.0, 34.0),
        (384.0, 28.0),
        (384.0, 0.0),
    ]
    .iter()
    .map(|&(x, h)| p(x * f.w / W, h - 1.0))
    .collect();
    sh = sh.fill(ridge, far.mix(near, 0.15).alpha(0.75 * far.a));
    let shore = |x0: f32, x1: f32, h0: f32, h1: f32| {
        Path::new()
            .move_to(p(x0, -1.0))
            .line_to(p(x0, h0))
            .cubic_to(p(x0 + (x1 - x0) * 0.4, h0), p(x0 + (x1 - x0) * 0.7, h1), p(x1, -1.0))
            .close()
    };
    sh = sh.fill_path(&shore(-4.0, 128.0, 16.0, 4.0), near).fill_path(&shore(f.w + 4.0, f.w - 92.0, 10.0, 3.0), near);
    // Pines: a stack of narrowing triangles each, taller near the edges.
    for i in 0..22 {
        let left = i < 14;
        let k = if left { i as f32 / 13.0 } else { (i - 14) as f32 / 7.0 };
        let x = if left { -2.0 + k * 118.0 } else { f.w + 2.0 - k * 80.0 };
        let base = if left { 16.0 * (1.0 - k).powf(0.7) } else { 10.0 * (1.0 - k).powf(0.7) };
        let tall = (34.0 - 26.0 * k) * (0.75 + 0.5 * crate::drawing::hash(i as u32 + 300));
        let wide = tall * 0.32;
        for tier in 0..3 {
            let f = tier as f32 / 3.0;
            let y = base + tall * f * 0.8;
            let w = wide * (1.0 - f * 0.55);
            sh = sh.fill(vec![p(x - w, y), p(x + w, y), p(x, y + tall * (0.5 + f * 0.1))], near);
        }
        sh = sh.line(p(x, base - 1.0), p(x, base + 3.0), 1.2, LineCap::Butt, near);
    }
    sh
}

/// Flat-topped mesas and a butte far off across the salt: a sheer caprock over a concave
/// skirt of scree.
/// The buttes keep their size; a wider window spreads them along the horizon.
fn mesa(sh: Shape, near: Color, far: Color, up: f32, f: Frame) -> Shape {
    let p = |x, h| at(f, x, h, up);
    let butte = |x0: f32, x1: f32, h: f32, skirt: f32| {
        let (x0, x1) = (x0 * f.w / W, x1 + x0 * (f.w / W - 1.0));
        Path::new()
            .move_to(p(x0 - skirt, -1.0))
            .quad_to(p(x0 - skirt * 0.2, h * 0.1), p(x0 - 2.0, h * 0.58))
            .line_to(p(x0 - 0.5, h - 2.0))
            .line_to(p(x0 + 1.5, h))
            .line_to(p(x0 + (x1 - x0) * 0.45, h - 0.8))
            .line_to(p(x1 - 1.5, h))
            .line_to(p(x1 + 0.5, h - 2.0))
            .line_to(p(x1 + 2.0, h * 0.58))
            .quad_to(p(x1 + skirt * 0.2, h * 0.1), p(x1 + skirt, -1.0))
            .close()
    };
    let haze = far.mix(near, 0.1).alpha(0.7 * far.a);
    sh.fill_path(&butte(240.0, 296.0, 26.0, 26.0), haze)
        .fill_path(&butte(322.0, 330.0, 19.0, 12.0), haze)
        .fill_path(&butte(104.0, 112.0, 15.0, 10.0), near.mix(far, 0.45))
        .fill_path(&butte(-30.0, 58.0, 40.0, 34.0), near)
}

/// sRGB hex to Rune's linear colour.
pub fn rgb(hex: u32) -> Color {
    let ch = |shift: u32| srgb_decode(((hex >> shift) & 0xff) as f32 / 255.0);
    Color { r: ch(16), g: ch(8), b: ch(0), a: 1.0 }
}

/// Warm white, for everything written on the night: Today, the keys, the readout after dusk.
pub fn ink() -> Color {
    rgb(0xfbf1e2)
}

/// What the readout and controls write in, and the shade they sit on: warm white on a dusky
/// shade, or, over pale afternoon water, deep blue on a light one. Whichever reads better.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Ink {
    pub text: Color,
    pub shade: Color,
}

impl Ink {
    pub fn light() -> Ink {
        Ink { text: ink(), shade: rgb(0x070a1c) }
    }

    pub fn dark() -> Ink {
        Ink { text: rgb(0x121630), shade: rgb(0xfff8ee) }
    }

    /// The ink with more contrast against `ground`.
    pub fn on(ground: Color) -> Ink {
        let lum = |c: Color| 0.2126 * c.r + 0.7152 * c.g + 0.0722 * c.b;
        let contrast = |a: Color, b: Color| (lum(a).max(lum(b)) + 0.05) / (lum(a).min(lum(b)) + 0.05);
        let (light, dark) = (Ink::light(), Ink::dark());
        if contrast(light.text, ground) >= contrast(dark.text, ground) { light } else { dark }
    }
}
