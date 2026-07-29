use std::ffi::{CStr, CString, c_void};
use std::ptr;
use std::sync::Mutex;
use std::thread::ThreadId;

#[cfg(target_os = "android")]
type AndroidEgl = khronos_egl::DynamicInstance<khronos_egl::EGL1_4>;

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
// Shader profile
// ============================================================================
//
// shader는 OS가 아니라 현재 OpenGL context 종류에 맞춰 선택 필요.
//
// OpenGL ES 3.0+ context      -> GLSL ES 3.00 (#version 300 es)
// Desktop OpenGL 3.3+ context -> GLSL 3.30 Core (#version 330 core)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShaderProfile {
    Gles300,
    Gl330,
}

struct ShaderSources {
    points_vert: &'static str,
    points_frag: &'static str,
    gizmos_vert: &'static str,
    gizmos_frag: &'static str,
}

const POINTS_VERT_GLES300: &str = include_str!("../shaders/gles300/points.vert");
const POINTS_FRAG_GLES300: &str = include_str!("../shaders/gles300/points.frag");
const GIZMOS_VERT_GLES300: &str = include_str!("../shaders/gles300/gizmos.vert");
const GIZMOS_FRAG_GLES300: &str = include_str!("../shaders/gles300/gizmos.frag");

const POINTS_VERT_GL330: &str = include_str!("../shaders/gl330/points.vert");
const POINTS_FRAG_GL330: &str = include_str!("../shaders/gl330/points.frag");
const GIZMOS_VERT_GL330: &str = include_str!("../shaders/gl330/gizmos.vert");
const GIZMOS_FRAG_GL330: &str = include_str!("../shaders/gl330/gizmos.frag");

fn shader_sources(profile: ShaderProfile) -> ShaderSources {
    match profile {
        ShaderProfile::Gles300 => ShaderSources {
            points_vert: POINTS_VERT_GLES300,
            points_frag: POINTS_FRAG_GLES300,
            gizmos_vert: GIZMOS_VERT_GLES300,
            gizmos_frag: GIZMOS_FRAG_GLES300,
        },
        ShaderProfile::Gl330 => ShaderSources {
            points_vert: POINTS_VERT_GL330,
            points_frag: POINTS_FRAG_GL330,
            gizmos_vert: GIZMOS_VERT_GL330,
            gizmos_frag: GIZMOS_FRAG_GL330,
        },
    }
}

// ============================================================================
// Renderer
// ============================================================================
//
// Renderer는 실제 OpenGL 리소스와 상태를 가지고 있는 구조체.
// Dart 쪽에서는 이 Renderer를 직접 알지 못하고,
// create_renderer()가 반환한 raw pointer만 가지고 있음.
pub struct Renderer {
    // OpenGL 함수 로딩, shader, buffer 초기화 여부
    gl_loaded: bool,

    // OpenGL resource를 생성한 render thread.
    // 같은 Renderer의 GL resource를 다른 thread/context에서 사용하는 것을 방지.
    gl_thread_id: Option<ThreadId>,

    // point cloud용 shader program
    shader_points: u32,

    // grid / axis / polygon용 shader program
    shader_gizmos: u32,

    // point cloud용 VAO/VBO
    vao_points: u32,
    vbo_points: u32,

    // line용 VAO/VBO
    vao_lines: u32,
    vbo_lines: u32,

    // polygon용 VAO/VBO
    vao_polys: u32,
    vbo_polys: u32,

    // ------------------------------------------------------------------------
    // Windows offscreen render target
    // ------------------------------------------------------------------------
    //
    // Flutter Windows 쪽은 OpenGL framebuffer를 직접 표시하지 않고,
    // CPU buffer를 Flutter PixelBufferTexture에 넘기는 구조.
    //
    // 따라서 OpenGL은 화면이 아니라 FBO에 먼저 그림.
    // 그 다음 glReadPixels로 결과를 buffer에 복사.
    //
    // 예전 구조:
    // 매 프레임 FBO/Texture/DepthBuffer 생성 후 삭제
    //
    // 개선 구조:
    // Renderer가 FBO를 들고 있다가 크기가 같으면 재사용
    fbo: u32,
    fbo_tex: u32,
    fbo_depth: u32,
    fbo_width: u32,
    fbo_height: u32,

    // Dart에서 넘어온 point 데이터는 바로 GPU에 올리지 않고 pending에 보관.
    // render()가 호출될 때 pending 데이터를 VBO로 업로드.
    pending_points: Option<Vec<f32>>,
    point_count: i32,

    pending_lines: Option<Vec<f32>>,
    line_count: i32,

    pending_polys: Option<Vec<f32>>,
    poly_count: i32,

    // 현재 render target 크기
    width: u32,
    height: u32,

    // camera 회전
    yaw: f32,
    pitch: f32,
    roll: f32,

    // camera 거리
    radius: f32,

    // camera가 바라보는 중심점
    target_x: f32,
    target_y: f32,
    target_z: f32,

    // point cloud 표시 옵션
    alpha: f32,
    point_size: f32,
    value_min: f32,
    value_max: f32,
    color_mode: i32,

    #[cfg(target_os = "android")]
    pub egl: Option<AndroidEgl>,
    #[cfg(target_os = "android")]
    pub display: Option<khronos_egl::Display>,
    #[cfg(target_os = "android")]
    pub egl_surface: Option<khronos_egl::Surface>,
    #[cfg(target_os = "android")]
    pub context: Option<khronos_egl::Context>,
    #[cfg(target_os = "android")]
    pub config: Option<khronos_egl::Config>,
}

impl Renderer {
    pub fn new() -> Self {
        Self {
            gl_loaded: false,
            gl_thread_id: None,

            shader_points: 0,
            shader_gizmos: 0,

            vao_points: 0,
            vbo_points: 0,
            vao_lines: 0,
            vbo_lines: 0,
            vao_polys: 0,
            vbo_polys: 0,

            fbo: 0,
            fbo_tex: 0,
            fbo_depth: 0,
            fbo_width: 0,
            fbo_height: 0,

            pending_points: None,
            point_count: 0,

            pending_lines: None,
            line_count: 0,

            pending_polys: None,
            poly_count: 0,

            width: 1,
            height: 1,

            yaw: 0.0,
            pitch: 0.0,
            roll: 0.0,
            radius: 8.0,

            target_x: 0.0,
            target_y: 0.0,
            target_z: 0.0,

            alpha: 1.0,
            point_size: 3.0,
            value_min: -2.0,
            value_max: 5.0,
            color_mode: 0,

            #[cfg(target_os = "android")]
            egl: None,
            #[cfg(target_os = "android")]
            display: None,
            #[cfg(target_os = "android")]
            egl_surface: None,
            #[cfg(target_os = "android")]
            context: None,
            #[cfg(target_os = "android")]
            config: None,
        }
    }

    // OpenGL resource를 최초 생성한 thread와 현재 thread가 같은지 확인.
    // GL context/resource는 생성 thread에서만 사용하도록 제한.
    fn bind_gl_thread(&mut self) -> Result<(), String> {
        let current = std::thread::current().id();

        if let Some(owner) = self.gl_thread_id.as_ref() {
            if owner != &current {
                return Err(format!(
                    "OpenGL renderer thread changed: owner={owner:?}, current={current:?}"
                ));
            }
        } else {
            self.gl_thread_id = Some(current);
        }

        Ok(())
    }

    fn parse_version_number(value: &str) -> Option<(u32, u32)> {
        value.split_whitespace().find_map(|token| {
            if !token.as_bytes().first().is_some_and(u8::is_ascii_digit) {
                return None;
            }

            let mut parts = token.split('.');
            let major = parts.next()?.parse::<u32>().ok()?;
            let minor_text = parts.next()?;
            let minor_digits: String = minor_text
                .chars()
                .take_while(char::is_ascii_digit)
                .collect();
            let minor = minor_digits.parse::<u32>().ok()?;

            Some((major, minor))
        })
    }

    unsafe fn read_gl_string(name: u32, label: &str) -> Result<String, String> {
        let value = unsafe { gl::GetString(name) };

        if value.is_null() {
            return Err(format!("glGetString({label}) returned null"));
        }

        Ok(unsafe { CStr::from_ptr(value.cast()) }
            .to_string_lossy()
            .into_owned())
    }

