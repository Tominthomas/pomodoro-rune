//! The scene: one focus block is one sunset, played in real time. The gap between the sun and
//! the sea is the time left: the sun's lower limb touches the water at 00:00, and the bell sinks
//! it. It is a pure function of `Scene`, which the app derives from the timer, the log and the
//! theme each frame; nothing here holds state.

use std::f32::consts::{PI, TAU};
use std::sync::OnceLock;

use rune::shape::{LineCap, Shape};
use rune::{Color, El, Pixels, canvas, el};

use crate::drawing::{disc, halo, hash, ring};
use crate::lantern::{self, Lantern, Pose};
use crate::theme::{H, HZ, Pal, Place, W, Water, rgb};
use crate::timer::{Clock, Phase, RING, Timer};

/// The sun is the progress, so it is drawn large enough to hold the eye.
const SUN_R: f32 = 36.0;
/// How far the glow, flash and glitter reach, relative to the sun they were tuned for.
const SUN_SCALE: f32 = SUN_R / 23.0;
/// How high the sun starts for an hour-long block. Shorter blocks start lower, on a square
/// root so that a five-minute block still has sky to cross.
const RISE: f32 = 240.0;
/// Along the sun's path, 1 is its lower limb on the water and `UNDER` its upper limb.
const UNDER: f32 = 2.0;
/// How long the bell takes to sink the sun, in seconds.
const SINK: f32 = 2.6;
/// The readout's corner of the water, kept calm: no lanterns, and little texture.
const CALM: [f32; 2] = [250.0, 388.0];

/// Where the evening is. Derived each frame from the timer, never stored.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Sky {
    /// Focus: 0 at the start, 1 when the sun touches the sea. Just after a break, `dawn` brings
    /// the morning up out of the break's own sky.
    Sunset { progress: f32, dawn: Option<Dawn> },
    /// A focus block just ended, `since` seconds ago. `kept` is false when it was let go
    /// at `from`: the sun sinks the rest of the way with no flash and no lantern. `long` when
    /// it rings in the long break, whose night it darkens toward.
    Bell { since: f32, kept: bool, from: f32, long: bool },
    /// Short break: 0..1 through blue hour.
    BlueHour { progress: f32 },
    /// Long break: 0..1, the moon's rise and the lanterns' ascent.
    Night { progress: f32 },
}

/// The morning after a break, as it comes up: `k` runs 0 to 1, from the sky the break ended
/// in (the night, or blue hour `from` of the way through) to the afternoon a block starts in.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Dawn {
    pub k: f32,
    pub night: bool,
    pub from: f32,
}

impl Dawn {
    /// How long a dawn takes, after the long break and after a short one.
    pub fn secs(night: bool) -> f32 {
        if night { 6.0 } else { 3.5 }
    }
}

impl Sky {
    /// The sky for the timer at `now`, with the dawn under way if a break just ended.
    pub fn of(timer: &Timer, now: f64, dawn: Option<Dawn>) -> Sky {
        let progress = timer.progress(now);
        match (timer.phase, &timer.clock) {
            (Phase::Focus, Clock::Ringing { until, kept, from }) => {
                let long = timer.settled().0 == Phase::Long;
                Sky::Bell { since: (RING - (until - now)).max(0.0) as f32, kept: *kept, from: *from, long }
            }
            (Phase::Focus, _) => Sky::Sunset { progress, dawn },
            (Phase::Short, _) => Sky::BlueHour { progress },
            (Phase::Long, _) => Sky::Night { progress },
        }
    }

    /// Where the sun is along its path: 0 at its start, 1 touching the sea, `UNDER` gone.
    fn sun(self) -> f32 {
        let gone = UNDER + 0.05;
        match self {
            // At dawn the sun comes up out of the sea to where the block starts it.
            Sky::Sunset { progress, dawn: Some(d) } => gone + (progress - gone) * ease((d.k - 0.3) / 0.7),
            Sky::Sunset { progress, .. } => progress,
            Sky::Bell { since, kept: true, .. } => 1.0 + (gone - 1.0) * ease(since / SINK),
            Sky::Bell { since, from, .. } => from + (gone - from) * ease(since / SINK),
            Sky::BlueHour { .. } | Sky::Night { .. } => gone,
        }
    }

