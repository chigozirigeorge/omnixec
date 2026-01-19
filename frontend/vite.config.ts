import { defineConfig, loadEnv } from "vite";
import react from "@vitejs/plugin-react-swc";
import path from "path";
import { componentTagger } from "lovable-tagger";

// https://vitejs.dev/config/
export default defineConfig(({ mode }) => {
  // Load env file based on `mode` in the current working directory.
  const env = loadEnv(mode, process.cwd(), '');
  
  return {
    base: '/',
    server: {
      host: "::",
      port: 8000,
      fs: {
        // Allow serving files from one level up from the package root
        allow: ['..']
      }
    },
    plugins: [
      react(), 
      mode === "development" && componentTagger()
    ].filter(Boolean),
    resolve: {
      alias: {
        "@": path.resolve(__dirname, "./src"),
        // Add these aliases to help with NEAR SDK resolution
        'near-api-js': path.resolve(__dirname, 'node_modules/near-api-js/dist/near-api-js.js'),
      },
    },
    build: {
      outDir: 'dist',
      sourcemap: true,
      chunkSizeWarningLimit: 1000,
      commonjsOptions: {
        transformMixedEsModules: true,
        // Add these to handle NEAR SDK dependencies
        include: [/node_modules/],
      },
      rollupOptions: {
        output: {
          manualChunks: {
            vendor: ['react', 'react-dom', 'react-router-dom'],
            near: ['near-api-js', '@near-wallet-selector/core', '@near-wallet-selector/modal-ui'],
          },
        },
      },
    },
    define: {
        'process.env': Object.entries(env).reduce((acc, [key, value]) => {
          acc[`process.env.${key}`] = JSON.stringify(value);
          return acc;
        }, {} as Record<string, any>),
        'process.env.NODE_ENV': JSON.stringify(mode)
      },
    optimizeDeps: {
      // Add these to handle NEAR SDK dependencies
      include: [
        'bn.js',
        'js-sha256',
        'borsh',
        'tweetnacl',
        'buffer',
        'crypto',
        'stream',
        'path-browserify',
        'os-browserify/browser',
        'process/browser',
      ],
      esbuildOptions: {
        // Node.js global to browser globalThis
        define: {
          global: 'globalThis',
        },
      },
    },
  };
});