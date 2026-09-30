# GBOS — Gbaja-Partners Device Management Platform

> **GBOS** is an authorized company device management and monitoring platform built for managing company-owned or explicitly authorized computers from a centralized administrative dashboard.

GBOS provides administrators with visibility into device health, connectivity, employee assignments, system activity, authorized application activity, and authorized remote monitoring.

The platform consists of:

* A **Next.js administrative frontend**
* A **FastAPI backend**
* A **PostgreSQL database**
* A **Rust device agent**
* WebSocket communication between devices and the backend
* Authorized remote monitoring infrastructure
* Device and employee management
* Activity and audit tracking
* Automatic device-health alerts

---

# 1. Project Purpose

The purpose of GBOS is to provide an organization with a centralized platform for managing its company-owned devices.

The system is designed around the following principles:

1. **Authorized device management**
2. **Transparent monitoring**
3. **Centralized administration**
4. **Device health visibility**
5. **Employee/device association**
6. **Auditable administrative actions**
7. **Authorized remote support**
8. **Reliable real-time communication**

GBOS is intended for devices that belong to, or are explicitly authorized by, the organization operating the platform.

The monitoring functionality is designed around legitimate administrative information such as:

* Device online/offline status
* CPU usage
* Memory usage
* Storage usage
* Network activity
* Login/logout activity
* Active/idle state
* Authorized application activity
* Device information
* Device health
* Heartbeats
* Authorized remote screen monitoring

Remote monitoring is a separate capability from ordinary telemetry and is intended to be explicitly authorized and auditable.

---

# 2. High-Level Architecture

```text
                         GBOS ADMIN DASHBOARD
                              Next.js
                                │
                                │ HTTP
                                ▼
                         ┌───────────────┐
                         │   FastAPI     │
                         │    Backend    │
                         └───────┬───────┘
                                 │
                ┌────────────────┼────────────────┐
                │                │                │
                ▼                ▼                ▼
           PostgreSQL       WebSocket        REST APIs
             Database       Connection
                │                │
                │                ▼
                │          Rust Device Agent
                │                │
                │                ▼
                │          Company Computer
                │
                ▼
        Activity / Alerts /
        Device / Employee Data
```

The system has three primary application layers.

## Frontend

Location:

```text
/Users/gbaja/Desktop/gbos/gbos
```

Technology:

* Next.js 16.3.4
* React
* TypeScript
* Tailwind CSS v4
* shadcn/ui / Base UI
* Nova preset
* Lucide icons
* Recharts

The frontend is the administrative interface.

---

## Backend

Location:

```text
/Users/gbaja/Desktop/gbos/backend
```

Technology:

* Python
* FastAPI
* SQLAlchemy
* PostgreSQL
* Pydantic Settings
* Python-JOSE
* bcrypt
* WebSockets

The backend handles:

* API requests
* Authentication infrastructure
* Device management
* Employee management
* Activity records
* Alerts
* Device telemetry
* WebSocket communication
* Remote monitoring sessions
* Device commands

---

## Device Agent

Location:

```text
/Users/gbaja/Desktop/gbos/agent
```

Technology:

* Rust
* Tokio
* tokio-tungstenite
* sysinfo
* screenshots
* OpenH264
* serde
* serde_json
* base64
* futures-util

The Rust agent runs on the managed computer.

It communicates with GBOS through a WebSocket connection.

---

# 3. Project Directory Structure

The overall project is:

```text
/Users/gbaja/Desktop/gbos
```

The major directories are:

```text
gbos/
├── backend/
│   ├── app/
│   │   ├── core/
│   │   ├── models/
│   │   ├── routes/
│   │   ├── schemas/
│   │   ├── websocket/
│   │   └── main.py
│   └── venv/
│
├── gbos/
│   ├── src/
│   │   ├── app/
│   │   ├── components/
│   │   └── ...
│   ├── package.json
│   └── ...
│
└── agent/
    ├── src/
    ├── Cargo.toml
    └── ...
```

---

# 4. Development Environment

## Frontend

Frontend root:

```bash
cd ~/Desktop/gbos/gbos
```

Start development server:

```bash
npm run dev
```

Frontend:

```text
http://localhost:3000
```

---

## Backend

Backend root:

