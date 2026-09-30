"use client";

import { useEffect, useState } from "react";
import {
  Cpu,
  HardDrive,
  MemoryStick,
  Wifi,
} from "lucide-react";

type DeviceHealthData = {
  id: number;
  device_name: string;
  hostname: string;
  status: string;
  cpu_usage: number;
  memory_usage: number;
  storage_usage: number;
  last_seen: string | null;
};

type Metric = {
  label: string;
  value: string;
  detail: string;
  icon: typeof Cpu;
};

interface DeviceHealthProps {
  id: string;
}

export default function DeviceHealth({ id }: DeviceHealthProps) {
  const [device, setDevice] = useState<DeviceHealthData | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    async function fetchDeviceHealth() {
      try {
        const response = await fetch(
          `https://gbos-backend-production.up.railway.app/devices/${id}`,
          {
            cache: "no-store",
          }
        );

        if (!response.ok) {
          throw new Error("Failed to fetch device health");
        }

        const data: DeviceHealthData = await response.json();

        setDevice(data);
      } catch (err) {
        console.error(err);
        setError("Unable to load device health");
      } finally {
        setLoading(false);
      }
    }

    fetchDeviceHealth();

    const interval = setInterval(fetchDeviceHealth, 5000);

    return () => clearInterval(interval);
  }, [id]);

  if (loading) {
    return (
      <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-4">
        {[1, 2, 3, 4].map((item) => (
          <div
            key={item}
            className="h-32 animate-pulse rounded-2xl border border-zinc-200 bg-white"
          />
        ))}
      </div>
    );
  }

  if (error || !device) {
    return (
      <div className="rounded-2xl border border-zinc-200 bg-white p-5">
        <p className="text-sm text-zinc-500">
          {error ?? "No device telemetry available"}
        </p>
      </div>
    );
  }

  const metrics: Metric[] = [
    {
      label: "CPU Usage",
      value: `${device.cpu_usage.toFixed(1)}%`,
      detail:
        device.cpu_usage < 70
          ? "Normal"
          : device.cpu_usage < 90
            ? "High"
            : "Critical",
      icon: Cpu,
    },
    {
      label: "Memory",
      value: `${device.memory_usage.toFixed(1)}%`,
      detail:
        device.memory_usage < 70
          ? "Normal"
          : device.memory_usage < 90
            ? "High"
            : "Critical",
      icon: MemoryStick,
    },
    {
      label: "Storage",
      value: `${device.storage_usage.toFixed(1)}%`,
      detail:
        device.storage_usage < 70
          ? "Healthy"
          : device.storage_usage < 90
            ? "Getting full"
            : "Almost full",
      icon: HardDrive,
    },
    {
      label: "Network",
      value:
        device.status === "online"
          ? "Connected"
          : device.status === "stale"
            ? "Stale"
            : "Offline",
      detail:
        device.status === "online"
          ? "Connected"
          : device.status === "stale"
            ? "Connection stale"
            : "No connection",
      icon: Wifi,
    },
  ];

  return (
    <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-4">
      {metrics.map((metric) => {
        const Icon = metric.icon;

        return (
          <div
            key={metric.label}
            className="rounded-2xl border border-zinc-200 bg-white p-5"
          >
            <div className="flex items-center justify-between">
              <p className="text-xs text-zinc-500">
                {metric.label}
              </p>

              <div className="flex h-9 w-9 items-center justify-center rounded-xl bg-zinc-100">
                <Icon size={17} className="text-zinc-600" />
              </div>
            </div>

            <p className="mt-4 text-2xl font-semibold tracking-tight text-zinc-950">
              {metric.value}
            </p>

            <p className="mt-1 text-xs text-zinc-400">
              {metric.detail}
            </p>
          </div>
        );
      })}
    </div>
  );
}