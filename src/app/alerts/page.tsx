"use client";

import {
  AlertCircle,
  AlertTriangle,
  Bell,
  CheckCircle2,
} from "lucide-react";
import { useEffect, useState } from "react";

import Sidebar from "@/components/layout/Sidebar";
import Topbar from "@/components/layout/Topbar";
import StatCard from "@/components/dashboard/StatCard";
import AlertsTable from "@/components/alerts/AlertsTable";

interface AlertRecord {
  id: number;
  severity: string;
  is_resolved: boolean;
  resolved_at: string | null;
}

interface AlertStats {
  activeAlerts: number;
  critical: number;
  warnings: number;
  resolvedToday: number;
}

function calculateStats(alerts: AlertRecord[]): AlertStats {
  const now = new Date();

  const activeAlerts = alerts.filter(
    (alert) => !alert.is_resolved
  ).length;

  const critical = alerts.filter(
    (alert) =>
      !alert.is_resolved &&
      ["critical", "high"].includes(alert.severity.toLowerCase())
  ).length;

  const warnings = alerts.filter(
    (alert) =>
      !alert.is_resolved &&
      ["warning", "medium"].includes(alert.severity.toLowerCase())
  ).length;

  const resolvedToday = alerts.filter((alert) => {
    if (!alert.is_resolved || !alert.resolved_at) {
      return false;
    }

    const resolvedAt = new Date(alert.resolved_at);

    return (
      resolvedAt.getFullYear() === now.getFullYear() &&
      resolvedAt.getMonth() === now.getMonth() &&
      resolvedAt.getDate() === now.getDate()
    );
  }).length;

  return {
    activeAlerts,
    critical,
    warnings,
    resolvedToday,
  };
}

export default function AlertsPage() {
  const [stats, setStats] = useState<AlertStats>({
    activeAlerts: 0,
    critical: 0,
    warnings: 0,
    resolvedToday: 0,
  });

  useEffect(() => {
    let cancelled = false;

    async function loadStats() {
      try {
        const response = await fetch(
          "https://gbos-backend-production.up.railway.app/alerts",
          {
            cache: "no-store",
          }
        );

        if (!response.ok) {
          throw new Error("Failed to load alerts");
        }

        const alerts: AlertRecord[] = await response.json();

        if (!cancelled) {
          setStats(calculateStats(alerts));
        }
      } catch (error) {
        console.error("Failed to load alert stats:", error);
      }
    }

    loadStats();

    const interval = window.setInterval(loadStats, 5000);

    return () => {
      cancelled = true;
      window.clearInterval(interval);
    };
  }, []);

  return (
    <div className="min-h-screen bg-zinc-50">
      <Sidebar />
      <Topbar />

      <main className="ml-64 pt-20">
        <div className="space-y-6 p-8">
          {/* Page Header */}
          <div>
            <p className="text-sm font-medium text-zinc-500">
              Monitoring
            </p>

            <h1 className="mt-1 text-2xl font-semibold tracking-tight text-zinc-950">
              Alerts
            </h1>

            <p className="mt-2 text-sm text-zinc-500">
              Stay informed about issues across your company devices.
            </p>
          </div>

          {/* Stats */}
          <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
            <StatCard
              title="Active Alerts"
              value={String(stats.activeAlerts)}
              description="require attention"
              icon={Bell}
            />

            <StatCard
              title="Critical"
              value={String(stats.critical)}
              description="critical issues"
              icon={AlertCircle}
            />

            <StatCard
              title="Warnings"
              value={String(stats.warnings)}
              description="warnings detected"
              icon={AlertTriangle}
            />

            <StatCard
              title="Resolved Today"
              value={String(stats.resolvedToday)}
              description="alerts resolved"
              icon={CheckCircle2}
            />
          </div>

          {/* Alerts Table */}
          <AlertsTable />
        </div>
      </main>
    </div>
  );
}
