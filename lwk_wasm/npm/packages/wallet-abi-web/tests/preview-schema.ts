import {
  WalletAbiPreviewAssetDelta,
  WalletAbiPreviewOutput,
  WalletAbiPreviewOutputKind,
  WalletAbiRequestPreview,
} from "lwk_wallet_abi_web";
import { WalletAbiRequestPreview as WalletAbiRequestPreviewFromSchema } from "lwk_wallet_abi_web/schema";

const previewCtor: typeof WalletAbiRequestPreviewFromSchema =
  WalletAbiRequestPreview;
const previewKind: WalletAbiPreviewOutputKind =
  WalletAbiPreviewOutputKind.Receive;

void previewCtor;
void previewKind;
void WalletAbiPreviewAssetDelta;
void WalletAbiPreviewOutput;
