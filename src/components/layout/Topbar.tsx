"use client";

import { Bell, LogOut, Search } from "lucide-react";
import { useRouter } from "next/navigation";
import { useState } from "react";

import { clearToken } from "@/lib/auth";

export default function Topbar() {
  const router = useRouter();
  const [userMenuOpen, setUserMenuOpen] = useState(false);

  function handleLogout() {
    clearToken();
    setUserMenuOpen(false);
    router.replace("/login");
  }

  return (
    <header className="fixed left-64 right-0 top-0 z-30 flex h-20 items-center justify-between border-b border-zinc-200 bg-white/95 px-8 backdrop-blur">
      <div>
        <p className="text-sm font-medium text-zinc-900">
          Company Overview
        </p>

        <p className="text-xs text-zinc-500">
          Monitor your organization's devices
        </p>
      </div>

      <div className="flex items-center gap-4">
        {/* Search */}
        <div className="hidden w-72 items-center gap-2 rounded-xl border border-zinc-200 bg-zinc-50 px-3 py-2 md:flex">
          <Search size={17} className="text-zinc-400" />

          <input
            type="text"
            placeholder="Search devices..."
            className="w-full bg-transparent text-sm text-zinc-900 outline-none placeholder:text-zinc-400"
          />

          <kbd className="rounded-md border border-zinc-200 bg-white px-1.5 py-0.5 text-[10px] text-zinc-400">
            ⌘K
          </kbd>
        </div>

        {/* Notifications */}
        <button
          type="button"
          className="relative flex h-10 w-10 items-center justify-center rounded-xl border border-zinc-200 text-zinc-500 transition hover:bg-zinc-50 hover:text-zinc-900"
        >
          <Bell size={18} />

          <span className="absolute right-2 top-2 h-2 w-2 rounded-full bg-red-500" />
        </button>

        {/* User */}
        <div className="relative">
          <button
            type="button"
            onClick={() => setUserMenuOpen((open) => !open)}
            className="flex items-center gap-3 rounded-xl px-2 py-1.5 text-left transition hover:bg-zinc-50"
          >
            <div className="flex h-9 w-9 items-center justify-center rounded-full bg-zinc-950 text-sm font-medium text-white">
              A
            </div>

            <div className="hidden lg:block">
              <p className="text-sm font-medium text-zinc-900">
                Administrator
              </p>

              <p className="text-xs text-zinc-500">
                Company Admin
              </p>
            </div>
          </button>

          {userMenuOpen && (
            <div className="absolute right-0 top-14 w-48 rounded-xl border border-zinc-200 bg-white p-1.5 shadow-lg">
              <button
                type="button"
                onClick={handleLogout}
                className="flex w-full items-center gap-2 rounded-lg px-3 py-2 text-sm text-zinc-700 transition hover:bg-zinc-50 hover:text-zinc-950"
              >
                <LogOut size={16} />
                <span>Log out</span>
              </button>
            </div>
          )}
        </div>
      </div>
    </header>
  );
}
