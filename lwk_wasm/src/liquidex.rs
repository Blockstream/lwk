use std::str::FromStr;

use lwk_wollet::{Unvalidated, Validated};
use wasm_bindgen::prelude::*;

use crate::{AssetId, Pset, Transaction};

/// LiquiDEX swap proposal
///
/// A LiquiDEX swap proposal is a transaction with one input and one output created by the "maker".
/// The transaction "swaps" the input for the output, meaning that the "maker" sends the input and
/// receives the output.
/// However the transaction is incomplete (unbalanced and without a fee output), thus it cannot be
/// broadcast.
/// The "taker" can "complete" the transaction (using [`crate::TxBuilder::liquidex_take()`]) by
/// adding more inputs and more outputs to balance the amounts, meaning that the "taker" sends the
/// output and receives the input.
#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct UnvalidatedLiquidexProposal {
    inner: lwk_wollet::LiquidexProposal<Unvalidated>,
}

/// Created by validating `UnvalidatedLiquidexProposal` via `validate()` or `insecure_validate()`
#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct ValidatedLiquidexProposal {
    inner: lwk_wollet::LiquidexProposal<Validated>,
}

/// An asset identifier and an amount in satoshi units
#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct AssetAmount {
    inner: lwk_wollet::AssetAmount,
}

impl std::fmt::Display for ValidatedLiquidexProposal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", &self.inner)
    }
}

impl std::fmt::Display for UnvalidatedLiquidexProposal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", &self.inner)
    }
}

impl FromStr for UnvalidatedLiquidexProposal {
    type Err = serde_json::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let inner = lwk_wollet::LiquidexProposal::from_str(s)?;
        Ok(UnvalidatedLiquidexProposal { inner })
    }
}

#[wasm_bindgen]
impl UnvalidatedLiquidexProposal {
    pub fn new(s: &str) -> Result<Self, crate::Error> {
        let inner = lwk_wollet::LiquidexProposal::from_str(s)?;
        Ok(UnvalidatedLiquidexProposal { inner })
    }

    #[wasm_bindgen(js_name = fromPset)]
    pub fn from_pset(pset: Pset) -> Result<Self, crate::Error> {
        let pset = pset.into();
        let proposal = lwk_wollet::LiquidexProposal::from_pset(&pset)?;
        Ok(Self { inner: proposal })
    }

    #[wasm_bindgen(js_name = insecureValidate)]
    pub fn insecure_validate(self) -> Result<ValidatedLiquidexProposal, crate::Error> {
        let inner = self.inner.insecure_validate()?;
        Ok(ValidatedLiquidexProposal { inner })
    }

    pub fn validate(self, tx: Transaction) -> Result<ValidatedLiquidexProposal, crate::Error> {
        let inner = self.inner.validate(tx.into())?;
        Ok(ValidatedLiquidexProposal { inner })
    }

    #[wasm_bindgen(js_name = toString)]
    pub fn to_string_js(&self) -> String {
        format!("{self}")
    }
}

impl From<ValidatedLiquidexProposal> for lwk_wollet::LiquidexProposal<Validated> {
    fn from(value: ValidatedLiquidexProposal) -> Self {
        value.inner
    }
}

#[wasm_bindgen]
impl AssetAmount {
    pub fn amount(&self) -> u64 {
        self.inner.amount
    }

    pub fn asset(&self) -> AssetId {
        self.inner.asset.into()
    }
}

#[wasm_bindgen]
impl ValidatedLiquidexProposal {
    pub fn input(&self) -> AssetAmount {
        let inner = self.inner.input();
        AssetAmount { inner }
    }

    pub fn output(&self) -> AssetAmount {
        let inner = self.inner.output();
        AssetAmount { inner }
    }

