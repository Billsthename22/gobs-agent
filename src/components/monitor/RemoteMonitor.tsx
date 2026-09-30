"use client";

import { useEffect, useRef, useState } from "react";
import {
  Monitor,
  Wifi,
  WifiOff,
  Cpu,
  MemoryStick,
  HardDrive,
  Activity,
  Loader2,
  Square,
} from "lucide-react";
import { API_URL, WS_URL } from "@/lib/api";

type Device = {
  id: number;
  device_name: string;
  hostname: string;
  device_code?: string;
  status: string;
  cpu_usage?: number;
  memory_usage?: number;
  storage_usage?: number;
  network_speed_mbps?: number;
  last_seen?: string;
};

type MonitoringSession = {
  id: number;
  device_id: number;
  status: string;
  is_authorized: boolean;
  started_at?: string;
  ended_at?: string | null;
};

type MonitoringData = {
  cpu_usage: number;
  memory_usage: number;
  storage_usage: number;
  network_speed_mbps: number;
};

type RemoteMonitorProps = {
  device?: Device | null;
  deviceId?: number | string;
};

// ---------------------------------------------------------------------------
// Screen stream wire format (v2)
//
// Every binary message from the device agent has a 6-byte header:
//   byte 0: kind  (1 = stream config, 2 = encoded video frame)
//   byte 1: flags (bit 0 = keyframe)
//   bytes 2..5: presentation timestamp in ms (big-endian)
//
// Config payload is UTF-8 JSON: { w, h, fps, codec, description }
// where `description` is the base64 avcC decoder config.
// Video payload is one frame of AVCC (4-byte length-prefixed) H.264 NAL units.
// ---------------------------------------------------------------------------
const STREAM_KIND_CONFIG = 1;
const STREAM_KIND_VIDEO = 2;
const STREAM_FLAG_KEYFRAME = 0x01;
const MONITOR_RECONNECT_MAX_ATTEMPTS = 5;
const MONITOR_RECONNECT_MAX_DELAY_MS = 10_000;

function base64ToBytes(value: string): Uint8Array {
  const binary = atob(value);
  const bytes = new Uint8Array(binary.length);

  for (let i = 0; i < binary.length; i++) {
    bytes[i] = binary.charCodeAt(i);
  }

  return bytes;
}

