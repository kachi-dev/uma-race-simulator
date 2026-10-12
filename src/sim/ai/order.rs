//! Race order (who is where in the field) and the per-horse order-change counters.

use crate::sim::defs::ORDER_CHANGE_COOLDOWN_SLOTS;
use crate::sim::field::horse_index_at_order;
use crate::sim::physics::last_spurt;
use crate::sim::race::Race;
use crate::sim::util::intro_sort;

/// One entry of the order array: a horse index and its distance at the last sort.
#[derive(Clone, Copy, Debug, Default)]
pub struct CurOrderInfo {
    pub horse: usize,
    pub distance: f32,
}

/// The field in race order. Persistent: each sort starts from last frame's order.
#[derive(Clone, Debug, Default)]
pub struct Standings {
    pub horse_array_by_order: Vec<CurOrderInfo>,
}

impl Standings {
    /// All horses in index order.
    pub fn init(n: usize) -> Self {
        Standings { horse_array_by_order: (0..n).map(|horse| CurOrderInfo { horse, distance: 0.0 }).collect() }
    }

    pub fn horse_at_order(&self, order: i32) -> Option<usize> {
        usize::try_from(order).ok().and_then(|o| self.horse_array_by_order.get(o)).map(|c| c.horse)
    }

    /// The horse index at `order`, 0 when out of range.
    pub fn horse_index_at_order(&self, order: i32) -> i32 {
        self.horse_at_order(order).map_or(0, |h| h as i32)
    }
}

/// Re-sorts the field, then updates every horse's order-change counters.
pub fn calc_horse_order(race: &mut Race, dt: f32) {
    update_horse_order(race);
    for i in 0..race.horses.len() {
        update_order_change_count(race, i, dt);
    }
}

/// Sorts the field by distance (descending, ties by horse index) and sets each horse's `prev_order` / `cur_order`.
pub fn update_horse_order(race: &mut Race) {
    let mut arr = std::mem::take(&mut race.field.standings.horse_array_by_order);
    for c in arr.iter_mut() {
        c.distance = race.horses[c.horse].distance;
    }
    intro_sort(&mut arr, cmp_cur_order);
    for (o, c) in arr.iter().enumerate() {
        let h = &mut race.horses[c.horse];
        h.prev_order = h.cur_order;
        h.cur_order = o as i32;
    }
    race.field.standings.horse_array_by_order = arr;
}

/// Distance descending (NaN last, as `float.CompareTo`), then horse index ascending.
fn cmp_cur_order(x: &CurOrderInfo, y: &CurOrderInfo) -> i32 {
    if x.distance > y.distance {
        return -1;
    }
    if x.distance < y.distance {
        return 1;
    }
    if x.distance != y.distance {
        if !y.distance.is_nan() {
            return 1;
        }
        if !x.distance.is_nan() {
            return -1;
        }
    }
    (x.horse as i32 - y.horse as i32).signum()
}

/// Counts a horse's overtakes (up) or times overtaken (down), by phase and course situation. Each other horse can
/// be counted again only after a cooldown.
#[derive(Clone, Debug, Default)]
pub struct OrderChangeCounter {
    /// Counts overtakes (true) or times overtaken (false).
    pub is_up: bool,
    pub num_race_horse: i32,
    /// Remaining cooldown per other horse, indexed by horse index.
    pub count_target_cool_down: [f32; ORDER_CHANGE_COOLDOWN_SLOTS],
    pub count_cool_down_time: f32,
    pub change_count_phase_start: i32,
    pub change_count_phase_middle: i32,
    pub change_count_phase_end_after: i32,
    pub change_count_corner: i32,
    pub change_count_corner_phase_end_after: i32,
    pub change_count_last_spurt: i32,
    /// Counted from 200 m on.
    pub change_count_distance1: i32,
    pub change_count_final_corner_after: i32,
    pub is_order_change_per_frame: bool,
}

impl OrderChangeCounter {
    pub fn new_up(num_race_horse: i32, count_cool_down_time: f32) -> Self {
        OrderChangeCounter { is_up: true, num_race_horse, count_cool_down_time, ..Default::default() }
    }

    pub fn new_down(num_race_horse: i32, count_cool_down_time: f32) -> Self {
        OrderChangeCounter { is_up: false, num_race_horse, count_cool_down_time, ..Default::default() }
    }
}

