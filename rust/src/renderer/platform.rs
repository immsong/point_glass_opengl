use std::ffi::{CString, c_void};
#[allow(unused_imports)]
use std::ptr;

#[cfg(target_os = "android")]
pub(crate) type AndroidEgl = khronos_egl::DynamicInstance<khronos_egl::EGL1_4>;

// Windows에서 OpenGL 함수를 동적으로 가져오기 위한 WinAPI
// opengl32.dll에서 기본 OpenGL 함수 주소를 찾을 때 사용
#[cfg(target_os = "windows")]
#[link(name = "kernel32")]
unsafe extern "system" {
    pub fn LoadLibraryA(lpLibFileName: *const u8) -> isize;
    pub fn GetProcAddress(hModule: isize, lpProcName: *const u8) -> *const c_void;
}

// ============================================================================
// Windows WGL offscreen context
// ============================================================================
//
// OpenGL은 그냥 함수만 호출한다고 동작하지 않음.
// 반드시 "현재 thread에 연결된 OpenGL context"가 있어야 함.
//
// Windows에서는 OpenGL context를 만들 때 WGL이라는 Windows 전용 API를 사용.
// 이 모듈은 Flutter 화면 위에 직접 OpenGL window를 띄우는 게 아니라,
// 보이지 않는 dummy window를 만들고 거기에 OpenGL context를 붙이는 역할.
//
// 이후 render_to_buffer()에서 이 context를 current로 만든 뒤,
// FBO에 렌더링하고 glReadPixels로 결과를 CPU buffer에 복사함.
#[cfg(target_os = "windows")]
mod wgl_helper {
    use std::ffi::c_void;

    // user32.dll 함수들
    // CreateWindowExA: OpenGL context 생성을 위한 dummy window 생성
    // GetDC: window의 device context 획득
    // ReleaseDC: device context 해제
    // DestroyWindow: dummy window 삭제
    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetDC(hWnd: isize) -> isize;
        fn ReleaseDC(hWnd: isize, hDC: isize) -> i32;
        fn DestroyWindow(hWnd: isize) -> i32;

