use std::ffi::c_void;

// 렌더링 상태를 관리할 코어 구조체
pub struct Renderer {
    // 임시: 이후에 OpenGL 컨텍스트, 셰이더 프로그램, 버퍼 정보 등이 들어갑니다.
    frame_count: u32,
}

impl Renderer {
    pub fn new() -> Self {
        println!("[Rust] Renderer created!");
        Self { frame_count: 0 }
    }

    pub fn render(&mut self) {
        self.frame_count += 1;
        // 테스트용: 렌더링이 호출되었는지 콘솔로 확인
        println!("[Rust] Rendering frame: {}", self.frame_count);

        // TODO: 여기에 실제 OpenGL glClear(GL_COLOR_BUFFER_BIT) 등을 넣어
        // 화면을 특정 색으로 칠하는 로직이 추가될 예정입니다.
    }
}

// -----------------------------------------
// FFI 바인딩 (Dart/C++에서 호출할 수 있는 함수들)
// -----------------------------------------

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
    // 안전하게 포인터를 구조체 참조로 변환
    let renderer = unsafe { &mut *(renderer_ptr as *mut Renderer) };
    renderer.render();
}
