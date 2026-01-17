import { Link, useLocation } from 'react-router-dom';
import { motion } from 'framer-motion';
import { WalletConnectButton } from '@/components/WalletConnectButton';
import { cn } from '@/lib/utils';
import omnixecLogo from '@/assets/omnixec-logo.jpg';
import { Menu, X } from 'lucide-react';
import { useState } from 'react';
import { Button } from '@/components/ui/button';

const navLinks = [
  { href: '/', label: 'Home' },
  { href: '/trade', label: 'Trade' },
  { href: '/charts', label: 'Charts' },
];

export function Header() {
  const location = useLocation();
  const [mobileMenuOpen, setMobileMenuOpen] = useState(false);

  return (
    <header className="fixed top-0 left-0 right-0 z-50 glass border-b border-white/5">
      <div className="container mx-auto px-4">
        <div className="flex items-center justify-between h-16">
          {/* Logo */}
          <Link to="/" className="flex items-center gap-3">
            <img
              src={omnixecLogo}
              alt="OmniXec"
              className="h-8 w-8 rounded-lg object-cover"
            />
            <span className="text-xl font-bold gradient-text">OmniXec</span>
          </Link>

          {/* Desktop Nav */}
          <nav className="hidden md:flex items-center gap-1">
            {navLinks.map((link) => {
              const isActive = location.pathname === link.href;
              return (
                <Link
                  key={link.href}
                  to={link.href}
                  className={cn(
                    'relative px-4 py-2 text-sm font-medium transition-colors rounded-lg',
                    isActive
                      ? 'text-foreground'
                      : 'text-muted-foreground hover:text-foreground hover:bg-secondary/50'
                  )}
                >
                  {link.label}
                  {isActive && (
                    <motion.div
                      layoutId="activeNav"
                      className="absolute inset-0 bg-secondary rounded-lg -z-10"
                      transition={{ type: 'spring', duration: 0.5 }}
                    />
                  )}
                </Link>
              );
            })}
          </nav>

          {/* Wallet + Mobile Menu */}
          <div className="flex items-center gap-3">
            <WalletConnectButton />
            <Button
              variant="ghost"
              size="icon"
              className="md:hidden"
              onClick={() => setMobileMenuOpen(!mobileMenuOpen)}
            >
              {mobileMenuOpen ? <X /> : <Menu />}
            </Button>
          </div>
        </div>
      </div>

      {/* Mobile Nav */}
      {mobileMenuOpen && (
        <motion.div
          initial={{ opacity: 0, height: 0 }}
          animate={{ opacity: 1, height: 'auto' }}
          exit={{ opacity: 0, height: 0 }}
          className="md:hidden glass border-t border-white/5"
        >
          <nav className="container mx-auto px-4 py-4 flex flex-col gap-2">
            {navLinks.map((link) => {
              const isActive = location.pathname === link.href;
              return (
                <Link
                  key={link.href}
                  to={link.href}
                  onClick={() => setMobileMenuOpen(false)}
                  className={cn(
                    'px-4 py-3 text-sm font-medium rounded-lg transition-colors',
                    isActive
                      ? 'bg-secondary text-foreground'
                      : 'text-muted-foreground hover:text-foreground hover:bg-secondary/50'
                  )}
                >
                  {link.label}
                </Link>
              );
            })}
          </nav>
        </motion.div>
      )}
    </header>
  );
}
