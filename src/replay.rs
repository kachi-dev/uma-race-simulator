//! Server replays: decoding, quantizing sim output to replay precision, and diffing the two.

use std::fmt;
use std::io::Read;

use base64::Engine;

use crate::api::{Event, HorseFrame, HorseResult, SimOutput};

/// Decoded server replay (header version 100000002).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Replay {
    pub version: i32,
    pub distance_diff_max: f32,
    pub horse_num: usize,
    pub frames: Vec<ReplayFrame>,
    /// Indexed by horse index.
    pub results: Vec<ReplayResult>,
    pub events: Vec<Event>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReplayFrame {
    pub time: f32,
    pub horses: Vec<ReplayHorseFrame>,
}

/// One horse in one frame, 12 bytes.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ReplayHorseFrame {
    pub distance: f32,
    /// `(ushort)(int)(lane * 10000f)`
    pub lane_position: u16,
    /// `(ushort)(int)(lastSpeed * 100f)`
    pub speed: u16,
    /// `(ushort)(int)hp`
    pub hp: u16,
    pub temptation_mode: i8,
    pub block_front_horse_index: i8,
}

/// One horse's result record, 31 bytes:
/// `i32 FinishOrder @0, f32 FinishTime(scaled) @4, f32 FinishDiffTime @8, f32 StartDelayTime @12,
///  u8 GutsOrder @16, u8 WizOrder @17, f32 LastSpurtStartDistance @18, u8 RunningStyle @22,
///  i32 Defeat @23, f32 FinishTimeRaw @27`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ReplayResult {
    /// 0-based (0 = winner).
    pub finish_order: i32,
    pub finish_time_scaled: f32,
    pub finish_diff_time: f32,
    pub start_delay_time: f32,
    pub guts_order: u8,
    pub wiz_order: u8,
    pub last_spurt_start_distance: f32,
    pub running_style: u8,
    /// Defeat reason (1 Win, 2 Lose, 5 GutsOrder, 9 PassiveSkillNum, 13 ProperGround, ...).
    pub defeat: i32,
    pub finish_time_raw: f32,
}

