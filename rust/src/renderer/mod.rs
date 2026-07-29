use std::ffi::c_void;
use std::ptr;
use std::thread::ThreadId;

use crate::math::{look_at, multiply_matrices, perspective};

pub(crate) mod platform;
mod shader;

// ============================================================================
// Renderer
// ============================================================================
//
// Renderer는 실제 OpenGL 리소스와 상태를 가지고 있는 구조체.
// Dart 쪽에서는 이 Renderer를 직접 알지 못하고,
// create_renderer()가 반환한 raw pointer만 가지고 있음.
pub struct Renderer {
    // OpenGL 함수 로딩, shader, buffer 초기화 여부
    pub(crate) gl_loaded: bool,

    // OpenGL resource를 생성한 render thread.
    // 같은 Renderer의 GL resource를 다른 thread/context에서 사용하는 것을 방지.
    pub(crate) gl_thread_id: Option<ThreadId>,

    // point cloud용 shader program
    pub(crate) shader_points: u32,

    // grid / axis / polygon용 shader program
    pub(crate) shader_gizmos: u32,

    // point cloud용 VAO/VBO
    pub(crate) vao_points: u32,
    pub(crate) vbo_points: u32,

    // line용 VAO/VBO
    pub(crate) vao_lines: u32,
    pub(crate) vbo_lines: u32,

    // polygon용 VAO/VBO
    pub(crate) vao_polys: u32,
    pub(crate) vbo_polys: u32,

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
    pub(crate) fbo: u32,
    pub(crate) fbo_tex: u32,
    pub(crate) fbo_depth: u32,
    pub(crate) fbo_width: u32,
    pub(crate) fbo_height: u32,

    // Dart에서 넘어온 point 데이터는 바로 GPU에 올리지 않고 pending에 보관.
    // render()가 호출될 때 pending 데이터를 VBO로 업로드.
    pub(crate) pending_points: Option<Vec<f32>>,
    pub(crate) point_count: i32,

    pub(crate) pending_lines: Option<Vec<f32>>,
    pub(crate) line_count: i32,

    pub(crate) pending_polys: Option<Vec<f32>>,
    pub(crate) poly_count: i32,

    // 현재 render target 크기
    pub(crate) width: u32,
    pub(crate) height: u32,

    // camera 회전
    pub(crate) yaw: f32,
    pub(crate) pitch: f32,
    pub(crate) roll: f32,

    // camera 거리
    pub(crate) radius: f32,

    // camera가 바라보는 중심점
    pub(crate) target_x: f32,
    pub(crate) target_y: f32,
    pub(crate) target_z: f32,

    // point cloud 표시 옵션
    pub(crate) alpha: f32,
    pub(crate) point_size: f32,
    pub(crate) value_min: f32,
    pub(crate) value_max: f32,
    pub(crate) color_mode: i32,

    #[cfg(target_os = "android")]
    pub(crate) egl: Option<platform::AndroidEgl>,
    #[cfg(target_os = "android")]
    pub(crate) display: Option<khronos_egl::Display>,
    #[cfg(target_os = "android")]
    pub(crate) egl_surface: Option<khronos_egl::Surface>,
    #[cfg(target_os = "android")]
    pub(crate) context: Option<khronos_egl::Context>,
    #[cfg(target_os = "android")]
    pub(crate) config: Option<khronos_egl::Config>,
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
    pub(crate) fn bind_gl_thread(&mut self) -> Result<(), String> {
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
    pub(crate) unsafe fn ensure_gl_loaded(&mut self) -> Result<(), String> {
        self.bind_gl_thread()?;

        if self.gl_loaded {
            return Ok(());
        }

        unsafe { platform::load_gl_functions()? };

        let shader_profile = unsafe { shader::detect_shader_profile() }?;
        let shaders = shader::shader_sources(shader_profile);

        println!("Selected shader profile: {shader_profile:?}");

        let points_program = unsafe {
            shader::create_program(shaders.points_vert, shaders.points_frag, shader_profile)
        }
        .map_err(|error| format!("Point shader initialization failed: {error}"))?;

        let gizmos_program = match unsafe {
            shader::create_program(shaders.gizmos_vert, shaders.gizmos_frag, shader_profile)
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
            if shader_profile == shader::ShaderProfile::Gl330 {
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
    pub(crate) unsafe fn ensure_offscreen_target(&mut self, width: u32, height: u32) -> bool {
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
    pub(crate) unsafe fn delete_gl_resources(&mut self) {
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
    pub(crate) fn calculate_mvp(&self) -> [f32; 16] {
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
