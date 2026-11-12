//! A lantern is a finished block made visible. Its light comes from how the user said the
//! block felt (plain paper when they didn't say), and every pause ties a small knot in the cord
//! along its bottom rim, so a paused block's lantern carries its pauses plainly, never with less
//! light.

use rune::shape::{LineCap, Path, Shape};
use rune::{Color, El, canvas, el};

use crate::drawing::{disc, halo};
use crate::log::{Block, Felt};
use crate::theme::rgb;

/// How one lantern looks, derived from its block and nothing else.
#[derive(Clone, PartialEq, Debug)]
pub struct Lantern {
    pub tone: Color,
    /// Glow strength, 0..1. Felt shifts the warmth far more than the brightness.
    pub power: f32,
    /// How many times the block paused.
    pub pauses: usize,
}

impl Lantern {
    pub fn of(block: &Block) -> Lantern {
        let (tone, power) = Lantern::light(block.felt());
        Lantern { tone, power, pauses: block.pauses.len() }
    }

    /// A lantern's colour and glow for how its block felt: deep amber, pale gold or dusk rose
    /// paper, far enough apart to tell at a glance; unanswered, plain cream paper.
    pub fn light(felt: Option<Felt>) -> (Color, f32) {
        match felt {
            Some(Felt::Deep) => (rgb(0xff9440), 1.0),
            Some(Felt::Steady) => (rgb(0xffd77a), 0.9),
            Some(Felt::Drifting) => (rgb(0xeaa2c6), 0.84),
            None => (rgb(0xf7e2bc), 0.82),
        }
    }
}

/// How the lantern is drawn this frame.
#[derive(Clone, Copy)]
pub struct Pose {
    pub scale: f32,
    /// Seconds of fine motion; the same value draws the same frame.
    pub t: f32,
    /// Keeps neighbouring lanterns from bobbing in step.
    pub seed: u32,
    /// 0 unlit, 1 fully lit; overshoots a little as a new lantern blooms.
    pub bloom: f32,
    /// How far the glow reaches, 1 on the water.
    pub glow: f32,
    /// Draws its broken reflection under it.
    pub reflect: bool,
    /// The halo's blur, in points.
    pub soft: f32,
}

impl Pose {
    /// The square the lantern, its embers and its glow fit in, centred on the lantern.
    pub fn reach(&self) -> f32 {
        (2.0 * (self.halo() + 3.0 * self.soft) + 4.0).ceil()
    }

    fn halo(&self) -> f32 {
        30.0 * self.scale * self.glow
    }
}

/// The lantern in a square of `pose.reach()` points, to be placed with its centre where the
/// lantern floats. Keyed and labelled so it can be read and pointed at.
pub fn view(key: String, label: String, look: &Lantern, pose: Pose) -> El {
    let size = pose.reach();
    let mut soft = Shape::new();
    let mut crisp = Shape::new();
    draw(&mut soft, &mut crisp, [size / 2.0, size / 2.0], look, pose);
    // The lantern bobs by moving the whole drawing, so its blurred glow stays the same picture
    // and is blurred once, not every frame.
    let (t, s) = (pose.t, pose.seed as f32);
    let bob = [(t * 0.21 + s).sin() * 2.0 * pose.scale, (t * 0.9 + s * 1.9).sin() * 1.3 * pose.scale];
    el().key(key).size(size, size).role("img").label(label).instant_changes().child(
        el().absolute(0.0, 0.0)
            .size(size, size)
            .offset(bob[0], bob[1])
            .child(canvas(soft).absolute(0.0, 0.0).size(size, size).blur(pose.soft).cache().a11y_hidden())
            .child(canvas(crisp).absolute(0.0, 0.0).size(size, size).a11y_hidden()),
    )
}