/// Little-endian cursor over a byte slice.
struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Reader { buf, pos: 0 }
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let s = self.buf.get(self.pos..self.pos + n).ok_or_else(|| format!("replay truncated at {}", self.pos))?;
        self.pos += n;
        Ok(s)
    }
    fn i32(&mut self) -> Result<i32, String> {
        Ok(i32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn f32(&mut self) -> Result<f32, String> {
        Ok(f32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn i16(&mut self) -> Result<i16, String> {
        Ok(i16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u16(&mut self) -> Result<u16, String> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    fn len(&mut self) -> Result<usize, String> {
        let n = self.i32()?;
        usize::try_from(n).map_err(|_| format!("negative length {n}"))
    }
    /// An i32-length-prefixed block.
    fn block(&mut self) -> Result<&'a [u8], String> {
        let n = self.len()?;
        self.take(n)
    }
}

/// `simDataBase64` → base64 → gzip (or zlib) → replay.
pub fn decode_base64(b64: &str) -> Result<Replay, String> {
    let raw = base64::engine::general_purpose::STANDARD.decode(b64.trim()).map_err(|e| e.to_string())?;
    let mut data = Vec::new();
    let res = if raw.starts_with(&[0x1f, 0x8b]) {
        flate2::read::GzDecoder::new(&raw[..]).read_to_end(&mut data)
    } else {
        flate2::read::ZlibDecoder::new(&raw[..]).read_to_end(&mut data)
    };
    res.map_err(|e| format!("decompress: {e}"))?;
    decode(&data)
}

/// Decodes an uncompressed replay (format 2020-03-03 or newer).
pub fn decode(data: &[u8]) -> Result<Replay, String> {
    let mut r = Reader::new(data);
    let header = r.block()?;
    let version = i32::from_le_bytes(header.get(..4).ok_or("short header")?.try_into().unwrap());
    let distance_diff_max = r.f32()?;
    let horse_num = r.len()?;
    let horse_frame_size = r.len()?;
    let horse_result_size = r.len()?;
    if horse_frame_size < 12 || horse_result_size < 31 {
        return Err(format!("unexpected record sizes {horse_frame_size}/{horse_result_size}"));
    }
    r.block()?;
    let frame_count = r.len()?;
    let frame_size = r.len()?;
    let mut frames = Vec::with_capacity(frame_count);
    for _ in 0..frame_count {
        let mut fr = Reader::new(r.take(frame_size)?);
        let time = fr.f32()?;
        let mut horses = Vec::with_capacity(horse_num);
        for i in 0..horse_num {
            fr.pos = 4 + i * horse_frame_size;
            horses.push(ReplayHorseFrame {
                distance: fr.f32()?,
                lane_position: fr.u16()?,
                speed: fr.u16()?,
                hp: fr.u16()?,
                temptation_mode: fr.u8()? as i8,
                block_front_horse_index: fr.u8()? as i8,
            });
        }
        frames.push(ReplayFrame { time, horses });
    }
    r.block()?;
    let mut results = Vec::with_capacity(horse_num);
    for _ in 0..horse_num {
        let mut h = Reader::new(r.take(horse_result_size)?);
        results.push(ReplayResult {
            finish_order: h.i32()?,
            finish_time_scaled: h.f32()?,
            finish_diff_time: h.f32()?,
            start_delay_time: h.f32()?,
            guts_order: h.u8()?,
            wiz_order: h.u8()?,
            last_spurt_start_distance: h.f32()?,
            running_style: h.u8()?,
            defeat: h.i32()?,
            finish_time_raw: h.f32()?,
        });
    }
    r.block()?;
    let event_count = r.len()?;
    let mut events = Vec::with_capacity(event_count);
    for _ in 0..event_count {
        let event_size = r.i16()? as usize;
        let mut e = Reader::new(r.take(event_size)?);
        let time = e.f32()?;
        let kind = e.u8()?;
        let param_count = e.u8()?;
        let params = (0..param_count).map(|_| e.i32()).collect::<Result<_, _>>()?;
        events.push(Event { time, kind, params });
    }
    Ok(Replay { version, distance_diff_max, horse_num, frames, results, events })
}

/// `(int)f` as x86 does it: truncation; NaN / out of range → `i32::MIN`.
pub fn cvtt(f: f32) -> i32 {
    if f.is_nan() || f >= 2147483648.0 || f < -2147483648.0 { i32::MIN } else { f as i32 }
}

/// One horse frame at replay precision. The `* 10000` / `* 100` products are evaluated in f64 before the
/// truncation (an f32 product rounds 16.8299997f * 100 up to 1683; replays say 1682).
pub fn quantize_frame(h: &HorseFrame) -> ReplayHorseFrame {
    let cvtt64 = |d: f64| if d.is_nan() || d >= 2147483648.0 || d < -2147483648.0 { i32::MIN } else { d as i32 };
    ReplayHorseFrame {
        distance: h.distance,
        lane_position: cvtt64(h.lane_position as f64 * 10000.0) as u16,
        speed: cvtt64(h.speed as f64 * 100.0) as u16,
        hp: cvtt(h.hp) as u16,
        temptation_mode: h.temptation_mode,
        block_front_horse_index: h.block_front_horse_index,
    }
}

pub fn quantize_result(r: &HorseResult) -> ReplayResult {
    ReplayResult {
        finish_order: r.finish_order,
        finish_time_scaled: r.finish_time_scaled,
        finish_diff_time: r.finish_diff_time,
        start_delay_time: r.start_delay_time,
        guts_order: r.guts_order as u8,
        wiz_order: r.wiz_order as u8,
        last_spurt_start_distance: r.last_spurt_start_distance,
        running_style: r.running_style as u8,
        defeat: r.defeat,
        finish_time_raw: r.finish_time_raw,
    }
}

/// Sim output at replay precision.
/// `version` / `distance_diff_max` are not part of `SimOutput` and stay 0.
pub fn quantize(sim: &SimOutput) -> Replay {
    Replay {
        version: 0,
        distance_diff_max: 0.0,
        horse_num: sim.frames.first().map_or(sim.results.len(), |f| f.horses.len()),
        frames: sim
            .frames
            .iter()
            .map(|f| ReplayFrame { time: f.time, horses: f.horses.iter().map(quantize_frame).collect() })
            .collect(),
        results: sim.results.iter().map(quantize_result).collect(),
        events: sim.events.clone(),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    Frames,
    Results,
    Events,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Divergence {
    pub section: Section,
    /// Frame index / event index (unused for results).
    pub index: usize,
    /// Frame or event time (replay's when available).
    pub time: f32,
    pub horse: Option<usize>,
    pub field: &'static str,
    pub sim: String,
    pub replay: String,
}

impl fmt::Display for Divergence {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?} #{} t={}", self.section, self.index, self.time)?;
        if let Some(h) = self.horse {
            write!(f, " horse {h}")?;
        }
        write!(f, " {}: sim {} vs replay {}", self.field, self.sim, self.replay)
    }
}

fn same_f32(a: f32, b: f32) -> bool {
    a == b || (a.is_nan() && b.is_nan())
}

macro_rules! cmp_fields {
    ($mk:expr, $a:expr, $b:expr, f32: [$($f:ident),*], int: [$($i:ident),*]) => {{
        $( if !same_f32($a.$f, $b.$f) { return Some($mk(stringify!($f), format!("{:?}", $a.$f), format!("{:?}", $b.$f))); } )*
        $( if $a.$i != $b.$i { return Some($mk(stringify!($i), $a.$i.to_string(), $b.$i.to_string())); } )*
    }};
}

/// First divergence: frames (time, then horses in index order, fields in record order),
/// then frame count, then per-horse results, then events.
pub fn diff(sim: &SimOutput, replay: &Replay) -> Option<Divergence> {
    diff_quantized(&quantize(sim), replay)
}

pub fn diff_quantized(q: &Replay, replay: &Replay) -> Option<Divergence> {
    let frames = |index, time, horse, field, sim, replay| Divergence {
        section: Section::Frames, index, time, horse, field, sim, replay,
    };
    for (i, (a, b)) in q.frames.iter().zip(&replay.frames).enumerate() {
        if !same_f32(a.time, b.time) {
            return Some(frames(i, b.time, None, "time", format!("{:?}", a.time), format!("{:?}", b.time)));
        }
        if a.horses.len() != b.horses.len() {
            return Some(frames(i, b.time, None, "horse_count", a.horses.len().to_string(), b.horses.len().to_string()));
        }
        for (h, (x, y)) in a.horses.iter().zip(&b.horses).enumerate() {
            let mk = |field, s, r| frames(i, b.time, Some(h), field, s, r);
            cmp_fields!(mk, x, y, f32: [distance],
                int: [lane_position, speed, hp, temptation_mode, block_front_horse_index]);
        }
    }
    if q.frames.len() != replay.frames.len() {
        let n = q.frames.len().min(replay.frames.len());
        let time = replay.frames.get(n).or(q.frames.get(n)).map_or(0.0, |f| f.time);
        return Some(frames(n, time, None, "frame_count", q.frames.len().to_string(), replay.frames.len().to_string()));
    }

    let results = |horse, field, sim, replay| Divergence {
        section: Section::Results, index: 0, time: 0.0, horse: Some(horse), field, sim, replay,
    };
    if q.results.len() != replay.results.len() {
        return Some(Divergence { horse: None, ..results(0, "result_count", q.results.len().to_string(), replay.results.len().to_string()) });
    }
    for (h, (x, y)) in q.results.iter().zip(&replay.results).enumerate() {
        let mk = |field, s, r| results(h, field, s, r);
        cmp_fields!(mk, x, y,
            f32: [start_delay_time, last_spurt_start_distance, finish_time_raw, finish_time_scaled, finish_diff_time],
            int: [finish_order, guts_order, wiz_order, running_style, defeat]);
    }

    for (i, (a, b)) in q.events.iter().zip(&replay.events).enumerate() {
        if a != b {
            return Some(Divergence {
                section: Section::Events, index: i, time: b.time, horse: b.params.first().map(|&h| h as usize),
                field: "event", sim: format!("{a:?}"), replay: format!("{b:?}"),
            });
        }
    }
    if q.events.len() != replay.events.len() {
        let n = q.events.len().min(replay.events.len());
        return Some(Divergence {
            section: Section::Events, index: n, time: replay.events.get(n).map_or(0.0, |e| e.time), horse: None,
            field: "event_count", sim: q.events.len().to_string(), replay: replay.events.len().to_string(),
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantize_truncates() {
        let h = HorseFrame { distance: 1.5, lane_position: 0.123456, speed: 19.999, hp: 1234.9, temptation_mode: 0, block_front_horse_index: -1, ..Default::default() };
        let q = quantize_frame(&h);
        assert_eq!((q.lane_position, q.speed, q.hp), (1234, 1999, 1234));
        assert_eq!(cvtt(f32::NAN), i32::MIN);
    }

    #[test]
    fn roundtrip_self_diff() {
        let sim = SimOutput {
            frames: vec![crate::api::Frame { time: 0.0, horses: vec![HorseFrame::default(); 2] }],
            results: vec![HorseResult::default(); 2],
            events: vec![Event { time: 0.5, kind: 3, params: vec![1, 2] }],
            spans: vec![],
            skill_spans: vec![],
        };
        let mut rep = quantize(&sim);
        assert_eq!(diff(&sim, &rep), None);
        rep.frames[0].horses[1].hp = 5;
        let d = diff(&sim, &rep).unwrap();
        assert_eq!((d.section, d.horse, d.field), (Section::Frames, Some(1), "hp"));
    }
}