```bash
cd ~/Desktop/gbos/backend
```

Activate the backend virtual environment:

```bash
source venv/bin/activate
```

Start FastAPI:

```bash
uvicorn app.main:app --reload
```

Backend:

```text
http://127.0.0.1:8000
```

---

# 5. Python Environment Note

There are two virtual environments in the overall project.

The backend environment is:

```text
/Users/gbaja/Desktop/gbos/backend/venv
```

This is the environment that should be used for the FastAPI backend.

It contains packages including:

```text
pydantic-settings 2.11.0
```

The outer environment:

```text
/Users/gbaja/Desktop/gbos/.venv
```

is a separate environment and previously lacked some backend dependencies.

When working on the backend, use:

```bash
cd ~/Desktop/gbos/backend
source venv/bin/activate
```

---

# 6. PostgreSQL Database

The application database is:

```text
gbos
```

The backend `.env` contains:

```text
DATABASE_URL=postgresql://gbaja@localhost:5432/gbos
```

PostgreSQL is responsible for storing:

* Devices
* Employees
* Activities
* Alerts
* Device telemetry
* Remote monitoring sessions
* Other application state

---

# 7. Database Device Model

The current `Device` model is:

```python
class Device(Base):
    __tablename__ = "devices"

    id = Column(Integer, primary_key=True, index=True)
    device_name = Column(String(255), nullable=False)
    hostname = Column(String(255), unique=True, index=True, nullable=False)
    operating_system = Column(String(100), nullable=False)
    ip_address = Column(String(45), nullable=True)

    employee_id = Column(
        Integer,
        ForeignKey("employees.id", ondelete="SET NULL"),
        nullable=True,
        index=True,
    )

    status = Column(String(50), default="offline", nullable=False)

    cpu_usage = Column(Float, default=0, nullable=False)
    memory_usage = Column(Float, default=0, nullable=False)
    storage_usage = Column(Float, default=0, nullable=False)

    last_seen = Column(DateTime(timezone=True), nullable=True)

    is_active = Column(Boolean, default=True, nullable=False)

    created_at = Column(
        DateTime(timezone=True),
        server_default=func.now(),
        nullable=False,
    )
```

Important:

The current database does **not** contain:

* `device_code`
* Device model field
* Network speed column on the Device table

Network speed is instead handled through telemetry.

---

# 8. Current Device

The primary test device currently configured is:

```text
ID:             1
Device Name:    Office PC 001
Hostname:       GBOS-PC-001
Operating OS:   Windows 11
IP Address:     192.168.1.101
Employee ID:    1
```

The assigned employee is:

```text
John Doe
IT
Systems Administrator
```

The device has successfully reported live metrics such as:

```text
CPU
Memory
Storage
Network
Online status
Last seen
```

---

# 9. Device Status

The system uses device connectivity to determine device status.

The general status model is:

```text
ONLINE
STALE
OFFLINE
```

The agent sends heartbeats approximately every:

```text
5 seconds
```

Previously established conceptual thresholds are approximately:

```text
Online:   ~5 seconds
Stale:    ~20 seconds
Offline:  ~45 seconds
```

The backend updates:

```text
device.status
device.last_seen
```

when heartbeats or telemetry are received.

---

# 10. Device Telemetry

The Rust agent periodically collects:

```text
CPU usage
Memory usage
Storage usage
Network speed
```

Telemetry is sent approximately every:

```text
5 seconds
```

Example telemetry:

```json
{
  "type": "telemetry",
  "cpu_usage": 32.4,
  "memory_usage": 81.5,
  "storage_usage": 17.3,
  "network_speed_mbps": 4.2
}
```

The backend stores telemetry in the device telemetry table and updates the current device metrics.

---

# 11. Telemetry Processing

The backend receives telemetry through the device WebSocket.

The backend:

1. Parses CPU usage.
2. Parses memory usage.
3. Parses storage usage.
4. Parses network speed.
5. Creates a telemetry record.
6. Updates the Device record.
7. Updates `last_seen`.
8. Marks the device online.
9. Evaluates metric alerts.
10. Commits the transaction.
11. Sends a telemetry acknowledgement.
12. Broadcasts telemetry to active authorized monitoring sessions.

---