    // 현재 연결된 context의 API와 버전을 검사하여 사용할 shader profile 반환.
    // OS가 아니라 glGetString(GL_VERSION)의 실제 결과를 기준으로 판단.
    unsafe fn detect_shader_profile() -> Result<ShaderProfile, String> {
        let gl_version = unsafe { Self::read_gl_string(gl::VERSION, "GL_VERSION") }?;
        let glsl_version =
            unsafe { Self::read_gl_string(gl::SHADING_LANGUAGE_VERSION, "GLSL_VERSION") }?;

        println!("OpenGL version: {gl_version}");
        println!("GLSL version: {glsl_version}");

        let gl_number = Self::parse_version_number(&gl_version)
            .ok_or_else(|| format!("Unable to parse OpenGL version: {gl_version}"))?;
        let glsl_number = Self::parse_version_number(&glsl_version)
            .ok_or_else(|| format!("Unable to parse GLSL version: {glsl_version}"))?;

        if gl_version.starts_with("OpenGL ES") {
            if gl_number < (3, 0) {
                return Err(format!(
                    "OpenGL ES 3.0 or later is required, current version: {gl_version}"
                ));
            }

            if !glsl_version.contains("GLSL ES") || glsl_number < (3, 0) {
                return Err(format!(
                    "GLSL ES 3.00 or later is required, current version: {glsl_version}"
                ));
            }

            return Ok(ShaderProfile::Gles300);
        }

        if gl_number < (3, 3) {
            return Err(format!(
                "Desktop OpenGL 3.3 or later is required, current version: {gl_version}"
            ));
        }

        if glsl_version.contains("GLSL ES") || glsl_number < (3, 30) {
            return Err(format!(
                "Desktop GLSL 3.30 or later is required, current version: {glsl_version}"
            ));
        }

        Ok(ShaderProfile::Gl330)
    }

    // shader source를 정규화하고 선택된 profile의 #version인지 확인.
    // shader 파일에 버전이 빠지거나 서로 뒤바뀐 경우 임의 보정하지 않고 오류 처리.
    fn clean_shader_source(src: &str, profile: ShaderProfile) -> Result<String, String> {
        let normalized = src
            .trim_start_matches('\u{feff}')
            .replace("\r\n", "\n")
            .replace('\r', "\n");

        let normalized = normalized.trim_start();
        let first_line = normalized.lines().next().unwrap_or_default().trim();

        let expected_version = match profile {
            ShaderProfile::Gles300 => "#version 300 es",
            ShaderProfile::Gl330 => "#version 330 core",
        };

        if first_line != expected_version {
            return Err(format!(
                "Shader profile mismatch: expected '{expected_version}', but starts with '{first_line}'"
            ));
        }

        Ok(normalized.to_string())
    }

    unsafe fn compile_shader(shader_type: u32, source: &str) -> Result<u32, String> {
        let shader = unsafe { gl::CreateShader(shader_type) };

        if shader == 0 {
            return Err("glCreateShader returned 0".to_string());
        }

        let c_source = match CString::new(source) {
            Ok(source) => source,
            Err(_) => {
                unsafe { gl::DeleteShader(shader) };
                return Err("Shader source contains an interior null byte".to_string());
            }
        };

        unsafe {
            gl::ShaderSource(shader, 1, &c_source.as_ptr(), ptr::null());
            gl::CompileShader(shader);
        }

        let mut success = 0;
        unsafe { gl::GetShaderiv(shader, gl::COMPILE_STATUS, &mut success) };

        if success == 0 {
            let mut length = 0;
            unsafe { gl::GetShaderiv(shader, gl::INFO_LOG_LENGTH, &mut length) };

            let mut log = vec![0u8; length.max(1) as usize];
            unsafe {
                gl::GetShaderInfoLog(shader, length, ptr::null_mut(), log.as_mut_ptr().cast());
                gl::DeleteShader(shader);
            }

            let log = String::from_utf8_lossy(&log)
                .trim_matches(char::from(0))
                .to_string();
            let preview = source.lines().take(12).collect::<Vec<_>>().join("\n");

            return Err(format!(
                "Shader compile failed:\n{log}\n--- source preview ---\n{preview}"
            ));
        }

        Ok(shader)
    }

    unsafe fn create_program(
        v_src: &str,
        f_src: &str,
        profile: ShaderProfile,
    ) -> Result<u32, String> {
        let vertex_source = Self::clean_shader_source(v_src, profile)?;
        let fragment_source = Self::clean_shader_source(f_src, profile)?;

        let vertex_shader = unsafe { Self::compile_shader(gl::VERTEX_SHADER, &vertex_source) }
            .map_err(|error| format!("Vertex {error}"))?;

        let fragment_shader =
            match unsafe { Self::compile_shader(gl::FRAGMENT_SHADER, &fragment_source) } {
                Ok(shader) => shader,
                Err(error) => {
                    unsafe { gl::DeleteShader(vertex_shader) };
                    return Err(format!("Fragment {error}"));
                }
            };

        let program = unsafe { gl::CreateProgram() };
        if program == 0 {
            unsafe {
                gl::DeleteShader(vertex_shader);
                gl::DeleteShader(fragment_shader);
            }
            return Err("glCreateProgram returned 0".to_string());
        }

        unsafe {
            gl::AttachShader(program, vertex_shader);
            gl::AttachShader(program, fragment_shader);
            gl::LinkProgram(program);
        }

        let mut success = 0;
        unsafe { gl::GetProgramiv(program, gl::LINK_STATUS, &mut success) };

        unsafe {
            gl::DeleteShader(vertex_shader);
            gl::DeleteShader(fragment_shader);
        }

        if success == 0 {
            let mut length = 0;
            unsafe { gl::GetProgramiv(program, gl::INFO_LOG_LENGTH, &mut length) };

            let mut log = vec![0u8; length.max(1) as usize];
            unsafe {
                gl::GetProgramInfoLog(program, length, ptr::null_mut(), log.as_mut_ptr().cast());
                gl::DeleteProgram(program);
            }

            let log = String::from_utf8_lossy(&log)
                .trim_matches(char::from(0))
                .to_string();

            return Err(format!("Program link failed:\n{log}"));
        }

        Ok(program)
    }

    // point cloud용 VAO/VBO 설정
    //
    // point 데이터 포맷:
    // [x, y, z, value]
    //
    // attribute 0: vec3 position
    // attribute 1: float value
    unsafe fn setup_buffers_points(vao: &mut u32, vbo: &mut u32) {
        unsafe {
            gl::GenVertexArrays(1, vao);
            gl::GenBuffers(1, vbo);

            gl::BindVertexArray(*vao);
            gl::BindBuffer(gl::ARRAY_BUFFER, *vbo);

            let stride = (4 * std::mem::size_of::<f32>()) as i32;

            gl::VertexAttribPointer(0, 3, gl::FLOAT, gl::FALSE, stride, ptr::null());
            gl::EnableVertexAttribArray(0);

            gl::VertexAttribPointer(
                1,
                1,
                gl::FLOAT,
                gl::FALSE,
                stride,
                (3 * std::mem::size_of::<f32>()) as *const c_void,
            );
            gl::EnableVertexAttribArray(1);

            gl::BindVertexArray(0);
        }
    }

    // line / polygon용 VAO/VBO 설정
    //
    // color 데이터 포맷:
    // [x, y, z, r, g, b, a, pad]
    //
    // attribute 0: vec3 position
    // attribute 1: vec4 color
    unsafe fn setup_buffers_color(vao: &mut u32, vbo: &mut u32) {
        unsafe {
            gl::GenVertexArrays(1, vao);
            gl::GenBuffers(1, vbo);

            gl::BindVertexArray(*vao);
            gl::BindBuffer(gl::ARRAY_BUFFER, *vbo);

            let stride = (8 * std::mem::size_of::<f32>()) as i32;

            gl::VertexAttribPointer(0, 3, gl::FLOAT, gl::FALSE, stride, ptr::null());
            gl::EnableVertexAttribArray(0);

            gl::VertexAttribPointer(
                1,
                4,
                gl::FLOAT,
                gl::FALSE,
                stride,
                (3 * std::mem::size_of::<f32>()) as *const c_void,
            );
            gl::EnableVertexAttribArray(1);

            gl::BindVertexArray(0);
        }
    }

    // OpenGL 함수 로딩 + shader/program/buffer 생성
    //
    // render()가 처음 호출될 때 한 번만 수행.
    // 이후에는 gl_loaded가 true라서 다시 하지 않음.
    unsafe fn ensure_gl_loaded(&mut self) -> Result<(), String> {
        self.bind_gl_thread()?;

        if self.gl_loaded {
            return Ok(());
        }

        #[cfg(target_os = "android")]
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

        #[cfg(target_os = "linux")]
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

        #[cfg(target_os = "windows")]
        unsafe {
            wgl_helper::make_current()?;

            let gl_library = LoadLibraryA(b"opengl32.dll\0".as_ptr());
            if gl_library == 0 {
                return Err("Failed to load opengl32.dll".to_string());
            }

            gl::load_with(|name| {
                let symbol = CString::new(name).expect("OpenGL symbol contains null byte");
                let mut pointer = wgl_helper::wglGetProcAddress(symbol.as_ptr().cast());
                let address = pointer as usize;

                if matches!(address, 0 | 1 | 2 | 3 | usize::MAX) {
                    pointer = GetProcAddress(gl_library, symbol.as_ptr().cast());
                }

                pointer
            });
        }

        let shader_profile = unsafe { Self::detect_shader_profile() }?;
        let shaders = shader_sources(shader_profile);

        println!("Selected shader profile: {shader_profile:?}");

        let points_program = unsafe {
            Self::create_program(shaders.points_vert, shaders.points_frag, shader_profile)
        }
        .map_err(|error| format!("Point shader initialization failed: {error}"))?;

        let gizmos_program = match unsafe {
            Self::create_program(shaders.gizmos_vert, shaders.gizmos_frag, shader_profile)
        } {
            Ok(program) => program,
            Err(error) => {
                unsafe { gl::DeleteProgram(points_program) };
                return Err(format!("Gizmo shader initialization failed: {error}"));
            }
        };

        self.shader_points = points_program;
        self.shader_gizmos = gizmos_program;

        unsafe {
            Self::setup_buffers_points(&mut self.vao_points, &mut self.vbo_points);
            Self::setup_buffers_color(&mut self.vao_lines, &mut self.vbo_lines);
            Self::setup_buffers_color(&mut self.vao_polys, &mut self.vbo_polys);

            // GL_PROGRAM_POINT_SIZE는 Desktop OpenGL에서만 활성화.
            // OpenGL ES에서는 별도 enable 없이 gl_PointSize가 적용됨.
            if shader_profile == ShaderProfile::Gl330 {
                gl::Enable(gl::PROGRAM_POINT_SIZE);
            }
        }

        self.gl_loaded = true;
        Ok(())
    }

