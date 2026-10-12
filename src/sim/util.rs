//! .NET / Unity math helpers with the game's exact semantics (incl. NaN behaviour).
//! Use these everywhere instead of `f32::clamp`/`f32::max`/`as i32` (those differ on NaN / out of range).
//!
//! # Float semantics
//! The game evaluates float expressions in double and rounds to f32 on stores, arguments and returns: intermediates
//! of one expression stay f64, and a value is rounded to f32 only when it is stored (field, array element,
//! Vector3/Quaternion component, local) or passed/returned as a `float`. A single f32 op on f32 operands is
//! unaffected (double rounding of +,-,*,/,sqrt is exact), so only multi-op expressions need `as f64`; each such site
//! says which parts are evaluated in f64.

/// `Mathf.Epsilon` = `float.Epsilon` (the smallest denormal; no flush-to-zero).
pub const MATHF_EPSILON: f32 = 1.401298e-45;

/// `Mathf.Pow(f, p)` = `(float)Math.Pow(f, p)`: the f64 pow, rounded once.
#[inline]
pub fn mathf_pow(f: f32, p: f32) -> f32 {
    (f as f64).powf(p as f64) as f32
}

/// `Mathf.Log10(f)` = `(float)Math.Log10(f)`: the f64 log10, rounded once.
#[inline]
pub fn mathf_log10(f: f32) -> f32 {
    (f as f64).log10() as f32
}

/// `Mathf.Max(float a, float b)` = `a > b ? a : b` (NaN in `a` → `b`).
#[inline]
pub fn max(a: f32, b: f32) -> f32 {
    if a > b { a } else { b }
}

/// `Mathf.Min(float a, float b)` = `a < b ? a : b`.
#[inline]
pub fn min(a: f32, b: f32) -> f32 {
    if a < b { a } else { b }
}

/// `Mathf.Approximately(a, b)` = `Abs(b - a) < Max(1e-6f * Max(Abs(a), Abs(b)), Epsilon * 8f)`.
/// NaN anywhere → false.
#[inline]
pub fn approximately(a: f32, b: f32) -> bool {
    (b - a).abs() < max(1e-6f32 * max(a.abs(), b.abs()), MATHF_EPSILON * 8.0)
}

/// `Mathf.Clamp(v, lo, hi)`: `if (v < lo) v = lo; else if (v > hi) v = hi;` (NaN passes through).
/// Also the hand-written clamp pattern used all over the game's race code.
#[inline]
pub fn clamp(v: f32, lo: f32, hi: f32) -> f32 {
    if v < lo {
        lo
    } else if v > hi {
        hi
    } else {
        v
    }
}

/// `Mathf.Clamp01(v)`.
#[inline]
pub fn clamp01(v: f32) -> f32 {
    if v < 0.0 {
        0.0
    } else if v > 1.0 {
        1.0
    } else {
        v
    }
}

/// `Mathf.Lerp(a, b, t)` = `a + (b - a) * Clamp01(t)`.
#[inline]
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * clamp01(t)
}

/// `(int)f`: truncation; NaN / out of range → `int.MinValue` (Rust `as` saturates instead).
#[inline]
pub fn f32_to_i32(f: f32) -> i32 {
    if f.is_nan() || f >= 2147483648.0 || f < -2147483648.0 { i32::MIN } else { f as i32 }
}

/// `(int)d` for doubles (same NaN / range behaviour as [`f32_to_i32`]).
#[inline]
pub fn f64_to_i32(d: f64) -> i32 {
    if d.is_nan() || d >= 2147483648.0 || d < -2147483648.0 { i32::MIN } else { d as i32 }
}

/// `Mathf.FloorToInt(f)` = `(int)Math.Floor((double)f)`.
#[inline]
pub fn floor_to_int(f: f32) -> i32 {
    f64_to_i32((f as f64).floor())
}

/// `List<T>.Sort(Comparison)` / `Array.Sort(T[], Comparison)`: the unstable .NET introsort.
/// `cmp` returns <0, 0, >0. Ties are permuted exactly like .NET (matters for n >= 17
/// and for non-transitive comparators).
pub fn intro_sort<T: Clone, F: FnMut(&T, &T) -> i32>(keys: &mut [T], mut cmp: F) {
    let length = keys.len();
    if length < 2 {
        return;
    }
    let mut depth = 0i32;
    let mut n = length;
    loop {
        n >>= 1;
        depth += 1;
        if n < 1 {
            break;
        }
    }
    intro_sort_rec(keys, 0, length as isize - 1, 2 * depth, &mut cmp);
}

fn intro_sort_rec<T: Clone, F: FnMut(&T, &T) -> i32>(keys: &mut [T], lo: isize, mut hi: isize, mut depth_limit: i32, cmp: &mut F) {
    while hi > lo {
        let size = hi - lo + 1;
        if size <= 16 {
            if size == 1 {
                return;
            }
            if size == 2 {
                swap_if_greater(keys, lo, hi, cmp);
                return;
            }
            if size == 3 {
                swap_if_greater(keys, lo, hi - 1, cmp);
                swap_if_greater(keys, lo, hi, cmp);
                swap_if_greater(keys, hi - 1, hi, cmp);
                return;
            }
            insertion_sort(keys, lo, hi, cmp);
            return;
        }
        if depth_limit == 0 {
            heapsort(keys, lo, hi, cmp);
            return;
        }
        depth_limit -= 1;
        let p = pick_pivot_and_partition(keys, lo, hi, cmp);
        intro_sort_rec(keys, p + 1, hi, depth_limit, cmp);
        hi = p - 1;
    }
}

