//! Small shape helpers every drawing in the app shares: a fixed pseudo-random value, and
//! ellipses, discs and soft glows built from points.

use std::f32::consts::TAU;

use rune::Color;
use rune::shape::Shape;

/// A fixed pseudo-random value in 0..1 for `n`, so every frame draws the same sky.
pub fn hash(n: u32) -> f32 {
    let mut x = n.wrapping_mul(0x9E37_79B1) ^ 0x85EB_CA6B;
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    (x & 0xffff) as f32 / 65535.0
}

/// `n + 1` points along an ellipse at `c` with radii `r`, from angle `from` to `to`.
pub fn ring(c: [f32; 2], r: [f32; 2], n: usize, from: f32, to: f32) -> Vec<[f32; 2]> {
    (0..=n)
        .map(|i| {
            let a = from + (to - from) * i as f32 / n as f32;
            [c[0] + r[0] * a.cos(), c[1] + r[1] * a.sin()]
        })
        .collect()
}

/// A filled ellipse, with as many edges as its size needs.
pub fn disc(sh: Shape, c: [f32; 2], r: [f32; 2], color: Color) -> Shape {
    let n = (r[0].max(r[1]) * 3.0).clamp(8.0, 48.0) as usize;
    sh.fill(ring(c, r, n, 0.0, TAU), color)
}

/// A soft glow from stacked translucent discs, for a canvas that the caller blurs.
pub fn halo(mut sh: Shape, c: [f32; 2], r: f32, squash: f32, color: Color, strength: f32) -> Shape {
    for k in 0..7 {
        let f = 1.0 - k as f32 / 7.0;
        sh = disc(sh, c, [r * f, r * f * squash], color.alpha(strength * 0.11));
    }
    sh
}