# 12. Heartbeats

The Rust agent sends heartbeat messages.

The backend responds with:

```json
{
  "type": "heartbeat_ack",
  "device_id": 1,
  "last_seen": "..."
}
```

Heartbeats intentionally do **not** create activity records.

This prevents the Activity table from being flooded with thousands of heartbeat events.

---

# 13. Activity Tracking

GBOS maintains an activity/audit system.

Current Activity model:

```python
class Activity(Base):
    __tablename__ = "activities"

    id = Column(Integer, primary_key=True, index=True)

    device_id = Column(
        Integer,
        ForeignKey("devices.id", ondelete="CASCADE"),
        nullable=False,
        index=True,
    )

    employee_id = Column(
        Integer,
        ForeignKey("employees.id", ondelete="SET NULL"),
        nullable=True,
        index=True,
    )

    activity_type = Column(String(100), nullable=False)

    description = Column(Text, nullable=False)

    timestamp = Column(
        DateTime(timezone=True),
        server_default=func.now(),
        nullable=False,
        index=True,
    )
```

---

# 14. Current Activity Types

GBOS currently records activity types including:

```text
DEVICE_CONNECTED
DEVICE_DISCONNECTED
LOGIN
LOGOUT
APPLICATION_ACTIVE
```

Future administrative commands will add command/audit activity types such as:

```text
REMOTE_RESTART
REMOTE_SHUTDOWN
```

or an equivalent standardized naming scheme.

---

# 15. Device Connection Activity

When an agent connects:

```text
DEVICE_CONNECTED
```

is recorded.

The device is marked:

```text
online
```

and:

```text
last_seen
```

is updated.

When the agent disconnects:

```text
DEVICE_DISCONNECTED
```

is recorded.

The device is marked:

```text
offline
```

---

# 16. User Session Tracking

The Rust agent detects login/logout events.

The backend supports:

```text
LOGIN
LOGOUT
```

activity records.

Example:

```text
gbaja logged into Office PC 001
```

The backend also protects against duplicate login events.

If the same login event has already been recorded, the backend returns:

```json
{
  "ignored": true,
  "reason": "duplicate_login"
}
```

This prevents duplicate login records when the agent reconnects or repeats a session event.

---

# 17. Application Activity

The Rust agent detects the current frontmost application on macOS using AppleScript/System Events.

The helper currently uses:

```text
osascript
System Events
frontmost application process
```

The agent periodically checks the active application.

It only sends an event when the frontmost application changes.

For example:

```text
Safari
Google Chrome
Code
Finder
```

The backend creates:

```text
APPLICATION_ACTIVE
```

records.

Example description:

```text
Safari became active on Office PC 001
```

Consecutive duplicate application events are ignored.

This avoids repeatedly storing:

```text
Chrome
Chrome
Chrome
Chrome
Chrome
```

every five seconds.

Instead, an application activity record is created when the active application changes.

---

# 18. Employee Management

Employee management has been completed.

The platform supports:

* Add employee
* View employee
* Edit employee
* Activate employee
* Deactivate employee
* Search employees
* Filter employees
* View employee details

The employee system currently supports fields including:

```text
Full name
Email
Department
Job title
Phone
Status
Active/inactive state
Created timestamp
```

---

# 19. Current Employee

The current test employee is:

```text
John Doe
```

Information:

```text
Department: IT
Job Title: Systems Administrator
Email: john.doe@gbos.com
Phone: +2348012345678
Status: active
```

Employee ID:

```text
1
```

This employee is currently associated with:

```text
Office PC 001
```

---

# 20. Employee API

The backend employee functionality supports:

```text
GET employees
GET employee by ID
POST employee
PATCH employee
DELETE employee
```

The frontend consumes these endpoints to populate the employee management interface.

---

# 21. Device Assignment

Device-to-employee assignment has been completed.

An administrator can:

* Assign a device to an employee
* Change the assigned employee
* Unassign a device

The backend validates the employee before assigning them to a device.

If an invalid employee ID is supplied, the assignment is rejected.

The database relationship is:

```text
employees
     │
     │ employee_id
     ▼
devices
```

The foreign key uses:

```text
ON DELETE SET NULL
```

meaning deleting an employee does not destroy the device.