export default function RemoteMonitor({
  device,
  deviceId,
}: RemoteMonitorProps) {
  const [resolvedDevice, setResolvedDevice] = useState<Device | null>(
    device ?? null
  );
  const [resolvingDevice, setResolvingDevice] = useState(!device);

  const [session, setSession] = useState<MonitoringSession | null>(null);

  const [isStarting, setIsStarting] = useState(false);
  const [isStopping, setIsStopping] = useState(false);
  const [connected, setConnected] = useState(false);

  // True once the decoder has rendered at least one frame.
  const [isStreaming, setIsStreaming] = useState(false);

  // Populated when the current browser lacks WebCodecs or the stream is
  // undecodable, so we can show a human explanation instead of a blank box.
  const [videoError, setVideoError] = useState<string | null>(null);

  const [monitoringData, setMonitoringData] = useState<MonitoringData>({
    cpu_usage: 0,
    memory_usage: 0,
    storage_usage: 0,
    network_speed_mbps: 0,
  });

  const websocketRef = useRef<WebSocket | null>(null);
  const reconnectTimerRef = useRef<ReturnType<typeof setTimeout> | null>(
    null
  );
  const reconnectAttemptRef = useRef(0);
  const [monitorConnectionEpoch, setMonitorConnectionEpoch] = useState(0);
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const decoderRef = useRef<VideoDecoder | null>(null);

  // Identity of the config the current decoder was built from; lets periodic
  // repeat configs be ignored without tearing the decoder down.
  const configKeyRef = useRef<string | null>(null);

  // isConfigSupported() is asynchronous. Track an in-flight config as well,
  // otherwise the agent's periodic config packets can create overlapping
  // VideoDecoder instances before the first promise settles.
  const pendingConfigKeyRef = useRef<string | null>(null);

  // Set whenever the decoder is (re)configured: WebCodecs refuses non-key
  // chunks after configure(), so delta frames are dropped until the next key
  // frame (~2 s cadence) arrives to re-arm the decoder.
  const needsKeyFrameRef = useRef(true);

  // Written by the decoder `output` callback to avoid a re-render per frame;
  // surfaced to the UI via `isStreaming` state.
  const isStreamingRef = useRef(false);

  const videoSupported =
    typeof window !== "undefined" &&
    typeof window.VideoDecoder !== "undefined" &&
    typeof VideoDecoder.isConfigSupported === "function";

  // -------------------------------------------------------------------------
  // Decoder lifecycle
  // -------------------------------------------------------------------------
  function closeDecoder() {
    const decoder = decoderRef.current;

    if (decoder && decoder.state !== "closed") {
      try {
        decoder.close();
      } catch (error) {
        console.error("Failed to close VideoDecoder:", error);
      }
    }

    decoderRef.current = null;

    configKeyRef.current = null;
    pendingConfigKeyRef.current = null;
    needsKeyFrameRef.current = true;
  }

  // -------------------------------------------------------------------------
  // Resolve device.
  //
  // The monitor page can pass "PC-001" while the backend needs device_id = 1.
  // -------------------------------------------------------------------------
  useEffect(() => {
    if (device) {
      setResolvedDevice(device);
      setResolvingDevice(false);
      return;
    }

    if (deviceId === undefined || deviceId === null || deviceId === "") {
      setResolvingDevice(false);
      return;
    }

    let cancelled = false;

    const resolveDevice = async () => {
      setResolvingDevice(true);

      try {
        /*
         * If the supplied ID is numeric, try the direct device endpoint first.
         */
        if (
          typeof deviceId === "number" ||
          /^\d+$/.test(String(deviceId))
        ) {
          const numericId = Number(deviceId);

          const directResponse = await fetch(
            `${API_URL}/devices/${numericId}`
          );

          if (directResponse.ok) {
            const data = await directResponse.json();

            if (!cancelled) {
              console.log(
                "Resolved device:",
                deviceId,
                "→",
                data.id,
                data.device_name,
                data.hostname
              );

              setResolvedDevice(data);
              setResolvingDevice(false);
            }

            return;
          }
        }

        /*
         * For values such as PC-001, resolve against the dashboard device
         * health endpoint.
         */
        const healthResponse = await fetch(
          `${API_URL}/dashboard/device-health`
        );

        if (!healthResponse.ok) {
          throw new Error(
            `Device health request failed: ${healthResponse.status}`
          );
        }

        const devices: Device[] = await healthResponse.json();

        const requestedId = String(deviceId).trim().toLowerCase();

        const matches = (item: Device) => {
          const hostname = String(item.hostname ?? "").trim().toLowerCase();

          return (
            String(item.id).toLowerCase() === requestedId ||
            String(item.device_code ?? "").trim().toLowerCase() ===
              requestedId ||
            hostname === requestedId ||
            /*
             * The backend has no device_code column; codes like "PC-001" are
             * embedded in the hostname ("GBOS-PC-001").
             */
            (requestedId.length > 1 &&
              hostname.endsWith(`-${requestedId}`)) ||
            String(item.device_name ?? "").trim().toLowerCase() ===
              requestedId
          );
        };

        const foundDevice = devices.find(matches);

        if (foundDevice) {
          if (!cancelled) {
            console.log(
              "Resolved device:",
              deviceId,
              "→",
              foundDevice.id,
              foundDevice.device_name,
              foundDevice.hostname
            );

            setResolvedDevice(foundDevice);
            setResolvingDevice(false);
          }

          return;
        }

        /*
         * Fallback to the regular devices endpoint.
         */
        const devicesResponse = await fetch(`${API_URL}/devices`);

        if (devicesResponse.ok) {
          const allDevices: Device[] = await devicesResponse.json();

          const fallbackDevice = allDevices.find(matches);

          /*
           * Always finish resolving: set the device (or null) and clear the
           * loading state. Otherwise a reference that matches no device leaves
           * the page stuck on "Loading device..." forever.
           */
          if (!cancelled) {
            if (fallbackDevice) {
              console.log(
                "Resolved device:",
                deviceId,
                "→",
                fallbackDevice.id,
                fallbackDevice.device_name,
                fallbackDevice.hostname
              );

              setResolvedDevice(fallbackDevice);
            } else {
              setResolvedDevice(null);
            }

            setResolvingDevice(false);
          }

          return;
        }

        throw new Error(`Could not resolve device "${deviceId}"`);
      } catch (error) {
        console.error("Failed to resolve device:", error);

        if (!cancelled) {
          setResolvedDevice(null);
          setResolvingDevice(false);
        }
      }
    };

    resolveDevice();

    return () => {
      cancelled = true;
    };
  }, [device, deviceId]);

  // -------------------------------------------------------------------------
  // Connect to the monitoring WebSocket and decode the H.264 screen stream.
  // -------------------------------------------------------------------------
  useEffect(() => {
    if (!session || session.status !== "active") {
      return;
    }

    let disposed = false;

    console.log("Opening monitoring WebSocket for session:", session.id);

    const websocket = new WebSocket(
      `${WS_URL}/ws/monitoring/${session.id}`
    );

    websocket.binaryType = "arraybuffer";
    websocketRef.current = websocket;

    websocket.onopen = () => {
      console.log("Monitoring WebSocket connected ✅");
      reconnectAttemptRef.current = 0;
      setConnected(true);
    };

    // ---- Incoming binary stream -------------------------------------------
    const handleStreamBytes = (bytes: Uint8Array) => {
      if (bytes.length < 6) {
        return;
      }

      const kind = bytes[0];
      const flags = bytes[1];
      const timestampMs =
        (bytes[2] << 24) | (bytes[3] << 16) | (bytes[4] << 8) | bytes[5];
      const payload = bytes.subarray(6);

      if (kind === STREAM_KIND_CONFIG) {
        handleConfig(payload);
        return;
      }

      if (kind === STREAM_KIND_VIDEO) {
        const keyframe = (flags & STREAM_FLAG_KEYFRAME) !== 0;
        handleVideo(keyframe, timestampMs, payload);
      }
    };

    const handleConfig = (payload: Uint8Array) => {
      try {
        const config = JSON.parse(new TextDecoder().decode(payload)) as {
          w: number;
          h: number;
          fps: number;
          codec: string;
          description: string;
        };

        console.log(
          "Screen stream config received:",
          config.codec,
          `${config.w}x${config.h}`
        );

        // A new session/stream always starts with a fresh config, so tear
        // down any previous decoder before configuring the new one. The agent
        // re-sends this same config periodically (~2 s) so late-joining
        // viewers can sync in; identical repeats are no-ops — recreating the
        // decoder each time would just stutter playback.
        const configKey = `${config.codec}|${config.w}x${config.h}|${config.description}`;

        const decoder = decoderRef.current;

        if (
          decoder &&
          decoder.state === "configured" &&
          configKeyRef.current === configKey
        ) {
          return;
        }

        if (pendingConfigKeyRef.current === configKey) {
          return;
        }

        closeDecoder();
        setVideoError(null);

        if (!videoSupported) {
          return;
        }

        const description = base64ToBytes(config.description);

        pendingConfigKeyRef.current = configKey;

        VideoDecoder.isConfigSupported({
          codec: config.codec,
          description,
          codedWidth: config.w,
          codedHeight: config.h,
        }).then((support) => {
          // A newer stream config superseded this asynchronous probe.
          if (pendingConfigKeyRef.current !== configKey) {
            return;
          }

          if (!support.supported) {
            console.error(
              "Decoder config not supported:",
              config.codec,
              support
            );

            setVideoError(
              `This browser cannot decode ${config.codec}.`
            );

            pendingConfigKeyRef.current = null;

            return;
          }

          const decoder = new VideoDecoder({
            output: (frame) => {
              const canvas = canvasRef.current;

              if (!canvas) {
                frame.close();
                return;
              }

              if (
                canvas.width !== frame.displayWidth ||
                canvas.height !== frame.displayHeight
              ) {
                canvas.width = frame.displayWidth;
                canvas.height = frame.displayHeight;
              }

              const context = canvas.getContext("2d");

              if (context) {
                context.drawImage(frame, 0, 0);
              }

              frame.close();

              if (!isStreamingRef.current) {
                isStreamingRef.current = true;
                setIsStreaming(true);
              }
            },
            error: (error) => {
              console.error("VideoDecoder error:", error);

              // Hard errors reset the codec; the next valid chunk must be a
              // key frame, so drop deltas until one arrives.
              needsKeyFrameRef.current = true;

              // A failed decoder is closed by WebCodecs. Clear its identity so
              // the next periodic config builds exactly one replacement.
              if (decoderRef.current === decoder) {
                decoderRef.current = null;
                configKeyRef.current = null;
              }
            },
          });

          decoder.configure({
            codec: config.codec,
            description,
            codedWidth: config.w,
            codedHeight: config.h,
            optimizeForLatency: true,
          });

          decoderRef.current = decoder;
          configKeyRef.current = configKey;
          pendingConfigKeyRef.current = null;
          needsKeyFrameRef.current = true;
          setVideoError(null);
        }).catch((error) => {
          if (pendingConfigKeyRef.current === configKey) {
            pendingConfigKeyRef.current = null;
            console.error("Failed to validate video decoder config:", error);
            setVideoError("Unable to initialize the screen decoder.");
          }
        });
      } catch (error) {
        console.error("Failed to handle stream config:", error);
        setVideoError("Received an invalid screen stream config.");
      }
    };

    const handleVideo = (
      keyframe: boolean,
      timestampMs: number,
      payload: Uint8Array
    ) => {
      const decoder = decoderRef.current;

      // Drop frames until a config has initialized the decoder.
      if (!decoder || decoder.state === "closed") {
        return;
      }

      // WebCodecs requires the first chunk after configure() to be a key
      // frame. The keyframe that followed a config can be dropped while the
      // decoder is still being (re)built asynchronously (isConfigSupported),
      // so gate on key frames here: drop deltas until one arrives (~2 s).
      if (needsKeyFrameRef.current && !keyframe) {
        return;
      }

      if (keyframe) {
        needsKeyFrameRef.current = false;
      }

      // Backpressure: if the decoder is far behind live, drop delta frames
      // rather than letting latency grow without bound. Dropping arbitrary
      // deltas mid-GOP leaves the decoder with reference gaps that cause
      // decode errors (and a decoder reset), so re-arm the key-frame gate and
      // let the next key frame resync instead.
      if (
        !keyframe &&
        decoder.decodeQueueSize > 12
      ) {
        needsKeyFrameRef.current = true;
        return;
      }

      try {
        const chunk = new EncodedVideoChunk({
          type: keyframe ? "key" : "delta",
          timestamp: timestampMs * 1000, // milliseconds -> microseconds
          data: payload,
        });

        decoder.decode(chunk);
      } catch (error) {
        console.error("Failed to decode video chunk:", error);

        // A decode failure resets the codec internally, after which the next
        // chunk must be a key frame — re-arm the gate so the cascade stops
        // rather than one error breeding one per frame.
        needsKeyFrameRef.current = true;
      }
    };

    websocket.onmessage = (event) => {
      if (event.data instanceof ArrayBuffer) {
        handleStreamBytes(new Uint8Array(event.data));
        return;
      }

      if (event.data instanceof Blob) {
        event.data.arrayBuffer().then((buffer) => {
          handleStreamBytes(new Uint8Array(buffer));
        });
        return;
      }

      if (typeof event.data === "string") {
        try {
          const message = JSON.parse(event.data);

          if (message.type === "monitoring_connection") {
            console.log("Monitoring connection established:", message);
            setConnected(true);
            return;
          }

          if (message.type === "monitoring_data") {
            const data = message.data ?? {};

            setMonitoringData({
              cpu_usage: Number(data.cpu_usage ?? 0),
              memory_usage: Number(data.memory_usage ?? 0),
              storage_usage: Number(data.storage_usage ?? 0),
              network_speed_mbps: Number(data.network_speed_mbps ?? 0),
            });
          }
        } catch (error) {
          console.error("Failed to parse monitoring message:", error);
        }
      }
    };

    websocket.onerror = (error) => {
      console.error("Monitoring WebSocket error ❌", error);
      setConnected(false);
    };

    websocket.onclose = () => {
      console.log("Monitoring WebSocket closed 🔌");
      setConnected(false);

      if (disposed || reconnectAttemptRef.current >= MONITOR_RECONNECT_MAX_ATTEMPTS) {
        if (!disposed) {
          setVideoError(
            "Monitoring connection was lost. Stop and start monitoring to try again."
          );
        }
        return;
      }

      const attempt = reconnectAttemptRef.current + 1;
      const delay = Math.min(
        1_000 * 2 ** (attempt - 1),
        MONITOR_RECONNECT_MAX_DELAY_MS
      );

      reconnectAttemptRef.current = attempt;
      console.log(
        `Monitoring WebSocket reconnect ${attempt}/${MONITOR_RECONNECT_MAX_ATTEMPTS} in ${delay}ms`
      );

      reconnectTimerRef.current = setTimeout(() => {
        reconnectTimerRef.current = null;
        setMonitorConnectionEpoch((epoch) => epoch + 1);
      }, delay);
    };

    return () => {
      disposed = true;
      websocket.close();

      if (reconnectTimerRef.current) {
        clearTimeout(reconnectTimerRef.current);
        reconnectTimerRef.current = null;
      }

      if (websocketRef.current === websocket) {
        websocketRef.current = null;
      }
    };
  }, [session, monitorConnectionEpoch]);

  // -------------------------------------------------------------------------
  // Close the decoder on unmount.
  // -------------------------------------------------------------------------
  useEffect(() => {
    return () => {
      closeDecoder();
    };
  }, []);

  // -------------------------------------------------------------------------
  // START MONITORING
  // -------------------------------------------------------------------------
  const startMonitoring = async () => {
    if (!resolvedDevice) {
      console.error("Cannot start monitoring: no device");
      return;
    }

    console.log(
      "Starting monitoring for device:",
      resolvedDevice.id,
      resolvedDevice.device_name
    );

    setIsStarting(true);
    setVideoError(null);

    try {
      const response = await fetch(
        `${API_URL}/remote-monitoring/start`,
        {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
          },
          body: JSON.stringify({
            device_id: resolvedDevice.id,
          }),
        }
      );

      const data = await response.json();

      if (!response.ok) {
        throw new Error(data?.detail || "Failed to start monitoring");
      }

      console.log("Monitoring session started:", data);
      reconnectAttemptRef.current = 0;
      setSession(data);
    } catch (error) {
      console.error("Failed to start monitoring:", error);
      setVideoError(error instanceof Error ? error.message : String(error));
    } finally {
      setIsStarting(false);
    }
  };

  // -------------------------------------------------------------------------
  // STOP MONITORING
  // -------------------------------------------------------------------------
  const stopMonitoring = async () => {
    if (!session) {
      return;
    }

    setIsStopping(true);

    try {
      const response = await fetch(
        `${API_URL}/remote-monitoring/${session.id}/stop`,
        {
          method: "POST",
        }
      );

      const data = await response.json();

      if (!response.ok) {
        throw new Error(data?.detail || "Failed to stop monitoring");
      }

      console.log("Monitoring session stopped:", data);
    } catch (error) {
      console.error("Failed to stop monitoring:", error);
    } finally {
      setIsStopping(false);
    }

    websocketRef.current?.close();

    if (reconnectTimerRef.current) {
      clearTimeout(reconnectTimerRef.current);
      reconnectTimerRef.current = null;
    }

    reconnectAttemptRef.current = 0;
    closeDecoder();

    // Reset so the next session starts from "Waiting for video..." and a fresh
    // config can tear down any stale state.
    isStreamingRef.current = false;

    setSession(null);
    setConnected(false);
    setIsStreaming(false);
    setVideoError(null);
  };

  // -------------------------------------------------------------------------
  // Loading / empty states
  // -------------------------------------------------------------------------
  if (resolvingDevice) {
    return (
      <div className="flex min-h-[500px] items-center justify-center rounded-2xl border border-zinc-200 bg-white">
        <div className="text-center">
          <Loader2
            size={30}
            className="mx-auto mb-3 animate-spin text-zinc-400"
          />

          <p className="text-sm font-medium text-zinc-900">Loading device...</p>

          <p className="mt-1 text-xs text-zinc-500">
            Resolving the selected GBOS device.
          </p>
        </div>
      </div>
    );
  }

  if (!resolvedDevice) {
    return (
      <div className="flex min-h-[500px] items-center justify-center rounded-2xl border border-zinc-200 bg-white">
        <div className="text-center">
          <Monitor size={32} className="mx-auto mb-3 text-zinc-400" />

          <p className="text-sm font-medium text-zinc-900">Device not found</p>

          <p className="mt-1 text-xs text-zinc-500">
            Unable to resolve the selected device.
          </p>

          {deviceId && (
            <p className="mt-2 text-[11px] text-zinc-400">
              Device reference: {String(deviceId)}
            </p>
          )}
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-5">
      {/* HEADER */}
      <div className="flex items-center justify-between rounded-2xl border border-zinc-200 bg-white p-5">
        <div className="flex items-center gap-4">
          <div className="flex h-11 w-11 items-center justify-center rounded-xl bg-zinc-100">
            <Monitor size={20} className="text-zinc-700" />
          </div>

          <div>
            <h2 className="text-sm font-semibold text-zinc-950">
              {resolvedDevice.device_name}
            </h2>

            <p className="mt-0.5 text-xs text-zinc-500">
              {resolvedDevice.hostname}
            </p>
          </div>
        </div>

        <div className="flex items-center gap-3">
          <div
            className={`flex items-center gap-2 rounded-full px-3 py-1.5 text-xs font-medium ${
              connected
                ? "bg-emerald-50 text-emerald-700"
                : "bg-zinc-100 text-zinc-500"
            }`}
          >
            {connected ? (
              <>
                <Wifi size={13} />
                Connected
              </>
            ) : (
              <>
                <WifiOff size={13} />
                Disconnected
              </>
            )}
          </div>

          {!session ? (
            <button
              type="button"
              onClick={startMonitoring}
              disabled={isStarting}
              className="flex items-center gap-2 rounded-xl bg-zinc-950 px-4 py-2.5 text-xs font-medium text-white transition hover:bg-zinc-800 disabled:cursor-not-allowed disabled:opacity-50"
            >
              {isStarting ? (
                <>
                  <Loader2 size={14} className="animate-spin" />
                  Starting...
                </>
              ) : (
                <>
                  <Monitor size={14} />
                  Start Monitoring
                </>
              )}
            </button>
          ) : (
            <button
              type="button"
              onClick={stopMonitoring}
              disabled={isStopping}
              className="flex items-center gap-2 rounded-xl border border-red-200 bg-red-50 px-4 py-2.5 text-xs font-medium text-red-700 transition hover:bg-red-100 disabled:cursor-not-allowed disabled:opacity-50"
            >
              {isStopping ? (
                <>
                  <Loader2 size={14} className="animate-spin" />
                  Stopping...
                </>
              ) : (
                <>
                  <Square size={13} />
                  Stop Monitoring
                </>
              )}
            </button>
          )}
        </div>
      </div>

      {/* AUTHORIZATION NOTICE */}
      {session && (
        <div className="flex items-center justify-between rounded-xl border border-amber-200 bg-amber-50 px-4 py-3">
          <div>
            <p className="text-xs font-semibold text-amber-900">
              Authorized remote monitoring session
            </p>

            <p className="mt-0.5 text-[11px] text-amber-700">
              Screen sharing is active for this device.
            </p>
          </div>

          <span className="rounded-full bg-amber-100 px-2.5 py-1 text-[10px] font-semibold text-amber-800">
            Session #{session.id}
          </span>
        </div>
      )}

      {/* LIVE SCREEN */}
      <div className="overflow-hidden rounded-2xl border border-zinc-200 bg-white">
        <div className="flex items-center justify-between border-b border-zinc-200 px-5 py-4">
          <div>
            <h3 className="text-sm font-semibold text-zinc-950">
              Live Screen
            </h3>

            <p className="mt-0.5 text-xs text-zinc-500">
              Real-time H.264 video from the authorized device
            </p>
          </div>

          {session && (
            <div className="flex items-center gap-2 text-xs text-zinc-500">
              <span
                className={`h-2 w-2 rounded-full ${
                  isStreaming
                    ? "animate-pulse bg-emerald-500"
                    : "bg-zinc-300"
                }`}
              />

              {isStreaming ? "Streaming" : "Waiting for video..."}
            </div>
          )}
        </div>

        <div className="flex min-h-[500px] items-center justify-center bg-black p-4">
          {!videoSupported ? (
            <div className="flex max-w-sm flex-col items-center justify-center text-center">
              <Monitor size={40} className="mb-4 text-zinc-600" />

              <p className="text-sm font-medium text-zinc-300">
                Video playback is not supported in this browser
              </p>

              <p className="mt-1 text-xs text-zinc-500">
                Live screen monitoring requires Chrome, Edge, or Safari.
              </p>
            </div>
          ) : videoError && !isStreaming ? (
            <div className="flex max-w-sm flex-col items-center justify-center text-center">
              <Monitor size={40} className="mb-4 text-zinc-600" />

              <p className="text-sm font-medium text-zinc-300">
                Screen stream unavailable
              </p>

              <p className="mt-1 text-xs text-zinc-500">{videoError}</p>
            </div>
          ) : isStreaming ? (
            <canvas
              ref={canvasRef}
              className="h-auto max-h-[700px] w-auto max-w-full rounded"
            />
          ) : session ? (
            <div className="flex flex-col items-center justify-center text-center">
              {/* Keep the decode target mounted while waiting. The decoder's
                  first output is what flips isStreaming to true; conditionally
                  mounting this canvas only after that state change creates a
                  circular wait and drops every first frame. */}
              <canvas ref={canvasRef} className="hidden" />

              <Loader2 size={32} className="mb-4 animate-spin text-white" />

              <p className="text-sm font-medium text-white">
                Waiting for screen stream
              </p>

              <p className="mt-1 max-w-sm text-xs text-zinc-400">
                The device is connecting and starting the authorized H.264
                video stream.
              </p>
            </div>
          ) : (
            <div className="flex flex-col items-center justify-center text-center">
              <Monitor size={40} className="mb-4 text-zinc-600" />

              <p className="text-sm font-medium text-zinc-300">
                Screen monitoring is not active
              </p>

              <p className="mt-1 max-w-sm text-xs text-zinc-500">
                Start an authorized monitoring session to view the device
                screen.
              </p>
            </div>
          )}
        </div>
      </div>

      {/* TELEMETRY */}
      <div className="grid grid-cols-2 gap-4 lg:grid-cols-4">
        <TelemetryCard
          icon={Cpu}
          label="CPU Usage"
          value={`${monitoringData.cpu_usage.toFixed(1)}%`}
        />

        <TelemetryCard
          icon={MemoryStick}
          label="Memory Usage"
          value={`${monitoringData.memory_usage.toFixed(1)}%`}
        />

        <TelemetryCard
          icon={HardDrive}
          label="Storage Usage"
          value={`${monitoringData.storage_usage.toFixed(1)}%`}
        />

        <TelemetryCard
          icon={Activity}
          label="Network"
          value={`${monitoringData.network_speed_mbps.toFixed(2)} Mbps`}
        />
      </div>
    </div>
  );
}

function TelemetryCard({
  icon: Icon,
  label,
  value,
}: {
  icon: typeof Cpu;
  label: string;
  value: string;
}) {
  return (
    <div className="rounded-2xl border border-zinc-200 bg-white p-4">
      <div className="mb-3 flex h-9 w-9 items-center justify-center rounded-xl bg-zinc-100">
        <Icon size={17} className="text-zinc-600" />
      </div>

      <p className="text-xs text-zinc-500">{label}</p>

      <p className="mt-1 text-lg font-semibold text-zinc-950">{value}</p>
    </div>
  );
}
