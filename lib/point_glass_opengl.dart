import 'dart:ffi';
import 'dart:io';

class PointGlassOpengl {
  late int Function() _testConnection;

  PointGlassOpengl() {
    // 플랫폼별 및 테스트 환경에 맞게 동적 라이브러리 로드 경로 설정
    // (추후 CMake 연동 시 이 경로는 플러그인 표준 방식으로 변경됩니다)
    final libPath = _getLibraryPath();
    final dylib = DynamicLibrary.open(libPath);

    // Rust에서 작성한 함수 바인딩
    _testConnection = dylib
        .lookup<NativeFunction<Int32 Function()>>(
          'point_glass_opengl_test_connection',
        )
        .asFunction();
  }

  int testConnection() {
    return _testConnection();
  }

  String _getLibraryPath() {
    // 로컬 'flutter test' 실행 시 rust 폴더 내의 빌드 결과물을 가리키도록 설정
    if (Platform.isWindows) {
      return 'rust/target/debug/point_glass_opengl_core.dll';
    } else {
      return 'rust/target/debug/libpoint_glass_opengl_core.so';
    }
  }
}