    // FBO 관련 OpenGL 리소스 삭제
    unsafe fn delete_offscreen_target(&mut self) {
        unsafe {
            if self.fbo_depth != 0 {
                gl::DeleteRenderbuffers(1, &self.fbo_depth);
                self.fbo_depth = 0;
            }

            if self.fbo_tex != 0 {
                gl::DeleteTextures(1, &self.fbo_tex);
                self.fbo_tex = 0;
            }

            if self.fbo != 0 {
                gl::DeleteFramebuffers(1, &self.fbo);
                self.fbo = 0;
            }

            self.fbo_width = 0;
            self.fbo_height = 0;
        }
    }

    // offscreen FBO 준비
    //
    // width/height가 기존과 같으면 재사용.
    // 크기가 바뀌었거나 아직 없으면 새로 생성.
    #[cfg(target_os = "windows")]
    unsafe fn ensure_offscreen_target(&mut self, width: u32, height: u32) -> bool {
        if self.fbo != 0 && self.fbo_width == width && self.fbo_height == height {
            return true;
        }

        unsafe {
            // 기존 FBO가 있으면 먼저 삭제
            self.delete_offscreen_target();

            // FBO 생성
            gl::GenFramebuffers(1, &mut self.fbo);
            gl::BindFramebuffer(gl::FRAMEBUFFER, self.fbo);

            // FBO에 붙일 color texture 생성
            gl::GenTextures(1, &mut self.fbo_tex);
            gl::BindTexture(gl::TEXTURE_2D, self.fbo_tex);

            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as i32);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as i32);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as i32);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as i32);

            // 실제 texture storage 할당
            gl::TexImage2D(
                gl::TEXTURE_2D,
                0,
                gl::RGBA as i32,
                width as i32,
                height as i32,
                0,
                gl::RGBA,
                gl::UNSIGNED_BYTE,
                ptr::null(),
            );

            // color texture를 FBO의 COLOR_ATTACHMENT0에 연결
            gl::FramebufferTexture2D(
                gl::FRAMEBUFFER,
                gl::COLOR_ATTACHMENT0,
                gl::TEXTURE_2D,
                self.fbo_tex,
                0,
            );

            // depth buffer 생성
            // 3D rendering에서는 앞뒤 관계 계산을 위해 depth buffer가 필요
            gl::GenRenderbuffers(1, &mut self.fbo_depth);
            gl::BindRenderbuffer(gl::RENDERBUFFER, self.fbo_depth);

            gl::RenderbufferStorage(
                gl::RENDERBUFFER,
                gl::DEPTH_COMPONENT24,
                width as i32,
                height as i32,
            );

            gl::FramebufferRenderbuffer(
                gl::FRAMEBUFFER,
                gl::DEPTH_ATTACHMENT,
                gl::RENDERBUFFER,
                self.fbo_depth,
            );

            // 이 FBO에서 어떤 color attachment에 그릴지 지정
            let draw_buffers = [gl::COLOR_ATTACHMENT0];
            gl::DrawBuffers(1, draw_buffers.as_ptr());

            // ReadPixels가 읽을 attachment 지정
            gl::ReadBuffer(gl::COLOR_ATTACHMENT0);

            // FBO가 완전한지 확인
            let status = gl::CheckFramebufferStatus(gl::FRAMEBUFFER);

            if status != gl::FRAMEBUFFER_COMPLETE {
                println!("FBO incomplete: 0x{:x}", status);

                self.delete_offscreen_target();
                gl::BindFramebuffer(gl::FRAMEBUFFER, 0);

                return false;
            }

            self.fbo_width = width;
            self.fbo_height = height;

            true
        }
    }

    // Renderer가 소유한 OpenGL 리소스 정리
    unsafe fn delete_gl_resources(&mut self) {
        unsafe {
            self.delete_offscreen_target();

            if self.vbo_points != 0 {
                gl::DeleteBuffers(1, &self.vbo_points);
                self.vbo_points = 0;
            }

            if self.vao_points != 0 {
                gl::DeleteVertexArrays(1, &self.vao_points);
                self.vao_points = 0;
            }

            if self.vbo_lines != 0 {
                gl::DeleteBuffers(1, &self.vbo_lines);
                self.vbo_lines = 0;
            }

            if self.vao_lines != 0 {
                gl::DeleteVertexArrays(1, &self.vao_lines);
                self.vao_lines = 0;
            }

            if self.vbo_polys != 0 {
                gl::DeleteBuffers(1, &self.vbo_polys);
                self.vbo_polys = 0;
            }

            if self.vao_polys != 0 {
                gl::DeleteVertexArrays(1, &self.vao_polys);
                self.vao_polys = 0;
            }

            if self.shader_points != 0 {
                gl::DeleteProgram(self.shader_points);
                self.shader_points = 0;
            }

            if self.shader_gizmos != 0 {
                gl::DeleteProgram(self.shader_gizmos);
                self.shader_gizmos = 0;
            }

            self.gl_loaded = false;
            self.gl_thread_id = None;
        }
    }

    // camera 상태와 좌표계 보정을 반영한 MVP matrix 계산
    //
    // MVP = Projection * View * Model
    //
    // Model:
    // - 입력된 3D 좌표의 X축 방향 반전
    // - roll 값에 따른 Z축 회전
    //
    // View:
    // - camera 위치, 바라보는 방향, up 방향 반영
    //
    // Projection:
    // - perspective 투영과 화면 종횡비 반영
    //
    // 최종 변환 순서 (오른쪽에서 왼쪽으로 적용):
    // position -> X축 반전 -> Z축 roll 회전 -> camera view 변환 -> perspective projection
    fn calculate_mvp(&self) -> [f32; 16] {
        let eye_x = self.target_x + self.radius * self.pitch.cos() * self.yaw.sin();
        let eye_y = self.target_y + self.radius * self.pitch.sin();
        let eye_z = self.target_z + self.radius * self.pitch.cos() * self.yaw.cos();

        let up_x = -self.pitch.sin() * self.yaw.sin();
        let up_y = self.pitch.cos();
        let up_z = -self.pitch.sin() * self.yaw.cos();

        let view = look_at(
            [eye_x, eye_y, eye_z],
            [self.target_x, self.target_y, self.target_z],
            [up_x, up_y, up_z],
        );

        let aspect = self.width as f32 / self.height.max(1) as f32;
        let projection = perspective(45.0f32.to_radians(), aspect, 1.0, 10_000.0);

        // View와 Projection을 결합
        let view_projection = multiply_matrices(projection, view);

        let cos_z = self.roll.cos();
        let sin_z = self.roll.sin();

        // roll 값에 따른 Z축 회전 matrix
        let rotation = [
            cos_z, sin_z, 0.0, 0.0, -sin_z, cos_z, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ];

        // 입력 좌표의 X축 방향을 반전하는 matrix
        // (x, y, z) -> (-x, y, z)
        let flip_x = [
            -1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ];

        // column vector 기준으로 오른쪽 matrix부터 연산 적용.
        // model = rotation * flip_x
        // 따라서: 1. X축 반전 -> 2. Z축 roll 회전 순서로 적용됨
        let model = multiply_matrices(rotation, flip_x);

        multiply_matrices(view_projection, model)
    }

    // 실제 draw 함수
    //
    // 이 함수는 현재 OpenGL context와 framebuffer가 이미 준비되어 있다는 전제에서 동작.
    // Windows에서는 render_to_buffer()가 FBO를 bind한 뒤 이 함수를 호출.
    // Linux에서는 Flutter texture/context 쪽에서 준비된 상태에서 호출하는 구조.
    pub fn render(&mut self) {
        if let Err(error) = self.bind_gl_thread() {
            eprintln!("OpenGL render thread validation failed: {error}");
            return;
        }

        unsafe {
            #[cfg(target_os = "android")]
            {
                // nativeSetSurface()가 EGL 객체를 준비하기 전에 render_frame()이
                // 호출될 수 있으므로, 준비되지 않은 경우에는 이번 frame을 건너뜀.
                let (Some(egl), Some(display), Some(surface), Some(context)) = (
                    self.egl.as_ref(),
                    self.display,
                    self.egl_surface,
                    self.context,
                ) else {
                    return;
                };

                // EGL context는 thread에 연결되므로 실제 render_frame()이 실행되는
                // 현재 thread에서 매번 current 상태를 보장해야 함.
                if let Err(error) =
                    egl.make_current(display, Some(surface), Some(surface), Some(context))
                {
                    eprintln!("eglMakeCurrent failed: {error:?}");
                    return;
                }
            }

            // Android에서는 반드시 EGL context를 current로 만든 뒤 OpenGL 함수를
            // 로드하고 shader/VAO/VBO를 생성해야 함.
            if let Err(error) = self.ensure_gl_loaded() {
                eprintln!("OpenGL initialization failed: {error}");
                return;
            }

            #[cfg(target_os = "android")]
            {
                // Android window surface의 기본 framebuffer에 렌더링.
                gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
                gl::Viewport(0, 0, self.width as i32, self.height as i32);
            }

            // pending data가 있으면 GPU VBO로 업로드
            let upload_data = |pending: &mut Option<Vec<f32>>,
                               vao: u32,
                               vbo: u32,
                               count: &mut i32,
                               stride_floats: usize| {
                if let Some(data) = pending.take() {
                    *count = (data.len() / stride_floats) as i32;

                    gl::BindVertexArray(vao);
                    gl::BindBuffer(gl::ARRAY_BUFFER, vbo);

                    let data_ptr = if data.is_empty() {
                        ptr::null()
                    } else {
                        data.as_ptr() as *const c_void
                    };

                    gl::BufferData(
                        gl::ARRAY_BUFFER,
                        (data.len() * std::mem::size_of::<f32>()) as isize,
                        data_ptr,
                        gl::DYNAMIC_DRAW,
                    );
                }
            };

            upload_data(
                &mut self.pending_points,
                self.vao_points,
                self.vbo_points,
                &mut self.point_count,
                4,
            );

            upload_data(
                &mut self.pending_lines,
                self.vao_lines,
                self.vbo_lines,
                &mut self.line_count,
                8,
            );

            upload_data(
                &mut self.pending_polys,
                self.vao_polys,
                self.vbo_polys,
                &mut self.poly_count,
                8,
            );

            // 기본 OpenGL 상태 설정
            gl::Enable(gl::DEPTH_TEST);
            gl::DepthFunc(gl::LESS);
            gl::DepthMask(gl::TRUE);

            gl::Enable(gl::BLEND);
            gl::BlendFuncSeparate(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA, gl::ZERO, gl::ONE);

            gl::ColorMask(gl::TRUE, gl::TRUE, gl::TRUE, gl::TRUE);

            // 배경색 + color/depth buffer 초기화
            gl::ClearColor(0.1, 0.1, 0.1, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);

            #[allow(unused_mut)]
            let mut mvp = self.calculate_mvp();

            // Android SurfaceTexture를 Flutter Texture로 표시할 때
            // OpenGL 출력이 상하 반전되므로 clip-space Y만 반전
            #[cfg(target_os = "android")]
            {
                // column-major matrix의 두 번째 row를 반전
                mvp[1] = -mvp[1];
                mvp[5] = -mvp[5];
                mvp[9] = -mvp[9];
                mvp[13] = -mvp[13];
            }

            // ----------------------------------------------------------------
            // Grid / Axis / Polygon 렌더링
            // ----------------------------------------------------------------
            if self.line_count > 0 || self.poly_count > 0 {
                gl::UseProgram(self.shader_gizmos);

                let mvp_loc = gl::GetUniformLocation(
                    self.shader_gizmos,
                    b"uMVP\0".as_ptr() as *const gl::types::GLchar,
                );

                gl::UniformMatrix4fv(mvp_loc, 1, gl::FALSE, mvp.as_ptr());

                if self.line_count > 0 {
                    gl::BindVertexArray(self.vao_lines);
                    gl::DrawArrays(gl::LINES, 0, self.line_count);
                }

                if self.poly_count > 0 {
                    // polygon을 그릴 때 depth write를 잠깐 끔.
                    // 투명 plane 같은 요소가 depth buffer를 오염시키는 걸 줄이기 위함.
                    gl::DepthMask(gl::FALSE);

                    gl::BindVertexArray(self.vao_polys);
                    gl::DrawArrays(gl::TRIANGLES, 0, self.poly_count);

                    gl::DepthMask(gl::TRUE);
                }
            }

            // ----------------------------------------------------------------
            // Point cloud 렌더링
            // ----------------------------------------------------------------
            if self.point_count > 0 {
                gl::UseProgram(self.shader_points);

                let mvp_loc = gl::GetUniformLocation(
                    self.shader_points,
                    b"uMVP\0".as_ptr() as *const gl::types::GLchar,
                );

                let size_loc = gl::GetUniformLocation(
                    self.shader_points,
                    b"uPointSize\0".as_ptr() as *const gl::types::GLchar,
                );

                let min_loc = gl::GetUniformLocation(
                    self.shader_points,
                    b"uMin\0".as_ptr() as *const gl::types::GLchar,
                );

                let max_loc = gl::GetUniformLocation(
                    self.shader_points,
                    b"uMax\0".as_ptr() as *const gl::types::GLchar,
                );

                let alpha_loc = gl::GetUniformLocation(
                    self.shader_points,
                    b"uAlpha\0".as_ptr() as *const gl::types::GLchar,
                );

                let mode_loc = gl::GetUniformLocation(
                    self.shader_points,
                    b"uColorMode\0".as_ptr() as *const gl::types::GLchar,
                );

                // shader uniform 값 전달
                gl::UniformMatrix4fv(mvp_loc, 1, gl::FALSE, mvp.as_ptr());
                gl::Uniform1f(size_loc, self.point_size);
                gl::Uniform1f(min_loc, self.value_min);
                gl::Uniform1f(max_loc, self.value_max);
                gl::Uniform1f(alpha_loc, self.alpha);
                gl::Uniform1i(mode_loc, self.color_mode);

                gl::BindVertexArray(self.vao_points);

                // point_count 개수만큼 GL_POINTS로 그리기
                gl::DrawArrays(gl::POINTS, 0, self.point_count);
            }

            #[cfg(target_os = "android")]
            if let (Some(egl), Some(display), Some(surface)) =
                (&self.egl, &self.display, &self.egl_surface)
            {
                if let Err(error) = egl.swap_buffers(*display, *surface) {
                    eprintln!("eglSwapBuffers failed: {error:?}");
                }
            }

            // OpenGL 상태 정리
            gl::BindVertexArray(0);
            gl::UseProgram(0);
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width.max(1);
        self.height = height.max(1);
    }
}

