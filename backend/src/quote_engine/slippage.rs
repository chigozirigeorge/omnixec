use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// Slippage impact information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlippageImpact {
    /// Expected amount user would receive at current price
    pub expected_amount: Decimal,
    /// Minimum amount user would accept
    pub minimum_amount: Decimal,
    /// Slippage percentage
    pub slippage_pct: Decimal,
    /// Slippage amount
    pub slippage_amount: Decimal,
    /// Price impact from liquidity
    pub price_impact_pct: Decimal,
    /// Is slippage acceptable
    pub is_acceptable: bool,
}

impl SlippageImpact {
    /// Calculate slippage impact for a trade
    /// 
    /// Parameters:
    /// - spot_price: Current market price
    /// - execution_price: Price user will get (after slippage)
    /// - output_amount: Amount user gets
    /// - max_slippage_pct: Maximum acceptable slippage (e.g., 0.5 for 0.5%)
    pub fn calculate(
        spot_price: Decimal,
        execution_price: Decimal,
        output_amount: Decimal,
        max_slippage_pct: Decimal,
    ) -> Self {
        // Expected amount at spot price
        let expected_amount = output_amount / execution_price * spot_price;

        // Actual slippage
        let slippage_amount = expected_amount - output_amount;

        // Slippage percentage
        let slippage_pct = if expected_amount.is_zero() {
            Decimal::ZERO
        } else {
            (slippage_amount / expected_amount) * Decimal::from(100)
        };

        // Price impact (difference between spot and execution price)
        let price_impact_pct = if spot_price.is_zero() {
            Decimal::ZERO
        } else {
            ((spot_price - execution_price) / spot_price) * Decimal::from(100)
        };

        let is_acceptable = slippage_pct <= max_slippage_pct;

        Self {
            expected_amount,
            minimum_amount: output_amount,
            slippage_pct,
            slippage_amount,
            price_impact_pct,
            is_acceptable,
        }
    }

    /// Format for display
    pub fn to_display_string(&self) -> String {
        format!(
            "Expected: {} | Min: {} | Slippage: {:.2}% | Impact: {:.2}%",
            self.expected_amount, self.minimum_amount, self.slippage_pct, self.price_impact_pct
        )
    }
}

/// Quote with slippage information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuoteWithSlippage {
    pub quote_id: String,
    pub funding_amount: Decimal,
    pub execution_amount: Decimal,
    pub funding_asset: String,
    pub execution_asset: String,
    pub slippage: SlippageImpact,
    pub warning: Option<String>,
}

impl QuoteWithSlippage {
    pub fn new(
        quote_id: String,
        funding_amount: Decimal,
        execution_amount: Decimal,
        funding_asset: String,
        execution_asset: String,
        spot_price: Decimal,
        execution_price: Decimal,
        max_slippage_pct: Decimal,
    ) -> Self {
        let slippage = SlippageImpact::calculate(
            spot_price,
            execution_price,
            execution_amount,
            max_slippage_pct,
        );

        let warning = if !slippage.is_acceptable {
            Some(format!(
                "Slippage of {:.2}% exceeds maximum of {:.2}%",
                slippage.slippage_pct, max_slippage_pct
            ))
        } else if slippage.slippage_pct > max_slippage_pct * Decimal::from_str("0.5").unwrap() {
            Some(format!(
                "High slippage: {:.2}%",
                slippage.slippage_pct
            ))
        } else {
            None
        };

        Self {
            quote_id,
            funding_amount,
            execution_amount,
            funding_asset,
            execution_asset,
            slippage,
            warning,
        }
    }
}

/// Slippage tolerance settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlippageTolerance {
    pub low: Decimal,    // 0.1%
    pub medium: Decimal, // 0.5%
    pub high: Decimal,   // 1.0%
    pub custom: Option<Decimal>,
}

impl SlippageTolerance {
    pub fn new_defaults() -> Self {
        Self {
            low: Decimal::from_str("0.1").unwrap(),
            medium: Decimal::from_str("0.5").unwrap(),
            high: Decimal::from_str("1.0").unwrap(),
            custom: None,
        }
    }

    pub fn with_custom(mut self, pct: Decimal) -> Self {
        self.custom = Some(pct);
        self
    }

    pub fn get_effective_tolerance(&self) -> Decimal {
        self.custom.unwrap_or(self.medium)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slippage_calculation() {
        let spot_price = Decimal::from(100);
        let execution_price = Decimal::from(99); // 1% slippage
        let output_amount = Decimal::from(1000);
        let max_slippage = Decimal::from_str("1.0").unwrap();

        let impact = SlippageImpact::calculate(
            spot_price,
            execution_price,
            output_amount,
            max_slippage,
        );

        assert!(impact.is_acceptable);
        assert!(impact.slippage_pct > Decimal::ZERO);
    }

    #[test]
    fn test_slippage_tolerance() {
        let tolerance = SlippageTolerance::new_defaults();
        assert_eq!(
            tolerance.get_effective_tolerance(),
            Decimal::from_str("0.5").unwrap()
        );

        let custom = tolerance.with_custom(Decimal::from_str("2.0").unwrap());
        assert_eq!(
            custom.get_effective_tolerance(),
            Decimal::from_str("2.0").unwrap()
        );
    }
}
