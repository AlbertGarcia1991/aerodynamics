//! # aeroflow-linalg
//!
//! A small dense linear-algebra kernel for the AeroFlow panel method.
//!
//! The panel method produces one dense, unsymmetric, moderately sized system
//! per solve (`N + M` unknowns for `N` panels across `M` bodies). That calls
//! for LU with partial pivoting — not an iterative method, because the matrix
//! is dense and we also want to *reuse* the factorisation: an angle-of-attack
//! sweep changes only the right-hand side, so `N_α` sweep points cost one
//! `O(n³)` factorisation plus `N_α` `O(n²)` back-substitutions instead of
//! `N_α` factorisations.
//!
//! Everything is `f64` (PRD §70). There are no dependencies, so the crate
//! compiles to `wasm32-unknown-unknown` unchanged.

// Dense numerical kernels read most clearly as explicit index loops over
// matrices and panel arrays; the iterator rewrites clippy suggests obscure them.
#![allow(clippy::needless_range_loop)]

use std::fmt;

/// A dense row-major matrix.
#[derive(Debug, Clone, PartialEq)]
pub struct Matrix {
    rows: usize,
    cols: usize,
    data: Vec<f64>,
}

impl Matrix {
    pub fn zeros(rows: usize, cols: usize) -> Self {
        Self {
            rows,
            cols,
            data: vec![0.0; rows * cols],
        }
    }

    pub fn identity(n: usize) -> Self {
        let mut m = Self::zeros(n, n);
        for i in 0..n {
            m[(i, i)] = 1.0;
        }
        m
    }

    pub fn from_rows(rows: &[Vec<f64>]) -> Self {
        let r = rows.len();
        let c = rows.first().map_or(0, |v| v.len());
        let mut m = Self::zeros(r, c);
        for (i, row) in rows.iter().enumerate() {
            for (j, v) in row.iter().enumerate() {
                m[(i, j)] = *v;
            }
        }
        m
    }

    #[inline]
    pub fn rows(&self) -> usize {
        self.rows
    }

    #[inline]
    pub fn cols(&self) -> usize {
        self.cols
    }

    #[inline]
    pub fn as_slice(&self) -> &[f64] {
        &self.data
    }

    #[inline]
    pub fn row(&self, i: usize) -> &[f64] {
        &self.data[i * self.cols..(i + 1) * self.cols]
    }

    #[inline]
    pub fn row_mut(&mut self, i: usize) -> &mut [f64] {
        &mut self.data[i * self.cols..(i + 1) * self.cols]
    }

    /// Matrix–vector product `A·x`.
    pub fn mul_vec(&self, x: &[f64]) -> Vec<f64> {
        assert_eq!(x.len(), self.cols, "dimension mismatch in mul_vec");
        (0..self.rows)
            .map(|i| {
                let r = self.row(i);
                let mut s = 0.0;
                for j in 0..self.cols {
                    s += r[j] * x[j];
                }
                s
            })
            .collect()
    }

    /// Maximum absolute column sum — the induced 1-norm.
    pub fn norm_1(&self) -> f64 {
        (0..self.cols)
            .map(|j| (0..self.rows).map(|i| self[(i, j)].abs()).sum::<f64>())
            .fold(0.0, f64::max)
    }

    /// Maximum absolute row sum — the induced ∞-norm.
    pub fn norm_inf(&self) -> f64 {
        (0..self.rows)
            .map(|i| self.row(i).iter().map(|v| v.abs()).sum::<f64>())
            .fold(0.0, f64::max)
    }

    pub fn is_finite(&self) -> bool {
        self.data.iter().all(|v| v.is_finite())
    }
}

impl std::ops::Index<(usize, usize)> for Matrix {
    type Output = f64;
    #[inline]
    fn index(&self, (i, j): (usize, usize)) -> &f64 {
        debug_assert!(i < self.rows && j < self.cols);
        &self.data[i * self.cols + j]
    }
}

impl std::ops::IndexMut<(usize, usize)> for Matrix {
    #[inline]
    fn index_mut(&mut self, (i, j): (usize, usize)) -> &mut f64 {
        debug_assert!(i < self.rows && j < self.cols);
        &mut self.data[i * self.cols + j]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinalgError {
    NotSquare { rows: usize, cols: usize },
    Singular { pivot_index: usize },
    NonFinite,
    DimensionMismatch { expected: usize, found: usize },
}

impl fmt::Display for LinalgError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LinalgError::NotSquare { rows, cols } => {
                write!(f, "The linear system is {rows}×{cols}; it must be square.")
            }
            LinalgError::Singular { pivot_index } => write!(
                f,
                "The linear system is singular at pivot {pivot_index}. The panel geometry is probably degenerate — check for zero-length or duplicated panels."
            ),
            LinalgError::NonFinite => write!(
                f,
                "The influence matrix contains non-finite values. Check the geometry for coincident points."
            ),
            LinalgError::DimensionMismatch { expected, found } => {
                write!(f, "Expected a vector of length {expected}, found {found}.")
            }
        }
    }
}