    #[wasm_bindgen(js_name = toString)]
    pub fn to_string_js(&self) -> String {
        format!("{self}")
    }
}
#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use serde_json::Value;
    use std::str::FromStr;
    use wasm_bindgen_test::*;

    use super::UnvalidatedLiquidexProposal;
    use crate::{Network, TxBuilder, Wollet, WolletDescriptor};

    #[wasm_bindgen_test]
    fn test_liquidex_proposal() {
        let proposal_str = r#"
        {
            "version": 1,
            "tx": "020000000101f4475ec862d28d1070669b04194ad6231a3d2d82ee6e2f7c13b36a8212b5e3c30100000017160014fc00fbb07fd444bcb8047462f254094abbbd473dfeffffff010ba4b2b1d6fe2413ae6e52c735d0915f9970b61eeb9e2fbc8e037e3afaee6b67f409df6081d0481c20496a06e9b11a7c6d2e25197d1f29021ee5599f2ad17cdefea90397b5f87779df2794ac377f24bf2e250dcc18deba748ef53745757fb4040b924d17a9143bc057e5c67a79fe63275ca75b51ba745537b9a98700000000000002483045022100fa178538555d6d70b023611326d7759ed567e037aa5ca3897db90d744e2c24db02206c11231f6f98c01ef933c3097877956e6acd96e932a0a3d1bfb578e51f0f5823832102095beb25637d96dca562e725f6e9bf04e7a7dbd55b3c00bbd062a853fe57a4a10000fd4e10603300000000000000016c25c8018bb6a994544ba1e9ed92ead831670490648e7e726cd66357fc893933695a50f8328d8e967ee4988d2e10e5c2f98856eb5ce3c4e3b892593b85ef36d96b938066ce8f5b7742d8564a47dcf69e534d7d40d02762c6db015e5291434fc7765519480ae6149b273ef8bb9454f50281208535d3cbf7b0c29ef8d94e16a9b6bf3710ac51b3f9552687c719aba3a00d52789158225d3e7776d8e4001d914d5a926dbdef6eb34d1f7cd31ccfbea5151790d93ae0b5f3264ed0196a67cbd1fe2335eafd470014929920af971316fb06a83c059dc366cdb1aa9dd535a70aa257fd7ba9a67af43463bedfdf45c99845328e7149389f0dc8d470ab1120bb809f57244bd964974f0f6d13dd1aab2c6ce6276e891e48d5101d4aaeef57fa20ad10d255b4295b38886e6e089a1388af1ad75af176a05cb2fdb66a07f4692fb35d6f0a89b903d180317c011702e10b5c829b27c91fbee0d8342df7375ff725353ebf4804076f0553864cb1762e80f3eed98a6b78abaa336a4efa8273a286a0241e8e60462ee6cce6950af3e9e0bfdc2c28f486fd3f8ebe98196505e2a80e7241c1bb1078b15e6e10e833d8050caf91ef6f754feb50daacedb2a88ea5298af85cef44c19963b7a0fb23fd1ccf2c7b47135fd5d0c601fe200768cafffe3937675297ae8ef1ad19057bc7828e326ed9b2ee4925d00e0397e6076eaa194d7642f61589b6ce64d70836eb59d7467b8e90ef72f6496df39cfd19b793b5a52afa3f24da992403be03bbf5ce68aff53d84c305fb14c0071fb3f4bca1a3351632d8bee0872e14a2d4b4d30716f299d1e2692cfa663fc71d24f2f261264e615d4d44c068385f83b7fcb45433201fb35fc5c411cd14b7aa2ade2559c5a252e78b8891bca4d6372114fa57bf3d0165313ac5764f39c3c04849d10d7821e35780d5fc4bebcb271963c24b893ddf5d4c42251257a43ef06a4d5364478272e9d8345a298020307cb61a0cb933a3fce56a68e6a6dbd5dcbc3e0893b904360f8c28e7d4bf7e11a922c5c2cb3d8c97c7e5a2e6c6d513a1e0d5e2151486e9a5bfbc2a2d6cb5850da716461f50a01f83084b2022184dca5d1f555dd454084e0a37ab3d21dbe85033dc24bb3f0c51b2a8a021e6c4f34114cc90cd233bc35b8b5cefc59110f32ed180c42febd34b656fec85328235493eb3eabab5babac7e848ce1549b6e17d41eaaa93758ef21fe68c824227d3be38417fb4331ec04ee8d2e68b341b2d49f421c6a69de5f109347b720f965cf87f7b7393fd85db823f03b693aad6c7e168dd7ec07bbf67c24793acfead485152a59b39557ac775e1b3dc62276fb73e0e3a2bc315e98c15374f0062ba3642793269932cd2515a31a48ba6620abaeee1d4edd654169a5bc200069b624de5004db39cfb41946b3bc13f6b41ac15f0ae22d4c3ea1c0d611fa127dc8b66954e0319683ccd91ac0846ba1475dd63a17e049990e47119e85993be4d679bcd8ba4e39c784d0a9fa31e294cf58b4ddac96f3c4d905a3e6c9c7cbc6050db01a76f2938dccebeefff17b1e1492fefc728c358fb220145348d46deb545e400590ee1c940a75c52c2689c03d2335c18a4144f396b12d3ff624adec9d34dd7a9551d4e3a41c3911844fb2e7e379a08d3628dcbfbe240573ced0e87d438f10cb7f531ca16f86e7be5a60dca355623778b4db1b15f380256528d9bbe1953b5338d15a0253a6ef3844de7b5b61a7b358c3de936303bcec9424f72ac6611a1f3d561b8b407c8dccaaf10552303d1fdb6d99341ecb5080ce2a4d46f8c357472c34153b8607f4334c0140a607a903cbfedeacf7bc4b763764294d6538c97f3bdf4c96352a4fbea5af713bc3912747c7d263bd02c4a4362c4b9350c9363a56f710050b91b09798132782d4fd46d48c80bbe917eff83caebd8e4cba0c2e5e69f0d5646bf056e42f0457cabeebaddc5ba47296d030172f7abfe0ccc74bdde0bb11fe7e54b5777ef113281b084e206da9da7867c7b4aff587b3c6c6beb8ba422ba672afb18d1d84f8f28affb1a9891a6b4f5c267b98f02225bf732fb6a6813ee782b757e4fc2a83ed0bf5eb1f6fca00049493103b01afd767a6b12fdc66cf0ae18833d1439550b44a6543ab13d9ec9ddbb3fcd343a144538bb0aea6fccadf1415c3bbe4ba2a4250c1a658cfd01c64de8ce855362362d3a12a07b1da62f0a300a20f6cb85f8fece8bdbb020d5a0122dc32a46a694d24b10e2340ff92440dfc57123f76e6ecacd5b21ef20c59cfa1a2885976246c9c22055e22487f0b0b14c1a7270d7bdf0c26517a30328072ec46d55d927a214fe3a31de14a33fab59f2ec61c25ddce934817e3a8dd499576ef9e59e82f20fd37fabe7d2ed59ee2f4541ad6826e025792a4c104af408a3f84c417eddfdab2ed0957ddca28801b750c663ceb1d3756032b2e3714f79c4cc8e42d5bc38c1b1cc2294a522a331ebd382dbf1a4bd62f6e2b31fc2e77a2ddd25812f8796294495ed7e5f6c23a27e9454a021a59dae2c2852191b06b7d0d2d744ec36c769e7de3e40df2daac4fcc2c8e2e2ea32dd67ac10754d2094b734f7c7ba3d07668beb964e6b0ba465fe79d88ec1c7af855d0bcc327f9c8b44657ca8acea8b17635fb5f6064cb008a2f56aaaeb0968de9a228d7a792e07de45dfd6011cf895834689359d1ed8e98bfacb7ecc0745787872096fe99bb1c60da92d71f9b1bee1dae439d897b7bc460d166f24c2dee0f708092f9b46b2cdd3769532b2d170f5ca549ce9fdaa604fa947c1daecd9aef9a53b529d5fa329a0938c6d5fc137b295d04b8de44e7c89620f25b36db3ef7618f765e280f60c08331a8c0180ad5ed3adf06dd801ec534dfe08ed3b49531eabef2a7785d042f851429aea0215dc528d47beb543794672a2d8a692c87c1e9de20b78864b320b0fad9a2edc4eecaa09cfefb8634432088066aa2438a0821a421b568e0d41f5e388c03a456c2135d8057e6887adb3b295c3a46d975b71ec9361537dd5cead13edbf30dd5d67024609388a70735a385e041bb838a4f087e550b8aaf990ca18983aeebb9be0c21aad696097d5222fa19de998a325e0cb4c9cde204a2c22c5ef663b4dbca9fec95289226ddfb31e0641a873ed13382f1dbd56f8e42386ae3b38ca869e9b6d3248ff1ba093ab364a81bc920aaf4d835c5815a2b0942a608be946263e61f6da9e84aa33a199a321d24f1a241ac36c0f41cc3c46fc5d203809cf7c0fa53ad4e18b7c22451b9850ecab8716925d32df49cde2c8d7d3d9220fdc35e1b23ba31c26e1d7f4007d82c02cb7b414b139df3735381d191eac987a0f54f5ed1dc59979dcca6ec0e2b7decf439599488217e05b0076fb39371dc1d4f5777101c3462211610e4e440a19f40a778c684d462208f68db7f06156419d60e28dfc86ea026f443d9e7d811260a1f25cedd9449e0a8bf07c35599fc947062286b1d34f6910e54ec73c47ce8981bf3e57d48fde2ceae92f92a8a125831ac6b2bfdb0528f6f3930c08894df73589993715ec14ae42c282fa60949bdaa9a67a1869cdefd9b08fa17875e62a214ed6f0560922df7fa64d8abea31b719c0260e3a8082317ef4e70a25c00a21cfba05bf68bbe857959d541617bf200228a270da04494999d5d3d052bbff927c8964f2bdeff2dd2936b8716c12974e2d565c3b8418b1a5d47a535f1d3f5c6346d92c4a2077a84167892bd9fe0d91ba3d3be65bb965bfcae01ecce556c2551cb17b64ffa887310a167c29e44e702b178e067dfe54d219dd03e9eb1aeef1bb37260995987dc4fcdb26840a1453eabd09fe198bc365538b937a0cbc77f2dce0d28fa9f06836f9aadd0619554ced3299b1232bf0be9349474979cdd72fa9a34bd15597b9ef68f3b1f8a609ae4f226c09b2b82b0054b96fd2559530a64f1c0dffd60dc1509a6044a7b518550341fbf7a8929b995863717c99a0531a837d96f8cd1fc0c49de90f113593990bf69d748c84119f4d8ee4995b7442b0a0944f077b8df43d185625a15d81b1a6341c0d5985c82e3bccaade8331e6c18b3659231e1dacd553223b45009a880ae6233b1694c2a96b559be11a15b3a42762b50931672f772836ce86cf5fce0eb2697d1cd13bc51ff6722203096b00c1f5a97a1ce8566b4f34bb675001858fbc83f1dd05a9486212c9a5043ca1caa2aae70d12eea403fc572e5c54b668097132d286565479a534abc6849c6def484c6ceec191bf89b3a797793fdb0ded2ef3d947f573499415c13c93c2363f9e436d6da81a57690286183dac50b7c5fdb363982675b4755a25e308eb69523bab1f8db3fc22583be3b5aa45678b92fe7348e8789a2ab9d22efc18d23ea15a0bae311ffcad660ddb66b7b0dbb1654d6e3e0752a7a3849ead651d72cbb232ff51083b01900ef98734f3eabf596d8e1df35fa2a9bafa3bc900a6dd279fd806fea003aea40d4209229f93bb5720792c982d3101b1f54d1f873b83b2d75eebf89c428084a0b993bebacc999af90761ebe8995ba4f18be0ef0e0ccfb0e9a13e5352c46efba77e167ea16c6466e3aafb67d8b3d8e4c92c7790409484eb46491fbabb406d85f4220ce111fb4e5a581f327b1d8c3691551896050bd2e2a5e9fa17a52a5ecd9570c83d9fde2ad17c4679f1d7d5140f40ef54deac80c602aaf6694229ad13e8684ee322c629db011d1a977ad39f001b9ff0d9ba21d82887996a2d343d933b0997ef73a6a73e01254b5c56cb4781e2929e786579dba75f5fd9194450bd6eeccc96b295a2f6dc8f13f02a09d86113de7b05f6c9df5a2c603bb2f6023c4d4ae466ae558e919bc262508c329a39c6a8604ca0edc7338c0ead05fdabe960c033e8e4054724caf58c7f025ccdd9372e36d462195e27fba99d7ea56bc28e10e1938e617142fcaf5a9e0489bbf24f23b732311c593112ae9d88e3785e4c10553dd041b490185d01299dbb1ef0c33f3c307b116ad4c18a7d08d977730389fdfa5e82784b97a7596e19ed4e45db4ec36770503604afb2074ad9e809a0003d670a1bc34574fbc0cffdfdc08d3a9156ff5251b2d5ae471a73662ca175429404611601cdf8270daccd5dc831396f0a038c0fb2f828aa11212c470df3f738a482ccb6d5e0e6be20ffe87ec9317c2d8c0117dc82893e3e63deaaa2985e9e83672641f61320634a4a145cd6f8b8333de304c26b660437d08c9b368dad209cde46d9f5da8ce20de97a8e183e2ddcdb5736135dcd912602eca3d85b0573551b6a38ade0926487a58b8e66b41e7bbd3b7671901d4b8e82d25c0357f9485ad30d32ed5bfe1b0bab76a50e92acc53c50f3717e980b60a6431ab65afdf9e7afdf3e93d8deec8fb4fdf9e8071cb65da2bb91d415892bb59035db6486258308bfe623347607386e029aaf45f92fd9830df21752929c774a804176c3987ed27f6ff37ebccb7af8f989356e9959484302ca486214ba25c60ad0ec8e7e9629419d54d9b31ccf547452ac8a3ee1d9f5c51e98a55c4d5b3a7ad91aa2486b7689e4e8cf483bd3d64143422b61005fc2d52e7482c606969a611a340b0bfdde3d1ef15ae0a8ac6acb2e467bd63b485abf64cc953acb042201a473c3506a9763113e638ef76706ffa467a527a70ce70f7bdaee1332e61acd0afb226e67366d76952a3253af6da9529a4d5f4a8876a8a125799485221ace7fcf96694f4757109f955f33c7242863a1ccc270c69b67b7bacc81b34fcd6a8621c07925ad17c8d5836abf0b0c8cd89b0939aa5b0cd2158a9807bd69fcc23a89b49eb30baa61be977ceeb131f0d18a1e8b03dc584a97e09c242d6a4766592fbb2a93d017b4d766c202dae5885de8c6b990c8ccecaee429235950bcad7ce2d93b734d7e1f04a6f79954a23c6ec33",
            "inputs": [{
                "asset":"6921c799f7b53585025ae8205e376bfd2a7c0571f781649fb360acece252a6a7",
                "asset_blinder":"5bfa3c033ed871572f3012eda1d2c9ae2ec662be07ed267a09f1b44c06eee86f",
                "satoshi":10000,
                "blind_value_proof":"2000000000000027101988645f8abc1ef9086b5d8e4c41c0e42475b412fc22245779f597007c5f48cb9c61ff6535866ef5150b326bf69c0d8c291f8c8e6afc015311ed405b74baf9d4"
            }],
            "outputs": [{
                "asset":"f13806d2ab6ef8ba56fc4680c1689feb21d7596700af1871aef8c2d15d4bfd28",
                "asset_blinder":"1e8c347f88a3e46e97a9a6556710812137a10b939983aef8a8f95da466abfae5",
                "satoshi":10000,
                "blind_value_proof":"200000000000002710c833ef127398975f30746b9ba1774f982d82dc658926f7d41fd52ff6cc34bc2282b168c578e337f667ad4c567ed4c39744021fc25486df3013ff2aa8b5b88a74"
            }],
            "scalars": [
                "95e27208708a7b57e0ec3b562bbe0dd56548bb9d0359d7f9a8604dfd9ec02eba"
            ]
        }"#;
        let proposal_json: Value = serde_json::from_str(proposal_str).unwrap();
        let proposal = UnvalidatedLiquidexProposal::from_str(proposal_str).unwrap();
        let proposal = proposal.insecure_validate().unwrap();
        let proposal_back_json: Value = serde_json::from_str(&proposal.to_string()).unwrap();
        assert_eq!(proposal_json, proposal_back_json);
    }

    #[wasm_bindgen_test]
    fn test_tx_builder_liquidex() {
        let network = Network::testnet(); // but offline with pre-loaded static update

        let mnemonic = crate::Mnemonic::new(include_str!(
            "../test_data/update_with_mnemonic/mnemonic.txt"
        ))
        .unwrap();
        let descriptor = include_str!("../test_data/update_with_mnemonic/descriptor.txt");
        let update_base64 =
            include_str!("../test_data/update_with_mnemonic/update_serialized_encrypted.txt");

        let signer = crate::Signer::new(&mnemonic, &network).unwrap();
        let descriptor = WolletDescriptor::new(&descriptor).unwrap();
        let mut wollet = Wollet::new(&network, &descriptor).unwrap();
        let update =
            crate::Update::deserialize_decrypted_base64(update_base64, &descriptor).unwrap();
        wollet.apply_update(&update).unwrap();

        let utxos = wollet.utxos().unwrap();
        let addr = wollet.address(None).unwrap().address();

        assert!(!utxos.is_empty());
        let utxo = utxos[0].outpoint();
        let wanted_asset =
            crate::AssetId::new("38fca2d939696061a8f76d4e6b5eecd54e3b4221c846f24a6b279e79952850a5")
                .unwrap();

        let builder = TxBuilder::new(&network);
        let pset_maker = builder
            .liquidex_make(utxo, addr, 1, wanted_asset)
            .unwrap()
            .finish(&wollet)
            .unwrap();

        let signed_pset_maker = signer.sign(pset_maker).unwrap();

        // TODO How do lwk_wasm users securely extract amounts and assets from the proposal?
        let proposal = UnvalidatedLiquidexProposal::from_pset(signed_pset_maker).unwrap();
        let proposal = proposal.insecure_validate().unwrap();

        let input = proposal.input();
        let output = proposal.output();

        assert_eq!(input.amount(), 100000);
        assert_eq!(input.asset(), network.policy_asset());
        assert_eq!(output.amount(), 1);
        assert_eq!(output.asset(), wanted_asset);

        let mnemonic = crate::Mnemonic::new(include_str!(
            "../test_data/update_with_mnemonic/mnemonic2.txt"
        ))
        .unwrap();
        let descriptor = include_str!("../test_data/update_with_mnemonic/descriptor2.txt");
        let update_base64 =
            include_str!("../test_data/update_with_mnemonic/update_serialized_encrypted2.txt");

        let signer = crate::Signer::new(&mnemonic, &network).unwrap();
        let descriptor = WolletDescriptor::new(&descriptor).unwrap();
        let mut wollet = Wollet::new(&network, &descriptor).unwrap();
        let update =
            crate::Update::deserialize_decrypted_base64(update_base64, &descriptor).unwrap();
        wollet.apply_update(&update).unwrap();

        let builder = TxBuilder::new(&network);
        let pset_taker = builder
            .liquidex_take([proposal].to_vec())
            .unwrap()
            .finish(&wollet)
            .unwrap();
        let signed_pset_taker = signer.sign(pset_taker).unwrap();

        let net_balance = wollet.pset_details(&signed_pset_taker).unwrap().balance();

        assert_eq!(format!("{:?}", net_balance), "PsetBalance { inner: PsetBalance { fee: 53, balances: SignedBalance({144c654344aa716d6f3abcc1ca90e5641e4e2a7f633bc09fe3baf64585819a49: 99947, 38fca2d939696061a8f76d4e6b5eecd54e3b4221c846f24a6b279e79952850a5: -1}), recipients: [Recipient { address: Some(tex1q0w8dczxn0rkt3jjz4ktfuu9gs5dz2p4nmhe83j), asset: Some(38fca2d939696061a8f76d4e6b5eecd54e3b4221c846f24a6b279e79952850a5), value: Some(1), vout: 0 }] } }");
    }
}