    /// The one scalar every colour is keyed on. Dark is the same evening later and dimmer: a
    /// focus block runs from golden hour into afterglow, and the bell brings blue hour. Light
    /// mode's night is moonlit; dark mode's is deep.
    pub fn axis(self, dark: bool) -> f32 {
        type Curve = fn(f32) -> f32;
        let (focus, short, night): (Curve, Curve, f32) =
            if dark { (|p| 0.8 + 0.55 * p, |p| 1.52 + 0.18 * p, 1.7) } else { (|p| p, |p| 1.25 + 0.15 * p, 1.48) };
        match self {
            Sky::Sunset { progress, dawn: Some(d) } => {
                let start = if d.night { night } else { short(d.from) };
                start + (focus(progress) - start) * ease(d.k / 0.85)
            }
            Sky::Sunset { progress, .. } => focus(progress),
            // The bell only ever darkens: from where the block ended to exactly where the break
            // begins, eased over the whole ring.
            Sky::Bell { since, kept, from, long } => {
                let start = focus(if kept { 1.0 } else { from });
                let end = if long { night } else { short(0.0) };
                start + (end - start) * ease(since / RING as f32)
            }
            Sky::BlueHour { progress } => short(progress),
            Sky::Night { .. } => night,
        }
    }

    /// How much of blue hour's afterglow band shows: it takes over from the sun's glow through
    /// the bell, then thins as the break runs out.
    fn afterglow(self) -> f32 {
        match self {
            Sky::Bell { long: false, since, .. } => ease(since / RING as f32),
            Sky::BlueHour { .. } => 1.0,
            Sky::Sunset { dawn: Some(d), .. } if !d.night => 1.0 - ease(d.k / 0.6),
            _ => 0.0,
        }
    }

    /// How far through blue hour the sky stands, for the evening star and the afterglow.
    fn blue(self) -> Option<f32> {
        match self {
            Sky::BlueHour { progress } => Some(progress),
            Sky::Bell { long: false, .. } => Some(0.0),
            Sky::Sunset { dawn: Some(d), .. } if !d.night => Some(d.from),
            _ => None,
        }
    }

    /// How much of the evening star shows, before the sky's own darkness has its say.
    fn venus(self) -> f32 {
        match self {
            Sky::Night { .. } | Sky::Sunset { dawn: Some(Dawn { night: true, .. }), .. } => 0.0,
            Sky::Bell { long: true, since, .. } => 1.0 - ease(since / RING as f32),
            Sky::Sunset { dawn: Some(d), .. } => 1.0 - ease(d.k / 0.6),
            _ => 1.0,
        }
    }
}

/// Everything the scene draws from.
pub struct Scene<'a> {
    pub sky: Sky,
    pub place: &'static Place,
    pub dark: bool,
    /// The length of the block being set or run, in seconds: it sets how high the sun starts.
    pub length: u32,
    /// Seconds of fine motion (glitter, drifting clouds, bobbing lanterns). Frozen while still.
    pub drift: f32,
    /// This cycle's lanterns, oldest first. At the bell the last one is the one blooming.
    pub water: &'a [Lantern],
    /// Lanterns from earlier cycles today, now stars that come out at dusk.
    pub stars: &'a [Lantern],
    /// Paused: the sky holds its breath under a light cool haze. Lanterns stay as bright.
    pub held: bool,
}

