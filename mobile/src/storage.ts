import { Platform } from 'react-native';
import * as NativeSecureStore from 'expo-secure-store';

type Store = Pick<typeof NativeSecureStore, 'getItemAsync' | 'setItemAsync' | 'deleteItemAsync'>;

// expo-secure-store has no web implementation: the browser build keeps values in localStorage
export const SecureStore: Store =
  Platform.OS === 'web'
    ? {
        getItemAsync: async (k) => localStorage.getItem(k),
        setItemAsync: async (k, v) => localStorage.setItem(k, v),
        deleteItemAsync: async (k) => localStorage.removeItem(k),
      }
    : NativeSecureStore;