// ============================================================================
// Math
// ============================================================================
//
// 아래 함수들은 OpenGL 자체 함수가 아니라,
// 우리가 직접 3D camera/projection 계산을 하기 위한 수학 함수들.
//
// OpenGL shader에는 최종적으로 uMVP matrix 하나만 넘김.
// 그 uMVP는 아래 흐름으로 만들어짐.
//
// Model matrix      : 물체 자체 회전/이동
// View matrix       : camera 위치/방향
// Projection matrix : 3D 공간을 2D 화면처럼 보이게 투영
//
// 최종:
// MVP = Projection * View * Model
//
// shader에서는 보통 이렇게 사용:
//
// gl_Position = uMVP * vec4(position, 1.0);

// camera 위치, target 위치, up 방향을 이용해서 view matrix 생성
//
// eye    : camera 위치
// target : camera가 바라보는 지점
// up     : 화면 위쪽 방향
//
// 예:
// eye    = [0, 3, 8]
// target = [0, 0, 0]
// up     = [0, 1, 0]
//
// 이 함수 결과는 "world 좌표를 camera 기준 좌표로 바꾸는 matrix"라고 보면 됨.
fn look_at(eye: [f32; 3], target: [f32; 3], up: [f32; 3]) -> [f32; 16] {
    // f = forward vector
    //
    // camera가 바라보는 방향.
    // target - eye 로 계산.
    //
    // normalize 해서 길이가 1인 방향 벡터로 만듦.
    let f = {
        let r = [target[0] - eye[0], target[1] - eye[1], target[2] - eye[2]];

        let len = (r[0] * r[0] + r[1] * r[1] + r[2] * r[2]).sqrt();

        [r[0] / len, r[1] / len, r[2] / len]
    };

    // s = side/right vector
    //
    // forward 방향과 up 방향의 cross product.
    // 즉 camera 기준 오른쪽 방향.
    //
    // cross(f, up)
    let s = {
        let r = [
            f[1] * up[2] - f[2] * up[1],
            f[2] * up[0] - f[0] * up[2],
            f[0] * up[1] - f[1] * up[0],
        ];

        let len = (r[0] * r[0] + r[1] * r[1] + r[2] * r[2]).sqrt();

        [r[0] / len, r[1] / len, r[2] / len]
    };

    // u = real up vector
    //
    // 입력으로 받은 up은 완전히 직교하지 않을 수 있음.
    // 그래서 s와 f를 기준으로 다시 up 방향을 계산.
    let u = [
        s[1] * f[2] - s[2] * f[1],
        s[2] * f[0] - s[0] * f[2],
        s[0] * f[1] - s[1] * f[0],
    ];

    // OpenGL column-major 기준 matrix.
    //
    // s: camera 오른쪽 축
    // u: camera 위쪽 축
    // -f: camera 뒤쪽 축
    //
    // 마지막 줄 쪽에는 camera 위치 보정값이 들어감.
    [
        s[0],
        u[0],
        -f[0],
        0.0,
        s[1],
        u[1],
        -f[1],
        0.0,
        s[2],
        u[2],
        -f[2],
        0.0,
        -(s[0] * eye[0] + s[1] * eye[1] + s[2] * eye[2]),
        -(u[0] * eye[0] + u[1] * eye[1] + u[2] * eye[2]),
        f[0] * eye[0] + f[1] * eye[1] + f[2] * eye[2],
        1.0,
    ]
}

