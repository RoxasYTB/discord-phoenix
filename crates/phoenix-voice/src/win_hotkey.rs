use std::{ptr::null_mut, sync::mpsc, thread, time::Duration};

use windows_sys::Win32::UI::{
    Input::KeyboardAndMouse::{
        MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN, RegisterHotKey, UnregisterHotKey, VK_BACK, VK_DELETE, VK_DOWN, VK_END, VK_ESCAPE, VK_F1,
        VK_HOME, VK_INSERT, VK_LEFT, VK_NEXT, VK_OEM_3, VK_PRIOR, VK_RETURN, VK_RIGHT, VK_SPACE, VK_TAB, VK_UP,
    },
    WindowsAndMessaging::{MSG, PM_REMOVE, PeekMessageW, WM_HOTKEY},
};

type Keys = Vec<(String, String)>;

pub struct Worker(mpsc::Sender<Keys>);

impl Worker {
    pub fn start(event: impl Fn(&str) + Send + 'static, registered: impl Fn(Keys) + Send + 'static) -> std::io::Result<Self> {
        let (sender, receiver) = mpsc::channel::<Keys>();
        thread::Builder::new().name("phoenix-game-hotkeys".into()).spawn(move || {
            let mut active: Vec<(i32, String, String)> = Vec::new();
            loop {
                let wait = if active.is_empty() {
                    Duration::from_secs(60)
                } else {
                    Duration::from_millis(24)
                };
                match receiver.recv_timeout(wait) {
                    Ok(keys) => {
                        unregister(&mut active);
                        for (index, (id, stroke)) in keys.into_iter().enumerate() {
                            let Some((modifiers, key)) = parse(&stroke) else { continue };
                            let native = index as i32 + 1;
                            // SAFETY: The null HWND binds the hotkey to this thread and the native id is unique here.
                            if unsafe { RegisterHotKey(null_mut(), native, modifiers | MOD_NOREPEAT, key) } != 0 {
                                active.push((native, id, stroke));
                            }
                        }
                        registered(active.iter().map(|(_, id, stroke)| (id.clone(), stroke.clone())).collect());
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                }
                let mut msg = MSG::default();
                // SAFETY: The thread owns its message queue and msg is valid writable storage.
                while unsafe { PeekMessageW(&mut msg, null_mut(), WM_HOTKEY, WM_HOTKEY, PM_REMOVE) } != 0 {
                    if let Some((_, id, _)) = active.iter().find(|(native, _, _)| *native as usize == msg.wParam) {
                        event(id);
                    }
                }
            }
            unregister(&mut active);
            registered(Vec::new());
        })?;
        Ok(Self(sender))
    }

    pub fn replace(&self, keys: Keys) -> bool {
        self.0.send(keys).is_ok()
    }
}

fn unregister(active: &mut Vec<(i32, String, String)>) {
    for (id, _, _) in active.drain(..) {
        // SAFETY: Every id in active was registered on the current thread.
        unsafe {
            UnregisterHotKey(null_mut(), id);
        }
    }
}

fn parse(stroke: &str) -> Option<(u32, u32)> {
    let mut modifiers = 0;
    let mut parts = stroke.split('-').peekable();
    while let Some(part) = parts.next() {
        if parts.peek().is_none() {
            let key = match part {
                "space" => VK_SPACE,
                "return" | "enter" => VK_RETURN,
                "escape" => VK_ESCAPE,
                "tab" => VK_TAB,
                "backspace" => VK_BACK,
                "delete" => VK_DELETE,
                "insert" => VK_INSERT,
                "home" => VK_HOME,
                "end" => VK_END,
                "pageup" => VK_PRIOR,
                "pagedown" => VK_NEXT,
                "left" => VK_LEFT,
                "right" => VK_RIGHT,
                "up" => VK_UP,
                "down" => VK_DOWN,
                "grave" => VK_OEM_3,
                key if key.len() == 1 && key.as_bytes()[0].is_ascii_alphanumeric() => u16::from(key.as_bytes()[0].to_ascii_uppercase()),
                function if function.starts_with('f') => {
                    let number: u16 = function[1..].parse().ok()?;
                    if !(1..=24).contains(&number) {
                        return None;
                    }
                    VK_F1 + number - 1
                }
                _ => return None,
            };
            return (modifiers != 0).then_some((modifiers, u32::from(key)));
        }
        modifiers |= match part {
            "alt" => MOD_ALT,
            "ctrl" => MOD_CONTROL,
            "shift" => MOD_SHIFT,
            "cmd" | "win" | "platform" | "super" => MOD_WIN,
            _ => return None,
        };
    }
    None
}
