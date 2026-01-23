import { describe, it, expect, beforeAll } from 'vitest';
import { readFile } from 'fs/promises';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

import initWasm from '../src/generated/wasm-bindgen/index.js';
import * as lwk from '../src/generated/lwk';

describe('LWK WASM Bindings', () => {
  beforeAll(async () => {
    const wasmPath = join(__dirname, '../src/generated/wasm-bindgen/index_bg.wasm');
    const wasmBuffer = await readFile(wasmPath);
    await initWasm(wasmBuffer);
    lwk.default.initialize();
  });

  describe('Network', () => {
    it('should create mainnet network', () => {
      const network = lwk.Network.mainnet();
      expect(network).toBeDefined();
      expect(network.isMainnet()).toBe(true);
      expect(network.toString()).toBe('liquid');
    });

    it('should create testnet network', () => {
      const network = lwk.Network.testnet();
      expect(network).toBeDefined();
      expect(network.isMainnet()).toBe(false);
      expect(network.toString()).toBe('liquid-testnet');
    });

    it('should create regtest network with default policy asset', () => {
      const network = lwk.Network.regtestDefault();
      expect(network).toBeDefined();
      expect(network.isMainnet()).toBe(false);
      expect(network.toString()).toBe('liquid-regtest');
    });

    it('should return policy asset for mainnet', () => {
      const network = lwk.Network.mainnet();
      const policyAsset = network.policyAsset();
      expect(policyAsset).toBeDefined();
      expect(policyAsset).toBe(
        '6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d'
      );
    });

    it('should return genesis block hash', () => {
      const network = lwk.Network.mainnet();
      const genesisHash = network.genesisBlockHash();
      expect(genesisHash).toBeDefined();
      expect(typeof genesisHash).toBe('string');
      expect(genesisHash.length).toBe(64);
    });

    it('should create TxBuilder for network', () => {
      const network = lwk.Network.testnet();
      const txBuilder = network.txBuilder();
      expect(txBuilder).toBeDefined();
    });
  });

  describe('Mnemonic', () => {
    it('should generate random 12-word mnemonic', () => {
      const mnemonic = lwk.Mnemonic.fromRandom(12);
      expect(mnemonic).toBeDefined();
      expect(mnemonic.wordCount()).toBe(12);

      const words = mnemonic.toString().split(' ');
      expect(words.length).toBe(12);
    });

    it('should generate random 24-word mnemonic', () => {
      const mnemonic = lwk.Mnemonic.fromRandom(24);
      expect(mnemonic).toBeDefined();
      expect(mnemonic.wordCount()).toBe(24);

      const words = mnemonic.toString().split(' ');
      expect(words.length).toBe(24);
    });

    it('should parse valid mnemonic string', () => {
      const validMnemonic = 'abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about';
      const mnemonic = new lwk.Mnemonic(validMnemonic);
      expect(mnemonic).toBeDefined();
      expect(mnemonic.wordCount()).toBe(12);
      expect(mnemonic.toString()).toBe(validMnemonic);
    });

    it('should throw error for invalid mnemonic', () => {
      expect(() => {
        new lwk.Mnemonic('invalid mnemonic words that do not work');
      }).toThrow();
    });
  });

  describe('Address', () => {
    it('should parse valid mainnet address', () => {
      const addressStr = 'VJLDwMVWXg8RKq4mRe3YFNTAEykVN6V8x5MRUKKoC3nfRnbpnZeiG6CgQyiB4ER4JGU4ZL3pHPNji8dj';
      const address = new lwk.Address(addressStr);
      expect(address).toBeDefined();
      expect(address.toString()).toBe(addressStr);
    });

    it('should throw for invalid address', () => {
      expect(() => {
        new lwk.Address('invalid-address');
      }).toThrow();
    });
  });

  describe('AssetId', () => {
    it('should be a string type alias', () => {
      const assetId: lwk.AssetId = '6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d';
      expect(assetId).toBeDefined();
      expect(typeof assetId).toBe('string');
      expect(assetId.length).toBe(64);
    });

    it('should match network policy asset', () => {
      const network = lwk.Network.mainnet();
      const policyAsset: lwk.AssetId = network.policyAsset();
      expect(policyAsset).toBe('6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d');
    });
  });

  describe('Signer', () => {
    it('should create signer from mnemonic', () => {
      const mnemonic = lwk.Mnemonic.fromRandom(12);
      const network = lwk.Network.testnet();
      const signer = new lwk.Signer(mnemonic, network);
      expect(signer).toBeDefined();
    });

    it('should generate wpkh slip77 descriptor', () => {
      const mnemonic = new lwk.Mnemonic(
        'abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about'
      );
      const network = lwk.Network.testnet();
      const signer = new lwk.Signer(mnemonic, network);

      const descriptor = signer.wpkhSlip77Descriptor();
      expect(descriptor).toBeDefined();
      expect(descriptor.toString()).toContain('ct(slip77');
    });
  });
});
