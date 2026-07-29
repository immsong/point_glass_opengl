use super::Renderer;
use std::ffi::c_void;
use std::ptr;

impl Renderer {
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
}
