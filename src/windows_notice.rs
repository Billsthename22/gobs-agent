//! Visible, per-user monitoring notice and executable-name/idle sampling.
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::Duration;
use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{COLOR_WINDOW, GetSysColorBrush};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::SystemInformation::GetTickCount;
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW,
    Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{PCWSTR, PWSTR, w};

const TRAY_MESSAGE: u32 = WM_APP + 7;
const NOTICE: &str = "Managed by your company - GBOS\r\n\r\nCollected: active application executable names, active/idle state, device health and connection/login events.\r\n\r\nIdle means no keyboard or mouse input for 5 minutes. No keystrokes, window titles, document contents, or website URLs are collected by app tracking.\r\n\r\nLive screen viewing is enabled by an authorized administrator and displays a separate on-screen indicator. GPS is collected only when enabled by an administrator.\r\n\r\nContact your company administrator about monitoring. Closing this window keeps the GBOS tray icon available.";

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}

unsafe extern "system" fn window_proc(hwnd: HWND, message: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if message == WM_CLOSE {
        let _ = unsafe { ShowWindow(hwnd, SW_HIDE) };
        return LRESULT(0);
    }
    if message == TRAY_MESSAGE
        && [WM_LBUTTONUP, WM_LBUTTONDBLCLK, WM_RBUTTONUP].contains(&(lp.0 as u32))
    {
        let _ = unsafe { ShowWindow(hwnd, SW_SHOWNORMAL) };
        let _ = unsafe { SetForegroundWindow(hwnd) };
        return LRESULT(0);
    }
    unsafe { DefWindowProcW(hwnd, message, wp, lp) }
}

enum Command {
    Screen(bool, mpsc::Sender<bool>),
    Stop,
}

pub struct Notice {
    tx: mpsc::Sender<Command>,
    healthy: Arc<AtomicBool>,
}

