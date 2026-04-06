use lwk_wollet::elements::{OutPoint, TxOutSecrets};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ResolvedInputMaterial {
    pub(crate) outpoint: OutPoint,
    pub(crate) secrets: TxOutSecrets,
}
