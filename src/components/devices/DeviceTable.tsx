"use client";

import {
  CheckCircle2,
  CircleAlert,
  MoreHorizontal,
  Search,
} from "lucide-react";
import Link from "next/link";
import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { getToken } from "@/lib/auth";

type Device = {
  id: number;
  device_name: string;
  hostname: string;
  operating_system: string;
  ip_address: string | null;
  employee_id: number | null;
  status: string;
  cpu_usage: number;
  memory_usage: number;
  storage_usage: number;
  last_seen: string | null;
  is_active: boolean;
  created_at: string;
};

type Employee = {
  id: number;
  full_name: string;
  email: string;
  department: string | null;
  job_title: string | null;
  phone: string | null;
  status: string;
  is_active: boolean;
  created_at: string;
};

function getDeviceCode(id: number) {
  return `PC-${String(id).padStart(3, "0")}`;
}

function formatLastSeen(lastSeen: string | null) {
  if (!lastSeen) {
    return "Never";
  }

  const timestamp = new Date(lastSeen).getTime();
  const now = Date.now();
  const seconds = Math.max(0, Math.floor((now - timestamp) / 1000));

  if (seconds < 10) {
    return "Active now";
  }

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

function formatStatus(status: string) {
  return status.charAt(0).toUpperCase() + status.slice(1);
}

export default function DeviceTable() {
  const [devices, setDevices] = useState<Device[]>([]);
  const [employees, setEmployees] = useState<Employee[]>([]);
  const [search, setSearch] = useState("");
  const [openMenuId, setOpenMenuId] = useState<number | null>(null);
  const [restartDevice, setRestartDevice] = useState<Device | null>(null);
  const [shutdownDevice, setShutdownDevice] = useState<Device | null>(null);
  const [selectedDeviceIds, setSelectedDeviceIds] = useState<number[]>([]);
  const [menuPosition, setMenuPosition] = useState<{
    top: number;
    left: number;
  } | null>(null);
  const [status, setStatus] = useState("All");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  async function fetchDevices() {
    try {
      const response = await fetch(
        "https://gbos-backend-production.up.railway.app/devices",
        {
          cache: "no-store",
        }
      );

      if (!response.ok) {
        throw new Error("Failed to fetch devices");
      }

      const data: Device[] = await response.json();

      const employeesResponse = await fetch(
        "https://gbos-backend-production.up.railway.app/employees",
        {
          cache: "no-store",
        }
      );

      if (!employeesResponse.ok) {
        throw new Error("Failed to fetch employees");
      }

      const employeesData: Employee[] = await employeesResponse.json();

      setDevices(data);
      setEmployees(employeesData);
      setError(null);
    } catch (err) {
      console.error(err);
      setError("Unable to load devices");
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    fetchDevices();

    const interval = setInterval(fetchDevices, 5000);

    return () => clearInterval(interval);
  }, []);

  async function toggleDeviceActive(device: Device) {
    try {
      const response = await fetch(
        `https://gbos-backend-production.up.railway.app/devices/${device.id}`,
        {
          method: "PATCH",
          headers: {
            "Content-Type": "application/json",
          },
          body: JSON.stringify({
            is_active: !device.is_active,
          }),
        }
      );

      if (!response.ok) {
        throw new Error("Failed to update device");
      }

      await fetchDevices();
      closeMenu();
    } catch (err) {
      console.error("Failed to update device status:", err);
      setError("Unable to update device");
    }
  }

  async function restartDeviceNow() {
    if (!restartDevice) return;

    try {
      setError("");

      const token = getToken();

      if (!token) {
        throw new Error("Authentication token not found");
      }

      const response = await fetch(
        `https://gbos-backend-production.up.railway.app/remote-monitoring/device/${restartDevice.id}/restart`,
        {
          method: "POST",
          headers: {
            Authorization: `Bearer ${token}`,
          },
        }
      );

      if (!response.ok) {
        const data = await response.json().catch(() => null);
        throw new Error(
          data?.detail || "Failed to restart device"
        );
      }

      setRestartDevice(null);
      closeMenu();
    } catch (err) {
      console.error("Failed to restart device:", err);
      setError(
        err instanceof Error
          ? err.message
          : "Unable to restart device"
      );
    }
  }

  async function shutdownDeviceNow() {
    if (!shutdownDevice) return;

    try {
      setError("");

      const token = getToken();

      if (!token) {
        throw new Error("Authentication token not found");
      }

      const response = await fetch(
        `https://gbos-backend-production.up.railway.app/remote-monitoring/device/${shutdownDevice.id}/shutdown`,
        {
          method: "POST",
          headers: {
            Authorization: `Bearer ${token}`,
          },
        }
      );

      if (!response.ok) {
        const data = await response.json().catch(() => null);
        throw new Error(
          data?.detail || "Failed to shut down device"
        );
      }

      setShutdownDevice(null);
      closeMenu();
    } catch (err) {
      console.error("Failed to shut down device:", err);
      setError(
        err instanceof Error
          ? err.message
          : "Unable to shut down device"
      );
    }
  }

  const filteredDevices = devices.filter((device) => {
    const deviceCode = getDeviceCode(device.id);

    const matchesSearch =
      deviceCode.toLowerCase().includes(search.toLowerCase()) ||
      device.device_name
        .toLowerCase()
        .includes(search.toLowerCase()) ||
      device.hostname
        .toLowerCase()
        .includes(search.toLowerCase()) ||
      String(device.employee_id ?? "")
        .toLowerCase()
        .includes(search.toLowerCase());

    const normalizedStatus = formatStatus(device.status);

    const matchesStatus =
      status === "All" || normalizedStatus === status;

    return matchesSearch && matchesStatus;
  });

  async function bulkUpdateDeviceStatus(isActive: boolean) {
    if (selectedDeviceIds.length === 0) return;

    try {
      setError("");

      await Promise.all(
        selectedDeviceIds.map(async (deviceId) => {
          const response = await fetch(
            `https://gbos-backend-production.up.railway.app/devices/${deviceId}`,
            {
              method: "PATCH",
              headers: {
                "Content-Type": "application/json",
              },
              body: JSON.stringify({
                is_active: isActive,
              }),
            }
          );

          if (!response.ok) {
            throw new Error(`Failed to update device ${deviceId}`);
          }
        })
      );

      await fetchDevices();
      setSelectedDeviceIds([]);
    } catch (err) {
      console.error("Failed to update devices:", err);
      setError("Unable to update selected devices");
    }
  }

  function toggleDeviceSelection(deviceId: number) {
    setSelectedDeviceIds((current) =>
      current.includes(deviceId)
        ? current.filter((id) => id !== deviceId)
        : [...current, deviceId]
    );
  }

  function toggleSelectAll() {
    const visibleIds = filteredDevices.map((device) => device.id);
    const allSelected =
      visibleIds.length > 0 &&
      visibleIds.every((id) => selectedDeviceIds.includes(id));

    setSelectedDeviceIds(allSelected ? [] : visibleIds);
  }

  const allVisibleSelected =
    filteredDevices.length > 0 &&
    filteredDevices.every((device) =>
      selectedDeviceIds.includes(device.id)
    );

  function closeMenu() {
    setOpenMenuId(null);
    setMenuPosition(null);
  }

  const selectedDevice =
    openMenuId === null
      ? null
      : devices.find((device) => device.id === openMenuId) ?? null;
  const selectedEmployee = selectedDevice?.employee_id
    ? employees.find((employee) => employee.id === selectedDevice.employee_id) ?? null
    : null;

  return (
    <div className="rounded-2xl border border-zinc-200 bg-white">
      {/* Toolbar */}
      <div className="flex flex-col gap-4 border-b border-zinc-100 p-5 lg:flex-row lg:items-center lg:justify-between">
        <div className="relative w-full lg:w-80">
          <Search
            size={17}
            className="absolute left-3 top-1/2 -translate-y-1/2 text-zinc-400"
          />

          <input
            type="text"
            placeholder="Search devices..."
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            className="h-10 w-full rounded-xl border border-zinc-200 bg-zinc-50 pl-10 pr-4 text-sm outline-none transition focus:border-zinc-400 focus:bg-white"
          />
        </div>

        <div className="flex items-center gap-2">
          {["All", "Online", "Stale", "Offline"].map((item) => (
            <button
              key={item}
              type="button"
              onClick={() => setStatus(item)}
              className={`rounded-lg px-3 py-2 text-xs font-medium transition ${
                status === item
                  ? "bg-zinc-950 text-white"
                  : "border border-zinc-200 text-zinc-500 hover:bg-zinc-50"
              }`}
            >
              {item}
            </button>
          ))}
        </div>
      </div>

      {/* Bulk Actions */}
      {!loading && !error && selectedDeviceIds.length > 0 && (
        <div className="mx-4 mb-4 flex items-center justify-between rounded-xl border border-zinc-200 bg-zinc-50 px-4 py-3">
          <div className="flex items-center gap-3">
            <span className="text-sm font-medium text-zinc-900">
              {selectedDeviceIds.length} device
              {selectedDeviceIds.length === 1 ? "" : "s"} selected
            </span>

            <button
              type="button"
              onClick={() => bulkUpdateDeviceStatus(true)}
              className="rounded-lg border border-zinc-200 bg-white px-3 py-2 text-xs font-medium text-zinc-700 transition hover:bg-zinc-100"
            >
              Activate
            </button>

            <button
              type="button"
              onClick={() => bulkUpdateDeviceStatus(false)}
              className="rounded-lg border border-zinc-200 bg-white px-3 py-2 text-xs font-medium text-zinc-700 transition hover:bg-zinc-100"
            >
              Deactivate
            </button>
          </div>

          <button
            type="button"
            onClick={() => setSelectedDeviceIds([])}
            className="text-xs font-medium text-zinc-500 transition hover:text-zinc-900"
          >
            Clear selection
          </button>
        </div>
      )}

      {/* Loading */}
      {loading && (
        <div className="p-12 text-center">
          <p className="text-sm text-zinc-500">
            Loading devices...
          </p>
        </div>
      )}

      {/* Error */}
      {!loading && error && (
        <div className="p-12 text-center">
          <p className="text-sm font-medium text-zinc-900">
            Unable to load devices
          </p>
          <p className="mt-1 text-xs text-zinc-500">
            {error}
          </p>
        </div>
      )}

      {/* Table */}
      {!loading && !error && (
        <div className="overflow-x-auto">
          <table className="w-full min-w-[900px]">
            <thead>
              <tr className="border-b border-zinc-100 text-left">
                <th className="w-12 px-4 py-4">
                  <input
                    type="checkbox"
                    checked={allVisibleSelected}
                    onChange={toggleSelectAll}
                    disabled={filteredDevices.length === 0}
                    aria-label="Select all visible devices"
                    className="h-4 w-4 rounded border-zinc-300"
                  />
                </th>

                <th className="px-6 py-4 text-[10px] font-semibold uppercase tracking-wider text-zinc-400">
                  Device
                </th>

                <th className="px-4 py-4 text-[10px] font-semibold uppercase tracking-wider text-zinc-400">
                  Employee
                </th>

                <th className="px-4 py-4 text-[10px] font-semibold uppercase tracking-wider text-zinc-400">
                  Status
                </th>

                <th className="px-4 py-4 text-[10px] font-semibold uppercase tracking-wider text-zinc-400">
                  CPU
                </th>

                <th className="px-4 py-4 text-[10px] font-semibold uppercase tracking-wider text-zinc-400">
                  Memory
                </th>

                <th className="px-4 py-4 text-[10px] font-semibold uppercase tracking-wider text-zinc-400">
                  Storage
                </th>

                <th className="px-4 py-4 text-[10px] font-semibold uppercase tracking-wider text-zinc-400">
                  Last Active
                </th>

                <th className="px-4 py-4" />
              </tr>
            </thead>

            <tbody className="divide-y divide-zinc-100">
              {filteredDevices.map((device) => {
                const deviceCode = getDeviceCode(device.id);
                const normalizedStatus = formatStatus(device.status);
                const isOnline = device.status === "online";
                const employee = device.employee_id
                  ? employees.find(
                      (item) => item.id === device.employee_id
                    )
                  : null;

                return (
                  <tr
                    key={device.id}
                    className="group transition hover:bg-zinc-50"
                  >
                    <td className="px-4 py-4">
                      <input
                        type="checkbox"
                        checked={selectedDeviceIds.includes(device.id)}
                        onChange={() => toggleDeviceSelection(device.id)}
                        aria-label={`Select ${device.device_name}`}
                        className="h-4 w-4 rounded border-zinc-300"
                      />
                    </td>

                    <td className="px-6 py-4">
                      <Link href={`/devices/${device.id}`}>
                        <div className="flex items-center gap-3">
                          <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-zinc-100">
                            <span className="text-xs font-semibold text-zinc-600">
                              PC
                            </span>
                          </div>

                          <div>
                            <p className="text-sm font-medium text-zinc-900">
                              {deviceCode}
                            </p>

                            <p className="mt-0.5 text-xs text-zinc-500">
                              {device.device_name}
                            </p>

                            <p className="mt-0.5 text-[10px] text-zinc-400">
                              {device.hostname}
                            </p>
                          </div>
                        </div>
                      </Link>
                    </td>

                    <td className="px-4 py-4">
                      <p className="text-sm text-zinc-700">
                        {employee?.full_name ?? "Unassigned"}
                      </p>

                      <p className="mt-0.5 text-xs text-zinc-400">
                        {employee?.department ??
                          device.operating_system}
                      </p>
                    </td>

                    <td className="px-4 py-4">
                      <div className="flex items-center gap-2">
                        {isOnline ? (
                          <CheckCircle2
                            size={15}
                            className="text-emerald-500"
                          />
                        ) : (
                          <CircleAlert
                            size={15}
                            className="text-zinc-400"
                          />
                        )}

                        <span
                          className={`text-xs font-medium ${
                            isOnline
                              ? "text-emerald-600"
                              : "text-zinc-500"
                          }`}
                        >
                          {normalizedStatus}
                        </span>
                      </div>
                    </td>

                    <td className="px-4 py-4">
                      <span className="text-sm text-zinc-700">
                        {device.status === "offline"
                          ? "—"
                          : `${device.cpu_usage.toFixed(1)}%`}
                      </span>
                    </td>

                    <td className="px-4 py-4">
                      <span className="text-sm text-zinc-700">
                        {device.status === "offline"
                          ? "—"
                          : `${device.memory_usage.toFixed(1)}%`}
                      </span>
                    </td>

                    <td className="px-4 py-4">
                      <div className="w-24">
                        <div className="flex justify-between text-xs">
                          <span className="text-zinc-600">
                            {device.storage_usage.toFixed(1)}%
                          </span>
                        </div>

                        <div className="mt-1.5 h-1.5 overflow-hidden rounded-full bg-zinc-100">
                          <div
                            className="h-full rounded-full bg-zinc-800"
                            style={{
                              width: `${Math.min(
                                100,
                                Math.max(0, device.storage_usage)
                              )}%`,
                            }}
                          />
                        </div>
                      </div>
                    </td>

                    <td className="px-4 py-4">
                      <span className="text-xs text-zinc-500">
                        {formatLastSeen(device.last_seen)}
                      </span>
                    </td>

                    <td className="px-4 py-4">
                      <div className="relative flex justify-end">
                        <button
                          type="button"
                          aria-label={`Device options for ${device.device_name}`}
                          aria-expanded={openMenuId === device.id}
                          onClick={(event) => {
                            if (openMenuId === device.id) {
                              closeMenu();
                              return;
                            }

                            const rect =
                              event.currentTarget.getBoundingClientRect();

                            setMenuPosition({
                              top: rect.bottom + 8,
                              left: Math.max(8, rect.right - 192),
                            });
                            setOpenMenuId(device.id);
                          }}
                          className="flex h-8 w-8 items-center justify-center rounded-lg text-zinc-400 opacity-0 transition group-hover:opacity-100 hover:bg-zinc-100 hover:text-zinc-900 focus:opacity-100 focus:outline-none focus:ring-2 focus:ring-zinc-200"
                        >
                          <MoreHorizontal size={17} />
                        </button>

                      </div>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>

          {filteredDevices.length === 0 && (
            <div className="p-12 text-center">
              <p className="text-sm font-medium text-zinc-900">
                No devices found
              </p>

              <p className="mt-1 text-xs text-zinc-500">
                Try changing your search or status filter.
              </p>
            </div>
          )}
        </div>
      )}

      {selectedDevice && menuPosition && typeof document !== "undefined" &&
        createPortal(
          <div
            className="fixed z-[100] w-48 overflow-hidden rounded-xl border border-zinc-200 bg-white p-1.5 shadow-xl"
            style={menuPosition}
            role="menu"
          >
            <Link
              href={`/devices/${selectedDevice.id}`}
              onClick={closeMenu}
              className="block rounded-lg px-3 py-2 text-sm text-zinc-700 transition hover:bg-zinc-50"
              role="menuitem"
            >
              View device
            </Link>

            {selectedEmployee && (
              <Link
                href={`/employees/${selectedEmployee.id}`}
                onClick={closeMenu}
                className="block rounded-lg px-3 py-2 text-sm text-zinc-700 transition hover:bg-zinc-50"
                role="menuitem"
              >
                View employee
              </Link>
            )}

            {selectedDevice.status === "online" && (
              <Link
                href={`/devices/${selectedDevice.id}/monitor`}
                onClick={closeMenu}
                className="block rounded-lg px-3 py-2 text-sm text-zinc-700 transition hover:bg-zinc-50"
                role="menuitem"
              >
                Remote monitor
              </Link>
            )}

            <button
              type="button"
              onClick={() => {
                setRestartDevice(selectedDevice);
                closeMenu();
              }}
              className="block w-full rounded-lg px-3 py-2 text-left text-sm text-zinc-700 transition hover:bg-zinc-50"
              role="menuitem"
            >
              Restart device
            </button>

            <button
              type="button"
              onClick={() => {
                setShutdownDevice(selectedDevice);
                closeMenu();
              }}
              className="block w-full rounded-lg px-3 py-2 text-left text-sm text-zinc-700 transition hover:bg-zinc-50"
              role="menuitem"
            >
              Shutdown device
            </button>

            <button
              type="button"
              onClick={() => toggleDeviceActive(selectedDevice)}
              className="block w-full rounded-lg px-3 py-2 text-left text-sm text-zinc-700 transition hover:bg-zinc-50"
              role="menuitem"
            >
              {selectedDevice.is_active
                ? "Deactivate device"
                : "Activate device"}
            </button>
          </div>,
          document.body
        )}

      {restartDevice && (
        <div className="fixed inset-0 z-[200] flex items-center justify-center bg-black/40 px-4">
          <div className="w-full max-w-md rounded-2xl border border-zinc-200 bg-white p-6 shadow-2xl">
            <div>
              <h2 className="text-lg font-semibold text-zinc-950">
                Restart device?
              </h2>

              <p className="mt-2 text-sm leading-6 text-zinc-500">
                This will restart{" "}
                <span className="font-medium text-zinc-900">
                  {restartDevice.device_name}
                </span>
                . Any unsaved work on the device may be lost.
              </p>
            </div>

            <div className="mt-6 flex justify-end gap-3">
              <button
                type="button"
                onClick={() => setRestartDevice(null)}
                className="rounded-xl border border-zinc-200 px-4 py-2.5 text-sm font-medium text-zinc-700 transition hover:bg-zinc-50"
              >
                Cancel
              </button>

              <button
                type="button"
                onClick={restartDeviceNow}
                className="rounded-xl bg-zinc-950 px-4 py-2.5 text-sm font-medium text-white transition hover:bg-zinc-800"
              >
                Restart device
              </button>
            </div>
          </div>
        </div>
      )}


      {shutdownDevice && (
        <div className="fixed inset-0 z-[200] flex items-center justify-center bg-black/40 px-4">
          <div className="w-full max-w-md rounded-2xl border border-zinc-200 bg-white p-6 shadow-2xl">
            <div>
              <h2 className="text-lg font-semibold text-zinc-950">
                Shut down device?
              </h2>

              <p className="mt-2 text-sm leading-6 text-zinc-500">
                This will completely shut down{" "}
                <span className="font-medium text-zinc-900">
                  {shutdownDevice.device_name}
                </span>
                . Any unsaved work may be lost, and the device will need to be
                powered on again before it can reconnect to GBOS.
              </p>
            </div>

            <div className="mt-6 flex justify-end gap-3">
              <button
                type="button"
                onClick={() => setShutdownDevice(null)}
                className="rounded-xl border border-zinc-200 px-4 py-2.5 text-sm font-medium text-zinc-700 transition hover:bg-zinc-50"
              >
                Cancel
              </button>

              <button
                type="button"
                onClick={shutdownDeviceNow}
                className="rounded-xl bg-zinc-950 px-4 py-2.5 text-sm font-medium text-white transition hover:bg-zinc-800"
              >
                Shut down device
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
