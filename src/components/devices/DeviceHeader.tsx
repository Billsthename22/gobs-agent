"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import {
  ArrowLeft,
  CheckCircle2,
  Circle,
  MoreHorizontal,
  Monitor,
} from "lucide-react";

interface DeviceHeaderProps {
  id: string;
}

type Device = {
  id: number;
  device_name: string;
  hostname: string;
  operating_system: string;
  status: string;
  employee_id: number | null;
};

function getDeviceCode(id: number) {
  return `PC-${String(id).padStart(3, "0")}`;
}

function formatStatus(status: string) {
  return status.charAt(0).toUpperCase() + status.slice(1);
}

export default function DeviceHeader({ id }: DeviceHeaderProps) {
  const [device, setDevice] = useState<Device | null>(null);

  useEffect(() => {
    async function fetchDevice() {
      try {
        const response = await fetch(
          `https://gbos-backend-production.up.railway.app/devices/${id}`,
          {
            cache: "no-store",
          }
        );

        if (!response.ok) {
          throw new Error("Failed to fetch device");
        }

        const data: Device = await response.json();
        setDevice(data);
      } catch (error) {
        console.error("Failed to load device header:", error);
      }
    }

    fetchDevice();

    const interval = setInterval(fetchDevice, 5000);

    return () => clearInterval(interval);
  }, [id]);

  const deviceCode = device ? getDeviceCode(device.id) : id;
  const status = device?.status ?? "offline";
  const isOnline = status === "online";

  return (
    <div className="flex flex-col gap-5 lg:flex-row lg:items-center lg:justify-between">
      <div>
        <Link
          href="/devices"
          className="mb-4 inline-flex items-center gap-2 text-xs text-zinc-500 transition hover:text-zinc-950"
        >
          <ArrowLeft size={14} />
          <span>Back to devices</span>
        </Link>

        <div className="flex items-center gap-4">
          <div className="flex h-14 w-14 items-center justify-center rounded-2xl bg-zinc-950 text-white">
            <span className="text-sm font-semibold">PC</span>
          </div>

          <div>
            <div className="flex items-center gap-2">
              <h1 className="text-2xl font-semibold tracking-tight text-zinc-950">
                {deviceCode}
              </h1>

              <span
                className={`flex items-center gap-1.5 rounded-full px-2.5 py-1 text-[10px] font-medium ${
                  isOnline
                    ? "bg-emerald-50 text-emerald-600"
                    : status === "stale"
                      ? "bg-amber-50 text-amber-600"
                      : "bg-zinc-100 text-zinc-500"
                }`}
              >
                {isOnline ? (
                  <CheckCircle2 size={12} />
                ) : (
                  <Circle size={12} />
                )}
                {formatStatus(status)}
              </span>
            </div>

            <p className="mt-1 text-sm text-zinc-500">
              {device
                ? `${device.device_name} · ${device.hostname}`
                : "Loading device..."}
            </p>
          </div>
        </div>
      </div>

      <div className="flex items-center gap-3">
        <Link
          href={`/devices/${id}/monitor`}
          className="flex items-center gap-2 rounded-xl bg-zinc-950 px-4 py-2.5 text-xs font-medium text-white transition hover:bg-zinc-800"
        >
          <Monitor size={14} />
          Live Monitor
        </Link>

        <button
          type="button"
          aria-label="Device options"
          className="flex h-10 w-10 items-center justify-center rounded-xl border border-zinc-200 bg-white text-zinc-500 transition hover:bg-zinc-50 hover:text-zinc-950"
        >
          <MoreHorizontal size={18} />
        </button>
      </div>
    </div>
  );
}
