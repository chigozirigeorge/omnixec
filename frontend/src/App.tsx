import { Toaster } from "@/components/ui/toaster";
import { Toaster as Sonner } from "@/components/ui/sonner";
import { TooltipProvider } from "@/components/ui/tooltip";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { BrowserRouter, Routes, Route } from "react-router-dom";
import { WalletProvider } from "@/contexts/WalletProvider";
import { AuthProvider } from "@/contexts/AuthContext";
import Landing from "./pages/Landing";
import ActionSelection from "./pages/ActionSelection";
import TokenDiscovery from "./pages/TokenDiscovery";
import TokenDetails from "./pages/TokenDetails";
import Trade from "./pages/Trade";
import QuoteReview from "./pages/QuoteReview";
import Execution from "./pages/Execution";
import Charts from "./pages/Charts";
import NotFound from "./pages/NotFound";

const queryClient = new QueryClient();

const App = () => (
  <QueryClientProvider client={queryClient}>
    <AuthProvider>
      <WalletProvider>
        <TooltipProvider>
          <Toaster />
          <Sonner />
          <BrowserRouter>
            <Routes>
              <Route path="/" element={<Landing />} />
              <Route path="/action-selection" element={<ActionSelection />} />
              <Route path="/token-discovery" element={<TokenDiscovery />} />
              <Route path="/token-details/:chain/:symbol" element={<TokenDetails />} />
              <Route path="/trade" element={<Trade />} />
              <Route path="/quote" element={<QuoteReview />} />
              <Route path="/execution" element={<Execution />} />
              <Route path="/charts" element={<Charts />} />
              <Route path="*" element={<NotFound />} />
            </Routes>
          </BrowserRouter>
        </TooltipProvider>
      </WalletProvider>
    </AuthProvider>
  </QueryClientProvider>
);

export default App;