Instead, the device becomes unassigned.

---

# 22. Device Details

The device detail page displays information such as:

```text
Device name
Device code/display identifier
Hostname
Operating system
IP address
Assigned employee
Device status
CPU
Memory
Storage
Last active
```

The frontend periodically refreshes device information.

The current device page successfully displays live device health information.

---

# 23. Device Table

The main device management interface provides:

* Device list
* Search
* Status filtering
* Employee information
* CPU
* Memory
* Storage
* Last active
* Device actions
* Device selection

The table supports a minimum width to preserve the desktop administration layout.

---

# 24. Device Selection

Bulk device selection has been implemented.

Each device row contains a checkbox.

The table header contains a:

```text
Select All
```

checkbox.

Selection state is stored in:

```tsx
selectedDeviceIds
```

The system supports:

```text
Select one device
Select multiple devices
Select all visible devices
Clear selection
```

The select-all functionality applies to the currently filtered/visible devices.

---

# 25. Bulk Device Actions

Bulk device activation/deactivation has been completed and tested.

When devices are selected, GBOS displays a bulk-action toolbar.

Example:

```text
2 devices selected

[Activate] [Deactivate]                 [Clear selection]
```

Bulk operations send PATCH requests to each selected device.

Activation:

```json
{
  "is_active": true
}
```

Deactivation:

```json
{
  "is_active": false
}
```

The requests are performed concurrently and the device table refreshes afterward.

The user has tested this functionality successfully.

---

# 26. Device Action Menu

Each device has a contextual action menu.

The menu currently contains:

```text
View device
View employee
Remote monitor
Activate/Deactivate device
```

The menu is rendered through a React portal into:

```text
document.body
```

This was done because the original menu could be clipped by the table/container.

The menu uses fixed positioning calculated from the action button's bounding rectangle.

Current behavior:

```text
More (...) button
        ↓
Portal
        ↓
Fixed-position menu
```

This prevents overflow/clipping issues.

---

# 27. Remote Monitoring

Authorized remote monitoring has been implemented.

The device monitor route is:

```text
/devices/[id]/monitor
```

The frontend includes a Live Monitor interface.

Remote monitoring is treated separately from normal device telemetry.

The system creates a remote monitoring session and connects the viewer to the monitoring WebSocket.

---

# 28. Remote Monitoring Authorization

Remote monitoring sessions are checked using:

```text
status == "active"
is_authorized == true
```

Only active authorized sessions receive monitoring data.

The backend also uses these checks when forwarding screen frames.

This is an important distinction:

```text
Device connected
        ≠
Remote monitoring active
```

A connected device does not automatically mean its screen is being streamed.

---

# 29. Monitoring WebSocket

The monitoring infrastructure includes:

```text
/ws/monitoring/{session_id}
```

The backend contains a:

```text
MonitoringConnectionManager
```

which manages authorized monitoring viewers.

Telemetry from a device is forwarded to active monitoring sessions.

The monitoring payload includes:

```text
CPU
Memory
Storage
Network speed
```

---

# 30. Remote Screen Streaming

The Rust agent can capture the device screen.

Screen capture was tested successfully.

The system discovered multiple displays during testing and successfully captured a display at approximately:

```text
3024 × 1964
```

The original screen streaming approach used PNG frames.

This was later migrated to:

```text
H.264
```

using OpenH264 in Rust.

---

# 31. H.264 Screen Streaming

The Rust agent uses OpenH264 to encode screen frames.

The browser consumes the H.264 stream using the browser's decoding capabilities.

During implementation there were issues involving:

```text
VideoDecoder
keyframes
codec configuration
EncodingError
```

These were resolved.

The user confirmed that remote screen sharing is now working.

The current implementation therefore supports:

```text
Device
  ↓
Screen capture
  ↓
H.264 encoding
  ↓
WebSocket
  ↓
Backend
  ↓
Authorized monitoring session
  ↓
Browser
  ↓
VideoDecoder
  ↓
Live screen
```

Remote screen streaming should not be changed unless a new problem appears.

---

# 32. Rust Device Agent

The Rust agent is responsible for communicating with the managed computer.

The agent connects to:

```text
ws://127.0.0.1:8000/ws/devices/1
```

