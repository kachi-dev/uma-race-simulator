//! The order-in/out flags behind `order_rate_in{N}_continue` / `order_rate_out{N}_continue`: set at the start,
//! cleared for good once the horse leaves that part of the field.

use crate::sim::race::Race;
use crate::sim::skill::trigger::rescale_rate;

/// Number of order-in / order-out condition types (10% ..= 90%).
pub const ORDER_IN_OUT_TYPE_COUNT: usize = 9;

/// Order-in/out thresholds in %.
const ORDER_IN_OUT_PER: [i32; ORDER_IN_OUT_TYPE_COUNT] = [10, 20, 30, 40, 50, 60, 70, 80, 90];

/// `rescale_rate(ORDER_IN_OUT_PER[k], n)` for n < 32, precomputed (the order-in/out updates run per horse per frame).
/// Same f32 ops at compile time; the values are small, so `as i32` equals `f32_to_i32`.
const ORDER_IN_OUT_TH: [[i32; ORDER_IN_OUT_TYPE_COUNT]; 32] = {
    let mut t = [[0; ORDER_IN_OUT_TYPE_COUNT]; 32];
    let mut n = 0;
    while n < 32 {
        let mut k = 0;
        while k < ORDER_IN_OUT_TYPE_COUNT {
            t[n][k] = (ORDER_IN_OUT_PER[k] as f32 / 100.0 * n as f32 + 0.5) as i32;
            k += 1;
        }
        n += 1;
    }
    t
};

fn order_in_out_th(k: usize, n: i32) -> i32 {
    match ORDER_IN_OUT_TH.get(n as usize) {
        Some(t) if n >= 0 => t[k],
        _ => rescale_rate(ORDER_IN_OUT_PER[k], n),
    }
}

/// Clears each order-in flag once the horse falls behind that threshold (from 5 s into the race).
pub fn update_order_in_continue(race: &mut Race, i: usize) {
    if race.accumulate_time_since_start < 5.0 {
        return;
    }
    let n = race.info.num_race_horses;
    let h = &mut race.horses[i];
    for k in 0..ORDER_IN_OUT_PER.len() {
        if h.order_in_continue[k] && order_in_out_th(k, n) < h.cur_order + 1 {
            h.order_in_continue[k] = false;
        }
    }
}

/// Clears each order-out flag once the horse gets ahead of that threshold (from 5 s into the race).
pub fn update_order_out_continue(race: &mut Race, i: usize) {
    if race.accumulate_time_since_start < 5.0 {
        return;
    }
    let n = race.info.num_race_horses;
    let h = &mut race.horses[i];
    for k in 0..ORDER_IN_OUT_PER.len() {
        if h.order_out_continue[k] && h.cur_order + 1 < order_in_out_th(k, n) {
            h.order_out_continue[k] = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_in_out_table() {
        for n in -1..40 {
            for k in 0..ORDER_IN_OUT_TYPE_COUNT {
                assert_eq!(order_in_out_th(k, n), rescale_rate(ORDER_IN_OUT_PER[k], n), "n {n} k {k}");
            }
        }
    }
}