#[expect(clippy::too_many_lines, reason = "one scene, painted back to front; splitting it would hide the layer order")]
pub fn view(s: &Scene<'_>) -> El {
    let x = s.sky.axis(s.dark);
    let p = s.place.pal(x);
    let t = s.drift;
    let sun_p = s.sky.sun();
    let rise = rise(f64::from(s.length)) as f32;
    let squash = SUN_R * squash(sun_p);
    let sun = [W * 0.5 - 4.0 + 34.0 * sun_p.min(1.0), sun_y(sun_p, rise)];
    let sun_vis = ((UNDER + 0.05 - sun_p) / 0.05).clamp(0.0, 1.0);
    let dusk = ((x - 1.0) / 0.7).clamp(0.0, 1.0);
    let night = match s.sky {
        Sky::Night { progress } => Some(progress),
        _ => None,
    };
    let dawn = match s.sky {
        Sky::Sunset { dawn: Some(d), .. } if d.night => Some(d.k),
        _ => None,
    };
    let blue = s.sky.blue();
    let hz = HZ / H;

    let sky = el().size(W, H).linear_gradient(
        PI,
        [
            (0.0, p.zen),
            (0.3 * hz, p.zen.mix(p.mid, 0.45)),
            (0.62 * hz, p.mid),
            (0.86 * hz, p.mid.mix(p.low, 0.6)),
            (0.97 * hz, p.low),
            (1.0, p.low),
        ],
    );
    let glow_k = sun_vis.max(0.35 * (1.0 - dusk));
    let glow = el().pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0)).radial_gradient(
        [sun[0] / W, sun[1].min(HZ) / H],
        [(250.0 + 90.0 * sun_p.min(1.0)) * SUN_SCALE.sqrt(), (150.0 + 40.0 * sun_p.min(1.0)) * SUN_SCALE.sqrt()],
        [
            (0.0, p.glow.alpha(0.75 * glow_k)),
            (0.1, p.glow.alpha(0.5 * glow_k)),
            (0.35, p.glow.alpha(0.18 * glow_k)),
            (1.0, p.glow.alpha(0.0)),
        ],
    );

    // Above the horizon: stars, today's lantern-stars, the moon, the sun, then the land.
    let mut soft = Shape::new();
    let mut a = Shape::new();
    let star_k = ((x - 1.1) / 0.5).clamp(0.0, 1.0);
    for k in 0..110u32 {
        let (sx, sy) = (hash(k) * W, hash(k + 500).powf(1.35) * (HZ - 14.0));
        let twinkle = 0.55 + 0.45 * (t * (0.5 + hash(k + 9)) + k as f32 * 2.0).sin();
        let r = 0.45 + hash(k + 77).powi(3) * 1.1;
        a = disc(a, [sx, sy], [r, r], rgb(0xe8ecff).alpha(star_k * twinkle * (1.0 - sy / HZ * 0.6) * 0.85));
    }
    // Today's lanterns come out before the ordinary stars, warm among the cold. The cycle the
    // long break just closed keeps its lanterns' shape aloft until the dawn makes them stars.
    let warm_k = ((x - 1.02) / 0.15).clamp(0.0, 1.0);
    let aloft = match s.sky {
        Sky::Night { .. } if s.water.is_empty() => 1.0,
        Sky::Sunset { dawn: Some(d), .. } if d.night => 1.0 - ease(d.k / 0.6),
        _ => 0.0,
    };
    let mut aloft_lanterns = Vec::new();
    for (i, l) in s.stars.iter().enumerate() {
        let c = star_at(i);
        let shaped = if i + 4 >= s.stars.len() { aloft } else { 0.0 };
        let k = warm_k * (0.85 + 0.15 * (t * 0.7 + i as f32 * 1.3).sin()) * (1.0 - shaped);
        soft = halo(soft, c, 6.0, 1.0, l.tone, 0.7 * warm_k * (1.0 - shaped));
        a = disc(a, c, [1.5, 1.5], rgb(0xfff4e0).mix(l.tone, 0.4).alpha(k));
        if shaped > 0.0 {
            let pose = Pose { scale: 1.26, t, seed: i as u32, bloom: shaped, glow: 0.5, reflect: false, soft: 5.0 };
            let half = pose.reach() / 2.0;
            aloft_lanterns.push(
                lantern::view(format!("aloft-{i}"), String::new(), l, pose)
                    .absolute(c[0] - half, c[1] - half)
                    .a11y_hidden(),
            );
        }
    }
    // The evening star comes out as the sun goes and sinks a little toward the sea through the
    // break: the break's own clock.
    let venus = ((x - 1.08) / 0.2).clamp(0.0, 1.0) * s.sky.venus();
    if venus > 0.0 {
        let v = [W * 0.27, HZ - 128.0 + 46.0 * blue.unwrap_or(0.0)];
        soft = halo(soft, v, 10.0 + 6.0 * venus, 1.0, rgb(0xfff4e0), venus * 0.9);
        a = disc(a, v, [1.5 + 0.6 * venus, 1.5 + 0.6 * venus], rgb(0xfffaf0).alpha(venus));
    }
    // The moon climbs through the whole long break, so how high it stands is how far the
    // break has gone; at dawn it sets.
    let moon = match (night, dawn) {
        (Some(n), _) => Some(HZ + 16.0 - ease(n) * 200.0),
        (_, Some(d)) => Some(HZ - 184.0 + ease(d / 0.6) * 214.0),
        _ => None,
    };
    // It clears the horizon as it rises and sinks behind it as it sets: never a pop.
    let moon_k = moon.map_or(0.0, |my| ((HZ - 6.0 - my) / 22.0).clamp(0.0, 1.0));
    if let Some(my) = moon {
        let k = moon_k;
        let m = [W * 0.74, my];
        soft = halo(soft, m, 54.0, 1.0, rgb(0xcfd6f5), 0.5 * k);
        a = disc(a, m, [15.0, 15.0], rgb(0xf3eedf).alpha(k));
        a = disc(a, [m[0] + 4.0, m[1] - 3.0], [3.0, 2.6], rgb(0xdcd5c3).alpha(k));
        a = disc(a, [m[0] - 5.0, m[1] + 4.0], [2.2, 2.0], rgb(0xe2dbca).alpha(k));
    }
    if sun_vis > 0.0 {
        if let Sky::Bell { kept: true, .. } = s.sky {
            // The green flash: the last limb's top edge turns green as it goes.
            let g = ((sun_p - 1.78) / 0.14).clamp(0.0, 1.0) * sun_vis;
            soft = halo(soft, [sun[0], sun[1] - squash * 0.95], 24.0 * SUN_SCALE, 0.45, rgb(0xb8f5c4), g * 1.1);
            a = disc(a, [sun[0], sun[1] - 2.2], [SUN_R * 0.98, squash], rgb(0xa9f0b4).alpha(g));
        }
        // Brighter toward the middle in fine steps, so the disc glows instead of ringing.
        let core = rgb(0xfffdf6).mix(p.sun, 0.35 + 0.65 * sun_p.min(1.0));
        a = disc(a, sun, [SUN_R, squash], p.sun.alpha(sun_vis));
        for k in 1..8 {
            let f = 1.0 - k as f32 * 0.09;
            a = disc(a, [sun[0], sun[1] - 2.0 * (1.0 - f) * 4.0], [SUN_R * f, squash * f], core.alpha(sun_vis * 0.12));
        }
    }
    let land = s.place.land(Shape::new(), &p);

    // The cycle's lanterns: on the water, or rising into today's sky through the long break,
    // one after another over minutes, to settle there as a small constellation.
    let mut lanterns = Vec::new();
    let mut sky_lanterns = Vec::new();
    for (i, l) in s.water.iter().enumerate().take(4) {
        let (spot, scale) = sea_spot(i);
        let newest = i + 1 == s.water.len();
        // The newest lantern lights at a size you can read, then settles to its place's size.
        let (bloom, big) = match s.sky {
            Sky::Bell { since, kept: true, .. } if newest => {
                (back((since - 0.5) / 1.3), 1.0 + 1.1 * ease((since - 0.4) / 0.7) * (1.0 - ease((since - 2.0) / 1.5)))
            }
            _ => (1.0, 1.0),
        };
        let (c, scale, glow, reflect) = match night {
            Some(n) => {
                let rise = ease((n - 0.04 - 0.1 * i as f32) / 0.36);
                let target = star_at(s.stars.len() + i);
                let sway = (t * 0.4 + i as f32 * 1.7).sin() * 5.0 * rise * (1.0 - rise) * 4.0;
                let c = [
                    spot[0] + (target[0] - spot[0]) * ease(rise * 1.15) + sway,
                    spot[1] + (target[1] - spot[1]) * rise,
                ];
                // Aloft they keep a lantern's shape, a little smaller than on the water, never a dot.
                (c, scale.max(1.4) * (1.0 - 0.1 * rise), 0.8 - 0.3 * rise, rise < 0.05)
            }
            None => (spot, scale * big, 1.0, true),
        };
        let pose = Pose { scale, t, seed: i as u32, bloom, glow, reflect, soft: 5.0 };
        let half = pose.reach() / 2.0;
        let lantern =
            lantern::view(format!("lantern-{i}"), lantern_label(i, l), l, pose).absolute(c[0] - half, c[1] - half);
        if c[1] < HZ - 2.0 { sky_lanterns.push(lantern) } else { lanterns.push(lantern) }
    }

    let stage = sky
        .child(glow)
        .child(canvas(soft).absolute(0.0, 0.0).size(W, H).blur(7.0).cache())
        .child(canvas(a).absolute(0.0, 0.0).size(W, H))
        // The sun's box, for tools that check where it stands; it draws nothing.
        .child(
            el().key("sun")
                .absolute(sun[0] - SUN_R, sun[1] - squash)
                .size(2.0 * SUN_R, 2.0 * squash)
                .opacity(sun_vis)
                .a11y_hidden(),
        )
        .child(clouds(&p, sun_p, dusk, t))
        .child(canvas(land).absolute(0.0, 0.0).size(W, H));
    // Every layer is drawn in every phase, empty when it has nothing to show, so the scene keeps
    // one shape and nothing is matched against a different layer as a phase ends.
    let mut stage = stage.child(afterglow(s.place, blue.unwrap_or(0.0), s.sky.afterglow()));
    stage = sky_lanterns.into_iter().chain(aloft_lanterns).fold(stage, El::child);
    stage = stage.child(water(s, &p, sun, sun_p, sun_vis, glow_k, moon.map(|_| moon_k).filter(|k| *k > 0.0)));
    stage = stage.child(canvas(ripple(s.sky, sun[0])).key("ripple").absolute(0.0, 0.0).size(W, H).blur(0.8));
    stage = stage.child(el().key("grain").absolute(0.0, 0.0).size(W, H).pixels(grain()));
    stage =
        stage.child(
            el().key("dusk-veil")
                .pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
                .fill(rgb(0x04061a).alpha(if s.dark { 0.22 } else { 0.0 })),
        );
    stage = stage.child(
        el().key("veil").pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0)).fill(rgb(0x2a3058).alpha(if s.held {
            0.14
        } else {
            0.0
        })),
    );
    lanterns.into_iter().fold(stage, El::child)
}

