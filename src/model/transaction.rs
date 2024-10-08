use rust_decimal::Decimal;
use serde::Deserialize;
use thiserror::Error;

use super::ClientId;

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
    ChargeBack(TxId),
}

/// Error type for transaction kind creation.
#[derive(Debug, Clone, Error)]
pub enum TransactionKindError {
    /// Amounts for transactions must be positive.
    #[error("Transaction amount must be strictily positive ({0} given)")]
    NegativeOrZeroAmount(Decimal),

    /// The transaction kind is unknown.
    #[error("Unknown transaction kind: '{0}'")]
    UnknownKind(String),

    /// The transaction must have an amount.
    #[error("Transaction amount is missing")]
    MissingAmount,
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
    /// assert_eq!(chargeback, TransactionKind::ChargeBack(1));
    /// ```
    pub fn chargeback(tx_id: TxId) -> Self {
        Self::ChargeBack(tx_id)
    }
}

/// A Transaction represents a single transaction that happened on the exchange.
/// A Transaction has already modified the ledgers and it cannot be modified or
/// deleted. The transaction identifier is unique. Unexpected behavior can
/// happen if two different transactions have the same identifier.
/// If a transaction relates to another transaction, the identifier is valid and
/// the related transaction can be found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transaction {
    /// The unique identifier of the transaction.
    pub tx_id: TxId,

    /// The client identifier that made the transaction.
    pub client_id: ClientId,

    /// The transaction kind.
    pub kind: TransactionKind,
}

/// TransactionOrder represents the order of a transaction in the CSV file. It
/// is a wish emitted by a client that Transaction should be processed in the
/// given order. This transaction has not yet been validated against the account.
#[derive(Debug, Clone)]
pub struct TransactionOrder {
    /// The unique identifier of the transaction.
    pub tx_id: TxId,

    /// The client identifier that made the order.
    pub client_id: ClientId,

    /// The transaction kind.
    pub kind: TransactionKind,
}

impl From<TransactionOrder> for Transaction {
    fn from(order: TransactionOrder) -> Self {
        Self {
            tx_id: order.tx_id,
            client_id: order.client_id,
            kind: order.kind,
        }
    }
}

/// Transaction entity read from CSV file.
#[derive(Debug, Clone, Deserialize)]
pub struct CSVTransactionEntity {
    /// The transaction kind.
    pub r#type: String,

    /// The client identifier that made the transaction.
    pub client: ClientId,

    /// The unique identifier of the transaction.
    pub tx: TxId,

    /// The amount of the transaction.
    pub amount: Option<Decimal>,
}

impl TryFrom<CSVTransactionEntity> for TransactionOrder {
    type Error = TransactionKindError;

    fn try_from(entity: CSVTransactionEntity) -> Result<Self, Self::Error> {
        let kind = match entity.r#type.as_str().to_lowercase().as_str() {
            "deposit" => {
                if let Some(amount) = entity.amount {
                    TransactionKind::deposit(amount)?
                } else {
                    return Err(TransactionKindError::MissingAmount);
                }
            }
            "withdrawal" => {
                if let Some(amount) = entity.amount {
                    TransactionKind::withdrawal(amount)?
                } else {
                    return Err(TransactionKindError::MissingAmount);
                }
            }
            "dispute" => TransactionKind::dispute(entity.tx),
            "resolve" => TransactionKind::resolve(entity.tx),
            "chargeback" => TransactionKind::chargeback(entity.tx),
            val => return Err(TransactionKindError::UnknownKind(val.to_owned())),
        };

        Ok(Self {
            tx_id: entity.tx,
            client_id: entity.client,
            kind,
        })
    }
}
