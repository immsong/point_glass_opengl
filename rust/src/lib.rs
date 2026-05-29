use std::ffi::c_void;
use std::ptr;

// 1. MVP 행렬(uMVP)이 추가된 셰이더
const VERTEX_SHADER_SOURCE: &str = "#version 300 es\n\
    layout (location = 0) in vec3 aPos;\n\
    uniform mat4 uMVP;\n\
    void main() {\n\
        // 점의 원래 좌표(aPos)에 MVP 행렬을 곱해 3D 공간으로 변환합니다.
        gl_Position = uMVP * vec4(aPos, 1.0);\n\
        gl_PointSize = 2.0;\n\
    }";

const FRAGMENT_SHADER_SOURCE: &str = "#version 300 es\n\
    precision mediump float;\n\
    out vec4 FragColor;\n\
    void main() {\n\
        FragColor = vec4(0.0, 1.0, 0.0, 1.0);\n\
    }";

pub struct Renderer {
    gl_loaded: bool,
    shader_program: u32,
    vao: u32,
    vbo: u32,
    pending_points: Option<Vec<f32>>,
    point_count: i32,

    // 카메라 파라미터 (Orbit Camera: 회전각, 상하각, 거리)
    yaw: f32,
    pitch: f32,
    radius: f32,
}

impl Renderer {
    pub fn new() -> Self {
        println!("[Rust] Renderer created for 3D Orbit Camera!");
        Self {
            gl_loaded: false,
            shader_program: 0,
            vao: 0,
            vbo: 0,
            pending_points: None,
            point_count: 0,
            yaw: 0.0,
            pitch: 0.0,
            radius: 2.5, // 기본 카메라 거리
        }
    }

    unsafe fn compile_shader(shader_type: u32, source: &str) -> u32 {
        let shader = unsafe { gl::CreateShader(shader_type) };
        let c_str = std::ffi::CString::new(source).unwrap();
        unsafe { gl::ShaderSource(shader, 1, &c_str.as_ptr(), ptr::null()) };
        unsafe { gl::CompileShader(shader) };
        shader
    }