/// The water below the horizon, as this place has it, with the sun's or the moon's light on it.
/// `moon` is how much of the moon shows, during the long break.
#[expect(
    clippy::too_many_lines,
    reason = "one surface's layers, painted back to front; splitting it would hide their order"
)]
fn water(s: &Scene<'_>, p: &Pal, sun: [f32; 2], sun_p: f32, sun_vis: f32, glow_k: f32, moon: Option<f32>) -> El {
    let t = s.drift;
    let depth = H - HZ;
    let kind = s.place.water;
    let stops = sea(kind, p);
    let (refl_x, refl) = match moon {
        Some(k) => (0.74, rgb(0xb8c2ea).alpha(0.22 * k)),
        None => (sun[0] / W, p.glow.alpha(0.45 * glow_k)),
    };

    // Everything mirrored or lying on the surface, clipped to the water.
    let mut w = Shape::new();
    match kind {
        Water::Swell => {}
        Water::Mirror => w = s.place.reflection(w, p, 0.8),
        Water::Salt => w = s.place.reflection(w, p, 0.55),
    }
    if moon.is_none() && sun_vis > 0.0 && kind != Water::Swell {
        // A still surface shows the sun broken into soft horizontal streaks, each pushed a
        // little sideways by the water and fainter the further from the horizon.
        let sq = SUN_R * squash(sun_p);
        let m = [sun[0], 2.0 * HZ - sun[1]];
        // Seen from the shore, only a sun low in the sky shows near enough to read.
        let k = if kind == Water::Mirror { 0.5 } else { 0.6 } * ((HZ + 100.0 - m[1]) / 50.0).clamp(0.0, 1.0) * sun_vis;
        let tone = p.sun.mix(p.glow, 0.3);
        for j in 0..14 {
            let f = (j as f32 + 0.5) / 14.0;
            let y = m[1] - sq * 0.95 + f * sq * 1.9;
            let across = (1.0 - (2.0 * f - 1.0).powi(2)).sqrt();
            let half = SUN_R * across * (0.75 + 0.35 * hash(j + 600));
            let shift = (t * 0.5 + j as f32 * 1.9).sin() * 3.0 + (hash(j + 610) - 0.5) * 8.0;
            let fall = 1.0 - ((y - HZ) / (sq * 2.6)).clamp(0.0, 1.0) * 0.7;
            w = w.line(
                [m[0] - half + shift, y],
                [m[0] + half + shift, y],
                sq * 0.07,
                LineCap::Round,
                tone.alpha(k * fall * 0.85),
            );
        }
    }
    let mut b = Shape::new();
    b = b.line([0.0, HZ + 0.5], [W, HZ + 0.5], 1.0, LineCap::Butt, p.low.mix(rgb(0xffffff), 0.3).alpha(0.22));
    let foam = p.low.mix(rgb(0xffffff), 0.4);
    match kind {
        Water::Swell => {
            // Swell lines, denser toward the horizon.
            for k in 0..26u32 {
                let fy = (k as f32 + 0.5) / 26.0;
                let y = HZ + 3.0 + fy.powf(1.8) * depth;
                for j in 0..4u32 {
                    let id = k * 7 + j;
                    let len = (8.0 + 26.0 * fy) * (0.6 + hash(id + 1));
                    let dir = if j % 2 == 0 { 1.0 } else { -0.6 };
                    let sx = (hash(id) * (W + 60.0) + t * (2.0 + 4.0 * fy) * dir).rem_euclid(W + 60.0) - 30.0;
                    b = b.line(
                        [sx, y],
                        [sx + len, y],
                        0.8 + 1.2 * fy,
                        LineCap::Round,
                        foam.alpha((0.05 + 0.06 * fy) * calm(sx, y)),
                    );
                }
            }
        }
        Water::Mirror => {
            // A few long, slow ripples on still water.
            for k in 0..7u32 {
                let fy = (k as f32 + 0.5) / 7.0;
                let y = HZ + 6.0 + fy.powf(1.5) * depth * 0.9;
                let len = 60.0 + 120.0 * hash(k + 40);
                let sx = (hash(k + 41) * (W + len) + t * 1.2).rem_euclid(W + len) - len;
                b = b.line([sx, y], [sx + len, y], 0.7, LineCap::Round, foam.alpha(0.07 * calm(sx + len / 2.0, y)));
            }
        }
        Water::Salt => {
            // Salt crust in the foreground: faint polygons where the water is thinnest.
            for k in 0..10u32 {
                let c = [hash(k + 60) * W, HZ + 80.0 + hash(k + 61) * (depth - 80.0)];
                let r = 16.0 + 20.0 * hash(k + 62);
                let turn = hash(k + 63) * TAU;
                // Uneven sides, as a crust cracks, and only part of each outline shows.
                let cell: Vec<[f32; 2]> = (0..5)
                    .map(|j| {
                        let a = turn + j as f32 * TAU / 6.0 + (hash(k * 9 + j) - 0.5) * 0.6;
                        let r = r * (0.75 + 0.5 * hash(k * 9 + j + 100));
                        [c[0] + r * a.cos(), c[1] + r * 0.28 * a.sin()]
                    })
                    .collect();
                b = b.stroke(cell, 0.7, foam.alpha(0.035 * calm(c[0], c[1])));
            }
            // Heat shimmer: the air over the flat wavers along the horizon while the sun is up.
            let heat = (1.0 - x_of(s) / 1.05).clamp(0.0, 1.0);
            for k in 0..9u32 {
                let y = HZ - 3.0 + k as f32 * 1.4;
                let len = 24.0 + 40.0 * hash(k + 80);
                let sx = hash(k + 81) * (W - len) + (t * 2.1 + k as f32 * 1.7).sin() * 3.0;
                b = b.line(
                    [sx, y],
                    [sx + len, y],
                    1.1,
                    LineCap::Round,
                    p.low.mix(rgb(0xffffff), 0.35).alpha(0.32 * heat),
                );
            }
        }
    }
    // The glitter path: it lengthens and narrows as the sun lowers. Still water keeps it short.
    if moon.is_none() && sun_vis > 0.0 {
        let reach = if kind == Water::Swell { 0.35 + 0.65 * sun_p.min(1.0) } else { 0.25 + 0.3 * sun_p.min(1.0) };
        let width = s.place.glitter * SUN_SCALE.sqrt() * (1.0 - 0.3 * sun_p.min(1.0));
        b = glitter(b, sun[0], depth * reach, width, p.sun.mix(rgb(0xffffff), 0.25), sun_vis, t);
    }
    if let Some(k) = moon {
        b = glitter(b, W * 0.74, depth * 0.85, s.place.glitter * 0.65, rgb(0xdde3fa), 0.55 * k, t);
    }

    // The lake breathes a low mist over still water; the salt flat lays the sky's glow out in
    // one long flat band.
    let haze = match kind {
        Water::Mirror => {
            let mist = p.low.mix(rgb(0xe8f2f0), 0.55).alpha(0.16 + 0.18 * ((x_of(s) - 0.6) / 0.9).clamp(0.0, 1.0));
            el().key("haze").absolute(0.0, HZ - 26.0).size(W, 64.0).linear_gradient(
                PI,
                [(0.0, mist.alpha(0.0)), (0.42, mist), (0.6, mist.alpha(mist.a * 0.7)), (1.0, mist.alpha(0.0))],
            )
        }
        Water::Salt => {
            let band = p.glow.alpha(0.42 * (1.0 - ((x_of(s) - 1.2) / 0.5).clamp(0.0, 1.0)));
            el().key("haze").absolute(0.0, HZ).size(W, 70.0).radial_gradient(
                [0.5, 0.0],
                [W * 0.75, 26.0],
                [(0.0, band), (0.6, band.alpha(band.a * 0.4)), (1.0, band.alpha(0.0))],
            )
        }
        Water::Swell => el().key("haze"),
    };
    el().key("water")
        .absolute(0.0, 0.0)
        .size(W, H)
        .child(
            el().absolute(0.0, HZ)
                .size(W, depth)
                .clip_rounded(0.0)
                .linear_gradient(PI, stops)
                .child(el().absolute(0.0, 0.0).size(W, depth).radial_gradient(
                    [refl_x, 0.0],
                    [170.0, 150.0],
                    [(0.0, refl), (0.4, refl.alpha(refl.a * 0.35)), (1.0, refl.alpha(0.0))],
                ))
                .child(canvas(w).absolute(0.0, -HZ).size(W, H).blur(1.2)),
        )
        .child(haze)
        .child(canvas(b).absolute(0.0, 0.0).size(W, H))
}