        fn CreateWindowExA(
            ex: u32,
            cls: *const u8,
            name: *const u8,
            style: u32,
            x: i32,
            y: i32,
            w: i32,
            h: i32,
            parent: isize,
            menu: isize,
            inst: isize,
            param: *mut c_void,
        ) -> isize;
    }

    // gdi32.dll 함수들
    // OpenGL context를 만들려면 device context에 pixel format을 설정해야 함.
    #[link(name = "gdi32")]
    unsafe extern "system" {
        fn ChoosePixelFormat(hdc: isize, ppfd: *const u8) -> i32;
        fn SetPixelFormat(hdc: isize, format: i32, ppfd: *const u8) -> i32;
    }

    // opengl32.dll의 WGL 함수들
    // wglCreateContext: legacy OpenGL context 생성
    // wglDeleteContext: OpenGL context 삭제
    // wglMakeCurrent: 현재 thread에 OpenGL context 연결
    // wglGetProcAddress: 확장 OpenGL/WGL 함수 주소 조회
    #[link(name = "opengl32")]
    unsafe extern "system" {
        fn wglCreateContext(hdc: isize) -> isize;
        fn wglDeleteContext(hglrc: isize) -> i32;

        pub fn wglMakeCurrent(hdc: isize, hglrc: isize) -> i32;
        pub fn wglGetProcAddress(name: *const u8) -> *const c_void;
    }

    struct WglContext {
        hwnd: isize,
        hdc: isize,
        hglrc: isize,
    }

    impl Drop for WglContext {
        fn drop(&mut self) {
            unsafe { destroy_context_parts(self.hwnd, self.hdc, self.hglrc) };
        }
    }

    // OpenGL context는 thread에 current 상태로 연결되므로 thread별로 공유.
    // 같은 render thread에서 여러 Renderer를 사용해도 하나의 context를 재사용.
    thread_local! {
        static CTX: std::cell::RefCell<Option<WglContext>> =
            const { std::cell::RefCell::new(None) };
    }

    unsafe fn destroy_context_parts(hwnd: isize, hdc: isize, hglrc: isize) {
        unsafe {
            if hglrc != 0 {
                wglMakeCurrent(0, 0);
                wglDeleteContext(hglrc);
            }

            if hwnd != 0 && hdc != 0 {
                ReleaseDC(hwnd, hdc);
            }

            if hwnd != 0 {
                DestroyWindow(hwnd);
            }
        }
    }

    pub fn make_current() -> Result<(), String> {
        CTX.with(|ctx| {
            let mut ctx_ref = ctx.borrow_mut();

            if ctx_ref.is_none() {
                let hwnd = unsafe {
                    CreateWindowExA(
                        0,
                        b"STATIC\0".as_ptr(),
                        b"PointGlassOpenGL\0".as_ptr(),
                        0,
                        0,
                        0,
                        1,
                        1,
                        0,
                        0,
                        0,
                        std::ptr::null_mut(),
                    )
                };

                if hwnd == 0 {
                    return Err("CreateWindowExA failed".to_string());
                }

                let hdc = unsafe { GetDC(hwnd) };
                if hdc == 0 {
                    unsafe { destroy_context_parts(hwnd, 0, 0) };
                    return Err("GetDC failed".to_string());
                }

                let mut pfd = [0u8; 40];
                pfd[0] = 40;
                pfd[2] = 1;
                pfd[4] = 0x25;
                pfd[9] = 32;
                pfd[23] = 24;

                let pixel_format = unsafe { ChoosePixelFormat(hdc, pfd.as_ptr()) };
                if pixel_format == 0 {
                    unsafe { destroy_context_parts(hwnd, hdc, 0) };
                    return Err("ChoosePixelFormat failed".to_string());
                }

                if unsafe { SetPixelFormat(hdc, pixel_format, pfd.as_ptr()) } == 0 {
                    unsafe { destroy_context_parts(hwnd, hdc, 0) };
                    return Err("SetPixelFormat failed".to_string());
                }

                // wglCreateContextAttribsARB 주소를 얻기 위한 임시 legacy context.
                let temp_context = unsafe { wglCreateContext(hdc) };
                if temp_context == 0 {
                    unsafe { destroy_context_parts(hwnd, hdc, 0) };
                    return Err("wglCreateContext failed".to_string());
                }

                if unsafe { wglMakeCurrent(hdc, temp_context) } == 0 {
                    unsafe { destroy_context_parts(hwnd, hdc, temp_context) };
                    return Err("Failed to activate temporary WGL context".to_string());
                }

                let attrib_func =
                    unsafe { wglGetProcAddress(b"wglCreateContextAttribsARB\0".as_ptr()) };

                if attrib_func.is_null() {
                    unsafe { destroy_context_parts(hwnd, hdc, temp_context) };
                    return Err(
                        "OpenGL 3.3 Core Profile is unavailable: wglCreateContextAttribsARB not found"
                            .to_string(),
                    );
                }

                let create_context_attribs: unsafe extern "system" fn(
                    isize,
                    isize,
                    *const i32,
                ) -> isize = unsafe { std::mem::transmute(attrib_func) };

                let attribs = [
                    0x2091, 3, // WGL_CONTEXT_MAJOR_VERSION_ARB
                    0x2092, 3, // WGL_CONTEXT_MINOR_VERSION_ARB
                    0x9126, 0x00000001, // WGL_CONTEXT_PROFILE_MASK_ARB / CORE
                    0,
                ];

                let modern_context =
                    unsafe { create_context_attribs(hdc, 0, attribs.as_ptr()) };

                if modern_context == 0 {
                    unsafe { destroy_context_parts(hwnd, hdc, temp_context) };
                    return Err("Failed to create OpenGL 3.3 Core Profile context".to_string());
                }

                unsafe {
                    wglMakeCurrent(0, 0);
                    wglDeleteContext(temp_context);
                }

                if unsafe { wglMakeCurrent(hdc, modern_context) } == 0 {
                    unsafe { destroy_context_parts(hwnd, hdc, modern_context) };
                    return Err("Failed to activate OpenGL 3.3 Core Profile context".to_string());
                }

                *ctx_ref = Some(WglContext {
                    hwnd,
                    hdc,
                    hglrc: modern_context,
                });
            }

            let context = ctx_ref
                .as_ref()
                .ok_or_else(|| "WGL context is not initialized".to_string())?;

            if unsafe { wglMakeCurrent(context.hdc, context.hglrc) } == 0 {
                return Err("wglMakeCurrent failed".to_string());
            }

            Ok(())
        })
    }
}

// ============================================================================
// Platform OpenGL function loading
// ============================================================================
//
// 현재 OS에서 사용할 OpenGL 함수 주소를 로드.
// 실제 shader profile은 함수 로딩 이후 GL_VERSION 값을 기준으로 별도 판단.

