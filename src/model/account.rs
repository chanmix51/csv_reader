use anyhow::{anyhow, Context};
use rust_decimal::Decimal;
use thiserror::Error;

use crate::Result;

/// The client ID type alias.
pub type ClientId = u16;

/// The error type for account operations.
#[derive(Debug, Error)]
pub enum AccountError {
    /// Insufficient available funds to perform the operation.
    #[error("Insufficient available funds: available {available}, requested {requested}.")]
    InsufficientAvailableFunds {
        /// The available funds in the account.
        available: Decimal,

        /// The withdraw amount requested
        requested: Decimal,
    },
    /// Insufficient held funds to perform the operation.
    #[error("Insufficient held funds: held {held}, requested {requested}.")]
    InsufficientHeldFunds {
        /// The held funds in the account.
        held: Decimal,

        /// The resolve amount requested
        requested: Decimal,
    },
    /// Operation cannot be performed because the account is locked.
    #[error("Account is locked.")]
    AccountLocked,
}

/// It represents the state of a client account. It contains the different types
/// of funds held by the account.
#[derive(Debug, Default)]
pub struct Account {
    /// The client ID of the account.
    pub client_id: ClientId,

    /// The available funds in the account.
    pub available: Decimal,

    /// The held funds in the account.
    pub held: Decimal,

    /// The total funds in the account.
    pub total: Decimal,

    /// The lock status of the account.
    pub locked: bool,
}

impl Account {
    /// Creates a new account with the given client ID. The account is initialized
    /// with zero funds and unlocked.
    pub fn new(client_id: u16) -> Self {
        Account {
            client_id,
            available: Decimal::ZERO,
            held: Decimal::ZERO,
            total: Decimal::ZERO,
            locked: false,
        }
    }

    fn check_locked(&self) -> Result<()> {
        if self.locked {
            Err(anyhow!(AccountError::AccountLocked))
                .context(format!("Account {} is locked.", self.client_id))
        } else {
            Ok(())
        }
    }

    fn update_total(&mut self) -> Result<()> {
        self.total = self.available + self.held;

        Ok(())
    }

    /// Deposits the given amount into the account. The given amount is added to
    /// the available funds.
    ///
    /// ```
    /// use rust_decimal::Decimal;
    /// use csv_reader::model::{Account, AccountError};
    ///
    /// let mut account = Account::new(1);
    /// account.deposit(Decimal::new(100, 0)).unwrap();
    ///
    /// assert_eq!(account.available, Decimal::new(100, 0));
    /// assert_eq!(account.held, Decimal::ZERO);
    /// assert_eq!(account.total, Decimal::new(100, 0));
    ///
    /// // locked account cannot deposit
    /// account.locked = true;
    /// let result = account.deposit(Decimal::new(100, 0)).unwrap_err();
    ///
    /// assert!(matches!(
    ///     result.downcast_ref::<AccountError>(),
    ///     Some(&AccountError::AccountLocked)
    /// ));
    /// ```
    pub fn deposit(&mut self, amount: Decimal) -> Result<()> {
        self.check_locked()?;
        self.available += amount;

        self.update_total()
    }

    /// Withdraws the given amount from the account. The given amount is subtracted
    /// from the available funds. If the available funds are less than the requested
    /// amount, an error is returned. If the account is locked, an error is returned.
    ///
    /// ```
    /// use rust_decimal::Decimal;
    /// use csv_reader::model::{Account, AccountError};
    ///
    /// let mut account = Account::new(1);
    /// account.deposit(Decimal::new(100, 0)).unwrap();
    /// account.withdraw(Decimal::new(50, 0)).unwrap();
    ///
    /// assert_eq!(account.available, Decimal::new(50, 0));
    /// assert_eq!(account.held, Decimal::ZERO);
    /// assert_eq!(account.total, Decimal::new(50, 0));
    ///
    /// // insufficient funds
    /// let result = account.withdraw(Decimal::new(150, 0)).unwrap_err();
    /// assert!(matches!(
    ///   result.downcast_ref::<AccountError>(),
    ///   Some(&AccountError::InsufficientAvailableFunds { available, requested })
    ///     if available == Decimal::new(50, 0) && requested == Decimal::new(150, 0)
    /// ));
    ///
    /// // locked account cannot withdraw
    /// account.locked = true;
    ///
    /// let result = account.withdraw(Decimal::new(50, 0)).unwrap_err();
    /// assert!(matches!(
    ///   result.downcast_ref::<AccountError>(),
    ///   Some(&AccountError::AccountLocked)
    /// ));
    ///
    /// ```
    pub fn withdraw(&mut self, amount: Decimal) -> Result<()> {
        self.check_locked()?;

        if self.available < amount {
            return Err(anyhow!(AccountError::InsufficientAvailableFunds {
                available: self.available,
                requested: amount,
            }))
            .context(format!("Account: {}", self.client_id));
        }
        self.available -= amount;

        self.update_total()
    }

