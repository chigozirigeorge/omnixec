// src/lib/wallet-adapters/solana.ts
import { WalletAdapter, WalletAccount, WalletAdapterConfig } from './types';

declare global {
  interface Window {
    solana?: any;
    solflare?: any;
  }
}

export class SolanaWalletAdapter implements WalletAdapter {
  private _publicKey: string | null = null;
  private _listeners: ((account: WalletAccount | null) => void)[] = [];
  private _disconnectListeners: (() => void)[] = [];
  private _config: WalletAdapterConfig;

  public name = 'Phantom';
  public chain = 'solana' as const;

  constructor(config: WalletAdapterConfig = {}) {
    this._config = { network: 'mainnet', ...config };
    this._handleAccountsChanged = this._handleAccountsChanged.bind(this);
    this._handleDisconnect = this._handleDisconnect.bind(this);
  }

  isInstalled(): boolean {
    return !!(window.solana?.isPhantom || window.solflare?.isPhantom);
  }

  async connect(): Promise<WalletAccount> {
    try {
      const provider = window.solana || window.solflare;
      if (!provider) {
        throw new Error('No Solana wallet found. Please install Phantom or Solflare.');
      }

      const response = await provider.connect();
      this._publicKey = response.publicKey.toString();

      provider.on('accountChanged', this._handleAccountsChanged);
      provider.on('disconnect', this._handleDisconnect);

      return {
        address: this._publicKey,
        publicKey: this._publicKey,
        chain: 'solana',
      };
    } catch (error) {
      console.error('Solana connection error:', error);
      throw new Error('Failed to connect to Solana wallet');
    }
  }

  async disconnect(): Promise<void> {
    const provider = window.solana || window.solflare;
    if (provider?.disconnect) {
      await provider.disconnect();
    }
    this._publicKey = null;
    this._handleAccountsChanged(null);
  }

  async signMessage(message: string): Promise<string> {
    if (!this._publicKey) {
      throw new Error('Wallet not connected');
    }

    try {
      const provider = window.solana || window.solflare;
      const encodedMessage = new TextEncoder().encode(message);
      const { signature } = await provider.signMessage(encodedMessage, 'utf8');
      return signature;
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
      chain: 'solana',
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
      chain: 'solana' as const,
    } : null;
    
    this._listeners.forEach(listener => listener(account));
  }

  private _handleDisconnect() {
    this._publicKey = null;
    this._disconnectListeners.forEach(listener => listener());
  }
}