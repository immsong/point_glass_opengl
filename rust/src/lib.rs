use std::ffi::c_void;
use std::ptr;

// 위치, 색상, 크기를 하나의 배열에서 받아오는 Vertex Shader
// aPos: 정점의 3D 좌표 (X, Y, Z)
// aColor: 정점의 색상 및 투명도 (R, G, B, A)
// aSize: 점의 크기 (gl_PointSize 로 전달됨)
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

// 전달받은 색상과 투명도를 화면에 칠하는 Fragment Shader
const FRAGMENT_SHADER_SOURCE: &str = "#version 300 es\n\
    precision mediump float;\n\
    in vec4 vColor;\n\
    out vec4 FragColor;\n\
    void main() {\n\
        FragColor = vColor;\n\
    }";

// 렌더링에 필요한 모든 상태를 저장하는 핵심 구조체
pub struct Renderer {
    gl_loaded: bool,
    shader_program: u32,

    // 점, 선, 면 각각의 VAO(정점 배열 객체), VBO(정점 버퍼 객체)
    // 3가지 데이터를 독립적으로 관리하여 렌더링 성능을 최적화합니다.
    vao_points: u32,
    vbo_points: u32,
    vao_lines: u32,
    vbo_lines: u32,
    vao_polys: u32,
    vbo_polys: u32,

    // Dart로부터 FFI를 통해 전달받은 데이터 대기열
    // 렌더링 루프가 돌 때 이 데이터가 존재하면 GPU 버퍼로 업로드됩니다.
    pending_points: Option<Vec<f32>>,
    point_count: i32,
    pending_lines: Option<Vec<f32>>,
    line_count: i32,
    pending_polys: Option<Vec<f32>>,
    poly_count: i32,

    // 화면 해상도 및 카메라 상태
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

    // 셰이더 코드를 컴파일하는 내부 헬퍼 함수
    unsafe fn compile_shader(shader_type: u32, source: &str) -> u32 {
        let shader = unsafe { gl::CreateShader(shader_type) };
        let c_str = std::ffi::CString::new(source).unwrap();
        unsafe { gl::ShaderSource(shader, 1, &c_str.as_ptr(), ptr::null()) };
        unsafe { gl::CompileShader(shader) };
        shader
    }

