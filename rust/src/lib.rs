use std::ffi::c_void;
use std::ptr;

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
    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetDC(hWnd: isize) -> isize;

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
    // wglMakeCurrent: 현재 thread에 OpenGL context 연결
    // wglGetProcAddress: 확장 OpenGL/WGL 함수 주소 조회
    #[link(name = "opengl32")]
    unsafe extern "system" {
        fn wglCreateContext(hdc: isize) -> isize;
        fn wglDeleteContext(hglrc: isize) -> i32;

        pub fn wglMakeCurrent(hdc: isize, hglrc: isize) -> i32;
        pub fn wglGetProcAddress(name: *const u8) -> *const c_void;
    }

    // thread_local:
    // OpenGL context는 보통 thread에 묶여 있음.
    // 따라서 thread마다 자기 context를 하나씩 가질 수 있게 저장.
    //
    // 주의:
    // Flutter texture callback이 여러 thread에서 불리면 context가 thread마다 생길 수 있음.
    // 지금 구조에서는 우선 동작 확인용으로 괜찮지만,
    // 최종 안정화에서는 render thread를 하나로 고정하는 방식이 더 안전할 수 있음.
    thread_local! {
        static CTX: std::cell::RefCell<Option<(isize, isize)>> =
            std::cell::RefCell::new(None);
    }

    pub fn make_current() {
        CTX.with(|ctx| {
            let mut ctx_ref = ctx.borrow_mut();

            // 현재 thread에 OpenGL context가 없으면 새로 생성
            if ctx_ref.is_none() {
                // STATIC class를 이용해서 보이지 않는 dummy window 생성
                // 이 window는 화면 표시 목적이 아니라 WGL context 생성용
                let hwnd = unsafe {
                    CreateWindowExA(
                        0,
                        b"STATIC\0".as_ptr(),
                        b"D\0".as_ptr(),
                        0,
                        0,
                        0,
                        0,
                        0,
                        0,
                        0,
                        0,
                        std::ptr::null_mut(),
                    )
                };

                // dummy window의 device context 획득
                let hdc = unsafe { GetDC(hwnd) };

                // PIXELFORMATDESCRIPTOR를 byte array로 간단 구성
                // 실제 구조체를 정의하지 않고 필요한 위치만 채운 방식
                //
                // pfd[0]  = size
                // pfd[2]  = version
                // pfd[4]  = flags
                // pfd[9]  = color bits
                // pfd[23] = depth bits
                let mut pfd = [0u8; 40];
                pfd[0] = 40;
                pfd[2] = 1;
                pfd[4] = 0x25;
                pfd[9] = 32;
                pfd[23] = 24;

                // device context에 pixel format 설정
                let pf = unsafe { ChoosePixelFormat(hdc, pfd.as_ptr()) };
                unsafe { SetPixelFormat(hdc, pf, pfd.as_ptr()) };

                // 먼저 legacy OpenGL context 생성
                // 이유:
                // wglCreateContextAttribsARB 같은 modern context 생성 함수는
                // 기존 context가 current 상태여야 주소를 가져올 수 있음.
                let temp_ctx = unsafe { wglCreateContext(hdc) };
                unsafe { wglMakeCurrent(hdc, temp_ctx) };

                // 가능하면 OpenGL 3.3 Core Profile context 생성
                let mut final_ctx = temp_ctx;

                let attrib_func =
                    unsafe { wglGetProcAddress(b"wglCreateContextAttribsARB\0".as_ptr()) };

                if !attrib_func.is_null() {
                    let wgl_create_context_attribs_arb: extern "system" fn(
                        isize,
                        isize,
                        *const i32,
                    )
                        -> isize = unsafe { std::mem::transmute(attrib_func) };

                    // OpenGL 3.3 Core Profile 요청
                    let attribs = [
                        0x2091, 3, // WGL_CONTEXT_MAJOR_VERSION_ARB = 3
                        0x2092, 3, // WGL_CONTEXT_MINOR_VERSION_ARB = 3
                        0x9126, 0x00000002, // WGL_CONTEXT_PROFILE_MASK_ARB = CORE
                        0,
                    ];

                    let modern_ctx = wgl_create_context_attribs_arb(hdc, 0, attribs.as_ptr());

                    // modern context 생성 성공 시 legacy context 제거 후 교체
                    if modern_ctx != 0 {
                        unsafe { wglMakeCurrent(0, 0) };
                        unsafe { wglDeleteContext(temp_ctx) };
                        unsafe { wglMakeCurrent(hdc, modern_ctx) };

                        final_ctx = modern_ctx;
                    }
                }

                // 현재 thread에 context 저장
                *ctx_ref = Some((hdc, final_ctx));
            }

            // 저장된 context를 현재 thread의 current context로 설정
            let (hdc, hglrc) = ctx_ref.unwrap();
            unsafe { wglMakeCurrent(hdc, hglrc) };
        });
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
}