    /// Disputes the given amount. The amount is subtracted from the available funds
    /// and added to the held funds while the total funds remain the same.
    ///
    /// What happens if the total funds are less than the requested amount? This
    /// is not specified in the requirements. For now, we will assume that the
    /// dispute is always successful and the available funds can be negative. In
    /// this configuration, the client can not withdraw any funds while it still
    /// can deposit funds.
    ///
    /// Dispute can arise even though the client acount is locked.
    ///
    /// ```
    /// use rust_decimal::Decimal;
    /// use csv_reader::model::Account;
    ///
    /// let mut account = Account::new(1);
    /// account.deposit(Decimal::new(100, 0)).unwrap();
    /// account.dispute(Decimal::new(50, 0)).unwrap();
    ///
    /// assert_eq!(account.available, Decimal::new(50, 0));
    /// assert_eq!(account.held, Decimal::new(50, 0));
    /// assert_eq!(account.total, Decimal::new(100, 0));
    ///
    /// // locked account can dispute
    /// account.locked = true;
    /// account.dispute(Decimal::new(50, 0)).unwrap();
    ///
    /// assert_eq!(account.available, Decimal::ZERO);
    /// assert_eq!(account.held, Decimal::new(100, 0));
    ///
    /// // dispute can produce negative available funds
    /// account.dispute(Decimal::new(20, 0)).unwrap();
    ///
    /// assert_eq!(account.available, Decimal::new(-20, 0));
    /// assert_eq!(account.held, Decimal::new(120, 0));
    /// assert_eq!(account.total, Decimal::new(100, 0));
    ///
    /// ```
    pub fn dispute(&mut self, amount: Decimal) -> Result<()> {
        self.available -= amount;
        self.held += amount;

        self.update_total()
    }

    /// Resolves the disputed amount. The amount is added to the available funds and
    /// subtracted from the held funds. The total funds remain the same.
    /// It is possible to resolve a disputed amount even though the account is locked.
    /// If the resolved amount is greater than the held amount, an error is returned.
    ///
    /// ```
    /// use rust_decimal::Decimal;
    /// use csv_reader::model::{Account, AccountError};
    ///
    /// let mut account = Account::new(1);
    /// account.deposit(Decimal::new(100, 0)).unwrap();
    /// account.dispute(Decimal::new(50, 0)).unwrap();
    /// account.resolve(Decimal::new(30, 0)).unwrap();
    ///
    /// assert_eq!(account.available, Decimal::new(80, 0));
    /// assert_eq!(account.held, Decimal::new(20, 0));
    /// assert_eq!(account.total, Decimal::new(100, 0));
    ///
    /// // locked account can resolve
    /// account.locked = true;
    /// account.resolve(Decimal::new(20, 0)).unwrap();
    ///
    /// assert_eq!(account.available, Decimal::new(100, 0));
    /// assert_eq!(account.held, Decimal::ZERO);
    ///
    /// // resolve more than held amount raises error
    /// let result = account.resolve(Decimal::new(50, 0)).unwrap_err();
    /// assert!(matches!(
    ///   result.downcast_ref::<AccountError>(),
    ///   Some(&AccountError::InsufficientHeldFunds { held, requested })
    ///     if held == Decimal::ZERO && requested == Decimal::new(50, 0)
    /// ));
    ///
    /// ```
    pub fn resolve(&mut self, amount: Decimal) -> Result<()> {
        if amount > self.held {
            return Err(anyhow!(AccountError::InsufficientHeldFunds {
                held: self.held,
                requested: amount,
            }))
            .context(format!("Account: {}", self.client_id));
        }
        self.available += amount;
        self.held -= amount;

        self.update_total()
    }

    /// Charges back the disputed amount. The amount is subtracted from the held funds
    /// and the account is locked. The total funds are lowered by the disputed amount.
    /// If the charged back amount is greater than the held amount, an error is returned.
    /// It is possible to chargeback a disputed amount even though the account is locked.
    ///
    /// ```
    /// use rust_decimal::Decimal;
    /// use csv_reader::model::{Account, AccountError};
    ///
    /// let mut account = Account::new(1);
    /// account.deposit(Decimal::new(100, 0)).unwrap();
    /// account.dispute(Decimal::new(50, 0)).unwrap();
    /// account.chargeback(Decimal::new(30, 0)).unwrap();
    ///
    /// assert_eq!(account.available, Decimal::new(50, 0));
    /// assert_eq!(account.held, Decimal::new(20, 0));
    /// assert_eq!(account.total, Decimal::new(70, 0));
    /// assert!(account.locked);
    ///
    /// // locked account can chargeback
    /// account.chargeback(Decimal::new(20, 0)).unwrap();
    ///
    /// assert_eq!(account.held, Decimal::ZERO);
    ///
    /// // chargeback more than held amount raises error
    /// let error = account.chargeback(Decimal::new(50, 0)).unwrap_err();
    ///
    /// assert!(matches!(
    ///     error.downcast_ref::<AccountError>(),
    ///     Some(&AccountError::InsufficientHeldFunds { held, requested })
    ///     if held == Decimal::ZERO && requested == Decimal::new(50, 0)
    /// ));
    /// ```
    pub fn chargeback(&mut self, amount: Decimal) -> Result<()> {
        if amount > self.held {
            return Err(anyhow!(AccountError::InsufficientHeldFunds {
                held: self.held,
                requested: amount,
            }))
            .context(format!("Account: {}", self.client_id));
        }
        self.held -= amount;
        self.locked = true;

        self.update_total()
    }
}

