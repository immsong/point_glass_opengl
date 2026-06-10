import 'package:flutter_test/flutter_test.dart';
import 'package:point_glass_opengl/point_glass_opengl.dart';

void main() {
  // 실제 OpenGL 컨텍스트나 Rust FFI 라이브러리를 로드하지 않고,
  // Dart 단의 컨트롤러 객체가 정상적으로 생성되는지만 확인하는 형식적인 테스트입니다.
  test('PointGlassController instantiation test', () {
    final controller = PointGlassController();

    // 객체가 정상적으로 생성되었는지 확인
    expect(controller, isNotNull);

    // 초기 카메라 세팅값이 정상적으로 들어가 있는지 확인
    expect(controller.radius, 8.0);
    expect(controller.roll, 0.0);
  });
}
