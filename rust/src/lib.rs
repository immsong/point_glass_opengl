use std::ffi::c_void;
use std::ptr;

// -----------------------------------------
// 1. 간단한 점 렌더링용 셰이더 소스 (GLSL ES 3.0 호환)
const VERTEX_SHADER_SOURCE: &str = "#version 300 es\n\
    layout (location = 0) in vec3 aPos;\n\
    void main() {\n\
        gl_Position = vec4(aPos, 1.0);\n\
        gl_PointSize = 3.0;\n\
    }";

const FRAGMENT_SHADER_SOURCE: &str = "#version 300 es\n\
    precision mediump float;\n\
    out vec4 FragColor;\n\
    void main() {\n\
        // 형광 초록색
        FragColor = vec4(0.0, 1.0, 0.0, 1.0);\n\
    }";
// -----------------------------------------

pub struct Renderer {
    gl_loaded: bool,
    shader_program: u32,
    vao: u32,
    vbo: u32,

    // Dart에서 받은 점 데이터 대기열
    pending_points: Option<Vec<f32>>,
    point_count: i32,
}

impl Renderer {
    pub fn new() -> Self {
        println!("[Rust] Renderer created for Point Cloud!");
        Self {
            gl_loaded: false,
            shader_program: 0,
            vao: 0,
            vbo: 0,
            pending_points: None,
            point_count: 0,
        }
    }

    // OpenGL 셰이더 컴파일 헬퍼 함수
    unsafe fn compile_shader(shader_type: u32, source: &str) -> u32 {
        let shader = unsafe { gl::CreateShader(shader_type) };

        // Rust 문자열을 C 스타일(Null-terminated)로 안전하게 변환
        let c_str = std::ffi::CString::new(source).unwrap();
        unsafe { gl::ShaderSource(shader, 1, &c_str.as_ptr(), ptr::null()) };
        unsafe { gl::CompileShader(shader) };

        // 💡 셰이더 컴파일 에러 체크 및 로그 출력
        let mut success = gl::FALSE as i32;
        unsafe { gl::GetShaderiv(shader, gl::COMPILE_STATUS, &mut success) };
        if success == gl::FALSE as i32 {
            let mut info_log = vec![0; 512];
            unsafe {
                gl::GetShaderInfoLog(
                    shader,
                    512,
                    ptr::null_mut(),
                    info_log.as_mut_ptr() as *mut i8,
                )
            };
            let err_msg = unsafe { std::ffi::CStr::from_ptr(info_log.as_ptr() as *const i8) }
                .to_string_lossy();
            println!("[Rust] 🚨 Shader Compile Error: {}", err_msg);
        }

        shader
    }

    pub fn render(&mut self) {
        unsafe {
            // 1. OpenGL 함수 로드 및 초기 셰이더/버퍼 세팅 (최초 1회)
            if !self.gl_loaded {
                let libgl = libc::dlopen(b"libGL.so.1\0".as_ptr() as *const _, libc::RTLD_LAZY);
                let libegl = libc::dlopen(b"libEGL.so.1\0".as_ptr() as *const _, libc::RTLD_LAZY);
                gl::load_with(|name| {
                    let symbol = std::ffi::CString::new(name).unwrap();
                    let mut p = ptr::null_mut();
                    if !libgl.is_null() {
                        p = libc::dlsym(libgl, symbol.as_ptr());
                    }
                    if p.is_null() && !libegl.is_null() {
                        p = libc::dlsym(libegl, symbol.as_ptr());
                    }
                    if p.is_null() {
                        p = libc::dlsym(libc::RTLD_DEFAULT, symbol.as_ptr());
                    }
                    p
                });
                self.gl_loaded = true;

                // 셰이더 프로그램 생성
                let vertex_shader = Self::compile_shader(gl::VERTEX_SHADER, VERTEX_SHADER_SOURCE);
                let fragment_shader =
                    Self::compile_shader(gl::FRAGMENT_SHADER, FRAGMENT_SHADER_SOURCE);
                self.shader_program = gl::CreateProgram();
                gl::AttachShader(self.shader_program, vertex_shader);
                gl::AttachShader(self.shader_program, fragment_shader);
                gl::LinkProgram(self.shader_program);

                // VAO, VBO 생성
                gl::GenVertexArrays(1, &mut self.vao);
                gl::GenBuffers(1, &mut self.vbo);

                // 점 렌더링 허용
                gl::Enable(gl::PROGRAM_POINT_SIZE);
                println!("[Rust] OpenGL Shaders and Buffers initialized.");
            }

            // 2. Dart에서 넘어온 새 데이터가 있다면 GPU(VRAM)로 업로드
            if let Some(points) = self.pending_points.take() {
                self.point_count = (points.len() / 3) as i32; // x,y,z 3개가 1개의 점

                gl::BindVertexArray(self.vao);
                gl::BindBuffer(gl::ARRAY_BUFFER, self.vbo);
                gl::BufferData(
                    gl::ARRAY_BUFFER,
                    (points.len() * std::mem::size_of::<f32>()) as isize,
                    points.as_ptr() as *const c_void,
                    gl::STATIC_DRAW,
                );

                gl::VertexAttribPointer(0, 3, gl::FLOAT, gl::FALSE, 0, ptr::null());
                gl::EnableVertexAttribArray(0);

                println!("[Rust] Uploaded {} points to GPU.", self.point_count);
            }

            // 3. 실제 화면 그리기
            gl::ClearColor(0.1, 0.1, 0.1, 1.0); // 배경을 어두운 회색으로 변경
            gl::Clear(gl::COLOR_BUFFER_BIT);

            if self.point_count > 0 {
                gl::UseProgram(self.shader_program);
                gl::BindVertexArray(self.vao);
                gl::DrawArrays(gl::POINTS, 0, self.point_count);
            }
        }
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

// -----------------------------------------
// 새로운 FFI 함수: Dart에서 3D 점 배열을 전달받음
// -----------------------------------------
#[unsafe(no_mangle)]
pub extern "C" fn set_points(renderer_ptr: *mut c_void, data_ptr: *const f32, length: usize) {
    if renderer_ptr.is_null() || data_ptr.is_null() {
        return;
    }
    let renderer = unsafe { &mut *(renderer_ptr as *mut Renderer) };

    // C 배열(포인터)을 Rust의 안전한 Vec<f32>으로 복사
    let slice = unsafe { std::slice::from_raw_parts(data_ptr, length) };
    renderer.pending_points = Some(slice.to_vec());
}
