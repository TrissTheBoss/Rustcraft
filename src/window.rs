//! Minimal native Windows presentation layer for the dependency-free prototype.
#![cfg(windows)]

use std::ffi::c_void;
use std::mem::size_of;
use std::ptr::{null, null_mut};

use crate::render::{
    mesh_chunk, Camera, CpuRenderer, DirectionalLight, InputKey, InputState, Material,
    MaterialRegistry, Renderer, Vec3,
};
use crate::terrain::{TerrainGenerator, ROCK, SOIL, SURFACE};
use crate::world::{BlockId, ChunkPos, World};

type Hinstance = *mut c_void;
type Hwnd = *mut c_void;
type Hdc = *mut c_void;
type Lresult = isize;
type Wparam = usize;
type Lparam = isize;

const WM_DESTROY: u32 = 0x0002;
const WM_PAINT: u32 = 0x000F;
const WM_ERASEBKGND: u32 = 0x0014;
const WM_TIMER: u32 = 0x0113;
const WM_KEYDOWN: u32 = 0x0100;
const WM_KEYUP: u32 = 0x0101;
const WM_MOUSEMOVE: u32 = 0x0200;
const CS_HREDRAW: u32 = 0x0002;
const CS_VREDRAW: u32 = 0x0001;
const WS_OVERLAPPEDWINDOW: u32 = 0x00CF0000;
const SW_SHOW: i32 = 5;
const DIB_RGB_COLORS: u32 = 0;
const SRCCOPY: u32 = 0x00CC0020;
const TIMER_ID: usize = 1;
const VK_ESCAPE: u32 = 0x1B;
const WM_LBUTTONDOWN: u32 = 0x0201;
const SM_CXSCREEN: i32 = 0;
const SM_CYSCREEN: i32 = 1;

#[repr(C)]
struct Point {
    x: i32,
    y: i32,
}
#[repr(C)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}
#[repr(C)]
struct PaintStruct {
    hdc: Hdc,
    erase: i32,
    rect: Rect,
    restore: i32,
    inc_update: i32,
    reserved: [u8; 32],
}
#[repr(C)]
struct WndClassW {
    style: u32,
    wnd_proc: Option<unsafe extern "system" fn(Hwnd, u32, Wparam, Lparam) -> Lresult>,
    extra_class: i32,
    extra_window: i32,
    instance: Hinstance,
    icon: *mut c_void,
    cursor: *mut c_void,
    background: *mut c_void,
    menu: *const u16,
    name: *const u16,
}
#[repr(C)]
struct BitmapInfoHeader {
    size: u32,
    width: i32,
    height: i32,
    planes: u16,
    bit_count: u16,
    compression: u32,
    size_image: u32,
    x_pels: i32,
    y_pels: i32,
    used: u32,
    important: u32,
}
#[repr(C)]
struct BitmapInfo {
    header: BitmapInfoHeader,
    colors: [[u8; 4]; 1],
}

#[link(name = "user32")]
extern "system" {
    fn GetModuleHandleW(name: *const u16) -> Hinstance;
    fn RegisterClassW(class: *const WndClassW) -> u16;
    fn CreateWindowExW(
        ex: u32,
        class: *const u16,
        title: *const u16,
        style: u32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        parent: Hwnd,
        menu: *mut c_void,
        instance: Hinstance,
        param: *mut c_void,
    ) -> Hwnd;
    fn ShowWindow(window: Hwnd, command: i32) -> i32;
    fn UpdateWindow(window: Hwnd) -> i32;
    fn GetMessageW(message: *mut Message, window: Hwnd, min: u32, max: u32) -> i32;
    fn TranslateMessage(message: *const Message) -> i32;
    fn DispatchMessageW(message: *const Message) -> Lresult;
    fn BeginPaint(window: Hwnd, paint: *mut PaintStruct) -> Hdc;
    fn EndPaint(window: Hwnd, paint: *const PaintStruct) -> i32;
    fn DefWindowProcW(window: Hwnd, message: u32, wparam: Wparam, lparam: Lparam) -> Lresult;
    fn PostQuitMessage(code: i32);
    fn InvalidateRect(window: Hwnd, rect: *const Rect, erase: i32) -> i32;
    fn SetTimer(window: Hwnd, id: usize, interval: u32, callback: *mut c_void) -> usize;
    fn ClipCursor(rect: *const Rect) -> i32;
    fn ShowCursor(show: i32) -> i32;
    fn GetWindowRect(window: Hwnd, rect: *mut Rect) -> i32;
    fn GetClientRect(window: Hwnd, rect: *mut Rect) -> i32;
    fn ClientToScreen(window: Hwnd, point: *mut Point) -> i32;
    fn SetCursorPos(x: i32, y: i32) -> i32;
    fn GetSystemMetrics(index: i32) -> i32;
}
#[link(name = "gdi32")]
extern "system" {
    fn StretchDIBits(
        hdc: Hdc,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        src_x: i32,
        src_y: i32,
        src_width: i32,
        src_height: i32,
        bits: *const c_void,
        info: *const BitmapInfo,
        usage: u32,
        rop: u32,
    ) -> i32;
}
#[repr(C)]
struct Message {
    hwnd: Hwnd,
    message: u32,
    wparam: Wparam,
    lparam: Lparam,
    time: u32,
    point: Point,
    private: u32,
}

