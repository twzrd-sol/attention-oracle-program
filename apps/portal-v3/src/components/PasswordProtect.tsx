import React, { useState, useEffect } from 'react';

const PASSWORD = 'ISHOWSPEED2025';

interface PasswordProtectProps {
  children: React.ReactNode;
}

export const PasswordProtect: React.FC<PasswordProtectProps> = ({ children }) => {
  const [isAuthenticated, setIsAuthenticated] = useState(false);
  const [password, setPassword] = useState('');
  const [error, setError] = useState('');

  useEffect(() => {
    if (sessionStorage.getItem('portal-auth') === 'true') {
      setIsAuthenticated(true);
    }
  }, []);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (password === PASSWORD) {
      setIsAuthenticated(true);
      sessionStorage.setItem('portal-auth', 'true');
      setError('');
    } else {
      setError('Invalid password');
      setPassword('');
    }
  };

  if (isAuthenticated) return <>{children}</>;

  return (
    <div className="min-h-screen flex items-center justify-center bg-gray-50 p-6">
      <div className="w-full max-w-md bg-white border border-gray-200 rounded-xl p-8 shadow-lg">
        <h1 className="text-3xl font-bold text-gray-900 text-center mb-2">Attention Oracle Portal</h1>
        <p className="text-gray-500 text-center mb-8">Early Access Portal</p>

        <form onSubmit={handleSubmit} className="space-y-6">
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-2">Enter Password</label>
            <input
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              placeholder="Password"
              autoComplete="new-password"
              autoFocus
              className="w-full px-4 py-3 border border-gray-300 rounded-lg font-mono focus:outline-none focus:ring-2 focus:ring-blue-500"
            />
          </div>

          {error && <p className="text-red-600 text-center text-sm">{error}</p>}

          <button
            type="submit"
            className="w-full py-3 bg-blue-600 text-white font-semibold rounded-lg hover:bg-blue-700 transition"
          >
            Unlock
          </button>
        </form>

        <p className="text-xs text-gray-400 text-center mt-6">
          This is an early access portal. Please contact support for access.
        </p>
      </div>
    </div>
  );
};

export default PasswordProtect;
