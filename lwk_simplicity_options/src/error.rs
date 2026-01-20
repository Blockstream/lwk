/// Errors that occur during Simplicity program compilation, execution, or environment setup.
///
/// These errors cover the full lifecycle of working with Simplicity programs:
/// loading source, satisfying witnesses, running on the Bit Machine, and
/// validating transaction environments.
#[derive(Debug, thiserror::Error)]
pub enum ProgramError {
    #[error("Failed to compile Simplicity program: {0}")]
    Compilation(String),

    /// Returned when witness values cannot satisfy the program's requirements.
    #[error("Failed to satisfy witness: {0}")]
    WitnessSatisfaction(String),

    /// Returned when the program cannot be pruned against the transaction environment.
    #[error("Failed to prune program: {0}")]
    Pruning(#[from] simplicityhl::simplicity::bit_machine::ExecutionError),

    #[error("Failed to construct a Bit Machine with enough space: {0}")]
    BitMachineCreation(#[from] simplicityhl::simplicity::bit_machine::LimitError),

    #[error("Failed to execute program on the Bit Machine: {0}")]
    Execution(simplicityhl::simplicity::bit_machine::ExecutionError),

    #[error("UTXO index {input_index} out of bounds (have {utxo_count} UTXOs)")]
    UtxoIndexOutOfBounds {
        input_index: usize,
        utxo_count: usize,
    },

    /// Returned when the UTXO's script does not match the expected program address.
    #[error("Script pubkey mismatch: expected hash {expected_hash}, got {actual_hash}")]
    ScriptPubkeyMismatch {
        expected_hash: String,
        actual_hash: String,
    },

    #[error("Input index exceeds u32 maximum: {0}")]
    InputIndexOverflow(#[from] std::num::TryFromIntError),
}