/// The water's colour from the horizon down.
fn sea(kind: Water, p: &Pal) -> [(f32, Color); 3] {
    match kind {
        Water::Swell => [(0.0, p.sea_hi), (0.22, p.sea_hi.mix(p.sea_lo, 0.5)), (1.0, p.sea_lo)],
        Water::Mirror => {
            [(0.0, p.low.mix(p.sea_hi, 0.4)), (0.3, p.mid.mix(p.sea_hi, 0.5)), (1.0, p.zen.mix(p.sea_lo, 0.55))]
        }
        Water::Salt => {
            [(0.0, p.low.mix(p.sea_hi, 0.45)), (0.35, p.mid.mix(p.sea_hi, 0.6)), (1.0, p.sea_lo.mix(p.zen, 0.3))]
        }
    }
}

/// The colour of the water behind the readout, where the time is written.
pub fn ground(s: &Scene<'_>) -> Color {
    let p = s.place.pal(s.sky.axis(s.dark));
    let [_, mid, deep] = sea(s.place.water, &p);
    let g = mid.1.mix(deep.1, 0.55);
    if s.dark { g.mix(rgb(0x04061a), 0.22) } else { g }
}

fn x_of(s: &Scene<'_>) -> f32 {
    s.sky.axis(s.dark)
}

