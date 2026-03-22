use crate::accounting::cost::CostAccountant;
use crate::config::BudgetsConfig;

pub struct BudgetEnforcer {
    pub config: BudgetsConfig,
}

#[derive(Debug)]
pub enum BudgetExceeded {
    PerCaller {
        caller_id: String,
        spend: f64,
        ceiling: f64,
    },
    PerRoute {
        route_key: String,
        spend: f64,
        ceiling: f64,
    },
    Global {
        spend: f64,
        ceiling: f64,
    },
}

impl std::fmt::Display for BudgetExceeded {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BudgetExceeded::PerCaller {
                caller_id,
                spend,
                ceiling,
            } => write!(
                f,
                "caller '{caller_id}' daily spend ${spend:.4} exceeds ceiling ${ceiling:.2}"
            ),
            BudgetExceeded::PerRoute {
                route_key,
                spend,
                ceiling,
            } => write!(
                f,
                "route '{route_key}' daily spend ${spend:.4} exceeds ceiling ${ceiling:.2}"
            ),
            BudgetExceeded::Global { spend, ceiling } => {
                write!(
                    f,
                    "global daily spend ${spend:.4} exceeds ceiling ${ceiling:.2}"
                )
            }
        }
    }
}

impl BudgetEnforcer {
    pub fn new(config: BudgetsConfig) -> Self {
        Self { config }
    }

    pub fn check(
        &self,
        cost_accountant: &CostAccountant,
        caller_id: &str,
        route_key: &str,
    ) -> Result<(), BudgetExceeded> {
        // Per-caller check
        let caller_spend = cost_accountant.caller_spend_24h(caller_id).unwrap_or(0.0);
        if caller_spend >= self.config.per_caller_daily_usd {
            return Err(BudgetExceeded::PerCaller {
                caller_id: caller_id.to_string(),
                spend: caller_spend,
                ceiling: self.config.per_caller_daily_usd,
            });
        }

        // Per-route check
        let route_spend = cost_accountant.route_spend_24h(route_key).unwrap_or(0.0);
        if route_spend >= self.config.per_route_daily_usd {
            return Err(BudgetExceeded::PerRoute {
                route_key: route_key.to_string(),
                spend: route_spend,
                ceiling: self.config.per_route_daily_usd,
            });
        }

        // Global check
        let global_spend = cost_accountant.global_spend_24h().unwrap_or(0.0);
        if global_spend >= self.config.global_daily_usd {
            return Err(BudgetExceeded::Global {
                spend: global_spend,
                ceiling: self.config.global_daily_usd,
            });
        }

        Ok(())
    }
}