impl std::error::Error for LinalgError {}

/// An LU factorisation with partial pivoting: `P·A = L·U`.
///
/// `L` and `U` share one array (`L` has an implicit unit diagonal), the
/// standard compact layout.
#[derive(Debug, Clone)]
pub struct Lu {
    lu: Matrix,
    /// `perm[i]` is the original row now occupying row `i`.
    perm: Vec<usize>,
    /// Number of row swaps, for the determinant sign.
    swaps: usize,
    n: usize,
}

impl Lu {
    /// Factorise `a` in place-ish (a copy is taken).
    ///
    /// `scale_guard` is the magnitude below which a pivot counts as zero,
    /// expressed relative to the matrix norm so the test is scale-invariant.
    pub fn factor(a: &Matrix) -> Result<Self, LinalgError> {
        if a.rows != a.cols {
            return Err(LinalgError::NotSquare {
                rows: a.rows,
                cols: a.cols,
            });
        }
        if !a.is_finite() {
            return Err(LinalgError::NonFinite);
        }
        let n = a.rows;
        let mut lu = a.clone();
        let mut perm: Vec<usize> = (0..n).collect();
        let mut swaps = 0usize;

        // Relative pivot floor: a pivot this small means the rows are linearly
        // dependent to working precision.
        let norm = a.norm_inf().max(f64::MIN_POSITIVE);
        let tiny = norm * f64::EPSILON * n as f64;

        for k in 0..n {
            // Partial pivoting: largest magnitude in the remaining column.
            let mut p = k;
            let mut best = lu[(k, k)].abs();
            for i in (k + 1)..n {
                let v = lu[(i, k)].abs();
                if v > best {
                    best = v;
                    p = i;
                }
            }
            if best <= tiny {
                return Err(LinalgError::Singular { pivot_index: k });
            }
            if p != k {
                for j in 0..n {
                    let tmp = lu[(k, j)];
                    lu[(k, j)] = lu[(p, j)];
                    lu[(p, j)] = tmp;
                }
                perm.swap(k, p);
                swaps += 1;
            }

            let pivot = lu[(k, k)];
            for i in (k + 1)..n {
                let factor = lu[(i, k)] / pivot;
                lu[(i, k)] = factor;
                if factor != 0.0 {
                    for j in (k + 1)..n {
                        let v = lu[(k, j)];
                        lu[(i, j)] -= factor * v;
                    }
                }
            }
        }

        Ok(Self { lu, perm, swaps, n })
    }

    #[inline]
    pub fn size(&self) -> usize {
        self.n
    }

    /// Solve `A·x = b` by forward/back substitution. `O(n²)`.
    pub fn solve(&self, b: &[f64]) -> Result<Vec<f64>, LinalgError> {
        if b.len() != self.n {
            return Err(LinalgError::DimensionMismatch {
                expected: self.n,
                found: b.len(),
            });
        }
        // Apply the permutation.
        let mut x: Vec<f64> = (0..self.n).map(|i| b[self.perm[i]]).collect();
        // Forward substitution with unit-diagonal L.
        for i in 1..self.n {
            let mut s = x[i];
            for j in 0..i {
                s -= self.lu[(i, j)] * x[j];
            }
            x[i] = s;
        }
        // Back substitution with U.
        for i in (0..self.n).rev() {
            let mut s = x[i];
            for j in (i + 1)..self.n {
                s -= self.lu[(i, j)] * x[j];
            }
            x[i] = s / self.lu[(i, i)];
        }
        Ok(x)
    }

    /// Solve `Aᵀ·x = b`. Needed by the condition estimator.
    pub fn solve_transpose(&self, b: &[f64]) -> Result<Vec<f64>, LinalgError> {
        if b.len() != self.n {
            return Err(LinalgError::DimensionMismatch {
                expected: self.n,
                found: b.len(),
            });
        }
        // Aᵀ = (P⁻¹LU)ᵀ = Uᵀ Lᵀ P, so solve Uᵀy = b, Lᵀz = y, then undo P.
        let mut y = b.to_vec();
        for i in 0..self.n {
            let mut s = y[i];
            for j in 0..i {
                s -= self.lu[(j, i)] * y[j];
            }
            y[i] = s / self.lu[(i, i)];
        }
        for i in (0..self.n).rev() {
            let mut s = y[i];
            for j in (i + 1)..self.n {
                s -= self.lu[(j, i)] * y[j];
            }
            y[i] = s;
        }
        let mut x = vec![0.0; self.n];
        for i in 0..self.n {
            x[self.perm[i]] = y[i];
        }
        Ok(x)
    }