    // Interleaved Buffer(교차 버퍼) 구조를 세팅하는 함수
    // 데이터 포맷: [X, Y, Z, R, G, B, A, Size] (정점당 8개의 float, 총 32바이트)
    // 객체를 따로 만들지 않고 1차원 배열로 압축하여 GPU 업로드 병목을 최소화합니다.
    unsafe fn setup_buffers(vao: &mut u32, vbo: &mut u32) {
        unsafe { gl::GenVertexArrays(1, vao) };
        unsafe { gl::GenBuffers(1, vbo) };
        unsafe { gl::BindVertexArray(*vao) };
        unsafe { gl::BindBuffer(gl::ARRAY_BUFFER, *vbo) };

        let stride = (8 * std::mem::size_of::<f32>()) as i32;

        // Location 0: aPos (vec3) - 오프셋 0
        unsafe { gl::VertexAttribPointer(0, 3, gl::FLOAT, gl::FALSE, stride, ptr::null()) };
        unsafe { gl::EnableVertexAttribArray(0) };

        // Location 1: aColor (vec4) - 오프셋 3 (X, Y, Z 이후)
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

        // Location 2: aSize (float) - 오프셋 7 (X, Y, Z, R, G, B, A 이후)
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

    // Model, View, Projection 행렬을 계산하고 곱하는 함수
    fn calculate_mvp(&self) -> [f32; 16] {
        // 1. View Matrix (카메라 위치 및 바라보는 방향)
        // roll 연산을 배제하여 카메라 자체가 기울어지지 않도록 고정합니다.
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

        // 2. Projection Matrix (원근감 처리)
        let aspect = self.width as f32 / self.height.max(1) as f32;
        let proj = perspective(45.0f32.to_radians(), aspect, 0.1, 1_000_000.0);

        let vp = multiply_matrices(proj, view);

        // 3. Model Matrix (월드 공간 회전)
        // Dart에서 전달받은 roll 값을 사용하여 3D 세상(Z축 기준) 전체를 턴테이블처럼 회전시킵니다.
        let cos_z = self.roll.cos();
        let sin_z = self.roll.sin();
        let model = [
            cos_z, sin_z, 0.0, 0.0, -sin_z, cos_z, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ];

        // 최종 행렬은 Proj * View * Model 순서로 결합됩니다.
        multiply_matrices(vp, model)
    }

    pub fn render(&mut self) {
        unsafe {
            // 최초 1회 OpenGL 함수 포인터 로드 및 셰이더, 버퍼 초기화
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
                gl::Enable(gl::DEPTH_TEST);
                gl::Enable(gl::BLEND);
                gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);

                self.gl_loaded = true;
            }

            // 수신된 대기열(pending) 데이터가 있으면 GPU 메모리(VBO)로 업로드합니다.
            let upload_data =
                |pending: &mut Option<Vec<f32>>, vao: u32, vbo: u32, count: &mut i32| {
                    if let Some(data) = pending.take() {
                        *count = (data.len() / 8) as i32;
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

            // 화면을 지우기 전에 모든 쓰기 마스크를 해제합니다.
            // 이전 프레임에서 폴리곤을 그리며 꺼두었던 DepthMask로 인해 화면이 지워지지 않는 버그를 방지합니다.
            gl::DepthMask(gl::TRUE);
            gl::ColorMask(gl::TRUE, gl::TRUE, gl::TRUE, gl::TRUE);

            gl::ClearColor(0.1, 0.1, 0.1, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);

            if self.point_count > 0 || self.line_count > 0 || self.poly_count > 0 {
                gl::UseProgram(self.shader_program);

                // 블렌딩 모드 설정
                // 색상은 자연스럽게 섞되, 도화지(FBO)의 알파값은 1.0으로 유지하여
                // Flutter 배경(검은색)이 투과되어 화면이 짙어지는 현상을 방지합니다.
                gl::Enable(gl::BLEND);
                gl::BlendFuncSeparate(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA, gl::ZERO, gl::ONE);

                let mvp = self.calculate_mvp();
                let mvp_loc =
                    gl::GetUniformLocation(self.shader_program, b"uMVP\0".as_ptr() as *const i8);
                gl::UniformMatrix4fv(mvp_loc, 1, gl::FALSE, mvp.as_ptr());

                // 불투명한 객체(선, 점)를 먼저 그립니다.
                if self.line_count > 0 {
                    gl::BindVertexArray(self.vao_lines);
                    gl::DrawArrays(gl::LINES, 0, self.line_count);
                }

                if self.point_count > 0 {
                    gl::BindVertexArray(self.vao_points);
                    gl::DrawArrays(gl::POINTS, 0, self.point_count);
                }

                // 반투명한 객체(면)는 가장 마지막에 그립니다.
                // 이때 반투명한 면끼리 Z-버퍼 충돌(Z-fighting)이 일어나 서로 가려지는 것을 막기 위해 DepthMask를 임시로 끕니다.
                if self.poly_count > 0 {
                    gl::DepthMask(gl::FALSE);
                    gl::BindVertexArray(self.vao_polys);
                    gl::DrawArrays(gl::TRIANGLES, 0, self.poly_count);
                    gl::DepthMask(gl::TRUE);
                }

                gl::BindVertexArray(0);
                gl::UseProgram(0);
            }
        }
    }
}

// 4x4 행렬 연산 유틸리티 (Column-major 기반)
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

// FFI 인터페이스 노출 (C ABI)
// Dart 측에서 호출할 수 있도록 맹글링을 방지하고 메모리 포인터를 교환합니다.

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

        // 화면 이동 비율 조절 (radius에 비례하여 자연스러운 패닝 구현)
        let scale = r.radius * 0.001;
        r.target_x += (right_x * dx - up_x * dy) * scale;
        r.target_y += (-up_y * dy) * scale;
        r.target_z += (right_z * dx - up_z * dy) * scale;
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn project_3d_to_screen_batch(
    renderer_ptr: *mut c_void,
    in_coords: *const f32, // [x1, y1, z1, x2, y2, z2, ...] (Dart에서 넘겨주는 3D 좌표들)
    count: usize,          // 점의 개수
    out_coords: *mut f32,  // [ndc_x1, ndc_y1, ndc_x2, ndc_y2, ...] (Rust가 적어줄 2D 비율)
) {
    if renderer_ptr.is_null() || in_coords.is_null() || out_coords.is_null() || count == 0 {
        return;
    }

    let r = unsafe { &*(renderer_ptr as *mut Renderer) };
    let mvp = r.calculate_mvp(); // 카메라 매트릭스는 한 번만 계산!

    for i in 0..count {
        let in_idx = i * 3;
        let out_idx = i * 2;

        let obj_x = unsafe { *in_coords.add(in_idx) };
        let obj_y = unsafe { *in_coords.add(in_idx + 1) };
        let obj_z = unsafe { *in_coords.add(in_idx + 2) };

        let clip_x = obj_x * mvp[0] + obj_y * mvp[4] + obj_z * mvp[8] + mvp[12];
        let clip_y = obj_x * mvp[1] + obj_y * mvp[5] + obj_z * mvp[9] + mvp[13];
        let clip_w = obj_x * mvp[3] + obj_y * mvp[7] + obj_z * mvp[11] + mvp[15];

        // 카메라 등 뒤로 넘어간 점 방어 로직
        if clip_w < 0.1 {
            unsafe {
                *out_coords.add(out_idx) = -10000.0;
                *out_coords.add(out_idx + 1) = -10000.0;
            }
        } else {
            // 순수 화면 비율(NDC) 저장
            unsafe {
                *out_coords.add(out_idx) = clip_x / clip_w;
                *out_coords.add(out_idx + 1) = clip_y / clip_w;
            }
        }
    }
}
