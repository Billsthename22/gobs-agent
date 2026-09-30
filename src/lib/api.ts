export const API_URL =
  process.env.NEXT_PUBLIC_API_URL ||
  "https://gbos-backend-production.up.railway.app";

export const WS_URL = API_URL.replace(/^http/, "ws");
