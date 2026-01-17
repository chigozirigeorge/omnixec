// Polyfills for Node.js globals used by wallet-selector bundles.
import './polyfills/node-globals';
import { createRoot } from 'react-dom/client';
import App from "./App.tsx";
import "./index.css";

createRoot(document.getElementById("root")!).render(<App />);