The device ID is currently:

```text
1
```

The agent sends:

```text
LOGIN
Heartbeat
Telemetry
Application activity
Screen frames
```

as appropriate.

---

# 33. Rust Agent Timers

The agent currently uses approximately five-second intervals for:

```rust
let mut heartbeat = interval(Duration::from_secs(5));
let mut telemetry_timer = interval(Duration::from_secs(5));
let mut session_timer = interval(Duration::from_secs(5));
let mut application_timer = interval(Duration::from_secs(5));
```

The application timer does not necessarily send an event every five seconds.

It checks whether the frontmost application changed.

Only changes are sent.

---

# 34. Rust Application Detection

The current helper effectively performs:

```text
System Events
    ↓
Find frontmost application process
    ↓
Read application name
    ↓
Compare with previous application
    ↓
Send only if changed
```

This has already been tested successfully.

Recorded examples include:

```text
Safari
Google Chrome
Code
Finder
```

---

# 35. Device WebSocket

The backend WebSocket endpoint is responsible for maintaining the device's live connection.

Conceptually:

```text
/ws/devices/{device_id}
```

The connection supports:

```text
Text/JSON messages
Binary screen frames
```

Binary messages are treated as screen frames.

Text messages are parsed as JSON.

---

# 36. Device WebSocket Message Types

Current message types include:

```text
heartbeat
session
application_activity
telemetry
```

The backend also handles:

```text
binary screen frame
```

Remote command handling is the next major capability to be added.

---

# 37. Device WebSocket Flow

When a device connects:

```text
Agent
  ↓
WebSocket connection
  ↓
Backend identifies device
  ↓
Device marked online
  ↓
DEVICE_CONNECTED activity
  ↓
Connection acknowledgement
```

While connected:

```text
Heartbeat
Telemetry
Session events
Application activity
Screen frames
```

are processed.

When disconnected:

```text
Device marked offline
DEVICE_DISCONNECTED activity
DEVICE_OFFLINE alert
```

may be created.

---

# 38. Device Disconnect Handling

The backend catches:

```python
WebSocketDisconnect
```

and then:

1. Marks device offline.
2. Updates `last_seen`.
3. Creates `DEVICE_DISCONNECTED`.
4. Checks whether an unresolved offline alert already exists.
5. Creates an offline alert if necessary.
6. Commits the database transaction.
7. Removes the WebSocket from the device connection manager.
8. Closes the database session.

This prevents the system from incorrectly keeping a device online after a lost connection.

---

# 39. Automatic Alerts

GBOS evaluates device health against configured thresholds.

Current alert categories include:

```text
HIGH_CPU
HIGH_MEMORY
LOW_STORAGE
DEVICE_OFFLINE
```

Examples:

```text
High CPU usage
High memory usage
Low storage space
Device went offline
```

---

# 40. CPU Alerts

CPU usage is checked against:

```text
CPU_ALERT_THRESHOLD
```

When the threshold condition is active, an alert can be created.

The alert includes:

```text
Alert type
Severity
Title
Description
Device
Employee
Resolution state
```

---

# 41. Memory Alerts

Memory usage is checked against:

```text
MEMORY_ALERT_THRESHOLD
```

A warning can be generated when memory consumption becomes excessive.

Example description:

```text
Memory usage reached 81.5%.
```

---

# 42. Storage Alerts

Storage usage is checked against:

```text
STORAGE_ALERT_THRESHOLD
```

The system can generate a low-storage warning when the threshold is reached.

---

# 43. Offline Alerts

When a device disconnects, GBOS checks for an existing unresolved:

```text
DEVICE_OFFLINE
```

alert.

If one does not exist, it creates one.

This prevents duplicate unresolved offline alerts from accumulating.

---

# 44. Frontend Routes

The current Next.js application contains:

```text
/
 /_not-found
 /activity
 /alerts
 /analytics
 /devices
 /devices/[id]
 /devices/[id]/monitor
 /employees
 /employees/[id]
 /settings
```

---

# 45. Devices Page

The Devices page is the central device administration interface.

It currently supports:

```text
Search
Status filter
Device selection
Bulk activation
Bulk deactivation
Device actions
Device details
Employee assignment
Remote monitoring
```

---

# 46. Employee Page

