"use client";

import {
  Activity,
  Laptop,
  LogIn,
  Settings2,
} from "lucide-react";
import { useEffect, useState } from "react";

import Sidebar from "@/components/layout/Sidebar";
import Topbar from "@/components/layout/Topbar";
import StatCard from "@/components/dashboard/StatCard";
import ActivityTable from "@/components/activity/ActivityTable";

interface ActivityRecord {
  id: number;
  device_id: number;
  employee_id: number | null;
  activity_type: string;
  description: string;
  timestamp: string;
}

interface Device {
  id: number;
  employee_id: number | null;
  status: string;
}

interface ActivityStats {
  eventsToday: number;
  activeSessions: number;
  applicationEvents: number;
  systemEvents: number;
}

function calculateStats(
  activities: ActivityRecord[],
  devices: Device[]
): ActivityStats {
  const now = new Date();

  const eventsToday = activities.filter((activity) => {
    const timestamp = new Date(activity.timestamp);

    return (
      timestamp.getFullYear() === now.getFullYear() &&
      timestamp.getMonth() === now.getMonth() &&
      timestamp.getDate() === now.getDate()
    );
  }).length;

  const applicationEvents = activities.filter(
    (activity) =>
      activity.activity_type === "APPLICATION_ACTIVE"
  ).length;

  const systemEvents = activities.filter(
    (activity) =>
      activity.activity_type !== "APPLICATION_ACTIVE"
  ).length;

  const activeSessions = new Set(
    devices
      .filter(
        (device) =>
          device.status.toLowerCase() === "online" &&
          device.employee_id !== null
      )
      .map((device) => device.employee_id)
  ).size;

  return {
    eventsToday,
    activeSessions,
    applicationEvents,
    systemEvents,
  };
}

export default function ActivityPage() {
  const [stats, setStats] = useState<ActivityStats>({
    eventsToday: 0,
    activeSessions: 0,
    applicationEvents: 0,
    systemEvents: 0,
  });

  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;

    async function loadStats() {
      try {
        const [
          activitiesResponse,
          devicesResponse,
        ] = await Promise.all([
          fetch(
            "https://gbos-backend-production.up.railway.app/activities",
            {
              cache: "no-store",
            }
          ),
          fetch(
            "https://gbos-backend-production.up.railway.app/devices",
            {
              cache: "no-store",
            }
          ),
        ]);

        if (!activitiesResponse.ok) {
          throw new Error(
            "Failed to load activities"
          );
        }

        if (!devicesResponse.ok) {
          throw new Error(
            "Failed to load devices"
          );
        }

        const [
          activities,
          devices,
        ]: [ActivityRecord[], Device[]] =
          await Promise.all([
            activitiesResponse.json(),
            devicesResponse.json(),
          ]);

        if (!cancelled) {
          setStats(
            calculateStats(
              activities,
              devices
            )
          );
        }
      } catch (error) {
        console.error(
          "Failed to load activity stats:",
          error
        );
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    }

    loadStats();

    const interval = window.setInterval(
      loadStats,
      5000
    );

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
              Activity
            </h1>

            <p className="mt-2 text-sm text-zinc-500">
              Everything happening across your company devices.
            </p>
          </div>

          {/* Stats */}
          <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
            <StatCard
              title="Events Today"
              value={
                loading
                  ? "..."
                  : stats.eventsToday.toLocaleString()
              }
              description="recorded events today"
              icon={Activity}
            />

            <StatCard
              title="Active Sessions"
              value={
                loading
                  ? "..."
                  : stats.activeSessions.toLocaleString()
              }
              description="employees currently online"
              icon={Laptop}
            />

            <StatCard
              title="Application Events"
              value={
                loading
                  ? "..."
                  : stats.applicationEvents.toLocaleString()
              }
              description="application activity"
              icon={LogIn}
            />

            <StatCard
              title="System Events"
              value={
                loading
                  ? "..."
                  : stats.systemEvents.toLocaleString()
              }
              description="device and system events"
              icon={Settings2}
            />
          </div>

          {/* Activity Table */}
          <ActivityTable />
        </div>
      </main>
    </div>
  );
}
