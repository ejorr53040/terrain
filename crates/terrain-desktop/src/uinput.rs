//! The real desktop, reached through Linux's `/dev/uinput`: a virtual
//! absolute pointer (like a VM's tablet) and a virtual keyboard, which any
//! compositor picks up like plugged-in devices.

use std::{
    fs::{File, OpenOptions},
    io::{self, Write},
    mem,
    os::{fd::AsRawFd, unix::fs::OpenOptionsExt},
};

use glam::{IVec2, Vec2};

use crate::{Button, Input};

const EV_SYN: u16 = 0x00;
const EV_KEY: u16 = 0x01;
const EV_REL: u16 = 0x02;
const EV_ABS: u16 = 0x03;
const REL_HWHEEL: u16 = 0x06;
const REL_WHEEL: u16 = 0x08;
const REL_WHEEL_HI_RES: u16 = 0x0b;
const REL_HWHEEL_HI_RES: u16 = 0x0c;
/// High-resolution wheel units in one wheel step.
const HI_RES_STEP: i32 = 120;
const SYN_REPORT: u16 = 0;
const ABS_X: u16 = 0x00;
const ABS_Y: u16 = 0x01;
const BTN_LEFT: u16 = 0x110;
const BTN_RIGHT: u16 = 0x111;
const BTN_MIDDLE: u16 = 0x112;
const KEY_LEFTMETA: u16 = 125;
const BUS_VIRTUAL: u16 = 0x06;

/// Pointer coordinates run 0..=ABS_MAX across the screen.
const ABS_MAX: i32 = 65535;

// ioctl requests, from <linux/uinput.h>.
const UI_DEV_CREATE: libc::c_ulong = 0x5501;
const UI_DEV_DESTROY: libc::c_ulong = 0x5502;
const UI_DEV_SETUP: libc::c_ulong = 0x405c_5503;
const UI_ABS_SETUP: libc::c_ulong = 0x401c_5504;
const UI_SET_EVBIT: libc::c_ulong = 0x4004_5564;
const UI_SET_KEYBIT: libc::c_ulong = 0x4004_5565;
const UI_SET_RELBIT: libc::c_ulong = 0x4004_5566;
const UI_SET_ABSBIT: libc::c_ulong = 0x4004_5567;

#[repr(C)]
struct InputId {
    bustype: u16,
    vendor: u16,
    product: u16,
    version: u16,
}

#[repr(C)]
struct UinputSetup {
    id: InputId,
    name: [u8; 80],
    ff_effects_max: u32,
}

#[repr(C)]
struct AbsInfo {
    value: i32,
    minimum: i32,
    maximum: i32,
    fuzz: i32,
    flat: i32,
    resolution: i32,
}

#[repr(C)]
struct UinputAbsSetup {
    code: u16,
    absinfo: AbsInfo,
}

#[repr(C)]
struct InputEvent {
    time: libc::timeval,
    kind: u16,
    code: u16,
    value: i32,
}

/// One virtual input device.
struct Device {
    file: File,
}

impl Device {
    fn create(
        name: &str,
        product: u16,
        keys: &[u16],
        rel: &[u16],
        abs: &[u16],
    ) -> io::Result<Self> {
        let file = OpenOptions::new()
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open("/dev/uinput")?;
        let fd = file.as_raw_fd();
        let ioctl = |request: libc::c_ulong, arg: libc::c_ulong| -> io::Result<()> {
            // SAFETY: each request below is passed the argument type
            // <linux/uinput.h> defines for it.
            if unsafe { libc::ioctl(fd, request, arg) } < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        };
        ioctl(UI_SET_EVBIT, EV_KEY.into())?;
        for &key in keys {
            ioctl(UI_SET_KEYBIT, key.into())?;
        }
        if !rel.is_empty() {
            ioctl(UI_SET_EVBIT, EV_REL.into())?;
            for &axis in rel {
                ioctl(UI_SET_RELBIT, axis.into())?;
            }
        }
        if !abs.is_empty() {
            ioctl(UI_SET_EVBIT, EV_ABS.into())?;
            for &axis in abs {
                ioctl(UI_SET_ABSBIT, axis.into())?;
                let setup = UinputAbsSetup {
                    code: axis,
                    absinfo: AbsInfo {
                        value: ABS_MAX / 2,
                        minimum: 0,
                        maximum: ABS_MAX,
                        fuzz: 0,
                        flat: 0,
                        resolution: 0,
                    },
                };
                ioctl(UI_ABS_SETUP, &setup as *const _ as libc::c_ulong)?;
            }
        }
        let mut setup = UinputSetup {
            id: InputId {
                bustype: BUS_VIRTUAL,
                vendor: 0x7465, // "te"
                product,
                version: 1,
            },
            name: [0; 80],
            ff_effects_max: 0,
        };
        // The kernel wants the name NUL-terminated.
        assert!(name.len() < setup.name.len(), "device name too long");
        setup.name[..name.len()].copy_from_slice(name.as_bytes());
        ioctl(UI_DEV_SETUP, &setup as *const _ as libc::c_ulong)?;
        ioctl(UI_DEV_CREATE, 0)?;
        Ok(Self { file })
    }