The Employees page supports:

```text
Employee list
Search
Status filter
Add employee
Edit employee
Activate employee
Deactivate employee
Employee action menu
Employee detail navigation
```

The Add Employee modal has been implemented.

The Edit Employee modal has been implemented and tested.

---

# 47. Employee Detail Page

The employee detail page provides employee information and related management functionality.

The page has been visually verified.

---

# 48. Device Assignment UI

The device details interface fetches:

```text
Device
Employees
```

and allows administrators to select an employee.

Assignment uses:

```text
PATCH /devices/{device_id}
```

with:

```json
{
  "employee_id": 1
}
```

Unassignment uses:

```json
{
  "employee_id": null
}
```

---

# 49. Frontend Device Refreshing

The DeviceInfo component refreshes device and employee information periodically.

This allows the interface to reflect:

```text
Live status
Updated telemetry
Employee changes
```

without requiring constant manual refreshes.

---

# 50. Current Build Status

The frontend currently builds successfully.

Command:

```bash
cd ~/Desktop/gbos/gbos
npm run build
```

Successful output has included:

```text
✓ Compiled successfully
✓ Finished TypeScript
✓ Collecting page data
✓ Generating static pages
✓ Finalizing page optimization
```

All current routes compile successfully.

---

# 51. Rust Build Status

The Rust agent currently passes:

```bash
cargo check
```

The only noted warning was in:

```text
src/bin/screentest.rs
```

because of an unused:

```text
rgba
```

variable.

This is a warning and does not prevent the project from compiling.

---

# 52. Rust Toolchain

Current Rust environment:

```text
rustc 1.98.1
cargo 1.98.1
```

Target:

```text
aarch64-apple-darwin
```

The development machine is Apple Silicon.

---

# 53. Current Rust Dependencies

The agent uses dependencies including:

```text
tokio
tokio-tungstenite
futures-util
serde
serde_json
sysinfo
screenshots
openh264
base64
```

These support:

* Async execution
* WebSocket networking
* JSON serialization
* System information
* Screen capture
* H.264 encoding
* Binary/data processing

---

# 54. Testing Performed

The following areas have been tested during development.

## Backend

Verified:

```text
FastAPI startup
Database connection
Device API
Employee API
Device WebSocket
Heartbeat
Telemetry
Login
Logout
Application activity
Device connection
Device disconnection
Alerts
Remote monitoring
```

---

## Frontend

Verified:

```text
Next.js build
Devices page
Device detail page
Device monitor page
Employees page
Employee detail page
Add employee
Edit employee
Employee activation/deactivation
Device assignment
Device reassignment
Device unassignment
Device selection
Select all
Bulk activation
Bulk deactivation
Device action menu
```

---

## Agent

Verified:

```text
Rust compilation
WebSocket connection
Heartbeat
Telemetry
Login/session events
Application activity
Screen capture
H.264 encoding
Remote screen streaming
```

---

# 55. Important Design Decisions

## No heartbeat activity spam

Heartbeats update device health but do not create Activity records.

This keeps the Activity system useful.

---

## Application activity is change-based

The agent only reports the frontmost application when it changes.

This reduces unnecessary traffic and database records.

---

## Remote monitoring is explicitly separate

A device being online does not automatically authorize screen viewing.

Monitoring sessions must be:

```text
active
AND
authorized
```

---

## Device assignment uses a foreign key

Devices reference employees using:

```text
employee_id
```

This allows employee management and device management to remain connected without duplicating employee information.

---

## Device menus use a portal

The device action menu is rendered through:

```text
createPortal(...)
```

to avoid clipping caused by table/container overflow.

---

# 56. Current Product Status

The platform has progressed beyond the initial prototype stage.

Completed major capabilities:

```text
✓ Backend
✓ PostgreSQL database
✓ Device registration
✓ Device WebSocket
✓ Heartbeat
✓ Telemetry
✓ Device health
✓ Device status
✓ Activity tracking
✓ Login/logout tracking
✓ Application activity
✓ Alerts
✓ Employee management
✓ Employee editing
✓ Employee activation/deactivation
✓ Device assignment
✓ Device reassignment
✓ Device unassignment
✓ Device selection
✓ Bulk device activation
✓ Bulk device deactivation
✓ Remote monitoring
✓ Authorized screen streaming
✓ H.264 screen transport
✓ Frontend production build
✓ Rust agent compilation
```

