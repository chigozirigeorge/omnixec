/// <reference types="vite/client" />

// Add type definitions for NEAR
interface Window {
  near: any;
  walletConnection: any;
  account: any;
  contract: any;
  nearInitPromise: Promise<void>;
}

// Add Node.js globals
declare const process: {
  env: {
    NODE_ENV: 'development' | 'production';
    [key: string]: string | undefined;
  };
};