// perspective projection matrix 생성
//
// fovy   : 세로 시야각, radian 단위
// aspect : width / height
// near   : camera에 가장 가까운 렌더링 거리
// far    : camera에서 가장 먼 렌더링 거리
//
// 이 matrix가 있어야 멀리 있는 점은 작게 보이고,
// 가까운 점은 크게 보이는 원근감이 생김.
fn perspective(fovy: f32, aspect: f32, near: f32, far: f32) -> [f32; 16] {
    // focal scale 같은 값.
    // fovy가 작을수록 zoom-in처럼 보임.
    let g = 1.0 / (fovy * 0.5).tan();

    [
        g / aspect,
        0.0,
        0.0,
        0.0,
        0.0,
        g,
        0.0,
        0.0,
        0.0,
        0.0,
        (far + near) / (near - far),
        -1.0,
        0.0,
        0.0,
        (2.0 * far * near) / (near - far),
        0.0,
    ]
}

// 4x4 matrix 곱셈
//
// OpenGL에서 주로 사용하는 column-major 형태 기준.
// a * b 결과를 반환.
//
// MVP 만들 때 사용:
// vp  = projection * view
// mvp = vp * model
fn multiply_matrices(a: [f32; 16], b: [f32; 16]) -> [f32; 16] {
    let mut res = [0.0f32; 16];

    for col in 0..4 {
        for row in 0..4 {
            res[col * 4 + row] = a[row] * b[col * 4]
                + a[4 + row] * b[col * 4 + 1]
                + a[8 + row] * b[col * 4 + 2]
                + a[12 + row] * b[col * 4 + 3];
        }
    }

    res
}

// 4x4 matrix 역행렬 계산
//
// 입력과 출력은 기존 OpenGL 코드와 동일한 column-major 형식을 사용.
//
// 화면의 NDC 좌표를 다시 3D 좌표로 되돌리려면
// MVP matrix의 역행렬이 필요함.
//
// 처리 방식:
//
// 1. column-major 배열을 행 단위 augmented matrix로 변환
// 2. Gauss-Jordan elimination으로 역행렬 계산
// 3. 결과를 다시 column-major 배열로 변환
//
// 역행렬을 만들 수 없는 singular matrix인 경우 None 반환.
fn invert_matrix(matrix: [f32; 16]) -> Option<[f32; 16]> {
    // 왼쪽에는 원본 matrix,
    // 오른쪽에는 identity matrix를 배치.
    //
    // [M | I]
    let mut augmented = [[0.0f32; 8]; 4];

    for row in 0..4 {
        for col in 0..4 {
            // 입력 matrix는 column-major 형식
            augmented[row][col] = matrix[col * 4 + row];
        }

        augmented[row][4 + row] = 1.0;
    }

    // Gauss-Jordan elimination 수행
    for pivot_col in 0..4 {
        // 현재 column에서 절댓값이 가장 큰 값을 pivot으로 선택.
        //
        // 작은 값을 pivot으로 사용하면 부동소수점 오차가 커질 수 있으므로
        // partial pivoting을 적용.
        let mut pivot_row = pivot_col;

        for row in (pivot_col + 1)..4 {
            if augmented[row][pivot_col].abs() > augmented[pivot_row][pivot_col].abs() {
                pivot_row = row;
            }
        }

        // pivot이 거의 0이면 역행렬을 만들 수 없는 matrix.
        if augmented[pivot_row][pivot_col].abs() < 1.0e-8 {
            return None;
        }

        // 필요한 경우 현재 row와 pivot row 교환
        if pivot_row != pivot_col {
            augmented.swap(pivot_row, pivot_col);
        }

        // pivot 값을 1로 정규화
        let pivot = augmented[pivot_col][pivot_col];

        for col in 0..8 {
            augmented[pivot_col][col] /= pivot;
        }

        // 다른 row의 현재 column 값을 0으로 제거
        for row in 0..4 {
            if row == pivot_col {
                continue;
            }

            let factor = augmented[row][pivot_col];

            for col in 0..8 {
                augmented[row][col] -= factor * augmented[pivot_col][col];
            }
        }
    }

    // 오른쪽에 만들어진 역행렬을
    // OpenGL column-major 배열로 다시 변환
    let mut inverse = [0.0f32; 16];

    for row in 0..4 {
        for col in 0..4 {
            inverse[col * 4 + row] = augmented[row][4 + col];
        }
    }

    Some(inverse)
}

// 4x4 matrix와 4차원 vector 곱셈
//
// 기존 shader와 project_3d_to_screen_batch()에서 사용하는 것과
// 동일한 OpenGL column-major 계산 방식을 사용.
//
// result = matrix * vector
fn transform_vec4(matrix: [f32; 16], vector: [f32; 4]) -> [f32; 4] {
    [
        matrix[0] * vector[0]
            + matrix[4] * vector[1]
            + matrix[8] * vector[2]
            + matrix[12] * vector[3],
        matrix[1] * vector[0]
            + matrix[5] * vector[1]
            + matrix[9] * vector[2]
            + matrix[13] * vector[3],
        matrix[2] * vector[0]
            + matrix[6] * vector[1]
            + matrix[10] * vector[2]
            + matrix[14] * vector[3],
        matrix[3] * vector[0]
            + matrix[7] * vector[1]
            + matrix[11] * vector[2]
            + matrix[15] * vector[3],
    ]
}

// ============================================================================
// FFI (Foreign Function Interface)
// ============================================================================
//
// Dart에서 Rust 함수를 호출하기 위한 C ABI 인터페이스.
//
// [Thread Safety 주의사항]
// Dart Isolate 특성상 여러 스레드에서 동시에 FFI를 호출할 수 있어
// Renderer 자체는 Mutex로 감싸서 메모리 충돌(Data Race)을 방지함.
//
// 하지만, OpenGL Context는 OS 스레드(Thread Local)에 강하게 종속됨.
// 즉, Mutex 락을 얻었더라도 GL context가 생성된 스레드와 현재 호출된 스레드가 다르면
// OpenGL 함수 호출 시 크래시가 나거나 무시됨.
// 따라서 UI 렌더링을 담당하는 메인/렌더 스레드 하나에서만 GL 관련 FFI를 호출해야 함.

type RendererHandle = Mutex<Renderer>;

// Renderer 생성
//
// Dart에서 가장 먼저 호출하는 함수.
//
// Rust 쪽에서는 Renderer를 Box로 heap에 생성하고,
// Box::into_raw로 raw pointer만 Dart에 넘김.
//
// Dart는 이 pointer 주소를 보관하고 있다가,
// set_points / update_camera / render_to_buffer 등에 다시 넘김.
#[unsafe(no_mangle)]
pub extern "C" fn create_renderer() -> *mut c_void {
    Box::into_raw(Box::new(Mutex::new(Renderer::new()))).cast()
}