/// Texture fades where the readout sits, so its small text has a quiet ground.
fn calm(x: f32, y: f32) -> f32 {
    if x < CALM[0] && y > CALM[1] { 0.3 } else { 1.0 }
}

/// Blue hour's last warmth: a band along the horizon that thins as the break runs out. `k`
/// brings it in through the bell as the sun's own glow goes.
fn afterglow(place: &Place, progress: f32, k: f32) -> El {
    let warm = place.pal(1.05).glow;
    let h = 26.0 + 74.0 * (1.0 - progress);
    el().key("afterglow")
        .absolute(0.0, HZ - h)
        .size(W, h)
        .linear_gradient(PI, [(0.0, warm.alpha(0.0)), (1.0, warm.alpha((0.5 * (1.0 - progress) + 0.06) * k))])
}

/// A faint grain over the whole scene, finer than a level of colour, so gradients don't band.
fn grain() -> &'static Pixels {
    static GRAIN: OnceLock<Pixels> = OnceLock::new();
    GRAIN.get_or_init(|| {
        let (w, h) = (W as u32 * 2, H as u32 * 2);
        let mut rgba = Vec::with_capacity((w * h * 4) as usize);
        for i in 0..w * h {
            let n = hash(i.wrapping_mul(2654435761) ^ 0x5bd1);
            let v = if n > 0.5 { 255 } else { 0 };
            rgba.extend_from_slice(&[v, v, v, (2.0 + 4.0 * (n - 0.5).abs() * 2.0) as u8]);
        }
        let px = Pixels::new();
        px.set(w, h, rgba);
        px
    })
}

