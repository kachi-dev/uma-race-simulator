//! The course's world path and the Unity vector math it needs.
//! Only the world position `W(d, l)` matters: it feeds the per-frame displacement cap and
//! `Race::world_per_metre`, so the math must be bit-exact.

use crate::sim::race::RaceInfo;
use crate::sim::util::{clamp01, f32_to_i32};
use crate::data::course::{lane_path_name, round_lane_path_name};
use crate::data::{CourseLaneAnim, GameData};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vector3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quaternion {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Default for Quaternion {
    fn default() -> Self {
        Quaternion { x: 0.0, y: 0.0, z: 0.0, w: 1.0 }
    }
}

impl Quaternion {
    /// Normalized lerp with `t` clamped to [0, 1], towards `-b` when the dot is negative.
    /// Plain f32 (a Unity native).
    pub fn lerp(a: Quaternion, b: Quaternion, t: f32) -> Quaternion {
        let t = clamp01(t);
        let q = if Self::dot(a, b) < 0.0 {
            Quaternion {
                x: a.x + t * (-b.x - a.x),
                y: a.y + t * (-b.y - a.y),
                z: a.z + t * (-b.z - a.z),
                w: a.w + t * (-b.w - a.w),
            }
        } else {
            Quaternion { x: a.x + t * (b.x - a.x), y: a.y + t * (b.y - a.y), z: a.z + t * (b.z - a.z), w: a.w + t * (b.w - a.w) }
        };
        let m = Self::dot(q, q).sqrt();
        Quaternion { x: q.x / m, y: q.y / m, z: q.z / m, w: q.w / m }
    }

    fn dot(a: Quaternion, b: Quaternion) -> f32 {
        ((a.x * b.x + a.y * b.y) + a.z * b.z) + a.w * b.w
    }

    /// From Euler angles in radians, Unity's ZXY order `(qY * qX) * qZ`. Only ever called with (0, 0, 0) or
    /// (0, PI, 0).
    pub fn euler(v: Vector3) -> Quaternion {
        let (sx, cx) = ((v.x / 2.0).sin(), (v.x / 2.0).cos());
        let (sy, cy) = ((v.y / 2.0).sin(), (v.y / 2.0).cos());
        let (sz, cz) = ((v.z / 2.0).sin(), (v.z / 2.0).cos());
        let qx = Quaternion { x: sx, y: 0.0, z: 0.0, w: cx };
        let qy = Quaternion { x: 0.0, y: sy, z: 0.0, w: cy };
        let qz = Quaternion { x: 0.0, y: 0.0, z: sz, w: cz };
        Self::mul(Self::mul(qy, qx), qz)
    }

    /// The conjugate (the inverse of a unit quaternion).
    pub fn inverse(q: Quaternion) -> Quaternion {
        Quaternion { x: -q.x, y: -q.y, z: -q.z, w: q.w }
    }

    /// Hamilton product `lhs * rhs`, in Unity's term order.
    pub fn mul(lhs: Quaternion, rhs: Quaternion) -> Quaternion {
        // Each component one f64 expression, rounded once.
        let (a, b) = (lhs, rhs);
        let f = |v: f32| v as f64;
        Quaternion {
            x: (((f(a.w) * f(b.x) + f(a.x) * f(b.w)) + f(a.y) * f(b.z)) - f(a.z) * f(b.y)) as f32,
            y: (((f(a.w) * f(b.y) + f(a.y) * f(b.w)) + f(a.z) * f(b.x)) - f(a.x) * f(b.z)) as f32,
            z: (((f(a.w) * f(b.z) + f(a.z) * f(b.w)) + f(a.x) * f(b.y)) - f(a.y) * f(b.x)) as f32,
            w: (((f(a.w) * f(b.w) - f(a.x) * f(b.x)) - f(a.y) * f(b.y)) - f(a.z) * f(b.z)) as f32,
        }
    }

    /// Rotates `p` by `q` (Unity's `rotation * point`).
    pub fn rotate(q: Quaternion, p: Vector3) -> Vector3 {
        // The products are f32 temporaries; each result component is one f64 expression.
        let (x, y, z) = (q.x + q.x, q.y + q.y, q.z + q.z);
        let (xx, yy, zz, xy, xz, yz, wx, wy, wz) = (q.x * x, q.y * y, q.z * z, q.x * y, q.x * z, q.y * z, q.w * x, q.w * y, q.w * z);
        let f = |v: f32| v as f64;
        Vector3 {
            x: (((1.0 - (f(yy) + f(zz))) * f(p.x) + (f(xy) - f(wz)) * f(p.y)) + (f(xz) + f(wy)) * f(p.z)) as f32,
            y: (((f(xy) + f(wz)) * f(p.x) + (1.0 - (f(xx) + f(zz))) * f(p.y)) + (f(yz) - f(wx)) * f(p.z)) as f32,
            z: (((f(xz) - f(wy)) * f(p.x) + (f(yz) + f(wx)) * f(p.y)) + (1.0 - (f(xx) + f(yy))) * f(p.z)) as f32,
        }
    }
}

/// Position and rotation of the lane animation at key time `t` (0..1000). An empty `valueY` means y = 0.
pub fn sample(anim: &CourseLaneAnim, t: f32) -> (Vector3, Quaternion) {
    let k = &anim.key;
    let i = f32_to_i32(t);
    // The fraction is clamped to [0, 1] only for the lerp.
    let frac = t - i as f32;
    let has_y = !k.valueY.is_empty();
    let in_range = i >= 0
        && (i as usize) < k.valueX.len()
        && (i as usize) < k.valueZ.len()
        && (!has_y || (i as usize) < k.valueY.len());
    let y_at = |j: usize| if has_y { k.valueY[j] } else { 0.0 };
    let pos = if !in_range {
        // Zero; unreachable for t in [0, 1000].
        Vector3::default()
    } else if (i as usize) < k.valueX.len() - 1 {
        let (i0, i1) = (i as usize, i as usize + 1);
        let frac = clamp01(frac);
        // `(b - a) * frac + a` in f64, rounded once.
        let l = |a: f32, b: f32| ((b as f64 - a as f64) * frac as f64 + a as f64) as f32;
        Vector3 { x: l(k.valueX[i0], k.valueX[i1]), y: l(y_at(i0), y_at(i1)), z: l(k.valueZ[i0], k.valueZ[i1]) }
    } else {
        let n = k.valueX.len() - 1;
        Vector3 { x: k.valueX[n], y: y_at(n), z: k.valueZ[n] }
    };
    let r = &k.rotation;
    let q = |j: usize| Quaternion { x: r[j].x, y: r[j].y, z: r[j].z, w: r[j].w };
    let rot = if i < r.len() as i32 - 1 { Quaternion::lerp(q(i as usize), q(i as usize + 1), t - i as f32) } else { q(r.len() - 1) };
    (pos, rot)
}

/// The course path: the main lane animation, the optional overrun loop, and the goal frame for
/// extrapolating past both. View-only fields (bank angle) omitted.
#[derive(Clone, Debug, Default)]
pub struct CourseLane {
    pub course_lane_anim: CourseLaneAnim,
    pub course_over_run_lane_anim: Option<CourseLaneAnim>,
    pub goal_pos: Vector3,
    /// Goal heading per metre (from 10 m before the goal).
    pub goal_dir: Vector3,
    pub goal_rot: Quaternion,
    /// Euler angles in degrees.
    pub rotation_fix: Vector3,
    pub rotation_fix_rot: Quaternion,
    /// Never read by the sim.
    pub inv_rotation_fix_rot: Quaternion,
    pub start_position: Vector3,
    pub start_rotation: Quaternion,
}

/// Right-handed courses are turned 180° about y.
const ROTATION_FIX_RIGHT: Vector3 = Vector3 { x: 0.0, y: 180.0, z: 0.0 };
const ROTATION_FIX_LEFT: Vector3 = Vector3 { x: 0.0, y: 0.0, z: 0.0 };

impl CourseLane {
    pub fn init(is_right: bool, lane_anim: CourseLaneAnim, over_run_anim: Option<CourseLaneAnim>, course_distance: i32) -> CourseLane {
        let mut lane = CourseLane::default();
        lane.rotation_fix = if is_right { ROTATION_FIX_RIGHT } else { ROTATION_FIX_LEFT };
        let f = lane.rotation_fix;
        lane.rotation_fix_rot = Quaternion::euler(Vector3 { x: f.x * 0.017453292, y: f.y * 0.017453292, z: f.z * 0.017453292 });
        lane.inv_rotation_fix_rot = Quaternion::inverse(lane.rotation_fix_rot);
        (lane.start_position, lane.start_rotation) = sample(&lane_anim, 0.0);
        lane.course_lane_anim = lane_anim;
        lane.course_over_run_lane_anim = over_run_anim;
        let (p, _) = lane.lane_transform(course_distance as f32 - 10.0);
        (lane.goal_pos, lane.goal_rot) = lane.lane_transform(course_distance as f32);
        lane.goal_dir = Vector3 {
            x: (lane.goal_pos.x - p.x) * 0.1,
            y: (lane.goal_pos.y - p.y) * 0.1,
            z: (lane.goal_pos.z - p.z) * 0.1,
        };
        lane
    }

    /// Lane-0 position and rotation at course distance `distance`. Past the main path it loops the overrun
    /// path (`fmod` of the distance beyond the main path), or extrapolates from the goal when there is none.
    /// The rotation is `sampled * rotation_fix_rot`.
    pub fn lane_transform(&self, distance: f32) -> (Vector3, Quaternion) {
        let anim = &self.course_lane_anim;
        let len = anim.Distance as f32;
        let (sample_anim, t) = if len >= distance {
            // The key time in f64, rounded once. Pairs with the f64 lerp in `sample`.
            (anim, (distance as f64 / len as f64 * 1000.0) as f32)
        } else if let Some(over) = &self.course_over_run_lane_anim {
            let olen = over.Distance as f32;
            // Likewise in f64.
            (over, ((distance as f64 - len as f64) % olen as f64 / olen as f64 * 1000.0) as f32)
        } else {
            let d = distance - len;
            let g = self.goal_pos;
            let gd = self.goal_dir;
            return (Vector3 { x: gd.x * d + g.x, y: gd.y * d + g.y, z: gd.z * d + g.z }, self.goal_rot);
        };
        let (pos, q) = sample(sample_anim, t);
        (pos, Quaternion::mul(q, self.rotation_fix_rot))
    }
}

pub fn load_course_lane_anims(data: &GameData, info: &RaceInfo) -> (CourseLaneAnim, Option<CourseLaneAnim>) {
    let cs = &info.race_course_set;
    let name = lane_path_name(cs.race_track_id, cs.distance, cs.ground, cs.inout, 0);
    let anim = data.course_paths.get(&name).unwrap_or_else(|| panic!("coursepath {name} not loaded")).as_ref().clone();
    let round = data.course_paths.get(&round_lane_path_name(cs.race_track_id, cs.ground, 0)).map(|a| a.as_ref().clone());
    (anim, round)
}

#[derive(Clone, Copy, Debug, Default)]
pub struct WorldTransform {
    pub pos_lane_origin: Vector3,
    pub pos_world: Vector3,
    pub rot: Quaternion,
}

/// World transform at `(distance, lane_distance)`: the lane-0 origin offset sideways by `lane_distance * 11.25`.
pub fn calc_world_transform(lane: &CourseLane, distance: f32, lane_distance: f32) -> WorldTransform {
    let (origin, rot) = lane.lane_transform(distance);
    let r = Quaternion::rotate(rot, Vector3 { x: lane_distance * 11.25, y: 0.0, z: 0.0 });
    WorldTransform {
        pos_lane_origin: origin,
        pos_world: Vector3 { x: r.x + origin.x, y: r.y + origin.y, z: r.z + origin.z },
        rot,
    }
}
