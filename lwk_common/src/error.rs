use elements_miniscript::elements::OutPoint;

#[allow(missing_docs)]
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("There is no unblinding information and Input #{idx} is missing witness_utxo of outpoint {previous_outpoint}")]
    MissingPreviousOutput {
        idx: usize,
        previous_outpoint: OutPoint,
    },

    #[error("Input #{idx} has a pegin, but it's not supported")]
    InputPeginUnsupported { idx: usize },

    #[error("Input #{idx} has a blinded issuance, but it's not supported")]
    InputBlindedIssuance { idx: usize },

    #[error("Input #{idx} is not blinded")]
    InputNotBlinded { idx: usize },

    #[error("Input #{idx} belongs to the wallet but cannot be unblinded")]
    InputMineNotUnblindable { idx: usize },

    #[error(
        "Input #{idx} belongs to the wallet but its commitments do not match the unblinded values"
    )]
    InputCommitmentsMismatch { idx: usize },

    #[error("Output #{idx} has none asset")]
    OutputAssetNone { idx: usize },

    #[error("Output #{idx} has none value")]
    OutputValueNone { idx: usize },

    #[error("Output #{idx} has none value and none asset")]
    OutputAssetValueNone { idx: usize },

    #[error("Multiple fee outputs")]
    MultipleFee,

    #[error("Fee output is blinded")]
    BlindedFee,

    #[error("Output #{idx} has invalid asset blind proof")]
    InvalidAssetBlindProof { idx: usize },

    #[error("Output #{idx} has invalid value blind proof")]
    InvalidValueBlindProof { idx: usize },

    #[error("Output #{idx} is not blinded")]
    OutputNotBlinded { idx: usize },

    #[error("Output #{idx} belongs to the wallet but cannot be unblinded")]
    OutputMineNotUnblindable { idx: usize },

    #[error(
        "Output #{idx} belongs to the wallet but its commitments do not match the unblinded values"
    )]
    OutputCommitmentsMismatch { idx: usize },

    #[error("Private blinding key not available")]
    MissingPrivateBlindingKey,

    #[error(transparent)]
    DescConversion(#[from] elements_miniscript::descriptor::ConversionError),

    #[error(transparent)]
    Miniscript(#[from] elements_miniscript::Error),
}
