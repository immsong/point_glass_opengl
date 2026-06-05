use std::ffi::c_void;
use std::ptr;

// 1. Vertex Shader
const VERTEX_SHADER_SOURCE: &str = "#version 300 es\n\
    layout (location = 0) in vec3 aPos;\n\
    uniform mat4 uMVP;\n\
    void main() {\n\
        gl_Position = uMVP * vec4(aPos, 1.0);\n\
    }";

// 2. 흰색 점을 칠하는 Fragment Shader
const FRAGMENT_SHADER_SOURCE: &str = "#version 300 es\n\
    precision mediump float;\n\
    out vec4 FragColor;\n\
    \n\
    void main() {\n\
        FragColor = vec4(1.0, 1.0, 1.0, 1.0);\n\
    }";

pub struct Renderer {
    gl_loaded: bool,
    shader_program: u32,
    vao: u32,
    vbo: u32,
    pending_points: Option<Vec<f32>>,
    point_count: i32,

    // 뷰포트 크기 (aspect ratio 계산용)
    width: u32,
    height: u32,

    // 카메라 파라미터 (Orbit Camera: 회전각, 상하각, 거리)
    yaw: f32,
    pitch: f32,
    radius: f32,
    roll: f32,     // Z축 회전 (Ctrl+드래그)
    target_x: f32, // pan 기준점
    target_y: f32,
    target_z: f32,
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
            width: 1,
            height: 1,
            yaw: 0.0,
            pitch: 0.0,
            radius: 8.0,
            roll: 0.0,
            target_x: 0.0,
            target_y: 0.0,
            target_z: 0.0,
        }
    }

    unsafe fn compile_shader(shader_type: u32, source: &str) -> u32 {
        let shader = unsafe { gl::CreateShader(shader_type) };
        let c_str = std::ffi::CString::new(source).unwrap();
        unsafe { gl::ShaderSource(shader, 1, &c_str.as_ptr(), ptr::null()) };
        unsafe { gl::CompileShader(shader) };

        // 컴파일 에러 확인
        let mut success: i32 = 0;
        unsafe { gl::GetShaderiv(shader, gl::COMPILE_STATUS, &mut success) };
        if success == 0 {
            let mut log_len: i32 = 0;
            unsafe { gl::GetShaderiv(shader, gl::INFO_LOG_LENGTH, &mut log_len) };
            let mut log = vec![0u8; log_len as usize];
            unsafe {
                gl::GetShaderInfoLog(
                    shader,
                    log_len,
                    ptr::null_mut(),
                    log.as_mut_ptr() as *mut i8,
                )
            };
            println!(
                "[Rust] Shader compile error: {}",
                String::from_utf8_lossy(&log)
            );
        }
        shader
    }

    // 2. 카메라 위치를 기반으로 MVP 행렬 계산
    fn calculate_mvp(&self) -> [f32; 16] {
        // 구면 좌표계를 이용해 카메라의 X, Y, Z 위치 계산
        let eye_x = self.target_x + self.radius * self.pitch.cos() * self.yaw.sin();
        let eye_y = self.target_y + self.radius * self.pitch.sin();
        let eye_z = self.target_z + self.radius * self.pitch.cos() * self.yaw.cos();

        // base up vector (gimbal lock 없음)
        let base_up_x = -self.pitch.sin() * self.yaw.sin();
        let base_up_y = self.pitch.cos();
        let base_up_z = -self.pitch.sin() * self.yaw.cos();

        // camera right vector (right_y = 0)
        let right_x = self.yaw.cos();
        let right_z = -self.yaw.sin();

        // roll 적용: up = cos(roll)*base_up + sin(roll)*right
        let cos_r = self.roll.cos();
        let sin_r = self.roll.sin();
        let up_x = cos_r * base_up_x + sin_r * right_x;
        let up_y = cos_r * base_up_y; // right_y = 0
        let up_z = cos_r * base_up_z + sin_r * right_z;

        let view = look_at(
            [eye_x, eye_y, eye_z],
            [self.target_x, self.target_y, self.target_z],
            [up_x, up_y, up_z],
        );
        let aspect = self.width as f32 / self.height.max(1) as f32;
        let proj = perspective(45.0f32.to_radians(), aspect, 0.1, 1_000_000.0);

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

                let vertex_shader = Self::compile_shader(gl::VERTEX_SHADER, VERTEX_SHADER_SOURCE);
                let fragment_shader =
                    Self::compile_shader(gl::FRAGMENT_SHADER, FRAGMENT_SHADER_SOURCE);
                self.shader_program = gl::CreateProgram();
                gl::AttachShader(self.shader_program, vertex_shader);
                gl::AttachShader(self.shader_program, fragment_shader);
                gl::LinkProgram(self.shader_program);

                // 링크 에러 확인
                let mut link_ok: i32 = 0;
                gl::GetProgramiv(self.shader_program, gl::LINK_STATUS, &mut link_ok);
                if link_ok == 0 {
                    let mut log_len: i32 = 0;
                    gl::GetProgramiv(self.shader_program, gl::INFO_LOG_LENGTH, &mut log_len);
                    let mut log = vec![0u8; log_len as usize];
                    gl::GetProgramInfoLog(
                        self.shader_program,
                        log_len,
                        ptr::null_mut(),
                        log.as_mut_ptr() as *mut i8,
                    );
                    println!(
                        "[Rust] Program link error: {}",
                        String::from_utf8_lossy(&log)
                    );
                } else {
                    println!("[Rust] Shader program linked successfully.");
                }

                gl::DeleteShader(vertex_shader);
                gl::DeleteShader(fragment_shader);

                gl::GenVertexArrays(1, &mut self.vao);
                gl::GenBuffers(1, &mut self.vbo);
                gl::Enable(gl::PROGRAM_POINT_SIZE);

                self.gl_loaded = true;
            }

            if let Some(points) = self.pending_points.take() {
                self.point_count = (points.len() / 3) as i32;
                gl::BindVertexArray(self.vao);
                gl::BindBuffer(gl::ARRAY_BUFFER, self.vbo);

                // 💡 핵심: STATIC_DRAW(고정) -> STREAM_DRAW(실시간 스트리밍 최적화) 로 변경!
                gl::BufferData(
                    gl::ARRAY_BUFFER,
                    (points.len() * 4) as isize,
                    points.as_ptr() as *const c_void,
                    gl::STREAM_DRAW,
                );

                gl::VertexAttribPointer(0, 3, gl::FLOAT, gl::FALSE, 0, ptr::null());
                gl::EnableVertexAttribArray(0);
            }

            gl::ClearColor(0.1, 0.1, 0.1, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);

            if self.point_count > 0 {
                gl::UseProgram(self.shader_program);
                let mvp = self.calculate_mvp();
                let mvp_loc =
                    gl::GetUniformLocation(self.shader_program, b"uMVP\0".as_ptr() as *const i8);
                gl::UniformMatrix4fv(mvp_loc, 1, gl::FALSE, mvp.as_ptr());

                gl::BindVertexArray(self.vao);
                gl::DrawArrays(gl::LINES, 0, self.point_count);
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
    // column-major: element (row, col) = arr[col*4 + row]
    // C = A*B → C[col*4+row] = sum_k a[k*4+row] * b[col*4+k]
    let mut res = [0.0f32; 16];
    for col in 0..4 {
        for row in 0..4 {
            res[col * 4 + row] = a[0 * 4 + row] * b[col * 4 + 0]
                + a[1 * 4 + row] * b[col * 4 + 1]
                + a[2 * 4 + row] * b[col * 4 + 2]
                + a[3 * 4 + row] * b[col * 4 + 3];
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
    renderer.pitch = pitch.clamp(-std::f32::consts::FRAC_PI_2, std::f32::consts::FRAC_PI_2);
    renderer.radius = radius.max(0.1_f32); // 줌 최소값만 제한
}

#[unsafe(no_mangle)]
pub extern "C" fn resize_renderer(renderer_ptr: *mut c_void, width: u32, height: u32) {
    if renderer_ptr.is_null() {
        return;
    }
    let renderer = unsafe { &mut *(renderer_ptr as *mut Renderer) };
    renderer.width = width.max(1);
    renderer.height = height.max(1);
}

/// Shift+드래그: 카메라 target을 스크린 평면 방향으로 이동
#[unsafe(no_mangle)]
pub extern "C" fn pan_camera(renderer_ptr: *mut c_void, dx: f32, dy: f32) {
    if renderer_ptr.is_null() {
        return;
    }
    let r = unsafe { &mut *(renderer_ptr as *mut Renderer) };
    // camera right: (yaw.cos(), 0, -yaw.sin())
    let right_x = r.yaw.cos();
    let right_z = -r.yaw.sin();
    // camera up (pitch 접선)
    let up_x = -r.pitch.sin() * r.yaw.sin();
    let up_y = r.pitch.cos();
    let up_z = -r.pitch.sin() * r.yaw.cos();
    // 민감도: radius에 비례
    let scale = r.radius * 0.002;
    r.target_x += (right_x * dx - up_x * dy) * scale;
    r.target_y += (-up_y * dy) * scale;
    r.target_z += (right_z * dx - up_z * dy) * scale;
}