// Renderer 제거
//
// create_renderer()에서 Box::into_raw로 넘긴 pointer는
// Rust가 자동으로 drop하지 못함.
// 그래서 반드시 destroy_renderer()에서 다시 Box::from_raw로 되돌려 drop해야 함.
//
// Windows에서는 OpenGL 리소스를 지우려면
// 먼저 해당 OpenGL context가 current 상태여야 안전함.
#[unsafe(no_mangle)]
pub extern "C" fn destroy_renderer(r: *mut c_void) {
    if r.is_null() {
        return;
    }

    // raw pointer를 다시 Box로 되돌림.
    // 호출자는 destroy와 다른 FFI 호출이 동시에 실행되지 않도록 보장해야 함.
    let handle = unsafe { Box::from_raw(r.cast::<RendererHandle>()) };
    let mut renderer = match handle.into_inner() {
        Ok(renderer) => renderer,
        Err(poisoned) => poisoned.into_inner(),
    };
    let current_thread = std::thread::current().id();
    let is_gl_thread = match renderer.gl_thread_id.as_ref() {
        Some(owner) => owner == &current_thread,
        None => true,
    };
    let mut can_delete_gl = is_gl_thread;

    #[cfg(target_os = "windows")]
    {
        if is_gl_thread {
            if let Err(error) = wgl_helper::make_current() {
                eprintln!("Failed to activate WGL context during destroy: {error}");
                can_delete_gl = false;
            }
        } else {
            eprintln!(
                "destroy_renderer was called from a different thread; Windows GL resource deletion is skipped"
            );
        }
    }

    #[cfg(target_os = "android")]
    {
        if is_gl_thread {
            if let (Some(egl), Some(display), Some(surface), Some(context)) = (
                renderer.egl.as_ref(),
                renderer.display,
                renderer.egl_surface,
                renderer.context,
            ) {
                if let Err(error) =
                    egl.make_current(display, Some(surface), Some(surface), Some(context))
                {
                    eprintln!("Failed to activate EGL context during destroy: {error:?}");
                    can_delete_gl = false;
                }
            }
        } else {
            eprintln!(
                "destroy_renderer was called from a different thread; Android GL resource deletion is skipped"
            );
        }
    }

    #[cfg(target_os = "linux")]
    if renderer.gl_loaded && can_delete_gl {
        let has_current_context = unsafe { !gl::GetString(gl::VERSION).is_null() };
        if !has_current_context {
            eprintln!(
                "No current Linux OpenGL context during destroy; GL resource deletion is skipped"
            );
            can_delete_gl = false;
        }
    }

    if renderer.gl_loaded && can_delete_gl {
        unsafe { renderer.delete_gl_resources() };
    }

    #[cfg(target_os = "android")]
    {
        let surface = renderer.egl_surface.take();
        let context = renderer.context.take();
        let display = renderer.display.take();
        let egl = renderer.egl.take();
        renderer.config = None;

        if let (Some(egl), Some(display)) = (egl, display) {
            if let Err(error) = egl.make_current(display, None, None, None) {
                eprintln!("Failed to clear current EGL context: {error:?}");
            }

            if let Some(surface) = surface {
                if let Err(error) = egl.destroy_surface(display, surface) {
                    eprintln!("Failed to destroy EGL surface: {error:?}");
                }
            }

            if let Some(context) = context {
                if let Err(error) = egl.destroy_context(display, context) {
                    eprintln!("Failed to destroy EGL context: {error:?}");
                }
            }

            if let Err(error) = egl.terminate(display) {
                eprintln!("Failed to terminate EGL display: {error:?}");
            }
        }
    }
}

// Linux 등 non-Windows에서 직접 frame render 요청
//
// Windows에서는 PixelBufferTexture callback 안에서 render_to_buffer()를 호출하므로
// render_frame()은 사용하지 않음.
//
// 기존 Dart 쪽에서 Linux는 render_frame,
// Windows는 render_to_buffer를 넘기던 구조와 맞음.
#[unsafe(no_mangle)]
pub extern "C" fn render_frame(r: *mut c_void) {
    if r.is_null() {
        return;
    }

    let handle = unsafe { &*r.cast::<RendererHandle>() };
    let mut renderer = handle
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    renderer.render();
}

// point cloud 데이터 설정
//
// d: f32 배열 pointer
// l: f32 개수
//
// 데이터 포맷:
// [x, y, z, value, x, y, z, value, ...]
//
// 여기서 바로 GPU에 올리지 않고 pending_points에 복사해둠.
// 실제 VBO 업로드는 render() 안에서 처리.
//
// 이유:
// - FFI 호출 thread와 OpenGL context thread가 다를 수 있음
// - OpenGL 작업은 render 시점에 몰아서 처리하는 쪽이 안정적
// 성능 주의:
// 현재는 안전을 위해 Dart 측 메모리를 Rust Vec으로 깊은 복사(Deep Copy)함.
// 수백만 개의 Point(수십~수백 MB)를 매 프레임 업데이트할 경우 메모리 병목이 발생할 수 있음.
// 향후 실시간 스트리밍이 필요하다면 Dart와 Rust가 메모리 버퍼를 공유(Zero-copy)하는 구조로 개선 필요.
#[unsafe(no_mangle)]
pub extern "C" fn set_points(r: *mut c_void, d: *const f32, l: usize) {
    if r.is_null() {
        return;
    }

    let handle = unsafe { &*r.cast::<RendererHandle>() };
    let mut re = handle
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    if l == 0 || d.is_null() {
        re.pending_points = Some(Vec::new());
    } else {
        // Dart 메모리를 계속 참조하면 위험하므로 Rust Vec으로 복사
        re.pending_points = Some(unsafe { std::slice::from_raw_parts(d, l) }.to_vec());
    }
}

// line 데이터 설정
//
// 데이터 포맷:
// [x, y, z, r, g, b, a, pad, ...]
//
// 주로 grid, axis 같은 선분 렌더링용.
#[unsafe(no_mangle)]
pub extern "C" fn set_lines(r: *mut c_void, d: *const f32, l: usize) {
    if r.is_null() {
        return;
    }

    let handle = unsafe { &*r.cast::<RendererHandle>() };
    let mut re = handle
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    if l == 0 || d.is_null() {
        re.pending_lines = Some(Vec::new());
    } else {
        re.pending_lines = Some(unsafe { std::slice::from_raw_parts(d, l) }.to_vec());
    }
}

// polygon 데이터 설정
//
// 데이터 포맷:
// [x, y, z, r, g, b, a, pad, ...]
//
// 삼각형 단위로 draw:
// gl::DrawArrays(gl::TRIANGLES, ...)
#[unsafe(no_mangle)]
pub extern "C" fn set_polygons(r: *mut c_void, d: *const f32, l: usize) {
    if r.is_null() {
        return;
    }

    let handle = unsafe { &*r.cast::<RendererHandle>() };
    let mut re = handle
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    if l == 0 || d.is_null() {
        re.pending_polys = Some(Vec::new());
    } else {
        re.pending_polys = Some(unsafe { std::slice::from_raw_parts(d, l) }.to_vec());
    }
}

// camera 회전/거리 갱신
//
// y    : yaw, 좌우 회전
// p    : pitch, 위아래 회전
// roll : z축 회전
// rad  : target으로부터 camera 거리
//
// Dart에서 마우스 drag, wheel zoom 등에 따라 이 값을 업데이트.
#[unsafe(no_mangle)]
pub extern "C" fn update_camera(r: *mut c_void, y: f32, p: f32, roll: f32, rad: f32) {
    if r.is_null() {
        return;
    }

    let handle = unsafe { &*r.cast::<RendererHandle>() };
    let mut re = handle
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    re.yaw = y;
    re.pitch = p;
    re.roll = roll;

    // camera 거리가 0 또는 음수가 되면 계산이 깨질 수 있어 최소값 보정
    re.radius = rad.max(0.1);
}

// Renderer 크기 갱신
//
// width/height는 projection aspect 계산에 사용.
// Windows render_to_buffer()에서도 매번 resize(w, h)를 호출함.
#[unsafe(no_mangle)]
pub extern "C" fn resize_renderer(r: *mut c_void, w: u32, h: u32) {
    if r.is_null() {
        return;
    }

    let handle = unsafe { &*r.cast::<RendererHandle>() };
    let mut re = handle
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    re.resize(w, h);
}

// camera pan
//
// 화면을 드래그해서 target 위치를 이동시키는 함수.
//
// dx/dy는 보통 screen drag delta.
// 이 값을 현재 camera 방향 기준의 right/up vector로 변환해서
// target_x/y/z를 이동시킴.
//
// 즉 물체를 움직이는 게 아니라,
// camera가 바라보는 중심점을 이동시키는 방식.
#[unsafe(no_mangle)]
pub extern "C" fn pan_camera(r: *mut c_void, dx: f32, dy: f32) {
    if r.is_null() {
        return;
    }

    let handle = unsafe { &*r.cast::<RendererHandle>() };
    let mut re = handle
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    // 현재 yaw 기준 오른쪽 방향
    let right_x = re.yaw.cos();
    let right_z = -re.yaw.sin();

    // 현재 yaw/pitch 기준 위쪽 방향
    let up_x = -re.pitch.sin() * re.yaw.sin();
    let up_y = re.pitch.cos();
    let up_z = -re.pitch.sin() * re.yaw.cos();

    // 멀리 있을수록 같은 drag도 더 크게 이동하게 보정
    let scale = re.radius * 0.001;

    re.target_x += (right_x * dx - up_x * dy) * scale;
    re.target_y += (-up_y * dy) * scale;
    re.target_z += (right_z * dx - up_z * dy) * scale;
}

// point cloud 표시 옵션 설정
//
// alpha : point 투명도
// size  : point size
// min   : value color mapping 최소값
// max   : value color mapping 최대값
// mode  : color mode
//
// 실제 색상 계산은 points.frag shader에서 처리한다고 보면 됨.
#[unsafe(no_mangle)]
pub extern "C" fn set_point_cloud_display_params(
    r: *mut c_void,
    alpha: f32,
    size: f32,
    min: f32,
    max: f32,
    mode: i32,
) {
    if r.is_null() {
        return;
    }

    let handle = unsafe { &*r.cast::<RendererHandle>() };
    let mut re = handle
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    re.alpha = alpha.clamp(0.0, 1.0);
    re.point_size = size;
    re.value_min = min;
    re.value_max = max;
    re.color_mode = mode;
}

