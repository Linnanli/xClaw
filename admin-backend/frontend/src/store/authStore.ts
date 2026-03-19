import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import axios from 'axios';
import { setAuthToken, clearAuthToken, getAuthToken } from '../api/client';
import type { User } from '../types';

interface AuthState {
  user: User | null;
  token: string | null;
  isAuthenticated: boolean;
  login: (username: string, password: string) => Promise<void>;
  logout: () => void;
  checkAuth: () => void;
}

export const useAuthStore = create<AuthState>()(
  persist(
    (set) => ({
      user: null,
      token: null,
      isAuthenticated: false,

      login: async (username: string, password: string) => {
        try {
          // 调用登录 API
          const response = await axios.post('/api/auth/login', {
            username,
            password,
          });

          const { token, user } = response.data;

          // 保存令牌
          setAuthToken(token);

          // 更新状态
          set({
            user,
            token,
            isAuthenticated: true,
          });
        } catch (error) {
          // 清除认证状态
          clearAuthToken();
          set({
            user: null,
            token: null,
            isAuthenticated: false,
          });
          throw error;
        }
      },

      logout: () => {
        // 清除令牌
        clearAuthToken();

        // 清除状态
        set({
          user: null,
          token: null,
          isAuthenticated: false,
        });
      },

      checkAuth: () => {
        // 检查本地存储中的令牌
        const token = getAuthToken();

        if (token) {
          set({
            token,
            isAuthenticated: true,
          });
        } else {
          set({
            user: null,
            token: null,
            isAuthenticated: false,
          });
        }
      },
    }),
    {
      name: 'auth-storage',
      partialPersist: (state) => ({
        user: state.user,
        token: state.token,
        isAuthenticated: state.isAuthenticated,
      }),
    }
  )
);