/// How far above the sea, in points, the sun starts for a block of `secs`.
pub fn rise(secs: f64) -> f64 {
    f64::from(RISE) * (secs / 3600.0).max(0.0).sqrt()
}

/// The block length whose sun starts `rise` points above the sea: dragging the sun sets it.
pub fn length_at(rise: f64) -> f64 {
    3600.0 * (rise.max(0.0) / f64::from(RISE)).powi(2)
}

/// Near the horizon the sun flattens a little, as refraction lifts its lower edge.
fn squash(sun_p: f32) -> f32 {
    1.0 - 0.13 * ((sun_p - 0.86) / 0.14).clamp(0.0, 1.0)
}

/// The sun's centre: its lower limb `rise` above the sea at the start, on the water at 1, and
/// its upper limb at the horizon by `UNDER`.
fn sun_y(sun_p: f32, rise: f32) -> f32 {
    let half = SUN_R * squash(sun_p);
    if sun_p <= 1.0 { HZ - half - rise * (1.0 - sun_p) } else { HZ - half + (sun_p - 1.0) / (UNDER - 1.0) * 2.0 * half }
}

/// Clouds: banks of billows on a flat base, lit from whichever side the sun is on: white
/// tops and grey bellies in the afternoon, then cool tops over glowing undersides as the sun
/// goes down. Each is drawn opaque and the layer fades as one, so overlapping billows never
/// stack into darker bars.
fn clouds(p: &Pal, sun_p: f32, dusk: f32, t: f32) -> El {
    // Clouds drift a fraction of a point a second, so they move once a second and the blurred
    // layer is redrawn then, not at every frame of glitter.
    let t = t.floor();
    let mut sh = Shape::new();
    let below = ((sun_p - 0.45) / 0.45).clamp(0.0, 1.0);
    let shade = p.cloud.mix(p.mid, 0.55).mix(p.zen, 0.2);
    let bright = p.cloud.mix(rgb(0xffffff), 0.35 * (1.0 - below)).mix(p.glow, 0.6 * below * (1.0 - dusk));
    let (top, under) = (bright.mix(shade, below), shade.mix(bright, below));
    for k in 0..5u32 {
        let y = HZ - 44.0 - hash(k + 31) * 124.0;
        // Higher banks are nearer, so they're bigger; low ones flatten toward the horizon.
        let near = ((HZ - y) / 168.0).clamp(0.0, 1.0);
        let len = (60.0 + hash(k + 3) * 120.0) * (0.6 + 0.6 * near);
        let thick = (4.0 + hash(k + 5) * 5.0) * (0.45 + 0.85 * near);
        let speed = 0.2 + hash(k) * 0.3;
        let x0 = (hash(k + 11) * (W + len + 40.0) + t * speed).rem_euclid(W + len + 40.0) - len - 20.0;
        let billows = 3 + (len / (thick * 1.9)) as u32;
        let skew = 0.8 + 0.4 * hash(k + 70);
        for pass in 0..2 {
            let (color, drop, grow) = if pass == 0 { (under, 0.42, 1.06) } else { (top, 0.0, 1.0) };
            sh = disc(sh, [x0 + len * 0.5, y - thick * (0.3 - drop * 0.4)], [len * 0.5, thick * 0.4 * grow], color);
            // Billows keep off the ends, which the base tapers to; a few more crown the middle.
            for j in 0..billows + billows / 3 {
                let id = k * 41 + j;
                let crown = j >= billows;
                let f = if crown { 0.3 + 0.4 * hash(id + 9) } else { 0.16 + 0.68 * (j as f32 + 0.5) / billows as f32 };
                let body = (PI * f.powf(skew)).sin().powf(1.4);
                let r = thick * (0.3 + 0.7 * body) * (0.75 + 0.4 * hash(id + 2)) * grow * if crown { 0.8 } else { 1.0 };
                let lift = if crown { thick * 0.55 * body } else { 0.0 };
                let cx = x0 + f * len + (hash(id + 5) - 0.5) * thick;
                sh = disc(sh, [cx, y - r * 0.8 - lift + r * drop], [r * 1.3, r], color);
            }
        }
    }
    canvas(sh).absolute(0.0, 0.0).size(W, H).blur(1.7).opacity(0.72 - 0.36 * dusk).cache()
}

