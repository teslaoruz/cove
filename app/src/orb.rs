//! The desk orb: thousands of tiny white particles on a sphere that a slow field of waves folds like cloth.
//! Drawn in software into one image per frame (Slint updating hundreds of dot elements was the expensive part).
//! The blob turns toward the pointer and bulges under it; dragging spins it (with inertia), a click sends a
//! ripple through it; while listening or speaking, the voice swells and shakes it.
use slint::{Image, Rgba8Pixel, SharedPixelBuffer};
use std::f32::consts::PI;

pub struct Orb {
    home: Vec<[f32; 3]>, // unit-sphere positions
    wander: Vec<[f32; 4]>, // per particle: three phases and a speed, for its own drift
    look: [f32; 2],      // eased pointer offset (-1..1), turns the blob
    touch: [f32; 3],     // eased pointer position (0..1) and strength (0..1)
    level: f32,          // eased voice level
    clock: f32,          // field time; runs faster with the voice
    spin: [f32; 2],      // extra yaw/pitch from dragging
    spin_v: [f32; 2],    // its velocity, so a flick keeps turning and slows down
    prev: Option<(f32, f32)>, // pointer at the last step while the button is down
    burst: [f32; 3],     // click ripple: position (sphere radii) and strength
    pub tint: [f32; 3], // particle colour: the agent in use, mixed with white
}

pub struct Input {
    pub dt: f32,                     // seconds since the last step
    pub pointer: Option<(f32, f32)>, // in the orb's 0..1 square (may lie outside it)
    pub voice: f32,                  // 0..1
    pub down: bool,                  // button held on the orb
}

fn ease(v: &mut f32, to: f32, rate: f32, dt: f32) -> bool {
    let d = (to - *v) * (1.0 - (-dt * rate).exp());
    *v += d;
    d.abs() > 1e-4
}

impl Orb {
    pub fn new(n: usize) -> Orb {
        let ga = PI * (3.0 - 5f32.sqrt()); // Fibonacci lattice: even spacing
        let home = (0..n)
            .map(|i| {
                let y = 1.0 - 2.0 * (i as f32 + 0.5) / n as f32;
                let r = (1.0 - y * y).sqrt();
                let th = ga * i as f32;
                [r * th.cos(), y, r * th.sin()]
            })
            .collect();
        // cheap deterministic randomness (no rand crate): a hashed sine per value
        let rnd = |i: usize, k: f32| ((i as f32 * 12.9898 + k * 78.233).sin() * 43758.547).fract().abs();
        let wander = (0..n).map(|i| [rnd(i, 1.0) * 6.283, rnd(i, 2.0) * 6.283, rnd(i, 3.0) * 6.283, 0.5 + rnd(i, 4.0)]).collect();
        Orb { home, wander, look: [0.0; 2], touch: [0.5, 0.5, 0.0], level: 0.0, clock: 0.0, spin: [0.0; 2], spin_v: [0.0; 2], prev: None, burst: [0.0; 3], tint: [1.0; 3] }
    }

