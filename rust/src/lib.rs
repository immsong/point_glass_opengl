use std::ffi::c_void;
use std::ptr;

// 1. 위치, 색상, 크기를 모두 배열에서 받아오는 Vertex Shader
const VERTEX_SHADER_SOURCE: &str = "#version 300 es\n\
    layout (location = 0) in vec3 aPos;\n\
    layout (location = 1) in vec4 aColor;\n\
    layout (location = 2) in float aSize;\n\
    uniform mat4 uMVP;\n\
    out vec4 vColor;\n\
    void main() {\n\
        gl_Position = uMVP * vec4(aPos, 1.0);\n\
        gl_PointSize = aSize;\n\
        vColor = aColor;\n\
    }";

// 2. 전달받은 색상과 투명도(Alpha)를 칠하는 Fragment Shader
const FRAGMENT_SHADER_SOURCE: &str = "#version 300 es\n\
    precision mediump float;\n\
    in vec4 vColor;\n\
    out vec4 FragColor;\n\
    void main() {\n\
        FragColor = vColor;\n\
    }";

pub struct Renderer {
    gl_loaded: bool,
    shader_program: u32,

    // 점, 선, 면 각각의 VAO, VBO 보관
    vao_points: u32,
    vbo_points: u32,
    vao_lines: u32,
    vbo_lines: u32,
    vao_polys: u32,
    vbo_polys: u32,

    // Dart에서 받은 데이터 대기열 (정점 1개당 8개의 f32 사용)
    pending_points: Option<Vec<f32>>,
    point_count: i32,
    pending_lines: Option<Vec<f32>>,
    line_count: i32,
    pending_polys: Option<Vec<f32>>,
    poly_count: i32,

    width: u32,
    height: u32,
    yaw: f32,
    pitch: f32,
    radius: f32,
    roll: f32,
    target_x: f32,
    target_y: f32,
    target_z: f32,
}

