use std::ffi::c_void;
use std::ptr;

// ============================================================================
// Renderer 구조체
// ============================================================================
pub struct Renderer {
    gl_loaded: bool,
    shader_points: u32, // 셰이더 2개로 분리!
    shader_gizmos: u32,

    vao_points: u32,
    vbo_points: u32,
    vao_lines: u32,
    vbo_lines: u32,
    vao_polys: u32,
    vbo_polys: u32,

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
    roll: f32,
    radius: f32,
    target_x: f32,
    target_y: f32,
    target_z: f32,
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

    unsafe fn compile_shader(shader_type: u32, source: &str) -> u32 {
        let shader = unsafe { gl::CreateShader(shader_type) };
        let c_str = std::ffi::CString::new(source).unwrap();
        unsafe { gl::ShaderSource(shader, 1, &c_str.as_ptr(), ptr::null()) };
        unsafe { gl::CompileShader(shader) };
        shader
    }

    unsafe fn create_program(v_src: &str, f_src: &str) -> u32 {
        let vs = unsafe { Self::compile_shader(gl::VERTEX_SHADER, v_src) };
        let fs = unsafe { Self::compile_shader(gl::FRAGMENT_SHADER, f_src) };
        let prog = unsafe { gl::CreateProgram() };
        unsafe { gl::AttachShader(prog, vs) };
        unsafe { gl::AttachShader(prog, fs) };
        unsafe { gl::LinkProgram(prog) };
        prog
    }

    // 1. 포인트용 버퍼 세팅 (4 Floats: X, Y, Z, Value)
    unsafe fn setup_buffers_points(vao: &mut u32, vbo: &mut u32) {
        unsafe { gl::GenVertexArrays(1, vao) };
        unsafe { gl::GenBuffers(1, vbo) };
        unsafe { gl::BindVertexArray(*vao) };
        unsafe { gl::BindBuffer(gl::ARRAY_BUFFER, *vbo) };

        let stride = (4 * std::mem::size_of::<f32>()) as i32;
        unsafe { gl::VertexAttribPointer(0, 3, gl::FLOAT, gl::FALSE, stride, ptr::null()) };
        unsafe { gl::EnableVertexAttribArray(0) };
        unsafe {
            gl::VertexAttribPointer(
                1,
                1,
                gl::FLOAT,
                gl::FALSE,
                stride,
                (3 * std::mem::size_of::<f32>()) as *const c_void,
            )
        };
        unsafe { gl::EnableVertexAttribArray(1) };
        unsafe { gl::BindVertexArray(0) };
    }

    // 2. 컬러 객체(Grid, Axis)용 버퍼 세팅 (8 Floats: X, Y, Z, R, G, B, A, Pad)
    unsafe fn setup_buffers_color(vao: &mut u32, vbo: &mut u32) {
        unsafe { gl::GenVertexArrays(1, vao) };
        unsafe { gl::GenBuffers(1, vbo) };
        unsafe { gl::BindVertexArray(*vao) };
        unsafe { gl::BindBuffer(gl::ARRAY_BUFFER, *vbo) };

        let stride = (8 * std::mem::size_of::<f32>()) as i32;
        unsafe { gl::VertexAttribPointer(0, 3, gl::FLOAT, gl::FALSE, stride, ptr::null()) };
        unsafe { gl::EnableVertexAttribArray(0) };
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
        unsafe { gl::BindVertexArray(0) };
    }

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

        // Z-Buffer 정밀도를 위해 near는 1.0, far는 10000.0 으로 고정
        let proj = perspective(45.0f32.to_radians(), aspect, 1.0, 10_000.0);

        let vp = multiply_matrices(proj, view);
        let cos_z = self.roll.cos();
        let sin_z = self.roll.sin();
        let model = [
            cos_z, sin_z, 0.0, 0.0, -sin_z, cos_z, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ];
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

                // 2개의 셰이더 컴파일
                const SHADER_POINTS_VERT: &str = include_str!("../shaders/points.vert");
                const SHADER_POINTS_FRAG: &str = include_str!("../shaders/points.frag");
                self.shader_points = Self::create_program(SHADER_POINTS_VERT, SHADER_POINTS_FRAG);

                const SHADER_GIZMOS_VERT: &str = include_str!("../shaders/gizmos.vert");
                const SHADER_GIZMOS_FRAG: &str = include_str!("../shaders/gizmos.frag");
                self.shader_gizmos = Self::create_program(SHADER_GIZMOS_VERT, SHADER_GIZMOS_FRAG);

                // 2가지 규격의 버퍼 세팅
                Self::setup_buffers_points(&mut self.vao_points, &mut self.vbo_points);
                Self::setup_buffers_color(&mut self.vao_lines, &mut self.vbo_lines);
                Self::setup_buffers_color(&mut self.vao_polys, &mut self.vbo_polys);