    /// `log|det A|` — safe for the large determinants a panel matrix produces.
    pub fn log_abs_det(&self) -> f64 {
        (0..self.n).map(|i| self.lu[(i, i)].abs().ln()).sum()
    }

    pub fn det_sign(&self) -> f64 {
        let mut s = if self.swaps.is_multiple_of(2) {
            1.0
        } else {
            -1.0
        };
        for i in 0..self.n {
            if self.lu[(i, i)] < 0.0 {
                s = -s;
            }
        }
        s
    }

    /// Ratio of largest to smallest `|U_ii|`.
    ///
    /// Cheap, always available, and a useful *lower* bound on conditioning —
    /// but it can badly understate the true condition number, so
    /// [`condition_number_1`] is what the solver reports.
    pub fn pivot_ratio(&self) -> f64 {
        let mut mn = f64::INFINITY;
        let mut mx: f64 = 0.0;
        for i in 0..self.n {
            let v = self.lu[(i, i)].abs();
            mn = mn.min(v);
            mx = mx.max(v);
        }
        if mn > 0.0 {
            mx / mn
        } else {
            f64::INFINITY
        }
    }
}

/// Solve `A·x = b` once.
pub fn solve(a: &Matrix, b: &[f64]) -> Result<Vec<f64>, LinalgError> {
    Lu::factor(a)?.solve(b)
}

/// `‖A·x − b‖₂`, for reporting how well the system was actually satisfied.
pub fn residual_norm(a: &Matrix, x: &[f64], b: &[f64]) -> f64 {
    let ax = a.mul_vec(x);
    ax.iter()
        .zip(b.iter())
        .map(|(p, q)| (p - q) * (p - q))
        .sum::<f64>()
        .sqrt()
}

/// Relative residual `‖A·x − b‖₂ / ‖b‖₂`, the scale-free version.
pub fn relative_residual(a: &Matrix, x: &[f64], b: &[f64]) -> f64 {
    let nb = b.iter().map(|v| v * v).sum::<f64>().sqrt();
    if nb > 0.0 {
        residual_norm(a, x, b) / nb
    } else {
        residual_norm(a, x, b)
    }
}

