use std::ffi::c_void;

pub struct Renderer {
    frame_count: u32,
    gl_loaded: bool,
}

impl Renderer {
    pub fn new() -> Self {
        println!("[Rust] Renderer created!");
        Self {
            frame_count: 0,
            gl_loaded: false,
        }
    }

    pub fn render(&mut self) {
        self.frame_count += 1;

        // 1. 리눅스 환경의 OpenGL 함수 포인터 동적 로드 (최초 1회만)
        if !self.gl_loaded {
            unsafe {
                // 리눅스의 실제 OpenGL/EGL 동적 라이브러리를 직접 메모리에 로드합니다.
                let libgl = libc::dlopen(b"libGL.so.1\0".as_ptr() as *const _, libc::RTLD_LAZY);
                let libegl = libc::dlopen(b"libEGL.so.1\0".as_ptr() as *const _, libc::RTLD_LAZY);

                gl::load_with(|name| {
                    let symbol = std::ffi::CString::new(name).unwrap();
                    let mut ptr = std::ptr::null_mut();

                    // 1순위: libGL.so.1 에서 검색
                    if !libgl.is_null() {
                        ptr = libc::dlsym(libgl, symbol.as_ptr());
                    }
                    // 2순위: libEGL.so.1 에서 검색
                    if ptr.is_null() && !libegl.is_null() {
                        ptr = libc::dlsym(libegl, symbol.as_ptr());
                    }
                    // 3순위: 전역(글로벌) 심볼에서 검색
                    if ptr.is_null() {
                        ptr = libc::dlsym(libc::RTLD_DEFAULT, symbol.as_ptr());
                    }
                    ptr
                });
            }
            self.gl_loaded = true;
            println!("[Rust] OpenGL function pointers explicitly loaded from libGL/libEGL.");
        }

        // 2. 실제 OpenGL 렌더링 로직
        unsafe {
            // 배경을 파란색(Blue)으로 지웁니다!
            gl::ClearColor(0.0, 0.0, 1.0, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);
        }

        println!("[Rust] Rendered frame: {} (Blue Clear)", self.frame_count);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn create_renderer() -> *mut c_void {
    let renderer = Box::new(Renderer::new());
    Box::into_raw(renderer) as *mut c_void
}

#[unsafe(no_mangle)]
pub extern "C" fn render_frame(renderer_ptr: *mut c_void) {
    if renderer_ptr.is_null() {
        return;
    }
    let renderer = unsafe { &mut *(renderer_ptr as *mut Renderer) };
    renderer.render();
}
