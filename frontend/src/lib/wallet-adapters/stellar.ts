// src/lib/wallet-adapters/stellar.ts
import { WalletAdapter, WalletAccount, WalletAdapterConfig } from './types';
import StellarSdk, { Keypair } from 'stellar-sdk';

declare global {
  interface Window {
    freighter?: any;
  }
}

export class StellarWalletAdapter implements WalletAdapter {
  private _publicKey: string | null = null;
  private _listeners: ((account: WalletAccount | null) => void)[] = [];
  private _disconnectListeners: (() => void)[] = [];
  private _config: WalletAdapterConfig;

  public name = 'Freighter';
  public chain = 'stellar' as const;

  constructor(config: WalletAdapterConfig = {}) {
    this._config = { network: 'testnet', ...config };
    this._handleAccountsChanged = this._handleAccountsChanged.bind(this);
    this._handleDisconnect = this._handleDisconnect.bind(this);
  }

   isInstalled(): boolean {
    return !!window.freighter;
  }

  async connect(): Promise<WalletAccount> {
    try {
      if (!window.freighter) {
        throw new Error('Freighter wallet not found. Please install it first.');
      }

      const isConnected = await window.freighter.isConnected();
      if (!isConnected) {
        await window.freighter.connect();
      }

      const publicKey = await window.freighter.getPublicKey();
      this._publicKey = publicKey;

      window.addEventListener("freighter:accountChanged" as any, this._handleAccountsChanged as any);
      window.addEventListener('freighter:disconnect', this._handleDisconnect);

      return {
        address: publicKey,
        publicKey: publicKey,
        chain: 'stellar',
      };
    } catch (error) {
      console.error('Stellar connection error:', error);
      throw new Error('Failed to connect to Stellar wallet');
    }
  }

  async disconnect(): Promise<void> {
    this._publicKey = null;
    window.removeEventListener('freighter:accountChanged' as any, this._handleAccountsChanged as any);
    window.removeEventListener('freighter:disconnect', this._handleDisconnect);
    this._handleDisconnect();
  }

  async signMessage(message: string): Promise<string> {
    if (!this._publicKey) {
      throw new Error('Wallet not connected');
    }

    try {
      const signedMessage = await window.freighter.signMessage(message);
      return signedMessage;
    } catch (error) {
      console.error('Sign message error:', error);
      throw new Error('Failed to sign message');
    }
  }

  async getAccount(): Promise<WalletAccount | null> {
    if (!this._publicKey) return null;
    return {
      address: this._publicKey,
      publicKey: this._publicKey,
      chain: 'stellar',
    };
  }

  onAccountChanged(callback: (account: WalletAccount | null) => void): void {
    this._listeners.push(callback);
  }

  onDisconnect(callback: () => void): void {
    this._disconnectListeners.push(callback);
  }

  private _handleAccountsChanged(publicKey: string | null) {
    this._publicKey = publicKey;
    const account = publicKey ? {
      address: publicKey,
      publicKey: publicKey,
      chain: 'stellar' as const,
    } : null;
    
    this._listeners.forEach(listener => listener(account));
  }

  private _handleDisconnect() {
    this._publicKey = null;
    this._disconnectListeners.forEach(listener => listener());
  }
}