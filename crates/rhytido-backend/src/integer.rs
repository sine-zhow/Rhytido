//! Arbitrary-precision signed integers.
//!
//! This is the backend facade integer type. The rest of Rhytido uses [`Integer`]
//! and never the underlying backend representation (facade isolation, ADR-2).

use core::fmt;
use core::ops::{Add, Mul, Neg, Sub};
use core::str::FromStr;

use dashu_int::IBig;

use crate::Error;

/// An arbitrary-precision signed integer.
///
/// Arithmetic is exact (no overflow). Division is truncating and fallible: use
/// [`Integer::checked_div`] / [`Integer::div_rem`], which report a zero divisor
/// instead of panicking.
///
/// # Examples
///
/// ```
/// use rhytido_backend::Integer;
///
/// let a = Integer::from(21);
/// let b = Integer::from(2);
/// assert_eq!((a.clone() * b.clone()).to_string(), "42");
/// assert_eq!(a.checked_div(&b).unwrap().to_string(), "10");
/// assert!(a.checked_div(&Integer::from(0)).is_none());
/// ```
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Integer {
    inner: IBig,
}

impl Integer {
    /// Returns `true` if this integer is zero.
    pub fn is_zero(&self) -> bool {
        self.inner == IBig::from(0)
    }

    /// Returns `true` if this integer is strictly negative.
    pub fn is_negative(&self) -> bool {
        self.inner < IBig::from(0)
    }

    /// Returns `true` if this integer is strictly positive.
    pub fn is_positive(&self) -> bool {
        self.inner > IBig::from(0)
    }

    /// Raises this integer to the power `exp`.
    ///
    /// `pow(0)` is `1` for any base, including zero.
    pub fn pow(&self, exp: usize) -> Self {
        Self {
            inner: self.inner.pow(exp),
        }
    }

    /// Parses an integer from `src` in the given `radix` (2..=36).
    ///
    /// # Errors
    ///
    /// Returns [`Error::Parse`] if `src` is not a valid integer in `radix`, or
    /// `radix` is out of range.
    pub fn from_str_radix(src: &str, radix: u32) -> Result<Self, Error> {
        IBig::from_str_radix(src, radix)
            .map(|inner| Self { inner })
            .map_err(|e| Error::Parse(e.to_string()))
    }

    /// Checked truncating division. Returns `None` if `other` is zero.
    pub fn checked_div(&self, other: &Self) -> Option<Self> {
        if other.is_zero() {
            None
        } else {
            Some(Self {
                inner: &self.inner / &other.inner,
            })
        }
    }

    /// Checked truncating remainder. Returns `None` if `other` is zero.
    pub fn checked_rem(&self, other: &Self) -> Option<Self> {
        if other.is_zero() {
            None
        } else {
            Some(Self {
                inner: &self.inner % &other.inner,
            })
        }
    }

    /// Truncating division and remainder together.
    ///
    /// # Errors
    ///
    /// Returns [`Error::DivisionByZero`] if `other` is zero.
    pub fn div_rem(&self, other: &Self) -> Result<(Self, Self), Error> {
        if other.is_zero() {
            return Err(Error::DivisionByZero);
        }
        Ok((
            Self {
                inner: &self.inner / &other.inner,
            },
            Self {
                inner: &self.inner % &other.inner,
            },
        ))
    }
}

impl Integer {
    /// Constructs from a backend big integer. Crate-internal so the backend
    /// type stays out of the public facade API (facade isolation, ADR-2).
    pub(crate) fn from_ibig(v: IBig) -> Self {
        Self { inner: v }
    }

    /// Converts into the backend big integer. Crate-internal so the backend
    /// type stays out of the public facade API (facade isolation, ADR-2).
    pub(crate) fn into_ibig(self) -> IBig {
        self.inner
    }
}

impl From<i64> for Integer {
    fn from(value: i64) -> Self {
        Self {
            inner: IBig::from(value),
        }
    }
}

impl From<i32> for Integer {
    fn from(value: i32) -> Self {
        Self {
            inner: IBig::from(value),
        }
    }
}

impl FromStr for Integer {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_str_radix(s, 10)
    }
}

impl fmt::Display for Integer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.inner, f)
    }
}

impl fmt::Debug for Integer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.inner, f)
    }
}

impl Add for Integer {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self {
            inner: self.inner + rhs.inner,
        }
    }
}

impl Sub for Integer {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        Self {
            inner: self.inner - rhs.inner,
        }
    }
}

impl Mul for Integer {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self {
        Self {
            inner: self.inner * rhs.inner,
        }
    }
}

impl Neg for Integer {
    type Output = Self;

    fn neg(self) -> Self {
        Self { inner: -self.inner }
    }
}

impl Add for &Integer {
    type Output = Integer;

    fn add(self, rhs: Self) -> Integer {
        Integer {
            inner: &self.inner + &rhs.inner,
        }
    }
}

impl Sub for &Integer {
    type Output = Integer;

    fn sub(self, rhs: Self) -> Integer {
        Integer {
            inner: &self.inner - &rhs.inner,
        }
    }
}

impl Mul for &Integer {
    type Output = Integer;

    fn mul(self, rhs: Self) -> Integer {
        Integer {
            inner: &self.inner * &rhs.inner,
        }
    }
}

impl Neg for &Integer {
    type Output = Integer;

    fn neg(self) -> Integer {
        Integer {
            inner: -&self.inner,
        }
    }
}
