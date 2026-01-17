/**
 * Wallet Signing Hooks
 * Handles signing of spending approvals for different blockchains
 * 
 * Uses:
 * - Solana: @solana/web3.js for message signing
 * - Stellar: stellar-sdk for transaction signing
 * - NEAR: near-api-js for signing
 */

import { Chain } from '@/stores/useStore';

/**
 * Sign message with Solana wallet
 * Uses @solana/web3.js message signing format
 */
const signWithSolana = async (
  walletAddress: string,
  nonce: string
): Promise<string> => {
  try {
    // In production, use:
    // const { solana } = window as any;
    // if (!solana?.isPhantom) throw new Error('Phantom wallet not found');
    // const message = new TextEncoder().encode(nonce);
    // const { signature } = await solana.request({
    //   method: 'signMessage',
    //   params: { message }
    // });
    // return signature;

    // For now, simulate signing
    const encoder = new TextEncoder();
    const messageBytes = encoder.encode(nonce);
    const hashBuffer = await crypto.subtle.digest('SHA-256', messageBytes);
    const hashArray = Array.from(new Uint8Array(hashBuffer));
    const hashHex = hashArray.map(b => b.toString(16).padStart(2, '0')).join('');
    
    // Return as Base58 (Solana format)
    return base58Encode(Buffer.from(hashHex, 'hex'));
  } catch (error) {
    throw new Error(`Solana signing failed: ${error}`);
  }
};

/**
 * Sign message with Stellar wallet
 * Uses stellar-sdk transaction signing
 */
const signWithStellar = async (
  walletAddress: string,
  nonce: string
): Promise<string> => {
  try {
    // In production, use:
    // const response = await window.stellar.signTransaction(nonce, {
    //   networkPassphrase: StellarSDK.Networks.PUBLIC_NETWORK
    // });
    // return response.signature;

    // For now, simulate signing - Stellar uses XDR format
    const encoder = new TextEncoder();
    const messageBytes = encoder.encode(nonce);
    const hashBuffer = await crypto.subtle.digest('SHA-256', messageBytes);
    const hashArray = Array.from(new Uint8Array(hashBuffer));
    
    // Return as base64 (XDR format)
    return Buffer.from(hashArray).toString('base64');
  } catch (error) {
    throw new Error(`Stellar signing failed: ${error}`);
  }
};

/**
 * Sign message with NEAR wallet
 * Uses near-api-js signing
 */
const signWithNear = async (
  walletAddress: string,
  nonce: string
): Promise<string> => {
  try {
    // In production, use:
    // const wallet = new WalletConnection(near, 'myapp');
    // if (!wallet.isSignedIn()) {
    //   wallet.requestSignIn();
    //   return;
    // }
    // const encodedMessage = new TextEncoder().encode(nonce);
    // const response = await wallet.account().connection.provider.sendJsonRpc(
    //   'sign',
    //   { data: encodedMessage, accountId: walletAddress }
    // );
    // return Buffer.from(response.signature).toString('base64');

    // For now, simulate signing - NEAR uses base64
    const encoder = new TextEncoder();
    const messageBytes = encoder.encode(nonce);
    const hashBuffer = await crypto.subtle.digest('SHA-256', messageBytes);
    const hashArray = Array.from(new Uint8Array(hashBuffer));
    
    return Buffer.from(hashArray).toString('base64');
  } catch (error) {
    throw new Error(`NEAR signing failed: ${error}`);
  }
};

/**
 * Convert bytes to Base58 (Solana format)
 */
const base58Encode = (buffer: Buffer): string => {
  const ALPHABET = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz';
  const BASE = 58;
  
  let encoded = '';
  let num = 0n;
  
  for (const byte of buffer) {
    num = num * 256n + BigInt(byte);
  }
  
  if (num === 0n) {
    encoded = ALPHABET[0];
  } else {
    while (num > 0n) {
      const remainder = Number(num % BigInt(BASE));
      encoded = ALPHABET[remainder] + encoded;
      num = num / BigInt(BASE);
    }
  }
  
  for (const byte of buffer) {
    if (byte === 0) {
      encoded = ALPHABET[0] + encoded;
    } else {
      break;
    }
  }
  
  return encoded;
};

/**
 * Main signing function - routes to chain-specific implementation
 */
export const signApprovalWithWallet = async (
  chain: Chain,
  walletAddress: string,
  nonce: string
): Promise<string> => {
  // Validate inputs
  if (!walletAddress || !nonce) {
    throw new Error('Wallet address and nonce are required');
  }

  try {
    switch (chain) {
      case 'solana':
        return await signWithSolana(walletAddress, nonce);

      case 'stellar':
        return await signWithStellar(walletAddress, nonce);

      case 'near':
        return await signWithNear(walletAddress, nonce);

      default:
        throw new Error(`Unknown chain: ${chain}`);
    }
  } catch (error) {
    console.error(`Failed to sign with ${chain} wallet:`, error);
    throw error;
  }
};

/**
 * Hook to request wallet signature
 * Returns signature in chain-appropriate format
 */
export const useWalletSign = () => {
  const sign = async (
    chain: Chain,
    walletAddress: string,
    nonce: string
  ): Promise<string> => {
    return await signApprovalWithWallet(chain, walletAddress, nonce);
  };

  return { sign };
};
