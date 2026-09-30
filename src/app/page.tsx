import LoginPage from "./login/page";

// The root route is the sign-in screen. The dashboard lives at /dashboard so
// that the first page anyone sees is authentication.
export default function Home() {
  return <LoginPage />;
}