#[cfg(target_os = "android")]
pub(crate) unsafe fn load_gl_functions() -> Result<(), String> {
    unsafe {
        let libgles = libc::dlopen(
            b"libGLESv3.so\0".as_ptr().cast(),
            libc::RTLD_LAZY | libc::RTLD_LOCAL,
        );
        let libegl = libc::dlopen(
            b"libEGL.so\0".as_ptr().cast(),
            libc::RTLD_LAZY | libc::RTLD_LOCAL,
        );

        if libgles.is_null() {
            return Err("Failed to open libGLESv3.so".to_string());
        }

        if libegl.is_null() {
            return Err("Failed to open libEGL.so".to_string());
        }

        type EglGetProcAddress = unsafe extern "C" fn(*const libc::c_char) -> *const c_void;

        let egl_get_proc_address = {
            let address = libc::dlsym(libegl, b"eglGetProcAddress\0".as_ptr().cast());
            if address.is_null() {
                None
            } else {
                Some(std::mem::transmute::<*mut c_void, EglGetProcAddress>(
                    address,
                ))
            }
        };

        gl::load_with(|name| {
            let symbol = CString::new(name).expect("OpenGL symbol contains null byte");
            let mut pointer = ptr::null();

            if let Some(get_proc_address) = egl_get_proc_address {
                pointer = get_proc_address(symbol.as_ptr());
            }

            if pointer.is_null() {
                pointer = libc::dlsym(libgles, symbol.as_ptr()).cast_const();
            }

            if pointer.is_null() {
                pointer = libc::dlsym(libegl, symbol.as_ptr()).cast_const();
            }

            pointer
        });
    }

    Ok(())
}

#[cfg(target_os = "linux")]
pub(crate) unsafe fn load_gl_functions() -> Result<(), String> {
    unsafe {
        let libgl = libc::dlopen(
            b"libGL.so.1\0".as_ptr().cast(),
            libc::RTLD_LAZY | libc::RTLD_LOCAL,
        );
        let libegl = libc::dlopen(
            b"libEGL.so.1\0".as_ptr().cast(),
            libc::RTLD_LAZY | libc::RTLD_LOCAL,
        );

        if libgl.is_null() && libegl.is_null() {
            return Err("Failed to open both libGL.so.1 and libEGL.so.1".to_string());
        }

        type GlxGetProcAddress = unsafe extern "C" fn(*const libc::c_uchar) -> *const c_void;
        type EglGetProcAddress = unsafe extern "C" fn(*const libc::c_char) -> *const c_void;

        let glx_get_proc_address = if libgl.is_null() {
            None
        } else {
            let address = libc::dlsym(libgl, b"glXGetProcAddressARB\0".as_ptr().cast());
            if address.is_null() {
                None
            } else {
                Some(std::mem::transmute::<*mut c_void, GlxGetProcAddress>(
                    address,
                ))
            }
        };

        let egl_get_proc_address = if libegl.is_null() {
            None
        } else {
            let address = libc::dlsym(libegl, b"eglGetProcAddress\0".as_ptr().cast());
            if address.is_null() {
                None
            } else {
                Some(std::mem::transmute::<*mut c_void, EglGetProcAddress>(
                    address,
                ))
            }
        };

        gl::load_with(|name| {
            let symbol = CString::new(name).expect("OpenGL symbol contains null byte");
            let mut pointer = ptr::null();

            if !libgl.is_null() {
                pointer = libc::dlsym(libgl, symbol.as_ptr()).cast_const();
            }

            if pointer.is_null() {
                if let Some(get_proc_address) = glx_get_proc_address {
                    pointer = get_proc_address(symbol.as_ptr().cast());
                }
            }

            if pointer.is_null() && !libegl.is_null() {
                pointer = libc::dlsym(libegl, symbol.as_ptr()).cast_const();
            }

            if pointer.is_null() {
                if let Some(get_proc_address) = egl_get_proc_address {
                    pointer = get_proc_address(symbol.as_ptr());
                }
            }

            if pointer.is_null() {
                pointer = libc::dlsym(libc::RTLD_DEFAULT, symbol.as_ptr()).cast_const();
            }

            pointer
        });
    }

    Ok(())
}

#[cfg(target_os = "windows")]
pub(crate) unsafe fn load_gl_functions() -> Result<(), String> {
    wgl_helper::make_current()?;

    let gl_library = unsafe { LoadLibraryA(b"opengl32.dll\0".as_ptr()) };
    if gl_library == 0 {
        return Err("Failed to load opengl32.dll".to_string());
    }

    gl::load_with(|name| {
        let symbol = CString::new(name).expect("OpenGL symbol contains null byte");
        let mut pointer = unsafe { wgl_helper::wglGetProcAddress(symbol.as_ptr().cast()) };
        let address = pointer as usize;

        if matches!(address, 0 | 1 | 2 | 3 | usize::MAX) {
            pointer = unsafe { GetProcAddress(gl_library, symbol.as_ptr().cast()) };
        }

        pointer
    });

    Ok(())
}

#[cfg(target_os = "windows")]
pub(crate) fn make_current() -> Result<(), String> {
    wgl_helper::make_current()
}