    /// Advance time and the eased inputs. Returns true while something is still changing beyond the drift.
    pub fn step(&mut self, inp: &Input) -> bool {
        let dt = inp.dt.min(0.1);
        let mut busy = ease(&mut self.level, inp.voice, 14.0, dt) || self.level > 0.01;
        self.clock += dt * (1.0 + self.level * 3.0);
        let (lx, ly, on) = match inp.pointer {
            Some((x, y)) => (((x - 0.5) * 1.6).clamp(-1.0, 1.0), ((y - 0.5) * 1.6).clamp(-1.0, 1.0), 1.0),
            None => (0.0, 0.0, 0.0),
        };
        busy |= ease(&mut self.look[0], lx, 3.0, dt);
        busy |= ease(&mut self.look[1], ly, 3.0, dt);
        if let Some((x, y)) = inp.pointer {
            ease(&mut self.touch[0], x, 10.0, dt);
            ease(&mut self.touch[1], y, 10.0, dt);
        }
        busy |= ease(&mut self.touch[2], on, 5.0, dt);
        // drag to spin; let go and it coasts
        match (inp.down, inp.pointer, self.prev) {
            (true, Some(p), Some(q)) => {
                let d = [(p.0 - q.0) * 6.0, (p.1 - q.1) * 6.0];
                self.spin[0] += d[0];
                self.spin[1] = (self.spin[1] + d[1]).clamp(-1.2, 1.2);
                self.spin_v = [d[0] / dt.max(1e-3), d[1] / dt.max(1e-3)];
                self.prev = Some(p);
            }
            (true, Some(p), None) => {
                self.prev = Some(p); // pressed: a ripple from the press point
                let rad = 0.30 / 0.5; // the sphere's radius in half-square units
                self.burst = [(p.0 - 0.5) / 0.5 / rad, (p.1 - 0.5) / 0.5 / rad, 1.0];
            }
            (false, ..) => {
                self.prev = None;
                let f = (-dt * 2.2).exp();
                self.spin_v = [self.spin_v[0] * f, self.spin_v[1] * f];
                self.spin[0] += self.spin_v[0] * dt;
                self.spin[1] = (self.spin[1] + self.spin_v[1] * dt).clamp(-1.2, 1.2);
                self.spin[1] *= (-dt * 0.8).exp(); // tilt drifts back level
            }
            _ => {}
        }
        busy |= self.spin_v[0].abs() + self.spin_v[1].abs() > 0.01 || self.spin[1].abs() > 0.01;
        self.burst[2] *= (-dt * 1.6).exp();
        busy |= self.burst[2] > 0.01;
        busy
    }

    /// Render `px`×`px` premultiplied pixels: each particle is a one-pixel additive splat, so folds and
    /// edges, where the surface is seen side-on, gather particles and glow.
    pub fn frame(&self, px: u32) -> Image {
        let mut buf = SharedPixelBuffer::<Rgba8Pixel>::new(px, px);
        let img = buf.make_mut_slice();
        let t = self.clock;
        let c = px as f32 / 2.0;
        let rad = px as f32 * 0.30;
        let (yaw, pitch) = (t * 0.13 + self.look[0] * 0.9 + self.spin[0], 0.25 + self.look[1] * 0.7 + self.spin[1]);
        let (sy, cy) = yaw.sin_cos();
        let (sp, cp) = pitch.sin_cos();
        let lv = self.level;
        let amp = 0.30 + lv * 0.30;
        let swell = 1.0 + lv * 0.16; // the voice makes the whole orb breathe out
        let [bx, by, bs] = self.burst;
        let ring = (1.0 - bs) * 2.6; // the click ripple's front, in sphere radii
        let [tx, ty, ts] = self.touch;
        let (tx, ty) = ((tx - 0.5) * px as f32 / rad, (ty - 0.5) * px as f32 / rad); // pointer in sphere radii
        let k = px as f32 / 420.0; // particle size scales with the image
        let jit = 0.07 + self.level * 0.10; // how far each particle wanders from the surface
        for (i, &[x, y, z]) in self.home.iter().enumerate() {
            let [a, b, cph, spd] = self.wander[i];
            let w = t * 0.9 * spd;
            // each particle drifts on its own slow loop around its spot on the surface
            let (x, y, z) = (x + jit * (w + a).sin(), y + jit * (w * 1.3 + b).sin(), z + jit * (w * 0.7 + cph).cos());
            // a few drifting, crossing waves: smooth folds, cheap to evaluate
            let n = (x * 2.1 + y * 0.9 + t * 0.55).sin() * (y * 1.8 - z * 1.3 + t * 0.41).sin()
                + 0.55 * (z * 2.7 + x * 1.6 - t * 0.67).sin() * (x * 1.2 - y * 2.4 + t * 0.29).cos()
                + 0.25 * (x * 5.1 + y * 4.3 + z * 3.7 + t * 1.3).sin();
            let mut r = (1.0 + amp * n) * swell;
            // rotate: yaw about y, then pitch about x
            let (x1, z1) = (x * cy + z * sy, z * cy - x * sy);
            let (y2, z2) = (y * cp - z1 * sp, y * sp + z1 * cp);
            if ts > 0.01 && z2 > -0.1 {
                // the side facing the viewer bulges toward the pointer
                let d2 = (x1 - tx).powi(2) + (y2 - ty).powi(2);
                r += ts * 0.32 * (-d2 / 0.18).exp();
            }
            if bs > 0.01 && z2 > -0.2 {
                // a ring wave runs out from where you clicked
                let d = ((x1 - bx).powi(2) + (y2 - by).powi(2)).sqrt();
                r += bs * 0.22 * (-(d - ring).powi(2) / 0.05).exp();
            }
            let persp = 1.0 / (1.0 - z2 * r * 0.18); // a little perspective: near side larger
            let sx = c + x1 * r * rad * persp;
            let syy = c + y2 * r * rad * persp;
            let depth = (z2 + 1.0) * 0.5;
            disc(img, px, sx, syy, (0.9 + 1.6 * depth + lv * 0.8) * k, ((0.18 + 0.72 * depth * depth) * (1.0 + lv * 0.4)).min(1.0), self.tint);
        }
        Image::from_rgba8_premultiplied(buf)
    }
}

