//! Exact rational numbers in canonical (reduced) form.
//!
//! Facade over the trusted backend rational representation (facade isolation,
//! ADR-2). The canonical form keeps a positive denominator and coprime
//! numerator/denominator, so equality and ordering are structural.

use core::fmt;
use core::ops::{Add, Mul, Neg, Sub};
use core::str::FromStr;

use dashu_int::{IBig, UBig};
use dashu_ratio::RBig;

use crate::{Error, Integer};

/// An exact rational number, always in canonical (reduced) form with a positive
/// denominator.
///
/// # Examples
///
/// ```
/// use rhytido_backend::Rational;
///
/// let half = Rational::new(1, 2).unwrap();
/// let third = Rational::new(1, 3).unwrap();
/// assert_eq!((half + third).to_string(), "5/6");
/// assert_eq!(Rational::new(2, 4).unwrap().to_string(), "1/2");
/// assert!(Rational::new(1, 0).is_err());
/// ```
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Rational {
    inner: RBig,
}

impl Rational {
    /// Constructs `num / den`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::DivisionByZero`] if `den` is zero. The sign is
    /// normalized so the denominator is positive.
    pub fn new(num: i64, den: i64) -> Result<Self, Error> {
        if den == 0 {
            return Err(Error::DivisionByZero);
        }
        Ok(Self {
            inner: RBig::from_parts_signed(IBig::from(num), IBig::from(den)),
        })
    }

    /// Constructs an integer as a rational (denominator 1).
    pub fn from_i64(v: i64) -> Self {
        Self {
            inner: RBig::from_parts_signed(IBig::from(v), IBig::from(1)),
        }
    }

    /// Returns the numerator in canonical form.
    pub fn numerator(&self) -> Integer {
        Integer::from_ibig(self.inner.numerator().clone())
    }

    /// Returns the (positive) denominator in canonical form.
    pub fn denominator(&self) -> Integer {
        Integer::from_ibig(IBig::from(self.inner.denominator().clone()))
    }

    /// Returns `true` if this rational is zero.
    pub fn is_zero(&self) -> bool {
        *self.inner.numerator() == IBig::from(0)
    }

    /// Returns `true` if this rational is strictly negative.
    pub fn is_negative(&self) -> bool {
        *self.inner.numerator() < IBig::from(0)
    }

    /// Returns `true` if this rational is strictly positive.
    pub fn is_positive(&self) -> bool {
        *self.inner.numerator() > IBig::from(0)
    }

    /// Returns `true` if this rational is an integer (denominator 1).
    pub fn is_integer(&self) -> bool {
        *self.inner.denominator() == UBig::from(1u8)
    }

    /// Checked division. Returns `None` if `other` is zero.
    pub fn checked_div(&self, other: &Self) -> Option<Self> {
        if other.is_zero() {
            None
        } else {
            Some(Self {
                inner: self.inner.clone() / other.inner.clone(),
            })
        }
    }

    /// Checked reciprocal. Returns `None` if this rational is zero.
    pub fn checked_recip(&self) -> Option<Self> {
        if self.is_zero() {
            None
        } else {
            Some(Self {
                inner: RBig::from_parts_signed(
                    IBig::from(self.inner.denominator().clone()),
                    self.inner.numerator().clone(),
                ),
            })
        }
    }
}

impl From<i64> for Rational {
    fn from(v: i64) -> Self {
        Self::from_i64(v)
    }
}

impl FromStr for Rational {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let t = s.trim();
        // Accept "a/b"; also accept a bare integer "a".
        if t.contains('/') {
            let inner = RBig::from_str(t).map_err(|e| Error::Parse(e.to_string()))?;
            // The backend accepts a zero denominator; the facade rejects it.
            if *inner.denominator() == UBig::from(0u8) {
                return Err(Error::Parse("zero denominator".to_string()));
            }
            Ok(Self { inner })
        } else {
            let n = t.parse::<IBig>().map_err(|e| Error::Parse(e.to_string()))?;
            Ok(Self {
                inner: RBig::from_parts_signed(n, IBig::from(1)),
            })
        }
    }
}

impl fmt::Display for Rational {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_integer() {
            fmt::Display::fmt(self.inner.numerator(), f)
        } else {
            fmt::Display::fmt(&self.inner, f)
        }
    }
}

impl fmt::Debug for Rational {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl Add for Rational {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self {
            inner: self.inner + rhs.inner,
        }
    }
}

impl Sub for Rational {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        Self {
            inner: self.inner - rhs.inner,
        }
    }
}

impl Mul for Rational {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self {
        Self {
            inner: self.inner * rhs.inner,
        }
    }
}

impl Neg for Rational {
    type Output = Self;

    fn neg(self) -> Self {
        Self { inner: -self.inner }
    }
}

impl Add for &Rational {
    type Output = Rational;

    fn add(self, rhs: Self) -> Rational {
        Rational {
            inner: &self.inner + &rhs.inner,
        }
    }
}

impl Sub for &Rational {
    type Output = Rational;

    fn sub(self, rhs: Self) -> Rational {
        Rational {
            inner: &self.inner - &rhs.inner,
        }
    }
}

impl Mul for &Rational {
    type Output = Rational;

    fn mul(self, rhs: Self) -> Rational {
        Rational {
            inner: &self.inner * &rhs.inner,
        }
    }
}

impl Neg for &Rational {
    type Output = Rational;

    fn neg(self) -> Rational {
        Rational {
            inner: -&self.inner,
        }
    }
}
