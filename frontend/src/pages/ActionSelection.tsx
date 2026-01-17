import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { motion } from 'framer-motion';
import { Header } from '@/components/Header';
import { GlassCard } from '@/components/ui/GlassCard';
import { Button } from '@/components/ui/button';
import { Zap, Image, Code, ArrowRight } from 'lucide-react';
import { useStore } from '@/stores/useStore';

type ActionType = 'trade' | 'mint' | 'interact';

export default function ActionSelection() {
  const navigate = useNavigate();
  const { wallet } = useStore();
  const [selectedAction, setSelectedAction] = useState<ActionType | null>(null);

  const actions = [
    {
      id: 'trade' as ActionType,
      title: 'Trade Tokens',
      description: 'Swap tokens across different chains',
      icon: Zap,
      color: 'primary',
    },
    {
      id: 'mint' as ActionType,
      title: 'Mint NFT',
      description: 'Create NFTs on your preferred chain',
      icon: Image,
      color: 'secondary',
    },
    {
      id: 'interact' as ActionType,
      title: 'Interact with Protocol',
      description: 'Execute smart contract interactions',
      icon: Code,
      color: 'accent',
    },
  ];

  const handleActionSelect = (action: ActionType) => {
    setSelectedAction(action);
    // Navigate based on action type
    if (action === 'trade') {
      navigate('/token-discovery');
    } else {
      // TODO: Implement mint and interact flows
      console.log(`Action ${action} not yet implemented`);
    }
  };

  // Check if user has connected wallets
  const hasWallets = wallet.isConnected; // TODO: Check for multiple wallets

  return (
    <div className="min-h-screen">
      <Header />

      <main className="pt-24 pb-12 px-4">
        <div className="container mx-auto max-w-4xl">
          <motion.div
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            className="text-center mb-12"
          >
            <h1 className="text-4xl font-bold mb-4">What would you like to do?</h1>
            <p className="text-muted-foreground text-lg">
              Select an action to get started
            </p>
          </motion.div>

          {!hasWallets && (
            <motion.div
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              className="mb-8"
            >
              <GlassCard className="p-6 border-yellow-500/30 bg-yellow-500/10">
                <p className="text-center text-sm text-yellow-200">
                  ⚠️ Please connect your wallets first to proceed
                </p>
              </GlassCard>
            </motion.div>
          )}

          <div className="grid md:grid-cols-3 gap-6">
            {actions.map((action, index) => {
              const Icon = action.icon;
              return (
                <motion.div
                  key={action.id}
                  initial={{ opacity: 0, y: 20 }}
                  animate={{ opacity: 1, y: 0 }}
                  transition={{ delay: index * 0.1 }}
                >
                  <GlassCard
                    glow={selectedAction === action.id ? action.color : undefined}
                    className={`cursor-pointer transition-all hover:scale-105 ${
                      selectedAction === action.id ? 'ring-2 ring-primary' : ''
                    }`}
                    onClick={() => handleActionSelect(action.id)}
                  >
                    <div className="flex flex-col items-center text-center space-y-4 p-6">
                      <div className={`p-4 rounded-full ${
                        action.color === 'primary' ? 'bg-primary/20' :
                        action.color === 'secondary' ? 'bg-secondary/20' :
                        'bg-accent/20'
                      }`}>
                        <Icon className={`h-8 w-8 ${
                          action.color === 'primary' ? 'text-primary' :
                          action.color === 'secondary' ? 'text-secondary' :
                          'text-accent'
                        }`} />
                      </div>
                      <h3 className="text-xl font-semibold">{action.title}</h3>
                      <p className="text-sm text-muted-foreground">
                        {action.description}
                      </p>
                      <Button
                        variant={selectedAction === action.id ? 'gradient' : 'outline'}
                        className="w-full"
                        disabled={!hasWallets}
                      >
                        Select
                        <ArrowRight className="ml-2 h-4 w-4" />
                      </Button>
                    </div>
                  </GlassCard>
                </motion.div>
              );
            })}
          </div>
        </div>
      </main>
    </div>
  );
}