/// Soft anti-aliased disc in `tint` (1 = white per channel), added onto premultiplied pixels so overlaps glow.
fn disc(img: &mut [Rgba8Pixel], px: u32, cx: f32, cy: f32, r: f32, a: f32, tint: [f32; 3]) {
    let (x0, x1) = ((cx - r - 1.0).floor().max(0.0) as u32, ((cx + r + 1.0).ceil().max(0.0) as u32).min(px));
    let (y0, y1) = ((cy - r - 1.0).floor().max(0.0) as u32, ((cy + r + 1.0).ceil().max(0.0) as u32).min(px));
    for y in y0..y1 {
        for x in x0..x1 {
            let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
            let cov = (r + 0.5 - d).clamp(0.0, 1.0) * a;
            if cov > 0.0 {
                let p = &mut img[(y * px + x) as usize];
                let v = cov * 255.0;
                let add = |c: u8, t: f32| (c as u16 + (v * t) as u16).min(255) as u8;
                (p.r, p.g, p.b, p.a) = (add(p.r, tint[0]), add(p.g, tint[1]), add(p.b, tint[2]), add(p.a, 1.0));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pointer_wakes_it_and_it_settles_without_one() {
        let mut o = Orb::new(2400);
        let mut inp = Input { dt: 1.0 / 60.0, pointer: Some((0.8, 0.3)), voice: 0.0, down: true };
        assert!(o.step(&inp));
        inp.pointer = None;
        inp.down = false;
        assert!((0..2000).any(|_| !o.step(&inp)), "never settled");
        let img = o.frame(390).to_rgba8_premultiplied().unwrap();
        let lit = img.as_slice().iter().filter(|p| p.a > 0).count();
        assert!(lit > 5_000, "{lit} pixels lit");
        assert_eq!(img.as_slice()[0].a, 0);
    }
}

#[cfg(test)]
mod bench {
    #[test]
    #[ignore]
    fn frame_cost() {
        let mut o = super::Orb::new(2400);
        let t0 = std::time::Instant::now();
        for _ in 0..120 {
            o.step(&super::Input { dt: 1.0 / 60.0, pointer: Some((0.6, 0.4)), voice: 0.0, down: false });
            let _ = o.frame(420);
        }
        eprintln!("orb step+frame: {:.2} ms", t0.elapsed().as_secs_f64() * 1000.0 / 120.0);
    }
}