impl Renderer {
    pub fn new() -> Self {
        Self {
            gl_loaded: false,

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
        }
    }

    // shader 파일 문자열을 OpenGL이 읽기 좋은 형태로 정리
    //
    // 처리 내용:
    // - UTF-8 BOM 제거
    // - Windows CRLF 줄바꿈을 LF로 변환
    // - 앞쪽 공백 제거
    // - #version이 없으면 기본으로 #version 330 core 추가
    //
    // Windows OpenGL driver는 #version 라인에 민감한 편이라
    // 이 정리가 없으면 shader compile error가 나기 쉬움.
    fn clean_shader_source(src: &str) -> String {
        let s = src
            .trim_start_matches('\u{feff}')
            .replace("\r\n", "\n")
            .replace('\r', "\n");

        let s = s.trim_start();

        if s.starts_with("#version") {
            s.to_string()
        } else {
            format!("#version 330 core\n{s}")
        }
    }

    // shader source 하나를 compile
    unsafe fn compile_shader(shader_type: u32, source: &str) -> u32 {
        unsafe {
            let shader = gl::CreateShader(shader_type);
            let c_str = std::ffi::CString::new(source).unwrap();

            // shader source 전달
            gl::ShaderSource(shader, 1, &c_str.as_ptr(), ptr::null());

            // 실제 shader compile
            gl::CompileShader(shader);

            // compile 성공 여부 확인
            let mut success = 0;
            gl::GetShaderiv(shader, gl::COMPILE_STATUS, &mut success);

            // 실패 시 driver가 제공하는 error log 출력
            if success == 0 {
                let mut len = 0;
                gl::GetShaderiv(shader, gl::INFO_LOG_LENGTH, &mut len);

                let mut log = vec![0u8; len as usize];
                gl::GetShaderInfoLog(shader, len, ptr::null_mut(), log.as_mut_ptr() as *mut i8);

                println!(
                    "Shader compile error:\n{}\n--- source preview ---\n{}",
                    String::from_utf8_lossy(&log),
                    source.lines().take(8).collect::<Vec<_>>().join("\n"),
                );
            }

            shader
        }
    }

