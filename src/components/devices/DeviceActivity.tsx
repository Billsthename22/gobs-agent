"use client";

import { useEffect, useState } from "react";
import {
  AppWindow,
  LogIn,
  LogOut,
  Monitor,
  UserRound,
  type LucideIcon,
} from "lucide-react";

interface Activity {
  id: number;
  device_id: number;
  employee_id: number | null;
  activity_type: string;
  description: string;
  timestamp: string;
}

interface DisplayActivity {
  id: number;
  Icon: LucideIcon;
  title: string;
  description: string;
  time: string;
}

function getActivityIcon(activityType: string): LucideIcon {
  switch (activityType.toUpperCase()) {
    case "LOGIN":
      return LogIn;

    case "LOGOUT":
      return LogOut;

    case "DEVICE_CONNECTED":
    case "DEVICE_DISCONNECTED":
      return Monitor;

    case "APPLICATION":
    case "APP_ACTIVITY":
      return AppWindow;

    case "HEARTBEAT":
      return Monitor;

    case "SESSION":
      return UserRound;

    default:
      return Monitor;
  }
}

function getActivityTitle(activity: Activity): string {
  switch (activity.activity_type.toUpperCase()) {
    case "LOGIN":
      return "User logged in";

    case "LOGOUT":
      return "User logged out";

    case "DEVICE_CONNECTED":
      return "Device connected";

    case "DEVICE_DISCONNECTED":
      return "Device disconnected";

    case "APPLICATION":
    case "APP_ACTIVITY":
      return "Application activity detected";

    case "HEARTBEAT":
      return "Device heartbeat received";

    case "SESSION":
      return "Employee session activity";

    default:
      return activity.activity_type.replace(/_/g, " ");
  }
}

function formatRelativeTime(timestamp: string): string {
  const activityTime = new Date(timestamp).getTime();
  const now = Date.now();

  const difference = Math.max(0, now - activityTime);
  const seconds = Math.floor(difference / 1000);

  if (seconds < 60) {
    return `${seconds}s ago`;
  }

  const minutes = Math.floor(seconds / 60);

  if (minutes < 60) {
    return `${minutes} min ago`;
  }

  const hours = Math.floor(minutes / 60);

  if (hours < 24) {
    return `${hours} hr ago`;
  }

  const days = Math.floor(hours / 24);

  return `${days}d ago`;
}

interface DeviceActivityProps {
  id: string;
}

export default function DeviceActivity({ id }: DeviceActivityProps) {
  const deviceId = Number(id);

  const [activities, setActivities] = useState<DisplayActivity[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    async function fetchActivities() {
      try {
        const response = await fetch(
          "https://gbos-backend-production.up.railway.app/activities",
          {
            cache: "no-store",
          }
        );

        if (!response.ok) {
          throw new Error("Failed to fetch activities");
        }

        const data: Activity[] = await response.json();

        const deviceActivities: DisplayActivity[] = data
          .filter((activity) => activity.device_id === deviceId)
          .sort(
            (a, b) =>
              new Date(b.timestamp).getTime() -
              new Date(a.timestamp).getTime()
          )
          .slice(0, 10)
          .map((activity) => ({
            id: activity.id,
            Icon: getActivityIcon(activity.activity_type),
            title: getActivityTitle(activity),
            description: activity.description,
            time: formatRelativeTime(activity.timestamp),
          }));

        setActivities(deviceActivities);
        setError(null);
      } catch (err) {
        console.error("Failed to load device activity:", err);
        setError("Unable to load device activity");
      } finally {
        setLoading(false);
      }
    }

    fetchActivities();

    const interval = setInterval(fetchActivities, 5000);

    return () => clearInterval(interval);
  }, [deviceId]);

  return (
    <div className="rounded-2xl border border-zinc-200 bg-white">
      <div className="border-b border-zinc-100 px-6 py-5">
        <h2 className="text-sm font-semibold text-zinc-950">
          Activity Timeline
        </h2>

        <p className="mt-1 text-xs text-zinc-500">
          Recent authorized activity from this device
        </p>
      </div>

      {loading ? (
        <div className="divide-y divide-zinc-100">
          {[1, 2, 3].map((item) => (
            <div
              key={item}
              className="flex items-center gap-4 px-6 py-4"
            >
              <div className="h-9 w-9 animate-pulse rounded-xl bg-zinc-100" />

              <div className="flex-1">
                <div className="h-4 w-40 animate-pulse rounded bg-zinc-100" />
                <div className="mt-2 h-3 w-56 animate-pulse rounded bg-zinc-100" />
              </div>
            </div>
          ))}
        </div>
      ) : error ? (
        <div className="px-6 py-8">
          <p className="text-sm text-zinc-500">{error}</p>
        </div>
      ) : activities.length === 0 ? (
        <div className="px-6 py-8">
          <p className="text-sm text-zinc-500">
            No activity recorded for this device yet.
          </p>
        </div>
      ) : (
        <div className="divide-y divide-zinc-100">
          {activities.map((activity) => {
            const { Icon } = activity;

            return (
              <div
                key={activity.id}
                className="flex items-center gap-4 px-6 py-4"
              >
                <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl bg-zinc-100">
                  <Icon
                    size={16}
                    strokeWidth={1.8}
                    className="text-zinc-600"
                  />
                </div>

                <div className="min-w-0 flex-1">
                  <p className="text-sm font-medium text-zinc-900">
                    {activity.title}
                  </p>

                  <p className="mt-0.5 text-xs text-zinc-500">
                    {activity.description}
                  </p>
                </div>

                <span className="shrink-0 text-xs text-zinc-400">
                  {activity.time}
                </span>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
