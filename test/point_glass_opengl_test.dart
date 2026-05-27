import 'package:flutter_test/flutter_test.dart';
import 'package:point_glass_opengl/point_glass_opengl.dart';

void main() {
  test('Rust core connection test', () {
    final plugin = PointGlassOpengl();

    // Rust의 point_glass_opengl_test_connection() 함수가 42를 반환하는지 검증
    final result = plugin.testConnection();

    expect(result, 42, reason: 'Dart and Rust FFI connection failed!');
  });
}
