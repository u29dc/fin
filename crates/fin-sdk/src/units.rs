use std::fmt::{Display, Formatter};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Currency {
    #[serde(rename = "GBP")]
    #[default]
    Gbp,
    #[serde(rename = "USD")]
    Usd,
    #[serde(rename = "EUR")]
    Eur,
}

impl Currency {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Gbp => "GBP",
            Self::Usd => "USD",
            Self::Eur => "EUR",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Money {
    pub minor: i64,
    #[serde(default)]
    pub currency: Currency,
}

impl Money {
    #[must_use]
    pub const fn new(minor: i64, currency: Currency) -> Self {
        Self { minor, currency }
    }

    #[must_use]
    pub const fn zero(currency: Currency) -> Self {
        Self { minor: 0, currency }
    }

    #[must_use]
    pub fn from_major(major: f64, currency: Currency) -> Self {
        let scaled = (major * 100.0).round() as i64;
        Self {
            minor: scaled,
            currency,
        }
    }

    #[must_use]
    pub fn as_major(self) -> f64 {
        self.minor as f64 / 100.0
    }

    #[must_use]
    pub fn checked_add(self, rhs: Money) -> Option<Money> {
        if self.currency != rhs.currency {
            return None;
        }
        self.minor.checked_add(rhs.minor).map(|minor| Money {
            minor,
            currency: self.currency,
        })
    }

    #[must_use]
    pub fn checked_sub(self, rhs: Money) -> Option<Money> {
        if self.currency != rhs.currency {
            return None;
        }
        self.minor.checked_sub(rhs.minor).map(|minor| Money {
            minor,
            currency: self.currency,
        })
    }
}

impl Display for Money {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {:.2}", self.currency.code(), self.as_major())
    }
}

#[cfg(test)]
mod tests {
    use super::{Currency, Money};

    #[test]
    fn from_major_rounds_half_away_from_zero() {
        let money = Money::from_major(12.345, Currency::Gbp);
        assert_eq!(money.minor, 1235);
    }

    #[test]
    fn checked_add_requires_same_currency() {
        let gbp = Money::new(100, Currency::Gbp);
        let usd = Money::new(100, Currency::Usd);
        assert!(gbp.checked_add(usd).is_none());
    }
}
