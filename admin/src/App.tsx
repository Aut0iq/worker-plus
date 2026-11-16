import { useEffect } from 'react';
import { Navigate, Route, Routes } from 'react-router-dom';
import { api } from './api/client';
import { setAppTimezone } from './format';
import { useAuth } from './auth';
import AppLayout from './components/AppLayout';
import LoginPage from './pages/Login';
import Dashboard from './pages/Dashboard';
import UsersPage from './pages/Users';
import GroupsPage from './pages/Groups';
import SessionsPage from './pages/Sessions';
import ShopPage from './pages/Shop';
import SummariesPage from './pages/Summaries';
import LogsPage from './pages/Logs';
import type { ReactNode } from 'react';

interface ServerConfig {
  timezone: string;
  max_session_hours: number;
}

export default function App() {
  const { user } = useAuth();
  useEffect(() => {
    api
      .get<ServerConfig>('/api/config')
      .then((c) => setAppTimezone(c.timezone))
      .catch(() => undefined);
  }, []);
  const need = (el: ReactNode) => (user ? el : <Navigate to="/login" replace />);
  return (
    <Routes>
      <Route path="/login" element={<LoginPage />} />
      <Route element={need(<AppLayout />)}>
        <Route path="/" element={<Dashboard />} />
        <Route path="/users" element={<UsersPage />} />
        <Route path="/groups" element={<GroupsPage />} />
        <Route path="/sessions" element={<SessionsPage />} />
        <Route path="/shop" element={<ShopPage />} />
        <Route path="/summaries" element={<SummariesPage />} />
        <Route path="/logs" element={<LogsPage />} />
      </Route>
      <Route path="*" element={<Navigate to="/" replace />} />
    </Routes>
  );
}
