import axios from 'axios';

export const apiClient = axios.create({
  baseURL: '/',
  timeout: 10000,
  headers: { 'Content-Type': 'application/json' },
});

apiClient.interceptors.response.use(
  (response) => response,
  (error) => {
    if (error.code === 'ERR_NETWORK') {
      console.error('Agent manager is unreachable');
    }
    return Promise.reject(error);
  },
);