/// Draws one lantern centred on `c`: its glow into `soft` (blurred by the caller), everything
/// with an edge into `crisp`.
pub fn draw(soft: &mut Shape, crisp: &mut Shape, c: [f32; 2], look: &Lantern, pose: Pose) {
    let Pose { scale, t, seed, bloom, .. } = pose;
    if bloom <= 0.0 {
        return;
    }
    let s = seed as f32;
    let (w, h) = (6.5 * scale, 8.5 * scale);
    let tone = look.tone;
    let lit = bloom.min(1.0);

    // The glow is always flame-warm, whatever the paper: a pale lantern still lights the water gold.
    let light = tone.mix(rgb(0xffb35c), 0.35);
    *soft = halo(
        std::mem::take(soft),
        c,
        pose.halo() * (0.6 + 0.4 * look.power) * (0.4 + 0.6 * lit),
        0.72,
        light,
        1.05 * look.power * lit,
    );
    let mut sh = std::mem::take(crisp);
    if pose.reflect {
        // A narrow column straight down, broken by the water, as a lantern lights it.
        for k in 0..7 {
            let y = c[1] + h * 0.85 + k as f32 * 3.0 * scale;
            let half = w * (0.42 - k as f32 * 0.04) * (0.7 + 0.3 * (t * 1.7 + k as f32 * 1.3 + s).sin());
            sh = sh.line(
                [c[0] - half, y],
                [c[0] + half, y],
                1.2 * scale,
                LineCap::Round,
                tone.alpha(look.power * (0.6 - k as f32 * 0.075) * lit),
            );
        }
    }
    // The paper body grows from nothing as it blooms, with a little overshoot.
    let (w, h) = (w * bloom, h * bloom);
    let body = Path::new()
        .move_to([c[0] - w * 0.38, c[1] - h * 0.5])
        .line_to([c[0] + w * 0.38, c[1] - h * 0.5])
        .cubic_to(
            [c[0] + w * 0.75, c[1] - h * 0.2],
            [c[0] + w * 0.75, c[1] + h * 0.25],
            [c[0] + w * 0.42, c[1] + h * 0.5],
        )
        .line_to([c[0] - w * 0.42, c[1] + h * 0.5])
        .cubic_to(
            [c[0] - w * 0.75, c[1] + h * 0.25],
            [c[0] - w * 0.75, c[1] - h * 0.2],
            [c[0] - w * 0.38, c[1] - h * 0.5],
        )
        .close();
    // Paper lit from inside: deeper toward its edge, brightest around the flame.
    sh = sh.fill_path(&body, tone.mix(rgb(0x6a2a10), 0.18).alpha(lit));
    let inner = Path::new()
        .move_to([c[0] - w * 0.26, c[1] - h * 0.42])
        .line_to([c[0] + w * 0.26, c[1] - h * 0.42])
        .cubic_to(
            [c[0] + w * 0.56, c[1] - h * 0.15],
            [c[0] + w * 0.56, c[1] + h * 0.2],
            [c[0] + w * 0.3, c[1] + h * 0.42],
        )
        .line_to([c[0] - w * 0.3, c[1] + h * 0.42])
        .cubic_to(
            [c[0] - w * 0.56, c[1] + h * 0.2],
            [c[0] - w * 0.56, c[1] - h * 0.15],
            [c[0] - w * 0.26, c[1] - h * 0.42],
        )
        .close();
    sh = sh.fill_path(&inner, rgb(0xfff4dc).mix(tone, 0.35).alpha(lit));
    sh = sh.line(
        [c[0], c[1] - h * 0.42],
        [c[0], c[1] + h * 0.42],
        0.6 * scale,
        LineCap::Butt,
        tone.mix(rgb(0x6a2a10), 0.3).alpha(0.35 * lit),
    );
    let rim = rgb(0x3b2a2a);
    sh = sh.line(
        [c[0] - w * 0.4, c[1] - h * 0.55],
        [c[0] + w * 0.4, c[1] - h * 0.55],
        1.6 * scale,
        LineCap::Round,
        rim.alpha(0.8 * lit),
    );
    sh = sh.line(
        [c[0] - w * 0.44, c[1] + h * 0.52],
        [c[0] + w * 0.44, c[1] + h * 0.52],
        1.4 * scale,
        LineCap::Round,
        rim.alpha(0.7 * lit),
    );

    // Pauses: a knot apiece on the cord along the bottom rim, evenly spaced from the middle.
    let n = look.pauses.min(5);
    for j in 0..n {
        let x = c[0] + (j as f32 - (n as f32 - 1.0) / 2.0) * 2.6 * scale;
        sh = disc(sh, [x, c[1] + h * 0.52], [1.05 * scale, 1.05 * scale], rim.alpha(0.95 * lit));
        sh =
            disc(sh, [x, c[1] + h * 0.52], [0.45 * scale, 0.45 * scale], rgb(0xfff1d8).mix(tone, 0.3).alpha(0.9 * lit));
    }
    *crisp = sh;
}
