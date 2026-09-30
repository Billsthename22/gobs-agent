"use client";

import { useEffect, useState } from "react";
import { getToken } from "@/lib/auth";
import { Check, MapPin, Monitor, UserRound, X } from "lucide-react";

interface DeviceInfoProps {
  id: string;
}

type Device = {
  id: number;
  device_name: string;
  hostname: string;
  operating_system: string;
  ip_address: string | null;
  location_name: string | null;
  location_address: string | null;
  latitude: number | null;
  longitude: number | null;
  location_updated_at: string | null;
  gps_enabled: boolean;
  gps_latitude: number | null;
  gps_longitude: number | null;
  gps_updated_at: string | null;
  employee_id: number | null;
  last_seen: string | null;
};

type Employee = {
  id: number;
  full_name: string;
  email: string;
  department: string | null;
  job_title: string | null;
  phone: string | null;
};

function formatLastSeen(lastSeen: string | null) {
  if (!lastSeen) return "Never";

  const timestamp = new Date(lastSeen).getTime();

  if (!Number.isFinite(timestamp)) {
    return "Unknown";
  }

  const seconds = Math.max(
    0,
    Math.floor((Date.now() - timestamp) / 1000)
  );

  if (seconds < 60) {
    return `${seconds} second${seconds === 1 ? "" : "s"} ago`;
  }

  const minutes = Math.floor(seconds / 60);

  if (minutes < 60) {
    return `${minutes} minute${minutes === 1 ? "" : "s"} ago`;
  }

  const hours = Math.floor(minutes / 60);

  return `${hours} hour${hours === 1 ? "" : "s"} ago`;
}

