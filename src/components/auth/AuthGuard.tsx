"use client";

import { useEffect, useState } from "react";
import { usePathname, useRouter } from "next/navigation";

import { getToken } from "@/lib/auth";

export default function AuthGuard({
  children,
}: {
  children: React.ReactNode;
}) {
  const router = useRouter();
  const pathname = usePathname();
  const [checking, setChecking] = useState(true);

  // The sign-in screen and the root route are both public — the root route
  // renders the sign-in screen. Treating them as public keeps that first paint
  // from flashing the "Checking authentication..." state.
  const isPublicRoute = pathname === "/login" || pathname === "/";

  useEffect(() => {
    if (isPublicRoute) {
      setChecking(false);
      return;
    }

    const token = getToken();

    if (!token) {
      router.replace("/login");
      return;
    }

    setChecking(false);
  }, [isPublicRoute, router]);

  if (checking && !isPublicRoute) {
    return (
      <main className="flex min-h-screen items-center justify-center bg-zinc-50">
        <div className="text-sm text-zinc-500">
          Checking authentication...
        </div>
      </main>
    );
  }

  return <>{children}</>;
}