                gl::Enable(gl::PROGRAM_POINT_SIZE);
                self.gl_loaded = true;
            }

            // 3. 업로드 시 데이터 보폭(stride_floats)에 맞춰서 점 개수 계산
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
                        (data.len() * 4) as isize,
                        data_ptr,
                        gl::DYNAMIC_DRAW,
                    );
                }
            };

            // 점은 4칸 단위, 선과 면은 8칸 단위로 업로드!
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

            gl::Enable(gl::DEPTH_TEST);
            gl::DepthFunc(gl::LESS);
            gl::DepthMask(gl::TRUE);
            gl::Enable(gl::BLEND);
            gl::BlendFuncSeparate(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA, gl::ZERO, gl::ONE);
            gl::ColorMask(gl::TRUE, gl::TRUE, gl::TRUE, gl::TRUE);

            gl::ClearColor(0.1, 0.1, 0.1, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);

            let mvp = self.calculate_mvp();

            // ---------------------------------------------------------
            // Draw 1: Grid와 Axis (Color Shader 사용)
            // ---------------------------------------------------------
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
                    gl::DepthMask(gl::FALSE); // 반투명 겹침 방지
                    gl::BindVertexArray(self.vao_polys);
                    gl::DrawArrays(gl::TRIANGLES, 0, self.poly_count);
                    gl::DepthMask(gl::TRUE);
                }
            }

            // ---------------------------------------------------------
            // Draw 2: Point Cloud (Rainbow Points Shader 사용)
            // ---------------------------------------------------------
            if self.point_count > 0 {
                gl::UseProgram(self.shader_points);
                let mvp_loc =
                    gl::GetUniformLocation(self.shader_points, b"uMVP\0".as_ptr() as *const i8);
                gl::UniformMatrix4fv(mvp_loc, 1, gl::FALSE, mvp.as_ptr());

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

                gl::Uniform1f(size_loc, self.point_size);
                gl::Uniform1f(min_loc, self.value_min);
                gl::Uniform1f(max_loc, self.value_max);
                gl::Uniform1f(alpha_loc, self.alpha);
                gl::Uniform1i(mode_loc, self.color_mode);

                gl::BindVertexArray(self.vao_points);
                gl::DrawArrays(gl::POINTS, 0, self.point_count);
            }

            gl::BindVertexArray(0);
            gl::UseProgram(0);
        }
    }
}

// ============================================================================
// 행렬 연산 및 FFI 함수들 (기존과 완전히 동일하므로 그대로 유지)
// ============================================================================
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

#[unsafe(no_mangle)]
pub extern "C" fn create_renderer() -> *mut c_void {
    Box::into_raw(Box::new(Renderer::new())) as *mut c_void
}

#[unsafe(no_mangle)]
pub extern "C" fn render_frame(r: *mut c_void) {
    if !r.is_null() {
        unsafe { &mut *(r as *mut Renderer) }.render();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn set_points(r: *mut c_void, d: *const f32, l: usize) {
    if !r.is_null() {
        let re = unsafe { &mut *(r as *mut Renderer) };
        if l == 0 || d.is_null() {
            re.pending_points = Some(Vec::new());
        } else {
            re.pending_points = Some(unsafe { std::slice::from_raw_parts(d, l) }.to_vec());
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn set_lines(r: *mut c_void, d: *const f32, l: usize) {
    if !r.is_null() {
        let re = unsafe { &mut *(r as *mut Renderer) };
        if l == 0 || d.is_null() {
            re.pending_lines = Some(Vec::new());
        } else {
            re.pending_lines = Some(unsafe { std::slice::from_raw_parts(d, l) }.to_vec());
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn set_polygons(r: *mut c_void, d: *const f32, l: usize) {
    if !r.is_null() {
        let re = unsafe { &mut *(r as *mut Renderer) };
        if l == 0 || d.is_null() {
            re.pending_polys = Some(Vec::new());
        } else {
            re.pending_polys = Some(unsafe { std::slice::from_raw_parts(d, l) }.to_vec());
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn update_camera(r: *mut c_void, y: f32, p: f32, roll: f32, rad: f32) {
    if !r.is_null() {
        let re = unsafe { &mut *(r as *mut Renderer) };
        re.yaw = y;
        re.pitch = p;
        re.roll = roll;
        re.radius = rad.max(0.1);
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn resize_renderer(r: *mut c_void, w: u32, h: u32) {
    if !r.is_null() {
        let re = unsafe { &mut *(r as *mut Renderer) };
        re.width = w.max(1);
        re.height = h.max(1);
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn pan_camera(r: *mut c_void, dx: f32, dy: f32) {
    if !r.is_null() {
        let re = unsafe { &mut *(r as *mut Renderer) };
        let right_x = re.yaw.cos();
        let right_z = -re.yaw.sin();
        let up_x = -re.pitch.sin() * re.yaw.sin();
        let up_y = re.pitch.cos();
        let up_z = -re.pitch.sin() * re.yaw.cos();
        let scale = re.radius * 0.001;
        re.target_x += (right_x * dx - up_x * dy) * scale;
        re.target_y += (-up_y * dy) * scale;
        re.target_z += (right_z * dx - up_z * dy) * scale;
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn set_point_cloud_display_params(
    r: *mut c_void,
    alpha: f32,
    size: f32,
    min: f32,
    max: f32,
    mode: i32,
) {
    if !r.is_null() {
        let re = unsafe { &mut *(r as *mut Renderer) };
        re.alpha = alpha.clamp(0.0, 1.0);
        re.point_size = size;
        re.value_min = min;
        re.value_max = max;
        re.color_mode = mode;
    }
}

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
    let mvp = re.calculate_mvp();
    for i in 0..count {
        let in_idx = i * 3;
        let out_idx = i * 2;
        let obj_x = unsafe { *in_c.add(in_idx) };
        let obj_y = unsafe { *in_c.add(in_idx + 1) };
        let obj_z = unsafe { *in_c.add(in_idx + 2) };
        let clip_x = obj_x * mvp[0] + obj_y * mvp[4] + obj_z * mvp[8] + mvp[12];
        let clip_y = obj_x * mvp[1] + obj_y * mvp[5] + obj_z * mvp[9] + mvp[13];
        let clip_w = obj_x * mvp[3] + obj_y * mvp[7] + obj_z * mvp[11] + mvp[15];
        if clip_w < 0.1 {
            unsafe {
                *out_c.add(out_idx) = -10000.0;
                *out_c.add(out_idx + 1) = -10000.0;
            }
        } else {
            unsafe {
                *out_c.add(out_idx) = clip_x / clip_w;
                *out_c.add(out_idx + 1) = clip_y / clip_w;
            }
        }
    }
}