/// Updates horse `i`'s up counter, then its down counter.
pub fn update_order_change_count(race: &mut Race, i: usize, dt: f32) {
    order_change_counter_update(race, i, true, dt);
    order_change_counter_update(race, i, false, dt);
}

/// Ticks the counter's cooldowns and counts this frame's order change, if any. A change counts against the one
/// horse now directly behind (up) or ahead (down), when that horse is off cooldown.
pub fn order_change_counter_update(race: &mut Race, i: usize, is_up: bool, dt: f32) {
    if race.horses[i].is_finished {
        return;
    }
    let is_last_spurt = last_spurt::is_last_spurt(race, i);
    let h = &race.horses[i];
    let (cur, prev) = (h.cur_order, h.prev_order);
    let (phase, cur_corner, distance, is_final_corner) = (h.phase as i32, h.cur_corner, h.distance, h.is_final_corner);
    let target = if is_up {
        (prev > cur).then(|| horse_index_at_order(race, cur + 1))
    } else {
        (cur > prev).then(|| horse_index_at_order(race, cur - 1))
    };
    let h = &mut race.horses[i];
    let counter = if is_up { &mut h.order_change_counter_up } else { &mut h.order_change_counter_down };
    for cd in counter.count_target_cool_down.iter_mut() {
        *cd = *cd - dt;
        if 0.0 > *cd {
            *cd = 0.0;
        }
    }
    counter.is_order_change_per_frame = false;
    let Some(target) = target else { return };
    let target = target as usize;
    if !(0.0 >= counter.count_target_cool_down[target]) {
        return;
    }
    counter.count_target_cool_down[target] = counter.count_cool_down_time;
    counter.is_order_change_per_frame = true;
    match phase {
        0 => counter.change_count_phase_start += 1,
        1 => counter.change_count_phase_middle += 1,
        2 | 3 => counter.change_count_phase_end_after += 1,
        _ => {}
    }
    if cur_corner != 0 {
        counter.change_count_corner += 1;
        if phase == 2 || phase == 3 {
            counter.change_count_corner_phase_end_after += 1;
        }
    }
    if is_last_spurt {
        counter.change_count_last_spurt += 1;
    }
    if !(200.0 > distance) {
        counter.change_count_distance1 += 1;
    }
    if is_final_corner {
        counter.change_count_final_corner_after += 1;
    }
    if is_up {
        h.overtake_target_have_no_order_up_continue_time = 0.0;
    } else {
        h.overtake_target_have_no_order_down_continue_time = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_and_change_counter() {
        let mut race = crate::sim::ai::test_race(3);
        for h in race.horses.iter_mut() {
            h.order_change_counter_up = OrderChangeCounter::new_up(3, 6.0);
            h.order_change_counter_down = OrderChangeCounter::new_down(3, 6.0);
        }
        for (i, d) in [10.0, 20.0, 20.0].into_iter().enumerate() {
            race.horses[i].distance = d;
        }
        update_horse_order(&mut race);
        let cur: Vec<i32> = race.horses.iter().map(|h| h.cur_order).collect();
        assert_eq!(cur, [2, 0, 1]); // distance desc, index asc on ties
        // horse 0 overtakes horse 2: counted once, then the 6 s cooldown on horse 2 blocks a recount.
        race.horses[0].distance = 21.0;
        race.horses[0].overtake_target_have_no_order_up_continue_time = 3.0;
        calc_horse_order(&mut race, 0.0666);
        let c = &race.horses[0].order_change_counter_up;
        assert_eq!((race.horses[0].cur_order, c.change_count_phase_start, c.count_target_cool_down[1]), (0, 1, 6.0));
        assert_eq!(race.horses[0].overtake_target_have_no_order_up_continue_time, 0.0);
        assert_eq!(race.horses[1].order_change_counter_down.change_count_phase_start, 1);
        race.horses[0].distance = 19.0;
        calc_horse_order(&mut race, 0.0666);
        race.horses[0].distance = 22.0;
        calc_horse_order(&mut race, 0.0666);
        assert_eq!(race.horses[0].order_change_counter_up.change_count_phase_start, 1);
    }
}