    // 2. 카메라 위치를 기반으로 MVP 행렬 계산
    fn calculate_mvp(&self) -> [f32; 16] {
        // 구면 좌표계를 이용해 카메라의 X, Y, Z 위치 계산
        let eye_x = self.radius * self.pitch.cos() * self.yaw.sin();
        let eye_y = self.radius * self.pitch.sin();
        let eye_z = self.radius * self.pitch.cos() * self.yaw.cos();

        // 뷰(View) 행렬: 카메라가 중앙(0,0,0)을 바라보게 설정
        let view = look_at([eye_x, eye_y, eye_z], [0.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        // 투영(Projection) 행렬: 45도 시야각, 1:1 비율
        let proj = perspective(45.0f32.to_radians(), 1.0, 0.1, 100.0);

        multiply_matrices(proj, view)
    }

    pub fn render(&mut self) {
        unsafe {
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

                let vertex_shader = Self::compile_shader(gl::VERTEX_SHADER, VERTEX_SHADER_SOURCE);
                let fragment_shader =
                    Self::compile_shader(gl::FRAGMENT_SHADER, FRAGMENT_SHADER_SOURCE);
                self.shader_program = gl::CreateProgram();
                gl::AttachShader(self.shader_program, vertex_shader);
                gl::AttachShader(self.shader_program, fragment_shader);
                gl::LinkProgram(self.shader_program);

                gl::GenVertexArrays(1, &mut self.vao);
                gl::GenBuffers(1, &mut self.vbo);
                gl::Enable(gl::PROGRAM_POINT_SIZE);
            }

            if let Some(points) = self.pending_points.take() {
                self.point_count = (points.len() / 3) as i32;
                gl::BindVertexArray(self.vao);
                gl::BindBuffer(gl::ARRAY_BUFFER, self.vbo);
                gl::BufferData(
                    gl::ARRAY_BUFFER,
                    (points.len() * 4) as isize,
                    points.as_ptr() as *const c_void,
                    gl::STATIC_DRAW,
                );
                gl::VertexAttribPointer(0, 3, gl::FLOAT, gl::FALSE, 0, ptr::null());
                gl::EnableVertexAttribArray(0);
            }

            gl::ClearColor(0.1, 0.1, 0.1, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);

            if self.point_count > 0 {
                gl::UseProgram(self.shader_program);

                // 3. 계산된 MVP 행렬을 셰이더로 전송
                let mvp = self.calculate_mvp();
                let mvp_loc =
                    gl::GetUniformLocation(self.shader_program, b"uMVP\0".as_ptr() as *const i8);
                gl::UniformMatrix4fv(mvp_loc, 1, gl::FALSE, mvp.as_ptr());

                gl::BindVertexArray(self.vao);
                gl::DrawArrays(gl::POINTS, 0, self.point_count);
            }
        }
    }
}

// --- 4x4 행렬 연산 헬퍼 함수들 ---
fn look_at(eye: [f32; 3], target: [f32; 3], up: [f32; 3]) -> [f32; 16] {
    let f = {
        let r = [target[0] - eye[0], target[1] - eye[1], target[2] - eye[2]];
        let len = (r[0] * r[0] + r[1] * r[1] + r[2] * r[2]).sqrt();
        [r[0] / len, r[1] / len, r[2] / len]
    };
    let s = {
        let r = [
            f[1] * up[2] - f[2] * up[1],
            f[2] * up[0] - f[0] * up[2],
            f[0] * up[1] - f[1] * up[0],
        ];
        let len = (r[0] * r[0] + r[1] * r[1] + r[2] * r[2]).sqrt();
        [r[0] / len, r[1] / len, r[2] / len]
    };
    let u = [
        s[1] * f[2] - s[2] * f[1],
        s[2] * f[0] - s[0] * f[2],
        s[0] * f[1] - s[1] * f[0],
    ];

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

fn perspective(fovy: f32, aspect: f32, near: f32, far: f32) -> [f32; 16] {
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

fn multiply_matrices(a: [f32; 16], b: [f32; 16]) -> [f32; 16] {
    let mut res = [0.0; 16];
    for i in 0..4 {
        for j in 0..4 {
            res[i * 4 + j] = a[i * 4 + 0] * b[0 * 4 + j]
                + a[i * 4 + 1] * b[1 * 4 + j]
                + a[i * 4 + 2] * b[2 * 4 + j]
                + a[i * 4 + 3] * b[3 * 4 + j];
        }
    }
    res
}

// --- FFI 인터페이스 ---
#[unsafe(no_mangle)]
pub extern "C" fn create_renderer() -> *mut c_void {
    Box::into_raw(Box::new(Renderer::new())) as *mut c_void
}

#[unsafe(no_mangle)]
pub extern "C" fn render_frame(renderer_ptr: *mut c_void) {
    if renderer_ptr.is_null() {
        return;
    }
    unsafe { &mut *(renderer_ptr as *mut Renderer) }.render();
}

#[unsafe(no_mangle)]
pub extern "C" fn set_points(renderer_ptr: *mut c_void, data_ptr: *const f32, length: usize) {
    if renderer_ptr.is_null() || data_ptr.is_null() {
        return;
    }
    let slice = unsafe { std::slice::from_raw_parts(data_ptr, length) };
    unsafe { &mut *(renderer_ptr as *mut Renderer) }.pending_points = Some(slice.to_vec());
}

// Dart에서 카메라 제어 신호를 받을 새로운 FFI 함수
#[unsafe(no_mangle)]
pub extern "C" fn update_camera(renderer_ptr: *mut c_void, yaw: f32, pitch: f32, radius: f32) {
    if renderer_ptr.is_null() {
        return;
    }
    let renderer = unsafe { &mut *(renderer_ptr as *mut Renderer) };
    renderer.yaw = yaw;
    renderer.pitch = pitch.clamp(-1.4, 1.4); // 화면이 뒤집히지 않도록 상하각 제한
    renderer.radius = radius.clamp(0.5, 20.0); // 극단적인 줌 제한
}
