use std::ffi::c_void;
use std::sync::Mutex;

use crate::math::{invert_matrix, transform_vec4};
use crate::renderer::Renderer;

#[cfg(target_os = "windows")]
use crate::renderer::platform;

// ============================================================================
// FFI (Foreign Function Interface)
// ============================================================================

//
// Dart에서 Rust 함수를 호출하기 위한 C ABI 인터페이스.
//
// [Thread Safety 주의사항]
// Dart Isolate 특성상 여러 스레드에서 동시에 FFI를 호출할 수 있어
// Renderer 자체는 Mutex로 감싸서 메모리 충돌(Data Race)을 방지함.
//
// 하지만, OpenGL Context는 OS 스레드(Thread Local)에 강하게 종속됨.
// 즉, Mutex 락을 얻었더라도 GL context가 생성된 스레드와 현재 호출된 스레드가 다르면
// OpenGL 함수 호출 시 크래시가 나거나 무시됨.
// 따라서 UI 렌더링을 담당하는 메인/렌더 스레드 하나에서만 GL 관련 FFI를 호출해야 함.

type RendererHandle = Mutex<Renderer>;

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
    Box::into_raw(Box::new(Mutex::new(Renderer::new()))).cast()
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

    // raw pointer를 다시 Box로 되돌림.
    // 호출자는 destroy와 다른 FFI 호출이 동시에 실행되지 않도록 보장해야 함.
    let handle = unsafe { Box::from_raw(r.cast::<RendererHandle>()) };
    let mut renderer = match handle.into_inner() {
        Ok(renderer) => renderer,
        Err(poisoned) => poisoned.into_inner(),
    };
    let current_thread = std::thread::current().id();
    let is_gl_thread = match renderer.gl_thread_id.as_ref() {
        Some(owner) => owner == &current_thread,
        None => true,
    };
    let mut can_delete_gl = is_gl_thread;

    #[cfg(target_os = "windows")]
    {
        if is_gl_thread {
            if let Err(error) = platform::make_current() {
                eprintln!("Failed to activate WGL context during destroy: {error}");
                can_delete_gl = false;
            }
        } else {
            eprintln!(
                "destroy_renderer was called from a different thread; Windows GL resource deletion is skipped"
            );
        }
    }

    #[cfg(target_os = "android")]
    {
        if is_gl_thread {
            if let (Some(egl), Some(display), Some(surface), Some(context)) = (
                renderer.egl.as_ref(),
                renderer.display,
                renderer.egl_surface,
                renderer.context,
            ) {
                if let Err(error) =
                    egl.make_current(display, Some(surface), Some(surface), Some(context))
                {
                    eprintln!("Failed to activate EGL context during destroy: {error:?}");
                    can_delete_gl = false;
                }
            }
        } else {
            eprintln!(
                "destroy_renderer was called from a different thread; Android GL resource deletion is skipped"
            );
        }
    }

    #[cfg(target_os = "linux")]
    if renderer.gl_loaded && can_delete_gl {
        let has_current_context = unsafe { !gl::GetString(gl::VERSION).is_null() };
        if !has_current_context {
            eprintln!(
                "No current Linux OpenGL context during destroy; GL resource deletion is skipped"
            );
            can_delete_gl = false;
        }
    }

    if renderer.gl_loaded && can_delete_gl {
        unsafe { renderer.delete_gl_resources() };
    }

    #[cfg(target_os = "android")]
    {
        let surface = renderer.egl_surface.take();
        let context = renderer.context.take();
        let display = renderer.display.take();
        let egl = renderer.egl.take();
        renderer.config = None;

        if let (Some(egl), Some(display)) = (egl, display) {
            if let Err(error) = egl.make_current(display, None, None, None) {
                eprintln!("Failed to clear current EGL context: {error:?}");
            }

            if let Some(surface) = surface {
                if let Err(error) = egl.destroy_surface(display, surface) {
                    eprintln!("Failed to destroy EGL surface: {error:?}");
                }
            }

            if let Some(context) = context {
                if let Err(error) = egl.destroy_context(display, context) {
                    eprintln!("Failed to destroy EGL context: {error:?}");
                }
            }

            if let Err(error) = egl.terminate(display) {
                eprintln!("Failed to terminate EGL display: {error:?}");
            }
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
    if r.is_null() {
        return;
    }

    let handle = unsafe { &*r.cast::<RendererHandle>() };
    let mut renderer = handle
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    renderer.render();
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
// 성능 주의:
// 현재는 안전을 위해 Dart 측 메모리를 Rust Vec으로 깊은 복사(Deep Copy)함.
// 수백만 개의 Point(수십~수백 MB)를 매 프레임 업데이트할 경우 메모리 병목이 발생할 수 있음.
// 향후 실시간 스트리밍이 필요하다면 Dart와 Rust가 메모리 버퍼를 공유(Zero-copy)하는 구조로 개선 필요.
#[unsafe(no_mangle)]
pub extern "C" fn set_points(r: *mut c_void, d: *const f32, l: usize) {
    if r.is_null() {
        return;
    }

    let handle = unsafe { &*r.cast::<RendererHandle>() };
    let mut re = handle
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

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

    let handle = unsafe { &*r.cast::<RendererHandle>() };
    let mut re = handle
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

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

    let handle = unsafe { &*r.cast::<RendererHandle>() };
    let mut re = handle
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

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

    let handle = unsafe { &*r.cast::<RendererHandle>() };
    let mut re = handle
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

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

    let handle = unsafe { &*r.cast::<RendererHandle>() };
    let mut re = handle
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    re.resize(w, h);
}

// Renderer 배경색 설정
//
// OpenGL clear color를 설정.
#[unsafe(no_mangle)]
pub extern "C" fn set_clear_color(r: *mut c_void, red: f32, green: f32, blue: f32, alpha: f32) {
    if r.is_null() {
        return;
    }

    let handle = unsafe { &*r.cast::<RendererHandle>() };
    let mut re = handle
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    re.clear_color = [
        red.clamp(0.0, 1.0),
        green.clamp(0.0, 1.0),
        blue.clamp(0.0, 1.0),
        alpha.clamp(0.0, 1.0),
    ];
}

// camera pan
//
// target을 XY 평면에서 이동
//
// dx: 좌우 이동
// dy: 전진/후진
//
// camera yaw/pitch와 관계없이 항상 고정된 월드 좌표계를 기준으로 이동.
// Z축 높이는 변경하지 않음.
#[unsafe(no_mangle)]
pub extern "C" fn pan_camera(r: *mut c_void, dx: f32, dy: f32) {
    if r.is_null() {
        return;
    }

    let handle = unsafe { &*r.cast::<RendererHandle>() };
    let mut re = handle
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    // 멀리 있을수록 같은 입력에서도 이동량을 크게 적용
    let scale = re.radius * 0.001;

    // 좌우 이동
    re.target_x -= dx * scale;
    re.target_y += dy * scale;
    // target_z는 변경하지 않음
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

    let handle = unsafe { &*r.cast::<RendererHandle>() };
    let mut re = handle
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

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

    let handle = unsafe { &*r.cast::<RendererHandle>() };
    let re = handle
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

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

// 화면 좌표를 지정한 Z 평면의 3D 좌표로 변환
//
// Flutter에서 전달한 마우스 화면 좌표를 기준으로
// camera에서 3D 공간으로 향하는 ray를 생성하고,
// 그 ray가 지정된 Z 평면과 만나는 위치를 계산.
//
// 주로 다음 용도로 사용:
//
// - 마우스가 가리키는 grid 좌표 표시
// - Z = 0 바닥 평면의 X/Y 위치 표시
// - 3D viewer 위치 선택
//
// 입력:
//
// r            : Renderer pointer
// ndc_x        : 화면 X 좌표를 -1.0 ~ 1.0으로 변환한 NDC 값
// ndc_y        : 화면 Y 좌표를 -1.0 ~ 1.0으로 변환한 NDC 값
// plane_z      : 교차점을 계산할 Z 평면 높이
// out_position : 계산된 [x, y, z]를 저장할 배열 pointer
//
// 출력:
//
// 1 : 교차점 계산 성공
// 0 : 잘못된 pointer, 역행렬 계산 실패, 평면과 교차하지 않는 경우
//
// 좌표 변환 과정:
//
// 1. 현재 camera 상태로 MVP matrix 계산
// 2. MVP 역행렬 계산
// 3. NDC near/far 좌표를 3D 좌표로 역투영
// 4. near → far 방향으로 ray 생성
// 5. ray와 Z = plane_z 평면의 교차점 계산
#[unsafe(no_mangle)]
pub extern "C" fn screen_to_world_on_plane(
    r: *mut c_void,
    ndc_x: f32,
    ndc_y: f32,
    plane_z: f32,
    out_position: *mut f32,
) -> u8 {
    // Renderer 또는 출력 buffer가 유효하지 않으면 계산 불가
    if r.is_null() || out_position.is_null() {
        return 0;
    }

    let handle = unsafe { &*r.cast::<RendererHandle>() };
    let renderer = handle
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    // project_3d_to_screen_batch() 및 실제 rendering에서 사용하는 것과
    // 동일한 MVP matrix를 사용해야 화면과 좌표가 정확하게 일치함.
    let mvp = renderer.calculate_mvp();

    // 화면 좌표를 3D 좌표로 되돌리기 위해 MVP 역행렬 계산
    let inverse_mvp = match invert_matrix(mvp) {
        Some(matrix) => matrix,
        None => return 0,
    };

    // OpenGL NDC의 depth 범위:
    //
    // near plane = -1.0
    // far plane  =  1.0
    //
    // 같은 화면 X/Y 위치에서 near와 far 좌표를 각각 생성하면
    // 두 점을 연결하는 3D picking ray를 만들 수 있음.
    let near_clip = [ndc_x, ndc_y, -1.0, 1.0];
    let far_clip = [ndc_x, ndc_y, 1.0, 1.0];

    // clip/NDC 좌표를 object 좌표로 역투영
    let near_world4 = transform_vec4(inverse_mvp, near_clip);
    let far_world4 = transform_vec4(inverse_mvp, far_clip);

    // perspective divide를 수행하려면 w가 0이 아니어야 함.
    if near_world4[3].abs() < f32::EPSILON || far_world4[3].abs() < f32::EPSILON {
        return 0;
    }

    // homogeneous 좌표를 일반 3D 좌표로 변환
    let near_world = [
        near_world4[0] / near_world4[3],
        near_world4[1] / near_world4[3],
        near_world4[2] / near_world4[3],
    ];

    let far_world = [
        far_world4[0] / far_world4[3],
        far_world4[1] / far_world4[3],
        far_world4[2] / far_world4[3],
    ];

    // near point에서 far point로 향하는 ray 방향 계산
    //
    // 평면 교차 계산에서는 방향 vector를 normalize할 필요가 없음.
    let ray_direction = [
        far_world[0] - near_world[0],
        far_world[1] - near_world[1],
        far_world[2] - near_world[2],
    ];

    // ray의 Z 방향 성분이 거의 0이면
    // Z 평면과 평행하므로 교차점을 계산할 수 없음.
    if ray_direction[2].abs() < 1.0e-6 {
        return 0;
    }

    // Ray 식:
    //
    // P(t) = near_world + ray_direction * t
    //
    // P(t).z = plane_z가 되는 t를 계산.
    let distance = (plane_z - near_world[2]) / ray_direction[2];

    // 교차점이 near point의 반대 방향에 있거나
    // 계산 결과가 유효하지 않으면 실패 처리.
    if !distance.is_finite() || distance < 0.0 {
        return 0;
    }

    let intersection = [
        near_world[0] + ray_direction[0] * distance,
        near_world[1] + ray_direction[1] * distance,
        near_world[2] + ray_direction[2] * distance,
    ];

    // NaN이나 infinity가 포함된 결과는 Flutter로 전달하지 않음.
    if !intersection[0].is_finite() || !intersection[1].is_finite() || !intersection[2].is_finite()
    {
        return 0;
    }

    // Dart에서 전달한 Float 배열에 결과 저장
    unsafe {
        *out_position.add(0) = intersection[0];
        *out_position.add(1) = intersection[1];
        *out_position.add(2) = intersection[2];
    }

    1
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

    let handle = unsafe { &*r.cast::<RendererHandle>() };
    let mut re = handle
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    if let Err(error) = re.bind_gl_thread() {
        eprintln!("OpenGL render thread validation failed: {error}");
        return;
    }

    // 현재 thread에 OpenGL context 연결
    //
    // 이게 없으면 아래 gl::* 호출들이 제대로 동작하지 않음.
    if let Err(error) = platform::make_current() {
        eprintln!("Failed to activate WGL context: {error}");
        return;
    }

    // renderer 내부 width/height 갱신
    //
    // calculate_mvp()에서 aspect ratio 계산에 사용됨.
    re.resize(w, h);

    unsafe {
        // OpenGL function loading, shader compile, VAO/VBO 생성
        //
        // 최초 1회만 수행.
        if let Err(error) = re.ensure_gl_loaded() {
            eprintln!("OpenGL initialization failed: {error}");
            return;
        }

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

// Android Window Surface 갱신 (JNI)
//
// Android는 앱이 백그라운드로 내려가거나 화면이 회전할 때 기존 Surface를 파괴하고,
// 다시 복귀할 때 새로운 Surface를 생성함.
//
// 동작 방식:
// - surface가 null로 넘어옴: 앱이 백그라운드로 감 -> 현재 EGL Surface 파괴 (GL Context는 유지)
// - 새로운 surface가 넘어옴: 앱 복귀 또는 초기화 -> 기존 Surface 파괴 후 새 Surface 생성 및 연결
//
// 주의:
// EGL Display, Config, Context는 앱이 살아있는 동안 Renderer당 한 번만 생성하여 재사용함.
// Surface만 교체하는 방식이므로, 기존에 VBO/텍스처에 올려둔 GPU 자원은 그대로 유지됨.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
#[allow(non_snake_case)]
pub extern "system" fn Java_io_github_immsong_point_1glass_1opengl_PointGlassOpenglPlugin_nativeSetSurface(
    env: jni::JNIEnv,
    _this: jni::objects::JObject,
    renderer_ptr: jni::sys::jlong,
    surface: jni::objects::JObject,
) {
    use ndk::native_window::NativeWindow;

    if renderer_ptr == 0 {
        return;
    }

    unsafe {
        let handle = &*(renderer_ptr as *const RendererHandle);
        let mut renderer = handle
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        // null Surface가 전달되면 현재 window surface만 해제
        if surface.is_null() {
            let display = renderer.display;
            let old_surface = renderer.egl_surface.take();

            if let (Some(egl), Some(display), Some(old_surface)) =
                (renderer.egl.as_ref(), display, old_surface)
            {
                if let Err(error) = egl.destroy_surface(display, old_surface) {
                    eprintln!("Failed to destroy EGL surface: {error:?}");
                }
            }
            return;
        }

        let native_window =
            match NativeWindow::from_surface(env.get_native_interface(), surface.into_raw()) {
                Some(window) => window,
                None => {
                    eprintln!("Failed to get NativeWindow from Java Surface");
                    return;
                }
            };

        let window_ptr = native_window.ptr().as_ptr();

        // display/config/context는 Renderer당 한 번 생성하고,
        // Android Surface 변경 시 window surface만 교체
        if renderer.egl.is_none() {
            let egl = match khronos_egl::DynamicInstance::<khronos_egl::EGL1_4>::load_required() {
                Ok(egl) => egl,
                Err(error) => {
                    eprintln!("Failed to load EGL: {error:?}");
                    return;
                }
            };

            let display = match egl.get_display(khronos_egl::DEFAULT_DISPLAY) {
                Some(display) => display,
                None => {
                    eprintln!("Failed to get EGL display");
                    return;
                }
            };

            if let Err(error) = egl.initialize(display) {
                eprintln!("Failed to initialize EGL: {error:?}");
                return;
            }

            if let Err(error) = egl.bind_api(khronos_egl::OPENGL_ES_API) {
                eprintln!("Failed to bind OpenGL ES API: {error:?}");
                let _ = egl.terminate(display);
                return;
            }

            let config_attributes = [
                khronos_egl::SURFACE_TYPE,
                khronos_egl::WINDOW_BIT,
                khronos_egl::RENDERABLE_TYPE,
                khronos_egl::OPENGL_ES3_BIT,
                khronos_egl::RED_SIZE,
                8,
                khronos_egl::GREEN_SIZE,
                8,
                khronos_egl::BLUE_SIZE,
                8,
                khronos_egl::ALPHA_SIZE,
                8,
                khronos_egl::DEPTH_SIZE,
                16,
                khronos_egl::NONE,
            ];

            let config = match egl.choose_first_config(display, &config_attributes) {
                Ok(Some(config)) => config,
                Ok(None) => {
                    eprintln!("No OpenGL ES 3 compatible EGL config found");
                    let _ = egl.terminate(display);
                    return;
                }
                Err(error) => {
                    eprintln!("Failed to choose EGL config: {error:?}");
                    let _ = egl.terminate(display);
                    return;
                }
            };

            let context_attributes = [khronos_egl::CONTEXT_CLIENT_VERSION, 3, khronos_egl::NONE];

            let context = match egl.create_context(display, config, None, &context_attributes) {
                Ok(context) => context,
                Err(error) => {
                    eprintln!("Failed to create OpenGL ES 3 context: {error:?}");
                    let _ = egl.terminate(display);
                    return;
                }
            };

            renderer.egl = Some(egl);
            renderer.display = Some(display);
            renderer.context = Some(context);
            renderer.config = Some(config);
        }

        let Some(display) = renderer.display else {
            return;
        };
        let Some(config) = renderer.config else {
            return;
        };
        let old_surface = renderer.egl_surface.take();

        let egl_surface = {
            let Some(egl) = renderer.egl.as_ref() else {
                return;
            };

            // 기존 surface를 덮어쓰기 전에 EGL에 정리 요청.
            // 다른 thread에서 current 상태라면 EGL이 current 해제 후 실제 제거함.
            if let Some(old_surface) = old_surface {
                if let Err(error) = egl.destroy_surface(display, old_surface) {
                    eprintln!("Failed to destroy previous EGL surface: {error:?}");
                }
            }

            match egl.create_window_surface(
                display,
                config,
                window_ptr as khronos_egl::NativeWindowType,
                None,
            ) {
                Ok(surface) => surface,
                Err(error) => {
                    eprintln!("Failed to create EGL window surface: {error:?}");
                    return;
                }
            }
        };

        // make_current는 실제 render_frame이 호출되는 render thread에서 수행하도록 함.
        renderer.egl_surface = Some(egl_surface);
    }
}
