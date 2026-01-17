// src/lib/wallet-adapters/near.ts
import { setupWalletSelector, WalletSelector } from "@near-wallet-selector/core";
import { setupNearWallet } from '@near-wallet-selector/near-wallet';
import { setupModal } from "@near-wallet-selector/modal-ui";
import { WalletAdapter, WalletAccount, WalletAdapterConfig } from "./types";
import { WalletConnection, Near, keyStores } from 'near-api-js';

export class NearWalletAdapter implements WalletAdapter {
  public name = 'NEAR Wallet';
  public chain: 'near' = 'near';
  private selector: WalletSelector | null = null;
  private account: WalletAccount | null = null;
  private accountListeners: ((account: WalletAccount | null) => void)[] = [];
  private disconnectListeners: (() => void)[] = [];
  private wallet: any;
  private network: 'mainnet' | 'testnet' = 'testnet';

  constructor(config: WalletAdapterConfig & { contractId?: string } = {}) {
    this.network = config.network === 'mainnet' ? 'mainnet' : 'testnet';
  }

  isInstalled(): boolean {
    return true; // NEAR wallet is a web wallet, so it's always available
  }

  async connect(): Promise<WalletAccount> {
  try {
    if (!this.selector) {
      this.selector = await setupWalletSelector({
        network: this.network,
        modules: [setupNearWallet() as any],
      });
    }

    const wallet = await this.selector.wallet('near-wallet');
    
    // First check if already connected
    const accounts = await wallet.getAccounts();
    if (accounts && accounts.length > 0) {
      const accountId = accounts[0].accountId;
      this.account = {
        address: accountId,
        publicKey: accountId,
        chain: 'near'
      };
      return this.account;
    }

    // If not connected, request sign in
    await wallet.signIn({
      // contractId: 'crosschain-payments.testnet',
      successUrl: window.location.href,
      failureUrl: window.location.href,
      accounts: []
    });

    // After sign in, get the account
    const newAccounts = await wallet.getAccounts();
    if (!newAccounts || newAccounts.length === 0) {
      throw new Error('No accounts found after sign in');
    }

    const accountId = newAccounts[0].accountId;
    this.account = {
      address: accountId,
      publicKey: accountId,
      chain: 'near'
    };

    this.accountListeners.forEach(listener => listener(this.account));
    return this.account;
  } catch (error) {
    console.error('NEAR wallet connection error:', error);
    throw new Error('Failed to connect to NEAR wallet: ' + (error as Error).message);
  }
}

  async disconnect(): Promise<void> {
    if (this.selector) {
      const wallet = await this.selector.wallet('near-wallet');
      if (wallet) {
        await wallet.signOut();
      }
    }
    this.account = null;
    this.accountListeners.forEach(listener => listener(null));
    this.disconnectListeners.forEach(listener => listener());
  }

  async signMessage(message: string): Promise<string> {
    if (!this.account) {
      throw new Error('Wallet not connected');
    }

    try {
      // Use the wallet-selector's signMessage if available
      const wallet = await this.selector?.wallet('near-wallet');
      if (wallet?.signMessage) {
        const result = await wallet.signMessage({
          message: Buffer.from(message).toString('base64'),
          nonce: Buffer.from(message).toString('base64'),
          recipient: this.account.address,
          state: ''
        });
        
        if (typeof result === 'object' && 'signature' in result) {
          return result.signature;
        }

        return result as string;
      }

      // Fallback to near-api-js if wallet-selector doesn't support signMessage
      const keyStore = new keyStores.BrowserLocalStorageKeyStore();
      const keyPair = await keyStore.getKey(this.network, this.account.address);
      if (!keyPair) {
        throw new Error('Key not found for account');
      }
      const signature = keyPair.sign(Buffer.from(message)).signature;
      return Buffer.from(signature).toString('base64');
    } catch (error) {
      console.error('Error signing message:', error);
      throw new Error('Failed to sign message');
    }
  }

  async getAccount(): Promise<WalletAccount | null> {
    if (this.account) {
      return this.account;
    }
    return null;
  }

  onAccountChanged(callback: (account: WalletAccount | null) => void): void {
    this.accountListeners.push(callback);
  }

  onDisconnect(callback: () => void): void {
    this.disconnectListeners.push(callback);
  }

  // Helper method to clean up listeners
  private clearListeners(): void {
    this.accountListeners = [];
    this.disconnectListeners = [];
  }
}