    // vertex shader + fragment shader를 하나의 program으로 link
    unsafe fn create_program(v_src: &str, f_src: &str) -> u32 {
        unsafe {
            let v_src = Self::clean_shader_source(v_src);
            let f_src = Self::clean_shader_source(f_src);

            let vs = Self::compile_shader(gl::VERTEX_SHADER, &v_src);
            let fs = Self::compile_shader(gl::FRAGMENT_SHADER, &f_src);

            let prog = gl::CreateProgram();

            gl::AttachShader(prog, vs);
            gl::AttachShader(prog, fs);
            gl::LinkProgram(prog);

            // link 성공 여부 확인
            let mut success = 0;
            gl::GetProgramiv(prog, gl::LINK_STATUS, &mut success);

            if success == 0 {
                let mut len = 0;
                gl::GetProgramiv(prog, gl::INFO_LOG_LENGTH, &mut len);

                let mut log = vec![0u8; len as usize];
                gl::GetProgramInfoLog(prog, len, ptr::null_mut(), log.as_mut_ptr() as *mut i8);

                println!("Program link error: {}", String::from_utf8_lossy(&log));
            }

            // program에 attach된 뒤에는 shader object 자체는 삭제해도 됨.
            // program 내부에는 link 결과가 유지됨.
            gl::DeleteShader(vs);
            gl::DeleteShader(fs);

            prog
        }
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
    unsafe fn ensure_gl_loaded(&mut self) {
        if self.gl_loaded {
            return;
        }

        #[cfg(target_os = "linux")]
        unsafe {
            // Linux에서는 libGL / libEGL / RTLD_DEFAULT에서 OpenGL 함수 주소를 탐색
            let libgl = libc::dlopen(b"libGL.so.1\0".as_ptr() as *const _, libc::RTLD_LAZY);
            let libegl = libc::dlopen(b"libEGL.so.1\0".as_ptr() as *const _, libc::RTLD_LAZY);

            gl::load_with(|name| {
                let symbol = std::ffi::CString::new(name).unwrap();

                let mut p = if libgl.is_null() {
                    ptr::null_mut()
                } else {
                    libc::dlsym(libgl, symbol.as_ptr())
                };

                if p.is_null() && !libegl.is_null() {
                    p = libc::dlsym(libegl, symbol.as_ptr());
                }

                if p.is_null() {
                    p = libc::dlsym(libc::RTLD_DEFAULT, symbol.as_ptr());
                }

                p
            });
        }

        #[cfg(target_os = "windows")]
        unsafe {
            // Windows에서는 먼저 WGL context를 current로 만들어야
            // gl::load_with가 정상적으로 OpenGL 함수 주소를 얻을 수 있음.
            wgl_helper::make_current();

            let gl_lib = LoadLibraryA(b"opengl32.dll\0".as_ptr());

            gl::load_with(|name| {
                let symbol = std::ffi::CString::new(name).unwrap();

                // modern OpenGL 함수는 보통 wglGetProcAddress로 얻음.
                let mut p = wgl_helper::wglGetProcAddress(symbol.as_ptr() as *const u8);
                let p_addr = p as usize;

                // 일부 기본 함수는 wglGetProcAddress가 이상한 값이나 null을 줄 수 있음.
                // 그 경우 opengl32.dll의 GetProcAddress로 fallback.
                if (p_addr == 0
                    || p_addr == 1
                    || p_addr == 2
                    || p_addr == 3
                    || p_addr == usize::MAX)
                    && gl_lib != 0
                {
                    p = GetProcAddress(gl_lib, symbol.as_ptr() as *const u8);
                }

                p as *const _
            });
        }

        unsafe {
            // shader 파일을 바이너리에 포함
            const SHADER_POINTS_VERT: &str = include_str!("../shaders/desktop/points.vert");
            const SHADER_POINTS_FRAG: &str = include_str!("../shaders/desktop/points.frag");

            self.shader_points = Self::create_program(SHADER_POINTS_VERT, SHADER_POINTS_FRAG);

            const SHADER_GIZMOS_VERT: &str = include_str!("../shaders/desktop/gizmos.vert");
            const SHADER_GIZMOS_FRAG: &str = include_str!("../shaders/desktop/gizmos.frag");

            self.shader_gizmos = Self::create_program(SHADER_GIZMOS_VERT, SHADER_GIZMOS_FRAG);

            // VAO/VBO 초기화
            Self::setup_buffers_points(&mut self.vao_points, &mut self.vbo_points);
            Self::setup_buffers_color(&mut self.vao_lines, &mut self.vbo_lines);
            Self::setup_buffers_color(&mut self.vao_polys, &mut self.vbo_polys);

            // vertex shader에서 gl_PointSize를 쓸 수 있게 활성화
            gl::Enable(gl::PROGRAM_POINT_SIZE);
        }

        self.gl_loaded = true;
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
        }
    }

    // camera 상태로 MVP matrix 계산
    //
    // MVP = Projection * View * Model
    //
    // Model      : object 자체 회전/이동
    // View       : camera 위치와 방향
    // Projection : 3D를 perspective 화면으로 투영
    fn calculate_mvp(&self) -> [f32; 16] {
        // yaw/pitch/radius를 이용해 camera eye 위치 계산
        let eye_x = self.target_x + self.radius * self.pitch.cos() * self.yaw.sin();
        let eye_y = self.target_y + self.radius * self.pitch.sin();
        let eye_z = self.target_z + self.radius * self.pitch.cos() * self.yaw.cos();

        // camera up vector 계산
        let up_x = -self.pitch.sin() * self.yaw.sin();
        let up_y = self.pitch.cos();
        let up_z = -self.pitch.sin() * self.yaw.cos();

        let view = look_at(
            [eye_x, eye_y, eye_z],
            [self.target_x, self.target_y, self.target_z],
            [up_x, up_y, up_z],
        );

        let aspect = self.width as f32 / self.height.max(1) as f32;

        let proj = perspective(45.0f32.to_radians(), aspect, 1.0, 10_000.0);

        let vp = multiply_matrices(proj, view);

        // roll 회전만 model matrix에 반영
        let cos_z = self.roll.cos();
        let sin_z = self.roll.sin();

        let model = [
            cos_z, sin_z, 0.0, 0.0, -sin_z, cos_z, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ];

        multiply_matrices(vp, model)
    }