impl Renderer {
    pub fn new() -> Self {
        Self {
            gl_loaded: false,
            shader_program: 0,
            vao_points: 0,
            vbo_points: 0,
            vao_lines: 0,
            vbo_lines: 0,
            vao_polys: 0,
            vbo_polys: 0,
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
        shader
    }

    // [X, Y, Z, R, G, B, A, Size] 형태의 Interleaved Buffer 구조를 세팅하는 헬퍼
    unsafe fn setup_buffers(vao: &mut u32, vbo: &mut u32) {
        unsafe { gl::GenVertexArrays(1, vao) };
        unsafe { gl::GenBuffers(1, vbo) };
        unsafe { gl::BindVertexArray(*vao) };
        unsafe { gl::BindBuffer(gl::ARRAY_BUFFER, *vbo) };

        let stride = (8 * std::mem::size_of::<f32>()) as i32;
        // location = 0: aPos (vec3)
        unsafe { gl::VertexAttribPointer(0, 3, gl::FLOAT, gl::FALSE, stride, ptr::null()) };
        unsafe { gl::EnableVertexAttribArray(0) };
        // location = 1: aColor (vec4)
        unsafe {
            gl::VertexAttribPointer(
                1,
                4,
                gl::FLOAT,
                gl::FALSE,
                stride,
                (3 * std::mem::size_of::<f32>()) as *const c_void,
            )
        };
        unsafe { gl::EnableVertexAttribArray(1) };
        // location = 2: aSize (float)
        unsafe {
            gl::VertexAttribPointer(
                2,
                1,
                gl::FLOAT,
                gl::FALSE,
                stride,
                (7 * std::mem::size_of::<f32>()) as *const c_void,
            )
        };
        unsafe { gl::EnableVertexAttribArray(2) };

        unsafe { gl::BindVertexArray(0) };
    }

    fn calculate_mvp(&self) -> [f32; 16] {
        // 1. 기존 카메라 뷰(View) 계산 (렌즈 회전인 roll 제거, 늘 똑바른 자세 유지)
        let eye_x = self.target_x + self.radius * self.pitch.cos() * self.yaw.sin();
        let eye_y = self.target_y + self.radius * self.pitch.sin();
        let eye_z = self.target_z + self.radius * self.pitch.cos() * self.yaw.cos();

        // 기둥(Up) 벡터는 무조건 월드 Y축 기준으로 고정
        let up_x = -self.pitch.sin() * self.yaw.sin();
        let up_y = self.pitch.cos();
        let up_z = -self.pitch.sin() * self.yaw.cos();

        let view = look_at(
            [eye_x, eye_y, eye_z],
            [self.target_x, self.target_y, self.target_z],
            [up_x, up_y, up_z],
        );
        let aspect = self.width as f32 / self.height.max(1) as f32;
        let proj = perspective(45.0f32.to_radians(), aspect, 0.1, 1_000_000.0);

        let vp = multiply_matrices(proj, view);

        // 2. 월드 Z축 회전 행렬 (Model Matrix)
        // Dart에서 받는 roll 값을 이제 "3D 공간 전체의 Z축 회전각"으로 사용합니다.
        let cos_z = self.roll.cos();
        let sin_z = self.roll.sin();
        let model = [
            cos_z, sin_z, 0.0, 0.0, // Column 0
            -sin_z, cos_z, 0.0, 0.0, // Column 1
            0.0, 0.0, 1.0, 0.0, // Column 2
            0.0, 0.0, 0.0, 1.0, // Column 3
        ];

        // 3. 최종 행렬 완성: MVP = Proj * View * Model
        multiply_matrices(vp, model)
    }

    pub fn render(&mut self) {
        unsafe {
            if !self.gl_loaded {
                let libgl = libc::dlopen(b"libGL.so.1\0".as_ptr() as *const _, libc::RTLD_LAZY);
                let libegl = libc::dlopen(b"libEGL.so.1\0".as_ptr() as *const _, libc::RTLD_LAZY);
                gl::load_with(|name| {
                    let symbol = std::ffi::CString::new(name).unwrap();
                    let mut p = libc::dlsym(libgl, symbol.as_ptr());
                    if p.is_null() {
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

                Self::setup_buffers(&mut self.vao_points, &mut self.vbo_points);
                Self::setup_buffers(&mut self.vao_lines, &mut self.vbo_lines);
                Self::setup_buffers(&mut self.vao_polys, &mut self.vbo_polys);

                gl::Enable(gl::PROGRAM_POINT_SIZE);
                gl::Enable(gl::DEPTH_TEST); // 깊이 테스트 켜기 (진짜 3D 공간을 위해)
                gl::Enable(gl::BLEND); // 알파(투명도) 블렌딩 켜기
                gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);

                self.gl_loaded = true;
            }

            // GPU 데이터 업로드 로직 (수신된 데이터가 있을 때만 갱신)
            let upload_data =
                |pending: &mut Option<Vec<f32>>, vao: u32, vbo: u32, count: &mut i32| {
                    if let Some(data) = pending.take() {
                        *count = (data.len() / 8) as i32; // 정점당 8 float
                        gl::BindVertexArray(vao);
                        gl::BindBuffer(gl::ARRAY_BUFFER, vbo);
                        gl::BufferData(
                            gl::ARRAY_BUFFER,
                            (data.len() * 4) as isize,
                            data.as_ptr() as *const c_void,
                            gl::STATIC_DRAW,
                        );
                    }
                };
            upload_data(
                &mut self.pending_points,
                self.vao_points,
                self.vbo_points,
                &mut self.point_count,
            );
            upload_data(
                &mut self.pending_lines,
                self.vao_lines,
                self.vbo_lines,
                &mut self.line_count,
            );
            upload_data(
                &mut self.pending_polys,
                self.vao_polys,
                self.vbo_polys,
                &mut self.poly_count,
            );

            gl::ClearColor(0.1, 0.1, 0.1, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT); // Depth 버퍼도 함께 Clear

            if self.point_count > 0 || self.line_count > 0 || self.poly_count > 0 {
                gl::UseProgram(self.shader_program);
                let mvp = self.calculate_mvp();
                let mvp_loc =
                    gl::GetUniformLocation(self.shader_program, b"uMVP\0".as_ptr() as *const i8);
                gl::UniformMatrix4fv(mvp_loc, 1, gl::FALSE, mvp.as_ptr());

                // 1. 면(Polygons) 먼저 그리기
                if self.poly_count > 0 {
                    gl::BindVertexArray(self.vao_polys);
                    gl::DrawArrays(gl::TRIANGLES, 0, self.poly_count);
                }
                // 2. 선(Lines) 그리기
                if self.line_count > 0 {
                    gl::BindVertexArray(self.vao_lines);
                    gl::DrawArrays(gl::LINES, 0, self.line_count);
                }
                // 3. 점(Points) 그리기
                if self.point_count > 0 {
                    gl::BindVertexArray(self.vao_points);
                    gl::DrawArrays(gl::POINTS, 0, self.point_count);
                }

                gl::BindVertexArray(0);
                gl::UseProgram(0);
            }
        }
    }
}

// --- 4x4 행렬 연산 ---
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
    if !renderer_ptr.is_null() {
        unsafe { &mut *(renderer_ptr as *mut Renderer) }.render();
    }
}

// 새로운 3개의 데이터 입력 인터페이스 (점, 선, 면)
#[unsafe(no_mangle)]
pub extern "C" fn set_points(renderer_ptr: *mut c_void, data_ptr: *const f32, length: usize) {
    if !renderer_ptr.is_null() && !data_ptr.is_null() {
        unsafe { &mut *(renderer_ptr as *mut Renderer) }.pending_points =
            Some(unsafe { std::slice::from_raw_parts(data_ptr, length) }.to_vec());
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn set_lines(renderer_ptr: *mut c_void, data_ptr: *const f32, length: usize) {
    if !renderer_ptr.is_null() && !data_ptr.is_null() {
        unsafe { &mut *(renderer_ptr as *mut Renderer) }.pending_lines =
            Some(unsafe { std::slice::from_raw_parts(data_ptr, length) }.to_vec());
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn set_polygons(renderer_ptr: *mut c_void, data_ptr: *const f32, length: usize) {
    if !renderer_ptr.is_null() && !data_ptr.is_null() {
        unsafe { &mut *(renderer_ptr as *mut Renderer) }.pending_polys =
            Some(unsafe { std::slice::from_raw_parts(data_ptr, length) }.to_vec());
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn update_camera(
    renderer_ptr: *mut c_void,
    yaw: f32,
    pitch: f32,
    roll: f32,
    radius: f32,
) {
    if !renderer_ptr.is_null() {
        let r = unsafe { &mut *(renderer_ptr as *mut Renderer) };
        r.yaw = yaw;
        r.pitch = pitch;
        r.roll = roll;
        r.radius = radius.max(0.1_f32);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn resize_renderer(renderer_ptr: *mut c_void, width: u32, height: u32) {
    if !renderer_ptr.is_null() {
        let r = unsafe { &mut *(renderer_ptr as *mut Renderer) };
        r.width = width.max(1);
        r.height = height.max(1);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn pan_camera(renderer_ptr: *mut c_void, dx: f32, dy: f32) {
    if !renderer_ptr.is_null() {
        let r = unsafe { &mut *(renderer_ptr as *mut Renderer) };
        let right_x = r.yaw.cos();
        let right_z = -r.yaw.sin();
        let up_x = -r.pitch.sin() * r.yaw.sin();
        let up_y = r.pitch.cos();
        let up_z = -r.pitch.sin() * r.yaw.cos();
        let scale = r.radius * 0.001;
        r.target_x += (right_x * dx - up_x * dy) * scale;
        r.target_y += (-up_y * dy) * scale;
        r.target_z += (right_z * dx - up_z * dy) * scale;
    }
}