// 3D 좌표를 2D screen 좌표로 project하는 함수
//
// 이 함수는 OpenGL 렌더링용이 아니라,
// Flutter overlay UI용으로 유용함.
//
// 예:
// - 3D point 위에 label 표시
// - axis label 표시
// - object 선택/annotation
//
// 입력:
// in_c  : [x, y, z, x, y, z, ...]
// count : point 개수
//
// 출력:
// out_c : [screen_x, screen_y, screen_x, screen_y, ...]
//
// 여기서 결과는 실제 pixel 좌표가 아니라 clip/NDC에 가까운 좌표.
// Dart 쪽에서 canvas size 기준으로 변환해서 쓰면 됨.
#[unsafe(no_mangle)]
pub extern "C" fn project_3d_to_screen_batch(
    r: *mut c_void,
    in_c: *const f32,
    count: usize,
    out_c: *mut f32,
) {
    if r.is_null() || in_c.is_null() || out_c.is_null() || count == 0 {
        return;
    }

    let handle = unsafe { &*r.cast::<RendererHandle>() };
    let re = handle
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    // 현재 camera 상태 기준 MVP 계산
    let mvp = re.calculate_mvp();

    for i in 0..count {
        let in_idx = i * 3;
        let out_idx = i * 2;

        let obj_x = unsafe { *in_c.add(in_idx) };
        let obj_y = unsafe { *in_c.add(in_idx + 1) };
        let obj_z = unsafe { *in_c.add(in_idx + 2) };

        // vec4 clip = MVP * vec4(position, 1.0)
        //
        // column-major matrix 기준 계산.
        let clip_x = obj_x * mvp[0] + obj_y * mvp[4] + obj_z * mvp[8] + mvp[12];
        let clip_y = obj_x * mvp[1] + obj_y * mvp[5] + obj_z * mvp[9] + mvp[13];
        let clip_w = obj_x * mvp[3] + obj_y * mvp[7] + obj_z * mvp[11] + mvp[15];

        // w가 너무 작거나 음수이면 camera 뒤쪽이거나 투영 불가능한 점.
        // Dart에서 쉽게 제외할 수 있게 큰 음수로 표시.
        if clip_w < 0.1 {
            unsafe {
                *out_c.add(out_idx) = -10000.0;
                *out_c.add(out_idx + 1) = -10000.0;
            }
        } else {
            // perspective divide
            //
            // clip 좌표를 w로 나누면 normalized device coordinate 비슷한 값이 됨.
            unsafe {
                *out_c.add(out_idx) = clip_x / clip_w;
                *out_c.add(out_idx + 1) = clip_y / clip_w;
            }
        }
    }
}

// 화면 좌표를 지정한 Z 평면의 3D 좌표로 변환
//
// Flutter에서 전달한 마우스 화면 좌표를 기준으로
// camera에서 3D 공간으로 향하는 ray를 생성하고,
// 그 ray가 지정된 Z 평면과 만나는 위치를 계산.
//
// 주로 다음 용도로 사용:
//
// - 마우스가 가리키는 grid 좌표 표시
// - Z = 0 바닥 평면의 X/Y 위치 표시
// - 3D viewer 위치 선택
//
// 입력:
//
// r            : Renderer pointer
// ndc_x        : 화면 X 좌표를 -1.0 ~ 1.0으로 변환한 NDC 값
// ndc_y        : 화면 Y 좌표를 -1.0 ~ 1.0으로 변환한 NDC 값
// plane_z      : 교차점을 계산할 Z 평면 높이
// out_position : 계산된 [x, y, z]를 저장할 배열 pointer
//
// 출력:
//
// 1 : 교차점 계산 성공
// 0 : 잘못된 pointer, 역행렬 계산 실패, 평면과 교차하지 않는 경우
//
// 좌표 변환 과정:
//
// 1. 현재 camera 상태로 MVP matrix 계산
// 2. MVP 역행렬 계산
// 3. NDC near/far 좌표를 3D 좌표로 역투영
// 4. near → far 방향으로 ray 생성
// 5. ray와 Z = plane_z 평면의 교차점 계산
#[unsafe(no_mangle)]
pub extern "C" fn screen_to_world_on_plane(
    r: *mut c_void,
    ndc_x: f32,
    ndc_y: f32,
    plane_z: f32,
    out_position: *mut f32,
) -> u8 {
    // Renderer 또는 출력 buffer가 유효하지 않으면 계산 불가
    if r.is_null() || out_position.is_null() {
        return 0;
    }

    let handle = unsafe { &*r.cast::<RendererHandle>() };
    let renderer = handle
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    // project_3d_to_screen_batch() 및 실제 rendering에서 사용하는 것과
    // 동일한 MVP matrix를 사용해야 화면과 좌표가 정확하게 일치함.
    let mvp = renderer.calculate_mvp();

    // 화면 좌표를 3D 좌표로 되돌리기 위해 MVP 역행렬 계산
    let inverse_mvp = match invert_matrix(mvp) {
        Some(matrix) => matrix,
        None => return 0,
    };

    // OpenGL NDC의 depth 범위:
    //
    // near plane = -1.0
    // far plane  =  1.0
    //
    // 같은 화면 X/Y 위치에서 near와 far 좌표를 각각 생성하면
    // 두 점을 연결하는 3D picking ray를 만들 수 있음.
    let near_clip = [ndc_x, ndc_y, -1.0, 1.0];
    let far_clip = [ndc_x, ndc_y, 1.0, 1.0];

    // clip/NDC 좌표를 object 좌표로 역투영
    let near_world4 = transform_vec4(inverse_mvp, near_clip);
    let far_world4 = transform_vec4(inverse_mvp, far_clip);

    // perspective divide를 수행하려면 w가 0이 아니어야 함.
    if near_world4[3].abs() < f32::EPSILON || far_world4[3].abs() < f32::EPSILON {
        return 0;
    }

    // homogeneous 좌표를 일반 3D 좌표로 변환
    let near_world = [
        near_world4[0] / near_world4[3],
        near_world4[1] / near_world4[3],
        near_world4[2] / near_world4[3],
    ];

    let far_world = [
        far_world4[0] / far_world4[3],
        far_world4[1] / far_world4[3],
        far_world4[2] / far_world4[3],
    ];

    // near point에서 far point로 향하는 ray 방향 계산
    //
    // 평면 교차 계산에서는 방향 vector를 normalize할 필요가 없음.
    let ray_direction = [
        far_world[0] - near_world[0],
        far_world[1] - near_world[1],
        far_world[2] - near_world[2],
    ];

    // ray의 Z 방향 성분이 거의 0이면
    // Z 평면과 평행하므로 교차점을 계산할 수 없음.
    if ray_direction[2].abs() < 1.0e-6 {
        return 0;
    }

    // Ray 식:
    //
    // P(t) = near_world + ray_direction * t
    //
    // P(t).z = plane_z가 되는 t를 계산.
    let distance = (plane_z - near_world[2]) / ray_direction[2];

    // 교차점이 near point의 반대 방향에 있거나
    // 계산 결과가 유효하지 않으면 실패 처리.
    if !distance.is_finite() || distance < 0.0 {
        return 0;
    }

    let intersection = [
        near_world[0] + ray_direction[0] * distance,
        near_world[1] + ray_direction[1] * distance,
        near_world[2] + ray_direction[2] * distance,
    ];

    // NaN이나 infinity가 포함된 결과는 Flutter로 전달하지 않음.
    if !intersection[0].is_finite() || !intersection[1].is_finite() || !intersection[2].is_finite()
    {
        return 0;
    }

    // Dart에서 전달한 Float 배열에 결과 저장
    unsafe {
        *out_position.add(0) = intersection[0];
        *out_position.add(1) = intersection[1];
        *out_position.add(2) = intersection[2];
    }

    1
}

