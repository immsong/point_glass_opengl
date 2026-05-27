// C ABI(Application Binary Interface)를 따르도록 설정하여 Dart에서 호출 가능하게 합니다.
#[unsafe(no_mangle)]
pub extern "C" fn point_glass_opengl_test_connection() -> i32 {
    // 통신이 성공적으로 연결되었음을 확인하기 위한 임의의 숫자 반환
    42
}