    /// Sends `events` as one report.
    fn send(&mut self, events: &[(u16, u16, i32)]) -> io::Result<()> {
        let mut bytes = Vec::with_capacity((events.len() + 1) * mem::size_of::<InputEvent>());
        for &(kind, code, value) in events.iter().chain([&(EV_SYN, SYN_REPORT, 0)]) {
            let event = InputEvent {
                time: libc::timeval {
                    tv_sec: 0,
                    tv_usec: 0,
                },
                kind,
                code,
                value,
            };
            // SAFETY: InputEvent is plain old data with no padding beyond
            // what the kernel's struct input_event has.
            bytes.extend_from_slice(unsafe {
                std::slice::from_raw_parts(
                    &event as *const InputEvent as *const u8,
                    mem::size_of::<InputEvent>(),
                )
            });
        }
        self.file.write_all(&bytes)
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        // SAFETY: UI_DEV_DESTROY takes no argument. The kernel releases
        // anything still held as the device goes away.
        unsafe { libc::ioctl(self.file.as_raw_fd(), UI_DEV_DESTROY, 0) };
    }
}

/// The desktop's pointer and keyboard, driven by `Input`s.
pub struct VirtualDesktop {
    pointer: Device,
    keyboard: Device,
    /// High-resolution scrolling sent since the last whole wheel step
    /// (x right, y up), so plain-wheel readers step at the same pace.
    wheel: IVec2,
}

impl VirtualDesktop {
    /// Plugs in the virtual devices. Needs write access to `/dev/uinput`.
    pub fn new() -> io::Result<Self> {
        let pointer = Device::create(
            "terrain hand pointer",
            1,
            &[BTN_LEFT, BTN_RIGHT, BTN_MIDDLE],
            &[REL_WHEEL, REL_HWHEEL, REL_WHEEL_HI_RES, REL_HWHEEL_HI_RES],
            &[ABS_X, ABS_Y],
        )?;
        // Ordinary keyboard keys too, so it's taken for a keyboard.
        let keys: Vec<u16> = (1..=88).chain([KEY_LEFTMETA]).collect();
        let keyboard = Device::create("terrain hand keys", 2, &keys, &[], &[])?;
        Ok(Self {
            pointer,
            keyboard,
            wheel: IVec2::ZERO,
        })
    }

    pub fn apply(&mut self, input: Input) -> io::Result<()> {
        match input {
            Input::PointTo(at) => {
                let at = (at.clamp(Vec2::ZERO, Vec2::ONE) * ABS_MAX as f32).round();
                self.pointer
                    .send(&[(EV_ABS, ABS_X, at.x as i32), (EV_ABS, ABS_Y, at.y as i32)])
            }
            Input::Scroll(by) => {
                // The wheel's y turns up the page; Scroll's goes down it.
                let hi_res = (by * Vec2::new(1.0, -1.0) * HI_RES_STEP as f32)
                    .round()
                    .as_ivec2();
                self.wheel += hi_res;
                let steps = self.wheel / HI_RES_STEP;
                self.wheel -= steps * HI_RES_STEP;
                let mut events = vec![
                    (EV_REL, REL_HWHEEL_HI_RES, hi_res.x),
                    (EV_REL, REL_WHEEL_HI_RES, hi_res.y),
                ];
                if steps.x != 0 {
                    events.push((EV_REL, REL_HWHEEL, steps.x));
                }
                if steps.y != 0 {
                    events.push((EV_REL, REL_WHEEL, steps.y));
                }
                self.pointer.send(&events)
            }
            Input::Press(button) => self.button(button, 1),
            Input::Release(button) => self.button(button, 0),
        }
    }

    fn button(&mut self, button: Button, value: i32) -> io::Result<()> {
        match button {
            Button::Left => self.pointer.send(&[(EV_KEY, BTN_LEFT, value)]),
            Button::Right => self.pointer.send(&[(EV_KEY, BTN_RIGHT, value)]),
            Button::Super => self.keyboard.send(&[(EV_KEY, KEY_LEFTMETA, value)]),
        }
    }
}