#[cfg(test)]
mod tests {
    pub use super::*;

    #[test]
    fn test_deposit() {
        let mut account = Account::new(1);
        account.deposit(Decimal::new(100, 0)).unwrap();

        assert_eq!(account.available, Decimal::new(100, 0));
        assert_eq!(account.held, Decimal::ZERO);
        assert_eq!(account.total, Decimal::new(100, 0));
    }

    #[test]
    fn test_deposit_locked() {
        let mut account = Account::new(1);
        account.locked = true;
        let result = account.deposit(Decimal::new(100, 0)).unwrap_err();

        assert!(matches!(
            result.downcast_ref::<AccountError>(),
            Some(&AccountError::AccountLocked)
        ));
    }

    #[test]
    fn test_successful_withdrawal() {
        let mut account = Account::new(1);
        account.deposit(Decimal::new(100, 0)).unwrap();
        account.withdraw(Decimal::new(50, 0)).unwrap();

        assert_eq!(account.available, Decimal::new(50, 0));
        assert_eq!(account.held, Decimal::ZERO);
        assert_eq!(account.total, Decimal::new(50, 0));
    }

    #[test]
    fn test_withdrawal_failure() {
        let mut account = Account::new(1);
        account.deposit(Decimal::new(100, 0)).unwrap();
        let result = account.withdraw(Decimal::new(150, 0)).unwrap_err();

        assert!(matches!(
            result.downcast_ref::<AccountError>(),
            Some(&AccountError::InsufficientAvailableFunds { available, requested })
            if available == Decimal::new(100, 0) && requested == Decimal::new(150, 0)
        ));
    }

    #[test]
    fn test_withdrawal_locked() {
        let mut account = Account::new(1);
        account.deposit(Decimal::new(100, 0)).unwrap();
        account.locked = true;
        let result = account.withdraw(Decimal::new(50, 0)).unwrap_err();

        assert!(matches!(
            result.downcast_ref::<AccountError>(),
            Some(&AccountError::AccountLocked)
        ));
    }

    #[test]
    fn test_successful_dispute() {
        let mut account = Account::new(1);
        account.deposit(Decimal::new(100, 0)).unwrap();
        account.dispute(Decimal::new(50, 0)).unwrap();

        assert_eq!(account.available, Decimal::new(50, 0));
        assert_eq!(account.held, Decimal::new(50, 0));
        assert_eq!(account.total, Decimal::new(100, 0));
    }

    #[test]
    fn test_dispute_locked() {
        let mut account = Account::new(1);
        account.deposit(Decimal::new(100, 0)).unwrap();
        account.locked = true;
        account.dispute(Decimal::new(50, 0)).unwrap();

        assert_eq!(account.available, Decimal::new(50, 0));
        assert_eq!(account.held, Decimal::new(50, 0));
        assert_eq!(account.total, Decimal::new(100, 0));
    }

    #[test]
    fn test_negative_available_funds() {
        let mut account = Account::new(1);
        account.deposit(Decimal::new(100, 0)).unwrap();
        account.dispute(Decimal::new(150, 0)).unwrap();

        assert_eq!(account.available, Decimal::new(-50, 0));
        assert_eq!(account.held, Decimal::new(150, 0));
        assert_eq!(account.total, Decimal::new(100, 0));
    }

    #[test]
    fn test_successful_resolve() {
        let mut account = Account::new(1);
        account.deposit(Decimal::new(100, 0)).unwrap();
        account.dispute(Decimal::new(50, 0)).unwrap();
        account.resolve(Decimal::new(30, 0)).unwrap();

        assert_eq!(account.available, Decimal::new(80, 0));
        assert_eq!(account.held, Decimal::new(20, 0));
        assert_eq!(account.total, Decimal::new(100, 0));
    }

    #[test]
    fn test_resolve_locked() {
        let mut account = Account::new(1);
        account.deposit(Decimal::new(100, 0)).unwrap();
        account.dispute(Decimal::new(50, 0)).unwrap();
        account.locked = true;
        account.resolve(Decimal::new(20, 0)).unwrap();

        assert_eq!(account.available, Decimal::new(70, 0));
        assert_eq!(account.held, Decimal::new(30, 0));
        assert_eq!(account.total, Decimal::new(100, 0));
    }

    #[test]
    fn test_insufficient_held_funds() {
        let mut account = Account::new(1);
        account.deposit(Decimal::new(100, 0)).unwrap();
        account.dispute(Decimal::new(50, 0)).unwrap();
        let result = account.resolve(Decimal::new(60, 0)).unwrap_err();

        assert!(matches!(
            result.downcast_ref::<AccountError>(),
            Some(&AccountError::InsufficientHeldFunds { held, requested })
            if held == Decimal::new(50, 0) && requested == Decimal::new(60, 0)
        ));
    }
}