impl Notice {
    pub fn start() -> Result<Self, String> {
        let (tx, rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::channel();
        let healthy = Arc::new(AtomicBool::new(false));
        let health = healthy.clone();
        std::thread::spawn(move || {
            let result = unsafe { run(rx, ready_tx, &health) };
            health.store(false, Ordering::SeqCst);
            if let Err(error) = result {
                crate::log_line!("Employee monitoring notice unavailable: {}", error);
            }
        });
        if ready_rx.recv_timeout(Duration::from_secs(10)) != Ok(true) {
            let _ = tx.send(Command::Stop);
            return Err("Could not create employee monitoring notice".into());
        }
        Ok(Self { tx, healthy })
    }
    pub fn healthy(&self) -> bool {
        self.healthy.load(Ordering::SeqCst)
    }
    pub fn screen(&self, enabled: bool) -> bool {
        let (tx, rx) = mpsc::channel();
        self.tx.send(Command::Screen(enabled, tx)).is_ok()
            && rx.recv_timeout(Duration::from_secs(2)) == Ok(true)
    }
}

impl Drop for Notice {
    fn drop(&mut self) {
        let _ = self.tx.send(Command::Stop);
    }
}

unsafe fn run(
    rx: mpsc::Receiver<Command>,
    ready: mpsc::Sender<bool>,
    healthy: &AtomicBool,
) -> windows::core::Result<()> {
    let instance = unsafe { GetModuleHandleW(None)? };
    let class = WNDCLASSW {
        lpfnWndProc: Some(window_proc),
        hInstance: instance.into(),
        lpszClassName: w!("GBOSMonitoringNotice"),
        hbrBackground: unsafe { GetSysColorBrush(COLOR_WINDOW) },
        ..Default::default()
    };
    if unsafe { RegisterClassW(&class) } == 0 {
        return Err(windows::core::Error::from_thread());
    }
    let hwnd = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class.lpszClassName,
            w!("GBOS - Company device monitoring"),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            540,
            350,
            None,
            None,
            Some(instance.into()),
            None,
        )?
    };
    let text = wide(NOTICE);
    let label = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("STATIC"),
            PCWSTR(text.as_ptr()),
            WS_CHILD | WS_VISIBLE,
            16,
            16,
            490,
            290,
            Some(hwnd),
            None,
            Some(instance.into()),
            None,
        )?
    };
    let mut tray = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: 1,
        uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
        uCallbackMessage: TRAY_MESSAGE,
        hIcon: unsafe { LoadIconW(None, IDI_INFORMATION)? },
        ..Default::default()
    };
    let tip =
        wide("GBOS: company-managed device; app and idle tracking enabled. Click for details.");
    tray.szTip[..tip.len()].copy_from_slice(&tip);
    if !unsafe { Shell_NotifyIconW(NIM_ADD, &tray) }.as_bool() {
        let _ = unsafe { DestroyWindow(hwnd) };
        return Err(windows::core::Error::from_thread());
    }
    // Onboarding supplies the initial notice. The tray opens these details on demand.
    healthy.store(true, Ordering::SeqCst);
    let _ = ready.send(true);
    let mut banner: Option<HWND> = None;
    let mut streaming = false;
    let mut refresh = std::time::Instant::now();
    'main: loop {
        while let Ok(command) = rx.try_recv() {
            match command {
                Command::Stop => break 'main,
                Command::Screen(enabled, reply) => {
                    if banner.is_some_and(|window| !unsafe { IsWindow(Some(window)) }.as_bool()) {
                        banner = None;
                    }
                    if enabled && banner.is_none() {
                        banner = unsafe { create_banner(instance.into()) }.ok();
                    }
                    if !enabled {
                        if let Some(window) = banner.take() {
                            let _ = unsafe { DestroyWindow(window) };
                        }
                    }
                    streaming = enabled && banner.is_some();
                    let status = wide(&format!(
                        "{}\r\n\r\nLive screen monitoring: {}",
                        NOTICE,
                        if streaming { "ACTIVE" } else { "off" }
                    ));
                    let _ = unsafe { SetWindowTextW(label, PCWSTR(status.as_ptr())) };
                    let _ = reply.send(!enabled || banner.is_some());
                }
            }
        }
        let mut message = MSG::default();
        while unsafe { PeekMessageW(&mut message, None, 0, 0, PM_REMOVE) }.as_bool() {
            let _ = unsafe { TranslateMessage(&message) };
            unsafe {
                DispatchMessageW(&message);
            }
        }
        if !unsafe { IsWindow(Some(hwnd)) }.as_bool() {
            break;
        }
        if streaming {
            if let Some(window) = banner {
                if !unsafe { IsWindow(Some(window)) }.as_bool() {
                    banner = None;
                }
            }
            if banner.is_none() {
                banner = unsafe { create_banner(instance.into()) }.ok();
            }
            if banner.is_none() {
                break;
            }
        }
        if refresh.elapsed() >= Duration::from_secs(2) {
            if !unsafe { Shell_NotifyIconW(NIM_MODIFY, &tray) }.as_bool()
                && !unsafe { Shell_NotifyIconW(NIM_ADD, &tray) }.as_bool()
            {
                break;
            }
            refresh = std::time::Instant::now();
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    healthy.store(false, Ordering::SeqCst);
    if let Some(window) = banner {
        let _ = unsafe { DestroyWindow(window) };
    }
    let _ = unsafe { Shell_NotifyIconW(NIM_DELETE, &tray) };
    let _ = unsafe { DestroyWindow(hwnd) };
    Ok(())
}

unsafe fn create_banner(
    instance: windows::Win32::Foundation::HINSTANCE,
) -> windows::core::Result<HWND> {
    unsafe {
        CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            w!("STATIC"),
            w!("GBOS: LIVE SCREEN MONITORING ACTIVE"),
            WS_POPUP | WS_VISIBLE | WS_BORDER,
            20,
            20,
            370,
            32,
            None,
            None,
            Some(instance),
            None,
        )
    }
}

/// Samples the foreground executable's basename only. Last-input time does not
/// expose which keys were pressed or where the mouse moved.
pub fn sample() -> Option<(Option<String>, bool)> {
    unsafe {
        let mut input = LASTINPUTINFO {
            cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
            ..Default::default()
        };
        if !GetLastInputInfo(&mut input).as_bool() {
            return None;
        }
        let idle = GetTickCount().wrapping_sub(input.dwTime) >= 300_000;
        let foreground = GetForegroundWindow();
        if foreground.is_invalid() {
            return Some((None, idle));
        }
        let mut pid = 0;
        GetWindowThreadProcessId(foreground, Some(&mut pid));
        let Ok(process) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return Some((None, idle));
        };
        let mut buffer = [0u16; 1024];
        let mut size = buffer.len() as u32;
        let result = QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut size,
        );
        let _ = CloseHandle(process);
        let app = result
            .ok()
            .and_then(|_| String::from_utf16(&buffer[..size as usize]).ok())
            .and_then(|path| {
                path.rsplit('\\')
                    .next()
                    .map(|name| name.chars().take(120).collect())
            });
        Some((app, idle))
    }
}