    // 실제 draw 함수
    //
    // 이 함수는 현재 OpenGL context와 framebuffer가 이미 준비되어 있다는 전제에서 동작.
    // Windows에서는 render_to_buffer()가 FBO를 bind한 뒤 이 함수를 호출.
    // Linux에서는 Flutter texture/context 쪽에서 준비된 상태에서 호출하는 구조.
    pub fn render(&mut self) {
        unsafe {
            self.ensure_gl_loaded();

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

            let mvp = self.calculate_mvp();

            // ----------------------------------------------------------------
            // Grid / Axis / Polygon 렌더링
            // ----------------------------------------------------------------
            if self.line_count > 0 || self.poly_count > 0 {
                gl::UseProgram(self.shader_gizmos);

                let mvp_loc =
                    gl::GetUniformLocation(self.shader_gizmos, b"uMVP\0".as_ptr() as *const i8);

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

                let mvp_loc =
                    gl::GetUniformLocation(self.shader_points, b"uMVP\0".as_ptr() as *const i8);

                let size_loc = gl::GetUniformLocation(
                    self.shader_points,
                    b"uPointSize\0".as_ptr() as *const i8,
                );

                let min_loc =
                    gl::GetUniformLocation(self.shader_points, b"uMin\0".as_ptr() as *const i8);

                let max_loc =
                    gl::GetUniformLocation(self.shader_points, b"uMax\0".as_ptr() as *const i8);

                let alpha_loc =
                    gl::GetUniformLocation(self.shader_points, b"uAlpha\0".as_ptr() as *const i8);

                let mode_loc = gl::GetUniformLocation(
                    self.shader_points,
                    b"uColorMode\0".as_ptr() as *const i8,
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

// ============================================================================
// FFI
// ============================================================================
//
// FFI = Foreign Function Interface
//
// Dart에서 Rust 함수를 직접 호출하려면,
// Rust 함수가 C ABI 형태로 노출되어야 함.
//
// 그래서 아래 함수들은 전부 이런 형태를 가짐:
//
// #[unsafe(no_mangle)]
// pub extern "C" fn ...
//
// 의미:
//
// no_mangle:
//   Rust compiler가 함수 이름을 이상하게 바꾸지 못하게 함.
//   그래야 Dart에서 "create_renderer" 같은 이름으로 lookup 가능.
//
// extern "C":
//   C ABI 호출 규칙을 사용.
//   Dart FFI, C/C++, Rust 사이에서 함수 호출 규칙을 맞추기 위함.
//
// *mut c_void:
//   Dart 쪽에서는 Rust의 Renderer 타입을 모름.
//   그래서 그냥 void pointer처럼 주소만 주고받음.
//   내부에서는 다시 Renderer pointer로 캐스팅해서 사용.

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
    Box::into_raw(Box::new(Renderer::new())) as *mut c_void
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

    #[cfg(target_os = "windows")]
    wgl_helper::make_current();

    // raw pointer를 다시 Box로 되돌림.
    // 이 함수가 끝나면 Box가 drop되면서 Renderer 메모리도 해제됨.
    let mut renderer = unsafe { Box::from_raw(r as *mut Renderer) };

    // shader, VAO, VBO, FBO 같은 GPU 리소스 정리
    if renderer.gl_loaded {
        unsafe {
            renderer.delete_gl_resources();
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
    if !r.is_null() {
        unsafe { &mut *(r as *mut Renderer) }.render();
    }
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
#[unsafe(no_mangle)]
pub extern "C" fn set_points(r: *mut c_void, d: *const f32, l: usize) {
    if r.is_null() {
        return;
    }

    let re = unsafe { &mut *(r as *mut Renderer) };

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

    let re = unsafe { &mut *(r as *mut Renderer) };

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

    let re = unsafe { &mut *(r as *mut Renderer) };

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

    let re = unsafe { &mut *(r as *mut Renderer) };

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

    let re = unsafe { &mut *(r as *mut Renderer) };

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

    let re = unsafe { &mut *(r as *mut Renderer) };

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

    let re = unsafe { &mut *(r as *mut Renderer) };

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

    let re = unsafe { &*(r as *mut Renderer) };

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

    // 현재 thread에 OpenGL context 연결
    //
    // 이게 없으면 아래 gl::* 호출들이 제대로 동작하지 않음.
    wgl_helper::make_current();

    let re = unsafe { &mut *(r as *mut Renderer) };

    // renderer 내부 width/height 갱신
    //
    // calculate_mvp()에서 aspect ratio 계산에 사용됨.
    re.resize(w, h);

    unsafe {
        // OpenGL function loading, shader compile, VAO/VBO 생성
        //
        // 최초 1회만 수행.
        re.ensure_gl_loaded();

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