static mut FRAME: Option<(Vec<u8>, BitmapInfo)> = None;
static mut APP: Option<AppState> = None;

struct AppState {
    camera: Camera,
    input: InputState,
    yaw: f32,
    pitch: f32,
    cursor_locked: bool,
    width: i32,
    height: i32,
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

#[allow(static_mut_refs)]
unsafe extern "system" fn window_proc(
    window: Hwnd,
    message: u32,
    wparam: Wparam,
    lparam: Lparam,
) -> Lresult {
    match message {
        WM_PAINT => {
            let mut paint = PaintStruct {
                hdc: null_mut(),
                erase: 0,
                rect: Rect {
                    left: 0,
                    top: 0,
                    right: 0,
                    bottom: 0,
                },
                restore: 0,
                inc_update: 0,
                reserved: [0; 32],
            };
            let hdc = BeginPaint(window, &mut paint);
            if let Some((pixels, info)) = FRAME.as_ref() {
                StretchDIBits(
                    hdc,
                    0,
                    0,
                    info.header.width,
                    info.header.height.abs(),
                    0,
                    0,
                    info.header.width,
                    info.header.height.abs(),
                    pixels.as_ptr() as *const c_void,
                    info,
                    DIB_RGB_COLORS,
                    SRCCOPY,
                );
            }
            EndPaint(window, &paint);
            0
        }
        WM_ERASEBKGND => 1,
        WM_TIMER if wparam == TIMER_ID => {
            if let Some(app) = APP.as_mut() {
                app.input.move_camera(&mut app.camera, 1.0 / 30.0, 8.0);
                let direction = Vec3::new(
                    app.pitch.cos() * app.yaw.cos(),
                    app.pitch.sin(),
                    app.pitch.cos() * app.yaw.sin(),
                );
                app.camera.target = app.camera.position + direction;
                FRAME = Some(build_frame(app.camera, app.width as u32, app.height as u32));
                InvalidateRect(window, null(), 0);
            }
            0
        }
        WM_MOUSEMOVE => {
            if let Some(app) = APP.as_mut() {
                let x = (lparam as u32 & 0xffff) as i16 as i32;
                let y = ((lparam as u32 >> 16) & 0xffff) as i16 as i32;
                if app.cursor_locked {
                    let mut client = Rect {
                        left: 0,
                        top: 0,
                        right: app.width,
                        bottom: app.height,
                    };
                    GetClientRect(window, &mut client);
                    let center_x = (client.left + client.right) / 2;
                    let center_y = (client.top + client.bottom) / 2;
                    app.yaw += (x - center_x) as f32 * 0.006;
                    app.pitch = (app.pitch - (y - center_y) as f32 * 0.006).clamp(-1.45, 1.45);
                    lock_cursor(window);
                }
            }
            0
        }
        WM_KEYDOWN | WM_KEYUP => {
            if let Some(app) = APP.as_mut() {
                if wparam as u32 == VK_ESCAPE && message == WM_KEYDOWN {
                    app.cursor_locked = false;
                    ClipCursor(null());
                    ShowCursor(1);
                }
                if let Some(key) = key_for(wparam as u32) {
                    app.input.set(key, message == WM_KEYDOWN);
                }
            }
            0
        }
        WM_LBUTTONDOWN => {
            if let Some(app) = APP.as_mut() {
                app.cursor_locked = true;
                ShowCursor(0);
                lock_cursor(window);
            }
            0
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(window, message, wparam, lparam),
    }
}

fn key_for(key: u32) -> Option<InputKey> {
    match key {
        0x57 => Some(InputKey::Forward),
        0x53 => Some(InputKey::Backward),
        0x41 => Some(InputKey::Left),
        0x44 => Some(InputKey::Right),
        0x20 => Some(InputKey::Up),
        0x11 => Some(InputKey::Down),
        _ => None,
    }
}

unsafe fn lock_cursor(window: Hwnd) {
    let mut rect = Rect {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    if GetClientRect(window, &mut rect) != 0 {
        let mut center = Point {
            x: (rect.left + rect.right) / 2,
            y: (rect.top + rect.bottom) / 2,
        };
        if ClientToScreen(window, &mut center) != 0 {
            SetCursorPos(center.x, center.y);
        }
        GetWindowRect(window, &mut rect);
        ClipCursor(&rect);
    }
}

pub fn run() {
    let width = unsafe { GetSystemMetrics(SM_CXSCREEN) };
    let height = unsafe { GetSystemMetrics(SM_CYSCREEN) };
    unsafe {
        APP = Some(AppState {
            camera: Camera::default(),
            input: InputState::default(),
            yaw: -2.16,
            pitch: -0.4,
            cursor_locked: true,
            width,
            height,
        });
        FRAME = Some(build_frame(Camera::default(), width as u32, height as u32));
    }
    let class = wide("VoxelEngineWindow");
    let title = wide("Voxel Engine - Prototype");
    unsafe {
        let instance = GetModuleHandleW(null());
        let wnd_class = WndClassW {
            style: CS_HREDRAW | CS_VREDRAW,
            wnd_proc: Some(window_proc),
            extra_class: 0,
            extra_window: 0,
            instance,
            icon: null_mut(),
            cursor: null_mut(),
            background: null_mut(),
            menu: null(),
            name: class.as_ptr(),
        };
        RegisterClassW(&wnd_class);
        let window = CreateWindowExW(
            0,
            class.as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPEDWINDOW,
            0,
            0,
            width,
            height,
            null_mut(),
            null_mut(),
            instance,
            null_mut(),
        );
        SetTimer(window, TIMER_ID, 33, null_mut());
        ShowWindow(window, SW_SHOW);
        UpdateWindow(window);
        ShowCursor(0);
        lock_cursor(window);
        let mut message = Message {
            hwnd: null_mut(),
            message: 0,
            wparam: 0,
            lparam: 0,
            time: 0,
            point: Point { x: 0, y: 0 },
            private: 0,
        };
        while GetMessageW(&mut message, null_mut(), 0, 0) > 0 {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

fn build_frame(camera: Camera, width: u32, height: u32) -> (Vec<u8>, BitmapInfo) {
    let mut world = World::new();
    let generated = TerrainGenerator::default().generate_chunk(ChunkPos { x: 0, y: 0, z: 0 });
    let chunk = world.get_or_create_chunk(ChunkPos { x: 0, y: 0, z: 0 });
    *chunk = generated;
    let mesh = mesh_chunk(chunk, |x, y, z| {
        x >= 0
            && y >= 0
            && z >= 0
            && x < 16
            && y < 16
            && z < 16
            && chunk.get(x as usize, y as usize, z as usize).unwrap() != BlockId(0)
    });
    let mut materials = MaterialRegistry::default();
    materials
        .register(
            SOIL,
            Material {
                color: [110, 160, 86],
                texture: None,
            },
        )
        .unwrap();
    materials
        .register(
            SURFACE,
            Material {
                color: [150, 105, 66],
                texture: None,
            },
        )
        .unwrap();
    materials
        .register(
            ROCK,
            Material {
                color: [106, 106, 112],
                texture: None,
            },
        )
        .unwrap();
    let frame = CpuRenderer {
        clear_color: [92, 142, 190],
    }
    .render(
        &[&mesh],
        &camera,
        &materials,
        DirectionalLight::default(),
        width,
        height,
    )
    .unwrap();
    let pixels = frame
        .color
        .iter()
        .flat_map(|pixel| [pixel[2], pixel[1], pixel[0], 0])
        .collect();
    let info = BitmapInfo {
        header: BitmapInfoHeader {
            size: size_of::<BitmapInfoHeader>() as u32,
            width: width as i32,
            height: -(height as i32),
            planes: 1,
            bit_count: 32,
            compression: 0,
            size_image: 0,
            x_pels: 0,
            y_pels: 0,
            used: 0,
            important: 0,
        },
        colors: [[0; 4]; 1],
    };
    (pixels, info)
}
