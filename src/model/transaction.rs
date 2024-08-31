use rust_decimal::Decimal;
use thiserror::Error;

/// Type alias for transaction identifiers.
pub type TxId = u32;

/// Represents the kind of a transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionKind {
    /// Deposit the given amount.
    Deposit(Decimal),

    /// Withdraw the given amount.
    Withdrawal(Decimal),

    /// Dispute the given transaction.
    Dispute(TxId),

    /// Resolve a dispute. The identifier refers to a transaction that was under
    /// dispute by ID.
    Resolve(TxId),

    /// Chargeback a transaction. The identifier refers to a transaction that was
    /// under dispute by ID.
    Chargeback(TxId),
}

/// Error type for transaction kind creation.
#[derive(Debug, Clone, Error)]
pub enum TransactionKindError {
    /// Amounts for transactions must be positive.
    #[error("Transaction amount must be strictily positive ({0} given)")]
    NegativeOrZeroAmount(Decimal),
}

impl TransactionKind {
    /// Create a new deposit transaction.
    ///
    /// ```
    /// use rust_decimal::Decimal;
    /// use rust_decimal_macros::dec;
    /// use csv_reader::model::{TransactionKind, TransactionKindError};
    ///
    /// // create a deposit transaction
    /// let deposit = TransactionKind::deposit(dec!(0.0001)).unwrap();
    ///
    /// // amounts of zero or less are not allowed
    /// let error = TransactionKind::deposit(Decimal::ZERO).unwrap_err();
    /// assert!(matches!(error, TransactionKindError::NegativeOrZeroAmount(value) if value == Decimal::ZERO));
    ///
    /// let error = TransactionKind::deposit(dec!(-0.0001)).unwrap_err();
    /// assert!(matches!(error, TransactionKindError::NegativeOrZeroAmount(value) if value == dec!(-0.0001)));
    /// ```
    pub fn deposit(amount: Decimal) -> Result<Self, TransactionKindError> {
        Ok(Self::Deposit(Self::check_positive_amount(amount)?))
    }

    /// Create a new withdrawal transaction.
    ///
    /// ```
    /// use rust_decimal::Decimal;
    /// use rust_decimal_macros::dec;
    /// use csv_reader::model::{TransactionKind, TransactionKindError};
    ///
    /// // create a withdrawal transaction
    /// let withdrawal = TransactionKind::withdrawal(dec!(0.0001)).unwrap();
    ///
    /// // amounts of zero or less are not allowed
    /// let error = TransactionKind::withdrawal(Decimal::ZERO).unwrap_err();
    /// assert!(matches!(error, TransactionKindError::NegativeOrZeroAmount(value) if value == Decimal::ZERO));
    ///
    /// let error = TransactionKind::withdrawal(dec!(-0.0001)).unwrap_err();
    /// assert!(matches!(error, TransactionKindError::NegativeOrZeroAmount(value) if value == dec!(-0.0001)));
    /// ```
    pub fn withdrawal(amount: Decimal) -> Result<Self, TransactionKindError> {
        Ok(Self::Withdrawal(Self::check_positive_amount(amount)?))
    }

    /// Create a new dispute transaction.
    ///
    /// ```
    /// use csv_reader::model::TransactionKind;
    ///
    /// // create a dispute transaction
    /// let dispute = TransactionKind::dispute(1);
    /// assert_eq!(dispute, TransactionKind::Dispute(1));
    /// ```
    pub fn dispute(tx_id: TxId) -> Self {
        Self::Dispute(tx_id)
    }

    /// Check if the given amount is strictly positive.
    fn check_positive_amount(amount: Decimal) -> Result<Decimal, TransactionKindError> {
        if amount <= Decimal::ZERO {
            return Err(TransactionKindError::NegativeOrZeroAmount(amount));
        }

        Ok(amount)
    }

    /// Create a new resolve transaction.
    ///
    /// ```
    /// use csv_reader::model::TransactionKind;
    ///
    /// // create a resolve transaction
    /// let resolve = TransactionKind::resolve(1);
    /// assert_eq!(resolve, TransactionKind::Resolve(1));
    /// ```
    pub fn resolve(tx_id: TxId) -> Self {
        Self::Resolve(tx_id)
    }

    /// Create a new chargeback transaction.
    ///
    /// ```
    /// use csv_reader::model::TransactionKind;
    ///
    /// // create a chargeback transaction
    /// let chargeback = TransactionKind::chargeback(1);
    /// assert_eq!(chargeback, TransactionKind::Chargeback(1));
    /// ```
    pub fn chargeback(tx_id: TxId) -> Self {
        Self::Chargeback(tx_id)
    }
}