/// Estimate the 1-norm condition number `‖A‖₁·‖A⁻¹‖₁` using Hager's algorithm,
/// the method behind LAPACK's `*gecon`.
///
/// Rationale: a true condition number costs an SVD, which is far more
/// expensive than the solve itself. Hager's estimator needs only a handful of
/// `O(n²)` triangular solves against the factorisation we already have, and in
/// practice lands within a small factor of the truth — plenty for the
/// engineering-credibility diagnostic the PRD asks for (§44).
pub fn condition_number_1(a: &Matrix, lu: &Lu) -> f64 {
    let n = lu.size();
    if n == 0 {
        return 0.0;
    }
    let norm_a = a.norm_1();
    if norm_a == 0.0 {
        return f64::INFINITY;
    }

    let mut x = vec![1.0 / n as f64; n];
    let mut est: f64 = 0.0;
    let mut visited = vec![false; n];

    for _ in 0..5 {
        let y = match lu.solve(&x) {
            Ok(v) => v,
            Err(_) => return f64::INFINITY,
        };
        let new_est: f64 = y.iter().map(|v| v.abs()).sum();
        let xi: Vec<f64> = y
            .iter()
            .map(|v| if *v >= 0.0 { 1.0 } else { -1.0 })
            .collect();
        let z = match lu.solve_transpose(&xi) {
            Ok(v) => v,
            Err(_) => return f64::INFINITY,
        };

        // Index of the largest |z|; also the next probe direction.
        let (j, zmax) =
            z.iter()
                .enumerate()
                .map(|(i, v)| (i, v.abs()))
                .fold(
                    (0usize, 0.0f64),
                    |acc, cur| if cur.1 > acc.1 { cur } else { acc },
                );

        let zx: f64 = z.iter().zip(x.iter()).map(|(p, q)| p * q).sum();
        if new_est > est {
            est = new_est;
        }
        // Converged, or the direction repeats: stop.
        if zmax <= zx || visited[j] {
            break;
        }
        visited[j] = true;
        x = vec![0.0; n];
        x[j] = 1.0;
    }

    // Second estimate from the alternating probe vector, which catches cases
    // the first sequence misses.
    let alt: Vec<f64> = (0..n)
        .map(|i| {
            let s = 1.0 + (i as f64) / ((n.max(2) - 1) as f64);
            if i % 2 == 0 {
                s
            } else {
                -s
            }
        })
        .collect();
    if let Ok(y) = lu.solve(&alt) {
        let alt_est = 2.0 * y.iter().map(|v| v.abs()).sum::<f64>() / (3.0 * n as f64);
        est = est.max(alt_est);
    }

    norm_a * est
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol * (1.0 + a.abs().max(b.abs()))
    }

    #[test]
    fn solves_a_small_system_exactly() {
        let a = Matrix::from_rows(&[
            vec![2.0, 1.0, -1.0],
            vec![-3.0, -1.0, 2.0],
            vec![-2.0, 1.0, 2.0],
        ]);
        let b = vec![8.0, -11.0, -3.0];
        let x = solve(&a, &b).unwrap();
        // Known solution (2, 3, -1).
        assert!(approx(x[0], 2.0, 1e-12), "{x:?}");
        assert!(approx(x[1], 3.0, 1e-12), "{x:?}");
        assert!(approx(x[2], -1.0, 1e-12), "{x:?}");
    }

    #[test]
    fn pivoting_handles_a_zero_leading_pivot() {
        let a = Matrix::from_rows(&[vec![0.0, 1.0], vec![1.0, 0.0]]);
        let x = solve(&a, &[2.0, 3.0]).unwrap();
        assert!(
            approx(x[0], 3.0, 1e-12) && approx(x[1], 2.0, 1e-12),
            "{x:?}"
        );
    }

    #[test]
    fn identity_round_trips() {
        let n = 20;
        let a = Matrix::identity(n);
        let b: Vec<f64> = (0..n).map(|i| i as f64 * 0.5 - 2.0).collect();
        let x = solve(&a, &b).unwrap();
        for i in 0..n {
            assert!(approx(x[i], b[i], 1e-14));
        }
    }

    #[test]
    fn singular_matrix_is_reported_not_silently_wrong() {
        let a = Matrix::from_rows(&[vec![1.0, 2.0], vec![2.0, 4.0]]);
        assert!(matches!(
            solve(&a, &[1.0, 2.0]),
            Err(LinalgError::Singular { .. })
        ));
    }

    #[test]
    fn non_finite_input_is_rejected() {
        let a = Matrix::from_rows(&[vec![f64::NAN, 1.0], vec![1.0, 1.0]]);
        assert_eq!(Lu::factor(&a).unwrap_err(), LinalgError::NonFinite);
    }

    #[test]
    fn non_square_is_rejected() {
        let a = Matrix::zeros(2, 3);
        assert!(matches!(Lu::factor(&a), Err(LinalgError::NotSquare { .. })));
    }

    #[test]
    fn dimension_mismatch_is_reported() {
        let lu = Lu::factor(&Matrix::identity(3)).unwrap();
        assert!(matches!(
            lu.solve(&[1.0, 2.0]),
            Err(LinalgError::DimensionMismatch {
                expected: 3,
                found: 2
            })
        ));
    }

    /// A random, well-conditioned, diagonally dominant system of realistic size:
    /// the residual must be at round-off level.
    #[test]
    fn residual_is_at_round_off_for_a_dense_system() {
        let n = 120;
        let mut a = Matrix::zeros(n, n);
        let mut seed = 0x2545F4914F6CDD1Du64;
        let mut rand = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed >> 11) as f64 / (1u64 << 53) as f64 - 0.5
        };
        for i in 0..n {
            for j in 0..n {
                a[(i, j)] = rand();
            }
            a[(i, i)] += n as f64; // diagonal dominance
        }
        let xt: Vec<f64> = (0..n).map(|i| (i as f64).sin()).collect();
        let b = a.mul_vec(&xt);
        let x = solve(&a, &b).unwrap();
        assert!(relative_residual(&a, &x, &b) < 1e-13);
        let err: f64 = x
            .iter()
            .zip(xt.iter())
            .map(|(p, q)| (p - q).abs())
            .fold(0.0, f64::max);
        assert!(err < 1e-12, "max error {err}");
    }

    #[test]
    fn transpose_solve_is_consistent_with_the_transposed_matrix() {
        let n = 8;
        let mut a = Matrix::zeros(n, n);
        for i in 0..n {
            for j in 0..n {
                a[(i, j)] = ((i * 7 + j * 3) % 11) as f64 - 5.0;
            }
            a[(i, i)] += 20.0;
        }
        let b: Vec<f64> = (0..n).map(|i| 1.0 + i as f64).collect();
        let lu = Lu::factor(&a).unwrap();
        let x = lu.solve_transpose(&b).unwrap();
        // Verify Aᵀ·x = b directly.
        for i in 0..n {
            let s: f64 = (0..n).map(|k| a[(k, i)] * x[k]).sum();
            assert!(approx(s, b[i], 1e-10), "row {i}: {s} vs {}", b[i]);
        }
    }

    #[test]
    fn factorisation_is_reused_across_right_hand_sides() {
        let a = Matrix::from_rows(&[vec![4.0, 1.0], vec![1.0, 3.0]]);
        let lu = Lu::factor(&a).unwrap();
        for k in 0..5 {
            let b = vec![k as f64, 1.0 - k as f64];
            let x = lu.solve(&b).unwrap();
            assert!(relative_residual(&a, &x, &b) < 1e-14);
        }
    }

    #[test]
    fn log_det_and_sign_match_a_known_determinant() {
        // det = 4·3 − 1·1 = 11
        let a = Matrix::from_rows(&[vec![4.0, 1.0], vec![1.0, 3.0]]);
        let lu = Lu::factor(&a).unwrap();
        assert!(approx(lu.log_abs_det().exp(), 11.0, 1e-12));
        assert_eq!(lu.det_sign(), 1.0);

        // Swapping rows flips the sign.
        let b = Matrix::from_rows(&[vec![1.0, 3.0], vec![4.0, 1.0]]);
        let lub = Lu::factor(&b).unwrap();
        assert_eq!(lub.det_sign(), -1.0);
    }

    #[test]
    fn condition_estimate_is_near_one_for_the_identity() {
        let a = Matrix::identity(10);
        let lu = Lu::factor(&a).unwrap();
        let c = condition_number_1(&a, &lu);
        assert!((1.0..=2.0).contains(&c), "cond = {c}");
    }

    /// The estimator must be a lower bound on the true 1-norm condition number
    /// and must stay within a modest factor of it for a matrix whose inverse we
    /// know exactly.
    #[test]
    fn condition_estimate_brackets_a_known_condition_number() {
        // diag(1, 1e-6): ‖A‖₁ = 1, ‖A⁻¹‖₁ = 1e6, so cond₁ = 1e6.
        let mut a = Matrix::identity(2);
        a[(1, 1)] = 1e-6;
        let lu = Lu::factor(&a).unwrap();
        let c = condition_number_1(&a, &lu);
        assert!(
            c <= 1.0e6 * 1.001,
            "estimate must not exceed the truth: {c}"
        );
        assert!(c > 1.0e5, "estimate far too small: {c}");
    }

    #[test]
    fn condition_estimate_grows_for_a_hilbert_matrix() {
        // The Hilbert matrix is the textbook ill-conditioned example: cond
        // grows roughly like e^{3.5n}.
        let make = |n: usize| {
            let mut h = Matrix::zeros(n, n);
            for i in 0..n {
                for j in 0..n {
                    h[(i, j)] = 1.0 / (i + j + 1) as f64;
                }
            }
            h
        };
        let mut prev = 0.0;
        for n in [3usize, 5, 7] {
            let h = make(n);
            let lu = Lu::factor(&h).unwrap();
            let c = condition_number_1(&h, &lu);
            assert!(c > prev, "cond should grow with n: {c} after {prev}");
            prev = c;
        }
        // cond₁(H₇) is about 1e9; just check the order of magnitude is large.
        assert!(prev > 1.0e7, "H7 condition estimate too small: {prev}");
    }

    #[test]
    fn norms_match_hand_computed_values() {
        let a = Matrix::from_rows(&[vec![1.0, -2.0], vec![-3.0, 4.0]]);
        // Column sums: |1|+|−3| = 4, |−2|+|4| = 6 → 1-norm 6.
        assert!(approx(a.norm_1(), 6.0, 1e-15));
        // Row sums: 3 and 7 → ∞-norm 7.
        assert!(approx(a.norm_inf(), 7.0, 1e-15));
    }

    #[test]
    fn mul_vec_matches_manual_product() {
        let a = Matrix::from_rows(&[vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]]);
        assert_eq!(a.mul_vec(&[1.0, 0.0, -1.0]), vec![-2.0, -2.0]);
    }
}