export default function DeviceInfo({ id }: DeviceInfoProps) {
  const [device, setDevice] = useState<Device | null>(null);
  const [employee, setEmployee] = useState<Employee | null>(null);
  const [employees, setEmployees] = useState<Employee[]>([]);
  const [showEmployeeSelector, setShowEmployeeSelector] = useState(false);
  const [savingAssignment, setSavingAssignment] = useState(false);
  const [assignmentError, setAssignmentError] = useState("");

  const [savingGps, setSavingGps] = useState(false);
  const [gpsError, setGpsError] = useState("");

  const [showLocationEditor, setShowLocationEditor] = useState(false);
  const [locationName, setLocationName] = useState("");
  const [locationAddress, setLocationAddress] = useState("");
  const [latitude, setLatitude] = useState("");
  const [longitude, setLongitude] = useState("");
  const [savingLocation, setSavingLocation] = useState(false);
  const [locationError, setLocationError] = useState("");

  useEffect(() => {
    async function fetchDeviceInfo() {
      try {
        const response = await fetch(
          `https://gbos-backend-production.up.railway.app/devices/${id}`,
          {
            cache: "no-store",
          }
        );

        if (!response.ok) {
          throw new Error("Failed to fetch device information");
        }

        const data: Device = await response.json();
        setDevice(data);

        const employeesResponse = await fetch(
          "https://gbos-backend-production.up.railway.app/employees",
          {
            cache: "no-store",
          }
        );

        if (employeesResponse.ok) {
          const employeeData: Employee[] =
            await employeesResponse.json();

          setEmployees(employeeData);

          const assignedEmployee = data.employee_id
            ? employeeData.find(
                (item) => item.id === data.employee_id
              ) ?? null
            : null;

          setEmployee(assignedEmployee);
        } else {
          setEmployee(null);
        }
      } catch (error) {
        console.error("Failed to load device information:", error);
      }
    }

    fetchDeviceInfo();

    const interval = setInterval(fetchDeviceInfo, 5000);

    return () => clearInterval(interval);
  }, [id]);

  async function assignEmployee(employeeId: number | null) {
    if (!device) return;

    setSavingAssignment(true);
    setAssignmentError("");

    try {
      const response = await fetch(
        `https://gbos-backend-production.up.railway.app/devices/${device.id}`,
        {
          method: "PATCH",
          headers: {
            "Content-Type": "application/json",
          },
          body: JSON.stringify({
            employee_id: employeeId,
          }),
        }
      );

      if (!response.ok) {
        throw new Error("Failed to update employee assignment");
      }

      const updatedDevice: Device = await response.json();
      setDevice(updatedDevice);

      const assignedEmployee = employeeId
        ? employees.find((item) => item.id === employeeId) ?? null
        : null;

      setEmployee(assignedEmployee);
      setShowEmployeeSelector(false);
    } catch (error) {
      console.error(
        "Failed to update employee assignment:",
        error
      );
      setAssignmentError("Unable to update assignment");
    } finally {
      setSavingAssignment(false);
    }
  }

  async function toggleGps() {
    if (!device) return;

    setSavingGps(true);
    setGpsError("");

    try {
      const response = await fetch(
        `https://gbos-backend-production.up.railway.app/devices/${device.id}/gps`,
        {
          method: "PATCH",
          headers: {
            "Content-Type": "application/json",
            Authorization: `Bearer ${getToken() ?? ""}`,
          },
          body: JSON.stringify({
            enabled: !device.gps_enabled,
          }),
        }
      );

      if (!response.ok) {
        const errorData = await response.json().catch(() => null);
        throw new Error(
          errorData?.detail ?? "Failed to update GPS collection"
        );
      }

      const data = await response.json();

      setDevice((current) =>
        current
          ? {
              ...current,
              gps_enabled: data.gps_enabled,
            }
          : current
      );
    } catch (error) {
      console.error("Failed to update GPS collection:", error);
      setGpsError(
        error instanceof Error
          ? error.message
          : "Unable to update GPS collection"
      );
    } finally {
      setSavingGps(false);
    }
  }

  function openLocationEditor() {
    if (!device) return;

    setLocationName(device.location_name ?? "");
    setLocationAddress(device.location_address ?? "");
    setLatitude(
      device.latitude !== null && device.latitude !== undefined
        ? String(device.latitude)
        : ""
    );
    setLongitude(
      device.longitude !== null && device.longitude !== undefined
        ? String(device.longitude)
        : ""
    );
    setLocationError("");
    setShowLocationEditor(true);
  }

  async function saveLocation() {
    if (!device) return;

    setSavingLocation(true);
    setLocationError("");

    const parsedLatitude =
      latitude.trim() === "" ? null : Number(latitude);
    const parsedLongitude =
      longitude.trim() === "" ? null : Number(longitude);

    if (
      (parsedLatitude !== null && !Number.isFinite(parsedLatitude)) ||
      (parsedLongitude !== null && !Number.isFinite(parsedLongitude))
    ) {
      setLocationError("Latitude and longitude must be valid numbers.");
      setSavingLocation(false);
      return;
    }

    if (
      parsedLatitude !== null &&
      (parsedLatitude < -90 || parsedLatitude > 90)
    ) {
      setLocationError("Latitude must be between -90 and 90.");
      setSavingLocation(false);
      return;
    }

    if (
      parsedLongitude !== null &&
      (parsedLongitude < -180 || parsedLongitude > 180)
    ) {
      setLocationError("Longitude must be between -180 and 180.");
      setSavingLocation(false);
      return;
    }

    try {
      const response = await fetch(
        `https://gbos-backend-production.up.railway.app/devices/${device.id}`,
        {
          method: "PATCH",
          headers: {
            "Content-Type": "application/json",
          },
          body: JSON.stringify({
            location_name:
              locationName.trim() || null,
            location_address:
              locationAddress.trim() || null,
            latitude: parsedLatitude,
            longitude: parsedLongitude,
          }),
        }
      );

      if (!response.ok) {
        const data = await response.json().catch(() => null);

        throw new Error(
          data?.detail || "Failed to update device location"
        );
      }

      const updatedDevice: Device = await response.json();

      setDevice(updatedDevice);
      setShowLocationEditor(false);
    } catch (error) {
      console.error(
        "Failed to update device location:",
        error
      );

      setLocationError(
        error instanceof Error
          ? error.message
          : "Unable to update device location"
      );
    } finally {
      setSavingLocation(false);
    }
  }

  return (
    <div className="rounded-2xl border border-zinc-200 bg-white">
      <div className="border-b border-zinc-100 px-6 py-5">
        <h2 className="text-sm font-semibold text-zinc-950">
          Device Information
        </h2>

        <p className="mt-1 text-xs text-zinc-500">
          Hardware and assignment details
        </p>
      </div>

      <div className="grid gap-x-8 gap-y-6 p-6 sm:grid-cols-2">
        <div>
          <div className="flex items-center gap-2 text-xs text-zinc-400">
            <Monitor size={14} />
            Operating System
          </div>
          <p className="mt-2 text-sm font-medium text-zinc-900">
            {device?.operating_system ?? "Loading..."}
          </p>
        </div>

        <div>
          <div className="flex items-center gap-2 text-xs text-zinc-400">
            <Monitor size={14} />
            Device
          </div>
          <p className="mt-2 text-sm font-medium text-zinc-900">
            {device?.device_name ?? "Loading..."}
          </p>
        </div>

        <div>
          <div className="flex items-center gap-2 text-xs text-zinc-400">
            <UserRound size={14} />
            Assigned Employee
          </div>

          <div className="mt-2">
            {employee ? (
              <>
                <p className="text-sm font-semibold text-zinc-900">
                  {employee.full_name}
                </p>

                <p className="mt-1 text-xs text-zinc-500">
                  {employee.job_title || "No job title"}
                  {employee.department
                    ? ` · ${employee.department}`
                    : ""}
                </p>

                <p className="mt-2 text-xs text-zinc-500">
                  {employee.email}
                </p>

                {employee.phone && (
                  <p className="mt-1 text-xs text-zinc-500">
                    {employee.phone}
                  </p>
                )}
              </>
            ) : (
              <p className="text-sm font-medium text-zinc-900">
                Unassigned
              </p>
            )}

            <button
              type="button"
              onClick={() => setShowEmployeeSelector(true)}
              className="mt-3 text-xs font-medium text-zinc-700 underline underline-offset-4 transition hover:text-zinc-950"
            >
              {employee ? "Change Employee" : "Assign Employee"}
            </button>

            {assignmentError && (
              <p className="mt-2 text-xs text-red-600">
                {assignmentError}
              </p>
            )}

            {showEmployeeSelector && (
              <div className="mt-4 rounded-xl border border-zinc-200 bg-zinc-50 p-3">
                <div className="mb-3 flex items-center justify-between">
                  <p className="text-xs font-semibold text-zinc-900">
                    Assign Employee
                  </p>

                  <button
                    type="button"
                    onClick={() =>
                      setShowEmployeeSelector(false)
                    }
                    className="text-zinc-400 transition hover:text-zinc-900"
                    aria-label="Close employee selector"
                  >
                    <X size={14} />
                  </button>
                </div>

                <div className="space-y-1">
                  {employees.map((item) => {
                    const isAssigned =
                      item.id === device?.employee_id;

                    return (
                      <button
                        key={item.id}
                        type="button"
                        disabled={savingAssignment}
                        onClick={() => assignEmployee(item.id)}
                        className="flex w-full items-center justify-between rounded-lg px-3 py-2 text-left transition hover:bg-white disabled:cursor-not-allowed disabled:opacity-50"
                      >
                        <div>
                          <p className="text-xs font-medium text-zinc-900">
                            {item.full_name}
                          </p>
                          <p className="mt-0.5 text-[11px] text-zinc-500">
                            {item.job_title || "No job title"}
                            {item.department
                              ? ` · ${item.department}`
                              : ""}
                          </p>
                        </div>

                        {isAssigned && (
                          <Check
                            size={14}
                            className="text-emerald-600"
                          />
                        )}
                      </button>
                    );
                  })}

                  <button
                    type="button"
                    disabled={savingAssignment}
                    onClick={() => assignEmployee(null)}
                    className="w-full rounded-lg px-3 py-2 text-left text-xs font-medium text-zinc-500 transition hover:bg-white hover:text-zinc-900 disabled:opacity-50"
                  >
                    Unassign device
                  </button>
                </div>
              </div>
            )}
          </div>
        </div>

        <div>
          <div className="flex items-center gap-2 text-xs text-zinc-400">
            <MapPin size={14} />
            Location
          </div>

          <div className="mt-2">
            <p className="text-sm font-semibold text-zinc-900">
              {device?.location_name ?? "Not assigned"}
            </p>

            {device?.location_address && (
              <p className="mt-1 text-xs leading-5 text-zinc-500">
                {device.location_address}
              </p>
            )}

            {device?.latitude !== null &&
              device?.latitude !== undefined &&
              device?.longitude !== null &&
              device?.longitude !== undefined && (
                <p className="mt-2 text-xs text-zinc-500">
                  GPS: {device.latitude.toFixed(6)},{" "}
                  {device.longitude.toFixed(6)}
                </p>
              )}

            {device?.location_updated_at && (
              <p className="mt-1 text-[11px] text-zinc-400">
                Updated{" "}
                {formatLastSeen(device.location_updated_at)}
              </p>
            )}


            <div className="mt-4 rounded-xl border border-zinc-200 bg-white p-4">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <MapPin size={14} className="text-zinc-500" />
                  <p className="text-xs font-semibold text-zinc-900">
                    Live GPS
                  </p>
                </div>

                <span
                  className={`rounded-full px-2 py-1 text-[10px] font-medium ${
                    device?.gps_enabled
                      ? "bg-emerald-50 text-emerald-700"
                      : "bg-zinc-100 text-zinc-500"
                  }`}
                >
                  {device?.gps_enabled ? "Enabled" : "Disabled"}
                </span>
              </div>

              {device?.gps_enabled &&
              device.gps_latitude !== null &&
              device.gps_latitude !== undefined &&
              device.gps_longitude !== null &&
              device.gps_longitude !== undefined ? (
                <>
                  <p className="mt-3 text-sm font-medium text-zinc-900">
                    {device.gps_latitude.toFixed(6)},{" "}
                    {device.gps_longitude.toFixed(6)}
                  </p>

                  {device.gps_updated_at && (
                    <p className="mt-1 text-[11px] text-zinc-400">
                      Updated {formatLastSeen(device.gps_updated_at)}
                    </p>
                  )}

                  <a
                    href={`https://www.google.com/maps?q=${device.gps_latitude},${device.gps_longitude}`}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="mt-3 inline-flex items-center rounded-lg border border-zinc-200 px-3 py-2 text-xs font-medium text-zinc-700 transition hover:bg-zinc-50 hover:text-zinc-950"
                  >
                    View on Map
                  </a>
                </>
              ) : (
                <p className="mt-3 text-xs text-zinc-500">
                  {device?.gps_enabled
                    ? "Waiting for a GPS fix..."
                    : "GPS collection is disabled."}
                </p>
              )}

              <div className="mt-4 border-t border-zinc-100 pt-4">
                <button
                  type="button"
                  onClick={toggleGps}
                  disabled={savingGps || !device}
                  className="inline-flex items-center rounded-lg border border-zinc-200 px-3 py-2 text-xs font-medium text-zinc-700 transition hover:bg-zinc-50 hover:text-zinc-950 disabled:cursor-not-allowed disabled:opacity-50"
                >
                  {savingGps
                    ? "Updating..."
                    : device?.gps_enabled
                      ? "Disable GPS Collection"
                      : "Enable GPS Collection"}
                </button>

                {gpsError && (
                  <p className="mt-2 text-[11px] text-red-600">
                    {gpsError}
                  </p>
                )}
              </div>
            </div>

            <button
              type="button"
              onClick={openLocationEditor}
              className="mt-3 text-xs font-medium text-zinc-700 underline underline-offset-4 transition hover:text-zinc-950"
            >
              Edit Location
            </button>

            {showLocationEditor && (
              <div className="mt-4 rounded-xl border border-zinc-200 bg-zinc-50 p-4">
                <div className="mb-4 flex items-center justify-between">
                  <div>
                    <p className="text-xs font-semibold text-zinc-900">
                      Edit Location
                    </p>
                    <p className="mt-1 text-[11px] text-zinc-500">
                      Set the device's assigned office location.
                    </p>
                  </div>

                  <button
                    type="button"
                    onClick={() => setShowLocationEditor(false)}
                    className="text-zinc-400 transition hover:text-zinc-900"
                    aria-label="Close location editor"
                  >
                    <X size={14} />
                  </button>
                </div>

                <div className="space-y-3">
                  <div>
                    <label className="text-[11px] font-medium text-zinc-600">
                      Location name
                    </label>
                    <input
                      type="text"
                      value={locationName}
                      onChange={(event) =>
                        setLocationName(event.target.value)
                      }
                      placeholder="Lagos HQ"
                      className="mt-1 w-full rounded-lg border border-zinc-200 bg-white px-3 py-2 text-xs text-zinc-900 outline-none transition placeholder:text-zinc-400 focus:border-zinc-400"
                    />
                  </div>

                  <div>
                    <label className="text-[11px] font-medium text-zinc-600">
                      Address
                    </label>
                    <textarea
                      value={locationAddress}
                      onChange={(event) =>
                        setLocationAddress(event.target.value)
                      }
                      placeholder="123 Example Street, Ikeja, Lagos"
                      rows={2}
                      className="mt-1 w-full resize-none rounded-lg border border-zinc-200 bg-white px-3 py-2 text-xs text-zinc-900 outline-none transition placeholder:text-zinc-400 focus:border-zinc-400"
                    />
                  </div>

                  <div className="grid grid-cols-2 gap-3">
                    <div>
                      <label className="text-[11px] font-medium text-zinc-600">
                        Latitude
                      </label>
                      <input
                        type="number"
                        step="any"
                        value={latitude}
                        onChange={(event) =>
                          setLatitude(event.target.value)
                        }
                        placeholder="6.5244"
                        className="mt-1 w-full rounded-lg border border-zinc-200 bg-white px-3 py-2 text-xs text-zinc-900 outline-none transition placeholder:text-zinc-400 focus:border-zinc-400"
                      />
                    </div>

                    <div>
                      <label className="text-[11px] font-medium text-zinc-600">
                        Longitude
                      </label>
                      <input
                        type="number"
                        step="any"
                        value={longitude}
                        onChange={(event) =>
                          setLongitude(event.target.value)
                        }
                        placeholder="3.3792"
                        className="mt-1 w-full rounded-lg border border-zinc-200 bg-white px-3 py-2 text-xs text-zinc-900 outline-none transition placeholder:text-zinc-400 focus:border-zinc-400"
                      />
                    </div>
                  </div>

                  {locationError && (
                    <p className="text-xs text-red-600">
                      {locationError}
                    </p>
                  )}

                  <div className="flex justify-end gap-2 pt-1">
                    <button
                      type="button"
                      onClick={() => setShowLocationEditor(false)}
                      disabled={savingLocation}
                      className="rounded-lg border border-zinc-200 bg-white px-3 py-2 text-xs font-medium text-zinc-700 transition hover:bg-zinc-100 disabled:opacity-50"
                    >
                      Cancel
                    </button>

                    <button
                      type="button"
                      onClick={saveLocation}
                      disabled={savingLocation}
                      className="rounded-lg bg-zinc-950 px-3 py-2 text-xs font-medium text-white transition hover:bg-zinc-800 disabled:cursor-not-allowed disabled:opacity-50"
                    >
                      {savingLocation
                        ? "Saving..."
                        : "Save Location"}
                    </button>
                  </div>
                </div>
              </div>
            )}
          </div>
        </div>

        <div>
          <p className="text-xs text-zinc-400">Hostname</p>
          <p className="mt-2 text-sm font-medium text-zinc-900">
            {device?.hostname ?? "Loading..."}
          </p>
        </div>

        <div>
          <p className="text-xs text-zinc-400">IP Address</p>
          <p className="mt-2 text-sm font-medium text-zinc-900">
            {device?.ip_address ?? "—"}
          </p>
        </div>

        <div>
          <p className="text-xs text-zinc-400">Last Heartbeat</p>
          <p className="mt-2 text-sm font-medium text-zinc-900">
            {formatLastSeen(device?.last_seen ?? null)}
          </p>
        </div>
      </div>
    </div>
  );
}