// Windows 전용 render_to_buffer
//
// Flutter Windows plugin의 PixelBufferTexture callback에서 호출하는 함수.
//
// 목적:
// OpenGL로 offscreen FBO에 렌더링하고,
// 그 결과를 RGBA CPU buffer로 복사해서 Flutter에 전달.
//
// 전체 흐름:
//
// 1. WGL context current
// 2. Renderer size 갱신
// 3. OpenGL 초기화 확인
// 4. FBO 준비 또는 재사용
// 5. FBO에 render()
// 6. glReadPixels로 buffer에 RGBA 복사
// 7. Flutter PixelBufferTexture가 이 buffer를 화면에 표시
//
// 주의:
// glReadPixels는 GPU 결과를 CPU로 가져오는 작업이라 느릴 수 있음.
// 그래도 구현 난이도가 낮아서 Windows 1차 backend로는 좋은 선택.
#[cfg(target_os = "windows")]
#[unsafe(no_mangle)]
pub extern "C" fn render_to_buffer(r: *mut c_void, buffer: *mut u8, w: u32, h: u32) {
    if r.is_null() || buffer.is_null() || w == 0 || h == 0 {
        return;
    }

    let handle = unsafe { &*r.cast::<RendererHandle>() };
    let mut re = handle
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    if let Err(error) = re.bind_gl_thread() {
        eprintln!("OpenGL render thread validation failed: {error}");
        return;
    }

    // 현재 thread에 OpenGL context 연결
    //
    // 이게 없으면 아래 gl::* 호출들이 제대로 동작하지 않음.
    if let Err(error) = wgl_helper::make_current() {
        eprintln!("Failed to activate WGL context: {error}");
        return;
    }

    // renderer 내부 width/height 갱신
    //
    // calculate_mvp()에서 aspect ratio 계산에 사용됨.
    re.resize(w, h);

    unsafe {
        // OpenGL function loading, shader compile, VAO/VBO 생성
        //
        // 최초 1회만 수행.
        if let Err(error) = re.ensure_gl_loaded() {
            eprintln!("OpenGL initialization failed: {error}");
            return;
        }

        // offscreen FBO 준비
        //
        // 이전 frame과 크기가 같으면 기존 FBO 재사용.
        // 크기가 바뀌었으면 FBO/texture/depth buffer 재생성.
        if !re.ensure_offscreen_target(w, h) {
            return;
        }

        // 앞으로의 렌더링 대상은 화면이 아니라 offscreen FBO
        gl::BindFramebuffer(gl::FRAMEBUFFER, re.fbo);

        // viewport를 buffer 크기에 맞춤
        gl::Viewport(0, 0, w as i32, h as i32);

        // glReadPixels가 CPU buffer에 데이터를 쓸 때의 정렬 규칙
        //
        // PACK_ALIGNMENT = 1:
        // row padding 없이 byte 단위로 촘촘하게 쓰게 함.
        // Flutter PixelBufferTexture는 RGBA 연속 buffer를 기대하므로 중요.
        gl::PixelStorei(gl::PACK_ALIGNMENT, 1);
        gl::PixelStorei(gl::PACK_ROW_LENGTH, 0);
        gl::PixelStorei(gl::PACK_SKIP_PIXELS, 0);
        gl::PixelStorei(gl::PACK_SKIP_ROWS, 0);

        // line smoothing
        //
        // grid/axis line이 조금 부드럽게 보이도록 함.
        // 드라이버에 따라 효과가 제한적일 수 있음.
        gl::Enable(gl::LINE_SMOOTH);

        // alpha blending
        //
        // 투명 polygon, alpha point 등을 자연스럽게 섞기 위함.
        gl::Enable(gl::BLEND);
        gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);

        // 실제 scene render
        //
        // 이 안에서:
        // - pending point/line/poly data VBO 업로드
        // - clear
        // - grid/axis draw
        // - point cloud draw
        re.render();

        // FBO의 color attachment를 읽도록 지정
        gl::ReadBuffer(gl::COLOR_ATTACHMENT0);

        // FBO 결과를 CPU buffer로 복사
        //
        // buffer는 C++ TextureState.pixels.data()가 넘어온 것.
        // w * h * 4 크기의 RGBA buffer여야 함.
        //
        // 이 함수는 GPU/CPU sync를 만들 수 있어 병목이 될 수 있음.
        gl::ReadPixels(
            0,
            0,
            w as i32,
            h as i32,
            gl::RGBA,
            gl::UNSIGNED_BYTE,
            buffer as *mut c_void,
        );

        // 기본 framebuffer로 복구
        //
        // 이후 다른 OpenGL 코드가 있다면 영향을 줄일 수 있음.
        gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
    }
}

// Android Window Surface 갱신 (JNI)
//
// Android는 앱이 백그라운드로 내려가거나 화면이 회전할 때 기존 Surface를 파괴하고,
// 다시 복귀할 때 새로운 Surface를 생성함.
//
// 동작 방식:
// - surface가 null로 넘어옴: 앱이 백그라운드로 감 -> 현재 EGL Surface 파괴 (GL Context는 유지)
// - 새로운 surface가 넘어옴: 앱 복귀 또는 초기화 -> 기존 Surface 파괴 후 새 Surface 생성 및 연결
//
// 주의:
// EGL Display, Config, Context는 앱이 살아있는 동안 Renderer당 한 번만 생성하여 재사용함.
// Surface만 교체하는 방식이므로, 기존에 VBO/텍스처에 올려둔 GPU 자원은 그대로 유지됨.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
#[allow(non_snake_case)]
pub extern "system" fn Java_io_github_immsong_point_1glass_1opengl_PointGlassOpenglPlugin_nativeSetSurface(
    env: jni::JNIEnv,
    _this: jni::objects::JObject,
    renderer_ptr: jni::sys::jlong,
    surface: jni::objects::JObject,
) {
    use ndk::native_window::NativeWindow;

    if renderer_ptr == 0 {
        return;
    }

    unsafe {
        let handle = &*(renderer_ptr as *const RendererHandle);
        let mut renderer = handle
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        // null Surface가 전달되면 현재 window surface만 해제
        if surface.is_null() {
            let display = renderer.display;
            let old_surface = renderer.egl_surface.take();

            if let (Some(egl), Some(display), Some(old_surface)) =
                (renderer.egl.as_ref(), display, old_surface)
            {
                if let Err(error) = egl.destroy_surface(display, old_surface) {
                    eprintln!("Failed to destroy EGL surface: {error:?}");
                }
            }
            return;
        }

        let native_window =
            match NativeWindow::from_surface(env.get_native_interface(), surface.into_raw()) {
                Some(window) => window,
                None => {
                    eprintln!("Failed to get NativeWindow from Java Surface");
                    return;
                }
            };

        let window_ptr = native_window.ptr().as_ptr();

        // display/config/context는 Renderer당 한 번 생성하고,
        // Android Surface 변경 시 window surface만 교체
        if renderer.egl.is_none() {
            let egl = match khronos_egl::DynamicInstance::<khronos_egl::EGL1_4>::load_required() {
                Ok(egl) => egl,
                Err(error) => {
                    eprintln!("Failed to load EGL: {error:?}");
                    return;
                }
            };

            let display = match egl.get_display(khronos_egl::DEFAULT_DISPLAY) {
                Some(display) => display,
                None => {
                    eprintln!("Failed to get EGL display");
                    return;
                }
            };

            if let Err(error) = egl.initialize(display) {
                eprintln!("Failed to initialize EGL: {error:?}");
                return;
            }

            if let Err(error) = egl.bind_api(khronos_egl::OPENGL_ES_API) {
                eprintln!("Failed to bind OpenGL ES API: {error:?}");
                let _ = egl.terminate(display);
                return;
            }

            let config_attributes = [
                khronos_egl::SURFACE_TYPE,
                khronos_egl::WINDOW_BIT,
                khronos_egl::RENDERABLE_TYPE,
                khronos_egl::OPENGL_ES3_BIT,
                khronos_egl::RED_SIZE,
                8,
                khronos_egl::GREEN_SIZE,
                8,
                khronos_egl::BLUE_SIZE,
                8,
                khronos_egl::ALPHA_SIZE,
                8,
                khronos_egl::DEPTH_SIZE,
                16,
                khronos_egl::NONE,
            ];

            let config = match egl.choose_first_config(display, &config_attributes) {
                Ok(Some(config)) => config,
                Ok(None) => {
                    eprintln!("No OpenGL ES 3 compatible EGL config found");
                    let _ = egl.terminate(display);
                    return;
                }
                Err(error) => {
                    eprintln!("Failed to choose EGL config: {error:?}");
                    let _ = egl.terminate(display);
                    return;
                }
            };

            let context_attributes = [khronos_egl::CONTEXT_CLIENT_VERSION, 3, khronos_egl::NONE];

            let context = match egl.create_context(display, config, None, &context_attributes) {
                Ok(context) => context,
                Err(error) => {
                    eprintln!("Failed to create OpenGL ES 3 context: {error:?}");
                    let _ = egl.terminate(display);
                    return;
                }
            };

            renderer.egl = Some(egl);
            renderer.display = Some(display);
            renderer.context = Some(context);
            renderer.config = Some(config);
        }

        let Some(display) = renderer.display else {
            return;
        };
        let Some(config) = renderer.config else {
            return;
        };
        let old_surface = renderer.egl_surface.take();

        let egl_surface = {
            let Some(egl) = renderer.egl.as_ref() else {
                return;
            };

            // 기존 surface를 덮어쓰기 전에 EGL에 정리 요청.
            // 다른 thread에서 current 상태라면 EGL이 current 해제 후 실제 제거함.
            if let Some(old_surface) = old_surface {
                if let Err(error) = egl.destroy_surface(display, old_surface) {
                    eprintln!("Failed to destroy previous EGL surface: {error:?}");
                }
            }

            match egl.create_window_surface(
                display,
                config,
                window_ptr as khronos_egl::NativeWindowType,
                None,
            ) {
                Ok(surface) => surface,
                Err(error) => {
                    eprintln!("Failed to create EGL window surface: {error:?}");
                    return;
                }
            }
        };

        // make_current는 실제 render_frame이 호출되는 render thread에서 수행하도록 함.
        renderer.egl_surface = Some(egl_surface);
    }
}
