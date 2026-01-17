
export type ChainType = 'solana' | 'stellar' | 'near';

export interface WalletAdapter<Chain = any> {
  name: string;
  chain: ChainType;
  isInstalled: () => boolean;
  connect: () => Promise<WalletAccount>;
  disconnect: () => Promise<void>;
  signMessage: (message: string) => Promise<string>;
  getAccount: () => Promise<WalletAccount | null>;
  onAccountChanged: (callback: (account: WalletAccount | null) => void) => void;
  onDisconnect: (callback: () => void) => void;
}

export interface WalletAccount {
  address: string;
  publicKey: string;
  chain: ChainType;
}

export interface WalletAdapterConfig {
  network?: 'mainnet' | 'testnet' | 'devnet';
  debug?: boolean;
}