---

# 57. Current Development Stage

The next major feature is:

# Remote Device Commands

Specifically:

```text
Remote Restart
Remote Shutdown
```

The intended flow is:

```text
Administrator
      │
      ▼
Device action menu
      │
      ├── Restart
      │
      └── Shutdown
             │
             ▼
       Confirmation modal
             │
             ▼
       Backend command API
             │
             ▼
       Device WebSocket
             │
             ▼
        Rust Agent
             │
             ▼
        Operating System
```

---

# 58. Remote Restart

The planned single-device restart functionality will allow an authorized administrator to select:

```text
Restart device
```

The platform should not immediately execute the action.

Instead:

```text
Restart
   ↓
Confirmation dialog
   ↓
Administrator confirms
   ↓
Backend validates request
   ↓
Backend sends command
   ↓
Rust agent receives command
   ↓
Agent requests OS restart
```

---

# 59. Remote Shutdown

The same architecture will support:

```text
Shutdown device
```

Because shutdown is more destructive than normal UI actions, the confirmation experience should clearly identify:

```text
Device name
Hostname
Assigned employee
Action
```

and require explicit confirmation.

---

# 60. Audit Trail for Remote Commands

Remote commands should be recorded in the Activity system.

For example:

```text
REMOTE_RESTART
```

could produce:

```text
Administrator requested a restart of Office PC 001
```

Likewise:

```text
REMOTE_SHUTDOWN
```

could produce:

```text
Administrator requested a shutdown of Office PC 001
```

The audit record should preserve:

```text
Device
Employee
Command
Timestamp
Initiating administrator
```

where the existing authentication/user model makes that information available.

---

# 61. Command Authorization

Remote commands must only be accepted from authorized administrative actions.

The backend should validate:

```text
Authenticated administrator
        +
Authorized device
        +
Valid command
        +
Current device state
```

before sending a command.

The Rust agent should not accept arbitrary commands from arbitrary clients.

Commands should arrive through the authenticated/authorized GBOS device connection.

---

# 62. Command Safety

Remote restart/shutdown should include safeguards.

For example:

```text
Do not execute merely because a menu item was clicked.

Require explicit confirmation.

Clearly identify the target device.

Do not silently execute commands.

Record the action.

Handle offline devices gracefully.

Return an acknowledgement when possible.
```

The backend should also prevent malformed or unsupported commands.

---

# 63. Future Bulk Remote Commands

After single-device commands are stable, the same architecture can be extended to selected devices.

Potential interface:

```text
3 devices selected

[Activate]
[Deactivate]
[Restart]
[Shutdown]
```

Restart/shutdown should have stronger confirmation than activation/deactivation because they immediately affect device availability.

---

# 64. Future Global Shutdown

A later feature may provide:

```text
Shutdown all devices
```

This should have a much stronger confirmation flow.

For example:

```text
WARNING

You are about to shut down every eligible managed device.

Affected devices:
47

This action may interrupt users and active work.

Type a confirmation phrase to continue.
```

This should not be implemented until individual remote commands and bulk commands are reliable.

---

# 65. Recommended Development Order

The remaining device-management roadmap is:

```text
1. Single-device Restart
2. Single-device Shutdown
3. Confirmation modal
4. Backend command endpoint
5. WebSocket command delivery
6. Rust command receiver
7. OS restart/shutdown execution
8. Command acknowledgement
9. Audit activity
10. Offline/error handling
11. Test restart
12. Test shutdown
13. Bulk restart
14. Bulk shutdown
15. Strong global shutdown workflow
```

---

# 66. Development Workflow

The preferred development workflow is incremental.

For backend changes:

```bash
cd ~/Desktop/gbos/backend
source venv/bin/activate
```

For frontend:

```bash
cd ~/Desktop/gbos/gbos
```

For Rust:

```bash
cd ~/Desktop/gbos/agent
```

After modifying a frontend file:

```bash
npm run build
```

After modifying Rust:

```bash
cargo check
```

For Python files:

```bash
python -m py_compile path/to/file.py
```

