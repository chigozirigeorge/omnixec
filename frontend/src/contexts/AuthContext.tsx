// src/contexts/AuthContext.tsx
import React, { createContext, useContext, useState, useEffect, ReactNode } from 'react';

interface AuthContextType {
  userId: string | null;
  setUserId: (id: string) => void;
  clearUserId: () => void;
}

const AuthContext = createContext<AuthContextType | undefined>(undefined);

export const AuthProvider = ({ children }: { children: ReactNode }) => {
  const [userId, setUserIdState] = useState<string | null>(() => {
    if (typeof window !== 'undefined') {
      return localStorage.getItem('omniexec_user_id');
    }
    return null;
  });

  const setUserId = (id: string) => {
    localStorage.setItem('omniexec_user_id', id);
    setUserIdState(id);
  };

  const clearUserId = () => {
    localStorage.removeItem('omniexec_user_id');
    setUserIdState(null);
  };

  return (
    <AuthContext.Provider value={{ userId, setUserId, clearUserId }}>
      {children}
    </AuthContext.Provider>
  );
};

export const useAuth = () => {
  const context = useContext(AuthContext);
  if (context === undefined) {
    throw new Error('useAuth must be used within an AuthProvider');
  }
  return context;
};