/// The bell's ripple, spreading from where the sun meets the water and scaled to it: two
/// rings for a lantern, one faint ring for a block let go.
fn ripple(sky: Sky, x: f32) -> Shape {
    let mut rip = Shape::new();
    let Sky::Bell { since, kept, .. } = sky else { return rip };
    let rings: &[f32] = if kept { &[0.0, 0.28] } else { &[0.2] };
    for (j, lag) in rings.iter().enumerate() {
        let k = since / 2.4 - lag;
        if !(0.0..1.0).contains(&k) {
            continue;
        }
        let rx = SUN_R * (0.7 + if kept { 3.2 } else { 2.2 } * ease(k));
        let a = if kept { 0.55 } else { 0.3 } * (1.0 - k) * (k * 8.0).min(1.0);
        rip =
            rip.stroke(ring([x, HZ + 2.0], [rx, rx * 0.17], 90, 0.0, PI), 1.4 - j as f32 * 0.4, rgb(0xffe2b8).alpha(a));
    }
    rip
}

fn glitter(mut b: Shape, cx: f32, len: f32, width: f32, tone: Color, k: f32, t: f32) -> Shape {
    const ROWS: u32 = 30;
    for r in 0..ROWS {
        let f = r as f32 / ROWS as f32;
        let y = HZ + 2.5 + f.powf(1.45) * len;
        let half = 3.0 + width * f.powf(0.8);
        let pieces = 1 + (hash(r + 900) * 2.6) as u32;
        for j in 0..pieces {
            let id = r * 5 + j;
            let ph = t * (0.8 + hash(id) * 0.9) + hash(id + 3) * TAU;
            let at = cx + ((hash(id + 7) - 0.5) * 2.0 + ph.sin() * 0.35) * half;
            let l = half * (0.25 + 0.45 * (0.5 + 0.5 * (ph * 1.3).cos())) / pieces as f32 * 1.6;
            let a = k * (0.95 - 0.6 * f) * (0.55 + 0.45 * (ph * 0.7).sin()) * calm(at, y);
            b = b.line([at - l, y], [at + l, y], 0.9 + 1.5 * f, LineCap::Round, tone.alpha(a));
        }
    }
    b
}

/// Where the i-th lantern of a cycle floats, and how large it is there (nearer is larger).
/// All four keep out of the readout's corner and clear of the controls.
fn sea_spot(i: usize) -> ([f32; 2], f32) {
    const SPOTS: [[f32; 2]; 4] = [[0.80, 0.08], [0.93, 0.32], [0.50, 0.0], [0.72, 0.56]];
    let [fx, fy] = SPOTS[i];
    ([W * fx, HZ + 14.0 + fy * (H - HZ - 40.0)], 0.8 + 0.6 * fy)
}

/// Where today's i-th lantern-star sits: each cycle's four make a small kite of their own,
/// the cycles spread high across the sky, clear of the moon's path.
fn star_at(i: usize) -> [f32; 2] {
    const KITE: [[f32; 2]; 4] = [[-26.0, 10.0], [-8.0, -12.0], [17.0, -4.0], [30.0, 16.0]];
    let c = (i / 4) as u32;
    let centre = [
        W * (0.22 + 0.3 * (c % 2) as f32) + 14.0 * (hash(c + 900) - 0.5),
        66.0 + 46.0 * ((c / 2) % 2) as f32 + 10.0 * hash(c + 901),
    ];
    let [dx, dy] = KITE[i % 4];
    [centre[0] + dx, centre[1] + dy]
}

fn lantern_label(i: usize, l: &Lantern) -> String {
    match l.pauses {
        0 => format!("Lantern {}", i + 1),
        1 => format!("Lantern {}, paused once", i + 1),
        2 => format!("Lantern {}, paused twice", i + 1),
        n => format!("Lantern {}, paused {n} times", i + 1),
    }
}

pub fn ease(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Eases out past 1 and settles back, for a lantern that blooms.
pub fn back(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    1.0 + 2.5 * (x - 1.0).powi(3) + 1.5 * (x - 1.0).powi(2)
}