Database verification can be performed with:

```bash
psql -d gbos
```

Activity records can be checked with:

```bash
psql -d gbos -c "SELECT id, activity_type, timestamp FROM activities ORDER BY timestamp DESC LIMIT 10;"
```

---

# 67. Verification Philosophy

GBOS development has followed a verify-before-continue approach.

After writing files, verify them using commands such as:

```bash
wc -c path/to/file
```

```bash
sed -n '1,200p' path/to/file
```

```bash
grep -n "specific text" path/to/file
```

This is particularly useful because a file appearing correctly in an editor does not necessarily guarantee that the filesystem contains the expected content.

---

# 68. Important Path Reference

For quick reference:

```text
PROJECT
/Users/gbaja/Desktop/gbos

FRONTEND
/Users/gbaja/Desktop/gbos/gbos

BACKEND
/Users/gbaja/Desktop/gbos/backend

AGENT
/Users/gbaja/Desktop/gbos/agent

DATABASE
gbos

BACKEND
http://127.0.0.1:8000

FRONTEND
http://localhost:3000

DEVICE WEBSOCKET
ws://127.0.0.1:8000/ws/devices/1
```

---

# 69. Current Device

```text
Device ID:       1
Device Name:     Office PC 001
Hostname:        GBOS-PC-001
OS:              Windows 11
IP:              192.168.1.101
Employee ID:     1
Employee:        John Doe
```

The device has successfully demonstrated:

```text
✓ WebSocket connectivity
✓ Heartbeat
✓ Telemetry
✓ Login detection
✓ Application activity
✓ Screen capture
✓ H.264 screen streaming
```

---

# 70. Current Employee

```text
Employee ID:     1
Name:            John Doe
Department:      IT
Job Title:       Systems Administrator
Email:           john.doe@gbos.com
Phone:           +2348012345678
Status:          Active
```

---

# 71. Current Architecture Summary

The GBOS platform can be summarized as:

```text
                         ┌─────────────────────┐
                         │     GBOS ADMIN      │
                         │      FRONTEND       │
                         │      Next.js        │
                         └──────────┬──────────┘
                                    │
                          HTTP / WebSocket
                                    │
                                    ▼
                         ┌─────────────────────┐
                         │    FASTAPI API      │
                         │      BACKEND        │
                         └───────┬─────┬───────┘
                                 │     │
                       PostgreSQL│     │WebSocket
                                 │     │
                                 ▼     ▼
                         ┌──────────┐  ┌──────────────┐
                         │PostgreSQL│  │ Rust Agent   │
                         │   GBOS   │  │              │
                         └──────────┘  └──────┬───────┘
                                              │
                                              ▼
                                      Managed Computer
                                              │
                           ┌──────────────────┼──────────────────┐
                           │                  │                  │
                           ▼                  ▼                  ▼
                       Telemetry          Activity          Screen
                       CPU/RAM/etc.       Login/App        H.264
```

---

# 72. Final Current-State Summary

GBOS now has the foundation of a complete company device-management platform.

The system can currently:

* Manage employees.
* Assign employees to devices.
* Reassign devices.
* Unassign devices.
* Monitor device connectivity.
* Collect system telemetry.
* Track login/logout events.
* Track authorized application activity.
* Generate health alerts.
* Display device health.
* Select multiple devices.
* Activate/deactivate devices individually.
* Activate/deactivate devices in bulk.
* Establish authorized remote monitoring sessions.
* Stream an authorized device screen using H.264.
* Maintain an auditable activity history.
* Recover cleanly from WebSocket disconnections.
* Build successfully on the frontend.
* Compile successfully on the Rust agent.

The immediate next milestone is:

```text
REMOTE RESTART + REMOTE SHUTDOWN
```

The implementation should begin by inspecting and extending the Rust agent's existing WebSocket receive loop, because the backend currently has no restart/shutdown command handler.

The intended command architecture is:

```text
GBOS Admin
    ↓
Confirmation
    ↓
Backend
    ↓
Device WebSocket
    ↓
Rust Agent
    ↓
OS Command
    ↓
Restart / Shutdown
    ↓
Audit Activity
```

Once that is complete, GBOS will have moved from primarily **monitoring and management** into **controlled remote device administration**.
