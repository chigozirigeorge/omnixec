import { motion } from 'framer-motion';
import { Button } from '@/components/ui/button';
import { GlassCard } from '@/components/ui/GlassCard';
import { ChainIcon, supportedChains, getChainConfig } from '@/components/ui/ChainIcon';
import { Link } from 'react-router-dom';
import { ArrowRight, Zap, Shield, Globe } from 'lucide-react';
import { Header } from '@/components/Header';

const stats = [
  { label: 'Avg Execution', value: '< 3s', icon: Zap },
  { label: 'Chains Supported', value: '3+', icon: Globe },
  { label: 'Secure & Audited', value: '100%', icon: Shield },
];

const fadeInUp = {
  initial: { opacity: 0, y: 30 },
  animate: { opacity: 1, y: 0 },
};

const stagger = {
  animate: {
    transition: {
      staggerChildren: 0.1,
    },
  },
};

export default function Landing() {
  return (
    <div className="min-h-screen">
      <Header />

      {/* Hero Section */}
      <section className="relative pt-32 pb-20 px-4 overflow-hidden">
        {/* Background Effects */}
        <div className="absolute inset-0 -z-10">
          <div className="absolute top-1/4 left-1/4 w-96 h-96 bg-primary/20 rounded-full blur-3xl animate-pulse-glow" />
          <div className="absolute bottom-1/4 right-1/4 w-80 h-80 bg-accent/15 rounded-full blur-3xl animate-pulse-glow" style={{ animationDelay: '1s' }} />
        </div>

        <div className="container mx-auto max-w-6xl">
          <motion.div
            className="text-center space-y-8"
            initial="initial"
            animate="animate"
            variants={stagger}
          >
            <motion.div variants={fadeInUp} className="space-y-4">
              <span className="inline-block px-4 py-1.5 text-xs font-medium bg-primary/20 text-primary border border-primary/30 rounded-full">
                Cross-Chain Execution Protocol
              </span>
              <h1 className="text-5xl md:text-7xl font-bold tracking-tight">
                <span className="gradient-text">Pay Anywhere.</span>
                <br />
                <span className="text-foreground">Execute Everywhere.</span>
              </h1>
            </motion.div>

            <motion.p
              variants={fadeInUp}
              className="text-xl text-muted-foreground max-w-2xl mx-auto text-balance"
            >
              Seamlessly bridge assets and execute trades across Solana, Stellar, and NEAR 
              with lightning-fast settlement and minimal fees.
            </motion.p>

            <motion.div variants={fadeInUp} className="flex flex-col sm:flex-row gap-4 justify-center">
              <Link to="/action-selection">
                <Button variant="gradient" size="xl" className="group">
                  Get Started
                  <ArrowRight className="h-5 w-5 transition-transform group-hover:translate-x-1" />
                </Button>
              </Link>
              <Link to="/charts">
                <Button variant="outline" size="xl">
                  View Charts
                </Button>
              </Link>
            </motion.div>
          </motion.div>

          {/* Chain Logos */}
          <motion.div
            className="mt-20 flex justify-center gap-6"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            transition={{ delay: 0.5 }}
          >
            {supportedChains.map((chain, index) => {
              const config = getChainConfig(chain);
              return (
                <motion.div
                  key={chain}
                  className="flex flex-col items-center gap-2"
                  initial={{ opacity: 0, y: 20 }}
                  animate={{ opacity: 1, y: 0 }}
                  transition={{ delay: 0.6 + index * 0.1 }}
                >
                  <div className="animate-float" style={{ animationDelay: `${index * 0.5}s` }}>
                    <ChainIcon chain={chain} size="lg" />
                  </div>
                  <span className="text-sm text-muted-foreground">{config.name}</span>
                </motion.div>
              );
            })}
          </motion.div>
        </div>
      </section>

      {/* Stats Section */}
      <section className="py-20 px-4">
        <div className="container mx-auto max-w-4xl">
          <motion.div
            className="grid grid-cols-1 md:grid-cols-3 gap-6"
            initial="initial"
            whileInView="animate"
            viewport={{ once: true }}
            variants={stagger}
          >
            {stats.map((stat) => (
              <motion.div key={stat.label} variants={fadeInUp}>
                <GlassCard className="text-center p-8">
                  <stat.icon className="h-8 w-8 mx-auto mb-4 text-primary" />
                  <p className="text-3xl font-bold gradient-text">{stat.value}</p>
                  <p className="text-muted-foreground mt-1">{stat.label}</p>
                </GlassCard>
              </motion.div>
            ))}
          </motion.div>
        </div>
      </section>

      {/* How It Works */}
      <section className="py-20 px-4">
        <div className="container mx-auto max-w-5xl">
          <motion.div
            className="text-center mb-16"
            initial={{ opacity: 0, y: 20 }}
            whileInView={{ opacity: 1, y: 0 }}
            viewport={{ once: true }}
          >
            <h2 className="text-3xl md:text-4xl font-bold mb-4">
              How <span className="gradient-text">OmniXec</span> Works
            </h2>
            <p className="text-muted-foreground max-w-xl mx-auto">
              Three simple steps to execute cross-chain transactions
            </p>
          </motion.div>

          <div className="grid grid-cols-1 md:grid-cols-3 gap-8">
            {[
              {
                step: '01',
                title: 'Select Chains',
                description: 'Choose your source chain for payment and destination chain for execution.',
              },
              {
                step: '02',
                title: 'Get Quote',
                description: 'Receive real-time pricing with transparent fees and guaranteed rates.',
              },
              {
                step: '03',
                title: 'Execute',
                description: 'Confirm and watch your transaction execute across chains in seconds.',
              },
            ].map((item, index) => (
              <motion.div
                key={item.step}
                initial={{ opacity: 0, y: 20 }}
                whileInView={{ opacity: 1, y: 0 }}
                viewport={{ once: true }}
                transition={{ delay: index * 0.1 }}
              >
                <GlassCard className="h-full">
                  <span className="text-5xl font-bold text-primary/20">{item.step}</span>
                  <h3 className="text-xl font-semibold mt-4 mb-2">{item.title}</h3>
                  <p className="text-muted-foreground">{item.description}</p>
                </GlassCard>
              </motion.div>
            ))}
          </div>
        </div>
      </section>

      {/* CTA Section */}
      <section className="py-20 px-4">
        <div className="container mx-auto max-w-3xl">
          <GlassCard glow="primary" className="text-center py-12">
            <h2 className="text-3xl font-bold mb-4">Ready to Trade Cross-Chain?</h2>
            <p className="text-muted-foreground mb-8 max-w-md mx-auto">
              Join the future of decentralized finance with seamless multi-chain execution.
            </p>
            <Link to="/trade">
              <Button variant="gradient" size="lg">
                Launch App
                <ArrowRight className="h-4 w-4" />
              </Button>
            </Link>
          </GlassCard>
        </div>
      </section>

      {/* Footer */}
      <footer className="py-8 px-4 border-t border-border/50">
        <div className="container mx-auto max-w-6xl flex flex-col md:flex-row items-center justify-between gap-4">
          <div className="flex items-center gap-2">
            <span className="text-lg font-bold gradient-text">OmniXec</span>
            <span className="text-muted-foreground text-sm">© 2026</span>
          </div>
          <nav className="flex gap-6 text-sm text-muted-foreground">
            <a href="#" className="hover:text-foreground transition-colors">Docs</a>
            <a href="#" className="hover:text-foreground transition-colors">GitHub</a>
            <a href="#" className="hover:text-foreground transition-colors">Discord</a>
          </nav>
        </div>
      </footer>
    </div>
  );
}