fn swap_if_greater<T, F: FnMut(&T, &T) -> i32>(keys: &mut [T], i: isize, j: isize, cmp: &mut F) {
    if i != j && cmp(&keys[i as usize], &keys[j as usize]) > 0 {
        keys.swap(i as usize, j as usize);
    }
}

fn pick_pivot_and_partition<T: Clone, F: FnMut(&T, &T) -> i32>(keys: &mut [T], lo: isize, hi: isize, cmp: &mut F) -> isize {
    let mid = lo + (hi - lo) / 2;
    swap_if_greater(keys, lo, mid, cmp);
    swap_if_greater(keys, lo, hi, cmp);
    swap_if_greater(keys, mid, hi, cmp);
    let pivot = keys[mid as usize].clone();
    if mid != hi - 1 {
        keys.swap(mid as usize, (hi - 1) as usize);
    }
    let mut left = lo;
    let mut right = hi - 1;
    while left < right {
        // No bounds guards in the scans (like .NET), only the array bounds check.
        loop {
            left += 1;
            if cmp(&keys[left as usize], &pivot) >= 0 {
                break;
            }
        }
        loop {
            right -= 1;
            if cmp(&pivot, &keys[right as usize]) >= 0 {
                break;
            }
        }
        if left >= right {
            break;
        }
        keys.swap(left as usize, right as usize);
    }
    if left != hi - 1 {
        keys.swap(left as usize, (hi - 1) as usize);
    }
    left
}

fn insertion_sort<T: Clone, F: FnMut(&T, &T) -> i32>(keys: &mut [T], lo: isize, hi: isize, cmp: &mut F) {
    let mut i = lo;
    while i < hi {
        let t = keys[(i + 1) as usize].clone();
        let mut j = i;
        while j >= lo && cmp(&t, &keys[j as usize]) < 0 {
            keys[(j + 1) as usize] = keys[j as usize].clone();
            j -= 1;
        }
        keys[(j + 1) as usize] = t;
        i += 1;
    }
}

/// The .NET introsort's heapsort fallback (from the .NET reference source, not checked against the game);
/// unreachable for n <= 18 with these partitions.
fn heapsort<T: Clone, F: FnMut(&T, &T) -> i32>(keys: &mut [T], lo: isize, hi: isize, cmp: &mut F) {
    let n = hi - lo + 1;
    let mut i = n / 2;
    while i >= 1 {
        down_heap(keys, i, n, lo, cmp);
        i -= 1;
    }
    let mut i = n;
    while i > 1 {
        keys.swap(lo as usize, (lo + i - 1) as usize);
        down_heap(keys, 1, i - 1, lo, cmp);
        i -= 1;
    }
}

fn down_heap<T: Clone, F: FnMut(&T, &T) -> i32>(keys: &mut [T], mut i: isize, n: isize, lo: isize, cmp: &mut F) {
    let d = keys[(lo + i - 1) as usize].clone();
    while i <= n / 2 {
        let mut child = 2 * i;
        if child < n && cmp(&keys[(lo + child - 1) as usize], &keys[(lo + child) as usize]) < 0 {
            child += 1;
        }
        if cmp(&d, &keys[(lo + child - 1) as usize]) >= 0 {
            break;
        }
        keys[(lo + i - 1) as usize] = keys[(lo + child - 1) as usize].clone();
        i = child;
    }
    keys[(lo + i - 1) as usize] = d;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mathf_semantics() {
        assert!(approximately(1.0, 1.0 + 1e-7));
        assert!(!approximately(1.0, 1.0001));
        assert!(approximately(0.0, 0.0));
        assert!(!approximately(0.0, 1e-40)); // 1e-40 > Epsilon*8
        assert!(!approximately(f32::NAN, 1.0));
        assert!(clamp(f32::NAN, 0.0, 1.0).is_nan());
        assert_eq!(clamp(-1.0, 0.0, 1.0), 0.0);
        assert_eq!(max(f32::NAN, 2.0), 2.0);
        assert!(max(2.0, f32::NAN).is_nan());
        assert_eq!(lerp(1.0, 3.0, 2.0), 3.0);
        assert_eq!(f32_to_i32(-1.9), -1);
        assert_eq!(f32_to_i32(f32::NAN), i32::MIN);
        assert_eq!(f32_to_i32(3e9), i32::MIN);
        assert_eq!(floor_to_int(-0.5), -1);
    }

    #[test]
    fn intro_sort_sorts() {
        let mut x: u32 = 12345;
        for len in [0usize, 1, 2, 3, 5, 16, 17, 18, 40, 200] {
            let mut v: Vec<i32> = (0..len)
                .map(|_| {
                    x = x.wrapping_mul(1103515245).wrapping_add(12345);
                    ((x >> 16) % 50) as i32
                })
                .collect();
            let mut e = v.clone();
            e.sort();
            intro_sort(&mut v, |a, b| a.cmp(b) as i32);
            assert_eq!(v, e, "len {len}");
        }
        let mut v: Vec<i32> = (0..100).rev().collect();
        heapsort(&mut v, 0, 99, &mut |a: &i32, b: &i32| a.cmp(b) as i32);
        assert_eq!(v, (0..100).collect::<Vec<_>>());
    }

    #[test]
    fn intro_sort_tie_permutation() {
        // All keys equal: <= 16 keeps order; 17 is permuted by one partition pass (hand-traced).
        let mut v: Vec<usize> = (0..16).collect();
        intro_sort(&mut v, |_, _| 0);
        assert_eq!(v, (0..16).collect::<Vec<_>>());
        let mut v: Vec<usize> = (0..17).collect();
        intro_sort(&mut v, |_, _| 0);
        assert_eq!(v, [0, 14, 13, 12, 11, 10, 9, 15, 8, 6, 5, 4, 3, 2, 1, 7, 16]);
    }
}
