"use client";

import {
  Activity,
  Bell,
  ChartNoAxesCombined,
  Computer,
  LayoutDashboard,
  Settings,
  Users,
} from "lucide-react";
import Link from "next/link";
import { usePathname } from "next/navigation";

const navigation = [
  {
    name: "Dashboard",
    href: "/dashboard",
    icon: LayoutDashboard,
  },
  {
    name: "Devices",
    href: "/devices",
    icon: Computer,
  },
  {
    name: "Employees",
    href: "/employees",
    icon: Users,
  },
  {
    name: "Activity",
    href: "/activity",
    icon: Activity,
  },
  {
    name: "Alerts",
    href: "/alerts",
    icon: Bell,
  },
  {
    name: "Analytics",
    href: "/analytics",
    icon: ChartNoAxesCombined,
  },
];

export default function Sidebar() {
  const pathname = usePathname();

  return (
    <aside className="fixed inset-y-0 left-0 z-40 flex w-64 flex-col border-r border-zinc-200 bg-white">
      {/* Logo */}
      <div className="flex h-20 items-center border-b border-zinc-200 px-6">
        <div className="flex items-center gap-3">
          <div className="flex h-9 w-9 items-center justify-center rounded-xl bg-zinc-950 text-white">
            <Computer size={19} />
          </div>

          <div>
            <h1 className="text-sm font-semibold tracking-tight text-zinc-950">
              DeviceControl
            </h1>
            <p className="text-xs text-zinc-500">
              Admin Console
            </p>
          </div>
        </div>
      </div>

      {/* Navigation */}
      <nav className="flex-1 px-3 py-6">
        <p className="mb-3 px-3 text-[10px] font-semibold uppercase tracking-[0.12em] text-zinc-400">
          Workspace
        </p>

        <div className="space-y-1">
          {navigation.map((item) => {
            const Icon = item.icon;

            const active =
              pathname === item.href ||
              pathname.startsWith(`${item.href}/`);

            return (
              <Link
                key={item.href}
                href={item.href}
                className={`flex items-center gap-3 rounded-xl px-3 py-2.5 text-sm transition-all ${
                  active
                    ? "bg-zinc-100 font-medium text-zinc-950"
                    : "text-zinc-500 hover:bg-zinc-50 hover:text-zinc-950"
                }`}
              >
                <Icon size={18} strokeWidth={1.8} />
                <span>{item.name}</span>
              </Link>
            );
          })}
        </div>
      </nav>

      {/* Bottom */}
      <div className="border-t border-zinc-200 p-3">
        <Link
          href="/settings"
          className="flex items-center gap-3 rounded-xl px-3 py-2.5 text-sm text-zinc-500 transition hover:bg-zinc-50 hover:text-zinc-950"
        >
          <Settings size={18} strokeWidth={1.8} />
          Settings
        </Link>

        <div className="mt-3 flex items-center gap-3 rounded-xl bg-zinc-50 p-3">
          <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-full bg-zinc-950 text-sm font-medium text-white">
            A
          </div>

          <div className="min-w-0">
            <p className="truncate text-sm font-medium text-zinc-900">
              Administrator
            </p>
            <p className="truncate text-xs text-zinc-500">
              Company Admin
            </p>
          </div>
        </div>
      </div>
    </aside>
  );
}